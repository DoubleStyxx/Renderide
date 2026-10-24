//! Open-HUD proof that the work counters are wired.
//!
//! The 300-frame hidden proof lives in `runtime/debug_hud_frame.rs` so this module does not
//! depend on the runtime (that edge would cycle `backend -> diagnostics -> runtime`).

use crate::frontend::input::WindowInputAccumulator;

use super::fixture::HudMeasureView;
use super::input::frame_hud_input;
use super::ui::HudUi;
use super::work_counters::{self, HudWorkCounts};

fn assert_open_counters_wired(counts: HudWorkCounts) {
    assert!(
        counts.ui_frames > 0,
        "open HUD did not call imgui.frame(): {counts:?}"
    );
    assert!(
        counts.draw_lists > 0,
        "open HUD did not build a draw list: {counts:?}"
    );
    assert!(
        counts.fmt_calls > 0,
        "open HUD did not call metric formatters: {counts:?}"
    );
    assert!(
        counts.settings_locks > 0,
        "open HUD did not take a settings lock: {counts:?}"
    );
    assert_eq!(
        counts.encodes, 0,
        "CPU fixture has no device; encodes stay 0 (counted in HudGpu::encode_pass)"
    );
}

#[test]
fn open_hud_wires_work_counters() {
    let _guard = work_counters::test_lock();
    work_counters::reset();
    let mut ui = HudUi::from_measure_fixture(HudMeasureView::Stats);
    for _ in 0..3 {
        ui.run_measure_frame();
    }
    assert_open_counters_wired(work_counters::snapshot());
}

#[test]
fn input_build_counter_tracks_frame_hud_input() {
    let _guard = work_counters::test_lock();
    work_counters::reset();
    let mut acc = WindowInputAccumulator::default();
    let _input = frame_hud_input(true, 1.0, &mut acc);
    assert_eq!(work_counters::snapshot().input_builds, 1);
}
