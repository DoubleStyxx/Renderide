//! Process-wide request to capture world-mesh draw counters for frame bench.
//!
//! The recorder stays in `app`. Runtime reads these flags when it builds
//! [`crate::diagnostics::DebugHudMetricInterest`] and when it decides whether a
//! frame may block on a queued compile, so those modules do not import `app`.

use std::sync::atomic::{AtomicBool, Ordering};

static DRAW_STATS: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Arms draw-stat capture for the rest of the process.
///
/// Called from `app` when `--frame-bench-out` is present.
pub(crate) fn set_frame_bench_draw_stats(enabled: bool) {
    DRAW_STATS.store(enabled, Ordering::Relaxed);
}

/// Records that this process is a frame bench, so non-capture frames stay async.
///
/// Called from `app` when `--frame-bench-out` is present, before GPU attach.
pub(crate) fn set_frame_bench_active(enabled: bool) {
    ACTIVE.store(enabled, Ordering::Relaxed);
}

/// Whether `--frame-bench-out` armed this process.
pub(crate) fn frame_bench_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Whether frame bench asked for world-mesh draw counters.
pub(crate) fn frame_bench_draw_stats() -> bool {
    DRAW_STATS.load(Ordering::Relaxed)
}
