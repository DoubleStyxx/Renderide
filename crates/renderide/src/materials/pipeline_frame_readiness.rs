//! Count of draws whose requested material pipeline was still compiling.
//!
//! Each [`PendingPipelineDraws`] belongs to one material registry. The count is not
//! cleared on a frame or view boundary. Every pending lookup, on every view, adds to
//! that registry's total until a headless PNG write takes a snapshot and resets it. A
//! frame that does not write a PNG leaves its draws in the total, so the next PNG
//! reports every pending draw since the previous write.
//!
//! The headless PNG writer stores that snapshot in a `tEXt` chunk of the PNG itself.
//! The golden harness refuses a frame until the count is zero, instead of skipping a
//! fixed number of non-flat images.

use std::sync::atomic::{AtomicU64, Ordering};

use super::cache::MaterialPipelineLookup;

/// Per-registry count of draws that observed a still-compiling pipeline.
///
/// Instances do not share state. A test can count a requested lookup and its Null
/// fallback on one value without touching any other registry.
pub(super) struct PendingPipelineDraws {
    count: AtomicU64,
}

impl PendingPipelineDraws {
    pub(super) const fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
        }
    }

    /// A pending lookup counts. A failed or ready lookup does not.
    pub(super) fn note(&self, lookup: &MaterialPipelineLookup) {
        if matches!(lookup, MaterialPipelineLookup::Pending) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Returns how many pending-pipeline draws were noted since the last take, then resets the count.
    ///
    /// Callers must treat the returned value as the sum across every view and every frame
    /// since the previous take, not as a single view's count.
    pub(super) fn take(&self) -> u64 {
        self.count.swap(0, Ordering::Relaxed)
    }
}
