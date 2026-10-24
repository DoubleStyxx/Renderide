use crate::render_graph::resources::BufferSizePolicy;
use crate::render_graph::{BufferKey, TransientPool};

/// Distinct buffer keys reused by one [`BufferPoolWorkload::reuse_cycle`] sample.
pub const BUFFER_POOL_SLOTS: usize = 1_024;

/// Static many-light shadow planning workload.
pub use crate::backend::{SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload};

/// Free-list reuse of transient-pool buffer keys. No GPU buffer is allocated.
#[derive(Debug)]
pub struct BufferPoolWorkload {
    pool: TransientPool,
    keys: Vec<BufferKey>,
    ids: Vec<usize>,
}

impl BufferPoolWorkload {
    /// Fills [`BUFFER_POOL_SLOTS`] distinct keys, then releases them so the next cycle hits.
    #[must_use]
    pub fn standard() -> Self {
        Self::with_slots(BUFFER_POOL_SLOTS)
    }

    /// Allocates `slots` distinct key-only buffer entries and returns them to the free list.
    #[must_use]
    pub fn with_slots(slots: usize) -> Self {
        let mut pool = TransientPool::new();
        pool.begin_generation();
        let mut keys = Vec::with_capacity(slots);
        let mut ids = Vec::with_capacity(slots);
        for index in 0..slots {
            let size_index = u64::try_from(index).unwrap_or(0);
            let key = BufferKey {
                size_policy: BufferSizePolicy::Fixed(256 + size_index * 16),
                usage_bits: 0x0020,
            };
            let id = pool.acquire_buffer(key);
            keys.push(key);
            ids.push(id);
        }
        for id in &ids {
            pool.release_buffer(*id);
        }
        Self { pool, keys, ids }
    }

    /// Acquires every key again (free-list hit) and releases it.
    ///
    /// The returned checksum depends on the recycled entry ids.
    #[must_use]
    pub fn reuse_cycle(&mut self) -> usize {
        self.pool.begin_generation();
        let mut checksum = 0_usize;
        for index in 0..self.keys.len() {
            let key = self.keys[index];
            let id = self.pool.acquire_buffer(key);
            self.ids[index] = id;
            checksum = checksum.wrapping_add(id);
        }
        for id in &self.ids {
            self.pool.release_buffer(*id);
        }
        checksum
    }
}
