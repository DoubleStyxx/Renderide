//! Per-frame resolved controller pose before IPC mapping.

use glam::{Quat, Vec3};

use crate::shared::Chirality;

use super::pose::{bound_hand_pose_defaults, openxr_grip_to_steamvr_raw, touch_pose_correction};
use super::profile::ActiveControllerProfile;

/// Resolved controller and optional bound-hand pose in tracking space.
#[derive(Clone, Copy)]
pub(super) struct ControllerFrame {
    /// Controller position in host-tracking space.
    pub(super) position: Vec3,
    /// Controller orientation in host-tracking space.
    pub(super) rotation: Quat,
    /// Whether the frame carries a calibrated bound-hand offset.
    pub(super) has_bound_hand: bool,
    /// Bound-hand position relative to the controller pose.
    pub(super) hand_position: Vec3,
    /// Bound-hand rotation relative to the controller pose.
    pub(super) hand_rotation: Quat,
}

/// Per-profile pose resolution.
///
/// The host controller node is the OpenVR raw device pose recovered from the OpenXR grip. An
/// untracked grip yields no frame: aim is not sampled, and `XR_EXT_palm_pose` is not a substitute.
/// Before this path, an untracked grip with a tracked palm still produced a frame because the palm
/// was reported as the controller pose. Touch then receives `SteamVRDriver.UpdateController`'s
/// extra 45° local pitch.
pub(super) fn resolve_controller_frame(
    profile: ActiveControllerProfile,
    side: Chirality,
    grip_pose: Option<(Vec3, Quat)>,
) -> Option<ControllerFrame> {
    let (has_bound_hand, hand_position_default, hand_rotation_default) =
        bound_hand_pose_defaults(profile, side);
    let (grip_position, grip_rotation) = grip_pose?;
    let (position, rotation) =
        openxr_grip_to_steamvr_raw(profile, side, grip_position, grip_rotation);
    let (position, rotation) = match profile {
        ActiveControllerProfile::Touch => touch_pose_correction(side, position, rotation),
        _ => (position, rotation),
    };

    Some(ControllerFrame {
        position,
        rotation,
        has_bound_hand,
        hand_position: hand_position_default,
        hand_rotation: hand_rotation_default,
    })
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use crate::shared::Chirality;
    use crate::xr::session::openxr_tracking_pose_to_host;

    use super::super::pose::openxr_grip_to_steamvr_raw;
    use super::super::profile::ActiveControllerProfile;
    use super::resolve_controller_frame;

    fn assert_vec3_near(actual: Vec3, expected: Vec3) {
        let delta = (actual - expected).length();
        assert!(
            delta < 1e-4,
            "vec3 mismatch: actual={actual:?} expected={expected:?} delta={delta}"
        );
    }

    fn assert_quat_near(actual: Quat, expected: Quat) {
        let dot = actual.normalize().dot(expected.normalize()).abs();
        assert!(
            (1.0 - dot) < 1e-4,
            "quat mismatch: actual={actual:?} expected={expected:?} dot={dot}"
        );
    }

    fn rotation_delta_angle(a: Quat, b: Quat) -> f32 {
        2.0 * a
            .normalize()
            .dot(b.normalize())
            .abs()
            .clamp(-1.0, 1.0)
            .acos()
    }

    #[test]
    fn index_raw_node_keeps_the_wrist_on_the_grip() {
        let grip_position = Vec3::new(0.2, 1.3, -0.4);
        let grip_rotation = (Quat::from_rotation_y(0.6) * Quat::from_rotation_x(-0.2)).normalize();
        let (host_grip_position, host_grip_rotation) =
            openxr_tracking_pose_to_host(grip_position, grip_rotation);

        for side in [Chirality::Left, Chirality::Right] {
            let frame = resolve_controller_frame(
                ActiveControllerProfile::Index,
                side,
                Some((grip_position, grip_rotation)),
            )
            .expect("frame");
            let (expected_position, expected_rotation) = openxr_grip_to_steamvr_raw(
                ActiveControllerProfile::Index,
                side,
                grip_position,
                grip_rotation,
            );
            assert_vec3_near(frame.position, expected_position);
            assert_quat_near(frame.rotation, expected_rotation);
            assert!(frame.has_bound_hand);
            let wrist_position = frame.position + frame.rotation * frame.hand_position;
            let wrist_rotation = (frame.rotation * frame.hand_rotation).normalize();
            assert_vec3_near(wrist_position, host_grip_position);
            assert_quat_near(wrist_rotation, host_grip_rotation);
        }
    }

    #[test]
    fn untracked_grip_yields_no_controller_frame() {
        for profile in [
            ActiveControllerProfile::Touch,
            ActiveControllerProfile::Index,
            ActiveControllerProfile::ViveFocus3,
            ActiveControllerProfile::Generic,
        ] {
            assert!(
                resolve_controller_frame(profile, Chirality::Left, None).is_none(),
                "{profile:?} must not invent a frame when grip is untracked",
            );
        }
    }

    /// Level OpenXR aim is identity, so its host forward is +Z. Facebook's published Quest 2
    /// components put aim at `RotX(-39.4°)` and grip at `RotX(+20.6°)` in the raw frame, so a
    /// level aim puts the grip at `RotX(+60°)` in OpenXR. The quaternion below is that 60°
    /// rotation (`sin(30°), 0, 0, cos(30°)`), not a value read back from the grip constants.
    ///
    /// Host forward of the recovered raw pose is `(0, sin 39.4°, cos 39.4°)` (39.4° up).
    /// The Touch node, after the host-space 45° pitch, is `(0, -sin 5.6°, cos 5.6°)` (5.6° below
    /// the level aim). Both hands share the pitch. The literals are `sin`/`cos` of those angles.
    #[test]
    fn level_touch_aim_raw_points_39_4_up_and_node_points_5_6_below() {
        let grip_rotation = Quat::from_xyzw(0.5, 0.0, 0.0, 0.866_025_4);
        let raw_forward = Vec3::new(0.0, 0.634_730_5, 0.772_733_6);
        let node_forward = Vec3::new(0.0, -0.097_582_9, 0.995_227_4);
        for side in [Chirality::Left, Chirality::Right] {
            let (_, raw_rotation) = openxr_grip_to_steamvr_raw(
                ActiveControllerProfile::Touch,
                side,
                Vec3::ZERO,
                grip_rotation,
            );
            assert_vec3_near(raw_rotation * Vec3::Z, raw_forward);
            let frame = resolve_controller_frame(
                ActiveControllerProfile::Touch,
                side,
                Some((Vec3::ZERO, grip_rotation)),
            )
            .expect("frame");
            assert_vec3_near(frame.rotation * Vec3::Z, node_forward);
        }
    }

    /// Composed wrist palm normal `(node_rotation * hand_rotation) * +Y` for the same level-aim
    /// grip, both hands. Literals come from Unity `Quaternion.Euler` order Y then X then Z:
    /// left `(185, -95, -90)` and right `(5, -95, -90)`, each post-multiplied by
    /// `inverse(Euler(90, 90, 90))`, then composed with host `RotX(+5.6°)`. They are not the
    /// output of [`bound_hand_pose_defaults`](super::super::pose::bound_hand_pose_defaults).
    #[test]
    fn touch_wrist_palm_normals_match_both_hands() {
        let grip_rotation = Quat::from_xyzw(0.5, 0.0, 0.0, 0.866_025_4);
        let expected = [
            (
                Chirality::Left,
                Vec3::new(-0.992_403_88, -0.078_267_24, -0.094_914_62),
            ),
            (
                Chirality::Right,
                Vec3::new(0.992_403_88, 0.078_267_24, 0.094_914_62),
            ),
        ];
        for (side, expected_normal) in expected {
            let frame = resolve_controller_frame(
                ActiveControllerProfile::Touch,
                side,
                Some((Vec3::ZERO, grip_rotation)),
            )
            .expect("frame");
            let palm_normal = (frame.rotation * frame.hand_rotation) * Vec3::Y;
            assert_vec3_near(palm_normal, expected_normal);
            let inward = match side {
                Chirality::Left => -1.0,
                Chirality::Right => 1.0,
            };
            assert!(
                palm_normal.x * inward > 0.9,
                "{side:?}: wrist palm normal {palm_normal:?} should point inward",
            );
        }
    }

    #[test]
    fn non_touch_profiles_skip_touch_correction() {
        let grip_position = Vec3::new(0.3, 1.2, -0.5);
        let grip_rotation = Quat::from_rotation_x(0.25).normalize();
        for profile in [
            ActiveControllerProfile::Index,
            ActiveControllerProfile::ViveFocus3,
            ActiveControllerProfile::Vive,
            ActiveControllerProfile::Generic,
        ] {
            let frame = resolve_controller_frame(
                profile,
                Chirality::Right,
                Some((grip_position, grip_rotation)),
            )
            .unwrap_or_else(|| panic!("frame for {profile:?}"));
            let (expected_position, expected_rotation) =
                openxr_grip_to_steamvr_raw(profile, Chirality::Right, grip_position, grip_rotation);
            assert_vec3_near(frame.position, expected_position);
            assert_quat_near(frame.rotation, expected_rotation);
        }
    }

    #[test]
    fn identity_offset_profiles_pass_grip_through() {
        let grip_position = Vec3::new(0.3, 1.2, -0.5);
        let grip_rotation = Quat::from_rotation_x(0.25).normalize();
        for profile in [
            ActiveControllerProfile::Vive,
            ActiveControllerProfile::WindowsMr,
            ActiveControllerProfile::HpReverbG2,
            ActiveControllerProfile::Pico4,
            ActiveControllerProfile::PicoNeo3,
            ActiveControllerProfile::ViveCosmos,
            ActiveControllerProfile::ViveFocus3,
            ActiveControllerProfile::Generic,
            ActiveControllerProfile::Simple,
        ] {
            let frame = resolve_controller_frame(
                profile,
                Chirality::Right,
                Some((grip_position, grip_rotation)),
            )
            .unwrap_or_else(|| panic!("frame for {profile:?}"));
            let (expected_position, expected_rotation) =
                openxr_tracking_pose_to_host(grip_position, grip_rotation);
            assert_vec3_near(frame.position, expected_position);
            assert_quat_near(frame.rotation, expected_rotation);
        }
    }

    #[test]
    fn raw_corrected_profiles_shift_grip_pose() {
        let grip_position = Vec3::new(0.3, 1.2, -0.5);
        let grip_rotation = Quat::from_rotation_x(0.25).normalize();
        let (host_grip_position, host_grip_rotation) =
            openxr_tracking_pose_to_host(grip_position, grip_rotation);
        for profile in [
            ActiveControllerProfile::Index,
            ActiveControllerProfile::Touch,
        ] {
            let frame = resolve_controller_frame(
                profile,
                Chirality::Right,
                Some((grip_position, grip_rotation)),
            )
            .unwrap_or_else(|| panic!("frame for {profile:?}"));
            assert!(
                (frame.position - host_grip_position).length() > 0.05,
                "{profile:?}: expected non-trivial raw-pose position correction",
            );
            assert!(
                rotation_delta_angle(frame.rotation, host_grip_rotation) > 0.2,
                "{profile:?}: expected non-trivial raw-pose rotation correction",
            );
        }
    }

    #[test]
    fn bound_hand_offsets_do_not_change_controller_pose() {
        let grip_position = Vec3::new(0.3, 1.2, -0.5);
        let grip_rotation = Quat::from_rotation_x(0.25).normalize();
        let frame = resolve_controller_frame(
            ActiveControllerProfile::Generic,
            Chirality::Left,
            Some((grip_position, grip_rotation)),
        )
        .expect("frame");
        let (expected_position, expected_rotation) =
            openxr_tracking_pose_to_host(grip_position, grip_rotation);

        assert_vec3_near(frame.position, expected_position);
        assert_quat_near(frame.rotation, expected_rotation);
        assert!(frame.hand_position.length() > 0.01);
        assert!(rotation_delta_angle(frame.hand_rotation, Quat::IDENTITY) > 0.2);
    }
}
