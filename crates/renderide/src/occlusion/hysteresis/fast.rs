//! Stable-visible fast path for history confirmation.
//!
//! When the previous frame stored no occluded caster and this frame's prepared set is the same
//! keys, meshes, and bounds, disocclusion rectangles are empty. A caster the pyramid does not
//! occlude is stamped in place. If every remembered caster was stamped once and nothing else was
//! queued, publishing the history skips the retain scan. A caster the pyramid occludes, a moved
//! or missing caster, and any row that was not tested still take the full update.

use std::sync::atomic::Ordering;

use hashbrown::HashSet;

use super::{
    CasterKey, CasterRef, OcclusionFacts, PresentCaster, VisibilityClass, VisibilityHistory,
    VisibilityHysteresis, bounds_moved,
};

/// Arms the fast path when `present` is exactly the visible history.
///
/// `present_keys` is cleared and filled with the matching keys. Returns the number of remembered
/// casters when the sets match, otherwise `None` after leaving `present_keys` for the caller to
/// rebuild. The set keeps its capacity.
#[must_use]
pub(crate) fn prepare_fast_visible(
    history: &VisibilityHistory,
    present: &[PresentCaster],
    present_keys: &mut HashSet<CasterKey>,
) -> Option<u32> {
    let caster_count = history.casters.len();
    if caster_count == 0 || !history.no_occluded_casters() {
        return None;
    }
    let expected = u32::try_from(caster_count).ok()?;
    present_keys.clear();
    present_keys.reserve(present.len());
    for caster in present {
        let record = history.casters.get(&caster.key)?;
        if record.class != VisibilityClass::Visible
            || record.mesh_asset_id != caster.mesh_asset_id
            || bounds_moved(
                (record.aabb_min, record.aabb_max),
                (caster.aabb_min, caster.aabb_max),
            )
        {
            return None;
        }
        present_keys.insert(caster.key);
    }
    (present_keys.len() == caster_count).then_some(expected)
}

/// `true` when this draw only restamps a stable visible row.
pub(super) fn accept_stable_visible(
    frame: &VisibilityHysteresis,
    caster: Option<CasterRef>,
    aabb_min: glam::Vec3,
    aabb_max: glam::Vec3,
    facts: OcclusionFacts,
) -> bool {
    if frame.fast_expected == 0 {
        return false;
    }
    // A missing id cannot be tracked. The full path draws it and writes nothing.
    let Some(caster) = caster else {
        return true;
    };
    if facts.occluded {
        return false;
    }
    let Some(previous) = frame.history.get(&caster.key) else {
        return false;
    };
    if previous.class != VisibilityClass::Visible
        || previous.mesh_asset_id != caster.mesh_asset_id
        || bounds_moved((previous.aabb_min, previous.aabb_max), (aabb_min, aabb_max))
    {
        return false;
    }
    let previous_epoch = previous.seen_epoch.swap(frame.epoch, Ordering::Relaxed);
    if previous_epoch != frame.epoch {
        frame.fast_visits.fetch_add(1, Ordering::Relaxed);
    }
    true
}
