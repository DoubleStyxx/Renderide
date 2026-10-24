//! CPU mirror of the rigid object-velocity shader.
//!
//! Screen-space velocity is `previous_uv - current_uv`, matching `motion_blur.wgsl`.

use glam::{Mat4, Vec2, Vec3};

/// UV-space velocity of one rigid local-space point, or `None` when a clip `w` is degenerate.
pub(crate) fn rigid_clip_velocity(
    current_vp: Mat4,
    previous_vp: Mat4,
    current_model: Mat4,
    previous_model: Mat4,
    local: Vec3,
) -> Option<Vec2> {
    let local = local.extend(1.0);
    let current_clip = current_vp * (current_model * local);
    let previous_clip = previous_vp * (previous_model * local);
    // Non-positive w is behind the near plane. The shader discards those fragments so camera
    // velocity remains. TAA (#170) must pass unjittered view-projections on both sides.
    if current_clip.w <= 1.0e-6 || previous_clip.w <= 1.0e-6 {
        return None;
    }
    let current_uv = clip_xy_to_uv(
        current_clip.x / current_clip.w,
        current_clip.y / current_clip.w,
    );
    let previous_uv = clip_xy_to_uv(
        previous_clip.x / previous_clip.w,
        previous_clip.y / previous_clip.w,
    );
    Some(previous_uv - current_uv)
}

fn clip_xy_to_uv(x: f32, y: f32) -> Vec2 {
    Vec2::new(x.mul_add(0.5, 0.5), 0.5 - y * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_previous_model_moves_along_x_when_camera_is_still() {
        let velocity = rigid_clip_velocity(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.2, 0.0, 0.0)),
            Vec3::ZERO,
        )
        .expect("finite clip");
        assert!((velocity.x - 0.1).abs() < 1.0e-5);
        assert!(velocity.y.abs() < 1.0e-5);
    }

    #[test]
    fn identical_models_have_zero_object_velocity() {
        let model = Mat4::from_translation(Vec3::new(0.2, -0.4, 1.0));
        let velocity = rigid_clip_velocity(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            model,
            model,
            Vec3::new(0.25, -0.5, 0.0),
        )
        .expect("finite clip");
        assert!(velocity.length() < 1.0e-5);
    }

    #[test]
    fn negative_clip_w_falls_back_instead_of_streaking() {
        let previous_vp = Mat4::from_cols(
            glam::Vec4::X,
            glam::Vec4::Y,
            glam::Vec4::Z,
            glam::Vec4::new(0.0, 0.0, 0.0, -1.0),
        );
        assert!(
            rigid_clip_velocity(
                Mat4::IDENTITY,
                previous_vp,
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                Vec3::ZERO,
            )
            .is_none()
        );
    }
}
