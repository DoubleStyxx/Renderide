//! Incremental world-matrix propagation and child index for transform hierarchies.

use glam::Mat4;
use rayon::prelude::*;

use crate::shared::RenderTransform;

use super::error::SceneError;
use super::math::{
    render_matrix_has_degenerate_scale, render_transform_has_degenerate_scale,
    render_transform_to_matrix,
};

const WORLD_BULK_REBUILD_PARALLEL_CHUNK_SIZE: usize = 32;
/// Hierarchy-depth chunks assigned to one bulk rebuild worker.
const WORLD_BULK_REBUILD_PARALLEL_CHUNK_TASKS: usize = 1;
/// Node count above which a fully dirty cache routes through the bulk rebuild path.
const WORLD_BULK_REBUILD_PARALLEL_MIN: usize = WORLD_BULK_REBUILD_PARALLEL_CHUNK_SIZE * 2;
/// Node count in one hierarchy depth level above which that level fans out across rayon.
const WORLD_BULK_REBUILD_PARALLEL_LEVEL_MIN: usize = WORLD_BULK_REBUILD_PARALLEL_CHUNK_SIZE * 2;
/// Nodes assigned to one partial dirty rebuild worker chunk.
const WORLD_PARTIAL_REBUILD_PARALLEL_CHUNK_NODES: usize = 32;
/// Dirty node count required before partial dirty rebuilds use the level-synchronous path.
const WORLD_PARTIAL_REBUILD_PARALLEL_MIN_DIRTY: usize =
    WORLD_PARTIAL_REBUILD_PARALLEL_CHUNK_NODES * 2;
/// Minimum dirty density divisor for the partial rebuild path.
const WORLD_PARTIAL_REBUILD_MIN_DENSITY_DIVISOR: usize = 16;

#[inline]
fn should_parallelize_bulk_level(node_count: usize) -> bool {
    node_count >= WORLD_BULK_REBUILD_PARALLEL_LEVEL_MIN
}

#[inline]
fn should_parallelize_partial_rebuild(node_count: usize, dirty_count: usize) -> bool {
    dirty_count >= WORLD_PARTIAL_REBUILD_PARALLEL_MIN_DIRTY
        && dirty_count.saturating_mul(WORLD_PARTIAL_REBUILD_MIN_DENSITY_DIVISOR) >= node_count
}

#[inline]
fn dirty_world_matrix_count(computed: &[bool]) -> usize {
    computed.iter().filter(|&&is_computed| !is_computed).count()
}

#[inline]
fn should_parallelize_partial_level(node_count: usize) -> bool {
    node_count >= WORLD_PARTIAL_REBUILD_PARALLEL_CHUNK_NODES * 2
}

fn collect_bulk_level_parallel<F>(
    indices: &[usize],
    chunks: &mut Vec<Vec<(Mat4, bool)>>,
    out: &mut Vec<(Mat4, bool)>,
    compute_one: F,
) where
    F: Fn(&usize) -> (Mat4, bool) + Sync + Send,
{
    let chunk_count = indices
        .len()
        .div_ceil(WORLD_BULK_REBUILD_PARALLEL_CHUNK_SIZE);
    if chunks.len() < chunk_count {
        chunks.resize_with(chunk_count, Vec::new);
    }
    chunks
        .par_iter_mut()
        .take(chunk_count)
        .with_min_len(WORLD_BULK_REBUILD_PARALLEL_CHUNK_TASKS)
        .zip(
            indices
                .par_chunks(WORLD_BULK_REBUILD_PARALLEL_CHUNK_SIZE)
                .with_min_len(WORLD_BULK_REBUILD_PARALLEL_CHUNK_TASKS),
        )
        .for_each(|(chunk_out, chunk_indices)| {
            profiling::scope!("scene::world_bulk_rebuild::chunk_worker");
            chunk_out.clear();
            chunk_out.reserve(chunk_indices.len());
            chunk_out.extend(chunk_indices.iter().map(&compute_one));
        });
    out.clear();
    out.reserve(indices.len());
    for chunk in chunks.iter_mut().take(chunk_count) {
        out.append(chunk);
    }
}

/// Returns whether the resolved local or world matrix cannot rasterize triangle geometry.
#[inline]
fn transform_matrix_has_degenerate_scale(transform: &RenderTransform, matrix: Mat4) -> bool {
    render_transform_has_degenerate_scale(transform) || render_matrix_has_degenerate_scale(matrix)
}

/// Per-space cache: world matrices and incremental recompute bookkeeping.
#[derive(Debug)]
pub struct WorldTransformCache {
    /// World matrix per dense transform index (parent chain only; no [`RenderSpaceState::root_transform`](super::render_space::RenderSpaceState) multiply).
    pub world_matrices: Vec<Mat4>,
    /// `true` when `world_matrices[i]` is valid for the current local poses.
    pub computed: Vec<bool>,
    /// Cached local TRS matrices.
    pub local_matrices: Vec<Mat4>,
    /// Stale local TRS when pose changed.
    pub local_dirty: Vec<bool>,
    /// `true` when this node's effective matrix has fewer than two renderable dimensions.
    pub degenerate_scales: Vec<bool>,
    /// Reused hierarchy traversal state.
    pub(super) hierarchy: WorldHierarchyScratch,
    /// Reused bulk and partial rebuild buffers.
    pub(super) bulk_rebuild: WorldBulkRebuildScratch,
    /// Reused dense transform-update scratch.
    pub(super) transform_apply: TransformApplyScratch,
    /// Last solved world matrix per node, copied in place when rigid object motion is published.
    ///
    /// Empty until the first publish. Steady-state publishes overwrite this storage and do not
    /// allocate.
    pub(in crate::scene) previous_world_matrices: Vec<Mat4>,
    /// `true` when [`Self::previous_world_matrices`] holds a solved pose from the previous publish.
    pub(in crate::scene) previous_valid: Vec<bool>,
    /// `true` when `world_matrices[i]` still holds a pose produced by an earlier solve.
    ///
    /// Apply marks `computed` false for dirty nodes without clearing `world_matrices`, so this bit
    /// is what lets the next publish treat that stored matrix as "previous".
    pub(in crate::scene) motion_history_ready: Vec<bool>,
}

/// Reused hierarchy traversal state for one [`WorldTransformCache`].
#[derive(Debug, Default)]
pub(super) struct WorldHierarchyScratch {
    /// Epoch per node for O(1) cycle detection during upward walks.
    pub(super) visit_epoch: Vec<u32>,
    /// Incremented before each upward walk.
    pub(super) walk_epoch: u32,
    /// Parent -> children lists; rebuilt when structure changes.
    pub(super) children: Vec<Vec<usize>>,
    /// `children` must be rebuilt before descendant marking.
    pub(super) children_dirty: bool,
}

/// Reused level-synchronous world-matrix rebuild scratch.
#[derive(Debug, Default)]
pub(super) struct WorldBulkRebuildScratch {
    /// Bulk-rebuild scratch: depth per node. Cycle members stay at [`DEPTH_ON_CHAIN`] until the
    /// next solve. Reused across frames.
    pub(super) depth: Vec<u32>,
    /// Bulk-rebuild scratch: per-depth-level node-index buckets. Inner Vec capacities persist
    /// across frames so a stable scene re-uses the same storage every solve.
    pub(super) levels: Vec<Vec<usize>>,
    /// Bulk-rebuild scratch: indices of nodes whose ancestor chain forms a cycle.
    pub(super) cycle_nodes: Vec<usize>,
    /// Parent-chain walk used by depth classification. Reused so a full rebuild does not
    /// allocate a stack per solve.
    pub(super) classify_stack: Vec<usize>,
    /// Bulk-rebuild scratch: per-level computed (world matrix, degenerate flag) pairs before
    /// they are written back to the cache.
    pub(super) writes: Vec<(Mat4, bool)>,
    /// Bulk-rebuild scratch: per-worker chunk outputs used to give Tracy one span per Rayon task.
    pub(super) parallel_writes: Vec<Vec<(Mat4, bool)>>,
    /// Partial-rebuild scratch: dirty node indices for the current hierarchy depth level.
    pub(super) dirty_level_indices: Vec<usize>,
}

/// Reused dense transform-update scratch for one [`WorldTransformCache`].
#[derive(Debug, Default)]
pub(super) struct TransformApplyScratch {
    /// Transform-apply scratch: per-node dirty flags reused across dense transform updates.
    pub(super) dirty_flags: Vec<bool>,
    /// Transform-apply scratch: dense list of nodes marked in [`Self::dirty_flags`].
    pub(super) dirty_indices: Vec<usize>,
    /// Transform-apply scratch: node index to pose-plan row while collapsing duplicate pose rows.
    pub(super) pose_plan_indices: Vec<usize>,
}

impl Default for WorldTransformCache {
    fn default() -> Self {
        Self {
            world_matrices: Vec::new(),
            computed: Vec::new(),
            local_matrices: Vec::new(),
            local_dirty: Vec::new(),
            degenerate_scales: Vec::new(),
            hierarchy: WorldHierarchyScratch {
                children_dirty: true,
                ..WorldHierarchyScratch::default()
            },
            bulk_rebuild: WorldBulkRebuildScratch::default(),
            transform_apply: TransformApplyScratch::default(),
            previous_world_matrices: Vec::new(),
            previous_valid: Vec::new(),
            motion_history_ready: Vec::new(),
        }
    }
}

impl WorldTransformCache {
    /// Copies the last solved world pose for `nodes` into reused previous-matrix storage.
    ///
    /// Call this before the current flush overwrites `world_matrices`. Only the listed nodes are
    /// written. Those nodes are the rigid `MotionVectorMode::Object` transforms, which includes
    /// both dirty and clean ones so a stopped object’s previous matrix catches up to current.
    /// Dirty nodes with no object-mode renderer are left alone. The copy stays inside the existing
    /// allocation when the node count is unchanged.
    pub(crate) fn publish_previous_world_matrices(&mut self, nodes: &[usize]) {
        let count = self.world_matrices.len();
        self.ensure_motion_history_len(count);
        for &index in nodes {
            if index >= count {
                continue;
            }
            if self.motion_history_ready[index] {
                self.previous_world_matrices[index] = self.world_matrices[index];
                self.previous_valid[index] = true;
            } else {
                self.previous_valid[index] = false;
            }
        }
    }

    /// Marks listed nodes whose current world matrix came from a completed solve.
    ///
    /// Dirty nodes keep a previous `true` until the next successful solve replaces it. That is
    /// what makes the following publish see the pre-solve matrix.
    pub(crate) fn seal_motion_history(&mut self, nodes: &[usize]) {
        let count = self.world_matrices.len();
        self.ensure_motion_history_len(count);
        for &index in nodes {
            if index < count && self.computed[index] {
                self.motion_history_ready[index] = true;
            }
        }
    }

    fn ensure_motion_history_len(&mut self, count: usize) {
        if self.previous_world_matrices.len() != count {
            self.previous_world_matrices.resize(count, Mat4::IDENTITY);
        }
        if self.previous_valid.len() != count {
            self.previous_valid.resize(count, false);
        }
        if self.motion_history_ready.len() != count {
            self.motion_history_ready.resize(count, false);
        }
    }

    /// Previous solved world matrix for one dense transform, when rigid motion history has one.
    pub(crate) fn previous_world_matrix(&self, index: usize) -> Option<Mat4> {
        if self.previous_valid.get(index).copied().unwrap_or(false) {
            self.previous_world_matrices.get(index).copied()
        } else {
            None
        }
    }

    /// Capacity of the reused previous-matrix buffer. Steady-state publishes do not grow it.
    #[cfg(any(test, feature = "bench-api"))]
    pub(crate) fn previous_world_matrix_capacity(&self) -> usize {
        self.previous_world_matrices.capacity()
    }
}

/// After `swap_remove` at `removed_id` index, remaps a stored transform reference.
pub(in crate::scene) fn fixup_transform_id(old: i32, removed_id: i32, last_index: usize) -> i32 {
    if old == removed_id {
        -1
    } else if old == last_index as i32 {
        removed_id
    } else {
        old
    }
}

/// Rebuilds parent -> children adjacency.
pub(super) fn rebuild_children(node_parents: &[i32], n: usize, children: &mut Vec<Vec<usize>>) {
    children.resize_with(n, Vec::new);
    for c in children.iter_mut() {
        c.clear();
    }
    for (i, &p) in node_parents.iter().take(n).enumerate() {
        if p >= 0 && (p as usize) < n && p != i as i32 {
            children[p as usize].push(i);
        }
    }
}

/// Marks descendants of any node with `computed[i] == false` as uncomputed.
pub(super) fn mark_descendants_uncomputed(children: &[Vec<usize>], computed: &mut [bool]) {
    let n = computed.len();
    if n == 0 {
        return;
    }
    let mut stack: Vec<usize> = Vec::with_capacity(64.min(n));
    for i in 0..n {
        if computed[i] {
            continue;
        }
        stack.clear();
        let child_list: &[usize] = children.get(i).map_or(&[], Vec::as_slice);
        stack.extend_from_slice(child_list);
        while let Some(child) = stack.pop() {
            computed[child] = false;
            let child_list: &[usize] = children.get(child).map_or(&[], Vec::as_slice);
            stack.extend_from_slice(child_list);
        }
    }
}

/// Marks descendants of the provided dirty roots as uncomputed.
pub(super) fn mark_descendants_uncomputed_from_roots(
    children: &[Vec<usize>],
    computed: &mut [bool],
    roots: &[usize],
) {
    if computed.is_empty() || roots.is_empty() {
        return;
    }
    let mut stack: Vec<usize> = Vec::with_capacity(64.min(computed.len()));
    for &root in roots {
        if root < computed.len() {
            computed[root] = false;
        }
        stack.clear();
        stack.extend_from_slice(children.get(root).map_or(&[], Vec::as_slice));
        while let Some(child) = stack.pop() {
            if child >= computed.len() {
                continue;
            }
            computed[child] = false;
            stack.extend_from_slice(children.get(child).map_or(&[], Vec::as_slice));
        }
    }
}

#[inline]
fn get_local_matrix(
    nodes: &[RenderTransform],
    local_matrices: &mut [Mat4],
    local_dirty: &mut [bool],
    i: usize,
) -> Mat4 {
    if i < local_dirty.len() && local_dirty[i] {
        let m = render_transform_to_matrix(&nodes[i]);
        local_matrices[i] = m;
        local_dirty[i] = false;
        m
    } else if i < local_matrices.len() {
        local_matrices[i]
    } else {
        render_transform_to_matrix(&nodes[i])
    }
}

/// Returns `true` when every entry of `computed` is `false`, signalling a bulk rebuild.
#[inline]
fn cache_is_fully_dirty(computed: &[bool]) -> bool {
    !computed.iter().any(|c| *c)
}

/// Unvisited node in [`classify_nodes_by_depth`].
const DEPTH_UNSEEN: u32 = u32::MAX;
/// Node currently on the parent-chain stack. Also the final mark for cycle members, which the
/// bucketing pass turns into [`cycle_nodes`](WorldBulkRebuildScratch::cycle_nodes).
const DEPTH_ON_CHAIN: u32 = u32::MAX - 1;

/// Fills `depth` / `levels` / `cycle_nodes` from a parent-array tree, reusing the supplied
/// buffers' allocations. After this call `levels[d]` lists every node at depth `d` whose
/// ancestor chain reaches a root, and `cycle_nodes` lists every node whose ancestor chain
/// forms a cycle. Cycle nodes get a local-only fallback matrix (matching
/// [`WorldTransformCache::compute_world_matrices_incremental`]).
///
/// One walk per node resolves the parent chain. The previous sweep advanced one depth level
/// per pass, so a child-before-parent chain of length `n` (bones and nested slots stored that
/// way) re-scanned the whole array `n` times.
fn classify_nodes_by_depth(
    node_parents: &[i32],
    n: usize,
    depth: &mut Vec<u32>,
    levels: &mut Vec<Vec<usize>>,
    cycle_nodes: &mut Vec<usize>,
    stack: &mut Vec<usize>,
) {
    depth.clear();
    depth.resize(n, DEPTH_UNSEEN);
    cycle_nodes.clear();
    stack.clear();
    for inner in levels.iter_mut() {
        inner.clear();
    }

    for start in 0..n {
        if depth[start] != DEPTH_UNSEEN {
            continue;
        }
        let end = walk_parent_chain(node_parents, n, depth, stack, start);
        match end {
            ChainEnd::Cycle => stack.clear(),
            ChainEnd::Root => assign_chain_depths(depth, stack, 0),
            ChainEnd::ParentDepth(parent_depth) => {
                assign_chain_depths(depth, stack, parent_depth.saturating_add(1));
            }
        }
    }

    let max_depth = depth
        .iter()
        .copied()
        .filter(|&d| d < DEPTH_ON_CHAIN)
        .max()
        .unwrap_or(0) as usize;
    if levels.len() < max_depth + 1 {
        levels.resize_with(max_depth + 1, Vec::new);
    }
    for (i, &d) in depth.iter().enumerate() {
        if d >= DEPTH_ON_CHAIN {
            cycle_nodes.push(i);
        } else {
            levels[d as usize].push(i);
        }
    }
}

/// How a parent walk in [`walk_parent_chain`] stopped.
enum ChainEnd {
    /// The chain reached a node with no parent, or a self-parent. Top of stack is depth 0.
    Root,
    /// The chain met a node whose depth is already known. That depth is the parent of the
    /// top of the stack.
    ParentDepth(u32),
    /// The chain re-entered a node already on this stack, or a previously recorded cycle.
    Cycle,
}

/// Walks from `start` toward the root, pushing unresolved nodes onto `stack`.
fn walk_parent_chain(
    node_parents: &[i32],
    n: usize,
    depth: &mut [u32],
    stack: &mut Vec<usize>,
    start: usize,
) -> ChainEnd {
    let mut cursor = start;
    loop {
        match depth[cursor] {
            DEPTH_ON_CHAIN => return ChainEnd::Cycle,
            DEPTH_UNSEEN => {
                depth[cursor] = DEPTH_ON_CHAIN;
                stack.push(cursor);
                let parent = node_parents.get(cursor).copied().unwrap_or(-1);
                if parent < 0 || (parent as usize) >= n || parent == cursor as i32 {
                    return ChainEnd::Root;
                }
                cursor = parent as usize;
            }
            parent_depth => return ChainEnd::ParentDepth(parent_depth),
        }
    }
}

/// Writes increasing depths while popping `stack` (the root side was pushed last).
fn assign_chain_depths(depth: &mut [u32], stack: &mut Vec<usize>, mut next: u32) {
    while let Some(id) = stack.pop() {
        if next >= DEPTH_ON_CHAIN {
            depth[id] = DEPTH_ON_CHAIN;
        } else {
            depth[id] = next;
            next += 1;
        }
    }
}

struct BulkLevelWriteCtx<'a> {
    nodes: &'a [RenderTransform],
    node_parents: &'a [i32],
    local_matrices: &'a [Mat4],
    world_matrices: &'a mut [Mat4],
    degenerate_scales: &'a mut [bool],
    computed: &'a mut [bool],
    writes: &'a mut Vec<(Mat4, bool)>,
    parallel_writes: &'a mut Vec<Vec<(Mat4, bool)>>,
}

impl BulkLevelWriteCtx<'_> {
    fn write_root_level(&mut self, roots: &[usize]) {
        if roots.is_empty() {
            return;
        }
        self.writes.clear();
        let nodes = self.nodes;
        let local_matrices = self.local_matrices;
        if should_parallelize_bulk_level(roots.len()) {
            collect_bulk_level_parallel(roots, self.parallel_writes, self.writes, |&i| {
                (
                    local_matrices[i],
                    transform_matrix_has_degenerate_scale(&nodes[i], local_matrices[i]),
                )
            });
        } else {
            self.writes.extend(roots.iter().map(|&i| {
                (
                    local_matrices[i],
                    transform_matrix_has_degenerate_scale(&nodes[i], local_matrices[i]),
                )
            }));
        }
        for (slot, &i) in self.writes.iter().zip(roots.iter()) {
            self.world_matrices[i] = slot.0;
            self.degenerate_scales[i] = slot.1;
            self.computed[i] = true;
        }
    }

    fn write_child_level(&mut self, level: &[usize]) {
        if level.is_empty() {
            return;
        }
        self.writes.clear();
        {
            let nodes = self.nodes;
            let node_parents = self.node_parents;
            let local_matrices = self.local_matrices;
            let world_ro: &[Mat4] = self.world_matrices;
            let degen_ro: &[bool] = self.degenerate_scales;
            let compute_one = |&i: &usize| {
                let p = node_parents[i] as usize;
                let parent_world = world_ro[p];
                let parent_degen = degen_ro[p];
                let local = local_matrices[i];
                let world = parent_world * local;
                (
                    world,
                    parent_degen || transform_matrix_has_degenerate_scale(&nodes[i], world),
                )
            };
            if should_parallelize_bulk_level(level.len()) {
                collect_bulk_level_parallel(level, self.parallel_writes, self.writes, compute_one);
            } else {
                self.writes.extend(level.iter().map(compute_one));
            }
        }
        for (slot, &i) in self.writes.iter().zip(level.iter()) {
            self.world_matrices[i] = slot.0;
            self.degenerate_scales[i] = slot.1;
            self.computed[i] = true;
        }
    }
}

impl WorldTransformCache {
    /// Bulk rebuild: recomputes every world matrix from scratch using a level-synchronous BFS
    /// over the transform hierarchy. Each level is independent across siblings, so the inner
    /// matrix multiplications fan out across rayon.
    ///
    /// Cycle nodes (transforms whose ancestor chain loops back) fall back to a local-only matrix,
    /// matching [`Self::compute_world_matrices_incremental`].
    fn compute_world_matrices_bulk_rebuild(
        &mut self,
        scene_id: i32,
        nodes: &[RenderTransform],
        node_parents: &[i32],
    ) {
        profiling::scope!("scene::world_bulk_rebuild");
        let n = nodes.len();
        if self.hierarchy.visit_epoch.len() < n {
            self.hierarchy.visit_epoch.resize(n, 0);
        }
        if self.degenerate_scales.len() < n {
            self.degenerate_scales.resize(n, false);
        }

        // Materialize every local matrix once. Read-only after this point so the parallel level
        // sweep below can capture &[Mat4] instead of fighting borrow rules around lazy materialise.
        for ((local, dirty), node) in self
            .local_matrices
            .iter_mut()
            .zip(self.local_dirty.iter_mut())
            .zip(nodes.iter())
            .take(n)
        {
            *local = render_transform_to_matrix(node);
            *dirty = false;
        }

        // Reused scratch state lives on the cache so the bulk-rebuild path only allocates on
        // the first call and grows lazily as the scene gets bigger.
        let WorldTransformCache {
            world_matrices,
            computed,
            local_matrices,
            degenerate_scales,
            bulk_rebuild:
                WorldBulkRebuildScratch {
                    depth,
                    levels,
                    cycle_nodes,
                    classify_stack,
                    writes,
                    parallel_writes,
                    ..
                },
            ..
        } = self;

        classify_nodes_by_depth(node_parents, n, depth, levels, cycle_nodes, classify_stack);

        for cycle_id in cycle_nodes.iter() {
            logger::trace!(
                "parent cycle at scene {} transform {} -- local-only fallback",
                scene_id,
                cycle_id
            );
            let local = local_matrices[*cycle_id];
            world_matrices[*cycle_id] = local;
            degenerate_scales[*cycle_id] =
                transform_matrix_has_degenerate_scale(&nodes[*cycle_id], local);
            computed[*cycle_id] = true;
        }

        if let Some(roots) = levels.first() {
            let mut level_writer = BulkLevelWriteCtx {
                nodes,
                node_parents,
                local_matrices,
                world_matrices,
                degenerate_scales,
                computed,
                writes,
                parallel_writes,
            };
            level_writer.write_root_level(roots);
            for level in levels.iter().skip(1) {
                level_writer.write_child_level(level);
            }
        }
    }

    /// Partial rebuild: recomputes many dirty nodes by hierarchy depth while preserving already
    /// valid ancestors.
    fn compute_world_matrices_partial_rebuild(
        &mut self,
        scene_id: i32,
        nodes: &[RenderTransform],
        node_parents: &[i32],
        dirty_count: usize,
    ) -> bool {
        profiling::scope!("scene::world_partial_rebuild");
        let n = nodes.len();
        let WorldTransformCache {
            world_matrices,
            computed,
            local_matrices,
            local_dirty,
            degenerate_scales,
            bulk_rebuild:
                WorldBulkRebuildScratch {
                    depth,
                    levels,
                    cycle_nodes,
                    classify_stack,
                    writes,
                    parallel_writes,
                    dirty_level_indices,
                },
            ..
        } = self;

        classify_nodes_by_depth(node_parents, n, depth, levels, cycle_nodes, classify_stack);
        if !cycle_nodes.is_empty()
            && cycle_nodes
                .len()
                .saturating_mul(WORLD_PARTIAL_REBUILD_MIN_DENSITY_DIVISOR)
                >= dirty_count
        {
            return false;
        }

        for (i, dirty) in local_dirty.iter_mut().enumerate().take(n) {
            if *dirty {
                local_matrices[i] = render_transform_to_matrix(&nodes[i]);
                *dirty = false;
            }
        }

        for cycle_id in cycle_nodes.iter() {
            if computed.get(*cycle_id).copied().unwrap_or(true) {
                continue;
            }
            logger::trace!(
                "parent cycle at scene {} transform {} -- local-only fallback",
                scene_id,
                cycle_id
            );
            let local = local_matrices[*cycle_id];
            world_matrices[*cycle_id] = local;
            degenerate_scales[*cycle_id] =
                transform_matrix_has_degenerate_scale(&nodes[*cycle_id], local);
            computed[*cycle_id] = true;
        }

        for level in levels.iter() {
            dirty_level_indices.clear();
            dirty_level_indices.extend(
                level
                    .iter()
                    .copied()
                    .filter(|&i| !computed.get(i).copied().unwrap_or(true)),
            );
            if dirty_level_indices.is_empty() {
                continue;
            }

            let local_ro: &[Mat4] = local_matrices;
            let world_ro: &[Mat4] = world_matrices;
            let computed_ro: &[bool] = computed;
            let degen_ro: &[bool] = degenerate_scales;
            let compute_one = |&i: &usize| {
                let local = local_ro[i];
                let p = node_parents.get(i).copied().unwrap_or(-1);
                if p < 0 || (p as usize) >= n || p == i as i32 || !computed_ro[p as usize] {
                    return (
                        local,
                        transform_matrix_has_degenerate_scale(&nodes[i], local),
                    );
                }
                let parent_index = p as usize;
                let world = world_ro[parent_index] * local;
                (
                    world,
                    degen_ro[parent_index]
                        || transform_matrix_has_degenerate_scale(&nodes[i], world),
                )
            };

            writes.clear();
            if should_parallelize_partial_level(dirty_level_indices.len()) {
                collect_bulk_level_parallel(
                    dirty_level_indices,
                    parallel_writes,
                    writes,
                    compute_one,
                );
            } else {
                writes.extend(dirty_level_indices.iter().map(compute_one));
            }
            for (slot, &i) in writes.iter().zip(dirty_level_indices.iter()) {
                world_matrices[i] = slot.0;
                degenerate_scales[i] = slot.1;
                computed[i] = true;
            }
        }

        true
    }

    /// Incremental world matrices: only recomputes indices with `computed[i] == false`.
    pub(super) fn compute_world_matrices_incremental(
        &mut self,
        scene_id: i32,
        nodes: &[RenderTransform],
        node_parents: &[i32],
    ) -> Result<(), SceneError> {
        let world_matrices = &mut self.world_matrices;
        let computed = &mut self.computed;
        let local_matrices = &mut self.local_matrices;
        let local_dirty = &mut self.local_dirty;
        let degenerate_scales = &mut self.degenerate_scales;
        let visit_epoch = &mut self.hierarchy.visit_epoch;
        let walk_epoch = &mut self.hierarchy.walk_epoch;
        let n = nodes.len();
        let mut stack: Vec<usize> = Vec::with_capacity(64.min(n));

        if visit_epoch.len() < n {
            visit_epoch.resize(n, 0);
        }
        if degenerate_scales.len() < n {
            degenerate_scales.resize(n, false);
        }

        for transform_index in (0..n).rev() {
            if computed[transform_index] {
                continue;
            }

            stack.clear();
            *walk_epoch = (*walk_epoch).wrapping_add(1);
            let epoch = *walk_epoch;

            let mut maybe_uppermost_matrix: Option<Mat4> = None;
            let mut id = transform_index;
            let mut cycle_detected = false;

            {
                profiling::scope!("world::upward_walk");
                loop {
                    if id >= n {
                        break;
                    }
                    if computed[id] {
                        maybe_uppermost_matrix = Some(world_matrices[id]);
                        break;
                    }
                    if visit_epoch[id] == epoch {
                        cycle_detected = true;
                        logger::trace!(
                            "parent cycle at scene {} transform {} -- local-only fallback",
                            scene_id,
                            id
                        );
                        break;
                    }
                    visit_epoch[id] = epoch;
                    stack.push(id);
                    let p = node_parents.get(id).copied().unwrap_or(-1);
                    if p < 0 || (p as usize) >= n || p == id as i32 {
                        break;
                    }
                    id = p as usize;
                }
            }

            if cycle_detected {
                for &cid in &stack {
                    if !computed[cid] {
                        let local = get_local_matrix(nodes, local_matrices, local_dirty, cid);
                        world_matrices[cid] = local;
                        degenerate_scales[cid] =
                            transform_matrix_has_degenerate_scale(&nodes[cid], local);
                        computed[cid] = true;
                    }
                }
                continue;
            }

            let (mut parent_matrix, mut parent_degenerate) = if let Some(m) = maybe_uppermost_matrix
            {
                (m, degenerate_scales.get(id).copied().unwrap_or(false))
            } else {
                let Some(top) = stack.pop() else {
                    continue;
                };
                let local = get_local_matrix(nodes, local_matrices, local_dirty, top);
                let degenerate = transform_matrix_has_degenerate_scale(&nodes[top], local);
                world_matrices[top] = local;
                degenerate_scales[top] = degenerate;
                computed[top] = true;
                (local, degenerate)
            };

            while let Some(child_id) = stack.pop() {
                let local = get_local_matrix(nodes, local_matrices, local_dirty, child_id);
                parent_matrix *= local;
                parent_degenerate |=
                    transform_matrix_has_degenerate_scale(&nodes[child_id], parent_matrix);
                world_matrices[child_id] = parent_matrix;
                degenerate_scales[child_id] = parent_degenerate;
                computed[child_id] = true;
            }
        }

        Ok(())
    }
}

/// Ensures cache vectors match `node_count`, invalidates if resized.
pub(super) fn ensure_cache_shapes(
    cache: &mut WorldTransformCache,
    node_count: usize,
    force_invalidate: bool,
) {
    if cache.world_matrices.len() != node_count || cache.degenerate_scales.len() != node_count {
        cache.world_matrices.resize(node_count, Mat4::IDENTITY);
        cache.computed.resize(node_count, false);
        cache.local_matrices.resize(node_count, Mat4::IDENTITY);
        cache.local_dirty.resize(node_count, true);
        cache.degenerate_scales.resize(node_count, false);
        cache.hierarchy.visit_epoch.resize(node_count, 0);
        cache.hierarchy.children_dirty = true;
        cache
            .previous_world_matrices
            .resize(node_count, Mat4::IDENTITY);
        cache.previous_valid.resize(node_count, false);
        cache.motion_history_ready.resize(node_count, false);
        cache.computed.fill(false);
        cache.previous_valid.fill(false);
        cache.motion_history_ready.fill(false);
    } else if force_invalidate {
        cache.computed.fill(false);
    }
}

/// Runs incremental solve if anything is dirty or sizes changed.
///
/// Routes to the level-synchronous parallel bulk rebuild when the cache is fully dirty (typical
/// after a structural change or a force-invalidate) and the node count crosses
/// [`WORLD_BULK_REBUILD_PARALLEL_MIN`]. Smaller spaces and incremental updates keep using the
/// existing serial upward-walk algorithm where the per-node bookkeeping cost still dominates the
/// rayon dispatch overhead.
pub fn compute_world_matrices_for_space(
    scene_id: i32,
    nodes: &[RenderTransform],
    node_parents: &[i32],
    cache: &mut WorldTransformCache,
) -> Result<(), SceneError> {
    profiling::scope!("scene::compute_world_matrices");
    let n = nodes.len();
    if n == 0 {
        *cache = WorldTransformCache::default();
        return Ok(());
    }

    ensure_cache_shapes(cache, n, false);

    if cache.hierarchy.children_dirty {
        rebuild_children(node_parents, n, &mut cache.hierarchy.children);
        cache.hierarchy.children_dirty = false;
    }

    if n >= WORLD_BULK_REBUILD_PARALLEL_MIN && cache_is_fully_dirty(&cache.computed) {
        cache.compute_world_matrices_bulk_rebuild(scene_id, nodes, node_parents);
        return Ok(());
    }

    let dirty_count = dirty_world_matrix_count(&cache.computed);
    if dirty_count == 0 {
        return Ok(());
    }
    if should_parallelize_partial_rebuild(n, dirty_count)
        && cache.compute_world_matrices_partial_rebuild(scene_id, nodes, node_parents, dirty_count)
    {
        return Ok(());
    }

    cache.compute_world_matrices_incremental(scene_id, nodes, node_parents)
}

#[cfg(test)]
mod tests;
