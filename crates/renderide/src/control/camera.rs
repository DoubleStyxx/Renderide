//! Camera pose override and keyframe sampling.

#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

use glam::Quat;
use renderide_control_client::CameraPoseParams;

/// Active camera drive set by the control API.
#[derive(Clone, Debug)]
pub(crate) enum CameraDrive {
    /// Host camera is unchanged.
    Idle,
    /// A single pose applied every frame.
    Pose(CameraPoseParams),
    /// Keyframes spaced by `dt` seconds, starting when the command arrived.
    Path {
        /// Path samples.
        keyframes: Vec<CameraPoseParams>,
        /// Seconds between keyframes.
        dt: f32,
        /// Playback start.
        started: Instant,
    },
}

impl CameraDrive {
    /// Pose to apply this frame, if the drone is driving the camera.
    pub(crate) fn current(&self, now: Instant) -> Option<CameraPoseParams> {
        match self {
            Self::Idle => None,
            Self::Pose(pose) => Some(pose.clone()),
            Self::Path {
                keyframes,
                dt,
                started,
            } => sample_path(keyframes, *dt, elapsed_secs(*started, now)),
        }
    }
}

/// Validates a single pose.
pub(crate) fn validate_pose(pose: &CameraPoseParams) -> Result<(), String> {
    if !pose.pos.iter().all(|value| value.is_finite()) {
        return Err("camera pos must be finite".to_owned());
    }
    if !pose.rot.iter().all(|value| value.is_finite()) {
        return Err("camera rot must be finite".to_owned());
    }
    let length_sq = pose.rot.iter().map(|value| value * value).sum::<f32>();
    if length_sq <= 1.0e-8 {
        return Err("camera rot must be a non-zero quaternion".to_owned());
    }
    if !pose.fov.is_finite() || pose.fov <= 0.0 || pose.fov >= 180.0 {
        return Err("camera fov must be in (0, 180) degrees".to_owned());
    }
    Ok(())
}

/// Validates path playback arguments.
pub(crate) fn validate_path(keyframes: &[CameraPoseParams], dt: f32) -> Result<(), String> {
    if keyframes.is_empty() {
        return Err("camera path requires at least one keyframe".to_owned());
    }
    if !dt.is_finite() || dt <= 0.0 {
        return Err("camera path dt must be a positive finite number of seconds".to_owned());
    }
    for pose in keyframes {
        validate_pose(pose)?;
    }
    Ok(())
}

/// Samples `keyframes` at `elapsed_secs`. Keyframe `i` sits at time `i * dt`.
pub(crate) fn sample_path(
    keyframes: &[CameraPoseParams],
    dt: f32,
    elapsed_secs: f32,
) -> Option<CameraPoseParams> {
    let first = keyframes.first()?.clone();
    if keyframes.len() == 1 || dt <= 0.0 {
        return Some(first);
    }
    let last_index = keyframes.len() - 1;
    let max_t = dt * last_index as f32;
    let t = elapsed_secs.clamp(0.0, max_t);
    let scaled = t / dt;
    let index = (scaled.floor() as usize).min(last_index - 1);
    let frac = (scaled - index as f32).clamp(0.0, 1.0);
    Some(lerp_pose(&keyframes[index], &keyframes[index + 1], frac))
}

fn lerp_pose(from: &CameraPoseParams, to: &CameraPoseParams, frac: f32) -> CameraPoseParams {
    let start = Quat::from_xyzw(from.rot[0], from.rot[1], from.rot[2], from.rot[3]).normalize();
    let end = Quat::from_xyzw(to.rot[0], to.rot[1], to.rot[2], to.rot[3]).normalize();
    let rot = start.slerp(end, frac);
    CameraPoseParams {
        pos: [
            lerp(from.pos[0], to.pos[0], frac),
            lerp(from.pos[1], to.pos[1], frac),
            lerp(from.pos[2], to.pos[2], frac),
        ],
        rot: [rot.x, rot.y, rot.z, rot.w],
        fov: lerp(from.fov, to.fov, frac),
    }
}

fn lerp(from: f32, to: f32, frac: f32) -> f32 {
    from + (to - from) * frac
}

fn elapsed_secs(started: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(started).as_secs_f32()
}

/// Test helper so path sampling does not depend on wall-clock sleeps.
#[cfg(test)]
pub(crate) fn path_at(
    keyframes: &[CameraPoseParams],
    dt: f32,
    elapsed: Duration,
) -> Option<CameraPoseParams> {
    sample_path(keyframes, dt, elapsed.as_secs_f32())
}

#[cfg(test)]
mod tests {
    use super::{path_at, validate_path, validate_pose};
    use renderide_control_client::CameraPoseParams;
    use std::time::Duration;

    fn pose(x: f32, fov: f32) -> CameraPoseParams {
        CameraPoseParams {
            pos: [x, 0.0, 0.0],
            rot: [0.0, 0.0, 0.0, 1.0],
            fov,
        }
    }

    #[test]
    fn path_holds_endpoints() {
        let keys = vec![pose(0.0, 60.0), pose(10.0, 80.0)];
        let start = path_at(&keys, 1.0, Duration::from_secs(0)).expect("start");
        let end = path_at(&keys, 1.0, Duration::from_secs(5)).expect("end");
        assert_eq!(start.pos[0], 0.0);
        assert_eq!(end.pos[0], 10.0);
        assert_eq!(end.fov, 80.0);
    }

    #[test]
    fn path_midpoint_lerps_position_and_fov() {
        let keys = vec![pose(0.0, 60.0), pose(10.0, 80.0)];
        let mid = path_at(&keys, 2.0, Duration::from_secs(1)).expect("mid");
        assert!((mid.pos[0] - 5.0).abs() < 1.0e-4);
        assert!((mid.fov - 70.0).abs() < 1.0e-4);
    }

    #[test]
    fn rejects_empty_path_and_bad_pose() {
        assert!(validate_path(&[], 1.0).is_err());
        assert!(validate_pose(&pose(f32::NAN, 60.0)).is_err());
        assert!(validate_path(&[pose(0.0, 60.0)], 0.0).is_err());
    }
}
