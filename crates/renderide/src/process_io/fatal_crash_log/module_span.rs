//! Executable address span captured before the crash handler is attached.
//!
//! The handler only loads these atomics. Computing the span reads `/proc/self/maps`
//! or the PE headers, which allocates, so that work stays on a normal thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Lowest mapped address of the executable that contains [`module_anchor_addr`].
static MODULE_BASE: AtomicU64 = AtomicU64::new(0);
/// One past the last byte of that executable image.
static MODULE_END: AtomicU64 = AtomicU64::new(0);
/// Set after a span has been stored. The handler does not consult this flag.
static MODULE_SPAN_READY: AtomicBool = AtomicBool::new(false);

// The handler loads these with `Ordering::Acquire`. That load is a single
// instruction on every architecture this crate ships (see release.yml). This
// toolchain's `AtomicU64` alias does not expose `is_always_lock_free`, so the
// guarantee is the architecture list instead of a runtime query.
const _: () = assert!(
    cfg!(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "powerpc64",
        target_arch = "s390x",
    )),
    "fatal-crash handler loads AtomicU64 from a signal or exception context"
);

/// Address of this function. Used to recognize which mapping is the executable.
///
/// The address is never called for its effect. `inline(never)` keeps a real
/// instruction in the image so the mapping lookup has a stable anchor.
#[inline(never)]
pub(super) fn module_anchor_addr() -> usize {
    module_anchor_addr as *const () as usize
}

/// Loads the published executable base. Zero when [`publish_module_span`] has not stored one.
pub(super) fn module_base() -> u64 {
    MODULE_BASE.load(Ordering::Acquire)
}

/// Loads the published executable end. Zero when no span has been stored.
pub(super) fn module_end() -> u64 {
    MODULE_END.load(Ordering::Acquire)
}

/// Records the executable span for the handler. Safe to call more than once.
///
/// A failed lookup leaves the atomics unchanged so a later call can retry.
/// This function allocates and must not run inside a signal or exception handler.
pub(super) fn publish_module_span() {
    if MODULE_SPAN_READY.load(Ordering::Acquire) {
        return;
    }
    let Some((base, end)) = current_exe_span() else {
        return;
    };
    if end <= base {
        return;
    }
    MODULE_BASE.store(base, Ordering::Release);
    MODULE_END.store(end, Ordering::Release);
    MODULE_SPAN_READY.store(true, Ordering::Release);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn current_exe_span() -> Option<(u64, u64)> {
    let maps = std::fs::read("/proc/self/maps").ok()?;
    let text = String::from_utf8_lossy(&maps);
    let anchor = module_anchor_addr() as u64;
    let mut anchor_path: Option<&str> = None;
    for line in text.lines() {
        let Some((start, end, path)) = parse_map_line(line) else {
            continue;
        };
        if anchor >= start && anchor < end {
            anchor_path = Some(path);
            break;
        }
    }
    let anchor_path = anchor_path?;
    let mut lo = u64::MAX;
    let mut hi = 0u64;
    let mut any = false;
    for line in text.lines() {
        let Some((start, end, path)) = parse_map_line(line) else {
            continue;
        };
        if path != anchor_path {
            continue;
        }
        lo = lo.min(start);
        hi = hi.max(end);
        any = true;
    }
    (any && hi > lo).then_some((lo, hi))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn parse_map_line(line: &str) -> Option<(u64, u64, &str)> {
    let mut rest = line;
    for _ in 0..5 {
        let trimmed = rest.trim_start();
        let sp = trimmed.find(char::is_whitespace)?;
        rest = &trimmed[sp..];
    }
    let path = rest.trim_start();
    if path.is_empty() {
        return None;
    }
    let range = line.split_whitespace().next()?;
    let (start_s, end_s) = range.split_once('-')?;
    let start = u64::from_str_radix(start_s, 16).ok()?;
    let end = u64::from_str_radix(end_s, 16).ok()?;
    Some((start, end, path))
}

#[cfg(windows)]
fn current_exe_span() -> Option<(u64, u64)> {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

    // SAFETY: a null module name returns the executable that started this process.
    // A null handle is a failed call and is not dereferenced.
    let base = unsafe { GetModuleHandleW(core::ptr::null()) };
    if base.is_null() {
        return None;
    }
    // SAFETY: `base` is the mapped image of this executable. `pe_image_span` reads
    // only the DOS and NT headers, and only after the signatures check out.
    let (lo, hi) = unsafe { pe_image_span(base as usize) }?;
    let anchor = module_anchor_addr() as u64;
    (lo <= anchor && anchor < hi).then_some((lo, hi))
}

/// Reads `SizeOfImage` from a mapped PE image.
///
/// # Safety
///
/// `base` must point at a readable in-process image. Signature mismatches return
/// `None`. `e_lfanew` is rejected above 1 MiB so a corrupt header cannot walk far.
#[cfg(windows)]
unsafe fn pe_image_span(base: usize) -> Option<(u64, u64)> {
    const DOS_MAGIC: u16 = 0x5A4D;
    const NT_MAGIC: u32 = 0x0000_4550;
    const PE32_MAGIC: u16 = 0x10B;
    const PE32_PLUS_MAGIC: u16 = 0x20B;
    /// `SizeOfImage` sits at the same optional-header offset for PE32 and PE32+.
    const SIZE_OF_IMAGE_OFFSET: usize = 56;

    if base == 0 {
        return None;
    }
    let dos = base as *const u8;
    if read_u16(dos) != DOS_MAGIC {
        return None;
    }
    // SAFETY: `dos` is the mapped image base. Offset 0x3C is the DOS `e_lfanew` field,
    // four bytes, which `read_u32` copies before it is interpreted.
    let e_lfanew = read_u32(unsafe { dos.add(0x3C) }) as usize;
    if e_lfanew > 1024 * 1024 {
        return None;
    }
    // SAFETY: `e_lfanew` was bounded to 1 MiB above. The NT header is read only after
    // this pointer is formed, and a bad signature returns `None`.
    let nt = unsafe { dos.add(e_lfanew) };
    if read_u32(nt) != NT_MAGIC {
        return None;
    }
    // PE signature (4) + COFF file header (20) = 24.
    // SAFETY: the NT signature matched, so the 20-byte COFF header and the following
    // optional header are the standard PE layout at this image base.
    let opt = unsafe { nt.add(24) };
    let magic = read_u16(opt);
    if magic != PE32_MAGIC && magic != PE32_PLUS_MAGIC {
        return None;
    }
    // SAFETY: both optional-header magics place `SizeOfImage` at byte 56.
    let size = read_u32(unsafe { opt.add(SIZE_OF_IMAGE_OFFSET) });
    if size == 0 {
        return None;
    }
    let base_u = base as u64;
    Some((base_u, base_u.saturating_add(u64::from(size))))
}

#[cfg(windows)]
fn read_u16(ptr: *const u8) -> u16 {
    let mut buf = [0u8; 2];
    // SAFETY: caller guarantees `ptr` is readable for 2 bytes.
    unsafe { core::ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), 2) };
    u16::from_le_bytes(buf)
}

#[cfg(windows)]
fn read_u32(ptr: *const u8) -> u32 {
    let mut buf = [0u8; 4];
    // SAFETY: caller guarantees `ptr` is readable for 4 bytes.
    unsafe { core::ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), 4) };
    u32::from_le_bytes(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_span_covers_anchor() {
        publish_module_span();
        let base = module_base();
        let end = module_end();
        let anchor = module_anchor_addr() as u64;
        assert!(
            base < end,
            "executable span was not published ({base:#x}..{end:#x})"
        );
        assert!(
            base <= anchor && anchor < end,
            "anchor {anchor:#x} outside executable span {base:#x}..{end:#x}"
        );
    }
}
