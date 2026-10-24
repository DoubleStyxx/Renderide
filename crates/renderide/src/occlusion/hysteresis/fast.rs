//! Stable-visible fast path for history confirmation.
//!
//! When the previous frame stored no occluded caster and this frame's prepared set is the same
//! keys, meshes, and bounds, disocclusion rectangles are empty and a caster the pyramid does not
//! occlude does not need a history-map probe. The frame still records a class change when the
//! pyramid does occlude, and it still drops rows that were not tested. Both of those keep the
//! published classification identical to the full update.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use glam::Vec3;

use super::{
    CasterKey, CasterRef, OcclusionFacts, PresentCaster, VisibilityClass, VisibilityCullBuffers,
    VisibilityHistory, VisibilityHysteresis, bounds_moved,
};

/// Largest dense slot table this path will build.
///
/// A wider index span falls back to the full history update. Packed renderer indices for one
/// view fit: static and skinned each take a slot, and the production caster cap is 8_192. The
/// uncapped history bench still fits; a sparse index does not allocate a table for the holes.
const FAST_TABLE_LIMIT: usize = 262_144;

/// Slot layout for one armed frame. Stored on the hysteresis plan, not in the pooled buffers.
#[derive(Clone, Copy)]
pub(crate) struct FastLayout {
    min_space: i32,
    space_span: usize,
    max_index: u32,
    stride: usize,
    table_len: usize,
    generation: u32,
    pub(super) expected: u32,
}

impl FastLayout {
    fn index(self, key: &CasterKey) -> Option<usize> {
        if key.space_id < self.min_space {
            return None;
        }
        let space = usize::try_from(key.space_id - self.min_space).ok()?;
        if space >= self.space_span {
            return None;
        }
        let index = u32::try_from(key.renderable_index).ok()?;
        if index > self.max_index {
            return None;
        }
        let within = (index as usize) * 2 + usize::from(key.skinned);
        if within >= self.stride {
            return None;
        }
        let slot = space * self.stride + within;
        (slot < self.table_len).then_some(slot)
    }
}

/// Pooled slot table and seen-bits for the stable-visible path.
#[derive(Default)]
pub(crate) struct FastTable {
    rows: Vec<FastSlot>,
    words: Vec<AtomicU64>,
    generation: u32,
}

impl FastTable {
    /// Allocation identity of the slot rows, so a test can show the next frame reuses it.
    #[cfg(test)]
    pub(super) fn row_storage(&self) -> (*const u8, usize) {
        (self.rows.as_ptr().cast(), self.rows.capacity())
    }
}

/// One packed renderer slot. `generation` matches the frame that wrote it.
struct FastSlot {
    key: CasterKey,
    mesh_asset_id: i32,
    aabb_min: Vec3,
    aabb_max: Vec3,
    generation: u32,
    matched_generation: u32,
}

impl FastSlot {
    const fn vacant() -> Self {
        Self {
            key: CasterKey {
                space_id: 0,
                renderable_index: 0,
                instance_id: 0,
                node_id: 0,
                skinned: false,
            },
            mesh_asset_id: 0,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
            generation: 0,
            matched_generation: 0,
        }
    }
}

/// Arms the fast path when `present` is exactly the visible history.
///
/// Returns `None` when any remembered caster is occluded, missing, new, moved, or outside the
/// dense index span. The caller then builds disocclusion rectangles the usual way. The pooled
/// slot table keeps its capacity either way.
#[must_use]
pub(crate) fn prepare_fast_visible(
    history: &VisibilityHistory,
    present: &[PresentCaster],
    buffers: &mut VisibilityCullBuffers,
) -> Option<FastLayout> {
    let table = &mut buffers.fast_table;
    let caster_count = history.casters.len();
    if caster_count == 0 || !history.no_occluded_casters() {
        return None;
    }
    let layout = layout_for(history, present, caster_count)?;
    let generation = bump_generation(table);
    if table.rows.len() < layout.table_len {
        table.rows.resize_with(layout.table_len, FastSlot::vacant);
    }
    let mut filled = 0usize;
    for (key, record) in &history.casters {
        if record.class != VisibilityClass::Visible {
            return None;
        }
        let slot = layout.index(key)?;
        let row = table.rows.get_mut(slot)?;
        if row.generation == generation {
            return None;
        }
        *row = FastSlot {
            key: *key,
            mesh_asset_id: record.mesh_asset_id,
            aabb_min: record.aabb_min,
            aabb_max: record.aabb_max,
            generation,
            matched_generation: 0,
        };
        filled += 1;
    }
    if filled != caster_count {
        return None;
    }
    let mut matched = 0usize;
    for caster in present {
        let slot = layout.index(&caster.key)?;
        let row = table.rows.get_mut(slot)?;
        if row.generation != generation || row.key != caster.key {
            return None;
        }
        if row.mesh_asset_id != caster.mesh_asset_id
            || bounds_moved(
                (row.aabb_min, row.aabb_max),
                (caster.aabb_min, caster.aabb_max),
            )
        {
            return None;
        }
        if row.matched_generation != generation {
            row.matched_generation = generation;
            matched += 1;
        }
    }
    if matched != caster_count {
        return None;
    }
    let words = layout.table_len.div_ceil(64);
    if table.words.len() < words {
        table.words.resize_with(words, || AtomicU64::new(0));
    }
    for word in &table.words[..words] {
        word.store(0, Ordering::Relaxed);
    }
    Some(FastLayout {
        generation,
        expected: u32::try_from(caster_count).ok()?,
        ..layout
    })
}

/// `true` when the fast path already accepted this draw and no history write is required.
pub(super) fn accept_stable_visible(
    frame: &VisibilityHysteresis,
    caster: Option<CasterRef>,
    aabb_min: Vec3,
    aabb_max: Vec3,
    facts: OcclusionFacts,
) -> bool {
    let Some(layout) = frame.fast else {
        return false;
    };
    // A missing id cannot be tracked. The full path draws it and writes nothing.
    let Some(caster) = caster else {
        return true;
    };
    if facts.occluded {
        return false;
    }
    let Some(slot) = layout.index(&caster.key) else {
        return false;
    };
    let Some(row) = frame.buffers.fast_table.rows.get(slot) else {
        return false;
    };
    if row.generation != layout.generation
        || row.key != caster.key
        || row.mesh_asset_id != caster.mesh_asset_id
        || bounds_moved((row.aabb_min, row.aabb_max), (aabb_min, aabb_max))
    {
        return false;
    }
    mark_seen(&frame.buffers.fast_table.words, &frame.fast_visits, slot);
    true
}

/// `true` when a stable-visible accept already confirmed `key` this frame.
pub(super) fn row_confirmed(
    layout: Option<FastLayout>,
    table: &FastTable,
    key: &CasterKey,
) -> bool {
    let Some(layout) = layout else {
        return false;
    };
    let Some(slot) = layout.index(key) else {
        return false;
    };
    let Some(word) = table.words.get(slot / 64) else {
        return false;
    };
    let mask = 1u64 << (slot % 64);
    word.load(Ordering::Relaxed) & mask != 0
}

fn layout_for(
    history: &VisibilityHistory,
    present: &[PresentCaster],
    caster_count: usize,
) -> Option<FastLayout> {
    let mut min_space = i32::MAX;
    let mut max_space = i32::MIN;
    let mut max_index = 0u32;
    for key in history
        .casters
        .keys()
        .chain(present.iter().map(|caster| &caster.key))
    {
        if key.space_id < 0 {
            return None;
        }
        min_space = min_space.min(key.space_id);
        max_space = max_space.max(key.space_id);
        let index = u32::try_from(key.renderable_index).ok()?;
        max_index = max_index.max(index);
    }
    let space_span = usize::try_from(max_space - min_space)
        .ok()?
        .saturating_add(1);
    let stride = (max_index as usize).saturating_add(1).saturating_mul(2);
    let table_len = space_span.checked_mul(stride)?;
    if table_len == 0 || table_len > FAST_TABLE_LIMIT || table_len > caster_count.saturating_mul(4)
    {
        return None;
    }
    Some(FastLayout {
        min_space,
        space_span,
        max_index,
        stride,
        table_len,
        generation: 0,
        expected: 0,
    })
}

fn bump_generation(table: &mut FastTable) -> u32 {
    let next = table.generation.wrapping_add(1);
    if next == 0 {
        for row in &mut table.rows {
            row.generation = 0;
            row.matched_generation = 0;
        }
        table.generation = 1;
        1
    } else {
        table.generation = next;
        next
    }
}

fn mark_seen(words: &[AtomicU64], visits: &AtomicU32, slot: usize) {
    let Some(word) = words.get(slot / 64) else {
        return;
    };
    let mask = 1u64 << (slot % 64);
    let previous = word.fetch_or(mask, Ordering::Relaxed);
    if previous & mask == 0 {
        visits.fetch_add(1, Ordering::Relaxed);
    }
}
