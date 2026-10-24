//! CPU visibility hysteresis on the one-frame-late Hi-Z pyramid.
//!
//! The GPU pyramid is still the previous frame's depth. There is no second GPU depth pass.
//! Rejecting every caster that fails the stale test pops objects when a camera cut, a moving
//! occluder, or a newly revealed surface disagrees with last frame's depth, and it oscillates
//! casters whose own previous depth covers their bounds.
//!
//! Previously visible casters, and casters with no history, are drawn. A previously occluded
//! caster is rejected only when its bounds and mesh are unchanged, its screen rectangle misses
//! every disocclusion region, and the previous pyramid still covers the rectangle after that
//! rectangle is expanded by the screen motion between the Hi-Z camera and the current camera.
//! Large motion, a failed projection, a parallax ratio above [`PARALLAX_SCALE_MAX`], and a
//! covering depth that is not closer than the caster all fail open and keep the draw. A visible
//! caster becomes occluded for the *next* frame only when the covering surface is closer in
//! linear view depth by more than [`other_occluder_margin`].
//!
//! That covering depth is the farthest reverse-Z sample in the rectangle, not the nearest
//! occluder. A thin surface much closer than that farthest sample never reaches the ratio gate,
//! so camera motion can still under-pad for one frame and hide a caster the thin surface no
//! longer covers.
//!
//! The cap counts prepared casters: one renderer run with a world AABB, inside the view's
//! render-space scope, and not overlay or hidden. It does not count frustum-visible casters,
//! and material slots of one renderer are one caster. Views above [`HISTORY_CASTER_BUDGET`]
//! skip this pass and use the previous-frame Hi-Z test directly. The uncapped history step
//! measured about 0.13 µs per caster on a 4-core guest (100_000 casters was about 13 ms), which
//! is past an 11.1 ms 90 Hz frame. Crossing the cap suspends the pass until the prepared count
//! falls to [`HISTORY_CASTER_RESUME`], so a scene hovering on the cap does not enable and
//! disable the pass on alternate frames. While suspended the stored history is empty, popping
//! can return, and the frame that turns the pass back on treats every caster as newly revealed.
//!
//! A previously visible caster that is absent this frame (hidden, removed, or toggled off) and a
//! mesh swap that leaves the AABB unchanged both emit disocclusion rectangles. A depth
//! contributor that was not a tracked visible caster can still leave one frame where the stale
//! pyramid hides whatever was behind it, because this pass has no remembered rectangle for it.
//!
//! This targets the one-frame-late popping described in issue #679. There is no checked-in repro
//! scene, headset capture, or discrete-GPU measurement for that issue, so the pass mitigates the
//! rejection rather than demonstrating that the issue is closed.

use std::sync::atomic::{AtomicU32, Ordering};

use glam::{Mat4, Vec3};
use hashbrown::{HashMap, HashSet};
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

/// Largest prepared-caster count that still runs history confirmation for one view.
///
/// At the measured ~0.13 µs per caster, this cap is about 1.1 ms. See the module docs for the
/// 90 Hz cutoff and for what the count includes.
pub(crate) const HISTORY_CASTER_BUDGET: usize = 8_192;

/// Prepared-caster count that turns history confirmation back on after [`HISTORY_CASTER_BUDGET`]
/// suspended it.
///
/// Counts between this value and the cap keep the previous decision.
pub(crate) const HISTORY_CASTER_RESUME: usize = HISTORY_CASTER_BUDGET - HISTORY_CASTER_BUDGET / 8;

/// Stable identity of one occlusion-culled caster across frames.
///
/// Material slots share one key. [`VisibilityHysteresis::finish`] keeps a `Visible` classification when
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
///
/// `seen_epoch` is stamped when this frame tests the caster and leaves the row unchanged.
/// [`VisibilityHysteresis::recycle`] drops rows whose epoch is older than the frame.
#[derive(Debug)]
pub(crate) struct CasterRecord {
    /// Classification written at the end of the previous tested frame.
    pub class: VisibilityClass,
    /// World AABB minimum at that classification.
    pub aabb_min: Vec3,
    /// World AABB maximum at that classification.
    pub aabb_max: Vec3,
    /// Mesh asset at that classification.
    pub mesh_asset_id: i32,
    /// Frame epoch that last tested this caster. Not part of the published classification.
    seen_epoch: AtomicU32,
}

impl CasterRecord {
    /// Builds a record that has not been tested in the current epoch.
    #[cfg(any(test, feature = "bench-api"))]
    pub(crate) fn new(
        class: VisibilityClass,
        aabb_min: Vec3,
        aabb_max: Vec3,
        mesh_asset_id: i32,
    ) -> Self {
        Self {
            class,
            aabb_min,
            aabb_max,
            mesh_asset_id,
            seen_epoch: AtomicU32::new(0),
        }
    }
}

/// Draw-or-reject outcome of history confirmation for one caster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryAction {
    /// Previously visible, or no history. Drawn. The stale pyramid does not reject it.
    DrawRemembered,
    /// Previously occluded, but this frame's confirmation no longer holds, so it is drawn.
    Unconfirmed,
    /// Previously occluded, and this frame's confirmation still holds.
    Culled,
}

/// Result of [`decide_visibility`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HistoryDecision {
    /// Whether the caster is drawn, and which confirmation path said so.
    pub action: HistoryAction,
    /// Class stored for the next frame when this caster is inside the frustum.
    pub next_class: VisibilityClass,
}

/// Inputs for one caster's hysteresis decision.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HistoryInputs<'a> {
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

/// Per-view buffers for the serial hysteresis plan.
///
/// [`VisibilityFrameScratch::clear`] keeps the allocation. The occlusion system parks one of
/// these on the view and hands it back next frame, so a steady scene does not allocate the
/// motion list, the present list, or the present-key set again.
#[derive(Default)]
pub(crate) struct VisibilityFrameScratch {
    /// One resolved camera pair per in-scope render space, including `None` when the pair is missing.
    pub resolved_motion: Vec<(RenderSpaceId, Option<SpaceMotion>)>,
    /// Prepared casters in scope. Filled only when history still has a visible caster.
    pub present: Vec<PresentCaster>,
    /// Keys inserted while planning disocclusion. Cleared and refilled with the present list.
    pub present_keys: HashSet<CasterKey>,
    /// Visible casters whose footprint must punch a hole. Cleared when presence is not tracked.
    pub movers: Vec<MovedCaster>,
}

impl VisibilityFrameScratch {
    /// Drops this frame's rows without releasing the buffers.
    pub(crate) fn clear(&mut self) {
        self.resolved_motion.clear();
        self.present.clear();
        self.present_keys.clear();
        self.movers.clear();
    }
}

/// Motions, disocclusion rectangles, and update shards for one view's cull.
///
/// The occlusion system parks one of these on the view. [`VisibilityHysteresis::recycle`]
/// returns it with the shard vectors drained, so the next frame does not allocate them again.
#[derive(Default)]
pub(crate) struct VisibilityCullBuffers {
    /// In-scope space motions. Cleared and refilled while the plan is built.
    pub motions: HashMap<RenderSpaceId, SpaceMotion>,
    /// Padded rectangles for movers. Read by the parallel cull, then returned with the view.
    pub stale_rects: Vec<ScreenRect>,
    /// One record buffer per rayon worker, plus one for callers outside the pool.
    pub shards: Vec<Mutex<Vec<(CasterKey, CasterRecord)>>>,
}

/// History snapshot plus the cleared map the next [`VisibilityHysteresis::finish`] refills.
pub(crate) struct VisibilityScratch {
    /// Classification published by the previous tested frame.
    pub history: VisibilityHistory,
    /// Cleared caster map retained from an earlier publication.
    pub spare: HashMap<CasterKey, CasterRecord>,
    /// `true` when the previous frame skipped history because the prepared count was over the cap.
    pub suspended: bool,
    /// Motion, present-caster, and present-key buffers reused across frames for this view.
    pub frame_scratch: VisibilityFrameScratch,
    /// Cull buffers returned by [`VisibilityHysteresis::recycle`].
    pub cull_buffers: VisibilityCullBuffers,
}

/// Per-view classification carried from the previous tested frame.
#[derive(Debug, Default)]
pub(crate) struct VisibilityHistory {
    casters: HashMap<CasterKey, CasterRecord>,
    /// Casters whose class is [`VisibilityClass::Visible`].
    visible_count: u32,
    /// Epoch stamped on rows written or confirmed by the last [`VisibilityHysteresis::recycle`].
    epoch: u32,
}

impl VisibilityHistory {
    /// Builds a history from an existing caster map.
    ///
    /// The bench API and tests construct a history from an outside map. The renderer fills
    /// history through [`VisibilityHysteresis::finish`].
    #[cfg(any(test, feature = "bench-api"))]
    pub(crate) fn from_casters(casters: HashMap<CasterKey, CasterRecord>) -> Self {
        let visible_count = casters
            .values()
            .filter(|record| record.class == VisibilityClass::Visible)
            .count();
        Self {
            casters,
            visible_count: u32::try_from(visible_count).unwrap_or(u32::MAX),
            epoch: 0,
        }
    }

    /// `true` when any remembered caster was drawn.
    ///
    /// Disocclusion only punches holes for previously visible casters. A history with none of
    /// them has nothing to plan, so the frame can skip the present-caster set.
    pub(crate) fn any_visible(&self) -> bool {
        self.visible_count > 0
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
pub(crate) struct VisibilityHysteresis {
    history: VisibilityHistory,
    buffers: VisibilityCullBuffers,
    spare: HashMap<CasterKey, CasterRecord>,
    /// Epoch stamped onto every caster this frame actually tests.
    epoch: u32,
}

/// History map plus the spare map and cull buffers to park on the view.
pub(crate) struct VisibilityRecycle {
    /// Classification to publish for the next frame.
    pub history: VisibilityHistory,
    /// Unused spare map. Its allocation stays on the view.
    pub spare: HashMap<CasterKey, CasterRecord>,
    /// Drained cull buffers. Shard capacity is kept.
    pub buffers: VisibilityCullBuffers,
}

impl VisibilityHysteresis {
    /// Builds the frame plan consulted by every fine cull in one view.
    ///
    /// `spare` is a cleared map from the previous publication. [`Self::finish`] keeps that
    /// allocation. Shards are created for this call; the view path uses [`Self::from_parts`]
    /// so the same shard vectors are drained and reused.
    #[cfg(test)]
    pub(crate) fn new(
        history: VisibilityHistory,
        spare: HashMap<CasterKey, CasterRecord>,
        motions: HashMap<RenderSpaceId, SpaceMotion>,
        stale_rects: Vec<ScreenRect>,
    ) -> Self {
        Self::from_parts(
            history,
            spare,
            VisibilityCullBuffers {
                motions,
                stale_rects,
                shards: Vec::new(),
            },
        )
    }

    /// Builds the frame plan from pooled shard, motion, and rectangle buffers.
    pub(crate) fn from_parts(
        history: VisibilityHistory,
        spare: HashMap<CasterKey, CasterRecord>,
        mut buffers: VisibilityCullBuffers,
    ) -> Self {
        let epoch = next_epoch(history.epoch);
        let threads = rayon::current_num_threads().saturating_add(1);
        if buffers.shards.len() < threads {
            buffers.shards.reserve(threads - buffers.shards.len());
            while buffers.shards.len() < threads {
                buffers.shards.push(Mutex::new(Vec::new()));
            }
        }
        for shard in &buffers.shards {
            shard.lock().clear();
        }
        Self {
            history,
            buffers,
            spare,
            epoch,
        }
    }

    /// Camera pair cached for `space_id` while this view's plan was built.
    pub(crate) fn space_motion(&self, space_id: RenderSpaceId) -> Option<&SpaceMotion> {
        self.buffers.motions.get(&space_id)
    }

    /// Decides whether `caster` is drawn and records a class only when it is new or changed.
    ///
    /// A missing `caster` id cannot be tracked, so the draw is kept. A caster the pyramid does
    /// not occlude stays visible. When that visible row is already stored with the same mesh and
    /// bounds, the existing row is stamped and no shard record is queued. Occluded casters, and
    /// any caster whose class, mesh, or bounds changed, are queued. Casters that are not tested
    /// this frame are dropped in [`Self::recycle`], so a frustum miss is newly revealed next time.
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
        if !facts.occluded {
            if let Some(previous) = self.history.get(&caster.key)
                && previous.class == VisibilityClass::Visible
                && previous.mesh_asset_id == caster.mesh_asset_id
                && !bounds_moved((previous.aabb_min, previous.aabb_max), (aabb_min, aabb_max))
            {
                previous.seen_epoch.store(self.epoch, Ordering::Relaxed);
                return true;
            }
            self.remember(caster, aabb_min, aabb_max, VisibilityClass::Visible);
            return true;
        }
        let previous = self.history.get(&caster.key);
        let caster_moved = previous.is_some_and(|record| {
            record.mesh_asset_id != caster.mesh_asset_id
                || bounds_moved((record.aabb_min, record.aabb_max), (aabb_min, aabb_max))
        });
        let decision = decide_visibility(HistoryInputs {
            previous,
            caster_moved,
            overlaps_disocclusion: rects_hit_disocclusion(facts.rects, &self.buffers.stale_rects),
            hi_z_occludes: facts.occluded,
            other_occluder: facts.other_occluder,
        });
        self.remember(caster, aabb_min, aabb_max, decision.next_class);
        decision.action != HistoryAction::Culled
    }

    fn remember(&self, caster: CasterRef, aabb_min: Vec3, aabb_max: Vec3, class: VisibilityClass) {
        self.shard().lock().push((
            caster.key,
            CasterRecord {
                class,
                aabb_min,
                aabb_max,
                mesh_asset_id: caster.mesh_asset_id,
                seen_epoch: AtomicU32::new(self.epoch),
            },
        ));
    }

    /// Publishes this frame's classification and drops the pooled buffers.
    #[cfg(test)]
    pub(crate) fn finish(self) -> VisibilityHistory {
        self.recycle().history
    }

    /// Publishes this frame's classification and returns the view's pooled buffers.
    ///
    /// Untested casters are removed. Duplicate material-slot updates collapse here: `Visible`
    /// wins, and the bounds are the union. Shard vectors are drained, not freed.
    pub(crate) fn recycle(self) -> VisibilityRecycle {
        let Self {
            history,
            buffers,
            spare,
            epoch,
        } = self;
        let (mut history, spare) = adopt_spare(history, spare);
        for shard in &buffers.shards {
            let mut records = shard.lock();
            for (key, record) in records.drain(..) {
                merge_record(&mut history.casters, key, record);
            }
            drop(records);
        }
        let mut visible_count = 0u32;
        history.casters.retain(|_, record| {
            if record.seen_epoch.load(Ordering::Relaxed) != epoch {
                return false;
            }
            if record.class == VisibilityClass::Visible {
                visible_count = visible_count.saturating_add(1);
            }
            true
        });
        history.visible_count = visible_count;
        history.epoch = epoch;
        VisibilityRecycle {
            history,
            spare,
            buffers,
        }
    }

    fn shard(&self) -> &Mutex<Vec<(CasterKey, CasterRecord)>> {
        let index = rayon::current_thread_index().map_or(0, |thread| thread.saturating_add(1));
        let index = index.min(self.buffers.shards.len().saturating_sub(1));
        &self.buffers.shards[index]
    }
}

fn next_epoch(epoch: u32) -> u32 {
    match epoch.wrapping_add(1) {
        0 => 1,
        next => next,
    }
}

/// Keeps the larger of the history map and the spare map as the map `recycle` mutates.
fn adopt_spare(
    history: VisibilityHistory,
    mut spare: HashMap<CasterKey, CasterRecord>,
) -> (VisibilityHistory, HashMap<CasterKey, CasterRecord>) {
    if history.casters.capacity() >= spare.capacity() {
        return (history, spare);
    }
    spare.clear();
    spare.reserve(history.casters.len());
    let VisibilityHistory {
        casters,
        visible_count,
        epoch,
    } = history;
    for (key, record) in casters {
        spare.insert(key, record);
    }
    (
        VisibilityHistory {
            casters: spare,
            visible_count,
            epoch,
        },
        HashMap::new(),
    )
}

/// Classifies one frustum-visible caster.
pub(crate) fn decide_visibility(inputs: HistoryInputs<'_>) -> HistoryDecision {
    if inputs.previous.map(|record| record.class) != Some(VisibilityClass::Occluded) {
        return HistoryDecision {
            action: HistoryAction::DrawRemembered,
            next_class: next_class_when_drawn(&inputs),
        };
    }
    if confirmed_rejection(&inputs) {
        HistoryDecision {
            action: HistoryAction::Culled,
            next_class: VisibilityClass::Occluded,
        }
    } else {
        HistoryDecision {
            action: HistoryAction::Unconfirmed,
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

/// `true` when a view with `caster_count` prepared casters may run history confirmation.
#[inline]
pub(crate) fn history_within_budget(caster_count: usize) -> bool {
    caster_count <= HISTORY_CASTER_BUDGET
}

/// `true` when this frame should attach history confirmation.
///
/// `caster_count` is `None` when the prepared count is above [`HISTORY_CASTER_BUDGET`]. A view
/// that was [`suspended`](VisibilityScratch::suspended) stays off until the count is at most
/// [`HISTORY_CASTER_RESUME`].
#[inline]
pub(crate) fn history_confirmation_applies(caster_count: Option<usize>, suspended: bool) -> bool {
    let Some(caster_count) = caster_count else {
        return false;
    };
    if suspended {
        caster_count <= HISTORY_CASTER_RESUME
    } else {
        history_within_budget(caster_count)
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

/// Casters whose previous visible footprint must punch a hole in this frame's confirmation.
///
/// A present visible caster is included when its bounds move or its mesh asset changes. A
/// previously visible caster missing from `present`, and inside `include_space`, is included with
/// its remembered bounds. Previously occluded absences are not disoccluders.
pub(crate) fn plan_disocclusion(
    history: &VisibilityHistory,
    present: &[PresentCaster],
    present_keys: &mut HashSet<CasterKey>,
    mut include_space: impl FnMut(RenderSpaceId) -> bool,
    movers: &mut Vec<MovedCaster>,
) {
    present_keys.clear();
    present_keys.reserve(present.len());
    movers.clear();
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
}

/// Projects moved or removed visible casters into padded disocclusion rectangles.
#[cfg(test)]
pub(crate) fn disocclusion_rects<F>(movers: &[MovedCaster], project: F) -> Vec<ScreenRect>
where
    F: FnMut(Vec3, Vec3) -> Option<ScreenRect>,
{
    let mut rects = Vec::new();
    append_disocclusion_rects(&mut rects, movers, project);
    rects
}

/// Appends padded rectangles for `movers` into `rects`, keeping that buffer's spare capacity.
pub(crate) fn append_disocclusion_rects<F>(
    rects: &mut Vec<ScreenRect>,
    movers: &[MovedCaster],
    mut project: F,
) where
    F: FnMut(Vec3, Vec3) -> Option<ScreenRect>,
{
    for mover in movers {
        push_padded(rects, project(mover.previous_aabb.0, mover.previous_aabb.1));
        push_padded(rects, project(mover.current_aabb.0, mover.current_aabb.1));
    }
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

fn next_class_when_drawn(inputs: &HistoryInputs<'_>) -> VisibilityClass {
    if !inputs.caster_moved && !inputs.overlaps_disocclusion && inputs.other_occluder {
        VisibilityClass::Occluded
    } else {
        VisibilityClass::Visible
    }
}

fn confirmed_rejection(inputs: &HistoryInputs<'_>) -> bool {
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
            existing.seen_epoch.store(
                incoming.seen_epoch.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
        }
    }
}

#[cfg(test)]
include!("hysteresis_tests.rs");
