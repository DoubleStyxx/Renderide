//! Unix `pipe` + `dup2` redirect of a stdio fd onto a logger-forwarder thread, preserving the
//! original fd as a duplicated owned handle so the launching terminal can still receive tee'd
//! output.

use super::forward::forward_pipe_lines_to_logger_unix;
use super::preserved::{StdioStream, store_preserved_unix};

/// Replaces `target_fd` with the write end of a fresh pipe and spawns a reader thread named
/// `thread_name` that forwards each line into the logger. The pre-redirect fd is duplicated
/// and stashed in the preserved-handle slot keyed by `stream`.
pub(super) fn try_redirect_unix_stream(
    target_fd: i32,
    thread_name: &'static str,
    stream: StdioStream,
) -> Result<(), String> {
    use std::thread;

    // SAFETY: all libc calls below operate on file descriptors that this function either just
    // created (via `pipe`/`dup`) or received from the caller (`target_fd` is always a valid
    // stdio fd). Ownership is tracked manually: each branch that errors out closes every fd it
    // created; the success path transfers ownership into `OwnedFd` via `store_preserved_unix`.
    unsafe {
        let mut fds = [0i32; 2];
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(format!("pipe: {}", std::io::Error::last_os_error()));
        }
        let rfd = fds[0];
        let wfd = fds[1];

        for fd in [rfd, wfd] {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
            }
        }

        let saved = libc::dup(target_fd);
        if saved < 0 {
            libc::close(rfd);
            libc::close(wfd);
            return Err(format!(
                "dup({target_fd}): {}",
                std::io::Error::last_os_error()
            ));
        }

        if libc::dup2(wfd, target_fd) < 0 {
            let e = std::io::Error::last_os_error();
            libc::close(rfd);
            libc::close(wfd);
            libc::close(saved);
            return Err(format!("dup2(pipe -> fd {target_fd}): {e}"));
        }
        libc::close(wfd);

        let spawn = thread::Builder::new()
            .name(thread_name.into())
            .spawn(move || forward_pipe_lines_to_logger_unix(rfd, stream));

        match spawn {
            Ok(_) => {
                store_preserved_unix(stream, saved);
                Ok(())
            }
            Err(e) => {
                let _ = libc::dup2(saved, target_fd);
                libc::close(rfd);
                libc::close(saved);
                Err(format!("thread spawn: {e}"))
            }
        }
    }
}
