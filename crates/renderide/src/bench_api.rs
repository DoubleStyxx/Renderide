//! CPU-only entry points for the `renderide-bench` crate.
//!
//! These helpers are not part of the renderer process API. They call the same transform, frustum,
//! transient-pool, material-uniform, bind-group, and keyword-variant cache bookkeeping the frame
//! loop uses, and they never open a wgpu device.

use glam::{Mat4, Quat, Vec3};

use crate::render_graph::resources::BufferSizePolicy;
use crate::render_graph::{BufferKey, TransientPool};
use crate::scene::{
    RenderSpaceId, RenderSpaceRead, SceneSpaceRead, SceneTransformRead, SkinnedMeshRenderer,
    WorldTransformCache, compute_world_matrices_for_space, render_matrix_has_degenerate_scale,
    render_transform_has_degenerate_scale, render_transform_to_matrix,
};
use crate::shared::{LayerType, RenderBoundingBox, RenderSH2, RenderTransform, RenderingContext};
use crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip;
use crate::world_mesh::culling::{MeshCullTarget, mesh_world_geometry_for_cull_with_head};

/// Local TRS rows folded by one [`TransformWorkload::fold_matrices`] sample.
pub const TRANSFORM_BATCH: usize = 4_096;

/// World AABBs tested by one [`CullWorkload::visible_count`] sample.
pub const CULL_BATCH: usize = 8_192;

/// Hierarchy nodes solved by one [`WorldMatrixWorkload::recompute`] sample.
pub const WORLD_MATRIX_NODES: usize = 4_096;

/// Distinct buffer keys reused by one [`BufferPoolWorkload::reuse_cycle`] sample.
pub const BUFFER_POOL_SLOTS: usize = 1_024;

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

/// Skinned rows for [`mesh_world_geometry_for_cull_with_head`].
///
/// Each row has a host posed box. The rest mesh box sits far away so a fold of the returned
/// world AABB only matches when the function uses the posed box. Setup allocates the rows;
/// [`Self::fold`] only reads them.
#[derive(Clone, Debug)]
pub struct SkinnedGeometryWorkload {
    scene: SkinnedGeometryScene,
    rest: Vec<RenderBoundingBox>,
    renderers: Vec<SkinnedMeshRenderer>,
}

impl SkinnedGeometryWorkload {
    /// Builds [`CULL_BATCH`] skinned rows.
    #[must_use]
    pub fn standard() -> Self {
        Self::batch(CULL_BATCH)
    }

    /// Builds `count` skinned rows with distinct root translations.
    #[must_use]
    pub fn batch(count: usize) -> Self {
        let mut matrices = Vec::with_capacity(count);
        let mut rest = Vec::with_capacity(count);
        let mut renderers = Vec::with_capacity(count);
        for index in 0..count {
            let step = f32::from(u16::try_from(index % 4_096).unwrap_or(0));
            matrices.push(Mat4::from_translation(Vec3::new(step * 0.01, 0.0, 0.0)));
            rest.push(RenderBoundingBox {
                center: Vec3::new(400.0 + step, 0.0, 0.0),
                extents: Vec3::splat(0.5),
            });
            renderers.push(SkinnedMeshRenderer {
                root_bone_transform_id: Some(i32::try_from(index).unwrap_or(i32::MAX)),
                posed_object_bounds: Some(RenderBoundingBox {
                    center: Vec3::ZERO,
                    extents: Vec3::splat(0.25),
                }),
                ..SkinnedMeshRenderer::default()
            });
        }
        Self {
            scene: SkinnedGeometryScene {
                space_id: RenderSpaceId(166),
                root: RenderTransform {
                    position: Vec3::ZERO,
                    scale: Vec3::ONE,
                    rotation: Quat::IDENTITY,
                },
                matrices,
            },
            rest,
            renderers,
        }
    }

    /// Folds world-AABB minima from [`mesh_world_geometry_for_cull_with_head`].
    #[must_use]
    pub fn fold(&self) -> f32 {
        let mut acc = 0.0_f32;
        for (index, renderer) in self.renderers.iter().enumerate() {
            let geometry = mesh_world_geometry_for_cull_with_head(
                &MeshCullTarget {
                    scene: &self.scene,
                    space_id: self.scene.space_id,
                    mesh_bounds: &self.rest[index],
                    skinned: true,
                    skinned_renderer: Some(renderer),
                    node_id: 0,
                },
                Mat4::IDENTITY,
                RenderingContext::UserView,
            );
            if let Some((min, _)) = geometry.world_aabb {
                acc += min.x;
            }
        }
        acc
    }
}

#[derive(Clone, Debug)]
struct SkinnedGeometryScene {
    space_id: RenderSpaceId,
    root: RenderTransform,
    matrices: Vec<Mat4>,
}

#[derive(Clone, Copy)]
struct SkinnedGeometrySpace<'a> {
    root: &'a RenderTransform,
}

impl<'a> RenderSpaceRead<'a> for SkinnedGeometrySpace<'a> {
    fn is_active(self) -> bool {
        true
    }

    fn is_overlay(self) -> bool {
        false
    }

    fn is_private(self) -> bool {
        false
    }

    fn root_transform(self) -> &'a RenderTransform {
        self.root
    }

    fn view_transform(self) -> &'a RenderTransform {
        self.root
    }

    fn local_transforms(self) -> &'a [RenderTransform] {
        std::slice::from_ref(self.root)
    }

    fn node_parents(self) -> &'a [i32] {
        &[]
    }
}

impl SceneSpaceRead for SkinnedGeometryScene {
    type Space<'a> = SkinnedGeometrySpace<'a>;
    type RenderSpaceIds<'a> = std::iter::Once<RenderSpaceId>;

    fn render_space_ids(&self) -> Self::RenderSpaceIds<'_> {
        std::iter::once(self.space_id)
    }

    fn space(&self, id: RenderSpaceId) -> Option<Self::Space<'_>> {
        (id == self.space_id).then_some(SkinnedGeometrySpace { root: &self.root })
    }

    fn active_main_space(&self) -> Option<Self::Space<'_>> {
        self.space(self.space_id)
    }

    fn active_main_ambient_light(&self) -> RenderSH2 {
        RenderSH2::default()
    }

    fn active_main_render_context(&self) -> RenderingContext {
        RenderingContext::UserView
    }

    fn render_context_affects_draw_prep(&self, _context: RenderingContext) -> bool {
        false
    }
}

impl SceneTransformRead for SkinnedGeometryScene {
    fn world_matrix_for_context(
        &self,
        id: RenderSpaceId,
        transform_index: usize,
        _context: RenderingContext,
    ) -> Option<Mat4> {
        self.world_matrix_for_render_context(
            id,
            transform_index,
            RenderingContext::UserView,
            Mat4::IDENTITY,
        )
    }

    fn world_matrix_for_render_context(
        &self,
        id: RenderSpaceId,
        transform_index: usize,
        _context: RenderingContext,
        _head_output_transform: Mat4,
    ) -> Option<Mat4> {
        if id != self.space_id {
            return None;
        }
        self.matrices.get(transform_index).copied()
    }

    fn overlay_layer_model_matrix_for_context(
        &self,
        _id: RenderSpaceId,
        _transform_index: usize,
        _context: RenderingContext,
    ) -> Option<Mat4> {
        None
    }

    fn transform_special_layer(
        &self,
        _id: RenderSpaceId,
        _transform_index: usize,
    ) -> Option<LayerType> {
        None
    }

    fn transform_is_in_overlay_layer(&self, _id: RenderSpaceId, _transform_index: usize) -> bool {
        false
    }

    fn transform_has_degenerate_scale_for_context(
        &self,
        _id: RenderSpaceId,
        _transform_index: usize,
        _context: RenderingContext,
    ) -> bool {
        false
    }

    fn overridden_material_asset_id(
        &self,
        _space_id: RenderSpaceId,
        _context: RenderingContext,
        _skinned: bool,
        _renderable_index: usize,
        _slot_index: usize,
    ) -> Option<i32> {
        None
    }
}
