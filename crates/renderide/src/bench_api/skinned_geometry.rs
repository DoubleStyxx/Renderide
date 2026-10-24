//! Skinned rows for [`mesh_world_geometry_for_cull_with_head`].

use glam::{Mat4, Quat, Vec3};

use crate::scene::{
    RenderSpaceId, RenderSpaceRead, SceneSpaceRead, SceneTransformRead, SkinnedMeshRenderer,
};
use crate::shared::{LayerType, RenderBoundingBox, RenderSH2, RenderTransform, RenderingContext};
use crate::world_mesh::culling::{MeshCullTarget, mesh_world_geometry_for_cull_with_head};

use super::CULL_BATCH;

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
