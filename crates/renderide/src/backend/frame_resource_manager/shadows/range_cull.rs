//! Conservative light-influence rejection before per-face shadow frustum tests.
//!
//! Point and spot faces only need casters that can intersect that light's shadow frustum. Testing
//! every world caster against all six cube faces is the dominant cost when many local lights are
//! shadowed.

use glam::Vec3;

use crate::gpu::GpuLight;
use crate::shared::LightType;
use crate::world_mesh::WorldMeshDrawItem;

use super::{light_type_u32, shadow_aabb_safe_for_cull};

/// Fills `mask` for a local light, or reports that every caster must be tested.
///
/// `Some` means byte `0` is outside the light's expanded influence and can skip the face test.
/// Directional lights return `None` because their cascades are not range-bounded the same way.
pub(super) fn influence_range_mask<'a>(
    light: &GpuLight,
    draws: &[WorldMeshDrawItem],
    scratch: &'a mut Vec<u8>,
) -> Option<&'a [u8]> {
    let radius_sq = influence_radius_sq(light)?;
    scratch.clear();
    scratch.reserve(draws.len());
    let center = Vec3::from_array(light.position);
    for draw in draws {
        let inside = u8::from(draw_affects_light(draw, center, radius_sq));
        scratch.push(inside);
    }
    Some(scratch.as_slice())
}

/// Squared radius that covers the light's shadow frustum, when the light is local.
pub(super) fn influence_radius_sq(light: &GpuLight) -> Option<f32> {
    let near = light.shadow_near_plane.max(0.001);
    let far = light.range.max(near + 0.001);
    let far_sq = far * far;
    if light.light_type == light_type_u32(LightType::Point) {
        // 90-degree cube faces reach sqrt(3) * far at the far-plane corners.
        // 3.1 leaves a margin so clip-edge casters are not dropped.
        return Some(far_sq * 3.1);
    }
    if light.light_type == light_type_u32(LightType::Spot) {
        let cos_half = light.spot_cos_half_angle.clamp(0.001, 1.0);
        let tan_half_sq = (1.0 - cos_half * cos_half).max(0.0) / (cos_half * cos_half);
        let corner_sq = tan_half_sq.mul_add(2.0, 1.0);
        return Some(far_sq * corner_sq * 1.02);
    }
    None
}

fn draw_affects_light(draw: &WorldMeshDrawItem, center: Vec3, radius_sq: f32) -> bool {
    let Some((min, max)) = draw.world_aabb else {
        return true;
    };
    if !shadow_aabb_safe_for_cull(min, max) {
        return true;
    }
    aabb_intersects_sphere(min, max, center, radius_sq)
}

fn aabb_intersects_sphere(min: Vec3, max: Vec3, center: Vec3, radius_sq: f32) -> bool {
    let dx = axis_outside(center.x, min.x, max.x);
    let dy = axis_outside(center.y, min.y, max.y);
    let dz = axis_outside(center.z, min.z, max.z);
    dx.mul_add(dx, dy.mul_add(dy, dz * dz)) <= radius_sq
}

fn axis_outside(center: f32, min: f32, max: f32) -> f32 {
    if center < min {
        min - center
    } else if center > max {
        center - max
    } else {
        0.0
    }
}

/// Returns whether `draw_idx` was rejected by a local-light influence mask.
pub(super) fn shadow_draw_outside_influence(mask: Option<&[u8]>, draw_idx: usize) -> bool {
    mask.and_then(|mask| mask.get(draw_idx).copied()) == Some(0)
}
