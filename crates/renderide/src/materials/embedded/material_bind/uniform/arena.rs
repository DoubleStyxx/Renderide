//! Slot table, deferred-retirement allocator, and GPU uniform buffer.

use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use hashbrown::HashSet;
use lru::LruCache;

use super::super::super::embedded_material_bind_error::EmbeddedMaterialBindError;
use super::key::{MaterialUniformArenaSlotBinding, MaterialUniformCacheKey};
use super::retire::{FreeSpan, PendingSpan, UniformArenaRetireClock};

const INITIAL_MATERIAL_UNIFORM_ARENA_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug)]
struct MaterialUniformSlot {
    offset: u64,
    size: u64,
    last_written_generation: u64,
    last_written_texture_state_sig: u64,
    buffer_generation: u64,
}

#[derive(Clone, Copy, Debug)]
struct MaterialUniformSlotResolution {
    offset: u64,
    size: NonZeroU64,
    buffer_generation: u64,
    needs_write: bool,
}

/// Pure allocator state for material uniform arena slots.
#[derive(Debug)]
struct MaterialUniformArenaAllocator {
    slots: LruCache<MaterialUniformCacheKey, MaterialUniformSlot>,
    /// Ranges whose recording frame has completed. Same-sized inserts reuse these first.
    free: Vec<FreeSpan>,
    /// Ranges retired by eviction but still referenced by a recorded or in-flight frame.
    pending: Vec<PendingSpan>,
    /// Frame currently recording draws. `0` until a frame begins.
    recording_frame: u64,
    /// Highest frame whose submission has completed.
    completed_frame: u64,
    cursor: u64,
    capacity: u64,
    max_bytes: u64,
    alignment: u64,
    generation: u64,
    slot_evictions: u64,
}

impl MaterialUniformArenaAllocator {
    fn new(capacity: u64, max_bytes: u64, alignment: u64) -> Self {
        Self::with_slot_capacity(
            capacity,
            max_bytes,
            alignment,
            super::super::cache::max_cached_material_uniform_slots_per_shard(),
        )
    }

    fn with_slot_capacity(
        capacity: u64,
        max_bytes: u64,
        alignment: u64,
        slot_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            slots: LruCache::new(slot_capacity),
            free: Vec::new(),
            pending: Vec::new(),
            recording_frame: 0,
            completed_frame: 0,
            cursor: 0,
            capacity,
            max_bytes,
            alignment: alignment.max(1),
            generation: 0,
            slot_evictions: 0,
        }
    }

    /// Test hook. Frame ids start at 1. Production frames come from [`UniformArenaRetireClock`].
    #[cfg(test)]
    fn begin_frame(&mut self) {
        self.recording_frame = self.recording_frame.saturating_add(1);
    }

    /// Test hook. Marks the current recording frame submitted and promotes its retired ranges.
    #[cfg(test)]
    fn retire_submitted_frame(&mut self) {
        self.completed_frame = self.completed_frame.max(self.recording_frame);
        self.promote_retired();
    }

    fn observe_frames(&mut self, recording_frame: u64, completed_frame: u64) {
        self.recording_frame = recording_frame;
        if completed_frame > self.completed_frame {
            self.completed_frame = completed_frame;
            self.promote_retired();
        }
    }

    fn slot_count(&self) -> usize {
        self.slots.len()
    }

    fn eviction_count(&self) -> u64 {
        self.slot_evictions
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn resolve_slot(
        &mut self,
        key: MaterialUniformCacheKey,
        size: NonZeroU64,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) -> Result<MaterialUniformSlotResolution, EmbeddedMaterialBindError> {
        let size = size.get();
        if let Some(slot) = self.slots.get(&key).copied() {
            if slot.size != size {
                return Err(EmbeddedMaterialBindError::from(format!(
                    "material uniform arena slot size changed for key {:?}: old={} new={}",
                    key, slot.size, size
                )));
            }
            let needs_write = slot.last_written_generation != mutation_gen
                || slot.last_written_texture_state_sig != texture_state_sig
                || slot.buffer_generation != self.generation;
            return Ok(MaterialUniformSlotResolution {
                offset: slot.offset,
                size: non_zero_u64(size)?,
                buffer_generation: self.generation,
                needs_write,
            });
        }

        let offset = self.allocate_offset(size, &key)?;
        if let Some((evicted_key, evicted_slot)) = self.slots.push(
            key,
            MaterialUniformSlot {
                offset,
                size,
                last_written_generation: u64::MAX,
                last_written_texture_state_sig: u64::MAX,
                buffer_generation: self.generation,
            },
        ) && evicted_key != key
        {
            // `LruCache::push` also returns the previous value when the key is replaced.
            // Only a different key is an eviction whose range can be reused.
            self.note_slot_evictions(1);
            self.retire_slot(evicted_slot);
        }
        Ok(MaterialUniformSlotResolution {
            offset,
            size: non_zero_u64(size)?,
            buffer_generation: self.generation,
            needs_write: true,
        })
    }

    fn mark_written(
        &mut self,
        key: &MaterialUniformCacheKey,
        buffer_generation: u64,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) {
        let Some(slot) = self.slots.get_mut(key) else {
            return;
        };
        if slot.buffer_generation > buffer_generation {
            return;
        }
        slot.buffer_generation = buffer_generation;
        slot.last_written_generation = mutation_gen;
        slot.last_written_texture_state_sig = texture_state_sig;
    }

    fn purge_material_assets(
        &mut self,
        material_ids: &HashSet<i32>,
        property_block_ids: &HashSet<i32>,
    ) -> usize {
        self.retain_slots(|key| {
            !material_ids.contains(&key.material_asset_id)
                && key
                    .property_block_slot0
                    .is_none_or(|id| !property_block_ids.contains(&id))
        })
    }

    fn evict_stem(&mut self, stem_hash: u64) -> usize {
        self.retain_slots(|key| key.stem_hash != stem_hash)
    }

    fn evict_stem_generation(&mut self, stem_hash: u64, shader_source_generation: u64) -> usize {
        self.retain_slots(|key| {
            key.stem_hash != stem_hash || key.shader_source_generation != shader_source_generation
        })
    }

    /// `lru` 0.18 has no `retain`. Keep entries for which `pred` is true.
    fn retain_slots(&mut self, mut pred: impl FnMut(&MaterialUniformCacheKey) -> bool) -> usize {
        let stale: Vec<MaterialUniformCacheKey> = self
            .slots
            .iter()
            .filter(|(key, _slot)| !pred(key))
            .map(|(key, _slot)| *key)
            .collect();
        let mut removed_slots = Vec::new();
        for key in stale {
            if let Some(slot) = self.slots.pop(&key) {
                removed_slots.push(slot);
            }
        }
        let removed = removed_slots.len();
        self.finish_slot_removal(removed_slots);
        removed
    }

    fn finish_slot_removal(&mut self, removed: Vec<MaterialUniformSlot>) {
        if removed.is_empty() {
            return;
        }
        self.note_slot_evictions(removed.len());
        if self.slots.is_empty() {
            // Rewinding onto the same GPU buffer would overwrite offsets already recorded
            // for this frame. Bump generation so the next resolve allocates a new buffer.
            self.free.clear();
            self.pending.clear();
            self.cursor = 0;
            self.generation = self.generation.wrapping_add(1);
            return;
        }
        for slot in removed {
            self.retire_slot(slot);
        }
    }

    /// Returns `size` bytes. Completed free spans are reused in place. A retired range stays
    /// pending until its recording frame's submit completes, so this frame cannot land a new
    /// occupant on a `dynamic_offset` it already recorded. Growth (a new buffer generation)
    /// is preferred over compacting onto those pending holes. Compaction runs only when growth
    /// would exceed `max_bytes`.
    fn allocate_offset(
        &mut self,
        size: u64,
        key: &MaterialUniformCacheKey,
    ) -> Result<u64, EmbeddedMaterialBindError> {
        self.promote_retired();
        if let Some(offset) = self.take_free(size) {
            return Ok(offset);
        }
        if let Some(offset) = self.try_bump(size)? {
            return Ok(offset);
        }
        if self.will_evict_on_insert(key)
            && let Some(offset) = self.reclaim_lru_for_insert(size)?
        {
            return Ok(offset);
        }
        let needed = self.bump_end(size)?;
        if needed <= self.max_bytes {
            self.ensure_capacity(needed)?;
            return self.try_bump(size)?.ok_or_else(|| {
                EmbeddedMaterialBindError::from(
                    "material uniform arena offset overflow".to_string(),
                )
            });
        }
        if let Some(packed) = self.try_compact_for_new(size) {
            return Ok(packed);
        }
        Err(EmbeddedMaterialBindError::from(
            "material uniform arena offset overflow".to_string(),
        ))
    }

    fn try_bump(&mut self, size: u64) -> Result<Option<u64>, EmbeddedMaterialBindError> {
        let offset = align_up(self.cursor, self.alignment);
        let end = self.checked_end(offset, size)?;
        if end > self.capacity {
            return Ok(None);
        }
        self.cursor = end;
        Ok(Some(offset))
    }

    fn bump_end(&self, size: u64) -> Result<u64, EmbeddedMaterialBindError> {
        let offset = align_up(self.cursor, self.alignment);
        self.checked_end(offset, size)
    }

    fn checked_end(&self, offset: u64, size: u64) -> Result<u64, EmbeddedMaterialBindError> {
        offset.checked_add(size).ok_or_else(|| {
            EmbeddedMaterialBindError::from("material uniform arena offset overflow".to_string())
        })
    }

    fn will_evict_on_insert(&self, key: &MaterialUniformCacheKey) -> bool {
        self.slots.len() == self.slots.cap().get() && !self.slots.contains(key)
    }

    /// Pops the LRU slot and parks its range on the pending list.
    ///
    /// The popped offset is not returned. A draw recorded earlier in this frame may already
    /// hold it as `dynamic_offset`, and `write_buffer` for the new occupant would land before
    /// that draw.
    fn reclaim_lru_for_insert(
        &mut self,
        size: u64,
    ) -> Result<Option<u64>, EmbeddedMaterialBindError> {
        let Some((evicted_key, evicted_slot)) = self.slots.pop_lru() else {
            return Ok(None);
        };
        let _ = evicted_key;
        self.note_slot_evictions(1);
        self.retire_slot(evicted_slot);
        if let Some(offset) = self.take_free(size) {
            return Ok(Some(offset));
        }
        self.try_bump(size)
    }

    fn take_free(&mut self, size: u64) -> Option<u64> {
        let index = self.free.iter().position(|span| span.size == size)?;
        Some(self.free.swap_remove(index).offset)
    }

    fn retire_slot(&mut self, slot: MaterialUniformSlot) {
        if slot.size == 0 {
            return;
        }
        self.pending.push(PendingSpan {
            offset: slot.offset,
            size: slot.size,
            frame: self.recording_frame,
        });
    }

    /// Moves pending ranges onto the free list once their recording frame has submitted.
    ///
    /// A `frame` of 0 (retired before any `begin_frame`) stays pending until some later
    /// submission completes, then becomes reusable. It is not reusable in the frame that
    /// produced it, because `completed_frame` is still 0.
    fn promote_retired(&mut self) {
        if self.completed_frame == 0 {
            return;
        }
        let mut index = 0;
        while index < self.pending.len() {
            let frame = self.pending[index].frame;
            let retired = frame == 0 || frame <= self.completed_frame;
            if retired {
                let span = self.pending.swap_remove(index);
                self.free.push(FreeSpan {
                    offset: span.offset,
                    size: span.size,
                });
            } else {
                index += 1;
            }
        }
    }

    fn span_overlaps_pending(&self, offset: u64, size: u64) -> bool {
        let end = offset.saturating_add(size);
        self.pending.iter().any(|span| {
            let span_end = span.offset.saturating_add(span.size);
            offset < span_end && span.offset < end
        })
    }

    /// Repacks live slots from offset 0 and reserves `size` bytes after them.
    ///
    /// Returns the new slot offset when that reservation fits in the current capacity.
    /// Live offsets that move bump `generation`.
    fn try_compact_for_new(&mut self, size: u64) -> Option<u64> {
        let mut live: Vec<(MaterialUniformCacheKey, u64, u64)> = self
            .slots
            .iter()
            .map(|(key, slot)| (*key, slot.offset, slot.size))
            .collect();
        live.sort_by_key(|(_key, offset, _size)| *offset);
        let mut cursor = 0u64;
        let mut packed = Vec::with_capacity(live.len());
        let mut moved = false;
        for (key, old_offset, slot_size) in live {
            let offset = align_up(cursor, self.alignment);
            let end = offset.checked_add(slot_size)?;
            if end > self.capacity {
                return None;
            }
            if offset != old_offset {
                moved = true;
            }
            packed.push((key, offset));
            cursor = end;
        }
        let new_offset = align_up(cursor, self.alignment);
        let new_end = new_offset.checked_add(size)?;
        if new_end > self.capacity {
            return None;
        }
        let overlaps_pending = self.span_overlaps_pending(new_offset, size);
        for (key, offset) in packed {
            if let Some(slot) = self.slots.peek_mut(&key) {
                slot.offset = offset;
            }
        }
        self.free.clear();
        self.cursor = new_end;
        if moved || overlaps_pending {
            // A moved live slot can sit on a pending hole. Drop those spans and replace the
            // GPU buffer so in-flight draws keep the previous allocation.
            self.pending.clear();
            self.generation = self.generation.wrapping_add(1);
        }
        Some(new_offset)
    }

    fn note_slot_evictions(&mut self, removed: usize) {
        self.slot_evictions = self
            .slot_evictions
            .saturating_add(u64::try_from(removed).unwrap_or(u64::MAX));
    }

    fn ensure_capacity(&mut self, needed: u64) -> Result<(), EmbeddedMaterialBindError> {
        if needed <= self.capacity {
            return Ok(());
        }
        if needed > self.max_bytes {
            return Err(EmbeddedMaterialBindError::from(format!(
                "material uniform arena needs {needed} bytes, exceeding cap {}",
                self.max_bytes
            )));
        }
        let grown = align_up(self.capacity.saturating_mul(2).max(needed), self.alignment);
        self.capacity = grown.min(self.max_bytes).max(needed);
        // Pending holes are still unoccupied. In-flight draws reference the buffer this
        // generation bump replaces, so the holes stay pending instead of being cleared.
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }
}

/// Shared GPU buffer plus slot allocator for embedded material uniform constants.
pub(in super::super) struct MaterialUniformArena {
    device: Arc<wgpu::Device>,
    allocator: MaterialUniformArenaAllocator,
    buffer: Arc<wgpu::Buffer>,
    retire_clock: Arc<UniformArenaRetireClock>,
    /// Reused CPU bytes for the next dirty material on this shard.
    pub(super) pack_scratch: Vec<u8>,
}

impl MaterialUniformArena {
    pub(in super::super) fn new(
        device: Arc<wgpu::Device>,
        limits: Arc<crate::gpu::GpuLimits>,
        retire_clock: Arc<UniformArenaRetireClock>,
    ) -> Self {
        let alignment = u64::from(limits.min_uniform_buffer_offset_alignment()).max(1);
        let max_bytes = limits.max_buffer_size().min(u64::from(u32::MAX));
        let initial = align_up(
            INITIAL_MATERIAL_UNIFORM_ARENA_BYTES
                .min(max_bytes)
                .max(alignment),
            alignment,
        );
        let buffer = Arc::new(create_material_uniform_arena_buffer(
            device.as_ref(),
            initial,
        ));
        Self {
            device,
            allocator: MaterialUniformArenaAllocator::new(initial, max_bytes, alignment),
            buffer,
            retire_clock,
            pack_scratch: Vec::new(),
        }
    }

    fn sync_retire_clock(&mut self) {
        self.allocator.observe_frames(
            self.retire_clock.recording_frame(),
            self.retire_clock.completed_frame(),
        );
    }

    pub(super) fn resolve_binding(
        &mut self,
        key: MaterialUniformCacheKey,
        size: NonZeroU64,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) -> Result<(MaterialUniformArenaSlotBinding, bool), EmbeddedMaterialBindError> {
        self.sync_retire_clock();
        let previous_generation = self.allocator.generation();
        let resolved = self
            .allocator
            .resolve_slot(key, size, mutation_gen, texture_state_sig)?;
        if self.allocator.generation() != previous_generation {
            self.buffer = Arc::new(create_material_uniform_arena_buffer(
                self.device.as_ref(),
                self.allocator.capacity(),
            ));
            logger::debug!(
                "material uniform arena: grew to {} bytes (generation {})",
                self.allocator.capacity(),
                self.allocator.generation()
            );
        }
        let dynamic_offset = u32::try_from(resolved.offset).map_err(|_err| {
            EmbeddedMaterialBindError::from(format!(
                "material uniform arena offset {} exceeds dynamic offset range",
                resolved.offset
            ))
        })?;
        Ok((
            MaterialUniformArenaSlotBinding {
                buffer: self.buffer.clone(),
                dynamic_offset,
                size: resolved.size,
                buffer_generation: resolved.buffer_generation,
            },
            resolved.needs_write,
        ))
    }

    pub(super) fn mark_written(
        &mut self,
        key: &MaterialUniformCacheKey,
        binding: &MaterialUniformArenaSlotBinding,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) {
        self.allocator.mark_written(
            key,
            binding.buffer_generation,
            mutation_gen,
            texture_state_sig,
        );
    }

    pub(in super::super) fn purge_material_assets(
        &mut self,
        material_ids: &HashSet<i32>,
        property_block_ids: &HashSet<i32>,
    ) -> usize {
        self.sync_retire_clock();
        self.allocator
            .purge_material_assets(material_ids, property_block_ids)
    }

    pub(in super::super) fn evict_stem(&mut self, stem_hash: u64) -> usize {
        self.sync_retire_clock();
        self.allocator.evict_stem(stem_hash)
    }

    pub(in super::super) fn evict_stem_generation(
        &mut self,
        stem_hash: u64,
        shader_source_generation: u64,
    ) -> usize {
        self.sync_retire_clock();
        self.allocator
            .evict_stem_generation(stem_hash, shader_source_generation)
    }

    pub(in super::super) fn slot_count(&self) -> usize {
        self.allocator.slot_count()
    }

    pub(in super::super) fn eviction_count(&self) -> u64 {
        self.allocator.eviction_count()
    }
}

fn create_material_uniform_arena_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("embedded_material_uniform_arena"),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    crate::profiling::note_resource_churn!(Buffer, "materials::embedded_material_uniform_arena");
    buffer
}

fn align_up(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment).saturating_mul(alignment)
}

pub(super) fn non_zero_u64(value: u64) -> Result<NonZeroU64, EmbeddedMaterialBindError> {
    NonZeroU64::new(value).ok_or_else(|| {
        EmbeddedMaterialBindError::from("material uniform block has zero size".to_string())
    })
}

/// WebGPU minimum `minUniformBufferOffsetAlignment`.
///
/// Production reads the device limit. Benches have no device; desktop adapters commonly report
/// this spec minimum, and the slot math matches [`MaterialUniformArenaAllocator`].
#[cfg(feature = "bench-api")]
const BENCH_UNIFORM_ALIGNMENT: u64 = 256;

/// CPU uniform-arena slot table. No GPU buffer is allocated.
#[cfg(feature = "bench-api")]
pub(crate) struct CpuMaterialUniformArena {
    allocator: MaterialUniformArenaAllocator,
    block_size: u64,
    pending: Option<PendingUniformWrite>,
}

#[cfg(feature = "bench-api")]
struct PendingUniformWrite {
    key: MaterialUniformCacheKey,
    buffer_generation: u64,
}

#[cfg(feature = "bench-api")]
impl CpuMaterialUniformArena {
    /// Sizes the arena so `slot_count` blocks fit without a later growth generation bump.
    pub(crate) fn for_slots(block_size: u64, slot_count: usize) -> Self {
        let stride = align_up(block_size.max(1), BENCH_UNIFORM_ALIGNMENT);
        let slots = u64::try_from(slot_count).unwrap_or(u64::MAX).max(1);
        let bytes = stride.saturating_mul(slots).max(BENCH_UNIFORM_ALIGNMENT);
        Self {
            allocator: MaterialUniformArenaAllocator::new(bytes, bytes, BENCH_UNIFORM_ALIGNMENT),
            block_size,
            pending: None,
        }
    }

    /// Inserts `material_asset_id` and marks it clean at `mutation_gen`.
    pub(crate) fn prime(
        &mut self,
        stem_hash: u64,
        material_asset_id: i32,
        texture_2d_asset_id: i32,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) -> Result<(), EmbeddedMaterialBindError> {
        if self
            .begin_rewrite(
                stem_hash,
                material_asset_id,
                texture_2d_asset_id,
                mutation_gen,
                texture_state_sig,
            )?
            .is_some()
        {
            self.finish_rewrite(mutation_gen, texture_state_sig);
        }
        Ok(())
    }

    /// Resolves an existing slot after a property change.
    ///
    /// Returns the dynamic offset when the slot must be rewritten.
    pub(crate) fn begin_rewrite(
        &mut self,
        stem_hash: u64,
        material_asset_id: i32,
        texture_2d_asset_id: i32,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) -> Result<Option<u32>, EmbeddedMaterialBindError> {
        let key = MaterialUniformCacheKey {
            stem_hash,
            material_asset_id,
            property_block_slot0: None,
            renderer_property_block_id: None,
            texture_2d_asset_id,
            shader_variant_bits: None,
            shader_source_generation: 0,
            texture_residency_generation: 0,
        };
        let resolved = self.allocator.resolve_slot(
            key,
            non_zero_u64(self.block_size)?,
            mutation_gen,
            texture_state_sig,
        )?;
        let offset = u32::try_from(resolved.offset).map_err(|_err| {
            EmbeddedMaterialBindError::from(format!(
                "material uniform arena offset {} exceeds dynamic offset range",
                resolved.offset
            ))
        })?;
        if !resolved.needs_write {
            self.pending = None;
            return Ok(None);
        }
        self.pending = Some(PendingUniformWrite {
            key,
            buffer_generation: resolved.buffer_generation,
        });
        Ok(Some(offset))
    }

    /// Records that the bytes for the pending rewrite were staged.
    pub(crate) fn finish_rewrite(&mut self, mutation_gen: u64, texture_state_sig: u64) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.allocator.mark_written(
            &pending.key,
            pending.buffer_generation,
            mutation_gen,
            texture_state_sig,
        );
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
