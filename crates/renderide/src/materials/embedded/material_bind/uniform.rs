//! Embedded `@group(1)` material uniform arena and upload.

use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use hashbrown::HashSet;
use lru::LruCache;

use super::super::embedded_material_bind_error::EmbeddedMaterialBindError;
use super::super::layout::StemMaterialLayout;
use super::super::texture_pools::EmbeddedTexturePools;
use super::super::uniform_pack::{
    MaterialUniformPackMetadata, UniformPackTextureContext,
    build_embedded_uniform_bytes_with_material_defaults,
};
use crate::frame_upload_batch::GraphUploadSink;
use crate::materials::host_data::{MaterialPropertyLookupIds, MaterialPropertyStore};

const INITIAL_MATERIAL_UNIFORM_ARENA_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(super) struct MaterialUniformCacheKey {
    pub(super) stem_hash: u64,
    pub(super) material_asset_id: i32,
    pub(super) property_block_slot0: Option<i32>,
    /// Renderer-level `MaterialPropertyBlock` id (applies to every material on the renderer).
    pub(super) renderer_property_block_id: Option<i32>,
    pub(super) texture_2d_asset_id: i32,
    pub(super) shader_variant_bits: Option<u32>,
    /// Custom-shader source generation. Packaged stems stay at zero.
    pub(super) shader_source_generation: u64,
    /// Fold of bound-texture residency, view/allocation generation, and content generation.
    ///
    /// Same value as the bind-group `texture_bind_signature`. A reupload or mip residency change
    /// must not reuse a slot packed against the previous stamp.
    pub(super) texture_residency_generation: u64,
}

/// GPU arena slot selected for a material uniform block.
#[derive(Clone)]
pub(super) struct MaterialUniformArenaSlotBinding {
    pub(super) buffer: Arc<wgpu::Buffer>,
    pub(super) dynamic_offset: u32,
    pub(super) size: NonZeroU64,
    pub(super) buffer_generation: u64,
}

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

/// Exact byte range returned by an evicted slot. Offsets stay aligned.
#[derive(Clone, Copy, Debug)]
struct FreeSpan {
    offset: u64,
    size: u64,
}

/// Pure allocator state for material uniform arena slots.
#[derive(Debug)]
struct MaterialUniformArenaAllocator {
    slots: LruCache<MaterialUniformCacheKey, MaterialUniformSlot>,
    /// Ranges whose owners were evicted. Same-sized inserts reuse these before the bump cursor.
    free: Vec<FreeSpan>,
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
            super::cache::max_cached_material_uniform_slots_per_shard(),
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
            cursor: 0,
            capacity,
            max_bytes,
            alignment: alignment.max(1),
            generation: 0,
            slot_evictions: 0,
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
            self.release_slot(evicted_slot);
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
            self.cursor = 0;
            self.generation = self.generation.wrapping_add(1);
            return;
        }
        for slot in removed {
            self.release_slot(slot);
        }
    }

    /// Returns `size` bytes. Exact free spans are reused in place. When the slot table is full,
    /// the LRU range is reclaimed before the arena grows. A bump that would still exceed capacity
    /// repacks live slots; a move bumps generation so the GPU buffer is replaced.
    fn allocate_offset(
        &mut self,
        size: u64,
        key: &MaterialUniformCacheKey,
    ) -> Result<u64, EmbeddedMaterialBindError> {
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
        if let Some(packed) = self.try_compact_for_new(size) {
            return Ok(packed);
        }
        let needed = self.bump_end(size)?;
        self.ensure_capacity(needed)?;
        self.try_bump(size)?.ok_or_else(|| {
            EmbeddedMaterialBindError::from("material uniform arena offset overflow".to_string())
        })
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

    /// Pops the LRU slot. Returns its offset when `size` matches so the caller can reuse it
    /// without putting that range back on the free list.
    fn reclaim_lru_for_insert(
        &mut self,
        size: u64,
    ) -> Result<Option<u64>, EmbeddedMaterialBindError> {
        let Some((evicted_key, evicted_slot)) = self.slots.pop_lru() else {
            return Ok(None);
        };
        let _ = evicted_key;
        self.note_slot_evictions(1);
        if evicted_slot.size == size {
            return Ok(Some(evicted_slot.offset));
        }
        self.release_slot(evicted_slot);
        if let Some(offset) = self.take_free(size) {
            return Ok(Some(offset));
        }
        self.try_bump(size)
    }

    fn take_free(&mut self, size: u64) -> Option<u64> {
        let index = self.free.iter().position(|span| span.size == size)?;
        Some(self.free.swap_remove(index).offset)
    }

    fn release_slot(&mut self, slot: MaterialUniformSlot) {
        if slot.size == 0 {
            return;
        }
        self.free.push(FreeSpan {
            offset: slot.offset,
            size: slot.size,
        });
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
        for (key, offset) in packed {
            if let Some(slot) = self.slots.peek_mut(&key) {
                slot.offset = offset;
            }
        }
        self.free.clear();
        self.cursor = new_end;
        if moved {
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
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }
}

/// Shared GPU buffer plus slot allocator for embedded material uniform constants.
pub(super) struct MaterialUniformArena {
    device: Arc<wgpu::Device>,
    allocator: MaterialUniformArenaAllocator,
    buffer: Arc<wgpu::Buffer>,
    /// Reused CPU bytes for the next dirty material on this shard.
    pack_scratch: Vec<u8>,
}

impl MaterialUniformArena {
    pub(super) fn new(device: Arc<wgpu::Device>, limits: Arc<crate::gpu::GpuLimits>) -> Self {
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
            pack_scratch: Vec::new(),
        }
    }

    fn resolve_binding(
        &mut self,
        key: MaterialUniformCacheKey,
        size: NonZeroU64,
        mutation_gen: u64,
        texture_state_sig: u64,
    ) -> Result<(MaterialUniformArenaSlotBinding, bool), EmbeddedMaterialBindError> {
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

    fn mark_written(
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

    pub(super) fn purge_material_assets(
        &mut self,
        material_ids: &HashSet<i32>,
        property_block_ids: &HashSet<i32>,
    ) -> usize {
        self.allocator
            .purge_material_assets(material_ids, property_block_ids)
    }

    pub(super) fn evict_stem(&mut self, stem_hash: u64) -> usize {
        self.allocator.evict_stem(stem_hash)
    }

    pub(super) fn evict_stem_generation(
        &mut self,
        stem_hash: u64,
        shader_source_generation: u64,
    ) -> usize {
        self.allocator
            .evict_stem_generation(stem_hash, shader_source_generation)
    }

    pub(super) fn slot_count(&self) -> usize {
        self.allocator.slot_count()
    }

    pub(super) fn eviction_count(&self) -> u64 {
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

fn non_zero_u64(value: u64) -> Result<NonZeroU64, EmbeddedMaterialBindError> {
    NonZeroU64::new(value).ok_or_else(|| {
        EmbeddedMaterialBindError::from("material uniform block has zero size".to_string())
    })
}

/// Uniform arena lookup and upload request for [`super::EmbeddedMaterialBindResources`].
pub(super) struct EmbeddedUniformArenaRequest<'a> {
    pub(super) uploads: GraphUploadSink<'a>,
    pub(super) stem: &'a str,
    pub(super) shader_variant_bits: Option<u32>,
    pub(super) layout: &'a Arc<StemMaterialLayout>,
    pub(super) uniform_key: &'a MaterialUniformCacheKey,
    pub(super) mutation_gen: u64,
    pub(super) store: &'a MaterialPropertyStore,
    pub(super) lookup: MaterialPropertyLookupIds,
    pub(super) pools: &'a EmbeddedTexturePools<'a>,
    pub(super) primary_texture_2d: i32,
    pub(super) texture_state_sig: u64,
}

use super::EmbeddedMaterialBindResources;

impl EmbeddedMaterialBindResources {
    /// Shared dynamic uniform arena slot for embedded `@group(1)` material constants.
    ///
    /// Refreshes bytes when [`MaterialPropertyStore`] mutates, when texture-derived uniform state
    /// changes, or when the arena grows to a new backing buffer generation.
    ///
    /// `resolve_binding`, the uniform byte build, the `GraphUploadSink::write_buffer` call, and
    /// `mark_written` all run under one arena shard lock. Splitting them across two critical
    /// sections allowed `slot.last_written_(generation|texture_state_sig)` to record a
    /// `(mutation_gen, texture_state_sig)` snapshot that disagreed with the bytes actually
    /// uploaded if any concurrent caller mutated relevant state between the resolve and the
    /// mark - a later cache hit could then skip a needed rewrite and let one material draw
    /// with another material's uniforms. `write_buffer` is a deferred queue push that does no
    /// GPU synchronization, so holding the shard lock across it is cheap.
    ///
    /// The arena is sharded by [`MaterialUniformCacheKey`] so concurrent rayon recording workers
    /// hitting distinct keys land on distinct shards and never block one another. A given key
    /// always routes to the same shard, so the per-shard slot table and `buffer_generation`
    /// counter remain self-consistent.
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the arena shard lock is intentionally held across resolve_binding, the uniform byte build, write_buffer, and mark_written so slot tracking cannot disagree with the uploaded bytes"
    )]
    pub(super) fn get_or_update_embedded_uniform_arena_slot(
        &self,
        req: EmbeddedUniformArenaRequest<'_>,
    ) -> Result<MaterialUniformArenaSlotBinding, EmbeddedMaterialBindError> {
        profiling::scope!("materials::embedded_uniform_arena");
        let EmbeddedUniformArenaRequest {
            uploads,
            stem,
            shader_variant_bits,
            layout,
            uniform_key,
            mutation_gen,
            store,
            lookup,
            pools,
            primary_texture_2d,
            texture_state_sig,
        } = req;
        let uniform_size = non_zero_u64(
            layout
                .reflected
                .material_uniform
                .as_ref()
                .ok_or_else(|| {
                    EmbeddedMaterialBindError::from(format!(
                        "stem {stem}: uniform block missing (shader has no material uniform)"
                    ))
                })?
                .total_size
                .into(),
        )?;
        let tex_ctx = UniformPackTextureContext {
            pools,
            primary_texture_2d,
        };

        profiling::scope!("materials::embedded_uniform_arena_critical_section");
        let mut arena = self.uniform_arena_shard(uniform_key).lock();
        let (binding, needs_write) =
            arena.resolve_binding(*uniform_key, uniform_size, mutation_gen, texture_state_sig)?;
        if needs_write {
            profiling::scope!("materials::embedded_uniform_arena_write");
            let quantize_toggle_floats =
                stem.starts_with(crate::materials::custom_shader::CUSTOM_STEM_PREFIX);
            if let Some(plan) = layout.uniform_pack_plan.as_ref() {
                if !plan.pack_into(
                    store,
                    lookup,
                    &tex_ctx,
                    shader_variant_bits,
                    &mut arena.pack_scratch,
                ) {
                    return Err(EmbeddedMaterialBindError::from(format!(
                        "stem {stem}: uniform block missing (shader has no material uniform)"
                    )));
                }
                uploads.write_buffer(
                    binding.buffer.as_ref(),
                    u64::from(binding.dynamic_offset),
                    &arena.pack_scratch,
                );
            } else {
                let uniform_bytes = build_embedded_uniform_bytes_with_material_defaults(
                    &layout.reflected,
                    layout.ids.as_ref(),
                    &MaterialUniformPackMetadata {
                        value_spaces: &layout.uniform_value_spaces,
                        material_defaults: &layout.uniform_default_by_field,
                        quantize_toggle_floats,
                    },
                    store,
                    lookup,
                    &tex_ctx,
                    shader_variant_bits,
                )
                .ok_or_else(|| {
                    format!("stem {stem}: uniform block missing (shader has no material uniform)")
                })?;
                uploads.write_buffer(
                    binding.buffer.as_ref(),
                    u64::from(binding.dynamic_offset),
                    &uniform_bytes,
                );
            }
            arena.mark_written(uniform_key, &binding, mutation_gen, texture_state_sig);
        } else {
            profiling::scope!("materials::embedded_uniform_arena_hit");
        }
        Ok(binding)
    }
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
mod tests {
    use super::*;

    fn key(material_asset_id: i32) -> MaterialUniformCacheKey {
        MaterialUniformCacheKey {
            stem_hash: 7,
            material_asset_id,
            property_block_slot0: None,
            renderer_property_block_id: None,
            texture_2d_asset_id: -1,
            shader_variant_bits: None,
            shader_source_generation: 0,
            texture_residency_generation: 0,
        }
    }

    fn key_with_block(material_asset_id: i32, property_block_id: i32) -> MaterialUniformCacheKey {
        MaterialUniformCacheKey {
            stem_hash: 7,
            material_asset_id,
            property_block_slot0: Some(property_block_id),
            renderer_property_block_id: None,
            texture_2d_asset_id: -1,
            shader_variant_bits: None,
            shader_source_generation: 0,
            texture_residency_generation: 0,
        }
    }

    #[test]
    fn arena_allocator_aligns_offsets() {
        let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
        let size = NonZeroU64::new(80).unwrap();

        let a = allocator.resolve_slot(key(1), size, 0, 0).unwrap();
        let b = allocator.resolve_slot(key(2), size, 0, 0).unwrap();

        assert_eq!(a.offset, 0);
        assert_eq!(b.offset, 256);
    }

    #[test]
    fn arena_allocator_property_change_rewrites_slot_without_new_buffer() {
        let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
        let size = NonZeroU64::new(64).unwrap();
        let first = allocator.resolve_slot(key(1), size, 1, 0).unwrap();
        allocator.mark_written(&key(1), first.buffer_generation, 1, 0);

        let updated = allocator.resolve_slot(key(1), size, 2, 0).unwrap();

        assert!(updated.needs_write);
        assert_eq!(updated.offset, first.offset);
        assert_eq!(updated.buffer_generation, first.buffer_generation);
        assert_eq!(allocator.generation(), first.buffer_generation);
    }

    #[test]
    fn arena_allocator_reuses_stable_slot() {
        let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
        let size = NonZeroU64::new(80).unwrap();

        let first = allocator.resolve_slot(key(1), size, 3, 5).unwrap();
        allocator.mark_written(&key(1), first.buffer_generation, 3, 5);
        let second = allocator.resolve_slot(key(1), size, 3, 5).unwrap();

        assert_eq!(first.offset, second.offset);
        assert!(!second.needs_write);
    }

    #[test]
    fn arena_allocator_growth_invalidates_existing_slots_for_new_generation() {
        let mut allocator = MaterialUniformArenaAllocator::new(256, 2048, 256);
        let size = NonZeroU64::new(128).unwrap();

        let first = allocator.resolve_slot(key(1), size, 1, 1).unwrap();
        allocator.mark_written(&key(1), first.buffer_generation, 1, 1);
        assert_eq!(allocator.generation(), 0);

        let _second = allocator.resolve_slot(key(2), size, 1, 1).unwrap();
        assert_eq!(allocator.generation(), 1);
        let first_after_growth = allocator.resolve_slot(key(1), size, 1, 1).unwrap();

        assert_eq!(first_after_growth.offset, first.offset);
        assert_eq!(first_after_growth.buffer_generation, 1);
        assert!(first_after_growth.needs_write);
    }

    #[test]
    fn arena_allocator_purges_material_and_property_block_slots() {
        let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
        let size = NonZeroU64::new(80).unwrap();
        allocator.resolve_slot(key(1), size, 0, 0).unwrap();
        allocator
            .resolve_slot(key_with_block(2, 20), size, 0, 0)
            .unwrap();
        allocator.resolve_slot(key(3), size, 0, 0).unwrap();

        let mut materials = HashSet::new();
        materials.insert(1);
        let mut blocks = HashSet::new();
        blocks.insert(20);
        assert_eq!(allocator.purge_material_assets(&materials, &blocks), 2);

        assert!(!allocator.slots.contains(&key(1)));
        assert!(!allocator.slots.contains(&key_with_block(2, 20)));
        assert!(allocator.slots.contains(&key(3)));
    }

    #[test]
    fn uniform_key_tracks_shader_generation_and_texture_residency() {
        let base = key(1);
        let mut generation = base;
        generation.shader_source_generation = 4;
        let mut residency = base;
        residency.texture_residency_generation = 9;

        assert_ne!(base, generation);
        assert_ne!(base, residency);
    }

    #[test]
    fn reinstall_drops_old_generation_uniform_slots() {
        let mut allocator =
            MaterialUniformArenaAllocator::with_slot_capacity(1024, 4096, 256, nonzero_slots(4));
        let size = NonZeroU64::new(80).unwrap();
        let mut old = key(1);
        old.shader_source_generation = 1;
        let mut current = key(1);
        current.shader_source_generation = 2;
        let other_stem = MaterialUniformCacheKey {
            stem_hash: 8,
            shader_source_generation: 1,
            ..key(1)
        };

        allocator.resolve_slot(old, size, 0, 0).unwrap();
        allocator.resolve_slot(current, size, 0, 0).unwrap();
        allocator.resolve_slot(other_stem, size, 0, 0).unwrap();

        assert_eq!(allocator.evict_stem_generation(old.stem_hash, 1), 1);
        assert!(!allocator.slots.contains(&old));
        assert!(allocator.slots.contains(&current));
        assert!(allocator.slots.contains(&other_stem));
        assert_eq!(allocator.eviction_count(), 1);
    }

    #[test]
    fn uniform_slot_lru_cap_holds() {
        let mut allocator =
            MaterialUniformArenaAllocator::with_slot_capacity(4096, 8192, 64, nonzero_slots(2));
        let size = NonZeroU64::new(64).unwrap();

        allocator.resolve_slot(key(1), size, 0, 0).unwrap();
        allocator.resolve_slot(key(2), size, 0, 0).unwrap();
        allocator.resolve_slot(key(3), size, 0, 0).unwrap();

        assert_eq!(allocator.slot_count(), 2);
        assert!(!allocator.slots.contains(&key(1)));
        assert!(allocator.slots.contains(&key(2)));
        assert!(allocator.slots.contains(&key(3)));
        assert_eq!(allocator.eviction_count(), 1);
    }

    #[test]
    fn mutation_rewrites_slot_without_new_offset_or_generation() {
        let mut allocator = MaterialUniformArenaAllocator::new(1024, 4096, 256);
        let size = NonZeroU64::new(64).unwrap();
        let first = allocator.resolve_slot(key(1), size, 1, 0).unwrap();
        allocator.mark_written(&key(1), first.buffer_generation, 1, 0);

        let updated = allocator.resolve_slot(key(1), size, 2, 0).unwrap();

        assert!(updated.needs_write);
        assert_eq!(updated.offset, first.offset);
        assert_eq!(updated.buffer_generation, first.buffer_generation);
        assert_eq!(allocator.generation(), 0);
        assert_eq!(allocator.capacity(), 1024);
    }

    #[test]
    fn evict_then_insert_reuses_offsets_without_growing() {
        let live = 4usize;
        let mut allocator = MaterialUniformArenaAllocator::with_slot_capacity(
            2048,
            8192,
            256,
            nonzero_slots(live * 2),
        );
        let size = NonZeroU64::new(80).unwrap();
        let mut freed = Vec::new();
        for id in 0..live {
            freed.push(
                allocator
                    .resolve_slot(key(i32::try_from(id).unwrap_or(i32::MAX)), size, 0, 0)
                    .unwrap()
                    .offset,
            );
        }
        for id in live..live * 2 {
            allocator
                .resolve_slot(key(i32::try_from(id).unwrap_or(i32::MAX)), size, 0, 0)
                .unwrap();
        }
        let capacity = allocator.capacity();
        let generation = allocator.generation();
        let mut materials = HashSet::new();
        for id in 0..live {
            materials.insert(i32::try_from(id).unwrap_or(i32::MAX));
        }
        assert_eq!(
            allocator.purge_material_assets(&materials, &HashSet::new()),
            live
        );

        let mut reused = Vec::new();
        for id in 100..100 + live {
            reused.push(
                allocator
                    .resolve_slot(key(i32::try_from(id).unwrap_or(i32::MAX)), size, 0, 0)
                    .unwrap()
                    .offset,
            );
        }
        freed.sort_unstable();
        reused.sort_unstable();
        assert_eq!(reused, freed);
        assert_eq!(allocator.capacity(), capacity);
        assert_eq!(allocator.generation(), generation);
        assert_eq!(allocator.slot_count(), live * 2);
    }

    #[test]
    fn churn_keeps_arena_capacity_bounded() {
        let mut allocator =
            MaterialUniformArenaAllocator::with_slot_capacity(1024, 65_536, 256, nonzero_slots(4));
        let size = NonZeroU64::new(64).unwrap();
        let generation = allocator.generation();

        for id in 0..128 {
            allocator.resolve_slot(key(id), size, 0, 0).unwrap();
        }

        assert_eq!(allocator.slot_count(), 4);
        assert_eq!(allocator.capacity(), 1024);
        assert_eq!(allocator.generation(), generation);
    }

    #[test]
    fn reallocation_bumps_generation_when_free_span_cannot_fit() {
        let mut allocator =
            MaterialUniformArenaAllocator::with_slot_capacity(512, 8192, 256, nonzero_slots(8));
        let small = NonZeroU64::new(64).unwrap();
        allocator.resolve_slot(key(1), small, 0, 0).unwrap();
        allocator.resolve_slot(key(2), small, 0, 0).unwrap();
        let mut drop_first = HashSet::new();
        drop_first.insert(1);
        assert_eq!(
            allocator.purge_material_assets(&drop_first, &HashSet::new()),
            1
        );
        assert_eq!(allocator.generation(), 0);
        let before_capacity = allocator.capacity();

        let allocated = allocator
            .resolve_slot(key(3), NonZeroU64::new(400).unwrap(), 0, 0)
            .unwrap();

        assert!(allocator.generation() > 0);
        assert!(allocator.capacity() > before_capacity);
        assert_eq!(allocated.buffer_generation, allocator.generation());
        assert!(allocated.needs_write);
    }

    #[test]
    fn compaction_bumps_generation_without_growing() {
        let mut allocator =
            MaterialUniformArenaAllocator::with_slot_capacity(1024, 1024, 256, nonzero_slots(8));
        let small = NonZeroU64::new(80).unwrap();
        let first = allocator.resolve_slot(key(1), small, 1, 1).unwrap();
        allocator.mark_written(&key(1), first.buffer_generation, 1, 1);
        allocator.resolve_slot(key(2), small, 1, 1).unwrap();
        let third = allocator.resolve_slot(key(3), small, 1, 1).unwrap();
        allocator.mark_written(&key(3), third.buffer_generation, 1, 1);
        let mut drop_middle = HashSet::new();
        drop_middle.insert(2);
        assert_eq!(
            allocator.purge_material_assets(&drop_middle, &HashSet::new()),
            1
        );
        assert_eq!(allocator.generation(), 0);

        let allocated = allocator
            .resolve_slot(key(4), NonZeroU64::new(300).unwrap(), 0, 0)
            .unwrap();

        assert_eq!(allocator.capacity(), 1024);
        assert_eq!(allocator.generation(), 1);
        assert_eq!(allocated.buffer_generation, 1);
        let repacked = allocator.resolve_slot(key(3), small, 1, 1).unwrap();
        assert!(repacked.needs_write);
        assert_eq!(repacked.buffer_generation, 1);
        assert!(repacked.offset < third.offset);
    }

    #[test]
    fn uniform_key_covers_pr24_material_inputs() {
        let base = MaterialUniformCacheKey {
            property_block_slot0: Some(2),
            renderer_property_block_id: Some(3),
            texture_2d_asset_id: 4,
            shader_variant_bits: Some(1),
            shader_source_generation: 5,
            texture_residency_generation: 6,
            ..key(1)
        };

        assert_ne!(
            base,
            MaterialUniformCacheKey {
                material_asset_id: 2,
                ..base
            }
        );
        assert_ne!(
            base,
            MaterialUniformCacheKey {
                property_block_slot0: Some(9),
                ..base
            }
        );
        assert_ne!(
            base,
            MaterialUniformCacheKey {
                renderer_property_block_id: Some(8),
                ..base
            }
        );
        assert_ne!(
            base,
            MaterialUniformCacheKey {
                texture_2d_asset_id: 5,
                ..base
            }
        );
        assert_ne!(
            base,
            MaterialUniformCacheKey {
                shader_variant_bits: Some(2),
                ..base
            }
        );
        assert_ne!(
            base,
            MaterialUniformCacheKey {
                shader_source_generation: 6,
                ..base
            }
        );
        assert_ne!(
            base,
            MaterialUniformCacheKey {
                texture_residency_generation: 7,
                ..base
            }
        );
    }

    fn nonzero_slots(cap: usize) -> NonZeroUsize {
        NonZeroUsize::new(cap).unwrap_or(NonZeroUsize::MIN)
    }
}
