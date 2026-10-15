//! Preserved (pre-redirect) stdio handles, the `RENDERIDE_LOG_TEE_TERMINAL` flag, and tee
//! helpers used by panic reporting and the fatal-crash handler to write to the launching
//! terminal after stdout/stderr have been redirected into the logger.

use std::io::Write;
use std::sync::OnceLock;

use parking_lot::Mutex;

/// Original stderr handle, captured before redirection.
#[cfg(any(unix, windows))]
static PRESERVED_STDERR: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

/// Original stdout handle, captured before redirection.
#[cfg(any(unix, windows))]
static PRESERVED_STDOUT: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

/// Which standard stream was redirected; used to tee to the matching preserved handle.
#[derive(Clone, Copy, Debug)]
pub(super) enum StdioStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// When `false`, forwarded native lines and panic terminal output are not copied to the
/// original console (log file only). Default: enabled unless `RENDERIDE_LOG_TEE_TERMINAL` is
/// `0`, `false`, `no`, or `off` (case-insensitive).
pub(super) fn tee_terminal_enabled() -> bool {
    match std::env::var("RENDERIDE_LOG_TEE_TERMINAL") {
        Ok(v) => {
            let v = v.trim();
            !matches!(
                v.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        }
        Err(_) => true,
    }
}

/// Writes `data` to the process's **original** stderr (before
/// [`super::ensure_stdio_forwarded_to_logger`]), for panic reporting. No-op if redirect did
/// not run, tee is disabled, or the platform is unsupported.
pub(crate) fn try_write_preserved_stderr(data: &[u8]) {
    if !tee_terminal_enabled() {
        return;
    }
    #[cfg(any(unix, windows))]
    if let Some(m) = PRESERVED_STDERR.get() {
        let mut f = m.lock();
        let _ = f.write_all(data);
        let _ = f.flush();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = data;
    }
}

/// Mirror of [`try_write_preserved_stderr`] for the stdout side. Used by the pipe forwarder
/// to tee stdout lines to the launching terminal.
#[cfg(any(unix, windows))]
pub(super) fn try_write_preserved_stdout(data: &[u8]) {
    if !tee_terminal_enabled() {
        return;
    }
    if let Some(m) = PRESERVED_STDOUT.get() {
        let mut f = m.lock();
        let _ = f.write_all(data);
        let _ = f.flush();
    }
}

/// Duplicates the **preserved** stderr stream (the launching terminal) for async-signal-safe
/// `write` from a fatal crash handler. Call only after
/// [`super::ensure_stdio_forwarded_to_logger`].
///
/// Returns [`None`] when tee is disabled, stderr was not redirected, or duplication fails.
#[cfg(unix)]
pub(crate) fn duplicate_preserved_stderr_raw_fd() -> Option<std::os::fd::OwnedFd> {
    if !tee_terminal_enabled() {
        return None;
    }
    let m = PRESERVED_STDERR.get()?;
    let guard = m.lock();
    use std::os::fd::AsFd;
    guard.as_fd().try_clone_to_owned().ok()
}

/// See [`duplicate_preserved_stderr_raw_fd`]. Windows uses a duplicated [`std::fs::File`].
#[cfg(windows)]
pub(crate) fn duplicate_preserved_stderr_file_for_crash_log() -> Option<std::fs::File> {
    if !tee_terminal_enabled() {
        return None;
    }
    let m = PRESERVED_STDERR.get()?;
    let guard = m.lock();
    guard.try_clone().ok()
}

/// Records the original Unix fd recovered by `dup(target_fd)` so later writes can tee back to
/// the launching terminal.
#[cfg(unix)]
pub(super) fn store_preserved_unix(stream: StdioStream, saved: i32) {
    use std::fs::File;
    use std::os::fd::FromRawFd;
    use std::os::fd::OwnedFd;

    // SAFETY: `saved` was just produced by `libc::dup`, is open, owned by this process, and has
    // not been handed to another `OwnedFd`/`File`. Transferring ownership to `OwnedFd` is sound.
    let owned = unsafe { OwnedFd::from_raw_fd(saved) };
    let file = File::from(owned);
    let cell = match stream {
        StdioStream::Stderr => &PRESERVED_STDERR,
        StdioStream::Stdout => &PRESERVED_STDOUT,
    };
    let _ = cell.set(Mutex::new(file));
}

/// Records the original Windows console handle (wrapped as [`std::fs::File`]) so later writes
/// can tee back to the launching terminal.
#[cfg(windows)]
pub(super) fn store_preserved_windows(stream: StdioStream, file: std::fs::File) {
    let cell = match stream {
        StdioStream::Stderr => &PRESERVED_STDERR,
        StdioStream::Stdout => &PRESERVED_STDOUT,
    };
    let _ = cell.set(Mutex::new(file));
}

#[cfg(test)]
mod tests {
    use super::tee_terminal_enabled;

    /// Environment variable consulted by [`tee_terminal_enabled`].
    const VAR: &str = "RENDERIDE_LOG_TEE_TERMINAL";

    /// RAII guard that restores the original value of [`VAR`] (or unsets it) on drop so tests do
    /// not leak process-global state.
    struct EnvGuard(Option<String>);

    impl EnvGuard {
        /// Captures the current value for later restoration.
        fn capture() -> Self {
            Self(std::env::var(VAR).ok())
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: env mutation in test; this Drop runs at end of the single env-mutating test.
            unsafe {
                match &self.0 {
                    Some(v) => std::env::set_var(VAR, v),
                    None => std::env::remove_var(VAR),
                }
            }
        }
    }

    /// All env-var parsing cases are exercised in a single serialized test because
    /// [`std::env::set_var`] mutates process-global state.
    #[test]
    fn tee_terminal_enabled_parses_env_var() {
        let _guard = EnvGuard::capture();

        // SAFETY: env mutation in test; this is the only test in the module that mutates env.
        unsafe {
            std::env::remove_var(VAR);
        }
        assert!(tee_terminal_enabled(), "unset should default to enabled");

        for disabled in ["0", "false", "no", "off", "FALSE", "  No  "] {
            // SAFETY: env mutation in test; this is the only test in the module that mutates env.
            unsafe {
                std::env::set_var(VAR, disabled);
            }
            assert!(
                !tee_terminal_enabled(),
                "value {disabled:?} should disable tee"
            );
        }

        for enabled in ["1", "true", "yes", "", "anything else"] {
            // SAFETY: env mutation in test; this is the only test in the module that mutates env.
            unsafe {
                std::env::set_var(VAR, enabled);
            }
            assert!(
                tee_terminal_enabled(),
                "value {enabled:?} should keep tee enabled"
            );
        }
    }
}
