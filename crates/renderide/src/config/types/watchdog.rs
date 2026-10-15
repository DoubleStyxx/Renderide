//! Cooperative hang/hitch detection. Persisted as `[watchdog]`.

use serde::{Deserialize, Serialize};

use crate::labeled_enum;

/// Cooperative hang/hitch detection. Persisted as `[watchdog]`.
///
/// The watchdog thread inspects per-thread heartbeats every
/// [`Self::poll_interval_ms`] and reports a *hitch* when a heartbeat misses its pet by
/// [`Self::hitch_threshold_ms`], escalating to a *hang* (with stack capture) at
/// [`Self::hang_threshold_ms`]. See [`crate::diagnostics::Watchdog`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchdogSettings {
    /// Master toggle. When `false`, the watchdog thread is not spawned and `pet()` is a no-op.
    pub enabled: bool,
    /// Interval between watchdog poll iterations, in milliseconds. Lower values reduce hang
    /// detection latency at the cost of slightly more idle CPU on the watchdog thread.
    pub poll_interval_ms: u32,
    /// Hitch threshold in milliseconds. A heartbeat that has not been pet for this long is
    /// logged at `warn` level. `0` disables hitch reporting entirely.
    pub hitch_threshold_ms: u32,
    /// Hang threshold in milliseconds. A heartbeat that has not been pet for this long is
    /// treated as fatal-class: a stack trace of the stuck thread is captured (Linux/macOS via
    /// `pthread_kill`+`SIGUSR2`; Windows logs without stack capture for now) and emitted via
    /// [`logger::error!`]. Then [`Self::action`] is applied.
    pub hang_threshold_ms: u32,
    /// What to do after logging a hang report.
    pub action: WatchdogAction,
}

impl Default for WatchdogSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_ms: 250,
            hitch_threshold_ms: 100,
            hang_threshold_ms: 10_000,
            action: WatchdogAction::LogAndContinue,
        }
    }
}

labeled_enum! {
    /// Policy applied after a hang report has been written by [`crate::diagnostics::Watchdog`].
    pub enum WatchdogAction: "watchdog action" {
        default => LogAndContinue;

        /// Snapshot stacks, write the report, keep the renderer running. Default; lets a
        /// developer attach a debugger after the fact while preserving the chance that the stuck
        /// operation eventually unblocks on its own.
        LogAndContinue => {
            persist: "log_and_continue",
            label: "Log and continue",
        },
        /// Snapshot stacks, write the report, then `std::process::abort()`. Pair this with a
        /// supervisor (the bootstrapper's driver-health monitor) that restarts the renderer.
        LogAndAbort => {
            persist: "log_and_abort",
            label: "Log and abort",
        },
    }
}
