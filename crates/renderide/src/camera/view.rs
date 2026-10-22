//! World-to-view construction and overlay-space rules for mesh rendering.

use glam::{Mat4, Vec3};

use crate::scene::render_transform_to_matrix;
use crate::scene::{RenderSpaceRead, SceneSpaceRead};
use crate::shared::RenderTransform;

use super::host_camera_frame::{HostCameraFrame, overlay_camera_view_matrix};

/// Clamps scale for view matrix construction: if any axis is nearly zero, use unit scale.
pub fn filter_scale_legacy(scale: Vec3) -> Vec3 {
    if scale.x.min(scale.y).min(scale.z) <= 1e-8 {
        Vec3::splat(1.0)
    } else {
        scale
    }
}

/// Z-flip for RH engine space to Vulkan/WebGPU-style clip.
#[inline]
pub fn apply_view_handedness_fix(view: Mat4) -> Mat4 {
    let z_flip = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
    z_flip * view
}

/// World-to-view matrix from a host [`RenderTransform`] (camera / eye TRS).
///
/// Applies scale filtering and handedness fix so `view_proj * world_pos` matches the mesh pass.
pub fn view_matrix_from_render_transform(tr: &RenderTransform) -> Mat4 {
    let mut t = *tr;
    let fs = filter_scale_legacy(Vec3::new(tr.scale.x, tr.scale.y, tr.scale.z));
    t.scale.x = fs.x;
    t.scale.y = fs.y;
    t.scale.z = fs.z;
    let cam = render_transform_to_matrix(&t);
    apply_view_handedness_fix(cam.inverse())
}

/// World-to-view for mesh rendering in `space`, accounting for overlay render spaces.
///
/// Overlay render spaces re-root object meshes into the main world's coordinates via
/// [`SceneCoordinator::world_matrix_for_render_context`]; the camera view must therefore match the
/// active main (non-overlay) space, not the overlay space's own view transform (Unity-style
/// head output + overlay positioning parity).
pub fn view_matrix_for_world_mesh_render_space<S>(scene: &S, space: S::Space<'_>) -> Mat4
where
    S: SceneSpaceRead + ?Sized,
{
    if space.is_overlay() {
        scene.active_main_space().map_or_else(
            || view_matrix_from_render_transform(space.view_transform()),
            |main| view_matrix_from_render_transform(main.view_transform()),
        )
    } else {
        view_matrix_from_render_transform(space.view_transform())
    }
}

/// World-to-view for mesh rendering, honoring an explicit camera override when present.
pub fn view_matrix_for_host_world_mesh_space<S>(
    scene: &S,
    space: S::Space<'_>,
    host_camera: &HostCameraFrame,
) -> Mat4
where
    S: SceneSpaceRead + ?Sized,
{
    host_camera
        .explicit_world_to_view()
        .unwrap_or_else(|| view_matrix_for_world_mesh_render_space(scene, space))
}

/// Left/right world-to-view matrices used by skybox ray reconstruction.
pub fn world_to_view_pair_for_skybox(
    scene: &(impl SceneSpaceRead + ?Sized),
    host_camera: &HostCameraFrame,
) -> (Mat4, Mat4) {
    if let Some(stereo) = host_camera.active_stereo() {
        return stereo.view_pair();
    }
    let view = host_camera.explicit_world_to_view().unwrap_or_else(|| {
        scene.active_main_space().map_or(Mat4::IDENTITY, |space| {
            view_matrix_for_world_mesh_render_space(scene, space)
        })
    });
    (view, view)
}

/// World-space camera forward for a world-to-view matrix.
///
/// View space is right-handed with −Z forward. The returned vector is
/// `inverse(view) * (0, 0, −1)`, normalized. A zero vector means the view basis is unusable.
#[inline]
pub(crate) fn view_forward_world_from_view_matrix(view: Mat4) -> Vec3 {
    unit_direction(view.inverse().transform_vector3(Vec3::new(0.0, 0.0, -1.0)))
}

/// Camera forward used when collecting one view's draws.
///
/// Prefers [`HostCameraFrame::explicit_world_to_view`] and otherwise the active main-space view.
/// Identity is used only when neither exists, which looks down world −Z.
pub(crate) fn view_forward_world_for_collection(
    scene: &(impl SceneSpaceRead + ?Sized),
    host_camera: &HostCameraFrame,
) -> Vec3 {
    let view = host_camera.explicit_world_to_view().unwrap_or_else(|| {
        scene.active_main_space().map_or(Mat4::IDENTITY, |space| {
            view_matrix_for_world_mesh_render_space(scene, space)
        })
    });
    view_forward_world_from_view_matrix(view)
}

/// Overlay-camera origin and forward in overlay-local space.
///
/// Desktop overlay draws are not in the main camera frame, so their canvas plane depth uses this
/// fixed basis instead of [`view_forward_world_for_collection`].
pub(crate) fn overlay_canvas_view_basis() -> (Vec3, Vec3) {
    let view = overlay_camera_view_matrix();
    let origin = view.inverse().transform_point3(Vec3::ZERO);
    (origin, view_forward_world_from_view_matrix(view))
}

/// Returns a unit direction, or zero when `direction` is non-finite or degenerate.
#[inline]
fn unit_direction(direction: Vec3) -> Vec3 {
    if !direction.is_finite() {
        return Vec3::ZERO;
    }
    let length_sq = direction.length_squared();
    if length_sq <= 1.0e-12 {
        Vec3::ZERO
    } else {
        direction * length_sq.sqrt().recip()
    }
}
