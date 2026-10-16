//! Dev-only drone control API.
//!
//! Compiled only with the `control-api` feature. The TCP server binds `127.0.0.1`
//! when `--control-port` is set. The Resonite IPC crates are not referenced here.

#![forbid(unsafe_code)]

mod camera;
mod cli;
mod input;
mod present;
mod probe;
mod server;
mod session;

#[cfg(test)]
mod tests;

pub(crate) use cli::parse_control_port;
pub(crate) use present::{finish_presented_frame, headless_tick_presents, meta_from_gpu};
pub(crate) use probe::{
    add_cull, add_draw_prep, add_draws, add_submit, add_upload, begin_frame, is_active,
};
pub(crate) use server::ControlServer;
pub(crate) use session::ControlSession;
