//! Windows install path: opens the log file, duplicates the preserved terminal stderr
//! handle, and attaches the crash handler with a vectored-exception callback that
//! writes through `WriteFile` only.

use std::os::windows::io::IntoRawHandle;
use std::path::Path;
use std::sync::OnceLock;

use crash_handler::{CrashEventResult, CrashHandler};
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::WriteFile;
use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};

use super::format::format_fatal_line_windows;
use super::stack_trace::write_signal_safe_stack;

/// Raw handles for the Windows fatal-crash callback.
///
/// Handles are stored as `usize` because `HANDLE` (`*mut c_void`) is not `Sync`.
/// They are taken with [`IntoRawHandle`] at install and never closed.
struct WindowsCrashHandles {
    /// Append handle for the renderer log file. Never zero after a successful open.
    log: usize,
    /// Optional duplicate of the launching terminal stderr. Zero when tee is off.
    term: usize,
    /// Preformatted final line pointing at the shared logs root.
    log_directory_footer: Box<[u8]>,
}

static WINDOWS_CRASH_HANDLES: OnceLock<WindowsCrashHandles> = OnceLock::new();

/// Opens the log file and the preserved terminal handle, then attaches the handler.
pub(super) fn install_impl(log_path: &Path) -> Result<(), String> {
    use std::fs::OpenOptions;

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| e.to_string())?;
    // `into_raw_handle` transfers the handle without closing it. The handler
    // process keeps it open until exit; closing it would race a crash write.
    let log_handle = log.into_raw_handle() as usize;
    let term_handle = crate::native_stdio::duplicate_preserved_stderr_file_for_crash_log()
        .map(|file| file.into_raw_handle() as usize)
        .unwrap_or(0);

    WINDOWS_CRASH_HANDLES
        .set(WindowsCrashHandles {
            log: log_handle,
            term: term_handle,
            log_directory_footer: super::log_directory_footer_bytes(),
        })
        .map_err(|_e| "fatal crash log handles already installed".to_string())?;

    // SAFETY: `CrashHandler::attach` installs a process-wide vectored exception handler.
    // The closure formats into stack arrays, loads the already-initialized
    // `WINDOWS_CRASH_HANDLES` cell, walks frames with `backtrace::trace_unsynchronized`,
    // and writes with `WriteFile` on the handles opened above. It does not allocate,
    // take a mutex, call `backtrace::resolve`, or use `catch_unwind`. The handle is
    // leaked below so the handler stays installed until exit.
    let handler = unsafe {
        CrashHandler::attach(crash_handler::make_crash_event(|ctx| {
            let mut buf = [0u8; 224];
            let n = format_fatal_line_windows(ctx, &mut buf);
            let data = &buf[..n];
            if let Some(handles) = WINDOWS_CRASH_HANDLES.get() {
                handles.write_all(data);
                let mut context_buf = [0u8; 512];
                let context_n = crate::crash_context::write_minimal_snapshot(&mut context_buf);
                handles.write_all(&context_buf[..context_n]);
                write_signal_safe_stack(|chunk| handles.write_all(chunk));
                handles.write_all(&handles.log_directory_footer);
            }
            CrashEventResult::from(false)
        }))
        .map_err(|e| e.to_string())?
    };
    #[expect(
        clippy::mem_forget,
        reason = "CrashHandler must not be dropped after attach; the handler is process-global until exit"
    )]
    std::mem::forget(handler);
    Ok(())
}

impl WindowsCrashHandles {
    fn write_all(&self, data: &[u8]) {
        // SAFETY: `log` and `term` came from `IntoRawHandle` at install and are never
        // closed. `WriteFile` on those handles does not take a user-mode mutex and
        // does not allocate. `GetStdHandle` reads the process stderr slot.
        unsafe {
            let remainder = write_loop_handle(self.log as HANDLE, data);
            if !remainder.is_empty() {
                let stderr = GetStdHandle(STD_ERROR_HANDLE);
                if !stderr.is_null() && stderr != INVALID_HANDLE_VALUE {
                    let _ = write_loop_handle(stderr, remainder);
                }
            }
            if self.term != 0 {
                let _ = write_loop_handle(self.term as HANDLE, data);
            }
        }
    }
}

/// Writes `data` to `handle`. Returns the suffix that was not written.
///
/// # Safety
///
/// `handle` must be an open synchronous file handle valid for `WriteFile` for
/// the duration of the call. The handle is not closed here.
unsafe fn write_loop_handle(handle: HANDLE, mut data: &[u8]) -> &[u8] {
    while !data.is_empty() {
        let len = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let mut written = 0u32;
        // SAFETY: `handle` is the caller's open handle. `data` is a readable slice.
        // A null overlapped pointer is the synchronous write.
        let ok = unsafe {
            WriteFile(
                handle,
                data.as_ptr(),
                len,
                &raw mut written,
                core::ptr::null_mut(),
            )
        };
        if ok == 0 || written == 0 {
            return data;
        }
        let n = written as usize;
        if n > data.len() {
            return data;
        }
        data = &data[n..];
    }
    &[]
}
