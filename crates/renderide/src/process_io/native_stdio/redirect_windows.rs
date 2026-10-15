//! Windows `CreatePipe` + `SetStdHandle` redirect of a stdio handle onto a logger-forwarder
//! thread, preserving the original console handle so the launching terminal can still receive
//! tee'd output.

use super::forward::forward_pipe_lines_to_logger_windows;
use super::preserved::{StdioStream, store_preserved_windows};

/// Replaces `std_handle` with the write end of a fresh pipe and spawns a reader thread named
/// `thread_name` that forwards each line into the logger. The pre-redirect console handle is
/// wrapped into [`std::fs::File`] and stashed in the preserved-handle slot keyed by `stream`.
pub(super) fn try_redirect_windows_stream(
    std_handle: u32,
    thread_name: &'static str,
    stream: StdioStream,
) -> Result<(), String> {
    use std::fs::File;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use std::ptr;
    use std::thread;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{GetStdHandle, SetStdHandle};
    use windows_sys::Win32::System::Pipes::CreatePipe;

    // SAFETY: Win32 API calls on handles this function owns; each error path closes every
    // handle it created, and the success path transfers handles into `OwnedHandle`/`File`.
    unsafe {
        let mut read_h: HANDLE = INVALID_HANDLE_VALUE;
        let mut write_h: HANDLE = INVALID_HANDLE_VALUE;
        if CreatePipe(&raw mut read_h, &raw mut write_h, ptr::null(), 0) == 0 {
            return Err(format!("CreatePipe: {}", std::io::Error::last_os_error()));
        }

        let old = GetStdHandle(std_handle);
        if old.is_null() || old == INVALID_HANDLE_VALUE {
            CloseHandle(read_h);
            CloseHandle(write_h);
            return Err(format!(
                "GetStdHandle({std_handle}): {}",
                std::io::Error::last_os_error()
            ));
        }

        if SetStdHandle(std_handle, write_h) == 0 {
            CloseHandle(read_h);
            CloseHandle(write_h);
            return Err(format!("SetStdHandle: {}", std::io::Error::last_os_error()));
        }

        let read_owned = OwnedHandle::from_raw_handle(read_h);

        let spawn = thread::Builder::new()
            .name(thread_name.into())
            .spawn(move || forward_pipe_lines_to_logger_windows(read_owned, stream));

        match spawn {
            Ok(_) => {
                let old_owned = OwnedHandle::from_raw_handle(old);
                let preserved_file = File::from(old_owned);
                store_preserved_windows(stream, preserved_file);
                Ok(())
            }
            Err(e) => {
                let _ = SetStdHandle(std_handle, old);
                CloseHandle(write_h);
                Err(format!("thread spawn: {e}"))
            }
        }
    }
}
