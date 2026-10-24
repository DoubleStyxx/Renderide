//! Reverse-Z view depth and the screen-motion pad used by history confirmation.

use glam::{Mat4, Vec3};

use crate::hi_z_cpu::project_world_aabb_uv;

use super::{
    DepthClip, OTHER_OCCLUDER_MIN_METERS, OTHER_OCCLUDER_RELATIVE, PARALLAX_RESAMPLE_MIN,
    PARALLAX_SCALE_MAX, SpaceMotion,
};

/// Linear view-space depth for a reverse-Z NDC z, using the projection's near and far.
///
/// NDC z is the renderer's reverse-Z interval `[0, 1]`: `1` at the near plane and `0` at the far
/// plane, matching [`crate::camera::reverse_z_perspective`]. Perspective uses
/// `far * near / (ndc_z * (far - near) + near)`. Orthographic uses
/// `far - ndc_z * (far - near)`. `None` when the clip or the depth is not usable.
pub(crate) fn view_depth_from_reverse_z(ndc_z: f32, clip: DepthClip) -> Option<f32> {
    if !ndc_z.is_finite() || !clip.near.is_finite() || !clip.far.is_finite() {
        return None;
    }
    if clip.far <= clip.near || clip.near <= 0.0 {
        return None;
    }
    let depth = if clip.orthographic {
        clip.far - ndc_z * (clip.far - clip.near)
    } else {
        let denom = ndc_z * (clip.far - clip.near) + clip.near;
        if denom.abs() <= f32::EPSILON {
            return None;
        }
        (clip.far * clip.near) / denom
    };
    (depth.is_finite() && depth > 0.0).then_some(depth)
}

/// View-space gap required before `covering` counts as a different occluder than the caster.
pub(crate) fn other_occluder_margin(caster_view_depth: f32) -> f32 {
    OTHER_OCCLUDER_MIN_METERS.max(OTHER_OCCLUDER_RELATIVE * caster_view_depth)
}

/// `true` when `covering_ndc_z` is closer than the caster's own front by [`other_occluder_margin`].
///
/// The comparison is in linear view depth. An invalid clip does not count as another occluder, so
/// a visible caster is not newly marked occluded.
pub(crate) fn covering_depth_is_other_occluder(
    closest_ndc_z: f32,
    covering_ndc_z: f32,
    clip: DepthClip,
) -> bool {
    let Some(caster) = view_depth_from_reverse_z(closest_ndc_z, clip) else {
        return false;
    };
    let Some(covering) = view_depth_from_reverse_z(covering_ndc_z, clip) else {
        return false;
    };
    covering < caster && caster - covering > other_occluder_margin(caster)
}

/// Chebyshev UV expansion from reprojecting one AABB through two view-projections.
///
/// `None` when a projection fails, which includes a box that left the frustum. Identical
/// cameras still project once: a box that straddles the near plane fails that project and must
/// not cull, and a successful project is a pad of zero.
pub(crate) fn screen_motion_uv_pad(
    previous_vp: Mat4,
    current_vp: Mat4,
    aabb_min: Vec3,
    aabb_max: Vec3,
) -> Option<f32> {
    if previous_vp == current_vp {
        project_world_aabb_uv(current_vp, aabb_min, aabb_max)?;
        return Some(0.0);
    }
    let previous = project_world_aabb_uv(previous_vp, aabb_min, aabb_max)?;
    let current = project_world_aabb_uv(current_vp, aabb_min, aabb_max)?;
    let du = (previous.0.0 - current.0.0)
        .abs()
        .max((previous.1.0 - current.1.0).abs());
    let dv = (previous.0.1 - current.0.1)
        .abs()
        .max((previous.1.1 - current.1.1).abs());
    let pad = du.max(dv);
    pad.is_finite().then_some(pad)
}

/// UV expansion for one space's cached camera pair.
///
/// `None` when this space has no comparable cameras or a projection fails. Callers then keep
/// the draw.
pub(crate) fn space_motion_pad(motion: &SpaceMotion, min: Vec3, max: Vec3) -> Option<f32> {
    if motion.stereo.is_some() {
        stereo_motion_pad(motion, min, max)
    } else {
        mono_motion_pad(motion, min, max)
    }
}

/// UV pad after widening `base_pad` for a closer occluder's parallax.
///
/// `Some(base_pad)` when the sample is not occluded, or the closer-surface scale is below
/// [`PARALLAX_RESAMPLE_MIN`]. `Some(base_pad * scale)` when the caster is between that minimum
/// and [`PARALLAX_SCALE_MAX`] times farther than the covering surface. `None` when the sample is
/// occluded and the covering depth is missing, not closer, or farther than the caster by more
/// than [`PARALLAX_SCALE_MAX`]. Callers treat `None` as "do not cull".
///
/// `covering_ndc_z` is the farthest depth in the rectangle. A nearer thin occluder in the same
/// rectangle is not visible here, so a scale that looks modest can still under-pad that surface
/// for one frame of camera motion.
pub(crate) fn parallax_adjusted_pad(
    base_pad: f32,
    closest_ndc_z: f32,
    covering_ndc_z: Option<f32>,
    occluded: bool,
    clip: DepthClip,
) -> Option<f32> {
    if !occluded {
        return Some(base_pad);
    }
    let covering_ndc_z = covering_ndc_z?;
    let caster = view_depth_from_reverse_z(closest_ndc_z, clip)?;
    let covering = view_depth_from_reverse_z(covering_ndc_z, clip)?;
    if !covering.is_finite() || covering <= 0.0 || covering >= caster {
        return None;
    }
    let scale = caster / covering;
    if !scale.is_finite() || scale > PARALLAX_SCALE_MAX {
        return None;
    }
    if scale < PARALLAX_RESAMPLE_MIN {
        return Some(base_pad);
    }
    let scaled = base_pad * scale;
    scaled.is_finite().then_some(scaled)
}

fn mono_motion_pad(motion: &SpaceMotion, min: Vec3, max: Vec3) -> Option<f32> {
    let (previous, current) = motion.mono?;
    screen_motion_uv_pad(previous, current, min, max)
}

fn stereo_motion_pad(motion: &SpaceMotion, min: Vec3, max: Vec3) -> Option<f32> {
    let ((previous_left, previous_right), (left, right)) = motion.stereo?;
    let left = screen_motion_uv_pad(previous_left, left, min, max)?;
    let right = screen_motion_uv_pad(previous_right, right, min, max)?;
    Some(left.max(right))
}
