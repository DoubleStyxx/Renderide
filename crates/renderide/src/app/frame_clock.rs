//! Frame-start wall-clock timing for the winit driver.

use std::time::Instant;

/// Default wall-frame time used before a second frame has established a real delta.
const COLD_START_FRAME_TIME_MS: f64 = 16.67;

/// Wall-clock timing sample produced at the start of a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrameStartSample {
    /// Milliseconds since the end of the previous app tick.
    pub(crate) event_loop_idle_ms: Option<f64>,
    /// Milliseconds between consecutive frame starts.
    pub(crate) wall_frame_time_ms: f64,
}

/// Tracks frame timing anchors used by the HUD and desktop FPS caps.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FrameClock {
    last_frame_start: Option<Instant>,
    previous_tick_end: Option<Instant>,
}

impl FrameClock {
    /// Records a new frame start and returns the timing sample derived from previous anchors.
    pub(crate) fn begin_frame(&mut self, frame_start: Instant) -> FrameStartSample {
        let event_loop_idle_ms = self.previous_tick_end.map(|prev_end| {
            frame_start
                .saturating_duration_since(prev_end)
                .as_secs_f64()
                * 1000.0
        });
        let wall_frame_time_ms = self
            .last_frame_start
            .map_or(COLD_START_FRAME_TIME_MS, |prev| {
                frame_start.duration_since(prev).as_secs_f64() * 1000.0
            });
        self.last_frame_start = Some(frame_start);
        FrameStartSample {
            event_loop_idle_ms,
            wall_frame_time_ms,
        }
    }

    /// Records the end of the current app tick.
    pub(crate) fn end_tick(&mut self, tick_end: Instant) {
        self.previous_tick_end = Some(tick_end);
    }

    /// Last frame-start anchor used for redraw pacing.
    pub(crate) const fn last_frame_start(&self) -> Option<Instant> {
        self.last_frame_start
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::FrameClock;

    #[test]
    fn frame_clock_reports_wall_and_idle_deltas() {
        let t0 = Instant::now();
        let mut clock = FrameClock::default();
        let first = clock.begin_frame(t0);
        assert_eq!(first.event_loop_idle_ms, None);
        assert_eq!(first.wall_frame_time_ms, 16.67);

        clock.end_tick(t0 + Duration::from_millis(4));
        let second = clock.begin_frame(t0 + Duration::from_millis(20));
        assert_eq!(second.event_loop_idle_ms, Some(16.0));
        assert_eq!(second.wall_frame_time_ms, 20.0);
    }
}
