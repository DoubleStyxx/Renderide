//! Side-effect logging helpers for the decoupling state machine.
//!
//! These wrap the existing `logger::info!` / `logger::trace!` calls so the
//! [`crate::frontend::RendererFrontend`] facade does not have to inline a
//! verbose `match` over every [`DecouplingSubmitDecision`] arm. The exact
//! message texts and field formatting are preserved as part of the observable
//! contract (operators grep these lines in `logs/renderer/*.log`).

use super::decisions::DecouplingSubmitDecision;
use super::state::DecouplingState;

/// Emits the activation log line when a tick promotes the renderer to decoupled mode.
pub(crate) fn log_activation(state: &DecouplingState, last_frame_index: i32) {
    logger::info!(
        "render decoupling activated: last_frame_index={} threshold_s={:.4} asset_budget_ms={}",
        last_frame_index,
        state.activate_interval_seconds(),
        state.effective_asset_integration_budget_ms(1),
    );
}

/// Emits the appropriate log line for the recouple decision triggered by a received frame submit.
pub(crate) fn log_submit_decision(
    decision: DecouplingSubmitDecision,
    state: &DecouplingState,
    frame_index: i32,
) {
    match decision {
        DecouplingSubmitDecision::Recouple => {
            logger::info!(
                "render decoupling recoupled: frame_index={} stable_frame_count={} last_submit_ms={:.3}",
                frame_index,
                state.recouple_frame_count(),
                state
                    .last_frame_begin_to_submit()
                    .map_or(-1.0, |duration| duration.as_secs_f64() * 1000.0),
            );
        }
        DecouplingSubmitDecision::ResetProgress => {
            logger::trace!(
                "render decoupling recouple progress reset: frame_index={} last_submit_ms={:.3} threshold_s={:.4}",
                frame_index,
                state
                    .last_frame_begin_to_submit()
                    .map_or(-1.0, |duration| duration.as_secs_f64() * 1000.0),
                state.activate_interval_seconds(),
            );
        }
        DecouplingSubmitDecision::AdvanceProgress(progress) => {
            logger::trace!(
                "render decoupling recouple progress: frame_index={} progress={}/{} last_submit_ms={:.3}",
                frame_index,
                progress,
                state.recouple_frame_count(),
                state
                    .last_frame_begin_to_submit()
                    .map_or(-1.0, |duration| duration.as_secs_f64() * 1000.0),
            );
        }
        DecouplingSubmitDecision::Hold => {}
    }
}
