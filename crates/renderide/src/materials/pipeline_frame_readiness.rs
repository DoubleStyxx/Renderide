//! Count of draws the headless harness must not accept yet.
//!
//! Each [`PendingPipelineDraws`] belongs to one material registry. The count is not
//! cleared on a frame or view boundary. Every counted lookup, on every view, adds to
//! that registry's total until a headless PNG write takes a snapshot and resets it. A
//! frame that does not write a PNG leaves its draws in the total, so the next PNG
//! reports every counted draw since the previous write.
//!
//! A lookup that is still compiling counts. A draw that is skipped because no pipeline
//! was available counts too, unless that draw was already counted as pending. A failed
//! lookup that still draws through a ready Null pipeline does not count: the harness
//! compares that frame. Counting a skip on the same total keeps
//! [`crate::materials::MaterialRegistry::take_pending_pipeline_draws`] a single `u64`.
//!
//! The headless PNG writer stores that snapshot in a `tEXt` chunk of the PNG itself.
//! The golden harness refuses a frame until the count is zero, instead of skipping a
//! fixed number of non-flat images.
//!
//! On a render-thread compile (headless or a software adapter), a positive count also
//! means the next frame must not Hi-Z reject world meshes. Those draws were not
//! rasterized, so the pyramid from this frame is not evidence they are occluded. A
//! drain that compiled pipelines holds the same way: warmup can queue the build
//! without counting a draw. Real GPUs leave the flag clear.

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
    ///
    /// A failed lookup that still draws is not a skip. Call [`Self::note_dropped`]
    /// only when the draw is abandoned and this method did not already count it.
    pub(super) fn note(&self, lookup: &MaterialPipelineLookup) {
        if matches!(lookup, MaterialPipelineLookup::Pending) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A draw was skipped and no pending lookup was already counted for it.
    pub(super) fn note_dropped(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// How many draws are currently counted. Does not reset.
    pub(super) fn outstanding(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Returns how many pending-pipeline draws were noted since the last take, then resets the count.
    ///
    /// Callers must treat the returned value as the sum across every view and every frame
    /// since the previous take, not as a single view's count.
    pub(super) fn take(&self) -> u64 {
        self.count.swap(0, Ordering::Relaxed)
    }
}

/// Whether the next frame should skip Hi-Z rejection of world meshes.
///
/// Real GPUs pass `compile_on_render_thread = false` and always return false. A
/// render-thread compile returns true while any draw is still counted, or when this
/// frame itself compiled pipelines. Warmup can queue those builds without counting a
/// draw, and the pyramid from the compile frame does not contain the casters yet.
pub(super) fn suppress_hiz_while_pipelines_unready(
    compile_on_render_thread: bool,
    outstanding: u64,
    compiled_this_frame: bool,
) -> bool {
    compile_on_render_thread && (outstanding > 0 || compiled_this_frame)
}
