//! Graceful windowed-driver shutdown coordination and exit-request handling.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use winit::event_loop::{ActiveEventLoop, ControlFlow};

use super::super::exit::ExitReason;
use super::AppDriver;
use super::target::RenderTarget;

/// Maximum time the winit driver will keep polling OpenXR shutdown before leaving the event loop.
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
/// Delay between shutdown polls while waiting for OpenXR lifecycle events or deferred finalizers.
const GRACEFUL_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Small state machine for the renderer's cooperative shutdown drain.
#[derive(Debug)]
pub(super) struct GracefulShutdown {
    started_at: Option<Instant>,
    openxr_exit_requested: bool,
    timeout: Duration,
    poll_interval: Duration,
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self {
            started_at: None,
            openxr_exit_requested: false,
            timeout: GRACEFUL_SHUTDOWN_TIMEOUT,
            poll_interval: GRACEFUL_SHUTDOWN_POLL_INTERVAL,
        }
    }
}

impl GracefulShutdown {
    /// Starts the shutdown drain. Returns `true` only on the first call.
    pub(super) fn begin(&mut self, now: Instant) -> bool {
        if self.started_at.is_some() {
            return false;
        }
        self.started_at = Some(now);
        true
    }

    /// Whether shutdown draining has started.
    pub(super) const fn is_started(&self) -> bool {
        self.started_at.is_some()
    }

    /// Whether the OpenXR session has already received `xrRequestExitSession`.
    pub(super) const fn openxr_exit_requested(&self) -> bool {
        self.openxr_exit_requested
    }

    /// Marks that `xrRequestExitSession` was attempted.
    pub(super) fn mark_openxr_exit_requested(&mut self) {
        self.openxr_exit_requested = true;
    }

    /// Returns whether the shutdown drain exceeded its bounded wait.
    pub(super) fn timed_out(&self, now: Instant) -> bool {
        self.started_at
            .is_some_and(|started_at| now.duration_since(started_at) >= self.timeout)
    }

    /// Configured shutdown timeout.
    pub(super) const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Poll cadence while the drain is pending.
    pub(super) const fn poll_interval(&self) -> Duration {
        self.poll_interval
    }
}

impl AppDriver {
    /// Records a normal exit request and either exits the event loop or starts the cooperative
    /// shutdown drain, depending on the reason's [`ExitReason::uses_graceful_shutdown`].
    pub(super) fn request_exit(&mut self, reason: ExitReason, event_loop: &dyn ActiveEventLoop) {
        let first_request = !self.exit_is_requested();
        let request = self.exit.borrow_mut().request(reason);
        if !request.reason().uses_graceful_shutdown() {
            event_loop.exit();
            return;
        }
        if first_request && self.shutdown.begin(Instant::now()) {
            self.runtime.begin_graceful_shutdown();
            // Suspend the main-thread watchdog for the rest of the process lifetime. The
            // cooperative drain, OpenXR exit handshake, target Drop, and GPU Drop are each
            // individually bounded but together can legitimately exceed the watchdog's
            // hang threshold; suppressing reports here matches the intent of
            // `WatchdogPause` and prevents misleading HANG lines during clean shutdown.
            if self.shutdown_watchdog_pause.is_none() {
                self.shutdown_watchdog_pause = self
                    .main_heartbeat
                    .as_ref()
                    .map(crate::diagnostics::Heartbeat::pause);
            }
            logger::info!("Graceful renderer shutdown started: {:?}", request.reason());
        }
        if self.openxr_frame_open() {
            return;
        }
        self.poll_graceful_shutdown(event_loop);
    }

    /// Polls the OS-driven cooperative shutdown coordinator and triggers exit if set.
    pub(super) fn check_external_shutdown(&mut self, event_loop: &dyn ActiveEventLoop) -> bool {
        let Some(coord) = self.external_shutdown.as_ref() else {
            return false;
        };
        if !coord.requested.load(Ordering::Relaxed) {
            return false;
        }
        if coord.log_when_checked {
            logger::info!("Graceful shutdown requested; exiting event loop");
        }
        self.request_exit(ExitReason::ExternalShutdown, event_loop);
        true
    }

    /// Drives the cooperative shutdown drain to either completion or the timeout deadline.
    pub(super) fn poll_graceful_shutdown(&mut self, event_loop: &dyn ActiveEventLoop) -> bool {
        if !self.shutdown.is_started() {
            return false;
        }

        let now = Instant::now();
        let target_complete = self
            .target
            .as_mut()
            .is_none_or(|target| target.poll_graceful_shutdown(&mut self.shutdown));
        let runtime_complete = self.runtime.graceful_shutdown_complete();
        let complete = target_complete && runtime_complete;

        if complete {
            logger::info!("Graceful renderer shutdown completed");
            event_loop.exit();
            return true;
        }

        if self.shutdown.timed_out(now) {
            logger::warn!(
                "Graceful renderer shutdown timed out after {}ms; exiting",
                self.shutdown.timeout().as_millis()
            );
            event_loop.exit();
            return true;
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(now + self.shutdown.poll_interval()));
        false
    }

    /// Whether an OpenXR frame is currently waiting on `xrEndFrame`.
    pub(super) fn openxr_frame_open(&self) -> bool {
        self.target
            .as_ref()
            .and_then(RenderTarget::xr_session)
            .is_some_and(|session| session.handles.xr_session.frame_open())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::GracefulShutdown;

    #[test]
    fn begin_only_reports_first_start() {
        let mut shutdown = GracefulShutdown::default();
        let now = Instant::now();
        assert!(shutdown.begin(now));
        assert!(!shutdown.begin(now + Duration::from_millis(1)));
        assert!(shutdown.is_started());
    }

    #[test]
    fn timeout_uses_start_instant() {
        let mut shutdown = GracefulShutdown::default();
        let now = Instant::now();
        shutdown.begin(now);
        let just_before_timeout = (now + shutdown.timeout())
            .checked_sub(Duration::from_millis(1))
            .expect("test timeout is larger than one millisecond");
        assert!(!shutdown.timed_out(just_before_timeout));
        assert!(shutdown.timed_out(now + shutdown.timeout()));
    }

    #[test]
    fn openxr_exit_request_is_tracked_once_marked() {
        let mut shutdown = GracefulShutdown::default();
        assert!(!shutdown.openxr_exit_requested());
        shutdown.mark_openxr_exit_requested();
        assert!(shutdown.openxr_exit_requested());
    }
}
