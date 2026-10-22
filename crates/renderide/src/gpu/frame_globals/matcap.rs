//! CPU mirror of the Matcap UV in `shaders/modules/frame/globals.wgsl`.
//!
//! The shader is the runtime lookup. These helpers are the per-eye uniform oracle: which
//! [`FrameGpuUniforms`] rows a view layer reads, and how multiview UVs move when the eye
//! translation changes. Mono permutations stay on the rotation-only UV so desktop and in-world
//! cameras keep the upright Unity `UNITY_MATRIX_V` result.

use bytemuck::Zeroable;
use glam::{Mat4, Vec2, Vec3};

use super::uniforms::{FRAME_PROJECTION_FLAG_ORTHOGRAPHIC, FrameGpuUniforms};

/// Matches `safe_normalize_or` in `shaders/modules/frame/globals.wgsl`.
const NORMALIZE_EPSILON: f32 = 0.000_001;

/// View rows and orthographic flag one Matcap view layer reads from [`FrameGpuUniforms`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MatcapViewRows {
    /// World -> view X row, including translation in `.w`.
    pub x: [f32; 4],
    /// World -> view Y row, including translation in `.w`.
    pub y: [f32; 4],
    /// World -> view Z row, including translation in `.w`.
    pub z: [f32; 4],
    /// `true` when this eye's projection flags mark an orthographic camera.
    pub orthographic: bool,
}

/// Packs per-eye world-to-view rows the same way the forward pass uploads them.
pub(super) fn frame_from_eye_views(
    left: Mat4,
    right: Mat4,
    orthographic: bool,
) -> FrameGpuUniforms {
    let mut frame = FrameGpuUniforms::zeroed();
    frame.view_space_x_coeffs = FrameGpuUniforms::view_space_x_coeffs_from_world_to_view(left);
    frame.view_space_y_coeffs = FrameGpuUniforms::view_space_y_coeffs_from_world_to_view(left);
    frame.view_space_z_coeffs = FrameGpuUniforms::view_space_z_coeffs_from_world_to_view(left);
    frame.view_space_x_coeffs_right =
        FrameGpuUniforms::view_space_x_coeffs_from_world_to_view(right);
    frame.view_space_y_coeffs_right =
        FrameGpuUniforms::view_space_y_coeffs_from_world_to_view(right);
    frame.view_space_z_coeffs_right =
        FrameGpuUniforms::view_space_z_coeffs_from_world_to_view(right);
    let flag = if orthographic {
        FRAME_PROJECTION_FLAG_ORTHOGRAPHIC
    } else {
        0
    };
    // `.y` is the left/mono projection flags; `.z` is the right-eye flags.
    frame.frame_tail = [0, flag, flag, 0];
    frame
}

/// Rows selected by `view_index_from_layer`: the low bit is the eye, matching multiview `view_idx`.
pub(super) fn matcap_view_rows(frame: &FrameGpuUniforms, view_layer: u32) -> MatcapViewRows {
    if view_layer & 1 != 0 {
        MatcapViewRows {
            x: frame.view_space_x_coeffs_right,
            y: frame.view_space_y_coeffs_right,
            z: frame.view_space_z_coeffs_right,
            orthographic: frame.frame_tail[2] & FRAME_PROJECTION_FLAG_ORTHOGRAPHIC != 0,
        }
    } else {
        MatcapViewRows {
            x: frame.view_space_x_coeffs,
            y: frame.view_space_y_coeffs,
            z: frame.view_space_z_coeffs,
            orthographic: frame.frame_tail[1] & FRAME_PROJECTION_FLAG_ORTHOGRAPHIC != 0,
        }
    }
}

/// Matcap UV for one view layer.
///
/// `multiview == false` matches the mono shader permutation (left/mono rows, rotation-only UV).
/// `multiview == true` matches the `#ifdef MULTIVIEW` perspective correction.
pub(super) fn matcap_uv(
    frame: &FrameGpuUniforms,
    world_normal: Vec3,
    world_pos: Vec3,
    view_layer: u32,
    multiview: bool,
) -> Vec2 {
    let rows = if multiview {
        matcap_view_rows(frame, view_layer)
    } else {
        matcap_view_rows(frame, 0)
    };
    let n_view = safe_normalize(
        Vec3::new(
            dot_row(rows.x, world_normal, false),
            dot_row(rows.y, world_normal, false),
            dot_row(rows.z, world_normal, false),
        ),
        Vec3::new(0.0, 0.0, 1.0),
    );
    let orientation_uv = n_view.truncate() * 0.5 + Vec2::splat(0.5);
    if !multiview {
        return orientation_uv;
    }
    let view_pos = Vec3::new(
        dot_row(rows.x, world_pos, true),
        dot_row(rows.y, world_pos, true),
        dot_row(rows.z, world_pos, true),
    );
    let fallback = Vec3::new(0.0, 0.0, -1.0);
    let view_dir = if rows.orthographic {
        fallback
    } else {
        safe_normalize(view_pos, fallback)
    };
    let c = view_dir.cross(n_view);
    Vec2::new(-c.y, c.x) * 0.5 + Vec2::splat(0.5)
}

fn dot_row(row: [f32; 4], p: Vec3, with_translation: bool) -> f32 {
    let linear = row[0] * p.x + row[1] * p.y + row[2] * p.z;
    if with_translation {
        linear + row[3]
    } else {
        linear
    }
}

fn safe_normalize(v: Vec3, fallback: Vec3) -> Vec3 {
    let len = v.length();
    if len <= NORMALIZE_EPSILON {
        fallback
    } else {
        v / len
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat4, Vec2, Vec3};

    use super::{frame_from_eye_views, matcap_uv, matcap_view_rows};
    use crate::camera::apply_view_handedness_fix;

    const EYE_X: f32 = 0.032;

    fn eye_view(eye_world: Vec3) -> Mat4 {
        apply_view_handedness_fix(Mat4::from_translation(eye_world).inverse())
    }

    fn assert_uv_close(actual: Vec2, expected: Vec2) {
        let delta = (actual - expected).length();
        assert!(
            delta < 1e-5,
            "matcap UV {actual:?} != {expected:?} (delta {delta})"
        );
    }

    /// Independent of [`matcap_uv`]: view-space cross product from the matrix itself.
    fn matrix_multiview_uv(view: Mat4, world_normal: Vec3, world_pos: Vec3) -> Vec2 {
        let n_view = (view * world_normal.extend(0.0)).truncate().normalize();
        let view_dir = view.transform_point3(world_pos).normalize();
        let c = view_dir.cross(n_view);
        Vec2::new(-c.y, c.x) * 0.5 + Vec2::splat(0.5)
    }

    #[test]
    fn mono_uv_stays_on_view_normal_xy_for_off_axis_points() {
        let left = eye_view(Vec3::new(-EYE_X, 0.0, 0.0));
        let right = eye_view(Vec3::new(EYE_X, 0.0, 0.0));
        let frame = frame_from_eye_views(left, right, false);
        let normal = Vec3::new(0.0, 0.0, -1.0);
        let point = Vec3::new(0.0, 0.0, 1.0);
        let n_view = (left * normal.extend(0.0)).truncate().normalize();
        let expected = n_view.truncate() * 0.5 + Vec2::splat(0.5);

        assert_uv_close(matcap_uv(&frame, normal, point, 0, false), expected);
        assert_uv_close(matcap_uv(&frame, normal, point, 1, false), expected);
    }

    #[test]
    fn world_up_samples_above_the_matcap_center() {
        let view = eye_view(Vec3::ZERO);
        let frame = frame_from_eye_views(view, view, false);
        let uv = matcap_uv(&frame, Vec3::Y, Vec3::new(0.0, 0.0, 1.0), 0, false);
        assert!(
            uv.y > 0.5 + 0.4,
            "camera +Y must stay toward the top of the matcap, got {uv:?}"
        );
        assert!(
            (uv.x - 0.5).abs() < 1e-5,
            "pure up normal stays centered in x"
        );
    }

    #[test]
    fn parallel_eyes_shift_the_highlight_with_parallax_not_a_swap() {
        let left = eye_view(Vec3::new(-EYE_X, 0.0, 0.0));
        let right = eye_view(Vec3::new(EYE_X, 0.0, 0.0));
        let frame = frame_from_eye_views(left, right, false);
        let normal = Vec3::new(0.0, 0.0, -1.0);
        let point = Vec3::new(0.0, 0.0, 1.0);

        let left_uv = matcap_uv(&frame, normal, point, 0, true);
        let right_uv = matcap_uv(&frame, normal, point, 1, true);
        assert_uv_close(left_uv, matrix_multiview_uv(left, normal, point));
        assert_uv_close(right_uv, matrix_multiview_uv(right, normal, point));

        // The geometric front faces the head center. The left eye sees that normal to its right,
        // so it samples to the right of the matcap center. The right eye samples to the left.
        assert!(left_uv.x > 0.5 + 1e-3, "left eye UV {left_uv:?}");
        assert!(right_uv.x < 0.5 - 1e-3, "right eye UV {right_uv:?}");
        assert!((left_uv.x - 0.5 + (right_uv.x - 0.5)).abs() < 1e-4);
        assert!((left_uv.y - 0.5).abs() < 1e-5);
        assert!((right_uv.y - 0.5).abs() < 1e-5);
    }

    #[test]
    fn swapping_eye_rows_inverts_the_horizontal_shift() {
        let left = eye_view(Vec3::new(-EYE_X, 0.0, 0.0));
        let right = eye_view(Vec3::new(EYE_X, 0.0, 0.0));
        let swapped = frame_from_eye_views(right, left, false);
        let normal = Vec3::new(0.0, 0.0, -1.0);
        let point = Vec3::new(0.0, 0.0, 1.0);
        let uv = matcap_uv(&swapped, normal, point, 0, true);
        assert!(
            uv.x < 0.5 - 1e-3,
            "view layer 0 must read the first eye's rows; swapped rows moved UV to {uv:?}"
        );
    }

    #[test]
    fn one_shared_view_matrix_keeps_both_eyes_screen_locked() {
        let left = eye_view(Vec3::new(-EYE_X, 0.0, 0.0));
        let locked = frame_from_eye_views(left, left, false);
        let normal = Vec3::new(0.0, 0.0, -1.0);
        let point = Vec3::new(0.0, 0.0, 1.0);
        let left_uv = matcap_uv(&locked, normal, point, 0, true);
        let right_uv = matcap_uv(&locked, normal, point, 1, true);
        assert_uv_close(left_uv, right_uv);

        let stereo = frame_from_eye_views(left, eye_view(Vec3::new(EYE_X, 0.0, 0.0)), false);
        let separated = (matcap_uv(&stereo, normal, point, 0, true)
            - matcap_uv(&stereo, normal, point, 1, true))
        .length();
        assert!(separated > 1e-3, "per-eye translations must separate UVs");
    }

    #[test]
    fn optical_axis_and_orthographic_match_the_orientation_uv() {
        let left = eye_view(Vec3::new(-EYE_X, 0.2, 0.0));
        let right = eye_view(Vec3::new(EYE_X, 0.2, 0.0));
        let normal = Vec3::new(0.6, -0.2, -0.7746).normalize();
        let on_axis = Vec3::new(-EYE_X, 0.2, 1.5);
        let perspective = frame_from_eye_views(left, right, false);
        let n_view = (left * normal.extend(0.0)).truncate().normalize();
        let orientation = n_view.truncate() * 0.5 + Vec2::splat(0.5);
        assert_uv_close(
            matcap_uv(&perspective, normal, on_axis, 0, true),
            orientation,
        );

        let ortho = frame_from_eye_views(left, right, true);
        let off_axis = Vec3::new(0.4, -0.3, 1.5);
        assert_uv_close(matcap_uv(&ortho, normal, off_axis, 0, true), orientation);
        assert_uv_close(
            matcap_uv(&ortho, normal, off_axis, 1, true),
            (right * normal.extend(0.0))
                .truncate()
                .normalize()
                .truncate()
                * 0.5
                + Vec2::splat(0.5),
        );
    }

    #[test]
    fn packed_view_layer_low_bit_selects_the_eye() {
        let left = eye_view(Vec3::ZERO);
        let right = Mat4::from_rotation_y(0.4) * eye_view(Vec3::new(EYE_X, 0.0, 0.0));
        let frame = frame_from_eye_views(left, right, false);
        assert_eq!(matcap_view_rows(&frame, 2).x, frame.view_space_x_coeffs);
        assert_eq!(
            matcap_view_rows(&frame, 3).x,
            frame.view_space_x_coeffs_right
        );
        let normal = Vec3::X;
        let point = Vec3::new(0.0, 0.0, 1.0);
        assert_uv_close(
            matcap_uv(&frame, normal, point, 3, true),
            matcap_uv(&frame, normal, point, 1, true),
        );
    }

    #[test]
    fn yawed_eye_rows_round_trip_through_the_uniform_rows() {
        let left = Mat4::from_rotation_y(0.35) * eye_view(Vec3::new(-EYE_X, 0.05, 0.1));
        let right = Mat4::from_rotation_y(-0.2) * eye_view(Vec3::new(EYE_X, -0.04, 0.1));
        let frame = frame_from_eye_views(left, right, false);
        let normal = Vec3::new(-0.2, 0.7, -0.6).normalize();
        let point = Vec3::new(0.15, -0.25, 0.8);
        assert_uv_close(
            matcap_uv(&frame, normal, point, 0, true),
            matrix_multiview_uv(left, normal, point),
        );
        assert_uv_close(
            matcap_uv(&frame, normal, point, 1, true),
            matrix_multiview_uv(right, normal, point),
        );
    }
}
