//! Time-limited quarantine for WARP lit captures (#216).
//!
//! `pbs_lit_material_matrix` still matches only a flat frame. The three negative-light
//! cases are quarantined on WARP for any tolerance result, and for that same flat
//! frame, with reason [`WARP_LIT_SHADING_UNTRUSTED`]. A harness error that is not
//! that flat frame stays a failure. After [`QUARANTINE_216_EXPIRES`] a match fails
//! closed. Whoever fixes #216 removes the expiry and this rule in that pull request.
//!
//! <https://github.com/DoubleStyxx/Renderide/issues/216>

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::output::{self, CaseReport};
use super::suite::{SuiteCaseReport, SuiteFailureKind, SuiteReport};

/// UTC expiry of the #216 quarantine.
///
/// Whoever fixes #216 removes this constant and the quarantine rule in that pull request.
///
/// Extended to 2026-11-14T23:59:59Z pending the #216 investigation
/// (https://github.com/DoubleStyxx/Renderide/issues/216); decision on Nov 14
/// based on the root-cause note due Nov 7.
///
/// <https://github.com/DoubleStyxx/Renderide/issues/216>
pub(crate) const QUARANTINE_216_EXPIRES: &str = "2026-11-14T23:59:59Z";

/// `report.json` and suite-report status while the quarantine applies.
pub(crate) const QUARANTINE_STATUS: &str = "quarantined";

/// Reason recorded next to [`QUARANTINE_STATUS`] for the original flat-frame rule.
pub(crate) const QUARANTINE_REASON: &str = "quarantined:#216";

/// Reason for a negative-light case quarantined on WARP.
pub(crate) const WARP_LIT_SHADING_UNTRUSTED: &str = "WARP lit shading untrusted";

/// `pbs_lit_material_matrix`, the original case this quarantine covers.
const PBS_LIT: &str = "pbs_lit_material_matrix";

/// Negative-light cases covered by the same rule, by exact name.
const NEGATIVE_LIGHT_PARITY: &str = "negative_light_parity";
const NEGATIVE_LIGHT_PHYSICALLY_BASED: &str = "negative_light_physically_based";
const NEGATIVE_COLOR_PHYSICALLY_BASED: &str = "negative_color_physically_based";

/// Negative-light cases whose WARP shading is untrusted, by exact name.
const NEGATIVE_LIGHT_CASES: &[&str] = &[
    NEGATIVE_LIGHT_PARITY,
    NEGATIVE_LIGHT_PHYSICALLY_BASED,
    NEGATIVE_COLOR_PHYSICALLY_BASED,
];

/// PNG `tEXt` keyword. Tests write it to prove the quarantine does not read it.
#[cfg(test)]
const PENDING_KEYWORD: &str = "renderide_pipeline_pending";

/// PNG `tEXt` keyword. Tests write it to prove the quarantine does not read it.
#[cfg(test)]
const GPU_DONE_KEYWORD: &str = "renderide_gpu_done";

/// Parsed from [`QUARANTINE_216_EXPIRES`] so a bad constant fails to compile.
const QUARANTINE_216_EXPIRES_UNIX: u64 = parse_rfc3339_unix(QUARANTINE_216_EXPIRES);

/// Warn once the expiry is this close, and not after it has passed.
const WARN_WINDOW: Duration = Duration::from_hours(48);

/// Labels a matching WARP flat frame when `now` is still before the expiry.
///
/// `is_warp` is injected so tests do not read the wall clock or an adapter log.
/// Production passes [`capture_is_warp`], which reads vendor `0x1414` and device
/// `0x008c` from the selected adapter (`0x8c` and `0x008c` are the same id).
pub(crate) fn apply_quarantine(
    report: &mut SuiteReport,
    now: SystemTime,
    is_warp: impl Fn(&Path) -> bool,
) {
    report.quarantine_warning = expiry_warning(now);
    if let Some(warning) = report.quarantine_warning.as_deref() {
        logger::warn!("{warning}");
    }
    let mut changed = false;
    for case in &mut report.cases {
        if label_case(case, now, &is_warp) {
            changed = true;
        }
    }
    if changed {
        report.recompute();
    }
}

/// The selected adapter is WARP. No selected-adapter line is not WARP.
pub(crate) fn capture_is_warp(output_png: &Path) -> bool {
    super::warp_pbs::output_warp_decision(output_png).unwrap_or(false)
}

/// Whether `case` was labeled by this quarantine and did not pass.
pub(crate) fn case_is_quarantined(case: &SuiteCaseReport) -> bool {
    !case.passed
        && case.status.as_deref() == Some(QUARANTINE_STATUS)
        && case.reason.as_deref().is_some_and(is_quarantine_reason)
}

fn is_quarantine_reason(reason: &str) -> bool {
    reason == QUARANTINE_REASON || reason == WARP_LIT_SHADING_UNTRUSTED
}

/// Loud suite line for one quarantined case.
pub(crate) fn quarantined_line(name: &str, artifacts: &Path, reason: &str) -> String {
    format!("QUARANTINED {name} ({}) {reason}", artifacts.display())
}

/// Loud suite line for the 48-hour expiry warning. `warning` is the detail.
pub(crate) fn warn_line(warning: &str) -> String {
    format!("WARN {warning}")
}

fn label_case(
    case: &mut SuiteCaseReport,
    now: SystemTime,
    is_warp: &impl Fn(&Path) -> bool,
) -> bool {
    let png = output::actual_png_in(&case.artifacts_dir);
    if !quarantine_matches(is_warp(&png), case) {
        return false;
    }
    if now >= expires_at() {
        return fail_closed(case);
    }
    let reason = quarantine_reason(case);
    // Stamp first. A failed write leaves the case a hard failure.
    if let Err(err) = stamp_case_report(&case.report_json, reason) {
        logger::warn!(
            "quarantine #216: could not stamp {}: {err}",
            case.report_json.display()
        );
        return fail_closed(case);
    }
    case.passed = false;
    case.status = Some(QUARANTINE_STATUS.to_string());
    case.reason = Some(reason.to_string());
    true
}

/// `pbs_lit` matches only the flat frame. A negative-light case also matches a finished comparison.
///
/// `None` pending or dropped draws is not zero. A harness error other than that flat frame
/// does not match. The expiry is applied by the caller.
fn quarantine_matches(is_warp: bool, case: &SuiteCaseReport) -> bool {
    if !is_warp {
        return false;
    }
    if case.name == PBS_LIT {
        return flat_frame_signature(case);
    }
    NEGATIVE_LIGHT_CASES.contains(&case.name.as_str())
        && (flat_frame_signature(case) || case.image_compared)
}

fn flat_frame_signature(case: &SuiteCaseReport) -> bool {
    case.failure_kind == Some(SuiteFailureKind::FlatOrUnstableFrame)
        && case.frame_pending == Some(0)
        && case.dropped_draws == Some(0)
        && case.gpu_done == Some(true)
}

fn quarantine_reason(case: &SuiteCaseReport) -> &'static str {
    if NEGATIVE_LIGHT_CASES.contains(&case.name.as_str()) {
        WARP_LIT_SHADING_UNTRUSTED
    } else {
        QUARANTINE_REASON
    }
}

/// After expiry, or when the report cannot be stamped, a match is a hard failure.
fn fail_closed(case: &mut SuiteCaseReport) -> bool {
    if !case.passed {
        return false;
    }
    case.passed = false;
    true
}

fn expiry_warning(now: SystemTime) -> Option<String> {
    if now >= expires_at() {
        return None;
    }
    let Ok(remaining) = expires_at().duration_since(now) else {
        return None;
    };
    if remaining > WARN_WINDOW {
        return None;
    }
    Some(format!(
        "quarantine #216 expires at {QUARANTINE_216_EXPIRES}; whoever fixes #216 removes QUARANTINE_216_EXPIRES and this rule in that PR (https://github.com/DoubleStyxx/Renderide/issues/216)"
    ))
}

fn expires_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(QUARANTINE_216_EXPIRES_UNIX)
}

/// Whether a WARP capture of this case may end the flat-frame wait early.
///
/// The four cases are the ones [`quarantine_matches`] can label. After
/// [`expires_at`] the capture keeps the full WARP budget and fails closed.
pub(crate) fn early_flat_exit_applies(case_name: &str, is_warp: bool, now: SystemTime) -> bool {
    if !is_warp || now >= expires_at() {
        return false;
    }
    case_name == PBS_LIT || NEGATIVE_LIGHT_CASES.contains(&case_name)
}

fn stamp_case_report(path: &Path, reason: &str) -> std::io::Result<()> {
    let bytes = fs::read(path)?;
    let mut report: CaseReport = serde_json::from_slice(&bytes)
        .map_err(|err| std::io::Error::other(format!("parse report: {err}")))?;
    report.passed = false;
    report.status = Some(QUARANTINE_STATUS.to_string());
    report.reason = Some(reason.to_string());
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| std::io::Error::other(format!("serialize report: {err}")))?;
    fs::write(path, json)
}

const fn parse_rfc3339_unix(text: &str) -> u64 {
    let bytes = text.as_bytes();
    assert!(bytes.len() == 20);
    assert!(bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T');
    assert!(bytes[13] == b':' && bytes[16] == b':' && bytes[19] == b'Z');
    let year = digits(bytes, 0, 4);
    let month = digits(bytes, 5, 2);
    let day = digits(bytes, 8, 2);
    let hour = digits(bytes, 11, 2);
    let minute = digits(bytes, 14, 2);
    let second = digits(bytes, 17, 2);
    assert!(month >= 1 && month <= 12 && day >= 1 && day <= 31);
    assert!(hour <= 23 && minute <= 59 && second <= 59);
    let days = days_from_civil(year as i64, month, day);
    assert!(days >= 0);
    (days as u64) * 86_400 + hour * 3_600 + minute * 60 + second
}

const fn digits(bytes: &[u8], index: usize, width: usize) -> u64 {
    let mut value = 0u64;
    let mut offset = 0;
    while offset < width {
        value = value * 10 + digit(bytes, index + offset);
        offset += 1;
    }
    value
}

const fn digit(bytes: &[u8], index: usize) -> u64 {
    let byte = bytes[index];
    assert!(byte.is_ascii_digit());
    (byte - b'0') as u64
}

/// Days since 1970-01-01. Howard Hinnant's civil-from-days inverse.
const fn days_from_civil(mut year: i64, month: u64, day: u64) -> i64 {
    if month <= 2 {
        year -= 1;
    }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u64;
    let month_shifted = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * month_shifted + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

#[cfg(test)]
mod tests;
