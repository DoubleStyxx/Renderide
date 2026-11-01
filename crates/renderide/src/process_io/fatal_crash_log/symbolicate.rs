//! Deferred symbolication of a fatal-crash instruction-pointer record.
//!
//! Runs on a normal thread: the next process start, or a test. It must not be
//! called from a signal handler or a vectored exception handler. `catch_unwind`
//! below is only a panic boundary so a symbolicator bug cannot abort the next
//! launch. It is not a signal-safety fence, and the handler never calls it.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use super::module_span::{module_base, publish_module_span};
use super::stack_trace::MAX_FRAMES;

const TAIL_BYTES: u64 = 64 * 1024;

/// Symbolicates the hex instruction pointers in a handler record.
///
/// Addresses that fell inside the saved executable span are relocated by
/// `current_base - saved_base` and resolved with [`backtrace::resolve`].
/// Addresses outside that span are left as `<other module>` so a libc frame
/// is not reported as a function in this executable. When the record has no
/// span, addresses are resolved as-is (correct in the process that captured them).
pub(super) fn symbolicate_saved_record(record: &str) -> String {
    publish_module_span();
    let current_base = module_base();
    let (saved_base, saved_end) = parse_module_span(record).unwrap_or((0, 0));
    let slide = if saved_base != 0 && current_base != 0 {
        current_base.wrapping_sub(saved_base)
    } else {
        0
    };
    let have_span = saved_end > saved_base;
    let ips = parse_ips(record);

    let mut out = String::from("SYMBOLS:\n");
    if ips.is_empty() {
        out.push_str("  <no frames>\n");
        return out;
    }
    for (idx, ip) in ips.iter().enumerate() {
        if have_span && (*ip < saved_base || *ip >= saved_end) {
            let _ = writeln!(out, "  #{idx:02} 0x{ip:016X} <other module>");
            continue;
        }
        let adjusted = ip.wrapping_add(slide);
        resolve_frame(idx, adjusted, &mut out);
    }
    out
}

/// Appends a `SYMBOLS:` block for the last raw stack in `log_path`.
///
/// Missing files and records that already contain `SYMBOLS:` are left unchanged.
/// A panic inside resolution is caught here, on this normal thread, and recorded
/// as a one-line failure so the next start does not retry it.
pub(super) fn append_deferred_symbolication(log_path: &Path) -> Result<(), String> {
    let tail = read_tail(log_path)?;
    let Some(record) = last_pending_stack(&tail) else {
        return Ok(());
    };
    let owned = record.to_string();
    let symbols = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        symbolicate_saved_record(&owned)
    }))
    .unwrap_or_else(|_| String::from("SYMBOLS:\n  <symbolication panicked>\n"));

    let mut file = OpenOptions::new()
        .append(true)
        .open(log_path)
        .map_err(|e| e.to_string())?;
    file.write_all(symbols.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_frame(idx: usize, ip: u64, out: &mut String) {
    let Ok(addr) = usize::try_from(ip) else {
        let _ = writeln!(out, "  #{idx:02} 0x{ip:016X} <no symbol>");
        return;
    };
    let mut any = false;
    backtrace::resolve(addr as *mut core::ffi::c_void, |sym| {
        any = true;
        let _ = write!(out, "  #{idx:02} ");
        match sym.name() {
            Some(name) => {
                let _ = write!(out, "{name}");
            }
            None => out.push_str("???"),
        }
        if let Some(file) = sym.filename() {
            let _ = write!(out, " at {}", file.display());
            if let Some(line) = sym.lineno() {
                let _ = write!(out, ":{line}");
            }
        }
        out.push('\n');
    });
    if !any {
        let _ = writeln!(out, "  #{idx:02} 0x{ip:016X} <no symbol>");
    }
}

fn parse_ips(record: &str) -> Vec<u64> {
    let mut ips = Vec::new();
    for line in record.lines() {
        if ips.len() >= MAX_FRAMES {
            break;
        }
        let Some(hex) = line.trim().strip_prefix("0x") else {
            continue;
        };
        if hex.len() < 16 {
            continue;
        }
        if let Ok(ip) = u64::from_str_radix(&hex[..16], 16) {
            ips.push(ip);
        }
    }
    ips
}

fn parse_module_span(record: &str) -> Option<(u64, u64)> {
    for line in record.lines() {
        let Some(rest) = line.trim().strip_prefix("MODULE base=0x") else {
            continue;
        };
        let (base_hex, end_hex) = rest.split_once(" end=0x")?;
        if base_hex.len() < 16 || end_hex.len() < 16 {
            return None;
        }
        let base = u64::from_str_radix(&base_hex[..16], 16).ok()?;
        let end = u64::from_str_radix(&end_hex[..16], 16).ok()?;
        return Some((base, end));
    }
    None
}

fn last_pending_stack(text: &str) -> Option<&str> {
    let idx = text.rfind("STACK (")?;
    let record = &text[idx..];
    if record.contains("\nSYMBOLS:\n") || record.starts_with("SYMBOLS:\n") {
        return None;
    }
    Some(record)
}

fn read_tail(path: &Path) -> Result<String, String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e.to_string()),
    };
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    // The file is already seeked to the last 64 KiB. `fs::read` would pull the whole log.
    #[expect(
        clippy::verbose_file_reads,
        reason = "the log is seeked to its tail before this read, so reading the whole file would be wrong"
    )]
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if start > 0
        && let Some(nl) = bytes.iter().position(|b| *b == b'\n')
    {
        bytes.drain(..=nl);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_io::fatal_crash_log::module_span::module_base;
    use crate::process_io::fatal_crash_log::stack_trace::{
        lock_reentry_for_test, reset_reentry_for_test, write_signal_safe_stack,
    };

    #[inline(never)]
    fn fatal_crash_symbol_canary(buf: &mut Vec<u8>) {
        write_signal_safe_stack(|chunk| buf.extend_from_slice(chunk));
        std::hint::black_box(buf.len());
    }

    fn capture_record() -> String {
        publish_module_span();
        let _guard = lock_reentry_for_test();
        reset_reentry_for_test();
        let mut buf = Vec::new();
        fatal_crash_symbol_canary(&mut buf);
        String::from_utf8(buf).expect("handler record is ascii")
    }

    fn shift_record(record: &str, delta: u64) -> String {
        let (base, end) = parse_module_span(record).expect("span");
        let mut out = String::new();
        for line in record.lines() {
            let trimmed = line.trim();
            if let Some(hex) = trimmed.strip_prefix("0x")
                && hex.len() >= 16
                && let Ok(ip) = u64::from_str_radix(&hex[..16], 16)
            {
                let shifted = ip.wrapping_sub(delta);
                let _ = writeln!(out, "  0x{shifted:016X}");
                continue;
            }
            if trimmed.starts_with("MODULE base=0x") {
                let _ = writeln!(
                    out,
                    "MODULE base=0x{:016X} end=0x{:016X}",
                    base.wrapping_sub(delta),
                    end.wrapping_sub(delta)
                );
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    fn shift_ips_only(record: &str, delta: u64) -> String {
        let mut out = String::new();
        for line in record.lines() {
            let trimmed = line.trim();
            if let Some(hex) = trimmed.strip_prefix("0x")
                && hex.len() >= 16
                && let Ok(ip) = u64::from_str_radix(&hex[..16], 16)
            {
                let shifted = ip.wrapping_sub(delta);
                let _ = writeln!(out, "  0x{shifted:016X}");
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    #[test]
    fn handler_record_symbolicates_later_with_slide() {
        let record = capture_record();
        assert!(record.contains("STACK ("), "{record}");
        assert!(record.contains("MODULE base="), "{record}");
        assert!(record.contains("SYMBOLS: deferred"), "{record}");
        assert!(
            !record.contains("SYMBOLS:\n"),
            "handler record must not contain a symbol block: {record}"
        );

        let symbols = symbolicate_saved_record(&record);
        assert!(
            symbols.contains("fatal_crash_symbol_canary"),
            "record:\n{record}\nsymbols:\n{symbols}"
        );

        let delta = 0x4000u64;
        assert!(module_base() > delta);
        let shifted = shift_record(&record, delta);
        let shifted_symbols = symbolicate_saved_record(&shifted);
        assert!(
            shifted_symbols.contains("fatal_crash_symbol_canary"),
            "slide-adjusted record lost the canary:\n{shifted}\n{shifted_symbols}"
        );

        // Subtract the executable base from the IPs only. They fall below the
        // saved span, so the helper must not resolve them as this image.
        let outside = shift_ips_only(&record, module_base());
        let outside_symbols = symbolicate_saved_record(&outside);
        assert!(
            outside_symbols.contains("<other module>"),
            "IPs below the saved span should be left unresolved:\n{outside_symbols}"
        );
        assert!(
            !outside_symbols.contains("fatal_crash_symbol_canary"),
            "an unrelocated IP must not resolve to the canary:\n{outside_symbols}"
        );
    }

    #[test]
    fn append_deferred_symbolication_is_idempotent() {
        let record = capture_record();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("renderer.log");
        std::fs::write(&path, &record).expect("write");

        append_deferred_symbolication(&path).expect("append");
        let once = std::fs::read_to_string(&path).expect("read");
        assert!(
            once.contains("\nSYMBOLS:\n") || once.starts_with("SYMBOLS:\n"),
            "{once}"
        );
        assert!(once.contains("fatal_crash_symbol_canary"), "{once}");

        append_deferred_symbolication(&path).expect("second append");
        let twice = std::fs::read_to_string(&path).expect("read");
        assert_eq!(once, twice, "a second start must not duplicate SYMBOLS");
    }

    #[test]
    fn missing_log_is_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("absent.log");
        append_deferred_symbolication(&path).expect("missing file");
        assert!(!path.exists());
    }
}
