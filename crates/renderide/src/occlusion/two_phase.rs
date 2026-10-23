//! CPU temporal hysteresis on the one-frame-late Hi-Z pyramid.
//!
//! This is not same-frame two-phase occlusion. The GPU pyramid is still the previous frame's
//! depth. Rejecting every caster that fails that test pops objects when a camera cut, a moving
//! occluder, or a newly revealed surface disagrees with last frame's depth, and it oscillates
//! casters whose own previous depth covers their bounds.
//!
//! Previously visible casters, and casters with no history, are drawn. A previously occluded
//! caster is rejected only when its bounds and mesh are unchanged, its screen rectangle misses
//! every disocclusion region, and the previous pyramid still covers the rectangle after that
//! rectangle is expanded by the screen motion between the Hi-Z camera and the current camera.
//! Large motion and a failed projection fail open and keep the draw. A visible caster becomes
//! occluded for the *next* frame only when the covering surface is closer in linear view depth
//! by more than [`other_occluder_margin`].
//!
//! A previously visible caster that is absent this frame (hidden, removed, or toggled off) and a
//! mesh swap that leaves the AABB unchanged both emit disocclusion rectangles. A depth
//! contributor that was not a tracked visible caster can still leave one frame where the stale
//! pyramid hides whatever was behind it, because this pass has no remembered rectangle for it.

use std::sync::Arc;

use glam::{Mat4, Vec3};
use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::hi_z_cpu::project_world_aabb_uv;
use crate::scene::RenderSpaceId;

/// Minimum view-space gap, in meters, before a covering surface counts as a different occluder.
const OTHER_OCCLUDER_MIN_METERS: f32 = 0.05;

/// Extra gap, as a fraction of the caster's view depth, added on top of [`OTHER_OCCLUDER_MIN_METERS`].
const OTHER_OCCLUDER_RELATIVE: f32 = 0.02;

/// World-space AABB epsilon. A larger change may have carried the caster out from behind an occluder.
const BOUNDS_MATCH_EPSILON: f32 = 1.0e-4;

/// UV padding added around a moved caster's old and new footprints.
const DISOCCLUSION_UV_PAD: f32 = 5.0e-3;

/// Largest scale applied when a closer occluder needs a wider motion pad.
const PARALLAX_SCALE_MAX: f32 = 8.0;

/// Parallax scales below this leave the base pad unchanged.
const PARALLAX_RESAMPLE_MIN: f32 = 1.05;

/// Stable identity of one occlusion-culled caster across frames.
///
/// Material slots share one key. [`TwoPhaseFrame::finish`] keeps a `Visible` classification when
/// any slot reports one, and unions their bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CasterKey {
    /// Render space that owns the caster.
    pub space_id: i32,
    /// Index into the static or skinned renderer list.
    pub renderable_index: u64,
    /// Renderer-local id that survives dense reindexing.
    pub instance_id: u64,
    /// Scene node id.
    pub node_id: i32,
    /// `true` when [`Self::renderable_index`] addresses the skinned list.
    pub skinned: bool,
}

impl CasterKey {
    /// Builds a key from the prepared-draw identity fields.
    pub(crate) fn new(
        space_id: RenderSpaceId,
        renderable_index: usize,
        instance_id: u64,
        node_id: i32,
        skinned: bool,
    ) -> Self {
        Self {
            space_id: space_id.0,
            renderable_index: u64::try_from(renderable_index).unwrap_or(u64::MAX),
            instance_id,
            node_id,
            skinned,
        }
    }
}

/// Caster identity plus the mesh that produced this frame's surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CasterRef {
    /// Key shared by every material slot of the caster.
    pub key: CasterKey,
    /// Mesh asset. A change with an unchanged AABB is still a new surface.
    pub mesh_asset_id: i32,
}

/// Last committed occlusion class for one caster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisibilityClass {
    /// Drawn last time this caster was inside the frustum.
    Visible,
    /// Rejected by a confirmation test that the following frame was allowed to trust.
    Occluded,
}

/// Bounds and mesh remembered with [`VisibilityClass`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CasterRecord {
    /// Classification written at the end of the previous tested frame.
    pub class: VisibilityClass,
    /// World AABB minimum at that classification.
    pub aabb_min: Vec3,
    /// World AABB maximum at that classification.
    pub aabb_max: Vec3,
    /// Mesh asset at that classification.
    pub mesh_asset_id: i32,
}

/// Which phase produced this frame's draw-or-reject decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhaseAction {
    /// Previously visible or unknown. Drawn, and not rejected by the stale pyramid.
    FirstPhase,
    /// Previously occluded, but current-frame confirmation says it may be visible.
    SecondPhase,
    /// Previously occluded, and current-frame confirmation still holds.
    Culled,
}

/// Result of [`decide_phase`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhaseDecision {
    /// Draw phase, or a confirmed rejection.
    pub action: PhaseAction,
    /// Class stored for the next frame when this caster is inside the frustum.
    pub next_class: VisibilityClass,
}

/// Inputs for one caster's hysteresis decision.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PhaseInputs<'a> {
    /// Missing history means the caster is newly revealed.
    pub previous: Option<&'a CasterRecord>,
    /// World bounds or the mesh asset changed since the stored record.
    pub caster_moved: bool,
    /// Screen rectangle hits a region where a visible caster moved, hid, or swapped meshes.
    pub overlaps_disocclusion: bool,
    /// Previous pyramid fully occludes the motion-expanded rectangle.
    pub hi_z_occludes: bool,
    /// Covering depth is closer than this caster's own front by [`other_occluder_margin`].
    pub other_occluder: bool,
}

/// Near and far clip used to convert reverse-Z NDC into linear view depth.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DepthClip {
    /// Positive near distance in view space.
    pub near: f32,
    /// Positive far distance in view space.
    pub far: f32,
    /// `true` for a reverse-Z orthographic projection.
    pub orthographic: bool,
}

/// Previous and current view-projections for one space, plus the clip used for linear depth.
///
/// Desktop stores `world_proj * space_view` in [`Self::mono`]. Stereo stores the full per-eye
/// view-projections in [`Self::stereo`]. A missing entry means the previous pyramid cannot be
/// compared with this frame, so the caster is kept.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpaceMotion {
    /// Previous and current mono view-projection.
    pub mono: Option<(Mat4, Mat4)>,
    /// Previous `(left, right)` and current `(left, right)` stereo view-projections.
    pub stereo: Option<((Mat4, Mat4), (Mat4, Mat4))>,
    /// Clip planes of the current camera.
    pub clip: DepthClip,
}

/// Axis-aligned UV rectangle in the unit square.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScreenRect {
    /// Minimum U.
    pub min_u: f32,
    /// Minimum V.
    pub min_v: f32,
    /// Maximum U.
    pub max_u: f32,
    /// Maximum V.
    pub max_v: f32,
}

impl ScreenRect {
    /// Clamps a projected UV pair into the unit square.
    pub(crate) fn from_uv(uv_min: (f32, f32), uv_max: (f32, f32)) -> Option<Self> {
        if !uv_min.0.is_finite()
            || !uv_min.1.is_finite()
            || !uv_max.0.is_finite()
            || !uv_max.1.is_finite()
        {
            return None;
        }
        let min_u = uv_min.0.min(uv_max.0);
        let max_u = uv_min.0.max(uv_max.0);
        let min_v = uv_min.1.min(uv_max.1);
        let max_v = uv_min.1.max(uv_max.1);
        if max_u < 0.0 || min_u > 1.0 || max_v < 0.0 || min_v > 1.0 {
            return None;
        }
        Some(Self {
            min_u: min_u.clamp(0.0, 1.0),
            min_v: min_v.clamp(0.0, 1.0),
            max_u: max_u.clamp(0.0, 1.0),
            max_v: max_v.clamp(0.0, 1.0),
        })
    }

    /// Expands the rectangle by `pad` UV units and clamps it to the unit square.
    pub(crate) fn padded(self, pad: f32) -> Self {
        Self {
            min_u: (self.min_u - pad).clamp(0.0, 1.0),
            min_v: (self.min_v - pad).clamp(0.0, 1.0),
            max_u: (self.max_u + pad).clamp(0.0, 1.0),
            max_v: (self.max_v + pad).clamp(0.0, 1.0),
        }
    }

    /// Inclusive overlap, including edge contact.
    pub(crate) fn overlaps(self, other: Self) -> bool {
        self.min_u <= other.max_u
            && other.min_u <= self.max_u
            && self.min_v <= other.max_v
            && other.min_v <= self.max_v
    }
}

/// Depth and screen coverage used to confirm a previous-frame Hi-Z hit.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OcclusionFacts {
    /// Previous pyramid fully occludes every tested eye after motion expansion.
    pub occluded: bool,
    /// A surface closer than this caster's own front covers that same rectangle.
    pub other_occluder: bool,
    /// Up to two eye rectangles. `None` slots are unused. These stay unpadded.
    pub rects: [Option<ScreenRect>; 2],
}

impl OcclusionFacts {
    /// No pyramid test applied. The caster stays eligible to draw.
    pub(crate) fn keep() -> Self {
        Self {
            occluded: false,
            other_occluder: false,
            rects: [None, None],
        }
    }
}

/// History snapshot plus the cleared map the next [`TwoPhaseFrame::finish`] refills.
pub(crate) struct VisibilityScratch {
    /// Classification published by the previous tested frame.
    pub history: Arc<VisibilityHistory>,
    /// Cleared caster map retained from an earlier publication.
    pub spare: HashMap<CasterKey, CasterRecord>,
}

/// Per-view classification carried from the previous tested frame.
#[derive(Clone, Debug, Default)]
pub(crate) struct VisibilityHistory {
    casters: HashMap<CasterKey, CasterRecord>,
}

impl VisibilityHistory {
    /// Builds a history from an existing caster map.
    pub(crate) fn from_casters(casters: HashMap<CasterKey, CasterRecord>) -> Self {
        Self { casters }
    }

    /// Record stored for `key`, if this caster was tested last frame.
    pub(crate) fn get(&self, key: &CasterKey) -> Option<&CasterRecord> {
        self.casters.get(key)
    }

    /// Number of remembered casters.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.casters.len()
    }

    /// Allocated capacity of the caster map.
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.casters.capacity()
    }

    /// Visits every remembered caster. Order is not significant.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&CasterKey, &CasterRecord)> {
        self.casters.iter()
    }

    /// Releases the caster map so the next frame can refill it.
    pub(crate) fn into_casters(self) -> HashMap<CasterKey, CasterRecord> {
        self.casters
    }
}

/// One caster whose previous visible footprint must not be trusted this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MovedCaster {
    /// Render space the bounds belong to.
    pub space_id: RenderSpaceId,
    /// Bounds remembered from the frame that wrote the pyramid.
    pub previous_aabb: (Vec3, Vec3),
    /// Bounds this frame. Equal to [`Self::previous_aabb`] when the caster is absent.
    pub current_aabb: (Vec3, Vec3),
}

/// One frustum-tested caster visible to the current view.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PresentCaster {
    /// Shared caster identity.
    pub key: CasterKey,
    /// Mesh asset drawn this frame.
    pub mesh_asset_id: i32,
    /// World AABB minimum.
    pub aabb_min: Vec3,
    /// World AABB maximum.
    pub aabb_max: Vec3,
    /// Render space that owns the caster.
    pub space_id: RenderSpaceId,
}

/// Read-only history plus this frame's motion bounds, disocclusion regions, and update shards.
pub(crate) struct TwoPhaseFrame {
    history: Arc<VisibilityHistory>,
    motions: HashMap<RenderSpaceId, SpaceMotion>,
    stale_rects: Vec<ScreenRect>,
    updates: Vec<Mutex<Vec<(CasterKey, CasterRecord)>>>,
    spare: HashMap<CasterKey, CasterRecord>,
}

impl TwoPhaseFrame {
    /// Builds the frame plan consulted by every fine cull in one view.
    ///
    /// `spare` is a cleared map from the previous publication. [`Self::finish`] refills it
    /// instead of allocating a new one. Update shards are one buffer per rayon worker, plus one
    /// for callers outside the pool.
    pub(crate) fn new(
        history: Arc<VisibilityHistory>,
        spare: HashMap<CasterKey, CasterRecord>,
        motions: HashMap<RenderSpaceId, SpaceMotion>,
        stale_rects: Vec<ScreenRect>,
    ) -> Self {
        let threads = rayon::current_num_threads().saturating_add(1);
        let updates = (0..threads).map(|_| Mutex::new(Vec::new())).collect();
        Self {
            history,
            motions,
            stale_rects,
            updates,
            spare,
        }
    }

    /// Previous classification for `key`.
    pub(crate) fn previous(&self, key: &CasterKey) -> Option<&CasterRecord> {
        self.history.get(key)
    }

    /// Clip planes used to interpret Hi-Z depth in `space_id`.
    pub(crate) fn depth_clip(&self, space_id: RenderSpaceId) -> Option<DepthClip> {
        self.motions.get(&space_id).map(|motion| motion.clip)
    }

    /// UV expansion that keeps a small camera motion from disabling the occlusion test.
    ///
    /// `None` when this space has no comparable cameras or either projection fails. Callers then
    /// keep the draw.
    pub(crate) fn motion_pad(&self, space_id: RenderSpaceId, min: Vec3, max: Vec3) -> Option<f32> {
        let motion = self.motions.get(&space_id)?;
        if motion.stereo.is_some() {
            stereo_motion_pad(motion, min, max)
        } else {
            mono_motion_pad(motion, min, max)
        }
    }

    /// Decides whether `caster` is drawn and records the class to commit at the end of the view.
    ///
    /// A missing `caster` id cannot be tracked, so the draw is kept.
    pub(crate) fn allows_draw(
        &self,
        caster: Option<CasterRef>,
        aabb_min: Vec3,
        aabb_max: Vec3,
        facts: OcclusionFacts,
    ) -> bool {
        let Some(caster) = caster else {
            return true;
        };
        let previous = self.previous(&caster.key);
        let caster_moved = previous.is_some_and(|record| {
            record.mesh_asset_id != caster.mesh_asset_id
                || bounds_moved((record.aabb_min, record.aabb_max), (aabb_min, aabb_max))
        });
        let decision = decide_phase(PhaseInputs {
            previous,
            caster_moved,
            overlaps_disocclusion: rects_hit_disocclusion(facts.rects, &self.stale_rects),
            hi_z_occludes: facts.occluded,
            other_occluder: facts.other_occluder,
        });
        self.shard().lock().push((
            caster.key,
            CasterRecord {
                class: decision.next_class,
                aabb_min,
                aabb_max,
                mesh_asset_id: caster.mesh_asset_id,
            },
        ));
        decision.action != PhaseAction::Culled
    }

    /// Replaces the view history with the casters tested this frame.
    ///
    /// Casters that missed the frustum are absent, so the next time they enter they are treated
    /// as newly revealed and drawn. Duplicate material-slot updates collapse here: `Visible`
    /// wins, and the bounds are the union.
    pub(crate) fn finish(self) -> VisibilityHistory {
        let Self {
            history,
            updates,
            mut spare,
            ..
        } = self;
        drop(history);
        spare.clear();
        let total = updates.iter().map(|shard| shard.lock().len()).sum();
        spare.reserve(total);
        for shard in updates {
            for (key, record) in shard.into_inner() {
                merge_record(&mut spare, key, record);
            }
        }
        VisibilityHistory { casters: spare }
    }

    fn shard(&self) -> &Mutex<Vec<(CasterKey, CasterRecord)>> {
        let index = rayon::current_thread_index().map_or(0, |thread| thread.saturating_add(1));
        let index = index.min(self.updates.len().saturating_sub(1));
        &self.updates[index]
    }
}

/// Classifies one frustum-visible caster.
pub(crate) fn decide_phase(inputs: PhaseInputs<'_>) -> PhaseDecision {
    if inputs.previous.map(|record| record.class) != Some(VisibilityClass::Occluded) {
        return PhaseDecision {
            action: PhaseAction::FirstPhase,
            next_class: next_class_for_visible_set(&inputs),
        };
    }
    if phase2_rejection_holds(&inputs) {
        PhaseDecision {
            action: PhaseAction::Culled,
            next_class: VisibilityClass::Occluded,
        }
    } else {
        PhaseDecision {
            action: PhaseAction::SecondPhase,
            next_class: VisibilityClass::Visible,
        }
    }
}

/// `true` when the world AABB changed enough that last frame's occlusion no longer applies.
pub(crate) fn bounds_moved(previous: (Vec3, Vec3), current: (Vec3, Vec3)) -> bool {
    (previous.0 - current.0).abs().max_element() > BOUNDS_MATCH_EPSILON
        || (previous.1 - current.1).abs().max_element() > BOUNDS_MATCH_EPSILON
}

/// Linear view-space depth for a reverse-Z NDC z, using the projection's near and far.
///
/// Perspective uses `far * near / (ndc_z * (far - near) + near)`. Orthographic uses
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
/// `None` when either projection fails, which includes a box that left the frustum. Identical
/// cameras return a pad near zero.
pub(crate) fn screen_motion_uv_pad(
    previous_vp: Mat4,
    current_vp: Mat4,
    aabb_min: Vec3,
    aabb_max: Vec3,
) -> Option<f32> {
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

/// UV pad after widening `base_pad` for a closer occluder's parallax.
///
/// Returns `base_pad` when the covering surface is not closer, or the scale is negligible.
/// `None` when the scaled pad is not finite. A scale above [`PARALLAX_RESAMPLE_MIN`] is clamped
/// to [`PARALLAX_SCALE_MAX`].
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
    let Some(covering_ndc_z) = covering_ndc_z else {
        return Some(base_pad);
    };
    let Some(caster) = view_depth_from_reverse_z(closest_ndc_z, clip) else {
        return Some(base_pad);
    };
    let Some(covering) = view_depth_from_reverse_z(covering_ndc_z, clip) else {
        return Some(base_pad);
    };
    if !covering.is_finite() || covering <= 0.0 || covering >= caster {
        return Some(base_pad);
    }
    let scale = (caster / covering).clamp(1.0, PARALLAX_SCALE_MAX);
    if scale < PARALLAX_RESAMPLE_MIN {
        return Some(base_pad);
    }
    let scaled = base_pad * scale;
    scaled.is_finite().then_some(scaled)
}

/// Casters whose previous visible footprint must punch a hole in this frame's confirmation.
///
/// A present visible caster is included when its bounds move or its mesh asset changes. A
/// previously visible caster missing from `present`, and inside `include_space`, is included with
/// its remembered bounds. Previously occluded absences are not disoccluders.
pub(crate) fn plan_disocclusion(
    history: &VisibilityHistory,
    present: &[PresentCaster],
    mut include_space: impl FnMut(RenderSpaceId) -> bool,
) -> Vec<MovedCaster> {
    let mut present_keys = hashbrown::HashSet::with_capacity(present.len());
    let mut movers = Vec::new();
    for caster in present {
        if !include_space(caster.space_id) {
            continue;
        }
        present_keys.insert(caster.key);
        let Some(previous) = history.get(&caster.key) else {
            continue;
        };
        if previous.class != VisibilityClass::Visible {
            continue;
        }
        let current = (caster.aabb_min, caster.aabb_max);
        let remembered = (previous.aabb_min, previous.aabb_max);
        let surface_changed = previous.mesh_asset_id != caster.mesh_asset_id;
        if !surface_changed && !bounds_moved(remembered, current) {
            continue;
        }
        movers.push(MovedCaster {
            space_id: caster.space_id,
            previous_aabb: remembered,
            current_aabb: current,
        });
    }
    for (key, record) in history.iter() {
        if record.class != VisibilityClass::Visible || present_keys.contains(key) {
            continue;
        }
        let space_id = RenderSpaceId(key.space_id);
        if !include_space(space_id) {
            continue;
        }
        let aabb = (record.aabb_min, record.aabb_max);
        movers.push(MovedCaster {
            space_id,
            previous_aabb: aabb,
            current_aabb: aabb,
        });
    }
    movers
}

/// Projects moved or removed visible casters into padded disocclusion rectangles.
pub(crate) fn disocclusion_rects<F>(movers: &[MovedCaster], mut project: F) -> Vec<ScreenRect>
where
    F: FnMut(Vec3, Vec3) -> Option<ScreenRect>,
{
    let mut rects = Vec::new();
    for mover in movers {
        push_padded(
            &mut rects,
            project(mover.previous_aabb.0, mover.previous_aabb.1),
        );
        push_padded(
            &mut rects,
            project(mover.current_aabb.0, mover.current_aabb.1),
        );
    }
    rects
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

fn push_padded(rects: &mut Vec<ScreenRect>, rect: Option<ScreenRect>) {
    if let Some(rect) = rect {
        rects.push(rect.padded(DISOCCLUSION_UV_PAD));
    }
}

fn next_class_for_visible_set(inputs: &PhaseInputs<'_>) -> VisibilityClass {
    if !inputs.caster_moved && !inputs.overlaps_disocclusion && inputs.other_occluder {
        VisibilityClass::Occluded
    } else {
        VisibilityClass::Visible
    }
}

fn phase2_rejection_holds(inputs: &PhaseInputs<'_>) -> bool {
    !inputs.caster_moved && !inputs.overlaps_disocclusion && inputs.hi_z_occludes
}

fn rects_hit_disocclusion(rects: [Option<ScreenRect>; 2], stale: &[ScreenRect]) -> bool {
    if stale.is_empty() {
        return false;
    }
    rects
        .into_iter()
        .flatten()
        .any(|rect| stale.iter().any(|stale_rect| rect.overlaps(*stale_rect)))
}

fn merge_record(
    map: &mut HashMap<CasterKey, CasterRecord>,
    key: CasterKey,
    incoming: CasterRecord,
) {
    match map.entry(key) {
        hashbrown::hash_map::Entry::Vacant(slot) => {
            slot.insert(incoming);
        }
        hashbrown::hash_map::Entry::Occupied(mut slot) => {
            let existing = slot.get_mut();
            existing.aabb_min = existing.aabb_min.min(incoming.aabb_min);
            existing.aabb_max = existing.aabb_max.max(incoming.aabb_max);
            if incoming.class == VisibilityClass::Visible {
                existing.class = VisibilityClass::Visible;
                existing.mesh_asset_id = incoming.mesh_asset_id;
            } else if existing.class != VisibilityClass::Visible {
                existing.mesh_asset_id = incoming.mesh_asset_id;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use glam::Mat4;
    use rayon::prelude::*;

    use super::*;
    use crate::camera::reverse_z_perspective;

    fn record(class: VisibilityClass, min: Vec3, max: Vec3) -> CasterRecord {
        CasterRecord {
            class,
            aabb_min: min,
            aabb_max: max,
            mesh_asset_id: 1,
        }
    }

    fn aabb() -> (Vec3, Vec3) {
        (Vec3::new(-0.5, -0.5, 2.0), Vec3::new(0.5, 0.5, 3.0))
    }

    fn inputs<'a>(previous: Option<&'a CasterRecord>, occludes: bool) -> PhaseInputs<'a> {
        PhaseInputs {
            previous,
            caster_moved: false,
            overlaps_disocclusion: false,
            hi_z_occludes: occludes,
            other_occluder: false,
        }
    }

    fn caster_ref(key: CasterKey) -> CasterRef {
        CasterRef {
            key,
            mesh_asset_id: 1,
        }
    }

    fn perspective_clip(near: f32, far: f32) -> DepthClip {
        DepthClip {
            near,
            far,
            orthographic: false,
        }
    }

    fn ndc_from_view_depth(depth: f32, clip: DepthClip) -> f32 {
        (clip.far * clip.near / depth - clip.near) / (clip.far - clip.near)
    }

    #[test]
    fn newly_revealed_caster_is_drawn_even_when_stale_hiz_occludes() {
        let decision = decide_phase(inputs(None, true));
        assert_eq!(decision.action, PhaseAction::FirstPhase);
        assert_eq!(decision.next_class, VisibilityClass::Visible);
    }

    #[test]
    fn moving_occluder_keeps_previously_occluded_caster() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let mut query = inputs(Some(&previous), true);
        query.overlaps_disocclusion = true;
        let decision = decide_phase(query);
        assert_eq!(decision.action, PhaseAction::SecondPhase);
        assert_eq!(decision.next_class, VisibilityClass::Visible);
    }

    #[test]
    fn camera_cut_keeps_previously_occluded_caster() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let mut query = inputs(Some(&previous), true);
        query.hi_z_occludes = false;
        let decision = decide_phase(query);
        assert_eq!(decision.action, PhaseAction::SecondPhase);
        assert_ne!(decision.action, PhaseAction::Culled);
    }

    #[test]
    fn stable_occluded_caster_stays_culled() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let decision = decide_phase(inputs(Some(&previous), true));
        assert_eq!(decision.action, PhaseAction::Culled);
        assert_eq!(decision.next_class, VisibilityClass::Occluded);
    }

    #[test]
    fn visible_caster_is_not_rejected_by_its_own_depth() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Visible, min, max);
        let decision = decide_phase(inputs(Some(&previous), true));
        assert_eq!(decision.action, PhaseAction::FirstPhase);
        assert_eq!(decision.next_class, VisibilityClass::Visible);
    }

    #[test]
    fn moved_caster_behind_stale_hiz_is_drawn() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Occluded, min, max);
        let mut query = inputs(Some(&previous), true);
        query.caster_moved = true;
        assert_eq!(decide_phase(query).action, PhaseAction::SecondPhase);
    }

    #[test]
    fn separate_occluder_schedules_visible_caster_for_the_next_frame() {
        let (min, max) = aabb();
        let previous = record(VisibilityClass::Visible, min, max);
        let mut query = inputs(Some(&previous), true);
        query.other_occluder = true;
        let decision = decide_phase(query);
        assert_eq!(decision.action, PhaseAction::FirstPhase);
        assert_eq!(decision.next_class, VisibilityClass::Occluded);
    }

    #[test]
    fn probe_disocclusion_never_culls_a_visible_caster() {
        let (min, max) = aabb();
        let occluded = record(VisibilityClass::Occluded, min, max);
        let visible = record(VisibilityClass::Visible, min, max);
        let cases = [
            ("newly revealed", inputs(None, true)),
            ("moving occluder", {
                let mut query = inputs(Some(&occluded), true);
                query.overlaps_disocclusion = true;
                query
            }),
            ("camera cut", {
                let mut query = inputs(Some(&occluded), true);
                query.hi_z_occludes = false;
                query
            }),
            ("self occlusion", inputs(Some(&visible), true)),
            ("caster slid out", {
                let mut query = inputs(Some(&occluded), true);
                query.caster_moved = true;
                query
            }),
        ];
        for (name, query) in cases {
            let decision = decide_phase(query);
            assert_ne!(
                decision.action,
                PhaseAction::Culled,
                "{name} culled a caster the current frame can still show"
            );
        }
    }

    #[test]
    fn frame_sink_keeps_disoccluded_caster_and_culls_stable_one() {
        let (min, max) = aabb();
        let mut history = VisibilityHistory::default();
        let hidden = CasterKey::new(RenderSpaceId(1), 3, 9, 4, false);
        let revealed = CasterKey::new(RenderSpaceId(1), 4, 10, 5, false);
        history
            .casters
            .insert(hidden, record(VisibilityClass::Occluded, min, max));
        let history = Arc::new(history);
        let frame = TwoPhaseFrame::new(history, HashMap::new(), HashMap::new(), Vec::new());
        let facts = OcclusionFacts {
            occluded: true,
            other_occluder: false,
            rects: [None, None],
        };
        assert!(!frame.allows_draw(Some(caster_ref(hidden)), min, max, facts));
        assert!(frame.allows_draw(Some(caster_ref(revealed)), min, max, facts));
        let committed = frame.finish();
        assert_eq!(committed.len(), 2);
        assert_eq!(
            committed.get(&hidden).map(|record| record.class),
            Some(VisibilityClass::Occluded)
        );
        assert_eq!(
            committed.get(&revealed).map(|record| record.class),
            Some(VisibilityClass::Visible)
        );
    }

    #[test]
    fn visible_slot_wins_when_material_slots_share_a_key() {
        let (min, max) = aabb();
        let key = CasterKey::new(RenderSpaceId(1), 3, 9, 4, false);
        let frame = TwoPhaseFrame::new(
            Arc::new(VisibilityHistory::default()),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
        );
        let occluded = OcclusionFacts {
            occluded: true,
            other_occluder: true,
            rects: [None, None],
        };
        let visible = OcclusionFacts::keep();
        assert!(frame.allows_draw(Some(caster_ref(key)), min, max, occluded));
        assert!(frame.allows_draw(
            Some(CasterRef {
                key,
                mesh_asset_id: 7,
            }),
            min + Vec3::splat(0.2),
            max + Vec3::splat(0.2),
            visible,
        ));
        let committed = frame.finish();
        let stored = committed.get(&key).expect("merged caster");
        assert_eq!(stored.class, VisibilityClass::Visible);
        assert_eq!(stored.mesh_asset_id, 7);
        assert_eq!(stored.aabb_min, min);
        assert_eq!(stored.aabb_max, max + Vec3::splat(0.2));
    }

    #[test]
    fn spare_map_capacity_survives_finish() {
        let mut spare = HashMap::with_capacity(64);
        spare.insert(
            CasterKey::new(RenderSpaceId(1), 0, 0, 0, false),
            record(VisibilityClass::Visible, Vec3::ZERO, Vec3::ONE),
        );
        spare.clear();
        let capacity = spare.capacity();
        let frame = TwoPhaseFrame::new(
            Arc::new(VisibilityHistory::default()),
            spare,
            HashMap::new(),
            Vec::new(),
        );
        let committed = frame.finish();
        assert!(committed.capacity() >= capacity);
    }

    #[test]
    fn parallel_updates_merge_into_one_history() {
        let frame = TwoPhaseFrame::new(
            Arc::new(VisibilityHistory::default()),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
        );
        let facts = OcclusionFacts::keep();
        let (min, max) = aabb();
        {
            let frame = &frame;
            (0..64_u64).into_par_iter().for_each(|index| {
                let key = CasterKey::new(
                    RenderSpaceId(1),
                    usize::try_from(index).unwrap_or(0),
                    index,
                    i32::try_from(index).unwrap_or(i32::MAX),
                    false,
                );
                assert!(frame.allows_draw(Some(caster_ref(key)), min, max, facts));
            });
        }
        assert_eq!(frame.finish().len(), 64);
    }

    #[test]
    fn distant_caster_uses_a_linear_margin_with_a_close_near_plane() {
        let clip = perspective_clip(0.01, 1_000.0);
        let caster_depth = 20.0;
        let caster_ndc = ndc_from_view_depth(caster_depth, clip);
        let within_margin = ndc_from_view_depth(19.7, clip);
        let separate = ndc_from_view_depth(19.5, clip);
        assert!(
            (separate - caster_ndc).abs() < 0.02,
            "a 0.02 reverse-Z NDC gap cannot see a half-meter separation at 20 m"
        );
        assert!(!covering_depth_is_other_occluder(
            caster_ndc,
            within_margin,
            clip
        ));
        assert!(covering_depth_is_other_occluder(caster_ndc, separate, clip));
        assert!(!covering_depth_is_other_occluder(
            caster_ndc,
            separate,
            DepthClip {
                near: f32::NAN,
                far: 1_000.0,
                orthographic: false,
            }
        ));
    }

    #[test]
    fn screen_motion_pad_is_zero_when_stable_and_open_when_unbounded() {
        let near = 0.01;
        let far = 100.0;
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, -5.0), Vec3::ZERO, Vec3::Y);
        let narrow = reverse_z_perspective(1.0, 90.0_f32.to_radians(), near, far) * view;
        let wide = reverse_z_perspective(1.0, 60.0_f32.to_radians(), near, far) * view;
        let min = Vec3::splat(-0.25);
        let max = Vec3::splat(0.25);
        let stable = screen_motion_uv_pad(narrow, narrow, min, max).expect("stable projection");
        assert!(stable < 1.0e-4);
        let changed = screen_motion_uv_pad(narrow, wide, min, max).expect("fov projection");
        assert!(changed > 0.01);
        let behind = reverse_z_perspective(1.0, 90.0_f32.to_radians(), near, far)
            * Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 10.0), Vec3::Y);
        assert!(screen_motion_uv_pad(narrow, behind, min, max).is_none());
    }

    #[test]
    fn bounds_motion_uses_the_world_epsilon() {
        let (min, max) = aabb();
        assert!(!bounds_moved((min, max), (min, max)));
        assert!(bounds_moved((min, max), (min + Vec3::splat(0.05), max)));
    }

    #[test]
    fn disocclusion_rects_cover_old_and_new_footprints() {
        let mover = MovedCaster {
            space_id: RenderSpaceId(1),
            previous_aabb: (Vec3::ZERO, Vec3::ONE),
            current_aabb: (Vec3::splat(2.0), Vec3::splat(3.0)),
        };
        let rects = disocclusion_rects(&[mover], |min, _max| {
            let anchor = min.x.clamp(0.0, 1.0);
            ScreenRect::from_uv((anchor, 0.2), (anchor, 0.4))
        });
        assert_eq!(rects.len(), 2);
        let probe = ScreenRect {
            min_u: 0.0,
            min_v: 0.25,
            max_u: 0.0,
            max_v: 0.3,
        };
        assert!(rects[0].overlaps(probe));
    }

    #[test]
    fn absent_visible_caster_and_mesh_swap_are_disoccluders() {
        let (min, max) = aabb();
        let hidden = CasterKey::new(RenderSpaceId(1), 1, 1, 1, false);
        let swapped = CasterKey::new(RenderSpaceId(1), 2, 2, 2, false);
        let steady = CasterKey::new(RenderSpaceId(1), 3, 3, 3, false);
        let occluded = CasterKey::new(RenderSpaceId(1), 4, 4, 4, false);
        let mut history = VisibilityHistory::default();
        history
            .casters
            .insert(hidden, record(VisibilityClass::Visible, min, max));
        history
            .casters
            .insert(swapped, record(VisibilityClass::Visible, min, max));
        history
            .casters
            .insert(steady, record(VisibilityClass::Visible, min, max));
        history
            .casters
            .insert(occluded, record(VisibilityClass::Occluded, min, max));
        let present = [
            PresentCaster {
                key: swapped,
                mesh_asset_id: 9,
                aabb_min: min,
                aabb_max: max,
                space_id: RenderSpaceId(1),
            },
            PresentCaster {
                key: steady,
                mesh_asset_id: 1,
                aabb_min: min,
                aabb_max: max,
                space_id: RenderSpaceId(1),
            },
        ];
        let movers = plan_disocclusion(&history, &present, |_| true);
        assert!(movers.iter().any(|mover| {
            mover.previous_aabb == (min, max) && mover.current_aabb == (min, max)
        }));
        assert!(
            movers
                .iter()
                .any(|mover| mover.space_id == RenderSpaceId(1))
        );
        assert_eq!(movers.len(), 2);
    }
}
