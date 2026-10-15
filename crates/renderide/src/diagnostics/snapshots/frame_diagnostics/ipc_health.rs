//! IPC outbound health and command-dispatch counters fragment of
//! [`super::FrameDiagnosticsSnapshot`].

/// Per-tick outbound IPC queue health for the Frame diagnostics HUD.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameDiagnosticsIpcQueues {
    /// At least one **primary** outbound IPC send failed this tick (queue full).
    pub ipc_primary_outbound_drop_this_tick: bool,
    /// At least one **background** outbound IPC send failed this tick (queue full).
    pub ipc_background_outbound_drop_this_tick: bool,
    /// Consecutive primary-queue enqueue failures (0 after a successful send).
    pub ipc_primary_consecutive_fail_streak: u32,
    /// Consecutive background-queue enqueue failures (0 after a successful send).
    pub ipc_background_consecutive_fail_streak: u32,
}

/// IPC and host-command health counters captured for the **Stats** tab.
#[derive(Clone, Copy, Debug, Default)]
pub struct IpcHealthFragment {
    /// Outbound primary/background queue drops and streaks for this tick.
    pub queues: FrameDiagnosticsIpcQueues,
    /// Cumulative failed scene applies after host [`crate::shared::FrameSubmitData`] (see
    /// [`crate::runtime::RendererRuntime`]).
    pub frame_submit_apply_failures: u64,
    /// Sum of post-init unhandled [`crate::shared::RendererCommand`] observations (running
    /// dispatch).
    pub unhandled_ipc_command_event_total: u64,
}

impl IpcHealthFragment {
    /// Builds the fragment from the per-tick capture inputs.
    pub fn capture(
        queues: FrameDiagnosticsIpcQueues,
        frame_submit_apply_failures: u64,
        unhandled_ipc_command_event_total: u64,
    ) -> Self {
        Self {
            queues,
            frame_submit_apply_failures,
            unhandled_ipc_command_event_total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_preserves_queue_and_counter_inputs() {
        let queues = FrameDiagnosticsIpcQueues {
            ipc_primary_outbound_drop_this_tick: true,
            ipc_background_outbound_drop_this_tick: false,
            ipc_primary_consecutive_fail_streak: 3,
            ipc_background_consecutive_fail_streak: 5,
        };

        let fragment = IpcHealthFragment::capture(queues, 7, 11);

        assert!(fragment.queues.ipc_primary_outbound_drop_this_tick);
        assert!(!fragment.queues.ipc_background_outbound_drop_this_tick);
        assert_eq!(fragment.queues.ipc_primary_consecutive_fail_streak, 3);
        assert_eq!(fragment.queues.ipc_background_consecutive_fail_streak, 5);
        assert_eq!(fragment.frame_submit_apply_failures, 7);
        assert_eq!(fragment.unhandled_ipc_command_event_total, 11);
    }
}
