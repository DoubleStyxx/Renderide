//! Clustered forward lighting: shared GPU buffers for per-cluster light lists.
//!
//! [`ClusterBufferCache`] tracks a grow-only high-water-mark over `cluster_light_counts`
//! (range rows) and `cluster_light_indices` -- the only two buffers shared across views. The
//! per-view compute
//! params uniform (`ClusterParams`) is **not** managed here; it lives in
//! `PerViewFrameState::cluster_params_buffer` to avoid a CPU-side write race during parallel
//! per-view recording (see `frame_resource_manager.rs`).

use std::mem::size_of;

use crate::gpu::{CLUSTER_LIGHT_RANGE_WORDS, GpuLimits};
pub use crate::world_mesh::cluster::{CLUSTER_COUNT_Z, TILE_SIZE};

/// References to the two storage buffers shared across all views.
///
/// `params_buffer` is intentionally **not** included: it is per-view and lives in
/// `PerViewFrameState::cluster_params_buffer` to prevent a CPU write-order race during
/// parallel rayon recording (multiple views share one graph upload sink, last write wins).
#[derive(Clone, Copy)]
pub struct ClusterBufferRefs<'a> {
    /// Two `u32` words per cluster: compact-index offset and count.
    pub cluster_light_counts: &'a wgpu::Buffer,
    /// Compact light-index storage addressed by the per-cluster range rows.
    pub cluster_light_indices: &'a wgpu::Buffer,
}

/// Shared cluster-buffer cache; grow-only high-water-mark across every active view so all views
/// can reference the same underlying storage. Bumps [`Self::version`] only when the underlying
/// buffers are reallocated (shrink is never performed), which drives bind-group invalidation for
/// every per-view consumer.
///
/// Correctness across views relies on WebGPU's in-order execution guarantee within a single
/// [`wgpu::Queue::submit`]: a view's clustered-light compute pass writes its lists, then its
/// raster reads them, all within one command buffer; the next view's command buffer (submitted
/// in order) overwrites only after the previous view's reads retire. This is why there is no
/// per-view offset -- each view uses range `[0..view_cluster_count)` in turn.
///
/// When `stereo` is true, range storage covers both eyes so the compute pass can write eye-0
/// clusters at `[0..N)` and eye-1 at `[N..2N)`. Compact index storage is sized from the largest
/// requested view layout.
pub struct ClusterBufferCache {
    cluster_light_counts: Option<wgpu::Buffer>,
    cluster_light_indices: Option<wgpu::Buffer>,
    /// High-water-mark configuration currently provisioned. Grow-only; never shrunk.
    cached_key: ClusterCacheKey,
    /// Incremented each time the buffers are reallocated to satisfy a larger request.
    pub version: u64,
}

/// Grow-only cache key: tracks the max viewport, Z slice count, stereo flag, and compact index
/// capacity observed so far.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct ClusterCacheKey {
    viewport: (u32, u32),
    cluster_count_z: u32,
    stereo: bool,
    index_capacity_words: u64,
}

impl ClusterBufferCache {
    /// Empty cache; [`Self::ensure_buffers`] allocates on first use.
    pub fn new() -> Self {
        Self {
            cluster_light_counts: None,
            cluster_light_indices: None,
            cached_key: ClusterCacheKey::default(),
            version: 0,
        }
    }

    /// Ensures buffers fit `viewport`, `cluster_count_z`, `stereo`, and
    /// `min_index_capacity_words`. **Grow-only**: if the current allocation already covers the
    /// request (equal or larger on every axis, with a one-way `mono -> stereo` transition), the
    /// existing buffers are reused and [`Self::version`] is not bumped. When a reallocation is
    /// required, the new high-water-mark replaces the previous allocation and `version` bumps to
    /// invalidate bind-group caches.
    ///
    /// When `stereo` is true, range storage is doubled to hold per-eye cluster data. Compact
    /// index storage uses the requested explicit capacity.
    ///
    /// Returns [`None`] when cluster storage would exceed [`GpuLimits`] storage/buffer caps.
    pub fn ensure_buffers(
        &mut self,
        device: &wgpu::Device,
        limits: &GpuLimits,
        viewport: (u32, u32),
        cluster_count_z: u32,
        stereo: bool,
        min_index_capacity_words: u64,
    ) -> Option<ClusterBufferRefs<'_>> {
        let plan = compute_cluster_cache_size(
            self.cached_key,
            limits,
            viewport,
            cluster_count_z,
            stereo,
            min_index_capacity_words,
        )?;
        self.reallocate_if_needed(device, plan);
        self.current_refs()
    }

    /// Allocates the shared cluster buffers when the cached key differs from the planned key,
    /// bumping [`Self::version`] so bind-group caches invalidate. No-op when the plan matches the
    /// already-allocated state.
    fn reallocate_if_needed(&mut self, device: &wgpu::Device, plan: ClusterAllocationPlan) {
        if self.cluster_light_counts.is_some() && self.cached_key == plan.new_key {
            return;
        }
        self.version = self.version.wrapping_add(1);
        self.cached_key = plan.new_key;

        self.cluster_light_counts = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cluster_light_counts"),
            size: plan.counts_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        crate::profiling::note_resource_churn!(Buffer, "backend::cluster_light_counts");
        self.cluster_light_indices = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cluster_light_indices"),
            size: plan.indices_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        crate::profiling::note_resource_churn!(Buffer, "backend::cluster_light_indices");
    }

    /// Returns refs to the currently-provisioned storage buffers, or [`None`] if not yet
    /// allocated. All views share these; see [`ClusterBufferCache`] for the ordering argument
    /// that makes GPU-side sharing safe. `params_buffer` is excluded -- it is per-view.
    pub fn current_refs(&self) -> Option<ClusterBufferRefs<'_>> {
        Some(ClusterBufferRefs {
            cluster_light_counts: self.cluster_light_counts.as_ref()?,
            cluster_light_indices: self.cluster_light_indices.as_ref()?,
        })
    }
}

impl Default for ClusterBufferCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Planned allocation footprint produced by [`compute_cluster_cache_size`] and consumed by
/// [`ClusterBufferCache::reallocate_if_needed`].
struct ClusterAllocationPlan {
    new_key: ClusterCacheKey,
    counts_bytes: u64,
    indices_bytes: u64,
}

/// Computes the grow-only allocation footprint for the next [`ClusterBufferCache::ensure_buffers`]
/// call. Returns [`None`] when the viewport is empty, the cluster math overflows, or the resulting
/// byte sizes exceed [`GpuLimits`] caps; in each failure case the matching `logger::warn!` is
/// emitted with the same message that [`ClusterBufferCache::ensure_buffers`] historically produced.
fn compute_cluster_cache_size(
    cached_key: ClusterCacheKey,
    limits: &GpuLimits,
    viewport: (u32, u32),
    cluster_count_z: u32,
    stereo: bool,
    min_index_capacity_words: u64,
) -> Option<ClusterAllocationPlan> {
    let (width, height) = viewport;
    if width == 0 || height == 0 {
        return None;
    }
    let new_max_w = width.max(cached_key.viewport.0);
    let new_max_h = height.max(cached_key.viewport.1);
    let new_max_z = cluster_count_z.max(cached_key.cluster_count_z);
    let new_max_stereo = stereo || cached_key.stereo;
    let cluster_count_x = new_max_w.div_ceil(TILE_SIZE);
    let cluster_count_y = new_max_h.div_ceil(TILE_SIZE);
    let Some(clusters_per_eye) = u64::from(cluster_count_x)
        .checked_mul(u64::from(cluster_count_y))
        .and_then(|xy| xy.checked_mul(u64::from(new_max_z)))
    else {
        logger::warn!(
            "cluster buffers: cluster grid {}x{}x{} overflows u64",
            cluster_count_x,
            cluster_count_y,
            new_max_z
        );
        return None;
    };
    let eye_multiplier = if new_max_stereo { 2_u64 } else { 1_u64 };
    let Some(total_clusters) = clusters_per_eye.checked_mul(eye_multiplier) else {
        logger::warn!(
            "cluster buffers: clusters_per_eye={} stereo={} overflows u64",
            clusters_per_eye,
            new_max_stereo
        );
        return None;
    };
    let Some(counts_bytes) = total_clusters
        .checked_mul(CLUSTER_LIGHT_RANGE_WORDS)
        .and_then(|words| words.checked_mul(size_of::<u32>() as u64))
    else {
        logger::warn!(
            "cluster buffers: total_clusters={} range byte size overflows u64",
            total_clusters
        );
        return None;
    };
    let index_capacity_words = min_index_capacity_words
        .max(1)
        .max(cached_key.index_capacity_words);
    let Some(indices_bytes) = index_capacity_words.checked_mul(size_of::<u32>() as u64) else {
        logger::warn!(
            "cluster buffers: index capacity {} byte size overflows u64",
            index_capacity_words
        );
        return None;
    };
    let max_bind = limits.max_storage_buffer_binding_size();
    let max_buf = limits.max_buffer_size();
    if counts_bytes > max_bind
        || indices_bytes > max_bind
        || counts_bytes > max_buf
        || indices_bytes > max_buf
    {
        logger::warn!(
            "cluster buffers: max viewport {:?} stereo={} would need ranges={} indices={} bytes; exceeds max_storage_buffer_binding_size ({}) or max_buffer_size ({})",
            (new_max_w, new_max_h),
            new_max_stereo,
            counts_bytes,
            indices_bytes,
            max_bind,
            max_buf
        );
        return None;
    }
    Some(ClusterAllocationPlan {
        new_key: ClusterCacheKey {
            viewport: (new_max_w, new_max_h),
            cluster_count_z: new_max_z,
            stereo: new_max_stereo,
            index_capacity_words,
        },
        counts_bytes,
        indices_bytes,
    })
}
