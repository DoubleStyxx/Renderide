//! Per-frame CPU counters gathered on the render thread.
//!
//! The scratch is thread-local and inert until [`begin_frame`]. Hooks in the
//! render schedule call the `add_*` functions; they no-op when no control
//! session armed the probe.

use std::cell::Cell;
use std::time::Duration;

/// Counters for the frame currently being recorded.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FrameProbe {
    active: bool,
    /// Draw prep milliseconds.
    pub draw_prep_ms: f64,
    /// Cull milliseconds.
    pub cull_ms: f64,
    /// Upload milliseconds.
    pub upload_ms: f64,
    /// Submit milliseconds.
    pub submit_ms: f64,
    /// Queued draws.
    pub draw_calls: u64,
    /// Instances represented by those draws.
    pub instances: u64,
    /// Triangles represented by those draws.
    pub triangles: u64,
    /// Upload writes.
    pub upload_count: u64,
    /// Upload bytes.
    pub upload_bytes: u64,
}

thread_local! {
    static SCRATCH: Cell<FrameProbe> = const { Cell::new(FrameProbe::new()) };
}

impl FrameProbe {
    const fn new() -> Self {
        Self {
            active: false,
            draw_prep_ms: 0.0,
            cull_ms: 0.0,
            upload_ms: 0.0,
            submit_ms: 0.0,
            draw_calls: 0,
            instances: 0,
            triangles: 0,
            upload_count: 0,
            upload_bytes: 0,
        }
    }
}

/// Whether [`begin_frame`] armed the scratch on this thread.
pub(crate) fn is_active() -> bool {
    SCRATCH.with(|cell| cell.get().active)
}

/// Starts a new sample. Previous scratch is discarded.
pub(crate) fn begin_frame() {
    SCRATCH.set(FrameProbe {
        active: true,
        ..FrameProbe::new()
    });
}

/// Adds cull time.
pub(crate) fn add_cull(duration: Duration) {
    update(|probe| probe.cull_ms += duration_ms(duration));
}

/// Adds draw-prep time.
pub(crate) fn add_draw_prep(duration: Duration) {
    update(|probe| probe.draw_prep_ms += duration_ms(duration));
}

/// Adds draw counters.
pub(crate) fn add_draws(draws: u64, instances: u64, triangles: u64) {
    update(|probe| {
        probe.draw_calls = probe.draw_calls.saturating_add(draws);
        probe.instances = probe.instances.saturating_add(instances);
        probe.triangles = probe.triangles.saturating_add(triangles);
    });
}

/// Adds upload time and traffic.
pub(crate) fn add_upload(upload_ms: f64, count: u64, bytes: u64) {
    update(|probe| {
        probe.upload_ms += upload_ms;
        probe.upload_count = probe.upload_count.saturating_add(count);
        probe.upload_bytes = probe.upload_bytes.saturating_add(bytes);
    });
}

/// Adds submit enqueue time.
pub(crate) fn add_submit(submit_ms: f64) {
    update(|probe| probe.submit_ms += submit_ms);
}

/// Takes the current sample and clears the scratch.
pub(crate) fn take() -> FrameProbe {
    SCRATCH.with(|cell| {
        let probe = cell.get();
        cell.set(FrameProbe::new());
        probe
    })
}

fn update(mutate: impl FnOnce(&mut FrameProbe)) {
    SCRATCH.with(|cell| {
        let mut probe = cell.get();
        if probe.active {
            mutate(&mut probe);
            cell.set(probe);
        }
    });
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
