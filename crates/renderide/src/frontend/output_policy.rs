//! Host output policy state derived from frame submits.

use crate::shared::OutputState;

/// Pure result of applying a frame-submit output payload to frontend output policy.
#[derive(Clone, Debug, Default)]
pub(crate) struct OutputPolicyTransition {
    /// Cursor-lock bit to expose to input packing after the transition.
    pub(crate) cursor_lock_requested: bool,
    /// One-shot output state for the app/window layer.
    pub(crate) pending_output_state: Option<OutputState>,
    /// Last non-null output state retained for per-frame cursor policy.
    pub(crate) last_output_state: Option<OutputState>,
}

/// Computes the next output policy from a previous state and an optional host payload.
pub(crate) fn next_output_policy(
    previous_cursor_lock_requested: bool,
    previous_last_output_state: Option<&OutputState>,
    output: Option<OutputState>,
) -> OutputPolicyTransition {
    match output {
        Some(state) => OutputPolicyTransition {
            cursor_lock_requested: state.lock_cursor,
            pending_output_state: Some(state.clone()),
            last_output_state: Some(state),
        },
        None => OutputPolicyTransition {
            cursor_lock_requested: previous_cursor_lock_requested,
            pending_output_state: None,
            last_output_state: previous_last_output_state.cloned(),
        },
    }
}

/// Retains host cursor/window policy between frame submits and app-frame application.
#[derive(Default)]
pub(crate) struct HostOutputPolicy {
    cursor_lock_requested: bool,
    pending_output_state: Option<OutputState>,
    last_output_state: Option<OutputState>,
}

impl HostOutputPolicy {
    /// Host wants relative mouse mode; merged into packed input.
    pub(crate) fn cursor_lock_requested(&self) -> bool {
        self.cursor_lock_requested
    }

    /// Applies optional output state from a frame submit.
    pub(crate) fn apply_frame_submit_output(&mut self, output: Option<OutputState>) {
        let next = next_output_policy(
            self.cursor_lock_requested,
            self.last_output_state.as_ref(),
            output,
        );
        self.cursor_lock_requested = next.cursor_lock_requested;
        self.pending_output_state = next.pending_output_state;
        self.last_output_state = next.last_output_state;
    }

    /// Last non-null host output state.
    pub(crate) fn last_output_state(&self) -> Option<&OutputState> {
        self.last_output_state.as_ref()
    }

    /// Takes the one-shot output state for app/window application.
    pub(crate) fn take_pending_output_state(&mut self) -> Option<OutputState> {
        self.pending_output_state.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_preserves_cursor_lock_and_last_state_but_clears_pending() {
        let previous = OutputState {
            lock_cursor: true,
            ..Default::default()
        };
        let next = next_output_policy(true, Some(&previous), None);
        assert!(next.cursor_lock_requested);
        assert!(next.pending_output_state.is_none());
        assert_eq!(
            next.last_output_state
                .as_ref()
                .map(|state| state.lock_cursor),
            Some(true)
        );
    }

    #[test]
    fn some_updates_cursor_lock_pending_and_last_state() {
        let next = next_output_policy(
            true,
            None,
            Some(OutputState {
                lock_cursor: false,
                ..Default::default()
            }),
        );
        assert!(!next.cursor_lock_requested);
        assert!(next.pending_output_state.is_some());
        assert_eq!(
            next.last_output_state
                .as_ref()
                .map(|state| state.lock_cursor),
            Some(false)
        );
    }
}
