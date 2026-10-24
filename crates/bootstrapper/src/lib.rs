//! ResoBoot-compatible bootstrapper: launches Renderite Host, bridges clipboard/renderer over shared-memory queues.
//!
//! Queue backing paths follow [`interprocess::default_memory_dir`] on each OS unless
//! `RENDERIDE_INTERPROCESS_DIR` is set--the Host must use the same directory (same env or defaults).
//!
//! When the Host spawns the renderer via the bootstrap queue, the bootstrapper tracks that
//! [`std::process::Child`]. If the **renderer** exits first (for example the user closes the
//! window), the bootstrapper terminates the **Host** process and exits--mirroring engine-side
//! watchdog behavior so the session does not outlive the GPU process.
//!
//! The binary entry point is [`run`]; use [`BootstrapOptions`] to supply Host arguments and logging.

#![deny(clippy::multiple_unsafe_ops_per_block)]

use std::path::PathBuf;

mod child_lifetime;
mod cleanup;
pub mod cli;
mod config;
mod constants;
mod error;
mod host;
pub mod ipc;
pub mod launch_args;
mod orchestration;
pub mod panic_hook;
mod paths;
pub mod photosensitivity_warning;
mod process_state;
mod protocol;
mod protocol_handlers;
mod renderer_link;
pub mod updater;
mod watchdogs;

/// Process-wide environment lock shared with [`interprocess::test_env`].
///
/// Not part of the supported API. Tests use it so environment mutation and
/// restore reads take one mutex.
#[doc(hidden)]
pub use interprocess::test_env;
pub mod vr_prompt;
mod wine_detect;

pub use error::BootstrapError;

/// Inputs for [`run`]: Host argv, optional verbosity, and log filename timestamp.
///
/// The global logger must already be initialized (via [`logger::init_for`] with
/// [`logger::LogComponent::Bootstrapper`]) before [`run`] is called; see `main.rs`. This
/// ordering guarantees that failures in argv parsing or the desktop/VR dialog reach the
/// bootstrapper log file instead of producing a silent "nothing happens" hang.
#[derive(Debug, Clone)]
pub struct BootstrapOptions {
    /// Arguments forwarded to Renderite Host (before `-Invisible` / `-shmprefix`).
    pub host_args: Vec<String>,
    /// Explicit Resonite installation directory supplied to the launcher.
    pub resonite_dir: Option<PathBuf>,
    /// Maximum level written to the bootstrapper log file; also forwarded to Renderide when set.
    pub log_level: Option<logger::LogLevel>,
    /// Filename segment from [`logger::log_filename_timestamp`] (without `.log`).
    pub log_timestamp: String,
}

/// Runs the bootstrap sequence: builds the `ResoBootConfig`, spawns Host and renderer,
/// and bridges clipboard / renderer IPC.
///
/// The caller (`main.rs`) is responsible for initializing the global logger and installing
/// the panic hook **before** invoking this function, so any earlier failure (argv parsing,
/// the `rfd` desktop/VR dialog) still lands in `logs/bootstrapper/*.log`. Panics inside
/// this function are caught, logged, and swallowed with `Ok(())` to mirror the production
/// `ResoBoot` behavior.
pub fn run(options: BootstrapOptions) -> Result<(), BootstrapError> {
    let shared_memory_prefix =
        config::generate_shared_memory_prefix(16).map_err(BootstrapError::Prefix)?;
    let resonite_config = config::ResoBootConfig::new(
        shared_memory_prefix,
        options.log_level,
        options.resonite_dir,
    )
    .map_err(BootstrapError::CurrentDir)?;

    let ctx = orchestration::RunContext {
        host_args: options.host_args,
        log_timestamp: options.log_timestamp,
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        orchestration::run(&resonite_config, ctx)
    }));

    match result {
        Ok(r) => r,
        Err(e) => {
            logger::error!("Exception in bootstrapper:\n{e:?}");
            logger::flush();
            Ok(())
        }
    }
}

#[cfg(test)]
mod process_env_lock_reexport {
    use std::ffi::OsStr;

    const PROBE: &str = "RENDERIDE_BOOTSTRAPPER_ENV_LOCK_PROBE";

    #[test]
    fn reexport_holds_the_shared_process_env_lock() {
        let guard = crate::test_env::lock();
        crate::test_env::assert_shared_env_lock_held();
        let _snapshot = guard.capture(&[PROBE]);
        guard.set_var(PROBE, "bootstrapper");
        assert_eq!(
            guard.var_os(PROBE).as_deref(),
            Some(OsStr::new("bootstrapper"))
        );
    }
}
