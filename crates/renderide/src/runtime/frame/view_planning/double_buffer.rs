//! Double-buffered secondary-camera write policy.
//!
//! The second color image is allocated by the render-texture pool. This module decides whether a
//! camera may sample last frame's contents and swaps that texture only after the camera is known
//! to render, so a skipped camera does not publish an unwritten image.

use std::sync::Arc;

use glam::Mat4;

use crate::backend::RenderBackend;
use crate::camera::{
    CameraRenderRect, camera_state_double_buffered, camera_state_enabled,
    host_camera_frame_for_render_texture,
};
use crate::gpu_pools::RenderTexturePool;
use crate::gpu_pools::pools::render_texture::claim_double_buffer_swap;
use crate::graph_inputs::RenderTextureSelfSampling;
use crate::render_graph::{
    FrameViewClear, OffscreenWriteTarget, RenderPathProfile, ViewPostProcessing,
};
use crate::scene::RenderSpaceId;
use crate::shared::{CameraState, RenderRect};
use crate::world_mesh::{CameraTransformDrawFilter, draw_filter_from_camera_entry};

use super::super::super::RendererRuntime;
use super::super::view_plan::{
    FrameViewPlan, FrameViewPlanParams, FrameViewPlanTarget, OffscreenColorCopy,
    OffscreenTargetHandles,
};
use super::{
    ResidentSecondaryRenderTexture, camera_render_space_scope, secondary_camera_layer_policy,
    secondary_camera_render_context, secondary_camera_shadows_enabled, secondary_camera_view_id,
};

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

/// Records an enabled double-buffered camera even when that camera will not render this pass.
fn note_double_buffer_flag(flagged: &mut Vec<i32>, state: CameraState) {
    if camera_state_double_buffered(state.flags) {
        let _claimed = claim_double_buffer_swap(flagged, state.render_texture_asset_id);
    }
}

/// Swaps a ready double-buffered render texture once per planning pass.
///
/// Call only after the camera has a resident target, a resolved viewport, scratch for a partial
/// viewport, and a world matrix. Returns `true` when this camera claimed the swap. Later cameras
/// on the same texture get `false` and seed the published write image.
pub(super) fn swap_double_buffer(
    backend: &mut RenderBackend,
    device: &wgpu::Device,
    state: CameraState,
    claimed: &mut Vec<i32>,
) -> bool {
    let rt_id = state.render_texture_asset_id;
    if !camera_state_double_buffered(state.flags) {
        return false;
    }
    if !secondary_render_texture_ready(backend.render_texture_pool(), rt_id, state.viewport) {
        return false;
    }
    if !claim_double_buffer_swap(claimed, rt_id) {
        return false;
    }
    let _allocated = backend.begin_double_buffered_render_texture(device, rt_id);
    true
}

/// Replaces the snapshotted color handles with the image published after the swap.
///
/// History is attached only for the camera that claimed the swap. A later camera on the same
/// texture seeds the published write image, which already contains this frame's first writer.
fn refresh_published_color(
    pool: &RenderTexturePool,
    rt_id: i32,
    rt: &mut ResidentSecondaryRenderTexture,
    claimed_swap: bool,
) {
    let Some(texture) = pool.get(rt_id) else {
        return;
    };
    rt.color_texture = Arc::clone(&texture.color_texture);
    rt.color_view = Arc::clone(&texture.color_view);
    if claimed_swap {
        rt.history_color_texture = texture.history_color_texture();
        rt.seed_published_color = false;
    } else {
        rt.history_color_texture = None;
        rt.seed_published_color = true;
    }
}

/// Points every planned view of a swapped texture at the post-swap published color image.
///
/// Portals and mirrors are planned before secondary cameras, so their handles still name the
/// image that the swap turns into history. Rewriting those handles makes every writer of the
/// texture target the same published image. History attachments are left alone.
pub(super) fn refresh_swapped_view_color_handles(
    pool: &RenderTexturePool,
    swapped: &[i32],
    views: &mut [FrameViewPlan<'_>],
) {
    if swapped.is_empty() {
        return;
    }
    for view in views {
        let Some(asset_id) = view.write_target().host_render_texture_asset_id() else {
            continue;
        };
        if !swapped.contains(&asset_id) {
            continue;
        }
        let Some(texture) = pool.get(asset_id) else {
            continue;
        };
        view.replace_published_color(
            texture.color_texture.as_ref().clone(),
            texture.color_view.as_ref().clone(),
        );
    }
}

/// Owned inputs for one secondary camera that is going to render.
struct SecondaryCameraPlanInput {
    /// Copied host camera row.
    state: CameraState,
    /// Host renderable index used for the view id.
    renderable_index: i32,
    /// Render-context world matrix.
    world_m: Mat4,
    /// Resolved viewport extent in pixels.
    viewport_px: (u32, u32),
    /// Selective or exclude filter for the camera.
    filter: CameraTransformDrawFilter,
    /// `true` when the camera's render space is an overlay.
    overlay: bool,
    /// Post-swap color and depth handles.
    handles: OffscreenTargetHandles,
}

/// Builds one secondary render-texture view, or skips the camera.
///
/// Secondary cameras use the render-context world matrix, not the bare hierarchy matrix. For
/// overlay render spaces (userspace world: dash camera, interactive-camera mirrors, avatar
/// previews) that multiplies in `head_output_transform` so the camera follows the user's head,
/// matching how `world_mesh` draws in the same space are positioned. Without this the secondary
/// camera sits at userspace-local coords while its selective-render meshes track the user, so
/// any movement away from origin shifts the dash UI out of the camera's view.
///
/// The double-buffer swap runs only after every skip has been decided, and the color handles are
/// read again after the swap so the plan writes the published image.
pub(super) fn plan_secondary_rt_camera<'a>(
    runtime: &mut RendererRuntime,
    device: &wgpu::Device,
    sid: RenderSpaceId,
    cam_idx: usize,
    swapped: &mut Vec<i32>,
    flagged: &mut Vec<i32>,
) -> Option<FrameViewPlan<'a>> {
    let (state, transform_id, renderable_index, filter, overlay) = {
        let space = runtime.scene.space(sid)?;
        let entry = space.cameras().get(cam_idx)?;
        if !camera_state_enabled(entry.state.flags) {
            return None;
        }
        let filter = draw_filter_from_camera_entry(entry);
        (
            entry.state,
            entry.transform_id,
            entry.renderable_index,
            filter,
            space.is_overlay(),
        )
    };
    note_double_buffer_flag(flagged, state);
    let rt_id = state.render_texture_asset_id;
    let mut rt = runtime.resident_secondary_render_texture(rt_id, sid, cam_idx)?;
    let Some(render_rect) = CameraRenderRect::resolve(state.viewport, rt.extent_px) else {
        logger::trace!(
            "secondary camera: render texture asset {rt_id} viewport {:?} resolved empty; skipping",
            state.viewport
        );
        return None;
    };
    let scratch = if render_rect.is_full_target(rt.extent_px) {
        None
    } else {
        let Some(scratch) = runtime.backend.secondary_render_rect_scratch(
            device,
            render_rect.extent_px,
            rt.color_format,
            rt.depth_format,
        ) else {
            logger::trace!(
                "secondary camera: render texture asset {rt_id} viewport {:?} scratch unavailable; skipping",
                state.viewport
            );
            return None;
        };
        Some(scratch)
    };
    let world_m = runtime.scene.world_matrix_for_render_context(
        sid,
        transform_id as usize,
        secondary_camera_render_context(),
        runtime.host_camera.head_output_transform,
    )?;
    if camera_state_double_buffered(state.flags) {
        let claimed_swap = swap_double_buffer(&mut runtime.backend, device, state, swapped);
        refresh_published_color(
            runtime.backend.render_texture_pool(),
            rt_id,
            &mut rt,
            claimed_swap,
        );
    }
    let handles = assemble_secondary_rt_handles(
        secondary_camera_write_target(rt_id, state.flags),
        rt,
        render_rect,
        scratch,
    );
    Some(secondary_camera_frame_plan(
        runtime,
        sid,
        cam_idx,
        SecondaryCameraPlanInput {
            state,
            renderable_index,
            world_m,
            viewport_px: render_rect.extent_px,
            filter,
            overlay,
            handles,
        },
    ))
}

/// Builds graph handles for a resolved rect, allocating partial-viewport scratch when needed.
pub(super) fn secondary_rt_handles_for_rect(
    backend: &mut RenderBackend,
    device: &wgpu::Device,
    write_target: OffscreenWriteTarget,
    rt: ResidentSecondaryRenderTexture,
    render_rect: CameraRenderRect,
) -> Option<OffscreenTargetHandles> {
    let scratch = if render_rect.is_full_target(rt.extent_px) {
        None
    } else {
        Some(backend.secondary_render_rect_scratch(
            device,
            render_rect.extent_px,
            rt.color_format,
            rt.depth_format,
        )?)
    };
    Some(assemble_secondary_rt_handles(
        write_target,
        rt,
        render_rect,
        scratch,
    ))
}

/// Builds graph handles from a post-swap color snapshot and scratch acquired before the swap.
fn assemble_secondary_rt_handles(
    write_target: OffscreenWriteTarget,
    rt: ResidentSecondaryRenderTexture,
    render_rect: CameraRenderRect,
    scratch: Option<crate::backend::SecondaryRtScratchTargets>,
) -> OffscreenTargetHandles {
    let Some(scratch) = scratch else {
        return with_history_color(
            OffscreenTargetHandles::new(
                write_target,
                rt.color_texture.as_ref().clone(),
                rt.color_view.as_ref().clone(),
                rt.depth_texture.as_ref().clone(),
                rt.depth_view.as_ref().clone(),
                rt.extent_px,
                rt.color_format,
            ),
            &rt,
        );
    };
    with_history_color(
        OffscreenTargetHandles::new(
            write_target,
            scratch.color_texture.as_ref().clone(),
            scratch.color_view.as_ref().clone(),
            scratch.depth_texture.as_ref().clone(),
            scratch.depth_view.as_ref().clone(),
            render_rect.extent_px,
            rt.color_format,
        )
        .with_copy_to_color(OffscreenColorCopy {
            destination_texture: rt.color_texture.as_ref().clone(),
            destination_origin_px: render_rect.origin_px,
            extent_px: render_rect.extent_px,
        }),
        &rt,
    )
}

/// Attaches history for the swap claimer, or marks a later writer to seed the published image.
fn with_history_color(
    handles: OffscreenTargetHandles,
    rt: &ResidentSecondaryRenderTexture,
) -> OffscreenTargetHandles {
    let handles = match rt.history_color_texture.as_ref() {
        Some(texture) => handles.with_history_color(texture.as_ref().clone()),
        None => handles,
    };
    if rt.seed_published_color {
        handles.with_seed_published_color()
    } else {
        handles
    }
}

/// Finishes the [`FrameViewPlan`] for a camera that already has post-swap handles.
fn secondary_camera_frame_plan<'a>(
    runtime: &RendererRuntime,
    sid: RenderSpaceId,
    cam_idx: usize,
    input: SecondaryCameraPlanInput,
) -> FrameViewPlan<'a> {
    let has_selective_roots = input.filter.has_selective_roots();
    let mut host_camera = host_camera_frame_for_render_texture(
        &runtime.host_camera,
        &input.state,
        input.viewport_px,
        input.world_m,
    );
    // Selective secondary cameras (dashboards, in-world UI panels, mirrors on specific
    // subtrees) render tens of draws, not thousands. Hi-Z snapshots + occlusion temporal
    // cost a per-camera readback path with negligible payoff at that scale -- skip them.
    if has_selective_roots {
        host_camera.suppress_occlusion_temporal = true;
    }
    let post_processing = if input.overlay && has_selective_roots {
        ViewPostProcessing::disabled()
    } else {
        ViewPostProcessing::from_camera_state(&input.state)
    };
    let mut plan = FrameViewPlan::new(
        &host_camera,
        FrameViewPlanParams {
            render_context: secondary_camera_render_context(),
            frame_time_seconds: runtime.tick_state.frame_time_seconds(),
            view_id: secondary_camera_view_id(sid, input.renderable_index, cam_idx),
            viewport_px: input.viewport_px,
            clear: FrameViewClear::from_camera_state(&input.state),
            profile: RenderPathProfile::secondary_camera(post_processing),
            target: FrameViewPlanTarget::offscreen(input.handles),
        },
    );
    plan.draw_filter = Some(input.filter);
    plan.transform_filter_space = Some(sid);
    plan.render_space_scope = camera_render_space_scope(has_selective_roots, sid);
    plan.layer_policy = secondary_camera_layer_policy(input.state.flags);
    plan.render_shadows = secondary_camera_shadows_enabled(input.state.flags);
    plan
}
