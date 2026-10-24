//! Per-frame HUD work counters.
//!
//! The atomics exist only for unit tests and the `hud-counters` feature. Release
//! builds compile every [`note_ui_frame`]-style call down to nothing, so the
//! hidden and open HUD frame cost stays the same as before the counters landed.
//!
//! Counts:
//! - `input_builds`: [`super::input::DebugHudInput::from_winit`] / frame input build
//! - `ui_frames`: `imgui.frame()`
//! - `draw_lists`: `imgui.render()` draw-list build
//! - `encodes`: the imgui-wgpu GPU pass
//! - `fmt_calls`: shared metric formatters in [`super::fmt`]
//! - `snapshot_clones`: HUD snapshot / per-view vec clones on the capture path
//! - `settings_locks`: HUD settings read or write locks

#[cfg(any(test, feature = "hud-counters"))]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(any(test, feature = "hud-counters"))]
mod atomics {
    use super::{AtomicU64, Ordering};

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

    #[cfg(test)]
    pub(crate) fn reset() {
        UI_FRAMES.store(0, Ordering::Relaxed);
        DRAW_LISTS.store(0, Ordering::Relaxed);
        ENCODES.store(0, Ordering::Relaxed);
        SNAPSHOT_CLONES.store(0, Ordering::Relaxed);
        FMT_CALLS.store(0, Ordering::Relaxed);
        INPUT_BUILDS.store(0, Ordering::Relaxed);
        SETTINGS_LOCKS.store(0, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn snapshot() -> super::HudWorkCounts {
        super::HudWorkCounts {
            ui_frames: UI_FRAMES.load(Ordering::Relaxed),
            draw_lists: DRAW_LISTS.load(Ordering::Relaxed),
            encodes: ENCODES.load(Ordering::Relaxed),
            snapshot_clones: SNAPSHOT_CLONES.load(Ordering::Relaxed),
            fmt_calls: FMT_CALLS.load(Ordering::Relaxed),
            input_builds: INPUT_BUILDS.load(Ordering::Relaxed),
            settings_locks: SETTINGS_LOCKS.load(Ordering::Relaxed),
        }
    }
}

#[cfg(not(any(test, feature = "hud-counters")))]
mod atomics {
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
    /// Shared metric formatter calls ([`super::fmt`]).
    pub(crate) fmt_calls: u64,
    /// HUD input snapshots built for a frame.
    pub(crate) input_builds: u64,
    /// Settings read/write locks taken for HUD work.
    pub(crate) settings_locks: u64,
}

#[cfg(test)]
pub(crate) use atomics::reset;

#[cfg(test)]
pub(crate) use atomics::snapshot;

/// Serializes tests that reset the process-wide counters.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    match LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[inline]
pub(crate) fn note_ui_frame() {
    atomics::ui_frame();
}

#[inline]
pub(crate) fn note_draw_list() {
    atomics::draw_list();
}

#[inline]
pub(crate) fn note_encode() {
    atomics::encode();
}

#[inline]
pub(crate) fn note_snapshot_clone() {
    atomics::snapshot_clone();
}

#[inline]
pub(crate) fn note_fmt_call() {
    atomics::fmt_call();
}

#[inline]
pub(crate) fn note_input_build() {
    atomics::input_build();
}

#[inline]
pub(crate) fn note_settings_lock() {
    atomics::settings_lock();
}
