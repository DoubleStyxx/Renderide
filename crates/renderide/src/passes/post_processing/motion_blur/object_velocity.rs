//! Object-velocity raster pass.
//!
//! Runs after the camera velocity pass and loads that `Rg16Float` target. Rigid draws with a
//! previous model matrix are re-rasterized with a depth-equal test so only their pixels are
//! overwritten. Every other pixel keeps the camera reprojection.
//!
//! Previous matrices advance once per host submit. A second render of the same view in that
//! submit emits no object velocity. TAA (#170) must keep both view-projections unjittered.
//! Near-plane crossings (`w <= 0`) discard in the shader and keep camera velocity; the blur
//! radius clamp bounds any streak that still gets through.

use std::num::NonZeroU32;

use crate::camera::ViewId;
use crate::passes::helpers::color_attachment;
use crate::passes::{
    ObjectMotionDraw, WorldMeshForwardEncodeRefs, WorldMeshForwardPlanSlot,
    record_object_motion_vectors,
};
use crate::render_graph::context::RasterPassCtx;
use crate::render_graph::error::{RenderPassError, SetupError};
use crate::render_graph::gpu_cache::raster_stereo_mask_override;
use crate::render_graph::pass::{PassBuilder, PassMergeHint, RasterPass, RenderPassTemplate};
use crate::render_graph::resources::{
    BufferAccess, ImportedBufferHandle, ImportedTextureHandle, StorageAccess, TextureHandle,
};

use super::{MotionBlurStateCache, motion_blur_settings, view_motion_blur_active};

/// Returns whether this view should write object velocity for `rendered_frame`.
///
/// Previous matrices are published once per host submit. The first render of a view for that
/// frame emits velocity. A repeat render of the same view keeps the camera vector, which is
/// already zero when the camera did not move between the two renders.
pub(super) fn take_object_velocity_emission(
    published_frame: Option<i32>,
    rendered_frame: i32,
    last_emitted_frame: &mut Option<i32>,
) -> bool {
    if published_frame != Some(rendered_frame) {
        return false;
    }
    if *last_emitted_frame == Some(rendered_frame) {
        return false;
    }
    *last_emitted_frame = Some(rendered_frame);
    true
}

/// Previous view-projection pair uploaded once per recorded object-velocity pass.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PreviousViewProjGpu {
    /// Left-eye or mono view-projection from the previous frame.
    left: [[f32; 4]; 4],
    /// Right-eye view-projection, or a copy of left for mono.
    right: [[f32; 4]; 4],
}

/// Depth-equal pass that composites rigid object velocity into the camera velocity target.
pub(super) struct ObjectMotionVectorsPass {
    /// Imported single-sample frame depth used as an equality mask.
    depth: ImportedTextureHandle,
    /// Camera velocity target. Loaded, not cleared.
    velocity: TextureHandle,
    /// Imported forward per-draw slab.
    per_draw_slab: ImportedBufferHandle,
    /// Per-view camera history, including the stashed previous view-projection.
    state_cache: std::sync::Arc<MotionBlurStateCache>,
}

impl ObjectMotionVectorsPass {
    pub(super) fn new(
        depth: ImportedTextureHandle,
        velocity: TextureHandle,
        per_draw_slab: ImportedBufferHandle,
        state_cache: std::sync::Arc<MotionBlurStateCache>,
    ) -> Self {
        Self {
            depth,
            velocity,
            per_draw_slab,
            state_cache,
        }
    }
}

impl RasterPass for ObjectMotionVectorsPass {
    fn name(&self) -> &str {
        "ObjectMotionVectors"
    }

    fn setup(&mut self, b: &mut PassBuilder<'_>) -> Result<(), SetupError> {
        b.read_blackboard::<crate::passes::post_processing::settings_slots::MotionBlurSettingsSlot>();
        b.read_optional_blackboard::<WorldMeshForwardPlanSlot>();
        b.merge_hint(PassMergeHint {
            attachment_reuse: true,
            tile_memory_preferred: true,
        });
        color_attachment(b, self.velocity, wgpu::LoadOp::Load);
        {
            let mut raster = b.raster();
            raster.depth(
                self.depth,
                wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                None,
            );
        }
        b.import_buffer(
            self.per_draw_slab,
            BufferAccess::Storage {
                stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                access: StorageAccess::ReadOnly,
            },
        );
        Ok(())
    }

    fn multiview_mask_override(
        &self,
        ctx: &RasterPassCtx<'_, '_>,
        template: &RenderPassTemplate,
    ) -> Option<NonZeroU32> {
        raster_stereo_mask_override(ctx, template)
    }

    fn should_record(&self, ctx: &RasterPassCtx<'_, '_>) -> Result<bool, RenderPassError> {
        let settings = motion_blur_settings(ctx.blackboard);
        if !view_motion_blur_active(ctx.frame.view, settings)
            || ctx.frame.view.depth_sample_view.is_none()
        {
            return Ok(false);
        }
        Ok(ctx
            .blackboard
            .get::<WorldMeshForwardPlanSlot>()
            .is_some_and(|prepared| prepared.has_rigid_object_motion))
    }

    fn release_view_resources(&mut self, retired_views: &[ViewId]) {
        self.state_cache.retire_views(retired_views);
    }

    fn record(
        &self,
        ctx: &mut RasterPassCtx<'_, '_>,
        rpass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), RenderPassError> {
        profiling::scope!("post_processing::object_motion_vectors");
        let frame = &ctx.frame;
        let Some(prepared) = ctx.blackboard.get::<WorldMeshForwardPlanSlot>() else {
            return Ok(());
        };
        if !prepared.has_rigid_object_motion {
            return Ok(());
        }
        let Some(per_draw_bind_group) = frame
            .systems
            .frame_resources
            .per_view_per_draw_bind_group(frame.view.view_id)
        else {
            return Ok(());
        };
        let Some(gpu_limits) = frame.view.gpu_limits.clone() else {
            return Ok(());
        };
        let state = self.state_cache.ensure(ctx.device, frame.view.view_id);
        let Some((left, right)) = *state.object_motion_previous_vp.lock() else {
            return Ok(());
        };
        let published = frame.systems.scene.object_motion_published_frame();
        let mut emitted = state.object_velocity_emitted_frame.lock();
        if !take_object_velocity_emission(
            published,
            frame.view.host_camera.frame_index,
            &mut emitted,
        ) {
            return Ok(());
        }
        drop(emitted);
        let params = PreviousViewProjGpu {
            left: left.to_cols_array_2d(),
            right: right.to_cols_array_2d(),
        };
        ctx.write_buffer(&state.previous_vp_buffer, 0, bytemuck::bytes_of(&params));
        let encode = WorldMeshForwardEncodeRefs::from_pass_frame(frame);
        record_object_motion_vectors(ObjectMotionDraw {
            rpass,
            device: ctx.device,
            prepared,
            encode: &encode,
            gpu_limits: gpu_limits.as_ref(),
            per_draw_bind_group: per_draw_bind_group.as_ref(),
            previous_vp_bind_group: &state.previous_vp_bind_group,
            depth_format: frame.view.depth_texture.format(),
            multiview_stereo: frame.view.multiview_stereo,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::take_object_velocity_emission;

    #[test]
    fn repeat_render_of_one_submit_emits_object_velocity_once() {
        let mut emitted = None;
        assert!(take_object_velocity_emission(Some(7), 7, &mut emitted));
        assert!(!take_object_velocity_emission(Some(7), 7, &mut emitted));
        assert!(take_object_velocity_emission(Some(8), 8, &mut emitted));
        assert!(!take_object_velocity_emission(None, 8, &mut emitted));
        assert_eq!(emitted, Some(8));
    }
}
