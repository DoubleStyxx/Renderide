//! Exercises the build-time shader composition helpers under `cargo test` by path-including the
//! build-script module.

#![allow(
    dead_code,
    reason = "the path-included build-script module exposes many helpers outside the focused unit tests"
)]
#![allow(
    clippy::print_stdout,
    reason = "the included build-script module emits Cargo directives through println!"
)]

#[path = "../build_support/shader.rs"]
mod shader;
