//! Per-frame HUD work counters.
//!
//! Under `cfg(test)` the counts are thread-local, so parallel tests cannot leak
//! `fmt_calls` or `settings_locks` into each other. The measured frame runs on
//! the test thread. The `hud-counters` feature keeps process-wide atomics for
//! non-test builds. Release builds compile every [`note_ui_frame`]-style call
//! down to nothing, so the hidden and open HUD frame cost stays the same as
//! before the counters landed.
//!
//! Counts:
//! - `input_builds`: [`super::input::DebugHudInput::from_winit`] / frame input build
//! - `ui_frames`: `imgui.frame()`
//! - `draw_lists`: `imgui.render()` draw-list build
//! - `encodes`: the imgui-wgpu GPU pass
//! - `fmt_calls`: [`super::fmt::f64_field`] and [`super::fmt::bytes_compact`] only, not every `format!`
//! - `snapshot_clones`: HUD snapshot / per-view vec clones on the capture path
//! - `settings_locks`: HUD settings read or write locks

#[cfg(test)]
mod storage {
    use std::cell::Cell;

    struct Counts {
        ui_frames: Cell<u64>,
        draw_lists: Cell<u64>,
        encodes: Cell<u64>,
        snapshot_clones: Cell<u64>,
        fmt_calls: Cell<u64>,
        input_builds: Cell<u64>,
        settings_locks: Cell<u64>,
    }

    impl Counts {
        const fn new() -> Self {
            Self {
                ui_frames: Cell::new(0),
                draw_lists: Cell::new(0),
                encodes: Cell::new(0),
                snapshot_clones: Cell::new(0),
                fmt_calls: Cell::new(0),
                input_builds: Cell::new(0),
                settings_locks: Cell::new(0),
            }
        }
    }

    thread_local! {
        static COUNTS: Counts = const { Counts::new() };
    }

    macro_rules! bump {
        ($field:ident) => {
            COUNTS.with(|counts| {
                let cell = &counts.$field;
                cell.set(cell.get() + 1);
            });
        };
    }

    #[inline]
    pub(super) fn ui_frame() {
        bump!(ui_frames);
    }
    #[inline]
    pub(super) fn draw_list() {
        bump!(draw_lists);
    }
    #[inline]
    pub(super) fn encode() {
        bump!(encodes);
    }
    #[inline]
    pub(super) fn snapshot_clone() {
        bump!(snapshot_clones);
    }
    #[inline]
    pub(super) fn fmt_call() {
        bump!(fmt_calls);
    }
    #[inline]
    pub(super) fn input_build() {
        bump!(input_builds);
    }
    #[inline]
    pub(super) fn settings_lock() {
        bump!(settings_locks);
    }

    pub(crate) fn reset() {
        COUNTS.with(|counts| {
            counts.ui_frames.set(0);
            counts.draw_lists.set(0);
            counts.encodes.set(0);
            counts.snapshot_clones.set(0);
            counts.fmt_calls.set(0);
            counts.input_builds.set(0);
            counts.settings_locks.set(0);
        });
    }

    pub(crate) fn snapshot() -> super::HudWorkCounts {
        COUNTS.with(|counts| super::HudWorkCounts {
            ui_frames: counts.ui_frames.get(),
            draw_lists: counts.draw_lists.get(),
            encodes: counts.encodes.get(),
            snapshot_clones: counts.snapshot_clones.get(),
            fmt_calls: counts.fmt_calls.get(),
            input_builds: counts.input_builds.get(),
            settings_locks: counts.settings_locks.get(),
        })
    }
}

#[cfg(all(not(test), feature = "hud-counters"))]
mod storage {
    use std::sync::atomic::{AtomicU64, Ordering};

    static UI_FRAMES: AtomicU64 = AtomicU64::new(0);
    static DRAW_LISTS: AtomicU64 = AtomicU64::new(0);
    static ENCODES: AtomicU64 = AtomicU64::new(0);
    static SNAPSHOT_CLONES: AtomicU64 = AtomicU64::new(0);
    static FMT_CALLS: AtomicU64 = AtomicU64::new(0);
    static INPUT_BUILDS: AtomicU64 = AtomicU64::new(0);
    static SETTINGS_LOCKS: AtomicU64 = AtomicU64::new(0);

    #[inline]
    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(super) fn ui_frame() {
        bump(&UI_FRAMES);
    }
    #[inline]
    pub(super) fn draw_list() {
        bump(&DRAW_LISTS);
    }
    #[inline]
    pub(super) fn encode() {
        bump(&ENCODES);
    }
    #[inline]
    pub(super) fn snapshot_clone() {
        bump(&SNAPSHOT_CLONES);
    }
    #[inline]
    pub(super) fn fmt_call() {
        bump(&FMT_CALLS);
    }
    #[inline]
    pub(super) fn input_build() {
        bump(&INPUT_BUILDS);
    }
    #[inline]
    pub(super) fn settings_lock() {
        bump(&SETTINGS_LOCKS);
    }
}

#[cfg(not(any(test, feature = "hud-counters")))]
mod storage {
    #[inline]
    pub(super) fn ui_frame() {}
    #[inline]
    pub(super) fn draw_list() {}
    #[inline]
    pub(super) fn encode() {}
    #[inline]
    pub(super) fn snapshot_clone() {}
    #[inline]
    pub(super) fn fmt_call() {}
    #[inline]
    pub(super) fn input_build() {}
    #[inline]
    pub(super) fn settings_lock() {}
}

/// One sample of [`super::work_counters`] after a measured stretch of frames.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HudWorkCounts {
    /// Calls to `imgui.frame()`.
    pub(crate) ui_frames: u64,
    /// Calls to `imgui.render()` (draw-list build).
    pub(crate) draw_lists: u64,
    /// Imgui-wgpu overlay passes.
    pub(crate) encodes: u64,
    /// Snapshot and per-view vec clones on the HUD capture path.
    pub(crate) snapshot_clones: u64,
    /// Calls to the HUD fmt helpers ([`super::fmt::f64_field`] and [`super::fmt::bytes_compact`]).
    ///
    /// This does not count every `format!` in the HUD.
    pub(crate) fmt_calls: u64,
    /// HUD input snapshots built for a frame.
    pub(crate) input_builds: u64,
    /// Settings read/write locks taken for HUD work.
    pub(crate) settings_locks: u64,
}

#[cfg(test)]
pub(crate) use storage::reset;

#[cfg(test)]
pub(crate) use storage::snapshot;

#[inline]
pub(crate) fn note_ui_frame() {
    storage::ui_frame();
}

#[inline]
pub(crate) fn note_draw_list() {
    storage::draw_list();
}

#[inline]
pub(crate) fn note_encode() {
    storage::encode();
}

#[inline]
pub(crate) fn note_snapshot_clone() {
    storage::snapshot_clone();
}

/// Counts one [`super::fmt::f64_field`] or [`super::fmt::bytes_compact`] call.
///
/// Other `format!` calls in the HUD are not counted.
#[inline]
pub(crate) fn note_fmt_call() {
    storage::fmt_call();
}

#[inline]
pub(crate) fn note_input_build() {
    storage::input_build();
}

#[inline]
pub(crate) fn note_settings_lock() {
    storage::settings_lock();
}
