//! Per-tick view collection on [`super::RendererRuntime`].
//!
//! Builds the ordered list of [`FrameViewPlan`]s that drive draw collection and graph
//! execution: camera portals and secondary render-texture cameras first, then the primary HMD or
//! desktop view when included. Logic sits between the render entry point in [`super::render`] and
//! the per-view extraction pipeline in [`super::extract`].

mod double_buffer;
mod portal_plan;

use std::sync::{Arc, LazyLock};

use crate::camera::{
    CameraPortalMode, CameraPortalSourceView, CameraPortalSurface, EyeView, ViewId, Viewport,
    WorldProjectionSet, camera_state_enabled, camera_state_render_private_ui,
    camera_state_render_shadows, host_camera_frame_for_camera_portal,
    view_matrix_for_world_mesh_render_space,
};
use crate::diagnostics::log_once::KeyedLogOnce;
use crate::gpu::GpuContext;
use crate::render_graph::{
    FrameGlobalView, FrameViewClear, OffscreenWriteTarget, RenderPathProfile, ViewPostProcessing,
    ViewWinding,
};
use crate::scene::{
    RenderSpaceId, SceneCoordinator, camera_portal_disable_per_pixel_lights,
    camera_portal_disable_shadows, camera_portal_has_camera_clear_mode,
    camera_portal_has_far_clip_value, camera_portal_portal_mode,
};
use crate::shared::{CameraClearMode, RenderingContext};
use crate::world_mesh::{ViewLayerPolicy, ViewRenderSpaceScope};

use super::super::RendererRuntime;
use super::render::PrimaryViewRequest;
use super::view_plan::{
    FrameViewPlan, FrameViewPlanParams, FrameViewPlanTarget, OffscreenTargetHandles, ViewFamilyPlan,
};
use double_buffer::{plan_secondary_rt_camera, secondary_rt_handles_for_rect};
#[cfg(test)]
use portal_plan::camera_portal_stereo_render_rects;
use portal_plan::{CameraPortalSourceViewPlan, CameraPortalSourceViewPlans, CameraPortalViewTask};

/// Once-only diagnostic gate for secondary render textures without a depth texture.
static SECONDARY_RT_MISSING_DEPTH_LOG: LazyLock<KeyedLogOnce<i32>> =
    LazyLock::new(KeyedLogOnce::new);

/// Once-only diagnostic gate for secondary render textures without a depth view.
static SECONDARY_RT_MISSING_DEPTH_VIEW_LOG: LazyLock<KeyedLogOnce<i32>> =
    LazyLock::new(KeyedLogOnce::new);

/// Once-only diagnostic gate for camera portals without a depth texture.
static CAMERA_PORTAL_RT_MISSING_DEPTH_LOG: LazyLock<KeyedLogOnce<i32>> =
    LazyLock::new(KeyedLogOnce::new);

/// Once-only diagnostic gate for camera portals without a depth view.
static CAMERA_PORTAL_RT_MISSING_DEPTH_VIEW_LOG: LazyLock<KeyedLogOnce<i32>> =
    LazyLock::new(KeyedLogOnce::new);

/// Once-only diagnostic gate for camera portal per-pixel-light disables.
static CAMERA_PORTAL_DISABLE_PIXEL_LIGHTS_LOG: LazyLock<KeyedLogOnce<i32>> =
    LazyLock::new(KeyedLogOnce::new);

/// Returns the stable logical identity for one secondary camera view.
pub(in crate::runtime) fn secondary_camera_view_id(
    render_space_id: RenderSpaceId,
    renderable_index: i32,
    camera_index: usize,
) -> ViewId {
    ViewId::secondary_camera(
        render_space_id,
        if renderable_index >= 0 {
            renderable_index
        } else {
            camera_index as i32
        },
    )
}

/// Returns the stable logical identity for one camera-portal view.
pub(in crate::runtime) fn camera_portal_view_id(
    render_space_id: RenderSpaceId,
    renderable_index: i32,
    portal_index: usize,
) -> ViewId {
    ViewId::camera_portal(
        render_space_id,
        camera_portal_identity_index(renderable_index, portal_index),
    )
}

fn camera_portal_eye_view_id(
    render_space_id: RenderSpaceId,
    renderable_index: i32,
    portal_index: usize,
    eye_index: u8,
) -> ViewId {
    ViewId::camera_portal_eye(
        render_space_id,
        camera_portal_identity_index(renderable_index, portal_index),
        eye_index,
    )
}

fn camera_portal_identity_index(renderable_index: i32, portal_index: usize) -> i32 {
    if renderable_index >= 0 {
        renderable_index
    } else {
        portal_index as i32
    }
}

fn sort_secondary_view_tasks(tasks: &mut [(RenderSpaceId, f32, usize)]) {
    tasks.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.2.cmp(&b.2))
    });
}

fn sort_camera_portal_view_tasks(tasks: &mut [(RenderSpaceId, usize)]) {
    tasks.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
}

fn collect_camera_portal_view_tasks(
    scene: &SceneCoordinator,
    tasks: &mut Vec<(RenderSpaceId, usize)>,
) {
    for sid in scene.render_space_ids() {
        let Some(space) = scene.space(sid) else {
            continue;
        };
        if !space.is_active() {
            continue;
        }
        for (idx, portal) in space.camera_portals().iter().enumerate() {
            if portal.state.render_texture_id >= 0 {
                tasks.push((sid, idx));
            }
        }
    }
    sort_camera_portal_view_tasks(tasks);
}

fn collect_secondary_rt_view_tasks(
    scene: &SceneCoordinator,
    tasks: &mut Vec<(RenderSpaceId, f32, usize)>,
) {
    for sid in scene.render_space_ids() {
        let Some(space) = scene.space(sid) else {
            continue;
        };
        if !space.is_active() {
            continue;
        }
        for (idx, cam) in space.cameras().iter().enumerate() {
            if !camera_state_enabled(cam.state.flags) {
                continue;
            }
            if cam.state.render_texture_asset_id < 0 {
                continue;
            }
            tasks.push((sid, cam.state.depth, idx));
        }
    }
    sort_secondary_view_tasks(tasks);
}

fn secondary_camera_render_context() -> RenderingContext {
    RenderingContext::Camera
}

fn camera_portal_render_context(mode: CameraPortalMode) -> RenderingContext {
    match mode {
        CameraPortalMode::Mirror => RenderingContext::Mirror,
        CameraPortalMode::Portal => RenderingContext::Portal,
    }
}

fn camera_portal_render_space_scope() -> ViewRenderSpaceScope {
    ViewRenderSpaceScope::AllActive
}

fn camera_portal_layer_policy() -> ViewLayerPolicy {
    ViewLayerPolicy::camera(false)
}

fn secondary_camera_layer_policy(flags: u16) -> ViewLayerPolicy {
    ViewLayerPolicy::camera(camera_state_render_private_ui(flags))
}

/// Returns the render-space scope used by a camera-like view with optional selective roots.
pub(in crate::runtime) fn camera_render_space_scope(
    has_selective_roots: bool,
    render_space_id: RenderSpaceId,
) -> ViewRenderSpaceScope {
    if has_selective_roots {
        ViewRenderSpaceScope::single(render_space_id)
    } else {
        ViewRenderSpaceScope::AllActive
    }
}

fn secondary_camera_shadows_enabled(flags: u16) -> bool {
    camera_state_render_shadows(flags)
}

fn camera_portal_write_target(rt_id: i32) -> OffscreenWriteTarget {
    OffscreenWriteTarget::host_render_texture(rt_id)
}

fn camera_portal_clear(flags: i32, mode: CameraClearMode) -> FrameViewClear {
    if camera_portal_has_camera_clear_mode(flags) {
        FrameViewClear {
            mode,
            color: glam::Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
    } else {
        FrameViewClear::skybox()
    }
}

/// Logs a missing secondary render-texture depth attachment once per render texture id.
fn log_secondary_rt_missing_depth(rt_id: i32, sid: RenderSpaceId, cam_idx: usize) {
    if SECONDARY_RT_MISSING_DEPTH_LOG.should_log(rt_id) {
        logger::warn!(
            "secondary camera: render texture {rt_id} missing depth; space={sid:?} camera_index={cam_idx}"
        );
    }
}

/// Logs a missing secondary render-texture depth view once per render texture id.
fn log_secondary_rt_missing_depth_view(rt_id: i32, sid: RenderSpaceId, cam_idx: usize) {
    if SECONDARY_RT_MISSING_DEPTH_VIEW_LOG.should_log(rt_id) {
        logger::warn!(
            "secondary camera: render texture {rt_id} missing depth view; space={sid:?} camera_index={cam_idx}"
        );
    }
}

/// Logs a missing camera-portal render-texture depth attachment once per render texture id.
fn log_camera_portal_rt_missing_depth(rt_id: i32, sid: RenderSpaceId, portal_idx: usize) {
    if CAMERA_PORTAL_RT_MISSING_DEPTH_LOG.should_log(rt_id) {
        logger::warn!(
            "camera portal: render texture {rt_id} missing depth; space={sid:?} portal_index={portal_idx}"
        );
    }
}

/// Logs a missing camera-portal render-texture depth view once per render texture id.
fn log_camera_portal_rt_missing_depth_view(rt_id: i32, sid: RenderSpaceId, portal_idx: usize) {
    if CAMERA_PORTAL_RT_MISSING_DEPTH_VIEW_LOG.should_log(rt_id) {
        logger::warn!(
            "camera portal: render texture {rt_id} missing depth view; space={sid:?} portal_index={portal_idx}"
        );
    }
}

fn log_camera_portal_disable_per_pixel_lights(rt_id: i32) {
    if CAMERA_PORTAL_DISABLE_PIXEL_LIGHTS_LOG.should_log(rt_id) {
        logger::debug!(
            "camera portal: disablePerPixelLights requested for render texture {rt_id}; clustered lights do not expose a per-pixel light-count switch"
        );
    }
}

/// Resident host render texture handles needed to plan one secondary camera view.
#[derive(Clone)]
struct ResidentSecondaryRenderTexture {
    /// Host render texture color storage.
    color_texture: Arc<wgpu::Texture>,
    /// Previous-frame color image after a double-buffer swap. Absent for every other camera.
    history_color_texture: Option<Arc<wgpu::Texture>>,
    /// Full host render texture color view.
    color_view: Arc<wgpu::TextureView>,
    /// Full host render texture depth storage.
    depth_texture: Arc<wgpu::Texture>,
    /// Full host render texture depth view.
    depth_view: Arc<wgpu::TextureView>,
    /// Full host render texture extent in pixels.
    extent_px: (u32, u32),
    /// Host render texture color format.
    color_format: wgpu::TextureFormat,
    /// Host render texture depth format.
    depth_format: wgpu::TextureFormat,
}

impl RendererRuntime {
    /// Snapshots the GPU handles for a resident secondary render texture.
    fn resident_secondary_render_texture(
        &self,
        rt_id: i32,
        sid: RenderSpaceId,
        cam_idx: usize,
    ) -> Option<ResidentSecondaryRenderTexture> {
        let Some(rt) = self.backend.render_texture_pool().get(rt_id) else {
            logger::trace!("secondary camera: render texture asset {rt_id} not resident; skipping");
            return None;
        };
        let Some(depth_texture) = rt.depth_texture.clone() else {
            log_secondary_rt_missing_depth(rt_id, sid, cam_idx);
            return None;
        };
        let Some(depth_view) = rt.depth_view.clone() else {
            log_secondary_rt_missing_depth_view(rt_id, sid, cam_idx);
            return None;
        };
        Some(ResidentSecondaryRenderTexture {
            color_texture: rt.color_texture.clone(),
            history_color_texture: None,
            color_view: rt.color_view.clone(),
            depth_format: depth_texture.format(),
            depth_texture,
            depth_view,
            extent_px: (rt.width, rt.height),
            color_format: rt.wgpu_color_format,
        })
    }

    /// Snapshots the GPU handles for a resident camera-portal render texture.
    fn resident_camera_portal_render_texture(
        &self,
        rt_id: i32,
        sid: RenderSpaceId,
        portal_idx: usize,
    ) -> Option<ResidentSecondaryRenderTexture> {
        let Some(rt) = self.backend.render_texture_pool().get(rt_id) else {
            logger::trace!("camera portal: render texture asset {rt_id} not resident; skipping");
            return None;
        };
        let Some(depth_texture) = rt.depth_texture.clone() else {
            log_camera_portal_rt_missing_depth(rt_id, sid, portal_idx);
            return None;
        };
        let Some(depth_view) = rt.depth_view.clone() else {
            log_camera_portal_rt_missing_depth_view(rt_id, sid, portal_idx);
            return None;
        };
        Some(ResidentSecondaryRenderTexture {
            color_texture: rt.color_texture.clone(),
            history_color_texture: None,
            color_view: rt.color_view.clone(),
            depth_format: depth_texture.format(),
            depth_texture,
            depth_view,
            extent_px: (rt.width, rt.height),
            color_format: rt.wgpu_color_format,
        })
    }

    /// Collects every active view for this tick into a single ordered list.
    ///
    /// Offscreen producer views are submitted before the primary HMD or desktop consumer so
    /// in-world materials sample current-frame render textures.
    pub(in crate::runtime) fn collect_prepared_views<'a>(
        &mut self,
        gpu: &GpuContext,
        primary: PrimaryViewRequest<'a>,
        main_extent_px: (u32, u32),
        main_profile: RenderPathProfile,
        fallback_frame_global_profile: RenderPathProfile,
        main_offscreen_target: Option<OffscreenTargetHandles>,
    ) -> ViewFamilyPlan<'a> {
        let secondary_views = self.collect_offscreen_scene_views(gpu, main_extent_px);
        self.assemble_prepared_views(
            primary,
            main_extent_px,
            main_profile,
            fallback_frame_global_profile,
            main_offscreen_target,
            secondary_views,
        )
    }

    /// Collects active views without GPU-backed secondary render targets for CPU-only tests.
    #[cfg(test)]
    pub(in crate::runtime) fn collect_prepared_views_without_secondaries<'a>(
        &self,
        primary: PrimaryViewRequest<'a>,
        main_extent_px: (u32, u32),
        main_profile: RenderPathProfile,
        fallback_frame_global_profile: RenderPathProfile,
    ) -> ViewFamilyPlan<'a> {
        self.assemble_prepared_views(
            primary,
            main_extent_px,
            main_profile,
            fallback_frame_global_profile,
            None,
            Vec::new(),
        )
    }

    /// Appends pre-collected offscreen views before the primary HMD or desktop view.
    fn assemble_prepared_views<'a>(
        &self,
        primary: PrimaryViewRequest<'a>,
        main_extent_px: (u32, u32),
        main_profile: RenderPathProfile,
        fallback_frame_global_profile: RenderPathProfile,
        main_offscreen_target: Option<OffscreenTargetHandles>,
        secondary_views: Vec<FrameViewPlan<'a>>,
    ) -> ViewFamilyPlan<'a> {
        let (includes_main, hmd_target) = match primary {
            PrimaryViewRequest::DesktopMain => (true, None),
            PrimaryViewRequest::HmdExternalMultiview(ext) => (false, Some(ext)),
            PrimaryViewRequest::None => (false, None),
        };

        let main_render_context = self.scene.active_main_render_context();
        let fallback_frame_global =
            self.frame_global_from_runtime(main_render_context, fallback_frame_global_profile);

        let hmd_view = hmd_target.map(|ext| {
            let extent_px = ext.extent_px;
            FrameViewPlan::new(
                &self.host_camera,
                FrameViewPlanParams {
                    render_context: main_render_context,
                    frame_time_seconds: self.tick_state.frame_time_seconds(),
                    view_id: ViewId::Main,
                    viewport_px: extent_px,
                    clear: FrameViewClear::skybox(),
                    profile: RenderPathProfile::xr_hmd(),
                    target: FrameViewPlanTarget::ExternalMultiview(ext),
                },
            )
        });
        let main_view = includes_main.then(|| {
            self.build_main_view_with_profile(main_extent_px, main_profile, main_offscreen_target)
        });

        assemble_view_family_plan(&fallback_frame_global, secondary_views, hmd_view, main_view)
    }

    /// Builds fallback primary-view metadata for frame-global passes when no HMD or main view is
    /// submitted.
    fn frame_global_from_runtime(
        &self,
        render_context: RenderingContext,
        profile: RenderPathProfile,
    ) -> FrameGlobalView {
        FrameGlobalView::new(
            &self.host_camera,
            render_context,
            self.tick_state.frame_time_seconds(),
            FrameViewClear::skybox(),
            profile.post_processing(),
        )
    }

    /// Builds prepared views for every enabled secondary render-texture camera in the scene,
    /// skipping cameras whose host render texture is not yet resident on the GPU.
    ///
    /// Reuses [`RendererRuntime::secondary_view_tasks_scratch`] for the depth-sort scratch buffer
    /// so a frame with secondary cameras does not allocate a fresh `Vec` for the sort each tick.
    fn collect_secondary_rt_views<'a>(&mut self, gpu: &GpuContext) -> Vec<FrameViewPlan<'a>> {
        let mut tasks = std::mem::take(&mut self.tick_state.secondary_view_tasks_scratch);
        let mut swapped =
            std::mem::take(&mut self.tick_state.double_buffered_render_texture_scratch);
        let mut flagged = std::mem::take(&mut self.tick_state.double_buffer_flag_scratch);
        tasks.clear();
        swapped.clear();
        flagged.clear();
        let result = self.collect_secondary_rt_views_using(
            gpu.device().as_ref(),
            &mut tasks,
            &mut swapped,
            &mut flagged,
        );
        self.tick_state.secondary_view_tasks_scratch = tasks;
        self.tick_state.double_buffered_render_texture_scratch = swapped;
        self.tick_state.double_buffer_flag_scratch = flagged;
        result
    }

    /// Collects camera portals followed by secondary cameras into the offscreen view prefix.
    fn collect_offscreen_scene_views<'a>(
        &mut self,
        gpu: &GpuContext,
        main_extent_px: (u32, u32),
    ) -> Vec<FrameViewPlan<'a>> {
        let mut portal_views = self.collect_camera_portal_views(gpu, main_extent_px);
        let mut secondary_views = self.collect_secondary_rt_views(gpu);
        portal_views.append(&mut secondary_views);
        portal_views
    }

    /// Collects active camera portals that render into resident host render textures.
    fn collect_camera_portal_views<'a>(
        &mut self,
        gpu: &GpuContext,
        main_extent_px: (u32, u32),
    ) -> Vec<FrameViewPlan<'a>> {
        let mut tasks = std::mem::take(&mut self.tick_state.camera_portal_view_tasks_scratch);
        tasks.clear();
        let result = self.collect_camera_portal_views_using(gpu, main_extent_px, &mut tasks);
        self.tick_state.camera_portal_view_tasks_scratch = tasks;
        result
    }

    /// Inner helper that consumes the supplied camera-portal scratch buffer.
    fn collect_camera_portal_views_using<'a>(
        &mut self,
        gpu: &GpuContext,
        main_extent_px: (u32, u32),
        tasks: &mut Vec<(RenderSpaceId, usize)>,
    ) -> Vec<FrameViewPlan<'a>> {
        collect_camera_portal_view_tasks(&self.scene, tasks);

        let mut views: Vec<FrameViewPlan<'a>> = Vec::with_capacity(tasks.len().saturating_mul(2));
        for (sid, portal_idx) in tasks.drain(..) {
            self.append_camera_portal_views_for_task(
                gpu,
                main_extent_px,
                sid,
                portal_idx,
                &mut views,
            );
        }
        views
    }

    fn append_camera_portal_views_for_task<'a>(
        &mut self,
        gpu: &GpuContext,
        main_extent_px: (u32, u32),
        sid: RenderSpaceId,
        portal_idx: usize,
        views: &mut Vec<FrameViewPlan<'a>>,
    ) {
        let Some(task) = self.camera_portal_view_task(sid, portal_idx) else {
            return;
        };
        let Some(rt) = self.resident_camera_portal_render_texture(
            task.render_texture_id,
            task.render_space_id,
            task.portal_index,
        ) else {
            return;
        };
        let Some(source_plans) = self.camera_portal_source_view_plans(main_extent_px, rt.extent_px)
        else {
            logger::trace!(
                "camera portal: render texture {} has zero extent; space={:?} portal_index={}",
                task.render_texture_id,
                task.render_space_id,
                task.portal_index
            );
            return;
        };
        if camera_portal_disable_per_pixel_lights(task.state.flags) {
            log_camera_portal_disable_per_pixel_lights(task.render_texture_id);
        }
        for source_plan in source_plans.iter() {
            if let Some(plan) =
                self.camera_portal_frame_view_plan(gpu, task, rt.clone(), source_plan)
            {
                views.push(plan);
            }
        }
    }

    fn camera_portal_view_task(
        &self,
        sid: RenderSpaceId,
        portal_idx: usize,
    ) -> Option<CameraPortalViewTask> {
        let space = self.scene.space(sid)?;
        let entry = space.camera_portals().get(portal_idx)?;
        let state = entry.state;
        let mode = if camera_portal_portal_mode(state.flags) {
            CameraPortalMode::Portal
        } else {
            CameraPortalMode::Mirror
        };
        let render_context = camera_portal_render_context(mode);
        let Some(surface_world_matrix) =
            self.camera_portal_surface_world_matrix(sid, state.mesh_renderer_index, render_context)
        else {
            logger::trace!(
                "camera portal: invalid mesh renderer index {}; space={sid:?} portal_index={portal_idx}",
                state.mesh_renderer_index
            );
            return None;
        };
        Some(CameraPortalViewTask {
            render_space_id: sid,
            portal_index: portal_idx,
            state,
            renderable_index: entry.renderable_index,
            render_texture_id: state.render_texture_id,
            mode,
            render_context,
            surface_world_matrix,
        })
    }

    fn camera_portal_frame_view_plan<'a>(
        &mut self,
        gpu: &GpuContext,
        task: CameraPortalViewTask,
        rt: ResidentSecondaryRenderTexture,
        source_plan: CameraPortalSourceViewPlan,
    ) -> Option<FrameViewPlan<'a>> {
        let Some(hc) = host_camera_frame_for_camera_portal(
            &self.host_camera,
            &task.state,
            source_plan.source,
            CameraPortalSurface::new(task.surface_world_matrix),
            task.mode,
            camera_portal_has_far_clip_value(task.state.flags),
        ) else {
            logger::trace!(
                "camera portal: invalid camera matrices; render texture {} space={:?} portal_index={} eye_index={}",
                task.render_texture_id,
                task.render_space_id,
                task.portal_index,
                source_plan.eye_index
            );
            return None;
        };
        let Some(target_handles) = secondary_rt_handles_for_rect(
            &mut self.backend,
            gpu.device().as_ref(),
            camera_portal_write_target(task.render_texture_id),
            rt,
            source_plan.render_rect,
        ) else {
            logger::trace!(
                "camera portal: render texture {} could not allocate split target; space={:?} portal_index={} eye_index={}",
                task.render_texture_id,
                task.render_space_id,
                task.portal_index,
                source_plan.eye_index
            );
            return None;
        };
        let view_id = if source_plan.eye_index == 0 {
            camera_portal_view_id(
                task.render_space_id,
                task.renderable_index,
                task.portal_index,
            )
        } else {
            camera_portal_eye_view_id(
                task.render_space_id,
                task.renderable_index,
                task.portal_index,
                source_plan.eye_index,
            )
        };
        let mut plan = FrameViewPlan::new(
            &hc,
            FrameViewPlanParams {
                render_context: task.render_context,
                frame_time_seconds: self.tick_state.frame_time_seconds(),
                view_id,
                viewport_px: source_plan.render_rect.extent_px,
                clear: camera_portal_clear(task.state.flags, task.state.override_clear_flag_value),
                profile: RenderPathProfile::secondary_camera(ViewPostProcessing::disabled()),
                target: FrameViewPlanTarget::offscreen(target_handles),
            },
        );
        if task.mode == CameraPortalMode::Mirror {
            plan.view_winding = ViewWinding::mirror_reflection();
        }
        plan.render_space_scope = camera_portal_render_space_scope();
        plan.layer_policy = camera_portal_layer_policy();
        plan.render_shadows = !camera_portal_disable_shadows(task.state.flags);
        Some(plan)
    }

    fn camera_portal_source_view_plans(
        &self,
        main_extent_px: (u32, u32),
        target_extent_px: (u32, u32),
    ) -> Option<CameraPortalSourceViewPlans> {
        if let Some(stereo) = self.host_camera.active_stereo() {
            let left = CameraPortalSourceView::new(
                stereo.left,
                self.host_camera.clip,
                self.host_camera.projection_kind,
            );
            let right = CameraPortalSourceView::new(
                stereo.right,
                self.host_camera.clip,
                self.host_camera.projection_kind,
            );
            if let Some(plans) = CameraPortalSourceViewPlans::stereo(left, right, target_extent_px)
            {
                return Some(plans);
            }
        }
        CameraPortalSourceViewPlans::mono(
            self.camera_portal_mono_source_view(main_extent_px),
            target_extent_px,
        )
    }

    fn camera_portal_mono_source_view(&self, main_extent_px: (u32, u32)) -> CameraPortalSourceView {
        let projections =
            WorldProjectionSet::from_scene_host(&self.scene, main_extent_px, &self.host_camera);
        if let Some(explicit) = self.host_camera.explicit_view {
            return CameraPortalSourceView::new(
                explicit,
                projections.clip,
                self.host_camera.projection_kind,
            );
        }
        let view = self
            .scene
            .active_main_space()
            .map_or(glam::Mat4::IDENTITY, |space| {
                view_matrix_for_world_mesh_render_space(&self.scene, space)
            });
        let world_position = self.host_camera.view_origin_world();
        let eye = EyeView::new(
            view,
            projections.world_proj,
            projections.world_proj * view,
            world_position,
        );
        CameraPortalSourceView::symmetric_perspective(
            eye,
            projections.clip,
            Viewport::from_tuple(main_extent_px),
            self.host_camera.desktop_fov_degrees,
        )
    }

    fn camera_portal_surface_world_matrix(
        &self,
        sid: RenderSpaceId,
        mesh_renderer_index: i32,
        render_context: RenderingContext,
    ) -> Option<glam::Mat4> {
        if mesh_renderer_index < 0 {
            return None;
        }
        let space = self.scene.space(sid)?;
        let renderer = space
            .static_mesh_renderers()
            .get(mesh_renderer_index as usize)?;
        if renderer.node_id < 0 {
            return None;
        }
        self.scene.world_matrix_for_render_context(
            sid,
            renderer.node_id as usize,
            render_context,
            self.host_camera.head_output_transform,
        )
    }

    /// Inner helper that consumes the supplied scratch buffers.
    ///
    /// `swapped` records textures already exchanged this pass. `flagged` records textures whose
    /// enabled cameras still have the double-buffer flag, including cameras that do not render.
    fn collect_secondary_rt_views_using<'a>(
        &mut self,
        device: &wgpu::Device,
        tasks: &mut Vec<(RenderSpaceId, f32, usize)>,
        swapped: &mut Vec<i32>,
        flagged: &mut Vec<i32>,
    ) -> Vec<FrameViewPlan<'a>> {
        collect_secondary_rt_view_tasks(&self.scene, tasks);

        let mut views: Vec<FrameViewPlan<'a>> = Vec::with_capacity(tasks.len());
        for (sid, _, cam_idx) in tasks.drain(..) {
            if let Some(plan) =
                plan_secondary_rt_camera(self, device, sid, cam_idx, swapped, flagged)
            {
                views.push(plan);
            }
        }
        self.backend.retire_idle_double_buffer_histories(flagged);
        views
    }

    /// Builds the main desktop/headless [`FrameViewPlan`] from the cached
    /// [`RendererRuntime::host_camera`].
    ///
    /// `main_extent_px` must match the current main-view target extent: it feeds
    /// [`crate::world_mesh::build_world_mesh_cull_proj_params`] on the pre-dispatch CPU cull
    /// path. A stale or zero extent produces a degenerate frustum and random scene-object
    /// culling.
    #[cfg(test)]
    pub(in crate::runtime) fn build_main_desktop_view<'a>(
        &self,
        main_extent_px: (u32, u32),
    ) -> FrameViewPlan<'a> {
        self.build_main_view_with_profile(main_extent_px, RenderPathProfile::desktop_main(), None)
    }

    fn build_main_view_with_profile<'a>(
        &self,
        main_extent_px: (u32, u32),
        profile: RenderPathProfile,
        offscreen_target: Option<OffscreenTargetHandles>,
    ) -> FrameViewPlan<'a> {
        let target = offscreen_target.map_or(
            FrameViewPlanTarget::Swapchain,
            FrameViewPlanTarget::offscreen,
        );
        FrameViewPlan::new(
            &self.host_camera,
            FrameViewPlanParams {
                render_context: self.scene.active_main_render_context(),
                frame_time_seconds: self.tick_state.frame_time_seconds(),
                view_id: ViewId::Main,
                viewport_px: main_extent_px,
                clear: FrameViewClear::skybox(),
                profile,
                target,
            },
        )
    }
}

fn assemble_view_family_plan<'a>(
    fallback_frame_global: &FrameGlobalView,
    secondary_views: Vec<FrameViewPlan<'a>>,
    hmd_view: Option<FrameViewPlan<'a>>,
    main_view: Option<FrameViewPlan<'a>>,
) -> ViewFamilyPlan<'a> {
    let frame_global = main_view
        .as_ref()
        .or(hmd_view.as_ref())
        .map_or(*fallback_frame_global, |view| view.frame_global_view());

    let views = assemble_ordered_view_plans(secondary_views, hmd_view, main_view);
    ViewFamilyPlan::new(&frame_global, views)
}

fn assemble_ordered_view_plans<'a>(
    mut secondary_views: Vec<FrameViewPlan<'a>>,
    hmd_view: Option<FrameViewPlan<'a>>,
    main_view: Option<FrameViewPlan<'a>>,
) -> Vec<FrameViewPlan<'a>> {
    let mut views = Vec::with_capacity(
        secondary_views.len() + usize::from(hmd_view.is_some()) + usize::from(main_view.is_some()),
    );
    views.append(&mut secondary_views);
    views.extend(hmd_view);
    views.extend(main_view);
    views
}

#[cfg(test)]
mod tests;
