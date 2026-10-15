//! Host/renderer init handshake phase ([`InitState`]).

/// Host init sequence state (replaces paired booleans such as `init_received` / `init_finalized`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InitState {
    /// Waiting for [`crate::shared::RendererCommand::RendererInitData`].
    #[default]
    Uninitialized,
    /// `renderer_init_data` received; waiting for [`crate::shared::RendererCommand::RendererInitFinalizeData`].
    InitReceived,
    /// Normal operation (or standalone mode).
    Finalized,
}

impl InitState {
    /// Whether host init handshake is complete.
    pub fn is_finalized(self) -> bool {
        matches!(self, InitState::Finalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_uninitialized() {
        assert_eq!(InitState::default(), InitState::Uninitialized);
    }

    #[test]
    fn is_finalized_returns_true_only_for_finalized_variant() {
        assert!(!InitState::Uninitialized.is_finalized());
        assert!(!InitState::InitReceived.is_finalized());
        assert!(InitState::Finalized.is_finalized());
    }
}
