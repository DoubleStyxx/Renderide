//! CPU-only entry points for the `renderide-bench` crate.
//!
//! These helpers are not part of the renderer process API. They call the same transform, frustum,
//! transient-pool, material-uniform, bind-group, and keyword-variant cache bookkeeping the frame
//! loop uses, and they never open a wgpu device.

use glam::{Mat4, Quat, Vec3};

use crate::gpu_pools::pools::render_texture::{claim_double_buffer_swap, swap_ping_pong};

use crate::render_graph::resources::BufferSizePolicy;
use crate::render_graph::{BufferKey, TransientPool};
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

/// Hierarchy nodes solved by one [`WorldMatrixWorkload::recompute`] sample.
pub const WORLD_MATRIX_NODES: usize = 4_096;

/// Distinct buffer keys reused by one [`BufferPoolWorkload::reuse_cycle`] sample.
pub const BUFFER_POOL_SLOTS: usize = 1_024;

/// Double-buffered cameras swapped by one [`DoubleBufferWorkload::steady_frame`] sample.
pub const DOUBLE_BUFFER_CAMERAS: usize = 32;

/// Static many-light shadow planning workload.
pub use crate::backend::{SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload};
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
