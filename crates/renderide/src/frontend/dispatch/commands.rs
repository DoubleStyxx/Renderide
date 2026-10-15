//! Decodes [`RendererCommand`] values after the host init handshake is finalized.

use crate::shared::RendererCommand;

use super::command_dispatch::{self, RunningCommandEffect};

/// Decodes IPC commands in the normal running state ([`crate::frontend::InitState::Finalized`]).
pub(crate) fn handle_running_command(cmd: RendererCommand) -> RunningCommandEffect {
    command_dispatch::dispatch_running_command(cmd)
}
