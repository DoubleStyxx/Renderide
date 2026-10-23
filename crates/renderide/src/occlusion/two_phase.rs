//! Two-phase occlusion decisions on top of the previous-frame Hi-Z pyramid.
//!
//! The GPU pyramid is read back one frame late. Rejecting every caster that fails that test
//! pops objects the moment a camera cut, a moving occluder, or a newly revealed surface
//! disagrees with last frame's depth, and it oscillates casters whose own previous depth
//! covers their bounds.
//!
//! Phase 1 draws casters that were visible (or have no history). They are not rejected by the
//! stale pyramid. Phase 2 retests casters that were occluded. That rejection stands only when
//! the current frame still agrees: the Hi-Z camera matches, the caster has not moved, and its
//! screen rectangle misses every disocclusion region left by a moved visible caster. A visible
//! caster becomes occluded for the *next* frame only when the pyramid is closer than the
//! caster's own front by [`OTHER_OCCLUDER_NDC_GAP`], so self-depth does not flip the class.

use std::sync::Arc;

use glam::{Mat4, Vec3};
use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::scene::RenderSpaceId;

/// Reverse-Z gap past the AABB's closest point required before a covering depth counts as a
/// different occluder. Smaller gaps are treated as the caster's own surface, mip bleed, or shell
/// extrusion.
const OTHER_OCCLUDER_NDC_GAP: f32 = 2.0e-2;

/// Element-wise matrix epsilon. Anything larger, including a camera cut or tracked motion, means
/// the previous pyramid is not confirmation of what the current camera can see.
const CAMERA_MATCH_EPSILON: f32 = 1.0e-4;

/// World-space AABB epsilon. A larger change may have carried the caster out from behind an occluder.
const BOUNDS_MATCH_EPSILON: f32 = 1.0e-4;

/// UV padding added around a moved caster's old and new footprints.
const DISOCCLUSION_UV_PAD: f32 = 5.0e-3;

/// Stable identity of one occlusion-culled caster across frames.
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

/// Last committed occlusion class for one caster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisibilityClass {
    /// Drawn last time this caster was inside the frustum.
    Visible,
    /// Rejected by a phase-2 test that the following frame was allowed to trust.
    Occluded,
}

/// Bounds remembered with [`VisibilityClass`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CasterRecord {
    /// Classification written at the end of the previous tested frame.
    pub class: VisibilityClass,
    /// World AABB minimum at that classification.
    pub aabb_min: Vec3,
    /// World AABB maximum at that classification.
    pub aabb_max: Vec3,
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

/// Inputs for one caster's two-phase decision.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PhaseInputs<'a> {
    /// Missing history means the caster is newly revealed.
    pub previous: Option<&'a CasterRecord>,
    /// Previous Hi-Z camera matches the camera used for this caster's space.
    pub camera_stable: bool,
    /// World bounds moved since the stored record.
    pub caster_moved: bool,
    /// Screen rectangle hits a region where a visible caster moved.
    pub overlaps_disocclusion: bool,
    /// Previous pyramid fully occludes the projected rectangle.
    pub hi_z_occludes: bool,
    /// Covering depth is closer than this caster's own front by [`OTHER_OCCLUDER_NDC_GAP`].
    pub other_occluder: bool,
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
    /// Previous pyramid fully occludes every tested eye.
    pub occluded: bool,
    /// A surface closer than this caster's own front covers that same rectangle.
    pub other_occluder: bool,
    /// Up to two eye rectangles. `None` slots are unused.
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

/// Per-view classification carried from the previous tested frame.
#[derive(Clone, Debug, Default)]
pub(crate) struct VisibilityHistory {
    casters: HashMap<CasterKey, CasterRecord>,
}

impl VisibilityHistory {
    /// Record stored for `key`, if this caster was tested last frame.
    pub(crate) fn get(&self, key: &CasterKey) -> Option<&CasterRecord> {
        self.casters.get(key)
    }

    /// Number of remembered casters.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.casters.len()
    }
}

/// One moved visible caster, in the space of the Hi-Z camera.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MovedCaster {
    /// Bounds remembered from the frame that wrote the pyramid.
    pub previous_aabb: (Vec3, Vec3),
    /// Bounds this frame.
    pub current_aabb: (Vec3, Vec3),
}

/// Read-only history plus the current frame's disocclusion regions and a sink for the next history.
pub(crate) struct TwoPhaseFrame {
    history: Arc<VisibilityHistory>,
    /// Per-space flag: the Hi-Z authoring camera still matches this frame.
    stable_spaces: HashMap<RenderSpaceId, bool>,
    stale_rects: Vec<ScreenRect>,
    updates: Mutex<Vec<(CasterKey, CasterRecord)>>,
}

impl TwoPhaseFrame {
    /// Builds the frame plan consulted by every fine cull in one view.
    pub(crate) fn new(
        history: Arc<VisibilityHistory>,
        stable_spaces: HashMap<RenderSpaceId, bool>,
        stale_rects: Vec<ScreenRect>,
    ) -> Self {
        Self {
            history,
            stable_spaces,
            stale_rects,
            updates: Mutex::new(Vec::new()),
        }
    }

    /// Previous classification for `key`.
    pub(crate) fn previous(&self, key: &CasterKey) -> Option<&CasterRecord> {
        self.history.get(key)
    }

    /// `true` when phase 2 may trust the previous pyramid in `space_id`.
    pub(crate) fn camera_stable(&self, space_id: RenderSpaceId) -> bool {
        self.stable_spaces.get(&space_id).copied().unwrap_or(false)
    }

    /// Decides whether `caster` is drawn and records the class to commit at the end of the view.
    ///
    /// A missing `caster` id cannot be tracked, so the draw is kept.
    pub(crate) fn allows_draw(
        &self,
        space_id: RenderSpaceId,
        caster: Option<CasterKey>,
        aabb_min: Vec3,
        aabb_max: Vec3,
        facts: OcclusionFacts,
    ) -> bool {
        let Some(caster) = caster else {
            return true;
        };
        let previous = self.previous(&caster);
        let caster_moved = previous.is_some_and(|record| {
            bounds_moved((record.aabb_min, record.aabb_max), (aabb_min, aabb_max))
        });
        let decision = decide_phase(PhaseInputs {
            previous,
            camera_stable: self.camera_stable(space_id),
            caster_moved,
            overlaps_disocclusion: rects_hit_disocclusion(facts.rects, &self.stale_rects),
            hi_z_occludes: facts.occluded,
            other_occluder: facts.other_occluder,
        });
        self.updates.lock().push((
            caster,
            CasterRecord {
                class: decision.next_class,
                aabb_min,
                aabb_max,
            },
        ));
        decision.action != PhaseAction::Culled
    }

    /// Replaces the view history with the casters tested this frame.
    ///
    /// Casters that missed the frustum are absent, so the next time they enter they are treated
    /// as newly revealed and drawn.
    pub(crate) fn finish(&self) -> VisibilityHistory {
        let updates = self.updates.lock();
        let mut casters = HashMap::with_capacity(updates.len());
        for (key, record) in updates.iter().copied() {
            casters.insert(key, record);
        }
        drop(updates);
        VisibilityHistory { casters }
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
        return PhaseDecision {
            action: PhaseAction::Culled,
            next_class: VisibilityClass::Occluded,
        };
    }
    PhaseDecision {
        action: PhaseAction::SecondPhase,
        next_class: VisibilityClass::Visible,
    }
}

/// `true` when two view or projection matrices still describe the same camera for occlusion.
pub(crate) fn matrices_match_for_occlusion(previous: Mat4, current: Mat4) -> bool {
    (0..4).all(|column| {
        (previous.col(column) - current.col(column))
            .abs()
            .max_element()
            <= CAMERA_MATCH_EPSILON
    })
}

/// `true` when the world AABB changed enough that last frame's occlusion no longer applies.
pub(crate) fn bounds_moved(previous: (Vec3, Vec3), current: (Vec3, Vec3)) -> bool {
    (previous.0 - current.0).abs().max_element() > BOUNDS_MATCH_EPSILON
        || (previous.1 - current.1).abs().max_element() > BOUNDS_MATCH_EPSILON
}

/// `true` when `covering_depth` is closer than the caster's own front, in reverse-Z.
pub(crate) fn covering_depth_is_other_occluder(closest_ndc_z: f32, covering_depth: f32) -> bool {
    closest_ndc_z.is_finite()
        && covering_depth.is_finite()
        && covering_depth > closest_ndc_z + OTHER_OCCLUDER_NDC_GAP
}

/// Projects moved visible casters into padded disocclusion rectangles.
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

fn push_padded(rects: &mut Vec<ScreenRect>, rect: Option<ScreenRect>) {
    if let Some(rect) = rect {
        rects.push(rect.padded(DISOCCLUSION_UV_PAD));
    }
}

fn next_class_for_visible_set(inputs: &PhaseInputs<'_>) -> VisibilityClass {
    if inputs.camera_stable
        && !inputs.caster_moved
        && !inputs.overlaps_disocclusion
        && inputs.other_occluder
    {
        VisibilityClass::Occluded
    } else {
        VisibilityClass::Visible
    }
}

fn phase2_rejection_holds(inputs: &PhaseInputs<'_>) -> bool {
    inputs.camera_stable
        && !inputs.caster_moved
        && !inputs.overlaps_disocclusion
        && inputs.hi_z_occludes
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

#[cfg(test)]
mod tests {
    use super::*;

    fn record(class: VisibilityClass, min: Vec3, max: Vec3) -> CasterRecord {
        CasterRecord {
            class,
            aabb_min: min,
            aabb_max: max,
        }
    }

    fn aabb() -> (Vec3, Vec3) {
        (Vec3::new(-0.5, -0.5, -2.0), Vec3::new(0.5, 0.5, -1.0))
    }

    fn inputs<'a>(previous: Option<&'a CasterRecord>, occludes: bool) -> PhaseInputs<'a> {
        PhaseInputs {
            previous,
            camera_stable: true,
            caster_moved: false,
            overlaps_disocclusion: false,
            hi_z_occludes: occludes,
            other_occluder: false,
        }
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
        query.camera_stable = false;
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
    fn separate_occluder_schedules_visible_caster_for_phase_two_next_frame() {
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
                query.camera_stable = false;
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
        let mut stable = HashMap::new();
        stable.insert(RenderSpaceId(1), true);
        let frame = TwoPhaseFrame::new(history, stable, Vec::new());
        let facts = OcclusionFacts {
            occluded: true,
            other_occluder: false,
            rects: [None, None],
        };
        assert!(!frame.allows_draw(RenderSpaceId(1), Some(hidden), min, max, facts));
        assert!(frame.allows_draw(RenderSpaceId(1), Some(revealed), min, max, facts));
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
    fn camera_motion_and_bounds_motion_match_thresholds() {
        assert!(matrices_match_for_occlusion(Mat4::IDENTITY, Mat4::IDENTITY));
        let moved = Mat4::from_translation(Vec3::new(0.2, 0.0, 0.0));
        assert!(!matrices_match_for_occlusion(Mat4::IDENTITY, moved));
        let (min, max) = aabb();
        assert!(!bounds_moved((min, max), (min, max)));
        assert!(bounds_moved((min, max), (min + Vec3::splat(0.05), max)));
        assert!(!covering_depth_is_other_occluder(0.40, 0.405));
        assert!(covering_depth_is_other_occluder(0.40, 0.45));
    }

    #[test]
    fn disocclusion_rects_cover_old_and_new_footprints() {
        let mover = MovedCaster {
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
}
