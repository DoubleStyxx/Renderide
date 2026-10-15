//! Fixed-width integer/hex writers and per-platform fatal-line formatters used by the crash
//! handler.
//!
//! Every function operates on caller-supplied byte slices and does not allocate, so the same
//! formatters are safe to call from the Unix async-signal-safe path as well as the Windows
//! structured-exception path.

#[cfg(any(unix, windows))]
use crash_handler::CrashContext;

/// Writes `n` as a decimal ASCII string into `out`. Returns the byte count written, or `0` if
/// `out` is empty.
///
/// A 20-byte stack scratch buffer covers the full `u64` range
/// (`18446744073709551615` = 20 digits) so the writer never allocates.
#[cfg(any(unix, windows))]
pub(super) fn write_decimal(mut n: u64, out: &mut [u8]) -> usize {
    if n == 0 {
        if out.is_empty() {
            return 0;
        }
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut i = 0usize;
    while n > 0 {
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let mut w = 0usize;
    while i > 0 {
        i -= 1;
        if w >= out.len() {
            break;
        }
        out[w] = tmp[i];
        w += 1;
    }
    w
}

/// Writes the low `N` nibbles of `n` as zero-padded uppercase hex into `out`. Returns `N` on
/// success or `0` if `out.len() < N`.
///
/// `N` must satisfy `1 <= N <= 16`. Bits above `N * 4` are silently dropped, which matches the
/// existing fixed-width formatting of 32-bit Windows exception codes and 64-bit instruction
/// pointers.
#[cfg(any(target_os = "linux", target_os = "android", windows))]
pub(super) fn write_hex_fixed<const N: usize>(n: u64, out: &mut [u8]) -> usize {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    if out.len() < N {
        return 0;
    }
    let mut v = n;
    for i in (0..N).rev() {
        out[i] = HEX[(v & 0xf) as usize];
        v >>= 4;
    }
    N
}

/// Formats the first line of a fatal-crash record on Unix targets.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn format_fatal_line_unix(ctx: &CrashContext, buf: &mut [u8; 224]) -> usize {
    format_linux_signal(ctx, buf)
}

/// Formats the first line of a fatal-crash record on macOS targets.
#[cfg(target_os = "macos")]
pub(super) fn format_fatal_line_unix(ctx: &CrashContext, buf: &mut [u8; 224]) -> usize {
    format_macos_exception(ctx, buf)
}

/// Fallback fatal-line formatter for Unix targets without a dedicated branch.
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos"))
))]
pub(super) fn format_fatal_line_unix(_ctx: &CrashContext, buf: &mut [u8; 224]) -> usize {
    const MSG: &[u8] = b"FATAL: unix crash (fatal fault; see crash-handler)\n";
    let n = MSG.len().min(buf.len());
    buf[..n].copy_from_slice(&MSG[..n]);
    n
}

/// Formats a Linux/Android fatal-signal first line as
/// `FATAL: fatal signal (ssi_signo=<n>)\n`.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn format_linux_signal(ctx: &CrashContext, buf: &mut [u8; 224]) -> usize {
    const PREFIX: &[u8] = b"FATAL: fatal signal (ssi_signo=";
    let sig = ctx.siginfo.ssi_signo;
    write_prefix_u32_newline(buf, PREFIX, sig)
}

/// Formats a macOS Mach exception first line as
/// `FATAL: macOS exception (kind=<n>, code=<n>)\n`, falling back to a generic message when the
/// crash callback supplies no exception details.
#[cfg(target_os = "macos")]
fn format_macos_exception(ctx: &CrashContext, buf: &mut [u8; 224]) -> usize {
    if let Some(ex) = ctx.exception {
        const P1: &[u8] = b"FATAL: macOS exception (kind=";
        const P2: &[u8] = b", code=";
        const SUF: &[u8] = b")\n";
        let mut w = 0usize;
        buf[w..w + P1.len()].copy_from_slice(P1);
        w += P1.len();
        w += write_decimal(u64::from(ex.kind), &mut buf[w..]);
        buf[w..w + P2.len()].copy_from_slice(P2);
        w += P2.len();
        w += write_decimal(ex.code, &mut buf[w..]);
        buf[w..w + SUF.len()].copy_from_slice(SUF);
        w += SUF.len();
        w
    } else {
        const MSG: &[u8] = b"FATAL: macOS crash (no exception details)\n";
        let n = MSG.len().min(buf.len());
        buf[..n].copy_from_slice(&MSG[..n]);
        n
    }
}

/// Formats a Windows structured-exception first line as
/// `FATAL: Windows exception (code=0x<XXXXXXXX>)\n`.
#[cfg(windows)]
pub(super) fn format_fatal_line_windows(ctx: &CrashContext, buf: &mut [u8; 224]) -> usize {
    const PREFIX: &[u8] = b"FATAL: Windows exception (code=0x";
    const SUFFIX: &[u8] = b")\n";
    let code = ctx.exception_code as u32;
    let mut w = 0usize;
    buf[w..w + PREFIX.len()].copy_from_slice(PREFIX);
    w += PREFIX.len();
    w += write_hex_fixed::<8>(u64::from(code), &mut buf[w..]);
    buf[w..w + SUFFIX.len()].copy_from_slice(SUFFIX);
    w + SUFFIX.len()
}

/// Writes `prefix`, decimal `n`, then `)\n` into the fixed fatal-line buffer.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn write_prefix_u32_newline(buf: &mut [u8; 224], prefix: &[u8], n: u32) -> usize {
    const SUFFIX: &[u8] = b")\n";
    let mut w = 0usize;
    if w + prefix.len() > buf.len() {
        return 0;
    }
    buf[w..w + prefix.len()].copy_from_slice(prefix);
    w += prefix.len();
    w += write_decimal(u64::from(n), &mut buf[w..]);
    if w + SUFFIX.len() <= buf.len() {
        buf[w..w + SUFFIX.len()].copy_from_slice(SUFFIX);
        w += SUFFIX.len();
    }
    w
}

#[cfg(test)]
mod tests {
    #[cfg(any(unix, windows))]
    #[test]
    fn write_decimal_formats() {
        let mut out = [0u8; 24];
        let n = super::write_decimal(12345, &mut out);
        assert_eq!(&out[..n], b"12345");
        let n0 = super::write_decimal(0, &mut out);
        assert_eq!(&out[..n0], b"0");
        let n_max = super::write_decimal(u64::MAX, &mut out);
        assert_eq!(&out[..n_max], b"18446744073709551615");
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn linux_fatal_signal_line_contains_signo() {
        use crash_handler::CrashContext;
        // SAFETY: `CrashContext` on Linux is a plain aggregate of integer fields (siginfo_t-like);
        // all-zero is a valid bit pattern. Test-only construction; never observed by the kernel.
        let mut ctx: CrashContext = unsafe { std::mem::zeroed() };
        ctx.siginfo.ssi_signo = 11;
        let mut buf = [0u8; 224];
        let n = super::format_linux_signal(&ctx, &mut buf);
        let line = std::str::from_utf8(&buf[..n]).expect("utf8");
        assert!(line.starts_with("FATAL: fatal signal (ssi_signo="));
        assert!(line.contains("11"));
        assert!(line.ends_with(")\n"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_fatal_line_no_exception() {
        use crash_handler::CrashContext;
        // SAFETY: macOS `CrashContext` fields are integers/`Option<ExceptionInfo>`; all-zero is a
        // valid bit pattern (`None` discriminant). Test-only value.
        let ctx: CrashContext = unsafe { std::mem::zeroed() };
        let mut buf = [0u8; 224];
        let n = super::format_macos_exception(&ctx, &mut buf);
        let line = std::str::from_utf8(&buf[..n]).expect("utf8");
        assert!(line.contains("FATAL:"));
        assert!(line.ends_with('\n'));
    }

    #[cfg(windows)]
    #[test]
    fn windows_fatal_line_contains_exception_code() {
        use crash_handler::CrashContext;
        // SAFETY: Windows `CrashContext` is an integer/pointer aggregate; all-zero is a valid
        // bit pattern. Test-only value that never traverses the real crash path.
        let mut ctx: CrashContext = unsafe { std::mem::zeroed() };
        ctx.exception_code = 0xC000_0005_u32 as i32;
        let mut buf = [0u8; 224];
        let n = super::format_fatal_line_windows(&ctx, &mut buf);
        let line = std::str::from_utf8(&buf[..n]).expect("utf8");
        assert!(line.starts_with("FATAL: Windows exception (code=0x"));
        assert!(line.contains("C0000005"));
        assert!(line.ends_with(")\n"));
    }

    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn write_hex_fixed_8_uppercase() {
        let mut out = [0u8; 8];
        let n = super::write_hex_fixed::<8>(0xDEAD_BEEF, &mut out);
        assert_eq!(n, 8);
        assert_eq!(&out[..n], b"DEADBEEF");
    }

    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    #[test]
    fn write_hex_fixed_16_formats() {
        let mut out = [0u8; 16];
        let n = super::write_hex_fixed::<16>(0xDEAD_BEEF_CAFE_BABE, &mut out);
        assert_eq!(n, 16);
        assert_eq!(&out[..n], b"DEADBEEFCAFEBABE");

        let mut small = [0u8; 8];
        let n_small = super::write_hex_fixed::<16>(0x1, &mut small);
        assert_eq!(n_small, 0, "buffer shorter than 16 bytes must return 0");
    }
}
