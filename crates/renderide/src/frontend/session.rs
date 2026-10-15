//! Init lifecycle and host-requested exit/error flags.

use crate::shared::RendererInitData;

use super::init_state::InitState;

/// Host session state that is independent from queue ownership and frame cadence.
pub(crate) struct FrontendSession {
    init_state: InitState,
    pending_init: Option<RendererInitData>,
    shutdown_requested: bool,
    fatal_error: bool,
}

impl FrontendSession {
    /// Builds session state for either standalone or host-connected mode.
    pub(crate) fn new(standalone: bool) -> Self {
        let init_state = if standalone {
            InitState::Finalized
        } else {
            InitState::default()
        };
        Self {
            init_state,
            pending_init: None,
            shutdown_requested: false,
            fatal_error: false,
        }
    }

    /// Current host/renderer init handshake phase.
    pub(crate) fn init_state(&self) -> InitState {
        self.init_state
    }

    /// Updates the init handshake phase.
    pub(crate) fn set_init_state(&mut self, state: InitState) {
        self.init_state = state;
    }

    /// Marks init data as received and moves into the init-finalize phase.
    pub(crate) fn mark_init_received(&mut self) {
        self.init_state = InitState::InitReceived;
    }

    /// Host init data waiting to be consumed after the renderer stack is ready.
    pub(crate) fn pending_init(&self) -> Option<&RendererInitData> {
        self.pending_init.as_ref()
    }

    /// Stores host init data until consumers are ready.
    pub(crate) fn set_pending_init(&mut self, data: RendererInitData) {
        self.pending_init = Some(data);
    }

    /// Takes pending init data once the consumer has applied it.
    pub(crate) fn take_pending_init(&mut self) -> Option<RendererInitData> {
        self.pending_init.take()
    }

    /// Host requested an orderly renderer exit.
    pub(crate) fn shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    /// Sets the orderly shutdown flag.
    pub(crate) fn set_shutdown_requested(&mut self, value: bool) {
        self.shutdown_requested = value;
    }

    /// Fatal init/IPC state that suppresses lock-step sends.
    pub(crate) fn fatal_error(&self) -> bool {
        self.fatal_error
    }

    /// Sets the fatal init/IPC flag.
    pub(crate) fn set_fatal_error(&mut self, value: bool) {
        self.fatal_error = value;
    }
}
