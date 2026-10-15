//! Headless render cadence: alternates full-frame and lock-step-only ticks.

use std::time::{Duration, Instant};

/// Headless tick flavor selected by the render cadence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HeadlessTickKind {
    /// Run a full render frame and optional readback.
    FullFrame,
    /// Keep lock-step IPC responsive without rendering.
    LockstepOnly,
}

/// Decides when the next full headless frame should run.
#[derive(Clone, Copy, Debug)]
pub(super) struct HeadlessSchedule {
    render_interval: Duration,
    next_full_frame_at: Instant,
}

impl HeadlessSchedule {
    /// Creates a schedule with a render cadence of `interval_ms` (clamped to >= 1 ms).
    pub(super) fn new(interval_ms: u64, now: Instant) -> Self {
        Self {
            render_interval: Duration::from_millis(interval_ms.max(1)),
            next_full_frame_at: now,
        }
    }

    /// Selects the kind of tick to run at `now`.
    pub(super) fn tick_kind(&self, now: Instant) -> HeadlessTickKind {
        if now >= self.next_full_frame_at {
            HeadlessTickKind::FullFrame
        } else {
            HeadlessTickKind::LockstepOnly
        }
    }

    /// Records that a full frame completed and schedules the next one.
    pub(super) fn complete_full_frame(&mut self, now: Instant) {
        self.next_full_frame_at = now + self.render_interval;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{HeadlessSchedule, HeadlessTickKind};

    #[test]
    fn first_tick_is_full_frame() {
        let now = Instant::now();
        let schedule = HeadlessSchedule::new(1000, now);
        assert_eq!(schedule.tick_kind(now), HeadlessTickKind::FullFrame);
    }

    #[test]
    fn after_full_frame_waits_until_interval_elapsed() {
        let now = Instant::now();
        let mut schedule = HeadlessSchedule::new(1000, now);
        schedule.complete_full_frame(now);
        assert_eq!(
            schedule.tick_kind(now + Duration::from_millis(999)),
            HeadlessTickKind::LockstepOnly
        );
        assert_eq!(
            schedule.tick_kind(now + Duration::from_millis(1000)),
            HeadlessTickKind::FullFrame
        );
    }

    #[test]
    fn zero_interval_is_clamped_to_one_millisecond() {
        let now = Instant::now();
        let mut schedule = HeadlessSchedule::new(0, now);
        schedule.complete_full_frame(now);
        assert_eq!(
            schedule.tick_kind(now + Duration::from_nanos(999_999)),
            HeadlessTickKind::LockstepOnly
        );
        assert_eq!(
            schedule.tick_kind(now + Duration::from_millis(1)),
            HeadlessTickKind::FullFrame
        );
    }
}
