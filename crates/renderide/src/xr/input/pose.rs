//! Controller grip pose math and OpenXR [`openxr::SpaceLocation`] conversion.
//!
//! Values follow the host-side pose convention expected by the `SteamVRDriver`-shaped
//! controller data path. The host (`VR_Manager`) writes the received `position` / `rotation`
//! straight into `RawPosition` / `RawRotation`, so the renderer is responsible for delivering
//! poses in the OpenVR raw device frame that path was authored against.
//!
//! `XR_EXT_palm_pose` is a different frame (palm center, palm normal as +Y). Reporting it as the
//! controller pose moves `LeftController` / `RightController` off Unity's node and, on runtimes
//! that do not supply a spec palm pose, rolls the bound wrist and the laser with it.

use glam::{EulerRot, Quat, Vec3};
use openxr as xr;

use crate::shared::Chirality;
use crate::xr::session::openxr_tracking_pose_to_host;

use super::profile::ActiveControllerProfile;

/// Builds a quaternion with the same Y-X-Z composition order Unity's `Quaternion.Euler` uses,
/// so per-profile rotation constants align with the host controller calibration data.
pub(super) fn unity_euler_deg(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_rotation_y(y.to_radians())
        * Quat::from_rotation_x(x.to_radians())
        * Quat::from_rotation_z(z.to_radians())
}

/// SteamVR rendermodel `component_local` of the grip, expressed in the raw device frame.
///
/// The name `grip_from_raw` is that component: `grip = raw * grip_from_raw`. `rotation` /
/// `translation` are `rotate_xyz` / `origin` from the rendermodel JSON (`origin` is the grip's
/// position in the raw frame). [`RawFromGripTransform`] stores the inverse, which maps a tracked
/// grip pose back onto the raw frame.
#[derive(Clone, Copy)]
struct GripInRaw {
    rotation: Quat,
    translation: Vec3,
}

/// Local OpenXR-space transform from a standardized grip pose back to the SteamVR raw pose.
///
/// This is the inverse of [`GripInRaw`]. `apply` does `position + rotation * translation` and
/// `rotation * rotation`, i.e. `raw = grip * raw_from_grip`.
#[derive(Clone, Copy)]
pub(super) struct RawFromGripTransform {
    rotation: Quat,
    translation: Vec3,
}

impl RawFromGripTransform {
    fn identity() -> Self {
        Self {
            rotation: Quat::IDENTITY,
            translation: Vec3::ZERO,
        }
    }

    fn from_grip_in_raw(grip_in_raw: GripInRaw) -> Self {
        let rotation = grip_in_raw.rotation.normalize();
        let raw_from_grip_rotation = rotation.inverse();
        Self {
            rotation: raw_from_grip_rotation,
            translation: raw_from_grip_rotation * -grip_in_raw.translation,
        }
    }

    fn apply(self, position: Vec3, rotation: Quat) -> (Vec3, Quat) {
        (
            position + rotation * self.translation,
            (rotation * self.rotation).normalize(),
        )
    }
}

/// Host-space pitch from the in-repo port of decompiled `SteamVRDriver.UpdateController`.
///
/// A public Valve or Resonite source for this angle was not found. It is not the 39.4° aim
/// component below. Applied only for [`ActiveControllerProfile::Touch`], after grip→raw and the
/// host basis change, as a post-multiply `rotation * RotX(pitch)`.
const TOUCH_UPDATE_CONTROLLER_PITCH_DEG: f32 = 45.0;

/// Quest 2 `openxr_grip` pitch, degrees, from the component Facebook gave Valve.
///
/// Valve quoted it in <https://steamcommunity.com/app/250820/discussions/8/3041607080045210902/>:
/// `rotate_xyz: [20.6, 0, 0]`, `origin: [0.007, -0.00182941, 0.1019482]`, next to aim
/// `rotate_xyz: [-39.4, 0, 0]`. Y and Z are zero, so Euler order does not change this pitch.
const TOUCH_GRIP_PITCH_DEG: f32 = 20.6;

/// Published Quest 2 grip-origin X. The Steam post shows one component, `x = +0.007`, not a
/// left/right pair. Left keeps that sign. Right negates it. The same post says Quest 2 aim and
/// grip share X ("no difference in the x-axis from grip"), and a measurement there has grip-in-aim
/// X = 0 on both hands, which holds when aim is mirrored with the grip. This code does not apply
/// an aim transform.
const TOUCH_GRIP_ORIGIN_X: f32 = 0.007;

/// Index `openxr_grip` `rotate_xyz` degrees for the left controller. Right negates Y and Z.
/// Origin for both hands is `(0, -0.015, 0.13)`.
///
/// These numbers have been in this repo since `34a0ae1c`. A public SteamVR rendermodel JSON
/// containing 15.392 / ∓2.071 / ±0.303 was not found; do not treat them as a cited Valve file.
///
/// `rotate_xyz` is applied as glam [`EulerRot::XYZ`], intrinsic X then Y then Z
/// (`Rx * Ry * Rz`). The OpenVR render-model reference describes `rotate_xyz` as the rotation
/// from the raw frame to the component and does not give a composition order
/// (<https://github.com/ValveSoftware/openvr/wiki/Render-Model-Reference>). Joe Ludwig, on OpenVR
/// issue 1446, says the API only needs the matrix and Euler order is the application's choice
/// (<https://github.com/ValveSoftware/openvr/issues/1446>). For Index, another order can move the
/// pose by about half a degree. That difference is not resolved here.
const INDEX_GRIP_ROTATE_XYZ_LEFT_DEG: (f32, f32, f32) = (15.392, -2.071, 0.303);

fn index_grip_in_raw(side: Chirality) -> GripInRaw {
    let (pitch, yaw, roll) = INDEX_GRIP_ROTATE_XYZ_LEFT_DEG;
    let (yaw, roll) = match side {
        Chirality::Left => (yaw, roll),
        Chirality::Right => (-yaw, -roll),
    };
    GripInRaw {
        rotation: Quat::from_euler(
            EulerRot::XYZ,
            pitch.to_radians(),
            yaw.to_radians(),
            roll.to_radians(),
        ),
        translation: Vec3::new(0.0, -0.015, 0.13),
    }
}

fn touch_grip_in_raw(side: Chirality) -> GripInRaw {
    let origin_x = match side {
        Chirality::Left => TOUCH_GRIP_ORIGIN_X,
        Chirality::Right => -TOUCH_GRIP_ORIGIN_X,
    };
    GripInRaw {
        rotation: Quat::from_euler(EulerRot::XYZ, TOUCH_GRIP_PITCH_DEG.to_radians(), 0.0, 0.0),
        translation: Vec3::new(origin_x, -0.001_829_41, 0.101_948_2),
    }
}

/// Grip component in the raw frame, or `None` when the OpenXR grip is already the node we report.
///
/// Vive Focus 3 is `None` on purpose. No Focus 3 `openxr_grip` component was found, so it does not
/// reuse the Quest 2 origin / 20.6° pitch. The 45° [`touch_pose_correction`] stays Touch-only, and
/// the Focus 3 bound-hand offset stays identity so the wrist sits on that grip.
fn grip_in_raw(profile: ActiveControllerProfile, side: Chirality) -> Option<GripInRaw> {
    match profile {
        ActiveControllerProfile::Index => Some(index_grip_in_raw(side)),
        ActiveControllerProfile::Touch => Some(touch_grip_in_raw(side)),
        _ => None,
    }
}

/// Returns the local OpenXR-space transform from grip pose to SteamVR raw controller pose.
pub(super) fn steamvr_raw_from_openxr_grip(
    profile: ActiveControllerProfile,
    side: Chirality,
) -> RawFromGripTransform {
    grip_in_raw(profile, side).map_or_else(RawFromGripTransform::identity, |grip| {
        RawFromGripTransform::from_grip_in_raw(grip)
    })
}

/// Converts an OpenXR grip pose into the SteamVR raw controller pose expected by host IPC.
///
/// OpenXR `/input/grip/pose` is a standardized hand grip frame. SteamVR raw poses are device
/// model frames, so controller models where those frames differ need a fixed local transform
/// before host-specific controller corrections are applied.
///
/// Calibration is composed in OpenXR right-handed tracking space first. The calibrated raw pose is
/// then converted into the host left-handed basis so controller frames and HMD frames enter
/// FrooxEngine in the same tracking space.
pub(super) fn openxr_grip_to_steamvr_raw(
    profile: ActiveControllerProfile,
    side: Chirality,
    position: Vec3,
    rotation: Quat,
) -> (Vec3, Quat) {
    let (raw_position, raw_rotation) =
        steamvr_raw_from_openxr_grip(profile, side).apply(position, rotation);
    openxr_tracking_pose_to_host(raw_position, raw_rotation)
}

/// Host-local nudge from the same `UpdateController` port as [`TOUCH_UPDATE_CONTROLLER_PITCH_DEG`].
///
/// A public citation for `(±0.01, 0.04, 0.03)` was not found. Left X is `-0.01`, right X is
/// `+0.01`. It is subtracted after the pitch (`position - rotation * nudge`), so the nudge is in
/// the already-pitched node frame, not the pre-pitch raw frame.
fn touch_update_controller_nudge(side: Chirality) -> Vec3 {
    match side {
        Chirality::Left => Vec3::new(-0.01, 0.04, 0.03),
        Chirality::Right => Vec3::new(0.01, 0.04, 0.03),
    }
}

/// Touch-only correction from `SteamVRDriver.UpdateController`, applied after the OpenXR grip has
/// already been converted to the SteamVR raw device pose and into the host basis.
///
/// Order, in host space: post-multiply [`TOUCH_UPDATE_CONTROLLER_PITCH_DEG`] about local X, then
/// subtract [`touch_update_controller_nudge`] in that pitched frame. Other profiles get neither.
///
/// This pitch is not the grip-to-raw conversion ([`steamvr_raw_from_openxr_grip`]). On Touch the
/// raw pose itself sits 39.4° above a level OpenXR aim (Facebook's aim `rotate_xyz` of `-39.4`
/// about X, same Steam post as [`TOUCH_GRIP_PITCH_DEG`]). After this 45° pitch the node is 5.6°
/// below that aim.
pub(super) fn touch_pose_correction(
    side: Chirality,
    position: Vec3,
    rotation: Quat,
) -> (Vec3, Quat) {
    let rotation = rotation * Quat::from_rotation_x(TOUCH_UPDATE_CONTROLLER_PITCH_DEG.to_radians());
    let offset = touch_update_controller_nudge(side);
    (position - rotation * offset, rotation)
}

/// Default `hand_position` / `hand_rotation` on the IPC controller state types in
/// [`crate::shared`] for bound-hand tracking (FrooxEngine `BodyNodePositionOffset` /
/// `BodyNodeRotationOffset` on the hand device).
///
/// The host does not hardcode these: `VR_Manager` forwards IPC `handPosition` / `handRotation` into
/// `MappableTrackedObject.Initialize` at registration.
///
/// `generic_fix = unity_euler_deg(90, 90, 90).inverse()` reproduces Unity's post-multiply
/// `Quaternion.Inverse(Quaternion.Euler(90,90,90))`, applied only to Vive / Touch / Generic in
/// that driver.
///
/// `hasBoundHand` is returned `true` for every profile. The host can otherwise prefer skeletal
/// hand tracking for some profiles, but the renderer has no skeletal hand tracking, so the
/// bound-hand defaults are always surfaced.
///
/// Index `InitHandData` in the `ce613392` port is identity relative to the grip, and no public
/// dump with a non-zero Index palm-to-wrist was found, so there is no extra palm-to-wrist term.
/// The IPC node is now the raw pose (~13 cm ahead of the grip), so the Index offset is the
/// grip-in-raw component converted into the host basis. The synthetic wrist
/// (`controller.position + controller.rotation * hand_position`) then lands on the grip.
///
/// Vive Focus 3 has no sourced grip component (see [`grip_in_raw`]), so its node is the OpenXR
/// grip and the identity wrist stays on that grip. Vive Cosmos is the same: no grip component,
/// identity wrist. Neither is a calibrated palm-to-wrist.
pub(super) fn bound_hand_pose_defaults(
    profile: ActiveControllerProfile,
    side: Chirality,
) -> (bool, Vec3, Quat) {
    let generic_fix = unity_euler_deg(90.0, 90.0, 90.0).inverse();
    let (position, rotation) = match (profile, side) {
        (ActiveControllerProfile::Touch, Chirality::Left) => (
            Vec3::new(-0.04, -0.025, -0.1),
            unity_euler_deg(185.0, -95.0, -90.0) * generic_fix,
        ),
        (ActiveControllerProfile::Touch, Chirality::Right) => (
            Vec3::new(0.04, -0.025, -0.1),
            unity_euler_deg(5.0, -95.0, -90.0) * generic_fix,
        ),
        (
            ActiveControllerProfile::Vive
            | ActiveControllerProfile::Generic
            | ActiveControllerProfile::Simple,
            Chirality::Left,
        ) => (
            Vec3::new(-0.02, 0.0, -0.16),
            unity_euler_deg(140.0, -90.0, -90.0) * generic_fix,
        ),
        (
            ActiveControllerProfile::Vive
            | ActiveControllerProfile::Generic
            | ActiveControllerProfile::Simple,
            Chirality::Right,
        ) => (
            Vec3::new(0.02, 0.0, -0.16),
            unity_euler_deg(40.0, -90.0, -90.0) * generic_fix,
        ),
        (
            ActiveControllerProfile::WindowsMr
            | ActiveControllerProfile::HpReverbG2
            | ActiveControllerProfile::Pico4
            | ActiveControllerProfile::PicoNeo3,
            Chirality::Left,
        ) => (
            Vec3::new(-0.028, 0.0, -0.18),
            unity_euler_deg(30.0, 5.0, 100.0),
        ),
        (
            ActiveControllerProfile::WindowsMr
            | ActiveControllerProfile::HpReverbG2
            | ActiveControllerProfile::Pico4
            | ActiveControllerProfile::PicoNeo3,
            Chirality::Right,
        ) => (
            Vec3::new(0.028, 0.0, -0.18),
            unity_euler_deg(30.0, -5.0, -100.0),
        ),
        (ActiveControllerProfile::Index, side) => {
            let grip = index_grip_in_raw(side);
            openxr_tracking_pose_to_host(grip.translation, grip.rotation)
        }
        (ActiveControllerProfile::ViveCosmos | ActiveControllerProfile::ViveFocus3, _) => {
            (Vec3::ZERO, Quat::IDENTITY)
        }
    };
    (true, position, rotation)
}

/// Converts an [`xr::SpaceLocation`] into OpenXR tracking-space `(position, rotation)`.
///
/// Returns `None` when either position or orientation is invalid. Callers then report no
/// controller frame for that hand.
pub(super) fn pose_from_location(location: &xr::SpaceLocation) -> Option<(Vec3, Quat)> {
    let tracked = location
        .location_flags
        .contains(xr::SpaceLocationFlags::ORIENTATION_VALID)
        && location
            .location_flags
            .contains(xr::SpaceLocationFlags::POSITION_VALID);
    tracked.then(|| {
        let pose = &location.pose;
        let position = Vec3::new(pose.position.x, pose.position.y, pose.position.z);
        let orientation = pose.orientation;
        let rotation = Quat::from_xyzw(orientation.x, orientation.y, orientation.z, orientation.w);
        let len_sq = rotation.length_squared();
        let rotation = if len_sq.is_finite() && len_sq >= 1e-10 {
            rotation.normalize()
        } else {
            Quat::IDENTITY
        };
        (position, rotation)
    })
}

/// Converts an [`xr::HandJointLocation`] into OpenXR tracking-space `(position, rotation)`.
///
/// Returns `None` when either position or orientation is invalid.
pub(super) fn pose_from_joint(location: &xr::HandJointLocation) -> Option<(Vec3, Quat)> {
    let tracked = location
        .location_flags
        .contains(xr::SpaceLocationFlags::ORIENTATION_VALID)
        && location
            .location_flags
            .contains(xr::SpaceLocationFlags::POSITION_VALID);
    tracked.then(|| {
        let pose = &location.pose;
        let position = Vec3::new(pose.position.x, pose.position.y, pose.position.z);
        let orientation = pose.orientation;
        let rotation = Quat::from_xyzw(orientation.x, orientation.y, orientation.z, orientation.w);
        let len_sq = rotation.length_squared();
        let rotation = if len_sq.is_finite() && len_sq >= 1e-10 {
            rotation.normalize()
        } else {
            Quat::IDENTITY
        };
        (position, rotation)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::xr::session::openxr_tracking_pose_to_host;

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

    fn rotation_angle_from_identity(rotation: Quat) -> f32 {
        2.0 * rotation.normalize().w.abs().clamp(-1.0, 1.0).acos()
    }

    /// Right Index, identity OpenXR grip. Expected pose is intrinsic XYZ
    /// (`Rx * Ry * Rz`) of `(15.392°, +2.071°, -0.303°)`, inverted, then the host map
    /// `(x, y, z) -> (x, y, -z)` and `(qx, qy, qz, qw) -> (-qx, -qy, qz, qw)`.
    ///
    /// The literals were computed outside this crate. They are not the result of calling
    /// [`openxr_grip_to_steamvr_raw`] or [`Quat::from_euler`] with the grip constants.
    /// Another Euler order moves this quaternion by about half a degree. The OpenVR wiki does not
    /// specify `rotate_xyz` composition; this golden assumes intrinsic XYZ.
    #[test]
    fn index_right_identity_grip_matches_host_space_rotation_golden() {
        let (position, rotation) = openxr_grip_to_steamvr_raw(
            ActiveControllerProfile::Index,
            Chirality::Right,
            Vec3::ZERO,
            Quat::IDENTITY,
        );
        assert_vec3_near(
            position,
            Vec3::new(0.004_779_23, -0.020_017_81, 0.129_234_07),
        );
        assert_quat_near(
            rotation,
            Quat::from_xyzw(0.133_847_3, 0.018_263_1, 0.000_199_8, 0.990_833_7),
        );
        assert_vec3_near(
            rotation * Vec3::Z,
            Vec3::new(0.036_244_87, -0.265_233_5, 0.963_502_7),
        );

        let (_, hand_position, hand_rotation) =
            bound_hand_pose_defaults(ActiveControllerProfile::Index, Chirality::Right);
        assert_vec3_near(hand_position, Vec3::new(0.0, -0.015, -0.13));
        assert_quat_near(
            hand_rotation,
            Quat::from_xyzw(-0.133_847_3, -0.018_263_1, -0.000_199_8, 0.990_833_7),
        );
        let wrist_position = position + rotation * hand_position;
        let wrist_rotation = (rotation * hand_rotation).normalize();
        assert_vec3_near(wrist_position, Vec3::ZERO);
        assert_quat_near(wrist_rotation, Quat::IDENTITY);
    }

    #[test]
    fn identity_calibration_profiles_emit_openxr_grip_unchanged_before_host_conversion() {
        let grip_position = Vec3::new(0.25, 1.4, -0.35);
        let grip_rotation = (Quat::from_rotation_y(0.6) * Quat::from_rotation_x(-0.2)).normalize();

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
            for side in [Chirality::Left, Chirality::Right] {
                let (position, rotation) =
                    steamvr_raw_from_openxr_grip(profile, side).apply(grip_position, grip_rotation);
                assert_vec3_near(position, grip_position);
                assert_quat_near(rotation, grip_rotation);
            }
        }
    }

    #[test]
    fn identity_calibration_profiles_convert_result_to_host_tracking_space() {
        let grip_position = Vec3::new(0.25, 1.4, -0.35);
        let grip_rotation = (Quat::from_rotation_y(0.6) * Quat::from_rotation_x(-0.2)).normalize();
        let (expected_position, expected_rotation) =
            openxr_tracking_pose_to_host(grip_position, grip_rotation);

        let (position, rotation) = openxr_grip_to_steamvr_raw(
            ActiveControllerProfile::Vive,
            Chirality::Left,
            grip_position,
            grip_rotation,
        );

        assert_vec3_near(position, expected_position);
        assert_quat_near(rotation, expected_rotation);
    }

    #[test]
    fn raw_pose_offset_magnitudes_match_supported_profiles() {
        // Lengths are |raw_from_grip * -origin|. Touch's angle is the published 20.6° (Y and Z
        // are 0). Index's angle is the magnitude of intrinsic XYZ (15.392, ∓2.071, ±0.303),
        // which is 15.527°, not 15.392°, because the axes are not aligned.
        for (profile, expected_translation, expected_degrees) in [
            (ActiveControllerProfile::Index, 0.130_862_52, 15.527_38),
            (ActiveControllerProfile::Touch, 0.102_204_61, 20.6),
        ] {
            for side in [Chirality::Left, Chirality::Right] {
                let (position, rotation) =
                    steamvr_raw_from_openxr_grip(profile, side).apply(Vec3::ZERO, Quat::IDENTITY);
                let length_delta = (position.length() - expected_translation).abs();
                assert!(
                    length_delta < 1e-5,
                    "{profile:?} {side:?}: position offset {position:?} (delta {length_delta})",
                );
                let angle_degrees = rotation_angle_from_identity(rotation).to_degrees();
                assert!(
                    (angle_degrees - expected_degrees).abs() < 0.01,
                    "{profile:?} {side:?}: rotation angle {angle_degrees}°, expected {expected_degrees}°",
                );
            }
        }
    }

    #[test]
    fn raw_pose_offsets_mirror_across_hands() {
        for profile in [
            ActiveControllerProfile::Index,
            ActiveControllerProfile::Touch,
        ] {
            let (left_position, left_rotation) =
                steamvr_raw_from_openxr_grip(profile, Chirality::Left)
                    .apply(Vec3::ZERO, Quat::IDENTITY);
            let (right_position, right_rotation) =
                steamvr_raw_from_openxr_grip(profile, Chirality::Right)
                    .apply(Vec3::ZERO, Quat::IDENTITY);

            assert!(
                (left_position.x + right_position.x).abs() < 1e-4,
                "{profile:?}: position X should mirror: left={left_position:?}, right={right_position:?}",
            );
            assert!(
                (left_position.y - right_position.y).abs() < 1e-4,
                "{profile:?}: position Y should match: left={left_position:?}, right={right_position:?}",
            );
            assert!(
                (left_position.z - right_position.z).abs() < 1e-4,
                "{profile:?}: position Z should match: left={left_position:?}, right={right_position:?}",
            );
            let left_angle = rotation_angle_from_identity(left_rotation);
            let right_angle = rotation_angle_from_identity(right_rotation);
            assert!(
                (left_angle - right_angle).abs() < 1e-4,
                "{profile:?}: rotation angle should match: left={left_angle}, right={right_angle}",
            );
        }
    }

    #[test]
    fn pose_from_location_returns_openxr_tracking_pose() {
        let expected_rotation = Quat::from_rotation_y(0.35).normalize();
        let location = xr::SpaceLocation {
            location_flags: xr::SpaceLocationFlags::ORIENTATION_VALID
                | xr::SpaceLocationFlags::POSITION_VALID,
            pose: xr::Posef {
                orientation: xr::Quaternionf {
                    x: expected_rotation.x,
                    y: expected_rotation.y,
                    z: expected_rotation.z,
                    w: expected_rotation.w,
                },
                position: xr::Vector3f {
                    x: 1.0,
                    y: 2.0,
                    z: -3.0,
                },
            },
        };

        let (position, rotation) = pose_from_location(&location).expect("tracked pose");

        assert_vec3_near(position, Vec3::new(1.0, 2.0, -3.0));
        assert_quat_near(rotation, expected_rotation);
    }

    /// Bound-hand rotations for profiles Unity post-multiplies with `generic_fix` must have a
    /// palm normal pointing inward (+/-X toward the other hand), not forward or down. Guards
    /// against accidentally dropping `generic_fix`.
    #[test]
    fn neutral_grip_palm_faces_inward_not_forward_generic() {
        for (side, expected_x_sign) in [(Chirality::Left, -1.0_f32), (Chirality::Right, 1.0_f32)] {
            let (_, _, hand_rot) = bound_hand_pose_defaults(ActiveControllerProfile::Generic, side);
            let palm_normal = hand_rot * Vec3::Y;
            assert!(
                palm_normal.x * expected_x_sign > 0.5,
                "{side:?}: palm normal {palm_normal:?} should have significant inward X component \
                 (expected sign {expected_x_sign}), got X={}",
                palm_normal.x,
            );
            assert!(
                palm_normal.z.abs() < 0.5,
                "{side:?}: palm normal {palm_normal:?} should not point strongly forward/back",
            );
        }
    }

    #[test]
    fn neutral_grip_palm_faces_inward_not_forward_touch() {
        for (side, expected_x_sign) in [(Chirality::Left, -1.0_f32), (Chirality::Right, 1.0_f32)] {
            let (_, _, hand_rot) = bound_hand_pose_defaults(ActiveControllerProfile::Touch, side);
            let palm_normal = hand_rot * Vec3::Y;
            assert!(
                palm_normal.x * expected_x_sign > 0.3,
                "{side:?}: palm normal {palm_normal:?} should have inward X component \
                 (expected sign {expected_x_sign}), got X={}",
                palm_normal.x,
            );
        }
    }

    #[test]
    fn bound_hand_chirality_mirrors_x_component() {
        for profile in [
            ActiveControllerProfile::Generic,
            ActiveControllerProfile::Touch,
            ActiveControllerProfile::Vive,
            ActiveControllerProfile::WindowsMr,
            ActiveControllerProfile::HpReverbG2,
            ActiveControllerProfile::Pico4,
            ActiveControllerProfile::PicoNeo3,
        ] {
            let (_, pos_l, rot_l) = bound_hand_pose_defaults(profile, Chirality::Left);
            let (_, pos_r, rot_r) = bound_hand_pose_defaults(profile, Chirality::Right);
            assert!(
                (pos_l.x + pos_r.x).abs() < 1e-4,
                "{profile:?}: position X should be mirrored: left={}, right={}",
                pos_l.x,
                pos_r.x,
            );
            let palm_l = rot_l * Vec3::Y;
            let palm_r = rot_r * Vec3::Y;
            assert!(
                (palm_l.x + palm_r.x).abs() < 0.15,
                "{profile:?}: palm normal X should be approximately mirrored: left={palm_l:?}, right={palm_r:?}",
            );
        }
    }

    /// Cosmos and Focus 3 have no sourced grip component, so the node is the OpenXR grip and the
    /// wrist offset stays identity. Index is not in this list: its node is the raw pose, and the
    /// wrist offset is the host-space grip component (see
    /// `index_right_identity_grip_matches_host_space_rotation_golden`).
    #[test]
    fn identity_profiles_have_zero_offset_and_identity_rotation() {
        for profile in [
            ActiveControllerProfile::ViveCosmos,
            ActiveControllerProfile::ViveFocus3,
        ] {
            for side in [Chirality::Left, Chirality::Right] {
                let (has, pos, rot) = bound_hand_pose_defaults(profile, side);
                assert!(has, "{profile:?} {side:?}: has_bound_hand must be true");
                assert!(
                    pos.length() < 1e-6,
                    "{profile:?} {side:?}: expected zero position, got {pos:?}",
                );
                let dot = rot.normalize().dot(Quat::IDENTITY).abs();
                assert!(
                    (1.0 - dot) < 1e-6,
                    "{profile:?} {side:?}: expected identity rotation, got {rot:?}",
                );
            }
        }
    }

    /// Pico4 / PicoNeo3 / HpReverbG2 share the Windows MR bound-hand values.
    #[test]
    fn pico_and_reverb_share_windowsmr_defaults() {
        let reference = [
            bound_hand_pose_defaults(ActiveControllerProfile::WindowsMr, Chirality::Left),
            bound_hand_pose_defaults(ActiveControllerProfile::WindowsMr, Chirality::Right),
        ];
        for profile in [
            ActiveControllerProfile::HpReverbG2,
            ActiveControllerProfile::Pico4,
            ActiveControllerProfile::PicoNeo3,
        ] {
            for (side, expected) in [
                (Chirality::Left, reference[0]),
                (Chirality::Right, reference[1]),
            ] {
                let got = bound_hand_pose_defaults(profile, side);
                assert_eq!(got.0, expected.0, "{profile:?} {side:?}");
                assert!(
                    (got.1 - expected.1).length() < 1e-6,
                    "{profile:?} {side:?}: position {:?} vs {:?}",
                    got.1,
                    expected.1,
                );
                let dot = got.2.normalize().dot(expected.2.normalize()).abs();
                assert!(
                    (1.0 - dot) < 1e-6,
                    "{profile:?} {side:?}: rotation {:?} vs {:?}",
                    got.2,
                    expected.2,
                );
            }
        }
    }
}
