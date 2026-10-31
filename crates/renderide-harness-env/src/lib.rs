//! Renderer-child environment shared by the golden harness and frame bench.
//!
//! [`resolve`] picks a Vulkan ICD, a graphics backend, and a Rayon thread count.
//! Callers apply [`ResolvedEnv::child_env`] with [`std::process::Command::env`] on
//! the renderer child. This crate never calls [`std::env::set_var`] or
//! [`std::env::remove_var`].
//!
//! Linux `--icd auto` uses `VK_DRIVER_FILES` or `VK_ICD_FILENAMES` when one is
//! set, and otherwise probes lavapipe. An explicit variable always wins and is
//! reported as `(env)`. Windows defaults to DX12 and macOS to Metal. `dx12-warp`
//! is not a backend. An unset `RAYON_NUM_THREADS` becomes `1` on the child.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod env;

pub use env::{
    BackendMode, EnvError, IcdMode, InheritedIcd, LAVAPIPE_FILENAMES, RAYON_NUM_THREADS,
    ResolveInput, ResolvedEnv, VK_DRIVER_FILES, VK_ICD_FILENAMES, WGPU_BACKEND, default_icd_roots,
    inherited_icd, parse_backend_mode, parse_icd_mode, probe_lavapipe, resolve,
};
