//! Process-wide request to capture world-mesh draw counters for frame bench.
//!
//! The recorder stays in `app`. Runtime reads this flag when it builds
//! [`crate::diagnostics::DebugHudMetricInterest`], so diagnostics does not import `app`.

use std::sync::atomic::{AtomicBool, Ordering};

static DRAW_STATS: AtomicBool = AtomicBool::new(false);

/// Arms draw-stat capture for the rest of the process.
pub(crate) fn set_frame_bench_draw_stats(enabled: bool) {
    DRAW_STATS.store(enabled, Ordering::Relaxed);
}

/// Whether frame bench asked for world-mesh draw counters.
pub(crate) fn frame_bench_draw_stats() -> bool {
    DRAW_STATS.load(Ordering::Relaxed)
}
