//! CPU-only entry points for the `renderide-bench` crate.
//!
//! These helpers are not part of the renderer process API. They call the same transform, frustum,
//! transient-pool, material-uniform, bind-group, and keyword-variant cache bookkeeping the frame
//! loop uses, and they never open a wgpu device.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::{Mat4, Quat, Vec3};
use hashbrown::{HashMap, HashSet};
use rayon::prelude::*;

use crate::camera::reverse_z_perspective;
use crate::gpu_pools::pools::render_texture::{claim_double_buffer_swap, swap_ping_pong};
use crate::hi_z_cpu::project_world_aabb_uv;
use crate::occlusion::hysteresis::{
    CasterKey, CasterRecord, CasterRef, DepthClip, HistoryAction, HistoryInputs, MovedCaster,
    OcclusionFacts, PresentCaster, ScreenRect, SpaceMotion, VisibilityClass, VisibilityCullBuffers,
    VisibilityHistory, VisibilityHysteresis, append_disocclusion_rects, decide_visibility,
    plan_disocclusion, screen_motion_uv_pad,
};
use crate::render_graph::resources::BufferSizePolicy;
use crate::render_graph::{BufferKey, TransientPool};
use crate::scene::{
    RenderSpaceId, WorldTransformCache, compute_world_matrices_for_space,
    render_matrix_has_degenerate_scale, render_transform_has_degenerate_scale,
    render_transform_to_matrix,
};
use crate::shared::RenderTransform;
use crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip;

mod skinned_geometry;
pub use skinned_geometry::SkinnedGeometryWorkload;

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

/// Double-buffered cameras swapped by one [`DoubleBufferWorkload::steady_frame`] sample.
pub const DOUBLE_BUFFER_CAMERAS: usize = 32;

/// Static many-light shadow planning workload.
pub use crate::backend::{SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload};
/// Custom-material uniform packing and bind-group cache workloads.
pub use crate::materials::{
    BIND_GROUP_BATCH, BindGroupChurnWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
};
/// Custom-shader keyword variant cache and per-frame stem lookup workloads.
pub use crate::materials::{CustomStemLookupWorkload, KeywordVariantCacheWorkload};
/// Cameras folded by one [`camera_clear_load_op_checksum`] sample.
pub const CAMERA_CLEAR_LOAD_OP_CAMERAS: usize = crate::passes::CAMERA_CLEAR_LOAD_OP_CAMERAS;

/// One synthetic camera for [`camera_clear_load_op_checksum`].
pub use crate::passes::CameraClearLoadOpInput;

/// Seeded, non-periodic load-op inputs. Build this once, outside the timed loop.
#[must_use]
pub fn camera_clear_load_op_inputs(cameras: usize) -> Vec<CameraClearLoadOpInput> {
    crate::passes::camera_clear_load_op_inputs(cameras)
}

/// Allocation-free checksum of per-camera color and depth load ops.
///
/// Each input is passed through [`std::hint::black_box`] inside the loop.
#[must_use]
pub fn camera_clear_load_op_checksum(inputs: &[CameraClearLoadOpInput]) -> u32 {
    crate::passes::camera_clear_load_op_checksum(inputs)
}

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

    /// Publishes rigid-motion history for `nodes` into reused previous-matrix storage.
    ///
    /// Returns that storage's capacity. After the first publish for a fixed node count, further
    /// publishes must return the same capacity. `nodes` is the object-mode subset; it is not
    /// allocated inside the method.
    #[must_use]
    pub fn publish_motion_history(&mut self, nodes: &[usize]) -> usize {
        self.cache.seal_motion_history(nodes);
        self.cache.publish_previous_world_matrices(nodes);
        self.cache.previous_world_matrix_capacity()
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
#[derive(Debug)]
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
            CasterRecord::new(
                VisibilityClass::Occluded,
                bounds,
                Vec3::new(0.5, 0.5, -1.0),
                1,
            ),
            CasterRecord::new(
                VisibilityClass::Visible,
                bounds,
                Vec3::new(0.5, 0.5, -1.0),
                1,
            ),
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
    present_keys: HashSet<CasterKey>,
    movers: Vec<MovedCaster>,
    previous_vp: Mat4,
    current_vp: Mat4,
    history: VisibilityHistory,
    spare: HashMap<CasterKey, CasterRecord>,
    buffers: VisibilityCullBuffers,
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
            casters.insert(key, CasterRecord::new(class, aabb_min, aabb_max, 1));
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
            present_keys: HashSet::new(),
            movers: Vec::new(),
            previous_vp,
            current_vp,
            history: VisibilityHistory::from_casters(casters),
            spare: HashMap::new(),
            buffers: VisibilityCullBuffers::default(),
        }
    }

    /// Plans disocclusion, reprojects every caster, records the decision, and publishes history.
    ///
    /// The history map, spare map, mover list, and cull buffers stay on the workload. The next
    /// call clears and refills them, matching the view slot's recycle path.
    #[must_use]
    pub fn step(&mut self) -> u32 {
        let history = std::mem::take(&mut self.history);
        let spare = std::mem::take(&mut self.spare);
        let mut buffers = std::mem::take(&mut self.buffers);
        plan_disocclusion(
            &history,
            &self.present,
            &mut self.present_keys,
            |_| true,
            &mut self.movers,
        );
        let current_vp = self.current_vp;
        let previous_vp = self.previous_vp;
        buffers.stale_rects.clear();
        append_disocclusion_rects(&mut buffers.stale_rects, &self.movers, |min, max| {
            project_screen(current_vp, min, max)
        });
        buffers.motions.clear();
        buffers.motions.insert(
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
        let frame = VisibilityHysteresis::from_parts(history, spare, buffers);
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
        let recycled = frame.recycle();
        self.history = recycled.history;
        self.spare = recycled.spare;
        self.buffers = recycled.buffers;
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
    let previous = glam::camera::rh::view::look_at_mat4(
        Vec3::new(0.0, 1.5, 12.0),
        Vec3::new(0.0, 1.5, 0.0),
        Vec3::Y,
    );
    let current = if moving {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(2.0, 1.5, 12.0),
            Vec3::new(2.0, 1.5, 0.0),
            Vec3::Y,
        )
    } else {
        previous
    };
    (projection * previous, projection * current)
}

/// Per-frame double-buffer bookkeeping for cameras that already own a second color image.
///
/// The measured path is the same claim-once scan and three-slot swap the renderer runs after the
/// history image exists. It does not create wgpu textures; that allocation happens once, outside
/// the steady frame.
pub struct DoubleBufferWorkload {
    scratch: Vec<i32>,
    published_texture: Vec<u64>,
    previous_texture: Vec<u64>,
    published_view: Vec<u64>,
    previous_view: Vec<u64>,
    published_generation: Vec<u64>,
    previous_generation: Vec<u64>,
}

impl DoubleBufferWorkload {
    /// Builds `DOUBLE_BUFFER_CAMERAS` already-allocated ping-pong slots and reserves the scratch.
    #[must_use]
    pub fn standard() -> Self {
        let cameras = DOUBLE_BUFFER_CAMERAS;
        let mut scratch = Vec::with_capacity(cameras);
        let mut published_texture = Vec::with_capacity(cameras);
        let mut previous_texture = Vec::with_capacity(cameras);
        let mut published_view = Vec::with_capacity(cameras);
        let mut previous_view = Vec::with_capacity(cameras);
        let mut published_generation = Vec::with_capacity(cameras);
        let mut previous_generation = Vec::with_capacity(cameras);
        for index in 0..cameras {
            let id = u64::try_from(index).unwrap_or(0);
            published_texture.push(id);
            previous_texture.push(id.wrapping_add(1_000));
            published_view.push(id.wrapping_add(2_000));
            previous_view.push(id.wrapping_add(3_000));
            published_generation.push(id.wrapping_add(4_000));
            previous_generation.push(id.wrapping_add(5_000));
        }
        // Pay the scratch growth before the measured loop.
        for index in 0..cameras {
            let asset_id = i32::try_from(index).unwrap_or(0);
            let _ = claim_double_buffer_swap(&mut scratch, asset_id);
        }
        scratch.clear();
        Self {
            scratch,
            published_texture,
            previous_texture,
            published_view,
            previous_view,
            published_generation,
            previous_generation,
        }
    }

    /// Claims each camera once and swaps its three slots. Returns the published generation sum.
    #[must_use]
    pub fn steady_frame(&mut self) -> u64 {
        self.scratch.clear();
        let mut checksum = 0_u64;
        for index in 0..self.published_generation.len() {
            let asset_id = i32::try_from(index).unwrap_or(0);
            if !claim_double_buffer_swap(&mut self.scratch, asset_id) {
                continue;
            }
            swap_ping_pong(
                &mut self.published_texture[index],
                &mut self.previous_texture[index],
            );
            swap_ping_pong(
                &mut self.published_view[index],
                &mut self.previous_view[index],
            );
            swap_ping_pong(
                &mut self.published_generation[index],
                &mut self.previous_generation[index],
            );
            checksum = checksum.wrapping_add(self.published_generation[index]);
        }
        checksum
    }

    /// Capacity of the reused id scratch after [`Self::standard`].
    #[must_use]
    pub fn scratch_capacity(&self) -> usize {
        self.scratch.capacity()
    }
}

/// Mixed transparent draws sorted by one [`TransparentSortWorkload`] sample at about a thousand rows.
pub const TRANSPARENT_SORT_1K: usize = 1_024;

/// Mixed transparent draws sorted by one [`TransparentSortWorkload`] sample at about ten thousand rows.
pub const TRANSPARENT_SORT_10K: usize = 10_240;

/// Unsorted transparent draws, half canvas stems and half world stems.
///
/// `sort_precomputed` reads the `f64` stored on the draw. `sort_recomputing_depth` rebuilds that
/// key on every compare, including the canvas stem-name match. `sort_master` is the `117a8df5`
/// farthest-corner comparator: it compares `camera_distance_sq` and does not match canvas stems.
/// Canvas-versus-world pairs can disagree between `sort_master` and `sort_precomputed`.
#[derive(Clone, Debug)]
pub struct TransparentSortWorkload {
    items: Vec<crate::world_mesh::WorldMeshDrawItem>,
}

impl TransparentSortWorkload {
    /// Builds `count` transparent draws with shuffled distances and equal `sorting_order`.
    ///
    /// Equal sorting order forces the comparator into the depth-key tie, which is the cost this
    /// bench measures. Canvas rows cycle `ui_unlit`, `ui_textunlit`, and `ui_circlesegment`.
    /// World rows use non-canvas embedded stems so the stem match still runs and returns false.
    #[must_use]
    pub fn mixed(count: usize) -> Self {
        use crate::materials::RasterPipelineKind;
        use crate::world_mesh::draw_prep::{pack_sort_prefix, sync_transparent_depth_key};
        use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};

        let mut items = Vec::with_capacity(count);
        for index in 0..count {
            let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
                material_asset_id: i32::try_from(index % 64).unwrap_or(0),
                property_block: None,
                skinned: false,
                sorting_order: 0,
                mesh_asset_id: 1,
                node_id: i32::try_from(index).unwrap_or(i32::MAX),
                slot_index: 0,
                collect_order: index,
                alpha_blended: true,
            });
            let mixed = index.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let meters = 0.4 + (mixed % 20_000) as f32 * 0.001;
            if index % 2 == 0 {
                let stem = match index % 6 {
                    0 => "ui_unlit_default",
                    2 => "ui_textunlit_multiview",
                    _ => "ui_circlesegment",
                };
                item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from(stem));
                item.is_canvas_ui = true;
                item.canvas_plane_depth = Some(meters);
            } else {
                let stem = if index % 4 == 1 {
                    "unlit_default"
                } else {
                    "pbs_metallic_default"
                };
                item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from(stem));
                item.is_canvas_ui = false;
                item.canvas_plane_depth = None;
            }
            item.camera_distance_sq = meters * meters;
            sync_transparent_depth_key(&mut item);
            item.sort_prefix = pack_sort_prefix(
                item.is_overlay,
                item.batch_key.render_queue,
                item.batch_key.uses_transparent_sorting(),
                item._opaque_depth_bucket,
                item.batch_key_hash,
            );
            items.push(item);
        }
        Self { items }
    }

    /// A canvas whose plane is farther than a world draw's farthest corner, while the canvas
    /// corner itself is nearer.
    ///
    /// Master orders the world draw first. The plane key orders the canvas first.
    #[must_use]
    pub fn plane_versus_corner() -> Self {
        use crate::materials::RasterPipelineKind;
        use crate::world_mesh::draw_prep::{pack_sort_prefix, sync_transparent_depth_key};
        use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};

        let mut items = Vec::with_capacity(2);
        for (index, canvas) in [true, false].into_iter().enumerate() {
            let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
                material_asset_id: 1,
                property_block: None,
                skinned: false,
                sorting_order: 0,
                mesh_asset_id: 1,
                node_id: i32::try_from(index).unwrap_or(0),
                slot_index: 0,
                collect_order: index,
                alpha_blended: true,
            });
            if canvas {
                item.batch_key.pipeline =
                    RasterPipelineKind::EmbeddedStem(Arc::from("ui_unlit_default"));
                item.is_canvas_ui = true;
                item.canvas_plane_depth = Some(9.0);
                item.camera_distance_sq = 1.0;
            } else {
                item.batch_key.pipeline =
                    RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default"));
                item.is_canvas_ui = false;
                item.canvas_plane_depth = None;
                item.camera_distance_sq = 64.0;
            }
            sync_transparent_depth_key(&mut item);
            item.sort_prefix = pack_sort_prefix(
                item.is_overlay,
                item.batch_key.render_queue,
                item.batch_key.uses_transparent_sorting(),
                item._opaque_depth_bucket,
                item.batch_key_hash,
            );
            items.push(item);
        }
        Self { items }
    }

    /// Sorts a clone with the production precomputed-key comparator.
    #[must_use]
    pub fn sort_precomputed(self) -> u64 {
        let mut items = self.items;
        crate::world_mesh::draw_prep::sort_order_sensitive_draws_precomputed(&mut items)
    }

    /// Sorts a clone while recomputing the depth key on every compare.
    #[must_use]
    pub fn sort_recomputing_depth(self) -> u64 {
        let mut items = self.items;
        crate::world_mesh::draw_prep::sort_order_sensitive_draws_recomputing_depth(&mut items)
    }

    /// Sorts a clone with the master farthest-corner comparator.
    #[must_use]
    pub fn sort_master(self) -> u64 {
        let mut items = self.items;
        crate::world_mesh::draw_prep::sort_order_sensitive_draws_master(&mut items)
    }

    /// Canvas rows jittered by about `1e-6` m inside a `0.1` mm snap cluster.
    ///
    /// [`Self::mixed`] sits on a 1 mm grid, which never writes during the snap pass. This
    /// constructor is the input for [`Self::snap_siblings`].
    #[must_use]
    pub fn jittered_canvas_planes(count: usize) -> Self {
        use crate::world_mesh::draw_prep::sync_transparent_depth_key;

        let mut workload = Self::mixed(count);
        for (index, item) in workload.items.iter_mut().enumerate() {
            if !item.is_canvas_ui {
                continue;
            }
            let cluster = (index / 8) as f32 * 0.01;
            let jitter = ((index % 8) as f32 - 3.5) * 1.0e-6;
            item.canvas_plane_depth = Some(1.0 + cluster + jitter);
            sync_transparent_depth_key(item);
        }
        workload
    }

    /// Snaps one chunk of sibling plane depths and folds the stored depths into a checksum.
    #[must_use]
    pub fn snap_siblings(self) -> u64 {
        let mut chunks = vec![self.items];
        crate::world_mesh::draw_prep::snap_canvas_sibling_plane_depths(&mut chunks);
        let mut checksum = 0_u64;
        for item in &chunks[0] {
            if let Some(depth) = item.canvas_plane_depth {
                checksum = checksum.wrapping_add(u64::from(depth.to_bits()));
            }
            checksum = checksum.wrapping_add(item.transparent_depth_key.to_bits());
        }
        checksum
    }
}

/// Perspective view-projection aimed at the origin from `+Z`.
///
/// Depth is wgpu NDC z in [0, 1] (near → 0, far → 1), the same interval
/// [`crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip`] tests.
fn bench_view_proj() -> Mat4 {
    let view = glam::camera::rh::view::look_at_mat4(Vec3::new(0.0, 1.5, 6.0), Vec3::ZERO, Vec3::Y);
    let projection = glam::camera::rh::proj::directx::perspective(
        60.0_f32.to_radians(),
        16.0 / 9.0,
        0.05,
        250.0,
    );
    projection * view
}
