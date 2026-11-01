//! Deliberate fatal fault for the crash-handler probe. Not used by the renderer.

/// Raises a fatal fault in this process and does not return.
///
/// Linux and macOS write a `PROT_NONE` page so the kernel delivers `SIGSEGV` or
/// `EXC_BAD_ACCESS`. Windows commits a `PAGE_NOACCESS` page so the vectored
/// handler sees an access violation. Core dumps and the Windows fault dialog
/// are suppressed first; those dialogs would hang CI.
#[inline(never)]
pub(super) fn trip_fatal_fault() -> ! {
    #[cfg(unix)]
    trip_unix();
    #[cfg(windows)]
    trip_windows();
    #[cfg(not(any(unix, windows)))]
    std::process::abort();
}

#[cfg(unix)]
#[inline(never)]
fn trip_unix() -> ! {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let anon = libc::MAP_ANONYMOUS;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let anon = libc::MAP_ANON;

    // SAFETY: `setrlimit` only changes this process's core limit. `mmap` returns
    // a page with no access rights; the volatile write is the fault the probe
    // exists to raise. The pointer is not used as a Rust reference.
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &raw const limit);
        let page = libc::mmap(
            core::ptr::null_mut(),
            4096,
            libc::PROT_NONE,
            libc::MAP_PRIVATE | anon,
            -1,
            0,
        );
        if page == libc::MAP_FAILED {
            std::process::abort();
        }
        let page = std::hint::black_box(page);
        core::ptr::write_volatile(page.cast::<u8>(), 1);
    }
    std::process::abort();
}

#[cfg(windows)]
#[inline(never)]
fn trip_windows() -> ! {
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEM_RESERVE, PAGE_NOACCESS, VirtualAlloc,
    };

    disable_windows_fault_ui();
    // SAFETY: `VirtualAlloc` reserves a page owned by this process. `PAGE_NOACCESS`
    // makes the volatile write an access violation. The pointer is not a Rust reference.
    let page = unsafe {
        VirtualAlloc(
            core::ptr::null(),
            4096,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_NOACCESS,
        )
    };
    if page.is_null() {
        std::process::abort();
    }
    let page = std::hint::black_box(page);
    // SAFETY: deliberate fault; see the function comment.
    unsafe {
        core::ptr::write_volatile(page.cast::<u8>(), 1);
    }
    std::process::abort();
}

#[cfg(windows)]
fn disable_windows_fault_ui() {
    const SEM_FAILCRITICALERRORS: u32 = 0x0001;
    const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
    const SEM_NOOPENFILEERRORBOX: u32 = 0x8000;

    unsafe extern "system" {
        fn SetErrorMode(mode: u32) -> u32;
    }

    // SAFETY: `SetErrorMode` stores a process error-mode bitset and returns the
    // previous mode. These are the documented Win32 flag values. No pointer is dereferenced.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    }
}
