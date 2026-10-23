//! CPU-only entry points for the `renderide-bench` crate.
//!
//! These helpers are not part of the renderer process API. They call the same transform, frustum,
//! transient-pool, material-uniform, bind-group, and keyword-variant cache bookkeeping the frame
//! loop uses, and they never open a wgpu device.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::{Mat4, Quat, Vec3};
use hashbrown::HashMap;
use rayon::prelude::*;

use crate::camera::reverse_z_perspective;
use crate::hi_z_cpu::project_world_aabb_uv;
use crate::occlusion::hysteresis::{
    CasterKey, CasterRecord, CasterRef, DepthClip, HistoryAction, HistoryInputs, OcclusionFacts,
    PresentCaster, ScreenRect, SpaceMotion, VisibilityClass, VisibilityHistory,
    VisibilityHysteresis, decide_visibility, disocclusion_rects, plan_disocclusion,
    screen_motion_uv_pad,
};
use crate::render_graph::resources::BufferSizePolicy;
use crate::render_graph::{BufferKey, TransientPool};
use crate::scene::RenderSpaceId;
use crate::scene::{
    WorldTransformCache, compute_world_matrices_for_space, render_matrix_has_degenerate_scale,
    render_transform_has_degenerate_scale, render_transform_to_matrix,
};
use crate::shared::RenderTransform;
use crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip;

/// Local TRS rows folded by one [`TransformWorkload::fold_matrices`] sample.
pub const TRANSFORM_BATCH: usize = 4_096;

/// World AABBs tested by one [`CullWorkload::visible_count`] sample.
pub const CULL_BATCH: usize = 8_192;

/// Casters classified by one [`HysteresisCullWorkload::kept_count`] sample.
pub const HYSTERESIS_BATCH: usize = 4_096;

/// Caster counts timed by [`HysteresisHistoryWorkload`].
///
/// The first entry is [`crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET`], the production
/// cutoff. Larger counts measure the uncapped history step that cutoff skips. On a 4-core guest
/// that step was about 13 ms at 100_000 casters, past an 11.1 ms 90 Hz frame.
pub const HYSTERESIS_HISTORY_COUNTS: [usize; 4] = [
    crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET,
    10_000,
    50_000,
    100_000,
];

/// Screen-motion pad below which the history bench treats the pyramid as still occluding.
///
/// The bench does not sample a Hi-Z pyramid. This stand-in keeps the timed path on
/// `plan_disocclusion`, the per-caster reprojection, the history lookup, and `finish`.
const HISTORY_CONFIRM_UV: f32 = 0.02;

/// Hierarchy nodes solved by one [`WorldMatrixWorkload::recompute`] sample.
pub const WORLD_MATRIX_NODES: usize = 4_096;

/// Distinct buffer keys reused by one [`BufferPoolWorkload::reuse_cycle`] sample.
pub const BUFFER_POOL_SLOTS: usize = 1_024;

/// Custom-shader keyword variant cache workload.
pub use crate::materials::KeywordVariantCacheWorkload;
/// Custom-material uniform packing and bind-group cache workloads.
pub use crate::materials::{
    BIND_GROUP_BATCH, BindGroupChurnWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
};

/// Sibling group width for [`WorldMatrixWorkload`]. Index 0 of each group is a root.
const WORLD_MATRIX_GROUP: usize = 32;

/// Grid width used to scatter cull boxes around the origin.
const CULL_GRID_SIDE: usize = 96;

/// Local poses for the TRS-to-matrix and degenerate-scale helpers.
#[derive(Clone, Debug)]
pub struct TransformWorkload {
    rows: Vec<RenderTransform>,
}

impl TransformWorkload {
    /// Builds [`TRANSFORM_BATCH`] varied poses.
    #[must_use]
    pub fn standard() -> Self {
        Self::varied(TRANSFORM_BATCH)
    }

    /// Builds `count` poses with distinct translation and yaw.
    #[must_use]
    pub fn varied(count: usize) -> Self {
        let rows = (0..count)
            .map(|index| {
                let step = u16::try_from(index % 3_600).unwrap_or(0);
                RenderTransform {
                    position: Vec3::new(f32::from(step) * 0.01, 0.25, -1.5),
                    scale: Vec3::new(1.0, if index.is_multiple_of(17) { 0.0 } else { 1.0 }, 1.0),
                    rotation: Quat::from_rotation_y(f32::from(step) * 0.001),
                }
            })
            .collect();
        Self { rows }
    }

    /// Folds TRS matrix conversion and the degenerate-scale checks into one scalar.
    #[must_use]
    pub fn fold_matrices(&self) -> f32 {
        let mut acc = 0.0_f32;
        for row in &self.rows {
            let matrix = render_transform_to_matrix(row);
            let degenerate = u8::from(render_transform_has_degenerate_scale(row))
                + u8::from(render_matrix_has_degenerate_scale(matrix));
            acc += matrix.w_axis.x + matrix.x_axis.x + f32::from(degenerate);
        }
        acc
    }
}

/// Prebuilt world AABBs and the view-projection used by homogeneous clip culling.
#[derive(Clone, Debug)]
pub struct CullWorkload {
    view_proj: Mat4,
    mins: Vec<Vec3>,
    maxs: Vec<Vec3>,
}

impl CullWorkload {
    /// Builds [`CULL_BATCH`] boxes, some inside the camera frustum and some outside it.
    #[must_use]
    pub fn standard() -> Self {
        Self::grid(CULL_BATCH)
    }

    /// Scatters `count` unit boxes on the XZ plane around the origin.
    #[must_use]
    pub fn grid(count: usize) -> Self {
        let mut mins = Vec::with_capacity(count);
        let mut maxs = Vec::with_capacity(count);
        for index in 0..count {
            let column = index % CULL_GRID_SIDE;
            let row = index / CULL_GRID_SIDE;
            let x = f32::from(u16::try_from(column).unwrap_or(0)) - 48.0;
            let z = f32::from(u16::try_from(row).unwrap_or(0)) - 48.0;
            let min = Vec3::new(x, 0.0, z);
            mins.push(min);
            maxs.push(min + Vec3::splat(0.5));
        }
        Self {
            view_proj: bench_view_proj(),
            mins,
            maxs,
        }
    }

    /// Counts boxes that survive the homogeneous clip AABB test.
    #[must_use]
    pub fn visible_count(&self) -> u32 {
        let mut visible = 0_u32;
        for (min, max) in self.mins.iter().zip(self.maxs.iter()) {
            if world_aabb_visible_in_homogeneous_clip(self.view_proj, *min, *max) {
                visible += 1;
            }
        }
        visible
    }
}

/// Parent-chain world-matrix solve over a reused world-transform cache.
#[derive(Debug)]
pub struct WorldMatrixWorkload {
    nodes: Vec<RenderTransform>,
    parents: Vec<i32>,
    cache: WorldTransformCache,
}

impl WorldMatrixWorkload {
    /// Builds [`WORLD_MATRIX_NODES`] nodes grouped into roots plus direct children.
    #[must_use]
    pub fn standard() -> Self {
        Self::forest(WORLD_MATRIX_NODES)
    }

    /// Builds `count` nodes. Every [`WORLD_MATRIX_GROUP`]th node is a root.
    #[must_use]
    pub fn forest(count: usize) -> Self {
        let mut nodes = Vec::with_capacity(count);
        let mut parents = Vec::with_capacity(count);
        for index in 0..count {
            let grouped = index % WORLD_MATRIX_GROUP == 0;
            let parent = if grouped {
                -1
            } else {
                i32::try_from(index - (index % WORLD_MATRIX_GROUP)).unwrap_or(-1)
            };
            let group = index / WORLD_MATRIX_GROUP;
            let x = if grouped {
                f32::from(u16::try_from(group).unwrap_or(0))
            } else {
                1.0
            };
            nodes.push(RenderTransform {
                position: Vec3::new(x, 0.0, 0.0),
                scale: Vec3::ONE,
                rotation: Quat::IDENTITY,
            });
            parents.push(parent);
        }
        Self {
            nodes,
            parents,
            cache: WorldTransformCache::default(),
        }
    }

    /// Builds one parent chain stored child-before-parent.
    ///
    /// Node `i` parents `i + 1`, and the last node is the root. Bulk rebuild classifies this
    /// shape on every full recompute; the chain is long enough that a per-depth sweep is quadratic.
    #[must_use]
    pub fn reverse_chain(count: usize) -> Self {
        let mut nodes = Vec::with_capacity(count);
        let mut parents = Vec::with_capacity(count);
        for index in 0..count {
            let is_root = index + 1 == count;
            nodes.push(RenderTransform {
                position: Vec3::new(if is_root { 0.0 } else { 1.0 }, 0.0, 0.0),
                scale: Vec3::ONE,
                rotation: Quat::IDENTITY,
            });
            let parent = if is_root {
                -1
            } else {
                i32::try_from(index + 1).unwrap_or(-1)
            };
            parents.push(parent);
        }
        Self {
            nodes,
            parents,
            cache: WorldTransformCache::default(),
        }
    }

    /// Invalidates the cache and solves world matrices.
    ///
    /// Returns the world-space X of node 1 (child of root 0), which is `1` for [`Self::forest`].
    /// For [`Self::reverse_chain`] that node sits `count - 2` steps below the root, so the value
    /// is `count - 2` when every step translates `+1` on X.
    #[must_use]
    pub fn recompute(&mut self) -> f32 {
        self.cache.computed.fill(false);
        if compute_world_matrices_for_space(0, &self.nodes, &self.parents, &mut self.cache).is_err()
        {
            return 0.0;
        }
        self.cache
            .world_matrices
            .get(1)
            .map(|matrix| matrix.w_axis.x)
            .unwrap_or(0.0)
    }
}

/// Free-list reuse of transient-pool buffer keys. No GPU buffer is allocated.
#[derive(Debug)]
pub struct BufferPoolWorkload {
    pool: TransientPool,
    keys: Vec<BufferKey>,
    ids: Vec<usize>,
}

impl BufferPoolWorkload {
    /// Fills [`BUFFER_POOL_SLOTS`] distinct keys, then releases them so the next cycle hits.
    #[must_use]
    pub fn standard() -> Self {
        Self::with_slots(BUFFER_POOL_SLOTS)
    }

    /// Allocates `slots` distinct key-only buffer entries and returns them to the free list.
    #[must_use]
    pub fn with_slots(slots: usize) -> Self {
        let mut pool = TransientPool::new();
        pool.begin_generation();
        let mut keys = Vec::with_capacity(slots);
        let mut ids = Vec::with_capacity(slots);
        for index in 0..slots {
            let size_index = u64::try_from(index).unwrap_or(0);
            let key = BufferKey {
                size_policy: BufferSizePolicy::Fixed(256 + size_index * 16),
                usage_bits: 0x0020,
            };
            let id = pool.acquire_buffer(key);
            keys.push(key);
            ids.push(id);
        }
        for id in &ids {
            pool.release_buffer(*id);
        }
        Self { pool, keys, ids }
    }

    /// Acquires every key again (free-list hit) and releases it.
    ///
    /// The returned checksum depends on the recycled entry ids.
    #[must_use]
    pub fn reuse_cycle(&mut self) -> usize {
        self.pool.begin_generation();
        let mut checksum = 0_usize;
        for index in 0..self.keys.len() {
            let key = self.keys[index];
            let id = self.pool.acquire_buffer(key);
            self.ids[index] = id;
            checksum = checksum.wrapping_add(id);
        }
        for id in &self.ids {
            self.pool.release_buffer(*id);
        }
        checksum
    }
}

/// Owned history-confirmation query reused by [`HysteresisCullWorkload`].
#[derive(Clone, Debug)]
struct HysteresisQuery {
    /// Index into the workload's caster records. `None` is a newly revealed caster.
    previous: Option<usize>,
    /// Bounds moved since the stored record.
    caster_moved: bool,
    /// Screen rect hits a moved occluder.
    overlaps_disocclusion: bool,
    /// Previous pyramid reports full occlusion.
    hi_z_occludes: bool,
    /// Covering depth is a separate occluder.
    other_occluder: bool,
}

/// Classifies a fixed mix of stable occluded casters and disoccluded casters.
#[derive(Clone, Debug)]
pub struct HysteresisCullWorkload {
    records: Vec<CasterRecord>,
    queries: Vec<HysteresisQuery>,
}

impl HysteresisCullWorkload {
    /// Builds [`HYSTERESIS_BATCH`] queries: one quarter stay culled, three quarters must be kept.
    #[must_use]
    pub fn standard() -> Self {
        let bounds = Vec3::new(-0.5, -0.5, -2.0);
        let records = vec![
            CasterRecord {
                class: VisibilityClass::Occluded,
                aabb_min: bounds,
                aabb_max: Vec3::new(0.5, 0.5, -1.0),
                mesh_asset_id: 1,
            },
            CasterRecord {
                class: VisibilityClass::Visible,
                aabb_min: bounds,
                aabb_max: Vec3::new(0.5, 0.5, -1.0),
                mesh_asset_id: 1,
            },
        ];
        let queries = (0..HYSTERESIS_BATCH)
            .map(|index| match index % 4 {
                0 => HysteresisQuery {
                    previous: Some(0),
                    caster_moved: false,
                    overlaps_disocclusion: false,
                    hi_z_occludes: true,
                    other_occluder: false,
                },
                1 => HysteresisQuery {
                    previous: None,
                    caster_moved: false,
                    overlaps_disocclusion: false,
                    hi_z_occludes: true,
                    other_occluder: false,
                },
                2 => HysteresisQuery {
                    previous: Some(0),
                    caster_moved: false,
                    overlaps_disocclusion: true,
                    hi_z_occludes: true,
                    other_occluder: false,
                },
                _ => HysteresisQuery {
                    previous: Some(1),
                    caster_moved: false,
                    overlaps_disocclusion: false,
                    hi_z_occludes: false,
                    other_occluder: false,
                },
            })
            .collect();
        Self { records, queries }
    }

    /// Number of casters history confirmation still draws.
    #[must_use]
    pub fn kept_count(&self) -> u32 {
        let mut kept = 0_u32;
        for query in &self.queries {
            let previous = query.previous.and_then(|index| self.records.get(index));
            let decision = decide_visibility(HistoryInputs {
                previous,
                caster_moved: query.caster_moved,
                overlaps_disocclusion: query.overlaps_disocclusion,
                hi_z_occludes: query.hi_z_occludes,
                other_occluder: query.other_occluder,
            });
            if decision.action != HistoryAction::Culled {
                kept = kept.saturating_add(1);
            }
        }
        kept
    }
}

/// History lookup, screen-motion pad, and `finish` over a prebuilt caster set.
///
/// Hi-Z pyramid sampling is not included. A pad under [`HISTORY_CONFIRM_UV`] stands in for a
/// pyramid that still covers the expanded rectangle. This is the uncapped cost of
/// `plan_disocclusion`, the per-caster reprojection, the history lookup, and `finish`.
/// Production does not run it once the prepared caster count passes
/// [`crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET`]. The sequential prepared-draw scan
/// that feeds the plan is a separate, cheaper walk and stops at that same cap. The count is
/// prepared casters in scope, not casters that survive the frustum.
pub struct HysteresisHistoryWorkload {
    present: Vec<PresentCaster>,
    previous_vp: Mat4,
    current_vp: Mat4,
    history: Arc<VisibilityHistory>,
    spare: HashMap<CasterKey, CasterRecord>,
}

impl HysteresisHistoryWorkload {
    /// Builds `count` casters. One quarter start occluded. `moving` separates the cameras.
    #[must_use]
    pub fn new(count: usize, moving: bool) -> Self {
        let (previous_vp, current_vp) = history_view_projections(moving);
        let mut side = 1_usize;
        while side.saturating_mul(side) < count.max(1) {
            side = side.saturating_add(1);
        }
        let mut casters = HashMap::with_capacity(count);
        let mut present = Vec::with_capacity(count);
        for index in 0..count {
            let column = index % side;
            let row = index / side;
            let x = f32::from(u16::try_from(column).unwrap_or(0)).mul_add(0.45, -8.0);
            let z = f32::from(u16::try_from(row).unwrap_or(0)).mul_add(0.45, -4.0);
            let aabb_min = Vec3::new(x, 0.0, z);
            let aabb_max = aabb_min + Vec3::splat(0.2);
            let node_id = i32::try_from(index).unwrap_or(i32::MAX);
            let key = CasterKey::new(
                RenderSpaceId(1),
                index,
                u64::try_from(index).unwrap_or(u64::MAX),
                node_id,
                false,
            );
            let class = if index % 4 == 0 {
                VisibilityClass::Occluded
            } else {
                VisibilityClass::Visible
            };
            casters.insert(
                key,
                CasterRecord {
                    class,
                    aabb_min,
                    aabb_max,
                    mesh_asset_id: 1,
                },
            );
            present.push(PresentCaster {
                key,
                mesh_asset_id: 1,
                aabb_min,
                aabb_max,
                space_id: RenderSpaceId(1),
            });
        }
        Self {
            present,
            previous_vp,
            current_vp,
            history: Arc::new(VisibilityHistory::from_casters(casters)),
            spare: HashMap::new(),
        }
    }

    /// Plans disocclusion, reprojects every caster, records the decision, and publishes history.
    ///
    /// The published map's allocation is recycled into the spare for the next call, matching
    /// [`crate::occlusion::OcclusionSystem::store_visibility_history`].
    #[must_use]
    pub fn step(&mut self) -> u32 {
        let movers = plan_disocclusion(&self.history, &self.present, |_| true);
        let current_vp = self.current_vp;
        let previous_vp = self.previous_vp;
        let stale = disocclusion_rects(&movers, |min, max| project_screen(current_vp, min, max));
        let mut motions = HashMap::new();
        motions.insert(
            RenderSpaceId(1),
            SpaceMotion {
                mono: Some((previous_vp, current_vp)),
                stereo: None,
                clip: DepthClip {
                    near: 0.05,
                    far: 200.0,
                    orthographic: false,
                },
            },
        );
        let frame = VisibilityHysteresis::new(
            Arc::clone(&self.history),
            std::mem::take(&mut self.spare),
            motions,
            stale,
        );
        let kept = AtomicU32::new(0);
        self.present.par_iter().for_each(|caster| {
            let facts = facts_for_motion_pad(screen_motion_uv_pad(
                previous_vp,
                current_vp,
                caster.aabb_min,
                caster.aabb_max,
            ));
            if frame.allows_draw(
                Some(CasterRef {
                    key: caster.key,
                    mesh_asset_id: caster.mesh_asset_id,
                }),
                caster.aabb_min,
                caster.aabb_max,
                facts,
            ) {
                kept.fetch_add(1, Ordering::Relaxed);
            }
        });
        let next = frame.finish();
        let old = std::mem::replace(&mut self.history, Arc::new(next));
        if let Ok(old) = Arc::try_unwrap(old) {
            let mut recycled = old.into_casters();
            recycled.clear();
            self.spare = recycled;
        }
        kept.load(Ordering::Relaxed)
    }
}

fn facts_for_motion_pad(pad: Option<f32>) -> OcclusionFacts {
    match pad {
        Some(pad) if pad < HISTORY_CONFIRM_UV => OcclusionFacts {
            occluded: true,
            other_occluder: true,
            rects: [None, None],
        },
        _ => OcclusionFacts::keep(),
    }
}

fn project_screen(view_proj: Mat4, min: Vec3, max: Vec3) -> Option<ScreenRect> {
    let (uv_min, uv_max) = project_world_aabb_uv(view_proj, min, max)?;
    ScreenRect::from_uv(uv_min, uv_max)
}

fn history_view_projections(moving: bool) -> (Mat4, Mat4) {
    let projection = reverse_z_perspective(1.0, 70.0_f32.to_radians(), 0.05, 200.0);
    let previous = Mat4::look_at_rh(Vec3::new(0.0, 1.5, 12.0), Vec3::new(0.0, 1.5, 0.0), Vec3::Y);
    let current = if moving {
        Mat4::look_at_rh(Vec3::new(2.0, 1.5, 12.0), Vec3::new(2.0, 1.5, 0.0), Vec3::Y)
    } else {
        previous
    };
    (projection * previous, projection * current)
}

/// Perspective view-projection aimed at the origin from `+Z`.
fn bench_view_proj() -> Mat4 {
    let view = Mat4::look_at_rh(Vec3::new(0.0, 1.5, 6.0), Vec3::ZERO, Vec3::Y);
    let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.05, 250.0);
    projection * view
}
