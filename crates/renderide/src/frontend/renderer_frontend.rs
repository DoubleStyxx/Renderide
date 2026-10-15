//! [`RendererFrontend`] facade: composes the small transport, session,
//! lock-step, performance, output-policy, and decoupling components into a
//! single side-effect adapter for queue and shared-memory access.
//!
//! The facade itself owns no GPU pools or scene graph. Domain-specific method
//! groups live in the submodules below; each provides one
//! `impl RendererFrontend` block so the facade's responsibilities are visible
//! at a glance.

mod decoupling;
mod init;
mod lockstep;
mod output;
mod performance;
mod transport;

use crate::connection::ConnectionParams;

use super::decoupling::DecouplingState;
use super::frame_start_performance::FrameStartPerformanceState;
use super::lockstep_state::LockstepState;
use super::output_policy::HostOutputPolicy;
use super::session::FrontendSession;
use super::transport::FrontendTransport;

/// IPC, shared memory, init sequence, lock-step, and host output state.
///
/// The facade owns no GPU pools or scene graph. Its fields are split by domain so pure transition
/// logic (init routing, begin-frame gating, decoupling, performance, output policy) stays separate
/// from side-effect adapters such as queue sends and shared-memory access.
pub struct RendererFrontend {
    transport: FrontendTransport,
    session: FrontendSession,
    lockstep: LockstepState,
    performance: FrameStartPerformanceState,
    output_policy: HostOutputPolicy,
    decoupling: DecouplingState,
}

impl RendererFrontend {
    /// Builds frontend state; does not open IPC yet (see [`Self::connect_ipc`]).
    pub fn new(params: Option<ConnectionParams>) -> Self {
        let standalone = params.is_none();
        Self {
            transport: FrontendTransport::new(params),
            session: FrontendSession::new(standalone),
            lockstep: LockstepState::new(standalone),
            performance: FrameStartPerformanceState::default(),
            output_policy: HostOutputPolicy::default(),
            decoupling: DecouplingState::default(),
        }
    }
}
