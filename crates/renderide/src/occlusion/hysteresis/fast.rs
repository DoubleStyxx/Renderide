//! Stable-visible fast path for history confirmation.
//!
//! When the previous frame stored no occluded caster and this frame's prepared set is the same
//! dense keys, meshes, and bounds, disocclusion rectangles are empty. The steady frame compares
//! the prepared rows to a reused index table and marks a caster the pyramid does not occlude with
//! a bit, instead of probing the history map. If every remembered caster was marked and nothing
//! else was queued, publishing skips the retain scan. A non-dense index, a moved or occluded
//! caster, and any row that was not tested still take the full update.

use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    CasterKey, CasterRef, HISTORY_CASTER_BUDGET, OcclusionFacts, PresentCaster, VisibilityClass,
    VisibilityCullBuffers, VisibilityHistory, VisibilityHysteresis, bounds_moved,
};

/// Reused index table for one view. The rows stay allocated across frames.
#[derive(Default)]
pub(super) struct FastSlots {
    rows: Vec<FastRow>,
    visits: Vec<AtomicU64>,
    valid: bool,
    count: usize,
    space_id: i32,
    skinned: bool,
}

struct FastRow {
    instance_id: u64,
    node_id: i32,
    mesh_asset_id: i32,
    aabb_min: glam::Vec3,
    aabb_max: glam::Vec3,
}

impl FastRow {
    fn blank() -> Self {
        Self {
            instance_id: 0,
            node_id: 0,
            mesh_asset_id: 0,
            aabb_min: glam::Vec3::ZERO,
            aabb_max: glam::Vec3::ZERO,
        }
    }
}

impl FastSlots {
    fn ensure(&mut self, count: usize) {
        if self.rows.len() < count {
            self.rows.resize_with(count, FastRow::blank);
        }
        let words = count.div_ceil(64);
        if self.visits.len() < words {
            self.visits.resize_with(words, || AtomicU64::new(0));
        }
    }

    fn clear_visits(&self) {
        let words = self.count.div_ceil(64).min(self.visits.len());
        for word in &self.visits[..words] {
            word.store(0, Ordering::Relaxed);
        }
    }

    fn bit(&self, index: usize) -> bool {
        let Some(word) = self.visits.get(index / 64) else {
            return false;
        };
        (word.load(Ordering::Relaxed) & (1u64 << (index % 64))) != 0
    }

    fn set_bit(&self, index: usize) {
        if let Some(word) = self.visits.get(index / 64) {
            word.fetch_or(1u64 << (index % 64), Ordering::Relaxed);
        }
    }

    fn popcount(&self) -> usize {
        let words = self.count.div_ceil(64).min(self.visits.len());
        self.visits[..words]
            .iter()
            .map(|word| usize::try_from(word.load(Ordering::Relaxed).count_ones()).unwrap_or(0))
            .sum()
    }

    fn row_matches_present(&self, index: usize, caster: &PresentCaster) -> bool {
        let Some(row) = self.rows.get(index) else {
            return false;
        };
        caster.key.instance_id == row.instance_id
            && caster.key.node_id == row.node_id
            && caster.mesh_asset_id == row.mesh_asset_id
            && !bounds_moved(
                (row.aabb_min, row.aabb_max),
                (caster.aabb_min, caster.aabb_max),
            )
    }

    fn row_matches_draw(
        &self,
        index: usize,
        caster: CasterRef,
        aabb_min: glam::Vec3,
        aabb_max: glam::Vec3,
    ) -> bool {
        let Some(row) = self.rows.get(index) else {
            return false;
        };
        caster.key.space_id == self.space_id
            && caster.key.skinned == self.skinned
            && caster.key.instance_id == row.instance_id
            && caster.key.node_id == row.node_id
            && caster.mesh_asset_id == row.mesh_asset_id
            && !bounds_moved((row.aabb_min, row.aabb_max), (aabb_min, aabb_max))
    }

    fn key_at(&self, index: usize) -> Option<CasterKey> {
        let row = self.rows.get(index)?;
        Some(CasterKey {
            space_id: self.space_id,
            renderable_index: u64::try_from(index).unwrap_or(u64::MAX),
            instance_id: row.instance_id,
            node_id: row.node_id,
            skinned: self.skinned,
        })
    }

    /// `true` when `present` still matches the table filled from the unchanged visible history.
    fn matches(&self, history: &VisibilityHistory, present: &[PresentCaster]) -> bool {
        if !self.valid
            || self.count == 0
            || self.count != history.casters.len()
            || present.len() != self.count
            || !history.no_occluded_casters()
        {
            return false;
        }
        self.clear_visits();
        for caster in present {
            if caster.key.space_id != self.space_id || caster.key.skinned != self.skinned {
                return false;
            }
            let Some(index) = slot_index(caster.key.renderable_index, self.count) else {
                return false;
            };
            if !self.row_matches_present(index, caster) || self.bit(index) {
                return false;
            }
            self.set_bit(index);
        }
        let complete = self.popcount() == self.count;
        self.clear_visits();
        complete
    }

    /// Fills the table from `history` when `present` is that same dense visible set.
    fn rebuild(&mut self, history: &VisibilityHistory, present: &[PresentCaster]) -> bool {
        self.valid = false;
        let count = history.casters.len();
        if count == 0
            || count > HISTORY_CASTER_BUDGET
            || present.len() != count
            || !history.no_occluded_casters()
        {
            return false;
        }
        self.count = count;
        self.ensure(count);
        self.clear_visits();
        let mut space_id = None;
        let mut skinned = None;
        for (key, record) in history.iter() {
            let Some(index) = slot_index(key.renderable_index, count) else {
                return false;
            };
            if record.class != VisibilityClass::Visible
                || space_id.is_some_and(|space| space != key.space_id)
                || skinned.is_some_and(|flag| flag != key.skinned)
                || self.bit(index)
            {
                return false;
            }
            space_id = Some(key.space_id);
            skinned = Some(key.skinned);
            self.set_bit(index);
            self.rows[index] = FastRow {
                instance_id: key.instance_id,
                node_id: key.node_id,
                mesh_asset_id: record.mesh_asset_id,
                aabb_min: record.aabb_min,
                aabb_max: record.aabb_max,
            };
        }
        if self.popcount() != count {
            return false;
        }
        self.space_id = space_id.unwrap_or(0);
        self.skinned = skinned.unwrap_or(false);
        self.clear_visits();
        for caster in present {
            let Some(index) = slot_index(caster.key.renderable_index, count) else {
                return false;
            };
            if caster.key.space_id != self.space_id
                || caster.key.skinned != self.skinned
                || !self.row_matches_present(index, caster)
                || self.bit(index)
            {
                return false;
            }
            self.set_bit(index);
        }
        if self.popcount() != count {
            return false;
        }
        self.clear_visits();
        self.valid = true;
        true
    }
}

/// Arms the fast path when `present` is exactly the visible history and the indices are dense.
///
/// The table keeps its allocation. `None` leaves it invalid so the caller plans disocclusion.
#[must_use]
pub(crate) fn prepare_fast_visible(
    history: &VisibilityHistory,
    present: &[PresentCaster],
    buffers: &mut VisibilityCullBuffers,
) -> Option<u32> {
    let slots = &mut buffers.fast_slots;
    if slots.matches(history, present) || slots.rebuild(history, present) {
        u32::try_from(slots.count).ok()
    } else {
        slots.valid = false;
        None
    }
}

/// `true` when this draw only marks a stable visible row in the index table.
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
    let Some(index) = slot_index(caster.key.renderable_index, frame.buffers.fast_slots.count)
    else {
        return false;
    };
    if !frame
        .buffers
        .fast_slots
        .row_matches_draw(index, caster, aabb_min, aabb_max)
    {
        return false;
    }
    frame.buffers.fast_slots.set_bit(index);
    true
}

fn slot_index(index: u64, count: usize) -> Option<usize> {
    let index = usize::try_from(index).ok()?;
    (index < count).then_some(index)
}

/// How many dense rows the cull marked. Read after the parallel cull has joined.
pub(super) fn visited_casters(slots: &FastSlots) -> u32 {
    u32::try_from(slots.popcount()).unwrap_or(u32::MAX)
}

/// Writes this frame's epoch onto rows the fast path marked, so a later shard can union with them.
pub(super) fn stamp_visited_epochs(slots: &FastSlots, history: &VisibilityHistory, epoch: u32) {
    if !slots.valid {
        return;
    }
    for index in 0..slots.count {
        if !slots.bit(index) {
            continue;
        }
        let Some(key) = slots.key_at(index) else {
            continue;
        };
        if let Some(record) = history.get(&key) {
            record.seen_epoch.store(epoch, Ordering::Relaxed);
        }
    }
}

/// Drops the table after a frame that did not keep every row unchanged.
pub(super) fn invalidate(slots: &mut FastSlots) {
    slots.valid = false;
}
