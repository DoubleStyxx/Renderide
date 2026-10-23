use super::{
    CANVAS_CLOSER_BIAS_CAP_METERS, CANVAS_DEPTH_STEP_CAP, CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS,
    CANVAS_NDC_ULPS_PER_STEP, CANVAS_OFFSET_UNITS_CAP, CANVAS_PLANE_DEPTH_QUANTUM_METERS,
    CanvasBiasProjection, apply_reverse_z_ndc_bias, assign_canvas_depth_steps,
    canvas_bias_projection, canvas_ndc_z_bias, canvas_plane_depth_quantum,
    canvas_plane_distance_meters, cap_closer_ndc_bias, quantize_canvas_plane_depth,
    reverse_z_constant_ndc_bias, reverse_z_ndc_closer_shift_meters,
    reverse_z_ortho_closer_shift_meters, snap_canvas_sibling_plane_depths,
    sync_transparent_depth_key, transparent_depth_key_from_parts,
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

fn steps_of(draws: &[super::super::item::WorldMeshDrawItem]) -> Vec<u32> {
    let mut steps = Vec::new();
    assign_canvas_depth_steps(draws, &mut steps);
    steps
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
    assert_eq!(steps_of(&draws), vec![0, 1, 0, 2, 0]);

    let same_plane = (0..CANVAS_DEPTH_STEP_CAP + 5)
        .map(|index| canvas_draw(index as usize, 1.0, Some(2.0)))
        .collect::<Vec<_>>();
    let steps = steps_of(&same_plane);
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
    assert_eq!(steps_of(&draws), vec![0, 0, 0, 0]);
    let mut opaque = canvas_draw(0, 1.0, None);
    opaque.is_canvas_ui = false;
    assert!(steps_of(&[opaque]).is_empty());
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
    let perspective = CanvasBiasProjection::Perspective;
    assert!(canvas_ndc_z_bias(&draw, 0, 0.1, 1000.0, perspective) < 0.0);
    assert_eq!(canvas_ndc_z_bias(&draw, 4, 0.1, 1000.0, perspective), 0.0);
    draw.is_canvas_ui = false;
    assert_eq!(canvas_ndc_z_bias(&draw, 4, 0.1, 1000.0, perspective), 0.0);
}

#[test]
fn one_step_moves_a_few_hundredths_of_a_millimeter_at_one_meter() {
    let quantum = CANVAS_NDC_ULPS_PER_STEP as f32 / 16_777_216.0;
    let far = CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS;
    let step_1m_near_001 = reverse_z_ndc_closer_shift_meters(0.01, far, 1.0, quantum);
    let cap_20m_near_01 =
        reverse_z_ndc_closer_shift_meters(0.1, far, 20.0, quantum) * CANVAS_DEPTH_STEP_CAP as f32;
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

    // Uncapped 64 steps at near 0.01 and 20 m is about 61 cm. The applied bias stops at 2 cm.
    let raw_far = reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP);
    let capped = cap_closer_ndc_bias(raw_far, 20.0, 0.01, far, CanvasBiasProjection::Perspective);
    let capped_pull = reverse_z_ndc_closer_shift_meters(0.01, far, 20.0, capped);
    assert!(
        (capped_pull - CANVAS_CLOSER_BIAS_CAP_METERS).abs() < 1.0e-3,
        "capped pull {capped_pull} m"
    );
    let near_camera = reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP);
    assert_eq!(
        cap_closer_ndc_bias(
            near_camera,
            1.0,
            0.01,
            far,
            CanvasBiasProjection::Perspective
        ),
        near_camera
    );
    let away = reverse_z_constant_ndc_bias(CANVAS_OFFSET_UNITS_CAP, 0);
    assert_eq!(
        cap_closer_ndc_bias(away, 20.0, 0.01, far, CanvasBiasProjection::Perspective),
        away
    );
}

#[test]
fn sheared_matrix_uses_the_xy_cross_product() {
    let origin = Vec3::ZERO;
    let sheared = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.5, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 3.0, 1.0),
    );
    let sibling = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.5, 0.0, 1.0, 0.0),
        Vec4::new(1.0, 0.0, 3.0, 1.0),
    );
    let sheared_distance = canvas_plane_distance_meters(origin, sheared).expect("sheared");
    let sibling_distance = canvas_plane_distance_meters(origin, sibling).expect("sibling");
    assert!(
        (sheared_distance - 3.0).abs() < 1.0e-4,
        "{sheared_distance}"
    );
    assert!(
        (sibling_distance - 3.0).abs() < 1.0e-4,
        "{sibling_distance}"
    );
    let z_column = Vec3::new(0.5, 0.0, 1.0).normalize();
    let z_column_distance = z_column.dot(Vec3::new(0.0, 0.0, 3.0)).abs();
    assert!(
        (sheared_distance - z_column_distance).abs() > 0.2,
        "z-column distance {z_column_distance} must not be the plane distance"
    );
}

#[test]
fn zero_z_scale_still_defines_the_xy_plane() {
    let flat_z = Mat4::from_cols(Vec4::X, Vec4::Y, Vec4::ZERO, Vec4::new(0.0, 0.0, 4.0, 1.0));
    let flat_distance = canvas_plane_distance_meters(Vec3::ZERO, flat_z).expect("zero z scale");
    assert!((flat_distance - 4.0).abs() < 1.0e-4, "{flat_distance}");
    assert_eq!(
        canvas_plane_distance_meters(Vec3::ZERO, Mat4::from_scale(Vec3::ZERO)),
        None
    );
}

#[test]
fn sibling_plane_jitter_across_a_quantum_boundary_stays_one_key() {
    let boundary = 1.00025_f32;
    let low = boundary - 5.0e-7;
    let high = boundary + 5.0e-7;
    assert!(
        (f64::from(high) - f64::from(low) - 1.0e-6).abs() < 1.0e-7,
        "jitter gap {} m",
        f64::from(high) - f64::from(low)
    );
    assert_ne!(
        quantize_canvas_plane_depth(low),
        quantize_canvas_plane_depth(high)
    );
    let mut chunks = vec![vec![
        canvas_draw(0, 1.0, Some(low)),
        canvas_draw(1, 1.0, Some(high)),
    ]];
    snap_canvas_sibling_plane_depths(&mut chunks);
    let snapped_low = chunks[0][0].canvas_plane_depth.expect("low");
    let snapped_high = chunks[0][1].canvas_plane_depth.expect("high");
    assert_eq!(
        quantize_canvas_plane_depth(snapped_low),
        quantize_canvas_plane_depth(snapped_high)
    );
    assert_eq!(steps_of(&chunks[0]), vec![0, 1]);

    let separated_gap = 4.0e-4;
    let mut separated = vec![vec![
        canvas_draw(0, 1.0, Some(1.0)),
        canvas_draw(1, 1.0, Some(1.0 + separated_gap)),
    ]];
    snap_canvas_sibling_plane_depths(&mut separated);
    let far = separated[0][1].canvas_plane_depth.expect("separated");
    assert!((far - (1.0 + separated_gap)).abs() < 1.0e-6);
    assert_ne!(
        quantize_canvas_plane_depth(separated[0][0].canvas_plane_depth.expect("near")),
        quantize_canvas_plane_depth(far)
    );
}

#[test]
fn non_parallel_canvases_order_by_plane_distance_not_overlap() {
    let origin = Vec3::ZERO;
    let face =
        canvas_plane_distance_meters(origin, Mat4::from_translation(Vec3::new(0.0, 0.0, 1.5)))
            .expect("face-on panel");
    let tilt = 60.0_f32.to_radians();
    let normal = Vec3::new(tilt.sin(), 0.0, tilt.cos());
    let tilted = canvas_plane_distance_meters(
        origin,
        Mat4::from_rotation_translation(Quat::from_rotation_y(tilt), normal),
    )
    .expect("tilted panel");
    assert!((face - 1.5).abs() < 1.0e-4, "{face}");
    assert!((tilted - 1.0).abs() < 1.0e-4, "{tilted}");
    let axis_z = 1.0 / tilt.cos();
    assert!(
        (axis_z - 2.0).abs() < 1.0e-4 && axis_z > face,
        "the tilted plane crosses +Z at {axis_z}, behind the face-on panel at {face}"
    );
    let face_key = transparent_depth_key_from_parts(true, Some(face), 1.0);
    let tilt_key = transparent_depth_key_from_parts(true, Some(tilted), 1.0);
    assert!(
        tilt_key < face_key,
        "the 1 m plane is drawn after the 1.5 m plane even where the tilted panel is behind it"
    );
}

#[test]
fn one_sibling_step_clears_the_distance_golden_gap_and_one_ulp_does_not() {
    let near = 0.1;
    let far = 100.0;
    let distance = 18.0;
    let quantum = CANVAS_NDC_ULPS_PER_STEP as f32 / 16_777_216.0;
    let four = reverse_z_ndc_closer_shift_meters(near, far, distance, quantum);
    let one = reverse_z_ndc_closer_shift_meters(near, far, distance, quantum * 0.25);
    let view_gap = 0.00015 / 60.0_f32.to_radians().cos();
    assert!(
        one < view_gap && view_gap < four,
        "one ulp {one} m, gap {view_gap} m, four ulps {four} m"
    );
    assert!(
        (cap_closer_ndc_bias(
            quantum,
            distance,
            near,
            far,
            CanvasBiasProjection::Perspective,
        ) - quantum)
            .abs()
            < 1.0e-12
    );
}

#[test]
fn sibling_steps_stay_distinct_at_twenty_meters() {
    let near = 0.01;
    let far = CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS;
    let plane = 20.0;
    let perspective = CanvasBiasProjection::Perspective;
    let near_corner = canvas_draw(0, plane * plane, Some(plane));
    let far_corner = canvas_draw(1, 40.0 * 40.0, Some(plane));
    let step_one = canvas_ndc_z_bias(&near_corner, 1, near, far, perspective);
    let step_two = canvas_ndc_z_bias(&near_corner, 2, near, far, perspective);
    assert!(
        step_two > step_one && step_one > 0.0,
        "step biases {step_one} and {step_two} collapsed"
    );
    let pull_one = reverse_z_ndc_closer_shift_meters(near, far, plane, step_one);
    let pull_two = reverse_z_ndc_closer_shift_meters(near, far, plane, step_two);
    assert!(
        pull_one < pull_two && pull_two <= CANVAS_CLOSER_BIAS_CAP_METERS + 1.0e-4,
        "pulls {pull_one} m and {pull_two} m"
    );
    assert!(
        (pull_one - 0.00953).abs() < 5.0e-4 && (pull_two - 0.01906).abs() < 5.0e-4,
        "expected about 9.5 mm and 19 mm, got {pull_one} m and {pull_two} m"
    );
    let far_sibling = canvas_ndc_z_bias(&far_corner, 2, near, far, perspective);
    assert_eq!(far_sibling.to_bits(), step_two.to_bits());
    let mut corner_only = far_corner;
    corner_only.canvas_plane_depth = None;
    sync_transparent_depth_key(&mut corner_only);
    let corner_bias = canvas_ndc_z_bias(&corner_only, 2, near, far, perspective);
    assert!(
        corner_bias < step_two,
        "a 40 m corner saturates ({corner_bias}); the shared 20 m plane must not ({step_two})"
    );
}

#[test]
fn orthographic_cap_does_not_use_the_perspective_pull() {
    let near = 0.01;
    let far = CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS;
    let raw = reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP);
    let perspective = cap_closer_ndc_bias(raw, 20.0, near, far, CanvasBiasProjection::Perspective);
    let ortho = cap_closer_ndc_bias(raw, 20.0, near, far, CanvasBiasProjection::Orthographic);
    assert_eq!(ortho, raw);
    assert!(perspective < raw);
    let ortho_pull = reverse_z_ortho_closer_shift_meters(near, far, ortho);
    assert!(
        ortho_pull < CANVAS_CLOSER_BIAS_CAP_METERS,
        "64 steps at far 1000 is about 1.5 cm orthographically, got {ortho_pull} m"
    );

    let wide_far = 10_000.0;
    let wide = cap_closer_ndc_bias(
        raw,
        20.0,
        near,
        wide_far,
        CanvasBiasProjection::Orthographic,
    );
    let wide_perspective =
        cap_closer_ndc_bias(raw, 20.0, near, wide_far, CanvasBiasProjection::Perspective);
    let wide_pull = reverse_z_ortho_closer_shift_meters(near, wide_far, wide);
    assert!(
        (wide_pull - CANVAS_CLOSER_BIAS_CAP_METERS).abs() < 1.0e-3,
        "orthographic pull {wide_pull} m"
    );
    assert!(
        wide > wide_perspective,
        "perspective overestimates: ortho {wide}, perspective {wide_perspective}"
    );
}

#[test]
fn overlay_projection_is_orthographic_and_a_missing_overlay_follows_the_world() {
    assert_eq!(
        canvas_bias_projection(true, true, false),
        CanvasBiasProjection::Orthographic
    );
    assert_eq!(
        canvas_bias_projection(true, false, false),
        CanvasBiasProjection::Perspective
    );
    assert_eq!(
        canvas_bias_projection(true, false, true),
        CanvasBiasProjection::Orthographic
    );
    assert_eq!(
        canvas_bias_projection(false, true, false),
        CanvasBiasProjection::Perspective
    );
    assert_eq!(
        canvas_bias_projection(false, false, true),
        CanvasBiasProjection::Orthographic
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
