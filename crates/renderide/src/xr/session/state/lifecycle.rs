//! OpenXR session lifecycle event handling.

use openxr as xr;

use super::XrSessionState;

/// Renderer-local projection of [`xr::SessionState`] used for exhaustive matching on
/// compositor visibility without depending on the raw OpenXR newtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedSessionState {
    /// No `SessionStateChanged` event has been observed yet.
    Unknown,
    /// Runtime-side session exists but no rendering is expected.
    Idle,
    /// Runtime signalled the app should call `xrBeginSession`.
    Ready,
    /// Frame loop is running but the compositor is not displaying the app.
    Synchronized,
    /// Compositor is displaying frames but the app does not have input focus.
    Visible,
    /// Compositor is displaying frames and the app has input focus.
    Focused,
    /// Runtime signalled the app should call `xrEndSession`.
    Stopping,
    /// Runtime plans to lose the session; the app should exit.
    LossPending,
    /// Runtime signalled the app to exit.
    Exiting,
}

/// Projects a raw [`xr::SessionState`] onto [`TrackedSessionState`]; unknown numeric values fall
/// back to [`TrackedSessionState::Unknown`] so future OpenXR additions don't panic.
pub(super) fn tracked_from_xr(state: xr::SessionState) -> TrackedSessionState {
    match state {
        xr::SessionState::IDLE => TrackedSessionState::Idle,
        xr::SessionState::READY => TrackedSessionState::Ready,
        xr::SessionState::SYNCHRONIZED => TrackedSessionState::Synchronized,
        xr::SessionState::VISIBLE => TrackedSessionState::Visible,
        xr::SessionState::FOCUSED => TrackedSessionState::Focused,
        xr::SessionState::STOPPING => TrackedSessionState::Stopping,
        xr::SessionState::LOSS_PENDING => TrackedSessionState::LossPending,
        xr::SessionState::EXITING => TrackedSessionState::Exiting,
        _ => TrackedSessionState::Unknown,
    }
}

/// Whether the compositor is currently displaying the app and will accept projection layers.
pub(super) fn is_visible_tracked(state: TrackedSessionState) -> bool {
    matches!(
        state,
        TrackedSessionState::Visible | TrackedSessionState::Focused
    )
}

/// Outcome of inspecting one `xrPollEvent` result, decoded into an owned enum so the caller can
/// release the event borrow before invoking `&mut self` side-effects.
enum PollEventAction {
    /// Session transitioned to a new [`xr::SessionState`]; apply via
    /// [`XrSessionState::handle_session_state_change`].
    SessionStateChanged(xr::SessionState),
    /// Instance is being destroyed; renderer must exit.
    InstanceLoss,
    /// Controller interaction profile was re-bound; informational only.
    InteractionProfileChanged,
    /// Event variant not reacted to by this renderer.
    Ignore,
}

impl XrSessionState {
    /// Poll events and return `false` if the session should exit.
    ///
    /// Callers may also read [`Self::exit_requested`] directly; both signals are kept in sync so a
    /// dropped return value (as at [`crate::xr::app_integration::openxr_begin_frame_tick`]) no
    /// longer silently strands the app in a terminating session.
    pub fn poll_events(&mut self) -> Result<bool, xr::sys::Result> {
        profiling::scope!("xr::session_poll_events");
        loop {
            // Bind the next event in an inner scope so its borrow on `self.event_storage` ends
            // before we invoke any `&mut self` state-change side-effects below.
            let action = {
                let Some(event) = self.xr_instance.poll_event(&mut self.event_storage)? else {
                    break;
                };
                use xr::Event::{
                    InstanceLossPending, InteractionProfileChanged, SessionStateChanged,
                };
                match event {
                    SessionStateChanged(e) => PollEventAction::SessionStateChanged(e.state()),
                    InstanceLossPending(_) => PollEventAction::InstanceLoss,
                    InteractionProfileChanged(_) => PollEventAction::InteractionProfileChanged,
                    _ => PollEventAction::Ignore,
                }
            };
            match action {
                PollEventAction::SessionStateChanged(state) => {
                    if !self.handle_session_state_change(state)? {
                        return Ok(false);
                    }
                }
                PollEventAction::InstanceLoss => {
                    self.exit_requested = true;
                    return Ok(false);
                }
                PollEventAction::InteractionProfileChanged => {
                    logger::info!("OpenXR interaction profile changed");
                }
                PollEventAction::Ignore => {}
            }
        }
        Ok(!self.exit_requested)
    }

    /// Applies a `SessionStateChanged` event, running any required runtime side-effects
    /// (`xrBeginSession` / `xrEndSession`). Returns `Ok(false)` on terminal transitions so the
    /// caller can break out of its event loop.
    fn handle_session_state_change(
        &mut self,
        new_state: xr::SessionState,
    ) -> Result<bool, xr::sys::Result> {
        let new_tracked = tracked_from_xr(new_state);
        if new_tracked != self.last_session_state {
            logger::info!(
                "OpenXR session state: {:?} -> {:?}",
                self.last_session_state,
                new_tracked
            );
        }
        self.last_session_state = new_tracked;
        match new_state {
            xr::SessionState::READY => {
                self.session
                    .begin(xr::ViewConfigurationType::PRIMARY_STEREO)?;
                self.session_running = true;
                logger::info!("OpenXR session begin succeeded (PRIMARY_STEREO)");
                Ok(true)
            }
            xr::SessionState::STOPPING => {
                self.session.end()?;
                self.session_running = false;
                logger::info!("OpenXR session end succeeded");
                Ok(true)
            }
            xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                self.exit_requested = true;
                Ok(false)
            }
            _ => Ok(true),
        }
    }
}

#[cfg(test)]
mod state_machine_tests {
    //! State-machine logic testable without a live OpenXR runtime. Frame-stream integration
    //! (`wait_frame` / `end_frame_*`) requires a real `xr::FrameWaiter` + `xr::FrameStream` and is
    //! exercised by the VR integration path instead of unit tests.
    use super::*;

    #[test]
    fn tracked_from_xr_covers_every_documented_variant() {
        assert_eq!(
            tracked_from_xr(xr::SessionState::IDLE),
            TrackedSessionState::Idle
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::READY),
            TrackedSessionState::Ready
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::SYNCHRONIZED),
            TrackedSessionState::Synchronized
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::VISIBLE),
            TrackedSessionState::Visible
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::FOCUSED),
            TrackedSessionState::Focused
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::STOPPING),
            TrackedSessionState::Stopping
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::LOSS_PENDING),
            TrackedSessionState::LossPending
        );
        assert_eq!(
            tracked_from_xr(xr::SessionState::EXITING),
            TrackedSessionState::Exiting
        );
    }

    #[test]
    fn is_visible_tracked_only_true_for_visible_and_focused() {
        for (s, expected) in [
            (TrackedSessionState::Unknown, false),
            (TrackedSessionState::Idle, false),
            (TrackedSessionState::Ready, false),
            (TrackedSessionState::Synchronized, false),
            (TrackedSessionState::Visible, true),
            (TrackedSessionState::Focused, true),
            (TrackedSessionState::Stopping, false),
            (TrackedSessionState::LossPending, false),
            (TrackedSessionState::Exiting, false),
        ] {
            assert_eq!(is_visible_tracked(s), expected, "state {s:?}");
        }
    }
}
