//! Log-flush cadence and log-level sync for the windowed driver.

use std::time::{Duration, Instant};

use super::super::bootstrap::effective_renderer_log_level;
use super::AppDriver;

/// Interval between log flushes when using file logging in the winit handler.
const LOG_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

/// Tracks when the renderer last flushed its file logger from the event loop.
#[derive(Debug)]
pub(super) struct LogFlushCadence {
    last_log_flush: Option<Instant>,
    interval: Duration,
}

impl Default for LogFlushCadence {
    fn default() -> Self {
        Self {
            last_log_flush: None,
            interval: LOG_FLUSH_INTERVAL,
        }
    }
}

impl LogFlushCadence {
    /// Flushes the file logger if at least `interval` has elapsed since the last flush.
    pub(super) fn flush_if_due(&mut self) {
        let now = Instant::now();
        let should = self
            .last_log_flush
            .is_none_or(|t| now.duration_since(t) >= self.interval);
        if should {
            logger::flush();
            self.last_log_flush = Some(now);
        }
    }
}

impl AppDriver {
    /// Flushes the file logger at most once per [`LOG_FLUSH_INTERVAL`].
    pub(super) fn flush_logs_if_due(&mut self) {
        profiling::scope!("app::flush_logs");
        self.log_flush.flush_if_due();
    }

    /// Applies the effective renderer log level from the current settings snapshot.
    pub(super) fn sync_log_level_from_settings(&self) {
        let log_verbose = self
            .runtime
            .settings()
            .read()
            .map(|s| s.debug.log_verbose)
            .unwrap_or(false);
        logger::set_max_level(effective_renderer_log_level(
            self.log_level_cli,
            log_verbose,
        ));
    }
}
