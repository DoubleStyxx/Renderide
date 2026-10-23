//! Canvas-plane ordering and reverse-Z constant depth bias for UIX stems.
//!
//! Coplanar UI meshes of different sizes do not share a farthest-AABB-corner distance, and a
//! camera-forward depth of the AABB center splits siblings on a tilted canvas. Canvas stems
//! instead compare the absolute distance from the view origin to the draw's own plane. A
//! constant NDC bias, four 24-bit depth units per Unity `Offset` unit, is baked into the
//! per-draw view-projection because wgpu's hardware depth-bias constant collapses on
//! `Depth32Float` reverse-Z away from the near plane.
//!
//! Unity `Offset` units reach the depth test through two different mechanisms.
//! `MaterialRenderState::depth_bias` still feeds `wgpu::DepthBiasState` so the slope term and the
//! existing constant-bias pipeline tests stay put. The NDC term is what actually separates a
//! face-on quad once the float depth ULP exceeds one hardware constant unit. Near the camera each
//! Offset unit is about five depth quanta: one hardware D24 unit, plus four NDC quanta. Farther
//! out the hardware constant stops moving `Depth32Float`, and the NDC term is the one that
//! remains. This path does not zero the hardware constant.
//!
//! The closer-direction NDC bias (sibling steps, and negative Offset units) is scaled so its
//! view-space pull at the shared canvas plane stays within [`CANVAS_CLOSER_BIAS_CAP_METERS`].
//! That bias is applied in the depth test for every canvas draw with a step greater than zero
//! or with negative Offset units, whatever the draw's `_ZWrite`. A canvas that does not write
//! depth can still paint over opaque geometry up to 2 cm in front, because the test compares
//! the biased depth with depth that was already written. Perspective fragments closer than the
//! plane move less. An orthographic view moves every fragment of the draw by the same distance,
//! `ndc_delta * (far - near)`, and the perspective formula overestimates that pull.
//!
//! One canvas is one plane distance. A panel tilted 60° whose plane is 1 m from the view origin
//! sorts closer than a face-on panel at 1.5 m, including fragments of the tilted panel that
//! extend behind the face-on panel. Ordering is not per-fragment back-to-front.
//!
//! Canvas keys and world-transparent keys share one total order and are not the same metric. A
//! world draw uses farthest-corner distance. A canvas uses plane distance. A glass pane in front
//! of a canvas sorts behind that canvas when the pane's farthest corner is farther than the
//! canvas plane. The swap is the difference between those two distances, so a large pane can be
//! ordered wrong by about its own extent.

use glam::{Mat4, Vec3};

use super::item::WorldMeshDrawItem;

/// Plane-distance separation below which two canvas elements are treated as coplanar.
///
/// Half a millimeter keeps siblings on one geometric plane in one bucket so hierarchy order
/// wins, while a real gap between canvases still sorts back to front.
pub(crate) const CANVAS_PLANE_DEPTH_QUANTUM_METERS: f32 = 5.0e-4;

/// Raw plane distances within this gap snap to one anchor before quantization.
///
/// Independent rounding to [`CANVAS_PLANE_DEPTH_QUANTUM_METERS`] splits siblings whose float
/// noise is about `1e-6` m when that noise straddles a quantum boundary. `0.1` mm is about
/// 100 times that noise and well under the quantum, so a real gap of a few tenths of a
/// millimeter still starts a new run. The anchor is the nearest sibling in the cluster, not a
/// running neighbor, so a chain of small gaps cannot swallow a larger one.
pub(crate) const CANVAS_PLANE_SIBLING_SNAP_METERS: f32 = 1.0e-4;

/// Maximum view-space pull of the closer-direction NDC bias, at the shared plane distance.
///
/// Every sibling on one snapped plane uses that plane's distance, so a far corner on the same
/// plane does not shrink the bias further. Sixty-four steps of four D24 quanta at near `0.01`
/// and a plane `20` m away is about `61` cm toward the camera under perspective. Capping the
/// pull at `2` cm keeps a long coplanar run from painting over opaque geometry that far in
/// front. The depth test sees this bias whether or not the draw writes depth. One step near
/// the camera stays four quanta, because that pull is a fraction of a millimeter. Two steps at
/// `20` m and near `0.01` stay under the cap and stay distinct. An orthographic pull is
/// `ndc_delta * (far - near)` and does not grow with distance.
pub(crate) const CANVAS_CLOSER_BIAS_CAP_METERS: f32 = 0.02;

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
/// UI quads lie in local `z = 0`, the plane spanned by the matrix X and Y axes. The normal is
/// `normalize(cross(x_axis, y_axis))`, which stays valid under shear and under a near-zero Z
/// scale. When that cross product is degenerate the Z column is the fallback. The returned
/// distance does not use the camera forward, so a yaw or a tilt cannot put coplanar siblings
/// into different buckets. `None` when no finite normal exists or the origin is unusable.
pub(crate) fn canvas_plane_distance_meters(origin: Vec3, rigid_world: Mat4) -> Option<f32> {
    if !origin.is_finite() {
        return None;
    }
    let point = rigid_world.w_axis.truncate();
    if !point.is_finite() {
        return None;
    }
    let normal = canvas_plane_normal(rigid_world)?;
    let distance = normal.dot(point - origin).abs();
    distance.is_finite().then_some(distance)
}

/// Unit normal of the local XY plane, or the unit Z column when the cross product vanishes.
fn canvas_plane_normal(rigid_world: Mat4) -> Option<Vec3> {
    let x_axis = rigid_world.x_axis.truncate();
    let y_axis = rigid_world.y_axis.truncate();
    if x_axis.is_finite()
        && y_axis.is_finite()
        && let Some(normal) = unit_axis(x_axis.cross(y_axis))
    {
        return Some(normal);
    }
    unit_axis(rigid_world.z_axis.truncate())
}

/// Unit vector, or `None` when `axis` is non-finite or shorter than `1e-6`.
fn unit_axis(axis: Vec3) -> Option<Vec3> {
    if !axis.is_finite() {
        return None;
    }
    let length_sq = axis.length_squared();
    if length_sq <= 1.0e-12 {
        None
    } else {
        Some(axis * length_sq.sqrt().recip())
    }
}

/// Farther-first depth key stored on a draw at collect time.
///
/// Canvas stems with a finite plane distance use the quantized plane distance in meters. Every
/// other draw uses `sqrt(camera_distance_sq)` so non-UI order stays identical to comparing
/// farthest-corner distance squared. A canvas stem with no plane falls back to that same
/// distance. Comparing the two kinds of key is a single total order, and it is not the same
/// order as comparing every draw by farthest-corner distance. A glass pane in front of a canvas
/// sorts behind that canvas when the pane's farthest corner is farther than the canvas plane.
/// The disagreement is that distance difference, which grows with the pane.
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
/// advance the rank.
///
/// `steps` is cleared and left empty when `draws` contains no canvas stem, so the common opaque
/// frame does not fill a zeroed buffer. Otherwise it has one entry per draw. Callers retain the
/// `Vec` across views and frames. The rank resets when the quantized plane distance changes and
/// is capped at [`CANVAS_DEPTH_STEP_CAP`]. Non-canvas draws between two same-plane canvases do
/// not reset it. A canvas with no plane distance is unrelated to every other draw, including
/// other missing planes, and does not continue a run.
pub(crate) fn assign_canvas_depth_steps(draws: &[WorldMeshDrawItem], steps: &mut Vec<u32>) {
    steps.clear();
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
}

/// Snaps canvas siblings whose raw plane distances sit within [`CANVAS_PLANE_SIBLING_SNAP_METERS`].
///
/// Runs before arrangement and before the command-cache fingerprint, so the stored depth, the
/// transparent key, and the cache key all see one value. The cluster anchor is the nearest
/// depth in the cluster. A later draw farther than the snap gap from that anchor starts a new
/// cluster instead of walking the boundary forward.
pub(crate) fn snap_canvas_sibling_plane_depths(chunks: &mut [Vec<WorldMeshDrawItem>]) {
    let mut canvas = Vec::new();
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        for (item_index, item) in chunk.iter().enumerate() {
            if item.is_canvas_ui
                && item
                    .canvas_plane_depth
                    .is_some_and(|depth| depth.is_finite())
            {
                canvas.push((chunk_index, item_index));
            }
        }
    }
    if canvas.len() < 2 {
        return;
    }
    canvas.sort_by(|&(left_chunk, left_item), &(right_chunk, right_item)| {
        let left = chunks[left_chunk][left_item]
            .canvas_plane_depth
            .unwrap_or(0.0);
        let right = chunks[right_chunk][right_item]
            .canvas_plane_depth
            .unwrap_or(0.0);
        left.total_cmp(&right)
    });
    let (anchor_chunk, anchor_item) = canvas[0];
    let mut anchor = chunks[anchor_chunk][anchor_item]
        .canvas_plane_depth
        .unwrap_or(0.0);
    for &(chunk_index, item_index) in canvas.iter().skip(1) {
        let depth = chunks[chunk_index][item_index]
            .canvas_plane_depth
            .unwrap_or(anchor);
        if (depth - anchor).abs() <= CANVAS_PLANE_SIBLING_SNAP_METERS {
            store_snapped_plane_depth(&mut chunks[chunk_index][item_index], anchor);
        } else {
            anchor = depth;
        }
    }
}

/// Writes one snapped plane distance and the transparent key derived from it.
fn store_snapped_plane_depth(item: &mut WorldMeshDrawItem, depth: f32) {
    if item
        .canvas_plane_depth
        .is_some_and(|current| current.to_bits() == depth.to_bits())
    {
        return;
    }
    item.canvas_plane_depth = Some(depth);
    item.transparent_depth_key = transparent_depth_key_from_parts(
        item.is_canvas_ui,
        item.canvas_plane_depth,
        item.camera_distance_sq,
    );
}

/// Constant reverse-Z NDC bias for Unity `Offset` units plus canvas sibling steps.
///
/// Positive Unity units push away from the camera (negative NDC under reverse-Z). Each canvas
/// step pulls [`CANVAS_NDC_ULPS_PER_STEP`] D24 quanta closer. Units outside
/// [`CANVAS_OFFSET_UNITS_CAP`] and steps above [`CANVAS_DEPTH_STEP_CAP`] clamp.
///
/// This bias is added on top of the hardware `wgpu::DepthBiasState` constant that
/// `MaterialRenderState::depth_bias` already builds from the same units. Near the camera each
/// Offset unit is about five depth quanta: one hardware D24 unit plus these four NDC quanta.
/// The hardware constant is not a second copy of this NDC term. It stays so the slope term and
/// the constant-bias pipeline tests remain, and it stops moving `Depth32Float` away from the
/// near plane.
#[inline]
pub(crate) fn reverse_z_constant_ndc_bias(units: i32, canvas_steps: u32) -> f32 {
    let units = units.clamp(-CANVAS_OFFSET_UNITS_CAP, CANVAS_OFFSET_UNITS_CAP) as f32;
    let steps = canvas_steps.min(CANVAS_DEPTH_STEP_CAP) as f32;
    let quanta = REVERSE_Z_D24_NDC_UNIT * CANVAS_NDC_ULPS_PER_STEP as f32;
    (-units + steps) * quanta
}

/// Projection family used when a closer NDC bias is turned into meters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CanvasBiasProjection {
    /// Reverse-Z perspective. The pull grows roughly with the square of distance.
    Perspective,
    /// Reverse-Z orthographic. The pull is `ndc_delta * (far - near)` at every fragment.
    Orthographic,
}

/// Scales a closer-direction NDC bias so its view-space pull stays within
/// [`CANVAS_CLOSER_BIAS_CAP_METERS`].
///
/// Zero and away-from-camera biases (negative NDC) are unchanged. Non-finite clip planes leave
/// the bias unchanged. For [`CanvasBiasProjection::Perspective`], `distance` is the shared plane
/// distance and the cap is the pull there; fragments closer than the plane move less. For
/// [`CanvasBiasProjection::Orthographic`] the pull does not depend on `distance`.
#[inline]
pub(crate) fn cap_closer_ndc_bias(
    ndc_bias: f32,
    distance: f32,
    near: f32,
    far: f32,
    projection: CanvasBiasProjection,
) -> f32 {
    if !(ndc_bias.is_finite() && ndc_bias > 0.0) {
        return ndc_bias;
    }
    let pull = match projection {
        CanvasBiasProjection::Perspective => {
            reverse_z_ndc_closer_shift_meters(near, far, distance, ndc_bias)
        }
        CanvasBiasProjection::Orthographic => {
            reverse_z_ortho_closer_shift_meters(near, far, ndc_bias)
        }
    };
    if !pull.is_finite() || pull <= CANVAS_CLOSER_BIAS_CAP_METERS {
        return ndc_bias;
    }
    let scaled = f64::from(ndc_bias) * (f64::from(CANVAS_CLOSER_BIAS_CAP_METERS) / f64::from(pull));
    if scaled.is_finite() {
        scaled as f32
    } else {
        ndc_bias
    }
}

/// Shared plane distance used to size the perspective cap.
///
/// A finite non-negative canvas plane depth is the key every sibling on that plane already
/// shares. A missing or non-finite plane falls back to the farthest-corner distance, then to
/// `far`. A plane at the origin (`0`) is kept: the perspective pull is undefined there, and the
/// cap leaves the bias unchanged instead of shrinking it to a far corner.
fn bias_reference_distance(plane_depth: Option<f32>, distance_sq: f32, far: f32) -> f32 {
    if let Some(depth) = plane_depth.filter(|depth| depth.is_finite() && *depth >= 0.0) {
        return depth;
    }
    if distance_sq.is_finite() && distance_sq > 0.0 {
        let distance = distance_sq.sqrt();
        if distance.is_finite() && distance > 0.0 {
            return distance;
        }
    }
    if far.is_finite() && far > 0.0 {
        far
    } else {
        0.0
    }
}

/// How far a positive NDC Z bias moves a fragment toward the camera under reverse-Z orthographic.
///
/// `ndc = (far - distance) / (far - near)`, so adding `ndc_delta` moves the sample by
/// `ndc_delta * (far - near)` meters at every distance. The perspective formula grows roughly
/// with the square of distance and overestimates this pull. Non-finite inputs and `far <= near`
/// return `0.0`.
fn reverse_z_ortho_closer_shift_meters(near: f32, far: f32, ndc_delta: f32) -> f32 {
    if !(near.is_finite() && far.is_finite() && ndc_delta.is_finite() && far > near) {
        return 0.0;
    }
    let pull = f64::from(ndc_delta) * (f64::from(far) - f64::from(near));
    if pull.is_finite() { pull as f32 } else { 0.0 }
}

/// How far a positive NDC Z bias moves a view-space point toward the camera, in meters.
///
/// Uses the renderer's finite reverse-Z perspective (`ndc = -z2 + z3 / distance` with
/// `z2 = near / (far - near)` and `z3 = far * near / (far - near)`). The result is positive when
/// `ndc_delta` moves the sample closer. Non-finite inputs return `0.0`.
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
///
/// `near` and `far` are the clip planes of the projection this draw will use. The closer
/// direction is capped at [`CANVAS_CLOSER_BIAS_CAP_METERS`] measured at the shared plane
/// distance. `projection` selects the perspective or orthographic pull.
#[inline]
pub(crate) fn canvas_ndc_z_bias(
    item: &WorldMeshDrawItem,
    canvas_steps: u32,
    near: f32,
    far: f32,
    projection: CanvasBiasProjection,
) -> f32 {
    if !item.is_canvas_ui {
        return 0.0;
    }
    let units = item
        .batch_key
        .render_state
        .depth_offset
        .map(|offset| offset.units())
        .unwrap_or(0);
    let raw = reverse_z_constant_ndc_bias(units, canvas_steps);
    let distance = bias_reference_distance(item.canvas_plane_depth, item.camera_distance_sq, far);
    cap_closer_ndc_bias(raw, distance, near, far, projection)
}

/// Whether this draw's view-projection is the orthographic family.
///
/// An overlay draw with an overlay projection uses that orthographic matrix. An overlay draw
/// without one falls back to the world projection, and a world draw always uses it, so both
/// follow `world_is_orthographic`.
#[inline]
pub(crate) fn canvas_bias_projection(
    is_overlay: bool,
    overlay_projection_present: bool,
    world_is_orthographic: bool,
) -> CanvasBiasProjection {
    let orthographic = if is_overlay && overlay_projection_present {
        true
    } else {
        world_is_orthographic
    };
    if orthographic {
        CanvasBiasProjection::Orthographic
    } else {
        CanvasBiasProjection::Perspective
    }
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
        CANVAS_CLOSER_BIAS_CAP_METERS, CANVAS_DEPTH_STEP_CAP,
        CANVAS_NDC_SHIFT_REFERENCE_FAR_METERS, CANVAS_NDC_ULPS_PER_STEP, CANVAS_OFFSET_UNITS_CAP,
        CANVAS_PLANE_DEPTH_QUANTUM_METERS, CanvasBiasProjection, apply_reverse_z_ndc_bias,
        assign_canvas_depth_steps, canvas_bias_projection, canvas_ndc_z_bias,
        canvas_plane_depth_quantum, canvas_plane_distance_meters, cap_closer_ndc_bias,
        quantize_canvas_plane_depth, reverse_z_constant_ndc_bias,
        reverse_z_ndc_closer_shift_meters, reverse_z_ortho_closer_shift_meters,
        snap_canvas_sibling_plane_depths, sync_transparent_depth_key,
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

        // Uncapped 64 steps at near 0.01 and 20 m is about 61 cm. The applied bias stops at 2 cm.
        let raw_far = reverse_z_constant_ndc_bias(0, CANVAS_DEPTH_STEP_CAP);
        let capped =
            cap_closer_ndc_bias(raw_far, 20.0, 0.01, far, CanvasBiasProjection::Perspective);
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
        let perspective =
            cap_closer_ndc_bias(raw, 20.0, near, far, CanvasBiasProjection::Perspective);
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
}
