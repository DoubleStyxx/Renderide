//! Pure decision logic for the renderer-side decoupling state machine.
//!
//! These functions are stateless and take all inputs as parameters so they can
//! be unit-tested without wiring the [`super::state::DecouplingState`] holder
//! or the runtime. See [`super`] for the broader contract and the
//! host-compatible behavior they encode.

use std::time::{Duration, Instant};

/// Pure activation decision for one renderer tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecouplingActivationDecision {
    /// No state mutation is needed.
    Hold,
    /// The renderer should switch to decoupled mode.
    Activate,
}

/// Pure recoupling decision for one received frame submit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecouplingSubmitDecision {
    /// No state mutation is needed.
    Hold,
    /// The submit was still slow; reset recouple progress.
    ResetProgress,
    /// The submit was fast but more stable frames are required.
    AdvanceProgress(i32),
    /// Enough fast submits arrived; clear decoupled mode.
    Recouple,
}

/// Computes whether a pending begin-frame wait should activate decoupled rendering.
pub(crate) fn activation_decision(
    active: bool,
    awaiting_submit: bool,
    last_frame_start_sent_at: Option<Instant>,
    now: Instant,
    activate_interval_seconds: f32,
) -> DecouplingActivationDecision {
    if !awaiting_submit || active {
        return DecouplingActivationDecision::Hold;
    }
    let Some(sent_at) = last_frame_start_sent_at else {
        return DecouplingActivationDecision::Hold;
    };
    let elapsed = now.saturating_duration_since(sent_at);
    if elapsed.as_secs_f32() >= activate_interval_seconds {
        DecouplingActivationDecision::Activate
    } else {
        DecouplingActivationDecision::Hold
    }
}

/// Computes decoupled recouple progress from a completed frame-start to frame-submit interval.
pub(crate) fn submit_decision(
    active: bool,
    elapsed: Option<Duration>,
    activate_interval_seconds: f32,
    recouple_progress: i32,
    recouple_frame_count: i32,
) -> DecouplingSubmitDecision {
    if !active {
        return DecouplingSubmitDecision::Hold;
    }
    let Some(elapsed) = elapsed else {
        return DecouplingSubmitDecision::Hold;
    };
    if elapsed.as_secs_f32() >= activate_interval_seconds {
        return DecouplingSubmitDecision::ResetProgress;
    }
    let next_progress = recouple_progress.saturating_add(1);
    if next_progress >= recouple_frame_count {
        DecouplingSubmitDecision::Recouple
    } else {
        DecouplingSubmitDecision::AdvanceProgress(next_progress)
    }
}
