//! Fatal process faults (POSIX signals, Windows structured exceptions, macOS Mach exceptions) do
//! not invoke Rust's panic hook. This module registers [`crash_handler::CrashHandler`] so a short
//! line is appended to the **same** log file as [`logger::init_for`], using only pre-opened fds and
//! stack buffers in [`crash_handler::CrashEvent::on_crash`]. On Unix, writes use [`libc::write`]
//! only (async-signal-safe). After [`crate::native_stdio::ensure_stdio_forwarded_to_logger`], fd 2
//! is a pipe; a **duplicate** of the preserved terminal stderr is used for console output when tee
//! is enabled.
//!
//! **Linux `write(2)`:** A failed `write` may set `errno` to **`EINTR`**; the handler must **retry**
//! the same buffer without advancing (POSIX async-signal-safe pattern). Otherwise the first fd
//! (log file) can fail while the second (terminal duplicate) still succeeds.
//!
//! If the dedicated append **log fd** still has **unwritten bytes** after retries, the remainder is
//! written to **fd 2** (the stderr **pipe** to the logger forwarder), so the line can still appear
//! in the log file without using [`logger::log`] (mutex).
//!
//! **macOS:** `crash-handler` uses Mach exception ports, which can interact with other signal
//! machinery (see upstream docs). **Manual testing:** `kill -BUS <pid>` on Linux; Windows fault
//! injection is environment-specific.
//!
//! **Stack traces (Linux, Android, and Windows):** the handler writes hex instruction
//! pointers and the executable's address span. That write is allocation-free and
//! lock-free: no [`backtrace::resolve`], no mutex, and [`std::panic::catch_unwind`] is
//! not used as a safety fence. Symbol names are resolved on the next start, on a
//! normal thread, from those saved addresses plus the span (the `symbolicate` module).
//! Frames outside the executable are left unresolved so a libc address is not
//! reported as a function in this binary. See the `symbolicate` module. macOS keeps the signal-info line alone:
//! the Mach exception callback runs on a dedicated thread, so a plain `trace` walks
//! the wrong stack.
//!
//! **Linux alt signal stack:** libstd's per-thread altstack (~8 KB) is too small for
//! the unwinder when the fault is a stack overflow. The Unix install path installs a
//! 512 KB altstack on the main thread before [`crash_handler::CrashHandler::attach`]
//! so the instruction-pointer list can still be written. Worker threads keep libstd's
//! smaller altstack and may lose that list on a stack overflow.

use std::path::Path;

mod format;
#[cfg(any(target_os = "linux", target_os = "android", windows))]
mod module_span;
#[cfg(any(target_os = "linux", target_os = "android", windows))]
mod stack_trace;
#[cfg(any(target_os = "linux", target_os = "android", windows))]
mod symbolicate;
#[cfg(any(unix, windows))]
mod trip;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// Prepares the final log-directory footer for crash-handler output.
///
/// The native crash callback may run in a signal or structured-exception context, so the footer is
/// allocated before the handler is attached and then reused as immutable bytes.
#[cfg(any(unix, windows))]
pub(super) fn log_directory_footer_bytes() -> Box<[u8]> {
    logger::log_directory_footer(logger::logs_root())
        .into_bytes()
        .into_boxed_slice()
}

/// Installs the crash handler after logging and stdio forwarding are initialized.
///
/// Failures are logged and ignored so startup continues without fatal-crash logging.
pub(crate) fn install(log_path: &Path) {
    if let Err(e) = try_install(log_path) {
        logger::warn!("Failed to install fatal crash log handler: {e}");
    }
}

/// Publishes the executable span, symbolicates a previous raw trace, then attaches the handler.
#[cfg(any(unix, windows))]
pub(super) fn try_install(log_path: &Path) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    {
        module_span::publish_module_span();
        if let Err(e) = symbolicate::append_deferred_symbolication(log_path) {
            logger::warn!("failed to symbolicate previous fatal crash log: {e}");
        }
    }
    #[cfg(unix)]
    {
        unix::install_impl(log_path)
    }
    #[cfg(windows)]
    {
        windows::install_impl(log_path)
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn try_install(log_path: &Path) -> Result<(), String> {
    let _ = log_path;
    Ok(())
}

/// Hidden entry points for the fatal-crash handler tests.
///
/// Not part of the renderer process API.
#[cfg(any(unix, windows))]
#[doc(hidden)]
pub mod test_support {
    use std::path::Path;

    /// Installs the process fatal-crash handler, writing records to `log_path`.
    pub fn install_handler(log_path: &Path) -> Result<(), String> {
        super::try_install(log_path)
    }

    /// Clears the one-shot handler reentry flag so a test can enter the writer again.
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    pub fn reset_handler_reentry_for_test() {
        super::stack_trace::reset_reentry_for_test();
    }

    /// Publishes this process's executable address span before a measured handler write.
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    pub fn publish_module_span_for_test() {
        super::module_span::publish_module_span();
    }

    /// Runs the signal-handler stack writer. Allocation-free after the span is published.
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    pub fn write_handler_stack(write_all: impl FnMut(&[u8])) {
        super::stack_trace::write_signal_safe_stack(write_all);
    }

    /// Symbolicates a handler record on a normal thread.
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    pub fn symbolicate_saved_record(record: &str) -> String {
        super::symbolicate::symbolicate_saved_record(record)
    }

    /// Appends deferred symbols for the last raw stack in `log_path`.
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    pub fn symbolicate_log(log_path: &Path) -> Result<(), String> {
        super::symbolicate::append_deferred_symbolication(log_path)
    }

    /// Raises a fatal fault in this process. Does not return.
    pub fn trip_fatal_fault() -> ! {
        super::trip::trip_fatal_fault()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(any(unix, windows))]
    fn log_directory_footer_bytes_are_single_line() {
        let footer = super::log_directory_footer_bytes();
        let text = std::str::from_utf8(&footer).expect("utf8 footer");

        assert!(text.starts_with(logger::LOG_DIRECTORY_FOOTER_PREFIX));
        assert!(text.ends_with('\n'));
        assert_eq!(text.lines().count(), 1);
    }
}
