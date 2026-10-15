//! Reader threads that pull bytes from the redirect pipe, split them on newlines, and forward
//! each line to [`logger::try_log`]. Each emitted line is also tee'd to the matching preserved
//! console handle so output appears on the launching terminal as well as in the log file.

#[cfg(any(unix, windows))]
use logger::LogLevel;

#[cfg(any(unix, windows))]
use super::preserved::{StdioStream, try_write_preserved_stdout};
#[cfg(any(unix, windows))]
use crate::native_stdio::try_write_preserved_stderr;

/// Reads from `reader` and emits each `\n`-terminated line as an info-level log entry,
/// flushing any trailing bytes on EOF. Designed for the pipe's read end so a missing newline
/// cannot fill the pipe and block writers.
#[cfg(any(unix, windows))]
fn forward_pipe_lines_to_logger_impl<R: std::io::Read>(mut reader: R, stream: StdioStream) {
    let mut pending = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => {
                if !pending.is_empty() {
                    emit_stdio_line(&pending, LogLevel::Info, stream);
                }
                break;
            }
            Ok(n) => {
                pending.extend_from_slice(&chunk[..n]);
                while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = pending.drain(..pos).collect();
                    if !pending.is_empty() && pending[0] == b'\n' {
                        pending.remove(0);
                    }
                    emit_stdio_line(&line, LogLevel::Info, stream);
                }
            }
            Err(e) => {
                let _ = logger::try_log(
                    LogLevel::Debug,
                    format_args!("stdio forward read ended: {e}"),
                );
                break;
            }
        }
    }
}

/// Thin Unix shim that wraps the raw read-end fd into [`std::fs::File`] and delegates to the
/// generic forwarder.
#[cfg(unix)]
pub(super) fn forward_pipe_lines_to_logger_unix(rfd: i32, stream: StdioStream) {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    // SAFETY: `rfd` is the read end of the pipe created in the Unix redirect path; ownership
    // is transferred exclusively to the spawned thread via this call and has no other owner.
    let f = unsafe { File::from_raw_fd(rfd) };
    forward_pipe_lines_to_logger_impl(f, stream);
}

/// Thin Windows shim that wraps the owned read handle into [`std::fs::File`] and delegates to
/// the generic forwarder, mirroring [`forward_pipe_lines_to_logger_unix`].
#[cfg(windows)]
pub(super) fn forward_pipe_lines_to_logger_windows(
    read_owned: std::os::windows::io::OwnedHandle,
    stream: StdioStream,
) {
    use std::fs::File;
    let f = File::from(read_owned);
    forward_pipe_lines_to_logger_impl(f, stream);
}

/// Trims and logs a single line, then tees a newline-terminated copy to the matching
/// preserved console handle.
#[cfg(any(unix, windows))]
fn emit_stdio_line(line: &[u8], level: LogLevel, stream: StdioStream) {
    let t = String::from_utf8_lossy(line).trim().to_string();
    if t.is_empty() {
        return;
    }
    let _ = logger::try_log(level, format_args!("{t}"));
    let mut out = t;
    out.push('\n');
    let bytes = out.as_bytes();
    match stream {
        StdioStream::Stderr => try_write_preserved_stderr(bytes),
        StdioStream::Stdout => try_write_preserved_stdout(bytes),
    }
}
