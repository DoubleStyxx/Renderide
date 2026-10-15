//! Host-output-policy methods on [`RendererFrontend`].

use crate::shared::OutputState;

use super::RendererFrontend;

impl RendererFrontend {
    /// Host wants relative mouse mode; merged into [`crate::shared::MouseState::is_active`].
    pub fn host_cursor_lock_requested(&self) -> bool {
        self.output_policy.cursor_lock_requested()
    }

    /// Updates cursor/window policy from a frame submit.
    pub fn apply_frame_submit_output(&mut self, output: Option<OutputState>) {
        self.output_policy.apply_frame_submit_output(output);
    }

    /// Last [`OutputState`] from a frame submit.
    pub fn last_output_state(&self) -> Option<&OutputState> {
        self.output_policy.last_output_state()
    }

    /// Takes the last one-shot [`OutputState`] so the winit layer can apply it once.
    pub fn take_pending_output_state(&mut self) -> Option<OutputState> {
        self.output_policy.take_pending_output_state()
    }
}
