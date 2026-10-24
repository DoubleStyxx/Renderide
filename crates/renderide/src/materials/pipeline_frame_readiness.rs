//! Count of draws whose requested material pipeline was still compiling.
//!
//! The counter is not cleared on a frame or view boundary. Every pending lookup, on
//! every view, adds to the same total until a headless PNG write takes a snapshot and
//! resets it. A frame that does not write a PNG leaves its draws in the total, so the
//! next PNG reports every pending draw since the previous write.
//!
//! The headless PNG writer stores that snapshot in a `tEXt` chunk of the PNG itself.
//! The golden harness refuses a frame until the count is zero, instead of skipping a
//! fixed number of non-flat images.

use std::sync::atomic::{AtomicU64, Ordering};

static PENDING_PIPELINE_DRAWS: AtomicU64 = AtomicU64::new(0);

/// Records one draw lookup that observed [`super::cache::MaterialPipelineLookup::Pending`].
pub(super) fn note_pending_pipeline_draw() {
    PENDING_PIPELINE_DRAWS.fetch_add(1, Ordering::Relaxed);
}

/// Returns how many pending-pipeline draws were noted since the last take, then resets the count.
///
/// Callers must treat the returned value as the sum across every view and every frame
/// since the previous take, not as a single view's count.
pub(crate) fn take_pending_pipeline_draws() -> u64 {
    PENDING_PIPELINE_DRAWS.swap(0, Ordering::Relaxed)
}
