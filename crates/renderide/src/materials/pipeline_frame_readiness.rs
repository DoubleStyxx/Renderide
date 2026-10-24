//! Per-frame count of draws whose requested material pipeline was still compiling.
//!
//! The headless PNG writer snapshots this counter into a sidecar. The golden harness
//! refuses a frame until the count is zero, instead of skipping a fixed number of
//! non-flat images.

use std::sync::atomic::{AtomicU64, Ordering};

static PENDING_PIPELINE_DRAWS: AtomicU64 = AtomicU64::new(0);

/// Records one draw lookup that observed [`super::cache::MaterialPipelineLookup::Pending`].
pub(super) fn note_pending_pipeline_draw() {
    PENDING_PIPELINE_DRAWS.fetch_add(1, Ordering::Relaxed);
}

/// Returns how many pending-pipeline draws were noted since the last take, then resets the count.
pub(crate) fn take_pending_pipeline_draws() -> u64 {
    PENDING_PIPELINE_DRAWS.swap(0, Ordering::Relaxed)
}
