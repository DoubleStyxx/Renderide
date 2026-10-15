//! Redirect native **stdout** and **stderr** into the Renderide file logger on Unix and Windows.
//!
//! Vulkan validation layers and **spirv-val** often emit via **`printf`** (stdout) and/or
//! **`fprintf(stderr, ...)`**. WGPU's instance flags do not control whether users enable layers via
//! `VK_INSTANCE_LAYERS`, so the renderer installs forwarding **unconditionally** after file logging
//! starts (see [`crate::app::run`]).
//!
//! OpenXR runtimes use the same native paths; [`crate::xr::bootstrap::init_wgpu_openxr`] also calls
//! [`ensure_stdio_forwarded_to_logger`] for entry points that skip `run` (idempotent via [`Once`]).
//!
//! - **Unix:** `pipe` + `dup2` per stream (`STDOUT_FILENO` / `STDERR_FILENO`).
//! - **Windows:** `CreatePipe` + `SetStdHandle(STD_OUTPUT_HANDLE / STD_ERROR_HANDLE, ...)`.
//!
//! The readers use [`logger::try_log`] (non-blocking lock + append fallback) so they cannot deadlock
//! with the main thread on the global logger mutex, and read in chunks so a missing newline cannot
//! fill the pipe and block writers.
//!
//! **Terminal tee:** Before redirecting, the original console file descriptors / handles are kept.
//! Each forwarded line is also written to that original stream so Vulkan validation and similar
//! output appears on the launching terminal as well as in the log file. Disable with
//! **`RENDERIDE_LOG_TEE_TERMINAL=0`** (or `false` / `no`) for CI or headless runs.
//!
//! Renderer `logger::error!` lines are also mirrored to the preserved original stderr handle after
//! redirection. This makes Rust-routed errors, including wgpu uncaptured validation callbacks and
//! watchdog hang reports, visible in both `logs/renderer/*.log` and the launching terminal.
//!
//! On other targets this module is a no-op.
//!
//! Avoid enabling the logger's **mirror-to-stderr** option together with this redirect: mirrored
//! lines would be written back into the pipe and re-logged. Tee uses the **preserved** handles, not
//! [`std::io::stderr`].

use std::sync::Once;

#[cfg(any(unix, windows))]
use logger::LogLevel;

mod forward;
mod preserved;
#[cfg(unix)]
mod redirect_unix;
#[cfg(windows)]
mod redirect_windows;

#[cfg(windows)]
pub(crate) use preserved::duplicate_preserved_stderr_file_for_crash_log;
#[cfg(unix)]
pub(crate) use preserved::duplicate_preserved_stderr_raw_fd;
pub(crate) use preserved::try_write_preserved_stderr;

static INSTALL: Once = Once::new();

/// Ensures process **stdout** and **stderr** are forwarded to [`logger`] and no longer write to the
/// original terminal streams. Idempotent.
pub(crate) fn ensure_stdio_forwarded_to_logger() {
    INSTALL.call_once(|| {
        #[cfg(unix)]
        {
            if let Err(e) = redirect_unix::try_redirect_unix_stream(
                libc::STDERR_FILENO,
                "renderide-stderr",
                preserved::StdioStream::Stderr,
            ) {
                logger::warn!("Native stderr could not be redirected to log file: {e}");
            }
            if let Err(e) = redirect_unix::try_redirect_unix_stream(
                libc::STDOUT_FILENO,
                "renderide-stdout",
                preserved::StdioStream::Stdout,
            ) {
                logger::warn!("Native stdout could not be redirected to log file: {e}");
            }
        }
        #[cfg(windows)]
        {
            if let Err(e) = redirect_windows::try_redirect_windows_stream(
                windows_sys::Win32::System::Console::STD_ERROR_HANDLE,
                "renderide-stderr",
                preserved::StdioStream::Stderr,
            ) {
                logger::warn!("Native stderr could not be redirected to log file: {e}");
            }
            if let Err(e) = redirect_windows::try_redirect_windows_stream(
                windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE,
                "renderide-stdout",
                preserved::StdioStream::Stdout,
            ) {
                logger::warn!("Native stdout could not be redirected to log file: {e}");
            }
        }
        #[cfg(any(unix, windows))]
        logger::set_mirror_writer(LogLevel::Error, try_write_preserved_stderr);
    });
}
