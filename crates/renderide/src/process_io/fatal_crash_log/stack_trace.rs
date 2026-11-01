//! Signal-safe stack capture for fatal crashes on Linux, Android, and Windows.
//!
//! The handler walks frames with [`backtrace::trace_unsynchronized`] and formats
//! hex instruction pointers plus the precomputed executable span into a stack
//! buffer. It does not allocate, take a lock, or call [`backtrace::resolve`].
//! Symbolication of the saved addresses is [`super::symbolicate`], which runs
//! on a normal thread.

use core::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use super::format::{write_decimal, write_hex_fixed};
use super::module_span::{module_base, module_end};

/// Upper bound on captured stack frames per fatal crash.
///
/// Sized to cover realistic render-graph depths while keeping the hex buffer
/// on the stack.
pub(super) const MAX_FRAMES: usize = 64;

/// Byte capacity of the stack buffer used by [`write_signal_safe_stack`].
const HEX_BUF_LEN: usize = 4096;

/// Notice written in place of a symbol block. The bytes are not `SYMBOLS:\n`,
/// so a later pass can tell a raw record from one that has already been resolved.
const DEFERRED_NOTICE: &[u8] =
    b"SYMBOLS: deferred (raw instruction pointers; symbolicated on next start)\n";

/// Reentry guard for fatal-crash stack capture.
///
/// [`write_signal_safe_stack`] sets this before walking. A nested fault inside
/// the walk finds it set and returns. The flag is not cleared on the crash path;
/// tests reset it through [`reset_reentry_for_test`].
static CRASH_REENTRY: AtomicBool = AtomicBool::new(false);

/// Captures instruction pointers and writes the raw record through `write_all`.
///
/// This is the function the signal handler and the vectored exception handler
/// call. It does not allocate, take a mutex, call [`backtrace::resolve`], or
/// use [`std::panic::catch_unwind`].
pub(super) fn write_signal_safe_stack(mut write_all: impl FnMut(&[u8])) {
    if CRASH_REENTRY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let mut ips: [*mut c_void; MAX_FRAMES] = [core::ptr::null_mut(); MAX_FRAMES];
    // SAFETY: `CRASH_REENTRY` was just set. The fatal handler does not walk
    // again for the rest of the process, so this call does not overlap another
    // `backtrace::trace_unsynchronized` from this handler.
    let n = unsafe { capture_frame_ips(&mut ips) };
    let mut buf = [0u8; HEX_BUF_LEN];
    let mut w = format_frames_hex(&ips[..n], &mut buf);
    w += format_module_span(&mut buf[w..]);
    let room = HEX_BUF_LEN.saturating_sub(w);
    let ncopy = DEFERRED_NOTICE.len().min(room);
    buf[w..w + ncopy].copy_from_slice(&DEFERRED_NOTICE[..ncopy]);
    w += ncopy;
    write_all(&buf[..w]);
}

/// Clears [`CRASH_REENTRY`] so a test process can enter the handler writer again.
pub(super) fn reset_reentry_for_test() {
    CRASH_REENTRY.store(false, Ordering::Release);
}

/// Serializes tests that reset [`CRASH_REENTRY`].
#[cfg(test)]
pub(super) fn lock_reentry_for_test() -> parking_lot::MutexGuard<'static, ()> {
    static LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    LOCK.lock()
}

/// Walks the current thread's stack and stores up to [`MAX_FRAMES`] instruction pointers.
///
/// # Safety
///
/// Caller must ensure no other thread is inside `backtrace::trace_unsynchronized`
/// for this process. The handler path holds [`CRASH_REENTRY`] for the process lifetime.
unsafe fn capture_frame_ips(out: &mut [*mut c_void; MAX_FRAMES]) -> usize {
    let mut n = 0usize;
    // SAFETY: see the function contract. The closure only stores pointers into `out`.
    unsafe {
        backtrace::trace_unsynchronized(|frame| {
            if n < out.len() {
                out[n] = frame.ip();
                n += 1;
                true
            } else {
                false
            }
        });
    }
    n
}

/// Formats `STACK (<n> frames):\n  0x...\n` into `out`. Returns bytes written.
fn format_frames_hex(ips: &[*mut c_void], out: &mut [u8]) -> usize {
    const HDR_PREFIX: &[u8] = b"STACK (";
    const HDR_SUFFIX: &[u8] = b" frames):\n";
    const LINE_PREFIX: &[u8] = b"  0x";
    const LINE_SUFFIX: &[u8] = b"\n";
    const HEX_DIGITS: usize = 16;

    let mut w = 0usize;
    if w + HDR_PREFIX.len() > out.len() {
        return 0;
    }
    out[w..w + HDR_PREFIX.len()].copy_from_slice(HDR_PREFIX);
    w += HDR_PREFIX.len();
    w += write_decimal(ips.len() as u64, &mut out[w..]);
    if w + HDR_SUFFIX.len() > out.len() {
        return w;
    }
    out[w..w + HDR_SUFFIX.len()].copy_from_slice(HDR_SUFFIX);
    w += HDR_SUFFIX.len();

    for ip in ips {
        if w + LINE_PREFIX.len() + HEX_DIGITS + LINE_SUFFIX.len() > out.len() {
            break;
        }
        out[w..w + LINE_PREFIX.len()].copy_from_slice(LINE_PREFIX);
        w += LINE_PREFIX.len();
        w += write_hex_fixed::<16>(*ip as u64, &mut out[w..]);
        out[w..w + LINE_SUFFIX.len()].copy_from_slice(LINE_SUFFIX);
        w += LINE_SUFFIX.len();
    }
    w
}

fn format_module_span(out: &mut [u8]) -> usize {
    const PREFIX: &[u8] = b"MODULE base=0x";
    const MID: &[u8] = b" end=0x";
    const SUFFIX: &[u8] = b"\n";
    let need = PREFIX.len() + 16 + MID.len() + 16 + SUFFIX.len();
    if out.len() < need {
        return 0;
    }
    let mut w = 0usize;
    out[w..w + PREFIX.len()].copy_from_slice(PREFIX);
    w += PREFIX.len();
    w += write_hex_fixed::<16>(module_base(), &mut out[w..]);
    out[w..w + MID.len()].copy_from_slice(MID);
    w += MID.len();
    w += write_hex_fixed::<16>(module_end(), &mut out[w..]);
    out[w..w + SUFFIX.len()].copy_from_slice(SUFFIX);
    w + SUFFIX.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_frames_hex_shape() {
        let ips: [*mut c_void; 3] = [
            0xDEAD_BEEF_CAFE_BABE_u64 as *mut _,
            0x0123_4567_89AB_CDEF_u64 as *mut _,
            0xFFFF_FFFF_FFFF_FFFF_u64 as *mut _,
        ];
        let mut out = [0u8; HEX_BUF_LEN];
        let n = format_frames_hex(&ips, &mut out);
        let s = std::str::from_utf8(&out[..n]).expect("utf8");
        assert!(s.starts_with("STACK (3 frames):\n"), "header: {s:?}");
        assert!(s.contains("  0xDEADBEEFCAFEBABE\n"));
        assert!(s.contains("  0x0123456789ABCDEF\n"));
        assert!(s.contains("  0xFFFFFFFFFFFFFFFF\n"));
        assert!(s.ends_with('\n'));
    }

    #[test]
    fn max_frames_leave_room_for_module_span() {
        let ips = [core::ptr::null_mut::<c_void>(); MAX_FRAMES];
        let mut out = [0u8; HEX_BUF_LEN];
        let n = format_frames_hex(&ips, &mut out);
        let rest = HEX_BUF_LEN - n;
        assert!(
            rest > DEFERRED_NOTICE.len() + 64,
            "hex record used {n} of {HEX_BUF_LEN}"
        );
    }

    #[test]
    fn deferred_notice_is_not_a_symbol_block() {
        let text = std::str::from_utf8(DEFERRED_NOTICE).expect("utf8");
        assert!(!text.contains("SYMBOLS:\n"));
        assert!(text.starts_with("SYMBOLS: deferred"));
    }

    #[test]
    fn reentry_guard_blocks_second_entry() {
        let _guard = lock_reentry_for_test();
        reset_reentry_for_test();

        let mut count = 0usize;
        write_signal_safe_stack(|_| {
            count += 1;
        });
        let first = count;
        write_signal_safe_stack(|_| {
            count += 1;
        });
        let second = count;

        assert!(
            first >= 1,
            "first call should have written the raw instruction-pointer record"
        );
        assert_eq!(
            first, second,
            "second call should be blocked by the reentry guard"
        );
    }

    #[test]
    fn handler_record_has_ips_and_no_symbol_block() {
        let _guard = lock_reentry_for_test();
        super::super::module_span::publish_module_span();
        reset_reentry_for_test();
        let mut buf = Vec::new();
        write_signal_safe_stack(|chunk| buf.extend_from_slice(chunk));
        let text = String::from_utf8(buf).expect("utf8");
        assert!(text.contains("STACK ("), "{text}");
        assert!(text.contains("  0x"), "{text}");
        assert!(text.contains("MODULE base=0x"), "{text}");
        assert!(text.contains("SYMBOLS: deferred"), "{text}");
        assert!(!text.contains("SYMBOLS:\n"), "{text}");
    }
}
