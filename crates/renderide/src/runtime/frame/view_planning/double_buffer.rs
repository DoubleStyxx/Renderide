//! Double-buffered secondary-camera write policy.
//!
//! The second color image is allocated by the render-texture pool. This module decides whether a
//! camera may sample last frame's contents, whether that texture is ready to swap, and performs
//! the once-per-frame swap before view planning snapshots color handles.

use crate::backend::RenderBackend;
use crate::camera::{CameraRenderRect, camera_state_double_buffered};
use crate::gpu::GpuContext;
use crate::gpu_pools::RenderTexturePool;
use crate::gpu_pools::pools::render_texture::claim_double_buffer_swap;
use crate::graph_inputs::RenderTextureSelfSampling;
use crate::render_graph::OffscreenWriteTarget;
use crate::shared::{CameraState, RenderRect};

/// Write target for a secondary camera. Double buffering samples the previous color image.
///
/// Post-processing does not change the policy: the camera still writes the published image and
/// samples the previous one. The post chain uses transient textures, not the render texture.
pub(super) fn secondary_camera_write_target(rt_id: i32, flags: u16) -> OffscreenWriteTarget {
    if camera_state_double_buffered(flags) {
        OffscreenWriteTarget::host_render_texture_with_self_sampling(
            rt_id,
            RenderTextureSelfSampling::AllowPreviousContents,
        )
    } else {
        OffscreenWriteTarget::host_render_texture(rt_id)
    }
}

/// `true` when this render texture can accept a secondary camera, including a resolved viewport.
///
/// Double buffering swaps before the view snapshots color handles. Skip the swap when the camera
/// will not render, so a missing depth target or empty viewport does not publish an unwritten image.
pub(super) fn secondary_render_texture_ready(
    pool: &RenderTexturePool,
    rt_id: i32,
    viewport: RenderRect,
) -> bool {
    let Some(texture) = pool.get(rt_id) else {
        return false;
    };
    if texture.depth_texture.is_none() || texture.depth_view.is_none() {
        return false;
    }
    CameraRenderRect::resolve(viewport, (texture.width, texture.height)).is_some()
}

/// Swaps a ready double-buffered render texture once per planning pass.
///
/// The published color image becomes this frame's write target. The previous image keeps the
/// contents completed last frame. Textures that are not double-buffered, or that this camera will
/// not render, are left unchanged.
pub(super) fn swap_double_buffer(
    backend: &mut RenderBackend,
    gpu: &GpuContext,
    state: CameraState,
    claimed: &mut Vec<i32>,
) {
    let rt_id = state.render_texture_asset_id;
    if !camera_state_double_buffered(state.flags) {
        return;
    }
    if !secondary_render_texture_ready(backend.render_texture_pool(), rt_id, state.viewport) {
        return;
    }
    if !claim_double_buffer_swap(claimed, rt_id) {
        return;
    }
    let _allocated = backend.begin_double_buffered_render_texture(gpu.device().as_ref(), rt_id);
}
