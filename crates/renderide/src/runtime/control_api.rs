//! Dev-only drone control hooks on [`RendererRuntime`](super::RendererRuntime).
//!
//! The TCP server is owned here so it lives until the process exits. This module does not
//! own scene submission; host IPC still does.

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};

use crate::camera::{
    CameraPose, CameraProjectionKind, EyeView, HostCameraFrame, OrthographicProjectionSpec,
    Viewport,
};
use crate::control::{ControlServer, ControlSession};
use crate::frontend::input::WindowInputAccumulator;
use crate::gpu::GpuContext;
use crate::shared::InputState;

use super::RendererRuntime;

/// Host camera fields replaced for the duration of one main-view render.
#[derive(Clone, Copy)]
pub(super) struct ControlCameraBackup {
    desktop_fov_degrees: f32,
    projection_kind: CameraProjectionKind,
    primary_ortho_task: Option<OrthographicProjectionSpec>,
    explicit_view: Option<EyeView>,
    eye_world_position: Option<Vec3>,
}

impl RendererRuntime {
    /// Stores the control server for the rest of the process lifetime.
    pub(crate) fn attach_control_server(&mut self, server: ControlServer) {
        self.control_server = Some(server);
    }

    /// Shared session, when `--control-port` started the server.
    pub(crate) fn control_session(&self) -> Option<Arc<ControlSession>> {
        self.control_server.as_ref().map(ControlServer::session)
    }

    /// Publishes `meta` once from the live device.
    pub(crate) fn publish_control_meta(&self, gpu: &GpuContext) {
        if let Some(session) = self.control_session() {
            session.set_meta_once(crate::control::meta_from_gpu(gpu));
        }
    }

    /// Merges injected window input into the winit accumulator.
    pub(crate) fn apply_control_input_to_window(&self, accumulator: &mut WindowInputAccumulator) {
        if let Some(session) = self.control_session() {
            session.apply_to_window(accumulator);
        }
    }

    /// Builds a host input snapshot from injected state, or an empty snapshot.
    pub(crate) fn control_input_state(&self) -> InputState {
        self.control_session()
            .map(|session| session.take_input_state())
            .unwrap_or_default()
    }

    /// Arms the per-frame stats probe when a control session exists.
    pub(crate) fn begin_control_frame(&self) {
        if self.control_session().is_some() {
            crate::control::begin_frame();
        }
    }

    /// Applies the drone camera for this main view, or restores the host camera when idle.
    pub(crate) fn latch_control_camera(&mut self, gpu: &GpuContext) {
        let Some(session) = self.control_session() else {
            return;
        };
        let Some(pose) = session.current_pose() else {
            self.restore_control_camera();
            return;
        };
        if self.control_camera_backup.is_none() {
            self.control_camera_backup = Some(ControlCameraBackup::from_frame(&self.host_camera));
        }
        apply_pose(&mut self.host_camera, &pose, gpu.surface_extent_px());
    }

    /// Puts host camera fields back after a drone pose was applied.
    pub(crate) fn restore_control_camera(&mut self) {
        let Some(backup) = self.control_camera_backup.take() else {
            return;
        };
        backup.write_to(&mut self.host_camera);
    }
}

impl ControlCameraBackup {
    fn from_frame(camera: &HostCameraFrame) -> Self {
        Self {
            desktop_fov_degrees: camera.desktop_fov_degrees,
            projection_kind: camera.projection_kind,
            primary_ortho_task: camera.primary_ortho_task,
            explicit_view: camera.explicit_view,
            eye_world_position: camera.eye_world_position,
        }
    }

    fn write_to(self, camera: &mut HostCameraFrame) {
        camera.desktop_fov_degrees = self.desktop_fov_degrees;
        camera.projection_kind = self.projection_kind;
        camera.primary_ortho_task = self.primary_ortho_task;
        camera.explicit_view = self.explicit_view;
        camera.eye_world_position = self.eye_world_position;
    }
}

fn apply_pose(
    camera: &mut HostCameraFrame,
    pose: &renderide_control_client::CameraPoseParams,
    extent: (u32, u32),
) {
    let [x, y, z, w] = pose.rot;
    let rotation = Quat::from_xyzw(x, y, z, w).normalize();
    let position = Vec3::from_array(pose.pos);
    let world = Mat4::from_rotation_translation(rotation, position);
    let camera_pose = CameraPose::from_world_matrix(world);
    let viewport = Viewport::from_tuple((extent.0.max(1), extent.1.max(1)));
    let eye = EyeView::perspective_from_pose(camera_pose, viewport, pose.fov, camera.clip);
    camera.desktop_fov_degrees = pose.fov;
    camera.projection_kind = CameraProjectionKind::Perspective;
    camera.primary_ortho_task = None;
    camera.explicit_view = Some(eye);
    camera.eye_world_position = Some(camera_pose.world_position);
}
