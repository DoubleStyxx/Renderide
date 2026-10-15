//! Renderer-side decoupling state machine driven by the host's
//! [`RenderDecouplingConfig`](crate::shared::RenderDecouplingConfig).
//!
//! Encodes the host decoupling behavior:
//! - Activation: when the renderer has been waiting for a [`crate::shared::FrameSubmitData`]
//!   longer than [`DecouplingState::activate_interval_seconds`], it goes "decoupled" so it can
//!   keep drawing the prior scene state instead of stalling.
//! - Re-couple: each arriving [`crate::shared::FrameSubmitData`] reports
//!   `frame_begin_to_submit` (time from sending the matching outgoing
//!   [`crate::shared::FrameStartData`] to receiving the host's submit). Sub-threshold submits
//!   advance a stable-frame counter; once it reaches [`DecouplingState::recouple_frame_count`]
//!   the decoupled flag clears. Submits at-or-above the threshold reset the counter.
//! - Asset integration budget: while decoupled, the per-tick budget is capped at
//!   [`DecouplingState::decoupled_max_asset_processing_seconds`] (the host-supplied ceiling)
//!   so the renderer can keep up with display while the host catches up.
//!
//! Pure state -- no IPC, no winit, no GPU. Activation/recouple are driven by `Instant` inputs
//! supplied by callers so the logic is unit-testable without wiring the runtime.
//!
//! `decouple_activate_interval = 0.0` together with `recouple_frame_count = i32::MAX` is the
//! `ForceDecouple` mode emitted by FrooxEngine: activation triggers on the first tick with a
//! pending begin-frame and recouple progress can never reach the threshold, so the renderer
//! stays decoupled until a non-forced config arrives.
//!
//! Layout:
//! - [`decisions`] -- pure activation/recouple decision functions and enums.
//! - [`state`] -- [`DecouplingState`] holder, host-config application, and the integration tests.
//! - [`logging`] -- thin side-effect helpers consumed by [`crate::frontend::RendererFrontend`]
//!   to keep the verbose decoupling log lines next to the state they describe.

pub(crate) mod decisions;
pub(crate) mod logging;
pub(crate) mod state;

pub use state::DecouplingState;
