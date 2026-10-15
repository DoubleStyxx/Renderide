//! IPC command dispatch: decodes incoming `RendererCommand`s into runtime-applied effects.
//!
//! Pulls IPC fan-out out of `crate::runtime/` so transport-shaped routing lives next to the queue
//! drain in `crate::frontend`. Dispatch code classifies command lifecycle and payload domains, then
//! `runtime::RendererRuntime::poll_ipc` applies the decoded effects to frontend, scene, backend,
//! settings, and IPC state.

pub(crate) mod command_dispatch;
pub(crate) mod command_kind;
pub(crate) mod commands;
pub(crate) mod ipc_init;
pub(crate) mod renderer_command_kind;
