//! UTC wall-clock timestamps for log file names (`YYYY-MM-DD_HH-MM-SS`) and per-line prefixes
//! (`HH:MM:SS.mmm`), derived from [`std::time::SystemTime`] without extra dependencies.

use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns a filename-safe UTC timestamp: `YYYY-MM-DD_HH-MM-SS`, used for log file names.
///
/// If [`SystemTime::now`] is before [`UNIX_EPOCH`], returns the literal `unknown`.
pub fn log_filename_timestamp() -> String {
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let secs = dur.as_secs();
    let day_secs = secs % 86400;
    let hour = day_secs / 3600;
    let minute = (day_secs / 60) % 60;
    let second = day_secs % 60;
    let (year, month, day) = days_since_epoch_to_ymd(secs / 86400);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}")
}

/// Appends a line-prefix UTC timestamp `HH:MM:SS.mmm` to `out` without allocating.
///
/// If wall time is before [`UNIX_EPOCH`], appends `"?"`. The underlying [`std::fmt::Write`] impl
/// for [`String`] is infallible, so the write result is intentionally discarded.
pub fn write_line_timestamp(out: &mut String) {
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        out.push('?');
        return;
    };
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    let mins = (secs / 60) % 60;
    let hours = (secs / 3600) % 24;
    let secs = secs % 60;
    let _ = write!(out, "{hours:02}:{mins:02}:{secs:02}.{millis:03}");
}

/// Converts days since Unix epoch (1970-01-01) to `(year, month, day)`.
///
/// Algorithm: <http://howardhinnant.github.io/date_algorithms.html> `civil_from_days`.
///
/// `days` is whole days since 1970-01-01 UTC; the result is calendar `(year, month, day)`.
fn days_since_epoch_to_ymd(days: u64) -> (u32, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe.min(146_096) / 146_096) / 365;
    let y = (yoe as i64 + era * 400) as u32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captures [`write_line_timestamp`] output into a fresh [`String`] for assertions.
    fn line_timestamp() -> String {
        let mut s = String::new();
        write_line_timestamp(&mut s);
        s
    }

    #[test]
    fn days_since_epoch_unix_epoch_is_1970_01_01() {
        assert_eq!(days_since_epoch_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn days_since_epoch_one_day_later() {
        assert_eq!(days_since_epoch_to_ymd(1), (1970, 1, 2));
    }

    #[test]
    fn days_since_epoch_to_ymd_known_dates() {
        for (days, ymd) in [
            (0_u64, (1970_u32, 1_u32, 1_u32)),
            (1, (1970, 1, 2)),
            (31, (1970, 2, 1)),
            (365, (1971, 1, 1)),
            (366, (1971, 1, 2)),
            (11_017, (2000, 3, 1)),
            (20_562, (2026, 4, 19)),
        ] {
            assert_eq!(days_since_epoch_to_ymd(days), ymd, "days={days}");
        }
    }

    #[test]
    fn write_line_timestamp_matches_pattern() {
        let s = line_timestamp();
        assert_eq!(s.len(), 12);
        assert_eq!(s.as_bytes()[2], b':');
        assert_eq!(s.as_bytes()[5], b':');
        assert_eq!(s.as_bytes()[8], b'.');
    }

    #[test]
    fn write_line_timestamp_zero_pads_all_components() {
        let s = line_timestamp();
        for (i, chunk) in [(0usize, 2usize), (3, 2), (6, 2), (9, 3)] {
            let slice = &s[i..i + chunk];
            assert!(
                slice.chars().all(|c| c.is_ascii_digit()),
                "expected digits in {slice:?} from {s:?}"
            );
        }
    }

    #[test]
    fn write_line_timestamp_appends_to_existing_content() {
        let mut s = String::from("prefix-");
        write_line_timestamp(&mut s);
        assert!(s.starts_with("prefix-"));
        assert_eq!(s.len(), "prefix-".len() + 12);
    }

    #[test]
    fn log_filename_timestamp_format() {
        let s = log_filename_timestamp();
        assert_eq!(s.len(), 19);
        assert!(s.contains('_'));
        let (date, time) = s.split_once('_').expect("timestamp contains underscore");
        assert_eq!(date.len(), 10);
        assert_eq!(time.len(), 8);
        assert!(date.chars().filter(|c| *c == '-').count() == 2);
        assert!(time.chars().filter(|c| *c == '-').count() == 2);
    }

    #[test]
    fn days_since_epoch_year_2000_includes_leap_day() {
        assert_eq!(days_since_epoch_to_ymd(11_016), (2000, 2, 29));
        assert_eq!(days_since_epoch_to_ymd(11_017), (2000, 3, 1));
    }

    #[test]
    fn days_since_epoch_year_2024_leap_day_then_march_first() {
        // 2024-02-29 = epoch + 19_782 days; assert the day after is March 1 (leap-year accepted).
        assert_eq!(days_since_epoch_to_ymd(19_782), (2024, 2, 29));
        assert_eq!(days_since_epoch_to_ymd(19_783), (2024, 3, 1));
    }

    #[test]
    fn days_since_epoch_year_2100_century_is_not_leap() {
        // 2100-02-28 = epoch + 47_540 days; the next day must be March 1 (no Feb 29 in 2100).
        assert_eq!(days_since_epoch_to_ymd(47_540), (2100, 2, 28));
        assert_eq!(days_since_epoch_to_ymd(47_541), (2100, 3, 1));
    }
}
