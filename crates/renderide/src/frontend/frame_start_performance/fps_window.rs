//! Count-based rolling FPS window for host performance samples.
//!
//! The window emits `frame_count / elapsed_seconds` once every
//! [`super::FPS_WINDOW`] worth of wall-clock spacing. Inside a window the
//! reported FPS is the value computed on the previous close, so the host-side
//! `Sync<float> FPS.Value` change events fire at the window cadence rather
//! than every frame.

use std::time::Instant;

use super::FPS_WINDOW;

/// Mutable count-based windowed FPS accumulator.
#[derive(Debug, Default)]
pub(crate) struct FpsWindow {
    /// Wall-clock anchor of the currently-open window, or [`None`] before the first tick.
    window_start: Option<Instant>,
    /// Number of ticks counted since `window_start`, not including the anchoring tick itself.
    counter: u32,
    /// Most recently emitted FPS value; `0.0` until the first window closes.
    last_fps: f32,
}

impl FpsWindow {
    /// Records one tick and rolls the window if [`FPS_WINDOW`] has elapsed since the anchor.
    pub(crate) fn record_tick(&mut self, now: Instant) {
        match self.window_start {
            None => {
                self.window_start = Some(now);
                self.counter = 0;
            }
            Some(start) => {
                self.counter = self.counter.saturating_add(1);
                let elapsed = now.duration_since(start);
                if elapsed >= FPS_WINDOW {
                    let elapsed_secs = elapsed.as_secs_f32();
                    if elapsed_secs > 0.0 {
                        self.last_fps = self.counter as f32 / elapsed_secs;
                    }
                    self.counter = 0;
                    self.window_start = Some(now);
                }
            }
        }
    }

    /// Most recently emitted windowed FPS value. `0.0` until the first window has closed.
    pub(crate) fn last_fps(&self) -> f32 {
        self.last_fps
    }
}
