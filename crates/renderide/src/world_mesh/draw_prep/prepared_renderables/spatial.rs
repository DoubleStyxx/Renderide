//! Per-render-space CPU spatial index for prepared renderer runs.

use std::cell::RefCell;

use glam::{Vec3, Vec3A};
use hashbrown::HashMap;

thread_local! {
    static SPATIAL_CANDIDATE_INDICES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static SPATIAL_CANDIDATE_RUNS: RefCell<Vec<FramePreparedRun>> = const { RefCell::new(Vec::new()) };
    static SPATIAL_CANDIDATE_BITS: RefCell<DenseBitSet> = const { RefCell::new(DenseBitSet::new_empty()) };
    static SPATIAL_CASTER_ROWS: RefCell<Vec<StoredWorldCaster>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
use crate::particles::ParticleDrawParams;
use crate::scene::{RenderSpaceId, SceneSpaceRead};
#[cfg(test)]
use crate::shared::ShadowCastMode;
use crate::world_mesh::culling::{PreparedSpaceCull, WorldMeshCullInput};

use super::super::bitset::DenseBitSet;
use super::super::item::WorldMeshVisibilityStats;
use super::{FramePreparedDraw, FramePreparedRun};

const BVH_LEAF_SIZE: usize = 8;
pub(super) const SPATIAL_LINEAR_RUN_LIMIT: usize = 64;
const SPATIAL_DENSE_GATHER_MIN_CANDIDATE_DIVISOR: usize = 2;

/// Spatial query output consumed by per-view prepared draw collection.
pub(in crate::world_mesh::draw_prep) struct PreparedSpatialRunCandidates {
    /// Renderer runs that survived spatial filtering, in original prepared-run order.
    pub(in crate::world_mesh::draw_prep) runs: Vec<FramePreparedRun>,
    /// Slot-level cull counters for runs rejected by the spatial index itself.
    pub(in crate::world_mesh::draw_prep) cull_stats: (usize, usize, usize),
    /// Visibility broadphase counters for this query.
    pub(in crate::world_mesh::draw_prep) visibility: WorldMeshVisibilityStats,
}

impl Drop for PreparedSpatialRunCandidates {
    fn drop(&mut self) {
        recycle_spatial_runs(std::mem::take(&mut self.runs));
    }
}

/// One prepared world caster plus the run whose first draw owns its bounds.
#[derive(Clone, Copy)]
struct StoredWorldCaster {
    run_index: usize,
    bounds: super::PreparedCasterBounds,
}

/// Per-render-space BVH and linear fallback buckets for prepared renderer runs.
#[derive(Default)]
pub(super) struct PreparedSpatialIndex {
    spaces: HashMap<RenderSpaceId, PreparedSpatialSpace>,
    /// Casters grouped by render space, each list sorted by run index.
    ///
    /// A refit rewrites only the spaces that changed. Linear and BVH runs are both included, so
    /// an indexed run whose world AABB disappears and comes back is removed and inserted again.
    casters_by_space: HashMap<RenderSpaceId, Vec<StoredWorldCaster>>,
    /// World casters in global run order, published from [`Self::casters_by_space`].
    stored_world_casters: Vec<StoredWorldCaster>,
}

impl PreparedSpatialIndex {
    /// Rebuilds all per-space spatial data from the current prepared run snapshot.
    pub(super) fn rebuild(&mut self, draws: &[FramePreparedDraw], runs: &[FramePreparedRun]) {
        profiling::scope!("mesh::prepared_renderables::spatial_rebuild");
        self.spaces.clear();
        self.fill_world_casters(draws, runs);
        let mut builders: HashMap<RenderSpaceId, PreparedSpatialSpaceBuilder> = HashMap::new();
        for (run_index, run) in runs.iter().copied().enumerate() {
            let Some(first) = draws.get(run.start as usize) else {
                continue;
            };
            let slot_count = (run.end - run.start) as usize;
            let builder = builders.entry(first.space_id).or_default();
            if let Some((aabb_min, aabb_max)) = indexable_run_bounds(first) {
                builder.indexed.push(IndexedPreparedRun {
                    run_index,
                    aabb_min,
                    aabb_max,
                    center: (aabb_min + aabb_max) * 0.5,
                    slot_count,
                });
            } else {
                builder.linear.push(LinearPreparedRun {
                    run_index,
                    bounds: None,
                    slot_count,
                });
            }
        }
        for (space_id, builder) in builders {
            self.spaces.insert(space_id, builder.finish());
        }
    }

    /// Collects prepared renderer runs for the requested render spaces after optional frustum culling.
    pub(super) fn query_runs(
        &self,
        runs: &[FramePreparedRun],
        space_ids: &[RenderSpaceId],
        scene: &(impl SceneSpaceRead + ?Sized),
        culling: Option<&WorldMeshCullInput<'_>>,
    ) -> PreparedSpatialRunCandidates {
        profiling::scope!("mesh::prepared_renderables::spatial_query");
        let mut candidate_indices = take_spatial_indices();
        let mut raw_candidate_marks = 0usize;
        let mut cull_stats = (0usize, 0usize, 0usize);
        let mut visibility = WorldMeshVisibilityStats::default();
        {
            profiling::scope!("mesh::prepared_renderables::spatial_query_mark_candidates");
            for &space_id in space_ids {
                let Some(space) = self.spaces.get(&space_id) else {
                    continue;
                };
                space.query(
                    space_id,
                    scene,
                    culling,
                    &mut PreparedSpatialQueryOutput {
                        candidates: &mut candidate_indices,
                        raw_candidate_marks: &mut raw_candidate_marks,
                        cull_stats: &mut cull_stats,
                        visibility: &mut visibility,
                    },
                );
            }
        }

        let out = gather_spatial_candidate_runs(runs, &mut candidate_indices);
        recycle_spatial_indices(candidate_indices);
        let candidate_runs = out.len();
        PreparedSpatialRunCandidates {
            runs: out,
            cull_stats,
            visibility: WorldMeshVisibilityStats {
                candidate_runs,
                raw_candidate_marks,
                duplicate_candidate_marks: raw_candidate_marks.saturating_sub(candidate_runs),
                ..visibility
            },
        }
    }

    /// Refits existing per-space spatial bounds without changing run membership or tree topology.
    pub(super) fn refit_spaces<I>(
        &mut self,
        draws: &[FramePreparedDraw],
        runs: &[FramePreparedRun],
        space_ids: I,
    ) -> usize
    where
        I: IntoIterator<Item = RenderSpaceId>,
    {
        let mut refit_count = 0usize;
        let mut membership_changed = false;
        for space_id in space_ids {
            if !self.refit_one_space(draws, runs, space_id) {
                continue;
            }
            membership_changed |= self.rewrite_space_casters(draws, runs, space_id);
            refit_count = refit_count.saturating_add(1);
        }
        if membership_changed {
            self.publish_world_casters();
        }
        refit_count
    }

    fn refit_one_space(
        &mut self,
        draws: &[FramePreparedDraw],
        runs: &[FramePreparedRun],
        space_id: RenderSpaceId,
    ) -> bool {
        let Some(space) = self.spaces.get_mut(&space_id) else {
            return false;
        };
        profiling::scope!("mesh::prepared_renderables::spatial_refit");
        space.refit(draws, runs);
        true
    }

    /// Fills the compact world-caster list from prepared runs without building a BVH.
    ///
    /// The predicate matches the hysteresis caster walk: not overlay, not hidden, and a world
    /// AABB is present. Zero-extent boxes stay in the list. [`Self::rebuild`] calls this before
    /// the spatial buckets, and tests that never build a BVH call it directly.
    pub(super) fn fill_world_casters(
        &mut self,
        draws: &[FramePreparedDraw],
        runs: &[FramePreparedRun],
    ) {
        for bucket in self.casters_by_space.values_mut() {
            bucket.clear();
        }
        for (run_index, run) in runs.iter().copied().enumerate() {
            let Some(first) = draws.get(run.start as usize) else {
                continue;
            };
            let Some(bounds) = prepared_world_caster(first) else {
                continue;
            };
            self.casters_by_space
                .entry(bounds.space_id)
                .or_default()
                .push(StoredWorldCaster { run_index, bounds });
        }
        self.casters_by_space.retain(|_, rows| !rows.is_empty());
        self.publish_world_casters();
    }

    /// Prepared world casters in run order.
    pub(super) fn world_casters(&self) -> impl Iterator<Item = &super::PreparedCasterBounds> {
        self.stored_world_casters.iter().map(|entry| &entry.bounds)
    }

    /// Rebuilds one space's caster rows from its linear and indexed runs.
    ///
    /// Returns whether the set of run indices changed. Unchanged rows patch the published list in
    /// place. A changed set is published once, after every refitted space has been rewritten.
    fn rewrite_space_casters(
        &mut self,
        draws: &[FramePreparedDraw],
        runs: &[FramePreparedRun],
        space_id: RenderSpaceId,
    ) -> bool {
        let mut fresh = take_stored_casters();
        if let Some(space) = self.spaces.get(&space_id) {
            push_space_casters(
                &mut fresh,
                draws,
                runs,
                space_id,
                &space.linear,
                &space.indexed,
            );
        }
        fresh.sort_unstable_by_key(|entry| entry.run_index);
        fresh.dedup_by_key(|entry| entry.run_index);
        let membership_changed =
            store_space_caster_rows(&mut self.casters_by_space, space_id, &mut fresh);
        if !membership_changed {
            patch_stored_caster_bounds(&mut self.stored_world_casters, &fresh);
        }
        recycle_stored_casters(fresh);
        membership_changed
    }

    fn publish_world_casters(&mut self) {
        profiling::scope!("mesh::prepared_renderables::spatial_publish_casters");
        publish_caster_run_order(&self.casters_by_space, &mut self.stored_world_casters);
    }

    /// Returns whether `space_id` uses a BVH instead of only linear buckets.
    #[cfg(test)]
    pub(super) fn space_uses_bvh_for_tests(&self, space_id: RenderSpaceId) -> bool {
        self.spaces
            .get(&space_id)
            .is_some_and(PreparedSpatialSpace::uses_bvh)
    }
}

#[derive(Default)]
struct PreparedSpatialSpaceBuilder {
    indexed: Vec<IndexedPreparedRun>,
    linear: Vec<LinearPreparedRun>,
}

impl PreparedSpatialSpaceBuilder {
    fn finish(mut self) -> PreparedSpatialSpace {
        let mut space = PreparedSpatialSpace {
            linear: self.linear,
            indexed: Vec::new(),
            order: Vec::new(),
            nodes: Vec::new(),
            root: None,
            indexed_run_count: self.indexed.len(),
            fallback_run_count: 0,
        };
        space.fallback_run_count = space.linear.len();
        if self.indexed.len() <= SPATIAL_LINEAR_RUN_LIMIT {
            space
                .linear
                .extend(self.indexed.drain(..).map(|entry| LinearPreparedRun {
                    run_index: entry.run_index,
                    bounds: Some((entry.aabb_min, entry.aabb_max)),
                    slot_count: entry.slot_count,
                }));
            return space;
        }

        space.order = (0..self.indexed.len()).collect();
        space.indexed = self.indexed;
        let mut order = std::mem::take(&mut space.order);
        let end = order.len();
        space.root = Some(space.build_node(&mut order, 0, end));
        space.order = order;
        space
    }
}

/// One render space's BVH plus conservative linear fallback runs.
#[derive(Default)]
struct PreparedSpatialSpace {
    linear: Vec<LinearPreparedRun>,
    indexed: Vec<IndexedPreparedRun>,
    order: Vec<usize>,
    nodes: Vec<PreparedBvhNode>,
    root: Option<usize>,
    indexed_run_count: usize,
    fallback_run_count: usize,
}

impl PreparedSpatialSpace {
    fn refit(&mut self, draws: &[FramePreparedDraw], runs: &[FramePreparedRun]) {
        for entry in &mut self.linear {
            entry.bounds = refit_linear_bounds(draws, runs, entry);
        }
        for entry in &mut self.indexed {
            let (aabb_min, aabb_max) = refit_indexed_bounds(draws, runs, entry.run_index);
            entry.aabb_min = aabb_min;
            entry.aabb_max = aabb_max;
            entry.center = (aabb_min + aabb_max) * 0.5;
        }
        self.refit_nodes();
    }

    fn refit_nodes(&mut self) {
        for node_index in (0..self.nodes.len()).rev() {
            let node = self.nodes[node_index];
            let (aabb_min, aabb_max, slot_count, run_count) = if node.count > 0 {
                bounds_for_order(
                    &self.indexed,
                    &self.order[node.start..node.start + node.count],
                )
            } else {
                let left = self.nodes[node.left];
                let right = self.nodes[node.right];
                (
                    left.aabb_min.min(right.aabb_min),
                    left.aabb_max.max(right.aabb_max),
                    left.slot_count.saturating_add(right.slot_count),
                    left.run_count.saturating_add(right.run_count),
                )
            };
            self.nodes[node_index].aabb_min = aabb_min;
            self.nodes[node_index].aabb_max = aabb_max;
            self.nodes[node_index].slot_count = slot_count;
            self.nodes[node_index].run_count = run_count;
        }
    }

    fn query(
        &self,
        space_id: RenderSpaceId,
        scene: &(impl SceneSpaceRead + ?Sized),
        culling: Option<&WorldMeshCullInput<'_>>,
        out: &mut PreparedSpatialQueryOutput<'_>,
    ) {
        out.visibility.indexed_runs += self.indexed_run_count;
        out.visibility.fallback_runs += self.fallback_run_count;
        out.visibility.linear_fallback_runs += self.linear.len();
        // One view inversion for every node and leaf in this space. Overlay runs are not indexed,
        // so the broadphase frustum is always the non-overlay clip volume.
        let prepared =
            culling.map(|culling| PreparedSpaceCull::frustum(scene, space_id, false, culling));
        {
            profiling::scope!("mesh::prepared_renderables::spatial_query_linear");
            for entry in &self.linear {
                query_linear_run(prepared.as_ref(), entry, out);
            }
        }
        if let Some(root) = self.root {
            profiling::scope!("mesh::prepared_renderables::spatial_query_bvh");
            self.query_node(root, prepared.as_ref(), out);
        }
    }

    fn query_node(
        &self,
        node_index: usize,
        prepared: Option<&PreparedSpaceCull<'_>>,
        out: &mut PreparedSpatialQueryOutput<'_>,
    ) {
        let node = self.nodes[node_index];
        if let Some(prepared) = prepared
            && !prepared.aabb_visible(Vec3::from(node.aabb_min), Vec3::from(node.aabb_max))
        {
            record_spatial_frustum_reject(
                out.cull_stats,
                out.visibility,
                node.run_count,
                node.slot_count,
            );
            return;
        }
        if node.count > 0 {
            for &entry_index in &self.order[node.start..node.start + node.count] {
                let entry = self.indexed[entry_index];
                query_indexed_run(prepared, entry, out);
            }
        } else {
            self.query_node(node.left, prepared, out);
            self.query_node(node.right, prepared, out);
        }
    }

    fn build_node(&mut self, order: &mut [usize], start: usize, end: usize) -> usize {
        let (aabb_min, aabb_max, slot_count, run_count) =
            bounds_for_order(&self.indexed, &order[start..end]);
        let index = self.nodes.len();
        self.nodes.push(PreparedBvhNode {
            aabb_min,
            aabb_max,
            slot_count,
            run_count,
            start,
            count: 0,
            left: 0,
            right: 0,
        });
        let count = end - start;
        if count <= BVH_LEAF_SIZE {
            self.nodes[index].count = count;
            return index;
        }

        let axis = largest_axis(aabb_max - aabb_min);
        order[start..end].sort_unstable_by(|&a, &b| {
            axis_value(self.indexed[a].center, axis)
                .total_cmp(&axis_value(self.indexed[b].center, axis))
                .then_with(|| self.indexed[a].run_index.cmp(&self.indexed[b].run_index))
        });
        let mid = start + count / 2;
        let left = self.build_node(order, start, mid);
        let right = self.build_node(order, mid, end);
        self.nodes[index].left = left;
        self.nodes[index].right = right;
        index
    }

    #[cfg(test)]
    fn uses_bvh(&self) -> bool {
        self.root.is_some()
    }
}

fn refit_linear_bounds(
    draws: &[FramePreparedDraw],
    runs: &[FramePreparedRun],
    entry: &LinearPreparedRun,
) -> Option<(Vec3A, Vec3A)> {
    let run = runs.get(entry.run_index)?;
    let first = draws.get(run.start as usize)?;
    indexable_run_bounds(first)
}

fn refit_indexed_bounds(
    draws: &[FramePreparedDraw],
    runs: &[FramePreparedRun],
    run_index: usize,
) -> (Vec3A, Vec3A) {
    runs.get(run_index)
        .and_then(|run| draws.get(run.start as usize))
        .and_then(indexable_run_bounds)
        .unwrap_or_else(conservative_visible_bounds)
}

fn conservative_visible_bounds() -> (Vec3A, Vec3A) {
    let extent = f32::MAX * 0.25;
    (Vec3A::splat(-extent), Vec3A::splat(extent))
}

/// Mutable query output shared across spatial traversal helpers.
struct PreparedSpatialQueryOutput<'a> {
    /// Unique candidate run marks.
    candidates: &'a mut Vec<usize>,
    /// Raw candidate marks before duplicate suppression.
    raw_candidate_marks: &'a mut usize,
    /// Slot-level cull counters.
    cull_stats: &'a mut (usize, usize, usize),
    /// Visibility broadphase counters.
    visibility: &'a mut WorldMeshVisibilityStats,
}

impl PreparedSpatialQueryOutput<'_> {
    /// Marks one visible run candidate.
    fn mark_candidate_run(&mut self, run_index: usize) {
        *self.raw_candidate_marks = self.raw_candidate_marks.saturating_add(1);
        self.candidates.push(run_index);
    }
}

fn gather_spatial_candidate_runs(
    runs: &[FramePreparedRun],
    candidate_indices: &mut Vec<usize>,
) -> Vec<FramePreparedRun> {
    profiling::scope!("mesh::prepared_renderables::spatial_query_gather");
    if should_gather_spatial_candidates_dense(candidate_indices.len(), runs.len()) {
        gather_spatial_candidate_runs_dense(runs, candidate_indices)
    } else {
        gather_spatial_candidate_runs_sparse(runs, candidate_indices)
    }
}

#[inline]
fn should_gather_spatial_candidates_dense(candidate_marks: usize, run_count: usize) -> bool {
    candidate_marks.saturating_mul(SPATIAL_DENSE_GATHER_MIN_CANDIDATE_DIVISOR) >= run_count
}

fn gather_spatial_candidate_runs_sparse(
    runs: &[FramePreparedRun],
    candidate_indices: &mut Vec<usize>,
) -> Vec<FramePreparedRun> {
    profiling::scope!("mesh::prepared_renderables::spatial_query_gather_sparse");
    candidate_indices.sort_unstable();
    candidate_indices.dedup();
    let mut out = take_spatial_runs();
    out.reserve(candidate_indices.len());
    for &run_index in candidate_indices.iter() {
        if let Some(run) = runs.get(run_index).copied() {
            out.push(run);
        }
    }
    out
}

fn gather_spatial_candidate_runs_dense(
    runs: &[FramePreparedRun],
    candidate_indices: &[usize],
) -> Vec<FramePreparedRun> {
    profiling::scope!("mesh::prepared_renderables::spatial_query_gather_dense");
    let mut candidate_bits = take_spatial_bits();
    candidate_bits.clear_and_resize(runs.len());
    for &run_index in candidate_indices {
        candidate_bits.insert(run_index);
    }
    let mut out = take_spatial_runs();
    out.reserve(candidate_indices.len().min(runs.len()));
    for (run_index, run) in runs.iter().copied().enumerate() {
        if candidate_bits.contains(run_index) {
            out.push(run);
        }
    }
    recycle_spatial_bits(candidate_bits);
    out
}

fn take_spatial_indices() -> Vec<usize> {
    take_tls_vec(&SPATIAL_CANDIDATE_INDICES)
}

fn recycle_spatial_indices(indices: Vec<usize>) {
    recycle_tls_vec(&SPATIAL_CANDIDATE_INDICES, indices);
}

fn take_spatial_runs() -> Vec<FramePreparedRun> {
    take_tls_vec(&SPATIAL_CANDIDATE_RUNS)
}

fn recycle_spatial_runs(runs: Vec<FramePreparedRun>) {
    recycle_tls_vec(&SPATIAL_CANDIDATE_RUNS, runs);
}

fn take_spatial_bits() -> DenseBitSet {
    SPATIAL_CANDIDATE_BITS.with(|slot| match slot.try_borrow_mut() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => DenseBitSet::default(),
    })
}

fn recycle_spatial_bits(bits: DenseBitSet) {
    SPATIAL_CANDIDATE_BITS.with(|slot| {
        if let Ok(mut guard) = slot.try_borrow_mut()
            && bits.word_capacity() > guard.word_capacity()
        {
            *guard = bits;
        }
    });
}

fn take_tls_vec<T>(slot: &'static std::thread::LocalKey<RefCell<Vec<T>>>) -> Vec<T> {
    slot.with(|cell| {
        let mut vec = match cell.try_borrow_mut() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => Vec::new(),
        };
        vec.clear();
        vec
    })
}

fn recycle_tls_vec<T>(slot: &'static std::thread::LocalKey<RefCell<Vec<T>>>, mut vec: Vec<T>) {
    vec.clear();
    slot.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut()
            && vec.capacity() > guard.capacity()
        {
            *guard = vec;
        }
    });
}

#[derive(Clone, Copy)]
struct LinearPreparedRun {
    run_index: usize,
    bounds: Option<(Vec3A, Vec3A)>,
    slot_count: usize,
}

#[derive(Clone, Copy)]
struct IndexedPreparedRun {
    run_index: usize,
    aabb_min: Vec3A,
    aabb_max: Vec3A,
    center: Vec3A,
    slot_count: usize,
}

#[derive(Clone, Copy)]
struct PreparedBvhNode {
    aabb_min: Vec3A,
    aabb_max: Vec3A,
    slot_count: usize,
    run_count: usize,
    start: usize,
    count: usize,
    left: usize,
    right: usize,
}

fn query_linear_run(
    prepared: Option<&PreparedSpaceCull<'_>>,
    entry: &LinearPreparedRun,
    out: &mut PreparedSpatialQueryOutput<'_>,
) {
    if let (Some(prepared), Some((aabb_min, aabb_max))) = (prepared, entry.bounds)
        && !prepared.aabb_visible(Vec3::from(aabb_min), Vec3::from(aabb_max))
    {
        record_spatial_frustum_reject(out.cull_stats, out.visibility, 1, entry.slot_count);
        return;
    }
    out.mark_candidate_run(entry.run_index);
}

fn query_indexed_run(
    prepared: Option<&PreparedSpaceCull<'_>>,
    entry: IndexedPreparedRun,
    out: &mut PreparedSpatialQueryOutput<'_>,
) {
    if let Some(prepared) = prepared
        && !prepared.aabb_visible(Vec3::from(entry.aabb_min), Vec3::from(entry.aabb_max))
    {
        record_spatial_frustum_reject(out.cull_stats, out.visibility, 1, entry.slot_count);
        return;
    }
    out.mark_candidate_run(entry.run_index);
}

fn record_spatial_frustum_reject(
    stats: &mut (usize, usize, usize),
    visibility: &mut WorldMeshVisibilityStats,
    run_count: usize,
    slot_count: usize,
) {
    stats.0 = stats.0.saturating_add(slot_count);
    stats.1 = stats.1.saturating_add(slot_count);
    visibility.broadphase_culled_runs = visibility.broadphase_culled_runs.saturating_add(run_count);
    visibility.broadphase_culled_draws = visibility
        .broadphase_culled_draws
        .saturating_add(slot_count);
}

fn push_space_casters(
    out: &mut Vec<StoredWorldCaster>,
    draws: &[FramePreparedDraw],
    runs: &[FramePreparedRun],
    space_id: RenderSpaceId,
    linear: &[LinearPreparedRun],
    indexed: &[IndexedPreparedRun],
) {
    let run_indices = linear
        .iter()
        .map(|entry| entry.run_index)
        .chain(indexed.iter().map(|entry| entry.run_index));
    for run_index in run_indices {
        let Some(bounds) = runs
            .get(run_index)
            .and_then(|run| draws.get(run.start as usize))
            .and_then(prepared_world_caster)
        else {
            continue;
        };
        if bounds.space_id != space_id {
            continue;
        }
        out.push(StoredWorldCaster { run_index, bounds });
    }
}

fn store_space_caster_rows(
    casters: &mut HashMap<RenderSpaceId, Vec<StoredWorldCaster>>,
    space_id: RenderSpaceId,
    fresh: &mut Vec<StoredWorldCaster>,
) -> bool {
    if fresh.is_empty() {
        let Some(bucket) = casters.get_mut(&space_id) else {
            return false;
        };
        let changed = !bucket.is_empty();
        bucket.clear();
        return changed;
    }
    let bucket = casters.entry(space_id).or_default();
    let changed = bucket.len() != fresh.len()
        || bucket
            .iter()
            .zip(fresh.iter())
            .any(|(old, new)| old.run_index != new.run_index);
    if changed {
        bucket.clear();
        bucket.append(fresh);
    } else {
        for (slot, row) in bucket.iter_mut().zip(fresh.iter()) {
            slot.bounds = row.bounds;
        }
    }
    changed
}

fn patch_stored_caster_bounds(stored: &mut [StoredWorldCaster], fresh: &[StoredWorldCaster]) {
    for row in fresh {
        if let Ok(position) = stored.binary_search_by_key(&row.run_index, |entry| entry.run_index) {
            stored[position].bounds = row.bounds;
        }
    }
}

fn publish_caster_run_order(
    casters: &HashMap<RenderSpaceId, Vec<StoredWorldCaster>>,
    stored: &mut Vec<StoredWorldCaster>,
) {
    stored.clear();
    let nonempty = casters.values().filter(|rows| !rows.is_empty()).count();
    if nonempty == 0 {
        return;
    }
    if nonempty == 1 {
        if let Some(rows) = casters.values().find(|rows| !rows.is_empty()) {
            stored.extend_from_slice(rows);
        }
        return;
    }
    for rows in casters.values() {
        stored.extend_from_slice(rows);
    }
    stored.sort_unstable_by(|left, right| {
        left.run_index
            .cmp(&right.run_index)
            .then_with(|| left.bounds.space_id.cmp(&right.bounds.space_id))
    });
}

fn take_stored_casters() -> Vec<StoredWorldCaster> {
    take_tls_vec(&SPATIAL_CASTER_ROWS)
}

fn recycle_stored_casters(rows: Vec<StoredWorldCaster>) {
    recycle_tls_vec(&SPATIAL_CASTER_ROWS, rows);
}

fn prepared_world_caster(first: &FramePreparedDraw) -> Option<super::PreparedCasterBounds> {
    if first.is_overlay || first.is_hidden {
        return None;
    }
    let (aabb_min, aabb_max) = first.cull_geometry?.world_aabb?;
    Some(super::PreparedCasterBounds {
        space_id: first.space_id,
        renderable_index: first.renderable_index,
        instance_id: first.instance_id.0,
        node_id: first.node_id,
        skinned: first.skinned,
        mesh_asset_id: first.mesh_asset_id,
        aabb_min,
        aabb_max,
    })
}

fn indexable_run_bounds(first: &FramePreparedDraw) -> Option<(Vec3A, Vec3A)> {
    if first.is_overlay {
        return None;
    }
    let (aabb_min, aabb_max) = first.cull_geometry?.world_aabb?;
    if !aabb_valid(aabb_min, aabb_max) {
        return None;
    }
    Some((Vec3A::from(aabb_min), Vec3A::from(aabb_max)))
}

fn aabb_valid(aabb_min: Vec3, aabb_max: Vec3) -> bool {
    aabb_min.is_finite() && aabb_max.is_finite() && (aabb_max - aabb_min).cmpgt(Vec3::ZERO).all()
}

fn bounds_for_order(
    entries: &[IndexedPreparedRun],
    order: &[usize],
) -> (Vec3A, Vec3A, usize, usize) {
    let mut aabb_min = Vec3A::splat(f32::INFINITY);
    let mut aabb_max = Vec3A::splat(f32::NEG_INFINITY);
    let mut slot_count = 0usize;
    for &entry_index in order {
        let entry = entries[entry_index];
        aabb_min = aabb_min.min(entry.aabb_min);
        aabb_max = aabb_max.max(entry.aabb_max);
        slot_count = slot_count.saturating_add(entry.slot_count);
    }
    (aabb_min, aabb_max, slot_count, order.len())
}

fn largest_axis(v: Vec3A) -> usize {
    if v.x >= v.y && v.x >= v.z {
        0
    } else if v.y >= v.z {
        1
    } else {
        2
    }
}

fn axis_value(v: Vec3A, axis: usize) -> f32 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

#[cfg(test)]
include!("spatial_tests.rs");
