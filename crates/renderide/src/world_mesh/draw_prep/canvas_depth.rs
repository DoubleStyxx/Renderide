//! Canvas-plane ordering and reverse-Z constant depth bias for UIX stems.
//!
//! Coplanar UI meshes of different sizes do not share a farthest-AABB-corner distance, and a
//! camera-forward depth of the AABB center splits siblings on a tilted canvas. Canvas stems
//! instead compare the absolute distance from the view origin to the draw's own plane. A
//! constant NDC bias, four 24-bit depth units per Unity `Offset` unit, is baked into the
//! per-draw view-projection because wgpu's hardware depth-bias constant collapses on
//! `Depth32Float` reverse-Z away from the near plane.
//!
//! Unity `Offset` units are applied in both places on purpose. `MaterialRenderState::depth_bias`
//! still feeds `wgpu::DepthBiasState` so the slope term and the existing constant-bias pipeline
//! tests stay put. The NDC term is what actually separates a face-on quad once the float depth
//! ULP exceeds one hardware constant unit. Near the camera the constant is therefore applied
//! twice. That doubling is a fraction of a millimeter at typical UI distances and is the
//! conservative choice: this path does not zero the hardware constant.

use glam::{Mat4, Vec3};

use super::item::WorldMeshDrawItem;

/// Plane-distance separation below which two canvas elements are treated as coplanar.
///
/// Half a millimeter keeps siblings on one geometric plane in one bucket so hierarchy order
/// wins, while a real gap between canvases still sorts back to front.
pub(crate) const CANVAS_PLANE_DEPTH_QUANTUM_METERS: f32 = 5.0e-4;

/// Maximum extra canvas steps applied on top of Unity `Offset` units.
///
/// Each step is [`CANVAS_NDC_ULPS_PER_STEP`] D24 quanta toward the camera. The cap stops a long
/// sibling list from walking through a surface that is actually in front.
pub(crate) const CANVAS_DEPTH_STEP_CAP: u32 = 64;

/// Largest absolute Unity `Offset` unit count baked into the NDC bias.
///
/// Matches [`CANVAS_DEPTH_STEP_CAP`]. Larger authored values still reach the hardware bias
/// unchanged; only the NDC copy is clamped, so a runaway property cannot shove a primitive
/// across the whole depth range.
pub(crate) const CANVAS_OFFSET_UNITS_CAP: i32 = 64;

/// D24 quanta moved per `Offset` unit and per canvas sibling step.
///
/// One quantum (`2^-24`) is at or below one `f32` ULP once reverse-Z NDC is in `[0.5, 1]`, so a
/// single unit does not reliably separate a tilted quad. Four quanta clear that binade.
pub(crate) const CANVAS_NDC_ULPS_PER_STEP: u32 = 4;

/// NDC Z delta of one 24-bit depth unit (`2^-24`).
///
/// Reverse-Z stores near as 1 and far as 0, so a positive value moves the primitive closer.
const REVERSE_Z_D24_NDC_UNIT: f32 = 1.0 / 16_777_216.0;

/// Far plane used when quoting the view-space size of one NDC step.
///
/// The shift scales with `(far - near) / far`, which is already `1` for any far that is large
/// next to `near`. `1000` matches a typical host far clip closely enough that `far = 100` (the
/// headless harness) changes the millimeter table only in the third digit.
#[cfg(test)]
pub(crate) const CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS: f32 = 1000.0;

/// Quantizes a plane distance in meters to [`CANVAS_PLANE_DEPTH_QUANTUM_METERS`].
///
/// A zero result is `+0.0`. Quantizing `-0.0` must not keep the sign bit, or two coplanar draws
/// would miss each other in a bitwise plane-run compare.
#[inline]
pub(crate) fn quantize_canvas_plane_depth(depth: f32) -> f32 {
    if !depth.is_finite() {
        return depth;
    }
    let quantum = f64::from(CANVAS_PLANE_DEPTH_QUANTUM_METERS);
    let steps = (f64::from(depth) / quantum).round();
    let quantized = (steps * quantum) as f32;
    if quantized == 0.0 { 0.0 } else { quantized }
}

/// Integer quantum of a plane distance. `None` when `depth` is non-finite or out of `i32` range.
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

/// Absolute distance in meters from `origin` to the draw plane stored in `rigid_world`.
///
/// UI quads lie in local `z = 0`. The plane normal is the matrix Z axis, normalized so scale
/// does not change the plane equation. The returned distance does not use the camera forward, so
/// a yaw or a tilt cannot put coplanar siblings into different buckets. `None` when the matrix
/// axis or the origin is unusable.
pub(crate) fn canvas_plane_distance_meters(origin: Vec3, rigid_world: Mat4) -> Option<f32> {
    if !origin.is_finite() {
        return None;
    }
    let axis = rigid_world.z_axis.truncate();
    let point = rigid_world.w_axis.truncate();
    if !axis.is_finite() || !point.is_finite() {
        return None;
    }
    let length_sq = axis.length_squared();
    if length_sq <= 1.0e-12 {
        return None;
    }
    let normal = axis * length_sq.sqrt().recip();
    let distance = normal.dot(point - origin).abs();
    distance.is_finite().then_some(distance)
}

/// Farther-first depth key stored on a draw at collect time.
///
/// Canvas stems with a finite plane distance use the quantized plane distance in meters. Every
/// other draw uses `sqrt(camera_distance_sq)` so non-UI order stays identical to comparing
/// farthest-corner distance squared. A canvas stem with no plane falls back to that same
/// distance. Comparing the two kinds of key is a single total order, and it is not the same
/// order as comparing every draw by farthest-corner distance: a canvas and a world transparent
/// at "the same" place can swap.
#[inline]
pub(crate) fn transparent_depth_key_from_parts(
    is_canvas_ui: bool,
    canvas_plane_depth: Option<f32>,
    camera_distance_sq: f32,
) -> f64 {
    if is_canvas_ui && let Some(depth) = canvas_plane_depth.filter(|depth| depth.is_finite()) {
        return f64::from(quantize_canvas_plane_depth(depth));
    }
    non_canvas_depth_meters(camera_distance_sq)
}

/// Writes [`WorldMeshDrawItem::transparent_depth_key`] from the item's current plane and distance.
#[cfg(any(test, feature = "bench-api"))]
#[inline]
pub(crate) fn sync_transparent_depth_key(item: &mut WorldMeshDrawItem) {
    item.transparent_depth_key = transparent_depth_key_from_parts(
        item.is_canvas_ui,
        item.canvas_plane_depth,
        item.camera_distance_sq,
    );
}

/// Depth key the way the pre-`WorldMeshDrawItem::transparent_depth_key` comparator computed it.
///
/// Calls `RasterPipelineKind::is_canvas_ui_stem` on every evaluation.
/// The sort microbench uses this so the "before" median still pays the stem match and the
/// divide or square root. Production compares [`WorldMeshDrawItem::transparent_depth_key`].
#[cfg(feature = "bench-api")]
#[inline]
pub(crate) fn recomputed_transparent_depth_key(item: &WorldMeshDrawItem) -> f64 {
    transparent_depth_key_from_parts(
        item.batch_key.pipeline.is_canvas_ui_stem(),
        item.canvas_plane_depth,
        item.camera_distance_sq,
    )
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

/// Per-draw canvas step along `draws`.
///
/// Production passes the arranged draw list, so the step follows painter order: the first canvas
/// in a coplanar run stays unbiased and each later one is pulled closer. Only canvas stems
/// advance the rank. The rank resets when the quantized plane distance changes and is capped at
/// [`CANVAS_DEPTH_STEP_CAP`]. Non-canvas draws between two same-plane canvases do not reset it.
/// A canvas with no plane distance is unrelated to every other draw, including other missing
/// planes, and does not continue a run.
///
/// The returned vector is empty when `draws` contains no canvas stem, so the common opaque frame
/// does not allocate a zero-filled step buffer. Otherwise it has one entry per draw.
pub(crate) fn assign_canvas_depth_steps(draws: &[WorldMeshDrawItem]) -> Vec<u32> {
    let mut steps = Vec::new();
    let mut rank = 0u32;
    let mut previous_quantum: Option<i32> = None;
    let mut have_previous = false;
    for (index, draw) in draws.iter().enumerate() {
        if !draw.is_canvas_ui {
            continue;
        }
        if steps.is_empty() {
            steps.resize(draws.len(), 0);
        }
        let Some(quantum) = draw
            .canvas_plane_depth
            .filter(|depth| depth.is_finite())
            .and_then(canvas_plane_depth_quantum)
        else {
            have_previous = false;
            previous_quantum = None;
            steps[index] = 0;
            continue;
        };
        if have_previous && previous_quantum == Some(quantum) {
            rank = rank.saturating_add(1);
        } else {
            rank = 0;
            previous_quantum = Some(quantum);
            have_previous = true;
        }
        steps[index] = rank.min(CANVAS_DEPTH_STEP_CAP);
    }
    steps
}

/// Constant reverse-Z NDC bias for Unity `Offset` units plus canvas sibling steps.
///
/// Positive Unity units push away from the camera (negative NDC under reverse-Z). Each canvas
/// step pulls [`CANVAS_NDC_ULPS_PER_STEP`] D24 quanta closer. Units outside
/// [`CANVAS_OFFSET_UNITS_CAP`] and steps above [`CANVAS_DEPTH_STEP_CAP`] clamp.
///
/// This bias is added on top of the hardware `wgpu::DepthBiasState` constant that
/// `MaterialRenderState::depth_bias` already builds from the same units.
/// Both copies are intentional: the hardware constant still carries the slope term, and the NDC
/// copy is the one that survives `Depth32Float` reverse-Z. Near the camera the unit count is
/// applied twice.
#[inline]
pub(crate) fn reverse_z_constant_ndc_bias(units: i32, canvas_steps: u32) -> f32 {
    let units = units.clamp(-CANVAS_OFFSET_UNITS_CAP, CANVAS_OFFSET_UNITS_CAP) as f32;
    let steps = canvas_steps.min(CANVAS_DEPTH_STEP_CAP) as f32;
    let quanta = REVERSE_Z_D24_NDC_UNIT * CANVAS_NDC_ULPS_PER_STEP as f32;
    (-units + steps) * quanta
}

/// How far a positive NDC Z bias moves a view-space point toward the camera, in meters.
///
/// Uses the renderer's finite reverse-Z perspective (`ndc = -z2 + z3 / distance` with
/// `z2 = near / (far - near)` and `z3 = far * near / (far - near)`). The result is positive when
/// `ndc_delta` moves the sample closer. Non-finite inputs return `0.0`.
#[cfg(test)]
pub(crate) fn reverse_z_ndc_closer_shift_meters(
    near: f32,
    far: f32,
    distance: f32,
    ndc_delta: f32,
) -> f32 {
    if !(near.is_finite()
        && far.is_finite()
        && distance.is_finite()
        && ndc_delta.is_finite()
        && near > 0.0
        && far > near
        && distance > 0.0)
    {
        return 0.0;
    }
    let span = f64::from(far) - f64::from(near);
    let z2 = f64::from(near) / span;
    let z3 = f64::from(far) * f64::from(near) / span;
    let ndc = -z2 + z3 / f64::from(distance);
    let target = ndc + f64::from(ndc_delta);
    let denominator = target + z2;
    if denominator.abs() <= 1.0e-18 {
        return 0.0;
    }
    let closer = f64::from(distance) - (z3 / denominator);
    if closer.is_finite() {
        closer as f32
    } else {
        0.0
    }
}

/// NDC Z bias to bake into one draw's view-projection. Non-canvas draws stay at zero.
#[inline]
pub(crate) fn canvas_ndc_z_bias(item: &WorldMeshDrawItem, canvas_steps: u32) -> f32 {
    if !item.is_canvas_ui {
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
        CANVAS_DEPTH_STEP_CAP, CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS, CANVAS_NDC_ULPS_PER_STEP,
        CANVAS_OFFSET_UNITS_CAP, CANVAS_PLANE_DEPTH_QUANTUM_METERS, apply_reverse_z_ndc_bias,
        assign_canvas_depth_steps, canvas_ndc_z_bias, canvas_plane_depth_quantum,
        canvas_plane_distance_meters, quantize_canvas_plane_depth, reverse_z_constant_ndc_bias,
        reverse_z_ndc_closer_shift_meters, sync_transparent_depth_key,
        transparent_depth_key_from_parts,
    };
    use crate::materials::MaterialDepthOffsetState;
    use crate::materials::RasterPipelineKind;
    use crate::world_mesh::materials::compute_batch_key_hash;
    use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};
    use glam::{Mat4, Quat, Vec3, Vec4};
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
        item.is_canvas_ui = true;
        item.canvas_plane_depth = plane_depth;
        sync_transparent_depth_key(&mut item);
        item
    }

    fn plane_matrix(yaw: f32, point: Vec3) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), point)
    }

    #[test]
    fn quantize_collapses_sub_millimeter_plane_gaps_and_clears_negative_zero() {
        let base = quantize_canvas_plane_depth(1.0);
        assert_eq!(base, quantize_canvas_plane_depth(1.0 + 1.0e-4));
        assert_ne!(
            canvas_plane_depth_quantum(1.0),
            canvas_plane_depth_quantum(1.0 + 1.0e-3)
        );
        assert!((CANVAS_PLANE_DEPTH_QUANTUM_METERS - 5.0e-4).abs() < f32::EPSILON);
        assert_eq!(
            quantize_canvas_plane_depth(-0.0).to_bits(),
            0.0_f32.to_bits()
        );
        assert_eq!(
            quantize_canvas_plane_depth(1.0e-7).to_bits(),
            0.0_f32.to_bits()
        );
    }

    #[test]
    fn tilted_canvas_siblings_share_a_plane_distance_at_two_yaws() {
        let origin = Vec3::new(0.0, 0.0, -3.0);
        for yaw_degrees in [35.0_f32, -50.0] {
            let yaw = yaw_degrees.to_radians();
            let rotation = Mat4::from_rotation_y(yaw);
            let parent_point = rotation.transform_point3(Vec3::ZERO) + Vec3::new(0.4, 0.0, 0.2);
            let child_point = parent_point + rotation.transform_vector3(Vec3::new(-0.25, 0.1, 0.0));
            let parent = canvas_plane_distance_meters(origin, plane_matrix(yaw, parent_point));
            let child = canvas_plane_distance_meters(origin, plane_matrix(yaw, child_point));
            let parent = parent.expect("parent plane");
            let child = child.expect("child plane");
            assert_eq!(
                quantize_canvas_plane_depth(parent),
                quantize_canvas_plane_depth(child),
                "yaw {yaw_degrees} split coplanar siblings"
            );
            let parent_forward = (parent_point - origin).dot(Vec3::Z);
            let child_forward = (child_point - origin).dot(Vec3::Z);
            assert!(
                (parent_forward - child_forward).abs() > CANVAS_PLANE_DEPTH_QUANTUM_METERS,
                "the camera-forward center metric must be the one that splits this pair"
            );
        }
        assert_eq!(
            canvas_plane_distance_meters(origin, Mat4::from_scale(Vec3::ZERO)),
            None
        );
    }

    #[test]
    fn canvas_depth_key_ignores_farthest_corner_when_plane_is_known() {
        let near_corner = canvas_draw(0, 1.0, Some(4.0));
        let far_corner = canvas_draw(1, 10_000.0, Some(4.0 + 1.0e-4));
        assert_eq!(
            near_corner.transparent_depth_key,
            far_corner.transparent_depth_key
        );
        assert_eq!(
            near_corner.transparent_depth_key,
            transparent_depth_key_from_parts(true, Some(4.0), 1.0)
        );

        let farther_plane = canvas_draw(0, 1.0, Some(6.0));
        let closer_plane = canvas_draw(1, 10_000.0, Some(2.0));
        assert!(farther_plane.transparent_depth_key > closer_plane.transparent_depth_key);
    }

    #[test]
    fn missing_plane_falls_back_to_monotonic_distance() {
        let near = canvas_draw(0, 1.0, None);
        let far = canvas_draw(1, 4096.0, None);
        assert!(far.transparent_depth_key > near.transparent_depth_key);
        assert!(
            (far.transparent_depth_key - 64.0).abs() < 1.0e-9,
            "sqrt(4096) stays the non-canvas meter key"
        );
    }

    #[test]
    fn canvas_steps_reset_on_plane_change_and_cap() {
        let mut gap = canvas_draw(2, 1.0, Some(2.0));
        gap.batch_key.pipeline = RasterPipelineKind::Null;
        gap.is_canvas_ui = false;
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
    fn missing_plane_depths_do_not_share_a_step_run() {
        let draws = vec![
            canvas_draw(0, 1.0, Some(2.0)),
            canvas_draw(1, 1.0, None),
            canvas_draw(2, 1.0, None),
            canvas_draw(3, 1.0, Some(2.0)),
        ];
        assert_eq!(assign_canvas_depth_steps(&draws), vec![0, 0, 0, 0]);
        let mut opaque = canvas_draw(0, 1.0, None);
        opaque.is_canvas_ui = false;
        assert!(assign_canvas_depth_steps(&[opaque]).is_empty());
    }

    #[test]
    fn reverse_z_units_push_away_and_steps_pull_closer() {
        let quantum = CANVAS_NDC_ULPS_PER_STEP as f32 / 16_777_216.0;
        assert!((reverse_z_constant_ndc_bias(10, 0) - (-10.0 * quantum)).abs() < 1.0e-12);
        assert!((reverse_z_constant_ndc_bias(10, 3) - (-7.0 * quantum)).abs() < 1.0e-12);
        assert_eq!(
            reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP + 9),
            reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP)
        );
        assert_eq!(
            reverse_z_constant_ndc_bias(CANVAS_OFFSET_UNITS_CAP + 100, 0),
            reverse_z_constant_ndc_bias(CANVAS_OFFSET_UNITS_CAP, 0)
        );
        assert_eq!(
            reverse_z_constant_ndc_bias(-(CANVAS_OFFSET_UNITS_CAP + 100), 0),
            reverse_z_constant_ndc_bias(-CANVAS_OFFSET_UNITS_CAP, 0)
        );

        let mut draw = canvas_draw(0, 1.0, Some(1.0));
        draw.batch_key.render_state.depth_offset = MaterialDepthOffsetState::new(1.0, 4);
        assert!(canvas_ndc_z_bias(&draw, 0) < 0.0);
        assert_eq!(canvas_ndc_z_bias(&draw, 4), 0.0);
        draw.is_canvas_ui = false;
        assert_eq!(canvas_ndc_z_bias(&draw, 4), 0.0);
    }

    #[test]
    fn one_step_moves_a_few_hundredths_of_a_millimeter_at_one_meter() {
        let quantum = CANVAS_NDC_ULPS_PER_STEP as f32 / 16_777_216.0;
        let far = CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS;
        let step_1m_near_001 = reverse_z_ndc_closer_shift_meters(0.01, far, 1.0, quantum);
        let cap_20m_near_01 = reverse_z_ndc_closer_shift_meters(0.1, far, 20.0, quantum)
            * CANVAS_DEPTH_STEP_CAP as f32;
        assert!((step_1m_near_001 - 2.384e-5).abs() < 1.0e-7);
        assert!((cap_20m_near_01 - 0.06103).abs() < 1.0e-4);
        // Headless golden camera: 64 units of 4 ulps at 3 m, near 0.1, far 100.
        // That pull is about 1.37 mm, which clears a 0.6 mm geometric gap. One D24 ulp
        // times 64 does not.
        let golden_pull = reverse_z_ndc_closer_shift_meters(0.1, 100.0, 3.0, quantum)
            * CANVAS_OFFSET_UNITS_CAP as f32;
        assert!(
            golden_pull > 1.2e-3 && golden_pull < 1.6e-3,
            "golden offset pull {golden_pull} m is outside the 0.6 mm gap window"
        );
        assert_eq!(
            reverse_z_ndc_closer_shift_meters(0.1, 0.1, 1.0, quantum),
            0.0
        );
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
