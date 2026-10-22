//! Canvas-plane ordering and reverse-Z constant depth bias for UIX stems.
//!
//! Coplanar UI meshes of different sizes do not share a farthest-AABB-corner distance, so that
//! metric flips painter order as the camera moves. Canvas stems instead compare quantized
//! view-plane depth. A constant NDC bias, one D24 unit per Unity `Offset` unit, is baked into
//! the per-draw view-projection because wgpu's hardware depth-bias constant collapses on
//! `Depth32Float` reverse-Z away from the near plane.

use glam::{Mat4, Vec3};

use super::item::WorldMeshDrawItem;

/// View-plane separation below which two canvas elements are treated as coplanar.
///
/// Half a millimeter keeps face-on siblings in one bucket so hierarchy order wins, while real
/// gaps between canvases still sort back to front.
pub(crate) const CANVAS_PLANE_DEPTH_QUANTUM_METERS: f32 = 5.0e-4;

/// Maximum extra canvas steps applied on top of Unity `Offset` units.
///
/// Each step is one D24-sized NDC quantum toward the camera. The cap stops a long sibling list
/// from walking through a surface that is actually in front.
pub(crate) const CANVAS_DEPTH_STEP_CAP: u32 = 64;

/// NDC Z delta of one Unity `Offset` unit under 24-bit depth (`2^-24`).
///
/// Reverse-Z stores near as 1 and far as 0, so a positive value moves the primitive closer.
const REVERSE_Z_D24_NDC_UNIT: f32 = 1.0 / 16_777_216.0;

/// Quantizes a view-plane depth in meters to [`CANVAS_PLANE_DEPTH_QUANTUM_METERS`].
#[inline]
pub(crate) fn quantize_canvas_plane_depth(depth: f32) -> f32 {
    if !depth.is_finite() {
        return depth;
    }
    let quantum = f64::from(CANVAS_PLANE_DEPTH_QUANTUM_METERS);
    let steps = (f64::from(depth) / quantum).round();
    (steps * quantum) as f32
}

/// Integer quantum of a view-plane depth. `None` when `depth` is non-finite or out of `i32` range.
#[inline]
pub(crate) fn canvas_plane_depth_quantum(depth: f32) -> Option<i32> {
    if !depth.is_finite() {
        return None;
    }
    let steps = (f64::from(depth) / f64::from(CANVAS_PLANE_DEPTH_QUANTUM_METERS)).round();
    if !steps.is_finite() || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&steps) {
        return None;
    }
    Some(steps as i32)
}

/// View-plane depth of an AABB center along `forward`.
///
/// `forward` does not need to be unit length. A zero or non-finite forward, or unusable bounds,
/// returns `None` so the caller can fall back to farthest-corner distance.
pub(crate) fn canvas_plane_depth_meters(
    origin: Vec3,
    forward: Vec3,
    world_aabb: Option<(Vec3, Vec3)>,
) -> Option<f32> {
    if !origin.is_finite() || !forward.is_finite() {
        return None;
    }
    let length_sq = forward.length_squared();
    if length_sq <= 1.0e-12 {
        return None;
    }
    let (min, max) = world_aabb?;
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    let center = min.min(max).midpoint(max.max(min));
    let depth = (center - origin).dot(forward * length_sq.sqrt().recip());
    depth.is_finite().then_some(depth)
}

/// Farther-first depth key for one transparent draw.
///
/// Canvas stems with a finite plane depth use the quantized view-plane distance in meters.
/// Every other draw uses `sqrt(camera_distance_sq)` so non-UI order stays identical to comparing
/// farthest-corner distance squared, and a canvas-versus-world pair still has one total order.
/// A canvas stem with no plane depth falls back to that same distance so a missing view basis
/// does not pull every UI draw to the front.
#[inline]
pub(super) fn transparent_depth_sort_key(item: &WorldMeshDrawItem) -> f64 {
    if item.batch_key.pipeline.is_canvas_ui_stem()
        && let Some(depth) = item.canvas_plane_depth.filter(|depth| depth.is_finite())
    {
        return f64::from(quantize_canvas_plane_depth(depth));
    }
    non_canvas_depth_meters(item.camera_distance_sq)
}

/// Monotonic meter key for a farthest-corner distance squared.
#[inline]
fn non_canvas_depth_meters(distance_sq: f32) -> f64 {
    if distance_sq.is_finite() && distance_sq > 0.0 {
        f64::from(distance_sq).sqrt()
    } else {
        f64::from(distance_sq)
    }
}

/// Per-draw canvas step in submission order.
///
/// Only canvas stems advance the rank. The rank resets when the quantized plane depth changes
/// and is capped at [`CANVAS_DEPTH_STEP_CAP`]. Later draws in a coplanar run get a closer bias.
pub(crate) fn assign_canvas_depth_steps(draws: &[WorldMeshDrawItem]) -> Vec<u32> {
    let mut steps = vec![0u32; draws.len()];
    let mut rank = 0u32;
    let mut previous_quantum = None;
    let mut have_previous = false;
    for (index, draw) in draws.iter().enumerate() {
        if !draw.batch_key.pipeline.is_canvas_ui_stem() {
            continue;
        }
        let quantum = draw
            .canvas_plane_depth
            .filter(|depth| depth.is_finite())
            .and_then(canvas_plane_depth_quantum);
        if have_previous && previous_quantum == quantum {
            rank = rank.saturating_add(1);
        } else {
            rank = 0;
            previous_quantum = quantum;
            have_previous = true;
        }
        steps[index] = rank.min(CANVAS_DEPTH_STEP_CAP);
    }
    steps
}

/// Constant reverse-Z NDC bias for Unity `Offset` units plus canvas sibling steps.
///
/// Positive Unity units push away from the camera (negative NDC under reverse-Z). Each canvas
/// step pulls one D24 quantum closer. Steps above [`CANVAS_DEPTH_STEP_CAP`] clamp.
#[inline]
pub(crate) fn reverse_z_constant_ndc_bias(units: i32, canvas_steps: u32) -> f32 {
    let steps = canvas_steps.min(CANVAS_DEPTH_STEP_CAP) as f32;
    (-(units as f32) + steps) * REVERSE_Z_D24_NDC_UNIT
}

/// NDC Z bias to bake into one draw's view-projection. Non-canvas draws stay at zero.
#[inline]
pub(crate) fn canvas_ndc_z_bias(item: &WorldMeshDrawItem, canvas_steps: u32) -> f32 {
    if !item.batch_key.pipeline.is_canvas_ui_stem() {
        return 0.0;
    }
    let units = item
        .batch_key
        .render_state
        .depth_offset
        .map(|offset| offset.units())
        .unwrap_or(0);
    reverse_z_constant_ndc_bias(units, canvas_steps)
}

/// Left-multiplies a constant NDC Z translation into a view-projection matrix.
///
/// `clip = bias * view_proj * position` adds `ndc_bias * clip.w` to clip Z, which is a constant
/// NDC offset after the perspective divide. Zero bias returns `view_proj` unchanged.
#[inline]
pub(crate) fn apply_reverse_z_ndc_bias(view_proj: Mat4, ndc_bias: f32) -> Mat4 {
    if ndc_bias == 0.0 {
        return view_proj;
    }
    Mat4::from_translation(Vec3::new(0.0, 0.0, ndc_bias)) * view_proj
}

#[cfg(test)]
mod tests {
    use super::{
        CANVAS_DEPTH_STEP_CAP, CANVAS_PLANE_DEPTH_QUANTUM_METERS, apply_reverse_z_ndc_bias,
        assign_canvas_depth_steps, canvas_ndc_z_bias, canvas_plane_depth_meters,
        canvas_plane_depth_quantum, quantize_canvas_plane_depth, reverse_z_constant_ndc_bias,
        transparent_depth_sort_key,
    };
    use crate::materials::MaterialDepthOffsetState;
    use crate::materials::RasterPipelineKind;
    use crate::world_mesh::materials::compute_batch_key_hash;
    use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};
    use glam::{Mat4, Vec3, Vec4};
    use std::sync::Arc;

    fn canvas_draw(
        collect_order: usize,
        distance_sq: f32,
        plane_depth: Option<f32>,
    ) -> super::super::item::WorldMeshDrawItem {
        let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
            material_asset_id: 1,
            property_block: None,
            skinned: false,
            sorting_order: 0,
            mesh_asset_id: 1,
            node_id: collect_order as i32,
            slot_index: 0,
            collect_order,
            alpha_blended: true,
        });
        item.camera_distance_sq = distance_sq;
        item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from("ui_unlit_default"));
        item.batch_key_hash = compute_batch_key_hash(&item.batch_key);
        item.canvas_plane_depth = plane_depth;
        item
    }

    #[test]
    fn quantize_collapses_sub_millimeter_plane_gaps() {
        let base = quantize_canvas_plane_depth(1.0);
        assert_eq!(base, quantize_canvas_plane_depth(1.0 + 1.0e-4));
        assert_ne!(
            canvas_plane_depth_quantum(1.0),
            canvas_plane_depth_quantum(1.0 + 1.0e-3)
        );
        assert!((CANVAS_PLANE_DEPTH_QUANTUM_METERS - 5.0e-4).abs() < f32::EPSILON);
    }

    #[test]
    fn plane_depth_uses_aabb_center_not_farthest_corner() {
        let origin = Vec3::ZERO;
        let forward = Vec3::Z;
        let small = canvas_plane_depth_meters(
            origin,
            forward,
            Some((Vec3::new(-0.2, -0.2, 1.0), Vec3::new(0.2, 0.2, 1.0))),
        );
        let large = canvas_plane_depth_meters(
            origin,
            forward,
            Some((Vec3::new(-2.0, -2.0, 1.0), Vec3::new(2.0, 2.0, 1.0))),
        );
        assert_eq!(small, Some(1.0));
        assert_eq!(large, Some(1.0));
        assert_eq!(
            canvas_plane_depth_meters(origin, Vec3::ZERO, small.map(|_| (Vec3::ZERO, Vec3::ONE))),
            None
        );
    }

    #[test]
    fn canvas_depth_key_ignores_farthest_corner_when_plane_is_known() {
        let near_corner = canvas_draw(0, 1.0, Some(4.0));
        let far_corner = canvas_draw(1, 10_000.0, Some(4.0 + 1.0e-4));
        assert_eq!(
            transparent_depth_sort_key(&near_corner),
            transparent_depth_sort_key(&far_corner)
        );

        let farther_plane = canvas_draw(0, 1.0, Some(6.0));
        let closer_plane = canvas_draw(1, 10_000.0, Some(2.0));
        assert!(
            transparent_depth_sort_key(&farther_plane) > transparent_depth_sort_key(&closer_plane)
        );
    }

    #[test]
    fn missing_plane_falls_back_to_monotonic_distance() {
        let near = canvas_draw(0, 1.0, None);
        let far = canvas_draw(1, 4096.0, None);
        assert!(transparent_depth_sort_key(&far) > transparent_depth_sort_key(&near));
        assert!(
            (transparent_depth_sort_key(&far) - 64.0).abs() < 1.0e-9,
            "sqrt(4096) stays the non-canvas meter key"
        );
    }

    #[test]
    fn canvas_steps_reset_on_plane_change_and_cap() {
        let mut gap = canvas_draw(2, 1.0, Some(2.0));
        gap.batch_key.pipeline = RasterPipelineKind::Null;
        let draws = vec![
            canvas_draw(0, 1.0, Some(2.0)),
            canvas_draw(1, 9.0, Some(2.0)),
            gap,
            canvas_draw(3, 1.0, Some(2.0)),
            canvas_draw(4, 1.0, Some(4.0)),
        ];
        assert_eq!(assign_canvas_depth_steps(&draws), vec![0, 1, 0, 2, 0]);

        let same_plane = (0..CANVAS_DEPTH_STEP_CAP + 5)
            .map(|index| canvas_draw(index as usize, 1.0, Some(2.0)))
            .collect::<Vec<_>>();
        let steps = assign_canvas_depth_steps(&same_plane);
        assert_eq!(steps[0], 0);
        assert_eq!(steps[1], 1);
        assert_eq!(steps[same_plane.len() - 1], CANVAS_DEPTH_STEP_CAP);
    }

    #[test]
    fn reverse_z_units_push_away_and_steps_pull_closer() {
        let unit = 1.0 / 16_777_216.0;
        assert!((reverse_z_constant_ndc_bias(10, 0) - (-10.0 * unit)).abs() < 1.0e-12);
        assert!((reverse_z_constant_ndc_bias(10, 3) - (-7.0 * unit)).abs() < 1.0e-12);
        assert_eq!(
            reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP + 9),
            reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP)
        );

        let mut draw = canvas_draw(0, 1.0, Some(1.0));
        draw.batch_key.render_state.depth_offset = MaterialDepthOffsetState::new(1.0, 4);
        assert!(canvas_ndc_z_bias(&draw, 0) < 0.0);
        assert_eq!(canvas_ndc_z_bias(&draw, 4), 0.0);
        draw.batch_key.pipeline = RasterPipelineKind::Null;
        assert_eq!(canvas_ndc_z_bias(&draw, 4), 0.0);
    }

    #[test]
    fn ndc_bias_matrix_offsets_clip_z_by_bias_times_w() {
        let view_proj = Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0));
        let biased = apply_reverse_z_ndc_bias(view_proj, 0.25);
        let point = Vec4::new(0.0, 0.0, 0.5, 2.0);
        let base = view_proj * point;
        let moved = biased * point;
        let base_ndc = base.z / base.w;
        let moved_ndc = moved.z / moved.w;
        assert!((moved_ndc - base_ndc - 0.25).abs() < 1.0e-5);
        assert_eq!(apply_reverse_z_ndc_bias(view_proj, 0.0), view_proj);
    }
}
