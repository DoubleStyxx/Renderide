//! Idle-backoff decisions for the OpenXR frame loop.
//!
//! `xrPollEvent` does not block, and nothing else signals `session_running`
//! (it flips on this thread inside `poll_events`). A condvar cannot wake the
//! idle sleep. The idle budget is therefore a short poll loop: exit as soon as
//! the session is running, otherwise grow a delay that caps at the old 10ms
//! sleep and resets once the session runs so the next stop starts short.

use std::time::Duration;

/// First idle budget after the session stops or before the first `xrBeginSession`.
const SESSION_IDLE_BACKOFF_BASE: Duration = Duration::from_millis(1);
/// Upper bound on one idle `wait_frame` park. Same ceiling as the old fixed sleep.
const SESSION_IDLE_BACKOFF_CAP: Duration = Duration::from_millis(10);
/// Gap between `xrPollEvent` calls inside one idle budget.
const SESSION_IDLE_POLL_SLICE: Duration = Duration::from_millis(1);

/// Consecutive idle `wait_frame` calls while `xrBeginSession` has not run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SessionIdleWait {
    consecutive_idle_waits: u32,
}

impl SessionIdleWait {
    /// No idle waits recorded.
    pub(crate) const fn new() -> Self {
        Self {
            consecutive_idle_waits: 0,
        }
    }

    /// How long this idle `wait_frame` may park before yielding to the rest of the tick.
    pub(crate) fn budget(self) -> Duration {
        session_idle_backoff_delay(self.consecutive_idle_waits)
    }

    /// Records an idle wait that ended with the session still stopped.
    pub(crate) fn note_still_idle(&mut self) {
        self.consecutive_idle_waits = self.consecutive_idle_waits.saturating_add(1);
    }

    /// Clears the streak. Call when `session_running` is observed.
    pub(crate) fn reset(&mut self) {
        self.consecutive_idle_waits = 0;
    }
}

/// Backoff for idle wait `consecutive_idle_waits` (0-based).
///
/// `1ms, 2ms, 4ms, 8ms`, then the 10ms cap.
pub(crate) fn session_idle_backoff_delay(consecutive_idle_waits: u32) -> Duration {
    let shift = consecutive_idle_waits.min(4);
    let scaled = SESSION_IDLE_BACKOFF_BASE.saturating_mul(1_u32 << shift);
    scaled.min(SESSION_IDLE_BACKOFF_CAP)
}

/// Sleep between idle `xrPollEvent` calls, never longer than `remaining`.
pub(crate) fn session_idle_poll_slice(remaining: Duration) -> Duration {
    remaining.min(SESSION_IDLE_POLL_SLICE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_idle_backoff_grows_caps_and_resets() {
        let mut idle = SessionIdleWait::new();
        assert_eq!(idle.budget(), Duration::from_millis(1));
        idle.note_still_idle();
        assert_eq!(idle.consecutive_idle_waits, 1);
        assert_eq!(idle.budget(), Duration::from_millis(2));
        idle.note_still_idle();
        assert_eq!(idle.budget(), Duration::from_millis(4));
        idle.note_still_idle();
        assert_eq!(idle.budget(), Duration::from_millis(8));
        idle.note_still_idle();
        assert_eq!(idle.budget(), Duration::from_millis(10));
        idle.note_still_idle();
        assert_eq!(idle.budget(), Duration::from_millis(10));
        idle.reset();
        assert_eq!(idle.consecutive_idle_waits, 0);
        assert_eq!(idle.budget(), Duration::from_millis(1));
    }

    #[test]
    fn session_idle_poll_slice_tracks_remaining_budget() {
        assert_eq!(
            session_idle_poll_slice(Duration::from_millis(5)),
            Duration::from_millis(1)
        );
        assert_eq!(
            session_idle_poll_slice(Duration::from_micros(200)),
            Duration::from_micros(200)
        );
        assert_eq!(session_idle_poll_slice(Duration::ZERO), Duration::ZERO);
    }
}
