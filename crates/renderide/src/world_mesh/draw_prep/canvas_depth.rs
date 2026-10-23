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
mod tests;
