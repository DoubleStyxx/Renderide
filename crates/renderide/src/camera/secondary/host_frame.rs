//! Secondary render-texture camera resolution and HostCameraFrame construction.

use glam::{Mat4, Vec3};

use crate::camera::host_camera_frame::{SingleCameraInputs, build_single_camera_frame};
use crate::camera::{
    CameraClipPlanes, CameraPose, HostCameraFrame, Viewport, clamp_desktop_fov_degrees,
};
use crate::shared::CameraState;

use super::flags::camera_state_use_transform_scale;

const CAMERA_CONTROLLER_SCALE_MIN: f32 = 1e-5;
const CAMERA_CONTROLLER_SCALE_MAX: f32 = 1e6;
const CAMERA_CONTROLLER_ORTHOGRAPHIC_SIZE_MIN: f32 = 1e-6;
const CAMERA_CONTROLLER_ORTHOGRAPHIC_SIZE_MAX: f32 = 1e6;
const CAMERA_CONTROLLER_NEAR_CLIP_MIN: f32 = 1e-4;
const CAMERA_CONTROLLER_CLIP_MAX: f32 = 1e6;
const CAMERA_CONTROLLER_FAR_ABOVE_NEAR_MIN: f32 = 1e-4;

/// Builds a [`HostCameraFrame`] for rendering through a secondary camera to a render texture.
pub fn host_camera_frame_for_render_texture(
    base: &HostCameraFrame,
    state: &CameraState,
    viewport_px: (u32, u32),
    camera_world_matrix: Mat4,
) -> HostCameraFrame {
    let transform_scale = camera_controller_transform_scale(state.flags, camera_world_matrix);
    build_single_camera_frame(
        base,
        SingleCameraInputs {
            viewport: Viewport::from_tuple(viewport_px),
            pose: CameraPose::from_world_matrix(camera_world_matrix),
            clip: camera_controller_clip_planes(state, transform_scale),
            fov_degrees: clamp_desktop_fov_degrees(state.field_of_view),
            orthographic_size: camera_controller_orthographic_size(state, transform_scale),
            projection: state.projection,
            suppress_occlusion_temporal: false,
        },
    )
}

fn camera_controller_transform_scale(flags: u16, matrix: Mat4) -> f32 {
    if !camera_state_use_transform_scale(flags) {
        return 1.0;
    }
    let (scale, _, _) = matrix.to_scale_rotation_translation();
    camera_controller_transform_scale_from_lossy(scale)
}

fn camera_controller_transform_scale_from_lossy(lossy_scale: Vec3) -> f32 {
    let mut scale = (lossy_scale.x + lossy_scale.y + lossy_scale.z) * 0.333_333_34;
    if scale.is_nan() {
        scale = 0.0;
    }
    camera_controller_clamp(
        scale,
        CAMERA_CONTROLLER_SCALE_MIN,
        CAMERA_CONTROLLER_SCALE_MAX,
    )
}

fn camera_controller_clip_planes(state: &CameraState, transform_scale: f32) -> CameraClipPlanes {
    let near_clip = camera_controller_clamp(
        state.near_clip * transform_scale,
        CAMERA_CONTROLLER_NEAR_CLIP_MIN,
        CAMERA_CONTROLLER_CLIP_MAX,
    );
    let far_min =
        CAMERA_CONTROLLER_NEAR_CLIP_MIN.max(near_clip + CAMERA_CONTROLLER_FAR_ABOVE_NEAR_MIN);
    let far_clip = camera_controller_clamp(
        state.far_clip * transform_scale,
        far_min,
        CAMERA_CONTROLLER_CLIP_MAX,
    );
    CameraClipPlanes::new(near_clip, far_clip)
}

fn camera_controller_orthographic_size(state: &CameraState, transform_scale: f32) -> f32 {
    camera_controller_clamp(
        state.orthographic_size * transform_scale,
        CAMERA_CONTROLLER_ORTHOGRAPHIC_SIZE_MIN,
        CAMERA_CONTROLLER_ORTHOGRAPHIC_SIZE_MAX,
    )
}

fn camera_controller_clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat4, Vec3};

    use super::{
        camera_controller_transform_scale_from_lossy, host_camera_frame_for_render_texture,
    };
    use crate::camera::{
        CameraClipPlanes, CameraProjectionKind, EyeView, HostCameraFrame,
        OrthographicProjectionSpec, StereoViewMatrices, Viewport, WorldProjectionSet,
        apply_view_handedness_fix,
    };
    use crate::scene::SceneCoordinator;
    use crate::shared::{CameraProjection, CameraState, HeadOutputDevice};

    #[test]
    fn host_camera_frame_secondary_without_transform_scale_uses_camera_controller_clamps() {
        let state = CameraState {
            projection: CameraProjection::Orthographic,
            orthographic_size: 0.0,
            near_clip: 0.0,
            far_clip: 0.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(
            &HostCameraFrame::default(),
            &state,
            (640, 480),
            Mat4::from_scale(Vec3::splat(9.0)),
        );

        let ortho = out.primary_ortho_task.expect("orthographic task");
        assert_eq!(ortho.half_height, 1e-6);
        assert!((out.near_clip() - 1e-4).abs() < 1e-8);
        assert!((out.far_clip() - 2e-4).abs() < 1e-8);
    }

    #[test]
    fn host_camera_frame_secondary_sets_pose_and_projection_override() {
        let base = HostCameraFrame {
            output_device: HeadOutputDevice::Screen,
            ..Default::default()
        };
        let cam_world = Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));
        let state = CameraState {
            projection: CameraProjection::Perspective,
            field_of_view: 55.0,
            near_clip: 0.05,
            far_clip: 2000.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(&base, &state, (1280, 720), cam_world);

        let expected_w2v = apply_view_handedness_fix(cam_world.inverse());
        let explicit = out.explicit_view.expect("secondary camera explicit view");
        assert_eq!(explicit.view, expected_w2v);
        assert!(explicit.proj.is_finite());
        assert_eq!(out.primary_ortho_task, None);
        assert_eq!(out.projection_kind, CameraProjectionKind::Perspective);
        assert_eq!(out.desktop_fov_degrees, state.field_of_view);
        assert!(!out.vr_active);
    }

    #[test]
    fn host_camera_frame_secondary_clears_vr_stereo_state() {
        let eye = EyeView::new(Mat4::IDENTITY, Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO);
        let base = HostCameraFrame {
            vr_active: true,
            stereo: Some(StereoViewMatrices::new(eye, eye)),
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(
            &base,
            &CameraState::default(),
            (1280, 720),
            Mat4::IDENTITY,
        );

        assert!(!out.vr_active);
        assert!(out.stereo.is_none());
        assert!(out.active_stereo().is_none());
    }

    #[test]
    fn host_camera_frame_secondary_orthographic_sets_primary_ortho_task() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::IDENTITY;
        let state = CameraState {
            projection: CameraProjection::Orthographic,
            orthographic_size: 8.0,
            near_clip: 0.1,
            far_clip: 500.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(&base, &state, (640, 480), cam_world);

        let ortho = out.primary_ortho_task.expect("orthographic task");
        assert_eq!(ortho.half_height, 8.0);
        assert_eq!(ortho.clip, out.clip);
        assert_eq!(out.projection_kind, CameraProjectionKind::Orthographic);
        assert!(out.explicit_view.expect("explicit view").proj.is_finite());
    }

    #[test]
    fn host_camera_frame_secondary_orthographic_uses_state_projection_matrix() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::IDENTITY;
        let viewport = (640, 480);
        let clip = CameraClipPlanes::new(0.1, 500.0);
        let perspective_state = CameraState {
            projection: CameraProjection::Perspective,
            field_of_view: 60.0,
            near_clip: clip.near,
            far_clip: clip.far,
            ..Default::default()
        };
        let orthographic_state = CameraState {
            projection: CameraProjection::Orthographic,
            orthographic_size: 8.0,
            near_clip: clip.near,
            far_clip: clip.far,
            ..Default::default()
        };

        let perspective =
            host_camera_frame_for_render_texture(&base, &perspective_state, viewport, cam_world);
        let orthographic =
            host_camera_frame_for_render_texture(&base, &orthographic_state, viewport, cam_world);
        let expected_ortho =
            OrthographicProjectionSpec::new(8.0, clip).projection(Viewport::from_tuple(viewport));

        let perspective_proj = perspective.explicit_view.expect("perspective view").proj;
        let orthographic_proj = orthographic.explicit_view.expect("orthographic view").proj;
        assert_eq!(orthographic_proj, expected_ortho);
        assert_ne!(orthographic_proj, perspective_proj);
    }

    #[test]
    fn orthographic_camera_state_reaches_world_projection_set() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::IDENTITY;
        let viewport = (640, 480);
        let clip = CameraClipPlanes::new(0.1, 500.0);
        let state = CameraState {
            projection: CameraProjection::Orthographic,
            orthographic_size: 8.0,
            near_clip: clip.near,
            far_clip: clip.far,
            ..Default::default()
        };

        let host_camera = host_camera_frame_for_render_texture(&base, &state, viewport, cam_world);
        let expected_ortho =
            OrthographicProjectionSpec::new(8.0, clip).projection(Viewport::from_tuple(viewport));
        let projections =
            WorldProjectionSet::from_scene_host(&SceneCoordinator::new(), viewport, &host_camera);

        assert_eq!(projections.world_proj, expected_ortho);
        assert_eq!(projections.overlay_proj, expected_ortho);
    }

    #[test]
    fn host_camera_frame_secondary_use_transform_scale_averages_non_uniform_scale() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::from_scale(Vec3::new(1.0, 2.0, 6.0));
        let state = CameraState {
            projection: CameraProjection::Orthographic,
            flags: 1 << 1,
            orthographic_size: 2.0,
            near_clip: 0.1,
            far_clip: 50.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(&base, &state, (640, 480), cam_world);

        let ortho = out.primary_ortho_task.expect("orthographic task");
        assert!((ortho.half_height - 6.0).abs() < 1e-6);
        assert!((out.near_clip() - 0.3).abs() < 1e-6);
        assert!((out.far_clip() - 150.0).abs() < 1e-4);
    }

    #[test]
    fn host_camera_frame_secondary_perspective_use_transform_scale_scales_clip_not_fov() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::from_scale(Vec3::splat(4.0));
        let state = CameraState {
            projection: CameraProjection::Perspective,
            flags: 1 << 1,
            field_of_view: 45.0,
            near_clip: 0.2,
            far_clip: 30.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(&base, &state, (640, 480), cam_world);

        assert_eq!(out.primary_ortho_task, None);
        assert_eq!(out.desktop_fov_degrees, 45.0);
        assert!((out.near_clip() - 0.8).abs() < 1e-6);
        assert!((out.far_clip() - 120.0).abs() < 1e-4);
    }

    #[test]
    fn camera_controller_transform_scale_matches_clamp_edges() {
        assert_eq!(
            camera_controller_transform_scale_from_lossy(Vec3::splat(f32::NAN)),
            1e-5
        );
        assert_eq!(
            camera_controller_transform_scale_from_lossy(Vec3::splat(-2.0)),
            1e-5
        );
        assert_eq!(
            camera_controller_transform_scale_from_lossy(Vec3::splat(2.0e6)),
            1e6
        );
    }

    #[test]
    fn host_camera_frame_secondary_use_transform_scale_scales_ortho_and_clip() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::from_scale(Vec3::splat(2.0));
        let state = CameraState {
            projection: CameraProjection::Orthographic,
            flags: 1 << 1,
            orthographic_size: 4.0,
            near_clip: 0.1,
            far_clip: 500.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(&base, &state, (640, 480), cam_world);

        let ortho = out.primary_ortho_task.expect("orthographic task");
        assert_eq!(ortho.half_height, 8.0);
        assert!((out.near_clip() - 0.2).abs() < 1e-6);
        assert!((out.far_clip() - 1000.0).abs() < 1e-4);
    }

    #[test]
    fn host_camera_frame_secondary_use_transform_scale_does_not_scale_view_matrix() {
        let base = HostCameraFrame::default();
        let cam_world = Mat4::from_scale(Vec3::splat(1.5));
        let state = CameraState {
            projection: CameraProjection::Orthographic,
            flags: 1 << 1,
            orthographic_size: 0.5,
            near_clip: 0.01,
            far_clip: 4.0,
            ..Default::default()
        };

        let out = host_camera_frame_for_render_texture(&base, &state, (1920, 1080), cam_world);

        let explicit = out.explicit_view.expect("scaled orthographic view");
        assert!(
            explicit
                .view
                .abs_diff_eq(apply_view_handedness_fix(Mat4::IDENTITY), 1e-6)
        );
        let ortho = out.primary_ortho_task.expect("orthographic task");
        assert!((ortho.half_height - 0.75).abs() < 1e-6);
        assert!((out.near_clip() - 0.015).abs() < 1e-6);
        assert!((out.far_clip() - 6.0).abs() < 1e-6);
    }
}
