//! CPU bind-group cache used by `bench-api`.
//!
//! The production cache stores `Arc<wgpu::BindGroup>`. These helpers use the same
//! [`ShardedLru`] construction, shard count, capacity, key builder, and hit/miss/evict
//! counters. The value is an `Arc<u64>` so a hit still pays an `Arc` clone and no GPU
//! device is opened. `wgpu::Device::create_bind_group` stays on the GPU path.

use std::sync::Arc;

use crate::frame_contract::OffscreenWriteTarget;
use crate::gpu_resource::{AtomicCacheCounters, ShardedLru};
use crate::materials::host_data::MaterialPropertyLookupIds;

use super::cache::{EMBEDDED_CACHE_SHARDS, max_cached_embedded_bind_groups};
use super::{MaterialBindCacheKey, build_material_bind_cache_key_parts};

/// Sharded LRU shaped like the embedded `@group(1)` bind-group cache.
pub(crate) struct CpuMaterialBindGroupCache {
    cache: ShardedLru<MaterialBindCacheKey, Arc<u64>>,
    stats: AtomicCacheCounters,
}

impl CpuMaterialBindGroupCache {
    /// Empty cache with the production shard count and capacity.
    pub(crate) fn new() -> Self {
        Self {
            cache: ShardedLru::new(max_cached_embedded_bind_groups(), EMBEDDED_CACHE_SHARDS),
            stats: AtomicCacheCounters::default(),
        }
    }

    /// Promotes `key` and records a hit. Returns the stored signature token.
    pub(crate) fn lookup_hit(&self, key: &MaterialBindCacheKey) -> Option<u64> {
        let hit = self.cache.get_cloned(key);
        if hit.is_some() {
            self.stats.note_hit();
        }
        hit.map(|token| *token)
    }

    /// Miss path: record a miss, insert `token`, and count an LRU eviction when one happens.
    pub(crate) fn rebuild_miss(&self, key: MaterialBindCacheKey, token: u64) -> u64 {
        self.stats.note_miss();
        let evicted = self.cache.put(key, Arc::new(token));
        self.stats.note_insertion();
        if evicted.is_some() {
            self.stats.note_eviction();
        }
        token
    }

    /// Drops every entry and counts one eviction per dropped group, matching `clear_bind_cache`.
    pub(crate) fn invalidate(&self) -> usize {
        let dropped = self.cache.clear();
        for _ in 0..dropped {
            self.stats.note_eviction();
        }
        dropped
    }
}

/// Builds a [`MaterialBindCacheKey`] with the production canonicalization rules.
pub(crate) fn material_bind_cache_key_for_bench(
    stem_hash: u64,
    lookup: MaterialPropertyLookupIds,
    texture_bind_signature: u64,
    offscreen_write_target: OffscreenWriteTarget,
    uniform_arena_generation: Option<u64>,
    shader_source_generation: u64,
) -> MaterialBindCacheKey {
    build_material_bind_cache_key_parts(
        stem_hash,
        lookup,
        texture_bind_signature,
        offscreen_write_target,
        uniform_arena_generation,
        shader_source_generation,
    )
}
