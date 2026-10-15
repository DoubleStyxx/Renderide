//! Generic keyed GPU-object caches.

use std::hash::Hash;
use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::concurrency::{KeyedSingleFlight, SingleFlightPermit};

/// Generic locked cache with double-check insertion and optional clear-on-overflow eviction.
#[derive(Debug)]
struct GpuCache<K, V> {
    entries: Mutex<HashMap<K, V>>,
    max_entries: Option<usize>,
}

impl<K, V> Default for GpuCache<K, V> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            max_entries: None,
        }
    }
}

impl<K, V> GpuCache<K, V> {
    fn new() -> Self {
        Self::default()
    }

    fn with_max_entries(max_entries: usize) -> Self {
        debug_assert!(
            max_entries > 0,
            "GpuCache::with_max_entries requires a non-zero bound; otherwise every insert clears"
        );
        Self {
            entries: Mutex::new(HashMap::new()),
            max_entries: Some(max_entries),
        }
    }
}

impl<K, V> GpuCache<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    fn get(&self, key: &K) -> Option<V> {
        self.entries.lock().get(key).cloned()
    }
}

impl<K, V> GpuCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    fn get_or_create(&self, key: K, build: impl FnOnce(&K) -> V) -> V {
        {
            let guard = self.entries.lock();
            if let Some(existing) = guard.get(&key) {
                return existing.clone();
            }
        }

        let value = build(&key);
        let mut guard = self.entries.lock();
        if let Some(existing) = guard.get(&key) {
            return existing.clone();
        }
        if self
            .max_entries
            .is_some_and(|max_entries| guard.len() >= max_entries)
        {
            guard.clear();
        }
        guard.insert(key, value.clone());
        value
    }

    #[cfg(test)]
    fn clear(&self) {
        self.entries.lock().clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.lock().len()
    }
}

/// Typed cache for `wgpu::BindGroup` values.
#[derive(Debug)]
pub(crate) struct BindGroupMap<K> {
    inner: GpuCache<K, wgpu::BindGroup>,
}

impl<K> Default for BindGroupMap<K> {
    fn default() -> Self {
        Self {
            inner: GpuCache::new(),
        }
    }
}

impl<K> BindGroupMap<K>
where
    K: Clone + Eq + Hash,
{
    /// Creates an empty bind-group map with clear-on-overflow eviction.
    pub(crate) fn with_max_entries(max_entries: usize) -> Self {
        Self {
            inner: GpuCache::with_max_entries(max_entries),
        }
    }

    /// Returns a cached bind group or builds one outside the map lock.
    pub(crate) fn get_or_create(
        &self,
        key: K,
        build: impl FnOnce(&K) -> wgpu::BindGroup,
    ) -> wgpu::BindGroup {
        self.inner.get_or_create(key, build)
    }
}

/// Typed cache for `wgpu::RenderPipeline` values.
///
/// Backed by [`GpuCache`] for double-check insertion and a [`KeyedSingleFlight`] that serializes
/// concurrent compilations of the same key so each pipeline is built at most once.
#[derive(Debug)]
pub(crate) struct RenderPipelineMap<K>
where
    K: Eq + Hash,
{
    inner: GpuCache<K, Arc<wgpu::RenderPipeline>>,
    compiles: KeyedSingleFlight<K>,
}

impl<K> Default for RenderPipelineMap<K>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self {
            inner: GpuCache::new(),
            compiles: KeyedSingleFlight::default(),
        }
    }
}

impl<K> RenderPipelineMap<K>
where
    K: Clone + Eq + Hash,
{
    /// Returns a cached render pipeline or builds one outside the map lock.
    pub(crate) fn get_or_create(
        &self,
        key: K,
        build: impl FnOnce(&K) -> wgpu::RenderPipeline,
    ) -> Arc<wgpu::RenderPipeline> {
        loop {
            if let Some(existing) = self.inner.get(&key) {
                return existing;
            }

            let leader = match self.compiles.acquire(key.clone()) {
                SingleFlightPermit::Leader(leader) => leader,
                SingleFlightPermit::Waiter(waiter) => {
                    waiter.wait();
                    continue;
                }
            };

            let pipeline = self.inner.get_or_create(key, |k| Arc::new(build(k)));
            drop(leader);
            return pipeline;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GpuCache;

    #[test]
    fn cache_separates_keys_and_reuses_values() {
        let cache = GpuCache::<u32, u32>::new();

        assert_eq!(cache.get_or_create(1, |_| 10), 10);
        assert_eq!(cache.get_or_create(1, |_| 20), 10);
        assert_eq!(cache.get_or_create(2, |_| 20), 20);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn bounded_cache_clears_before_overflow_insert() {
        let cache = GpuCache::<u32, u32>::with_max_entries(2);

        assert_eq!(cache.get_or_create(1, |_| 10), 10);
        assert_eq!(cache.get_or_create(2, |_| 20), 20);
        assert_eq!(cache.len(), 2);

        assert_eq!(cache.get_or_create(3, |_| 30), 30);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get_or_create(1, |_| 40), 40);
    }

    #[test]
    fn cache_clear_drops_entries() {
        let cache = GpuCache::<u32, u32>::new();

        cache.get_or_create(1, |_| 10);
        cache.clear();

        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cache_get_returns_cloned_existing_value() {
        let cache = GpuCache::<u32, u32>::new();

        assert_eq!(cache.get(&1), None);
        cache.get_or_create(1, |_| 10);
        assert_eq!(cache.get(&1), Some(10));
        assert_eq!(cache.get(&2), None);
    }
}
