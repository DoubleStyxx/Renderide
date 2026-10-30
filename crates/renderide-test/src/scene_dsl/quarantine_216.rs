//! Time-limited quarantine for the WARP flat frame (#216).
//!
//! The case still runs. A match is recorded as `quarantined` / `quarantined:#216`,
//! apart from passes and from the #212 expected failure. Whoever fixes #216 removes
//! [`QUARANTINE_216_EXPIRES`] and this rule in that pull request.
//!
//! <https://github.com/DoubleStyxx/Renderide/issues/216>

use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::output::{self, CaseReport};
use super::suite::{SuiteCaseReport, SuiteReport};

/// UTC expiry of the #216 quarantine.
///
/// Whoever fixes #216 removes this constant and the quarantine rule in that pull request.
///
/// <https://github.com/DoubleStyxx/Renderide/issues/216>
pub(crate) const QUARANTINE_216_EXPIRES: &str = "2026-10-31T23:59:59Z";

/// `report.json` and suite-report status while the quarantine applies.
pub(crate) const QUARANTINE_STATUS: &str = "quarantined";

/// Reason recorded next to [`QUARANTINE_STATUS`].
pub(crate) const QUARANTINE_REASON: &str = "quarantined:#216";

/// Case the quarantine is allowed to cover.
const PBS_LIT: &str = "pbs_lit_material_matrix";

/// PNG `tEXt` keyword. `0` means pending=0 and no dropped draws.
const PENDING_KEYWORD: &str = "renderide_pipeline_pending";

/// PNG `tEXt` keyword. Exactly `1` means that frame's submission finished.
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
    let mut labeled = false;
    for case in &mut report.cases {
        if !label_case(case, now, &is_warp) {
            continue;
        }
        labeled = true;
    }
    if labeled {
        report.recompute();
    }
}

/// Applies the quarantine using the wall clock and the selected adapter.
pub(crate) fn apply_quarantine_now(report: &mut SuiteReport) {
    apply_quarantine(report, SystemTime::now(), capture_is_warp);
}

/// The selected adapter is WARP. No selected-adapter line is not WARP.
fn capture_is_warp(output_png: &Path) -> bool {
    super::warp_pbs::output_warp_decision(output_png).unwrap_or(false)
}

/// Whether `case` was labeled by this quarantine and did not pass.
pub(crate) fn case_is_quarantined(case: &SuiteCaseReport) -> bool {
    !case.passed
        && case.status.as_deref() == Some(QUARANTINE_STATUS)
        && case.reason.as_deref() == Some(QUARANTINE_REASON)
}

/// Loud suite line for one quarantined case.
pub(crate) fn quarantined_line(name: &str, artifacts: &Path) -> String {
    format!(
        "QUARANTINED {name} ({}) {QUARANTINE_REASON}",
        artifacts.display()
    )
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
    let texts = png_text_chunks(&png);
    if !quarantine_matches(
        is_warp(&png),
        &case.name,
        case.error.as_deref(),
        text_value(&texts, PENDING_KEYWORD),
        text_value(&texts, GPU_DONE_KEYWORD),
        now,
    ) {
        return false;
    }
    case.passed = false;
    case.status = Some(QUARANTINE_STATUS.to_string());
    case.reason = Some(QUARANTINE_REASON.to_string());
    if let Err(err) = stamp_case_report(&case.report_json) {
        logger::warn!(
            "quarantine #216: could not stamp {}: {err}",
            case.report_json.display()
        );
    }
    true
}

fn quarantine_matches(
    is_warp: bool,
    name: &str,
    error: Option<&str>,
    pending_chunk: Option<&str>,
    gpu_done_chunk: Option<&str>,
    now: SystemTime,
) -> bool {
    if now >= expires_at() || !is_warp || name != PBS_LIT || !is_flat_or_unstable_frame(error) {
        return false;
    }
    let (pending, dropped_draws) = counts_from_pending_chunk(pending_chunk);
    let gpu_done = gpu_done_chunk.is_some_and(|text| text.trim() == "1");
    pending == Some(0) && dropped_draws == Some(0) && gpu_done
}

/// `FlatOrUnstableFrame` only. A flat golden-check image and a pipeline timeout are not it.
fn is_flat_or_unstable_frame(error: Option<&str>) -> bool {
    let Some(error) = error else {
        return false;
    };
    if error.contains("pipeline-pending") || error.contains("PipelineNotReady") {
        return false;
    }
    error.contains("flat or unstable")
}

/// Only the chunk text `0` is pending=0 and zero dropped draws. Anything else is not.
fn counts_from_pending_chunk(chunk: Option<&str>) -> (Option<u64>, Option<u64>) {
    match chunk.map(str::trim) {
        Some("0") => (Some(0), Some(0)),
        Some(text) => match text.parse::<u64>() {
            Ok(pending) => (Some(pending), None),
            Err(_) => (None, None),
        },
        None => (None, None),
    }
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

fn stamp_case_report(path: &Path) -> std::io::Result<()> {
    let bytes = fs::read(path)?;
    let mut report: CaseReport = serde_json::from_slice(&bytes)
        .map_err(|err| std::io::Error::other(format!("parse report: {err}")))?;
    report.passed = false;
    report.status = Some(QUARANTINE_STATUS.to_string());
    report.reason = Some(QUARANTINE_REASON.to_string());
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| std::io::Error::other(format!("serialize report: {err}")))?;
    fs::write(path, json)
}

fn png_text_chunks(path: &Path) -> Vec<(String, String)> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let Ok(reader) = decoder.read_info() else {
        return Vec::new();
    };
    reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .map(|chunk| (chunk.keyword.clone(), chunk.text.clone()))
        .collect()
}

fn text_value<'a>(texts: &'a [(String, String)], keyword: &str) -> Option<&'a str> {
    texts
        .iter()
        .find(|(key, _)| key == keyword)
        .map(|(_, text)| text.as_str())
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
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::*;
    use crate::HarnessError;

    fn before_expiry() -> SystemTime {
        expires_at()
            .checked_sub(Duration::from_hours(168))
            .expect("before expiry")
    }

    fn after_expiry() -> SystemTime {
        expires_at() + Duration::from_secs(1)
    }

    fn within_warning_window() -> SystemTime {
        expires_at()
            .checked_sub(Duration::from_hours(1))
            .expect("inside 48h")
    }

    fn outside_warning_window() -> SystemTime {
        expires_at()
            .checked_sub(WARN_WINDOW + Duration::from_secs(1))
            .expect("before the warning window")
    }

    fn flat_message() -> String {
        HarnessError::FlatOrUnstableFrame {
            path: PathBuf::from("actual.png"),
            wait: Duration::from_secs(300),
            last_pending: 0,
            ready_chunks: 1,
            pending_chunks: 0,
            missing_chunks: 0,
            frame_count: 2,
        }
        .to_string()
    }

    fn write_frame_png(path: &Path, pending: &str, gpu_done: &str) {
        let file = fs::File::create(path).expect("create png");
        let mut encoder = png::Encoder::new(file, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .add_text_chunk(PENDING_KEYWORD.to_string(), pending.to_string())
            .expect("pending chunk");
        encoder
            .add_text_chunk(GPU_DONE_KEYWORD.to_string(), gpu_done.to_string())
            .expect("gpu_done chunk");
        let mut writer = encoder.write_header().expect("png header");
        writer
            .write_image_data(&[0, 0, 0, 255])
            .expect("png pixels");
        writer.finish().expect("png finish");
    }

    fn suite_case(
        dir: &Path,
        name: &str,
        error: Option<&str>,
        pending: &str,
        gpu_done: &str,
    ) -> SuiteCaseReport {
        let artifacts = dir.join(name);
        fs::create_dir_all(&artifacts).expect("artifacts");
        write_frame_png(&artifacts.join("actual.png"), pending, gpu_done);
        let report_json = artifacts.join("report.json");
        let integration = crate::scene_dsl::cases::unlit_sphere();
        let mut case_report = output::report_from_error(&integration, error.unwrap_or("passed"));
        case_report.name = name.to_string();
        case_report.passed = error.is_none();
        case_report.error = error.map(str::to_string);
        fs::write(
            &report_json,
            serde_json::to_string_pretty(&case_report).expect("report json"),
        )
        .expect("write report");
        SuiteCaseReport {
            name: name.to_string(),
            passed: error.is_none(),
            artifacts_dir: artifacts,
            report_json,
            error: error.map(str::to_string),
            result: None,
            status: None,
            reason: None,
        }
    }

    fn apply(report: &mut SuiteReport, now: SystemTime, warp: bool) {
        apply_quarantine(report, now, move |_| warp);
    }

    #[test]
    fn expiry_constant_is_the_utc_instant_named_in_the_ruling() {
        assert_eq!(QUARANTINE_216_EXPIRES, "2026-10-31T23:59:59Z");
        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_199);
        assert_eq!(expires_at(), expected);
        assert_eq!(QUARANTINE_STATUS, "quarantined");
        assert_eq!(QUARANTINE_REASON, "quarantined:#216");
    }

    #[test]
    fn warp_flat_frame_is_quarantined_before_expiry_and_fails_after() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![
            suite_case(dir.path(), "unlit_sphere", None, "0", "1"),
            suite_case(dir.path(), PBS_LIT, Some(&flat_message()), "0", "1"),
        ]);
        apply(&mut report, before_expiry(), true);

        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(report.expected_failures, 0);
        assert_eq!(report.quarantined, 1);
        assert!(report.passed());
        assert!(report.quarantine_warning.is_none());
        let case = &report.cases[1];
        assert!(!case.passed);
        assert_eq!(case.status.as_deref(), Some(QUARANTINE_STATUS));
        assert_eq!(case.reason.as_deref(), Some(QUARANTINE_REASON));
        assert!(case_is_quarantined(case));

        let suite_json = serde_json::to_value(&report).expect("suite json");
        assert_eq!(suite_json["quarantined"], 1);
        assert!(suite_json.get("quarantine_warning").is_none());
        assert_eq!(suite_json["cases"][1]["status"], QUARANTINE_STATUS);
        assert_eq!(suite_json["cases"][1]["reason"], QUARANTINE_REASON);
        let case_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&case.report_json).expect("read report"))
                .expect("case json");
        assert_eq!(case_json["status"], QUARANTINE_STATUS);
        assert_eq!(case_json["reason"], QUARANTINE_REASON);
        assert_eq!(case_json["passed"], false);
        assert!(case_json.get("result").is_none());
    }

    #[test]
    fn warp_flat_frame_fails_after_expiry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![
            suite_case(dir.path(), "unlit_sphere", None, "0", "1"),
            suite_case(dir.path(), PBS_LIT, Some(&flat_message()), "0", "1"),
        ]);
        apply(&mut report, after_expiry(), true);
        assert_eq!(report.passed, 1);
        assert_eq!(report.quarantined, 0);
        assert_eq!(report.failed, 1);
        assert!(!report.passed());
        assert!(report.cases[1].status.is_none());
        assert!(report.cases[1].reason.is_none());
        assert!(report.quarantine_warning.is_none());
        let expired_json: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&report.cases[1].report_json).expect("read report"),
        )
        .expect("expired json");
        assert!(expired_json.get("status").is_none());
    }

    #[test]
    fn non_warp_flat_frame_stays_a_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut report, before_expiry(), false);
        assert_eq!(report.quarantined, 0);
        assert_eq!(report.failed, 1);
        assert!(!report.passed());
        assert!(report.cases[0].status.is_none());
    }

    #[test]
    fn warp_flat_frame_with_pending_above_zero_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "4",
            "1",
        )]);
        apply(&mut report, before_expiry(), true);
        assert_eq!(report.failed, 1);
        assert_eq!(report.quarantined, 0);
        assert!(!report.passed());
    }

    #[test]
    fn warp_flat_frame_in_another_scene_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            "unlit_sphere",
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut report, before_expiry(), true);
        assert_eq!(report.failed, 1);
        assert_eq!(report.quarantined, 0);
        assert!(report.cases[0].reason.is_none());
    }

    #[test]
    fn warp_flat_frame_without_gpu_done_or_a_zero_chunk_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut not_done = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "0",
            "0",
        )]);
        apply(&mut not_done, before_expiry(), true);
        assert_eq!(not_done.failed, 1);
        assert_eq!(not_done.quarantined, 0);

        let mut missing = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        let png = output::actual_png_in(&missing.cases[0].artifacts_dir);
        let file = fs::File::create(&png).expect("bare png");
        let mut encoder = png::Encoder::new(file, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&[0, 0, 0, 255]).expect("pixels");
        writer.finish().expect("finish");
        apply(&mut missing, before_expiry(), true);
        assert_eq!(missing.failed, 1, "a missing pending chunk is not zero");
        assert_eq!(missing.quarantined, 0);
    }

    #[test]
    fn other_failures_stay_hard_failures() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pipeline = HarnessError::PipelineNotReady {
            path: PathBuf::from("actual.png"),
            wait: Duration::from_secs(180),
        }
        .to_string();
        let crash = HarnessError::SpawnRenderer(std::io::Error::other("spawn")).to_string();
        let flat_image = HarnessError::FlatImage {
            path: PathBuf::from("actual.png"),
            color: [0, 0, 0, 255],
        }
        .to_string();
        assert!(pipeline.contains("pipeline-pending") || pipeline.contains("PipelineNotReady"));
        assert!(flat_image.contains("flat single color"));
        assert!(!flat_image.contains("flat or unstable"));
        for error in [pipeline, crash, flat_image] {
            let mut report = SuiteReport::from_cases(vec![suite_case(
                dir.path(),
                PBS_LIT,
                Some(&error),
                "0",
                "1",
            )]);
            apply(&mut report, before_expiry(), true);
            assert_eq!(report.quarantined, 0, "{error}");
            assert_eq!(report.failed, 1, "{error}");
        }
    }

    #[test]
    fn quarantine_warns_only_inside_the_last_48_hours() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut early = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut early, outside_warning_window(), true);
        assert_eq!(early.quarantined, 1);
        assert!(early.quarantine_warning.is_none());

        let mut close = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut close, within_warning_window(), true);
        let warning = close.quarantine_warning.expect("warning");
        assert!(warning.contains("#216"), "{warning}");
        assert!(warning.contains(QUARANTINE_216_EXPIRES), "{warning}");
        assert!(
            warning.contains("https://github.com/DoubleStyxx/Renderide/issues/216"),
            "{warning}"
        );
        assert!(warn_line(&warning).starts_with("WARN "));
        assert_eq!(close.quarantined, 1);

        let mut expired = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut expired, expires_at(), true);
        assert_eq!(expired.quarantined, 0);
        assert!(expired.quarantine_warning.is_none());
    }

    #[test]
    fn expected_failure_label_is_left_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut case = suite_case(dir.path(), PBS_LIT, Some("case failed tolerance"), "0", "1");
        case.passed = false;
        case.result = Some(super::super::warp_pbs::EXPECTED_FAILURE_LABEL.to_string());
        let mut report = SuiteReport::from_cases(vec![case]);
        report.cases[0].passed = false;
        report.cases[0].result = Some(super::super::warp_pbs::EXPECTED_FAILURE_LABEL.to_string());
        report.recompute();
        apply(&mut report, before_expiry(), true);
        assert!(super::super::warp_pbs::case_is_expected_failure(
            &report.cases[0]
        ));
        assert!(!case_is_quarantined(&report.cases[0]));
        assert_eq!(report.expected_failures, 1);
        assert_eq!(report.quarantined, 0);
        assert_eq!(report.failed, 0);
        assert!(report.passed());
    }

    #[test]
    fn quarantined_line_names_the_case_and_the_reason() {
        let line = quarantined_line(PBS_LIT, Path::new("out/pbs_lit_material_matrix"));
        assert_eq!(
            line,
            "QUARANTINED pbs_lit_material_matrix (out/pbs_lit_material_matrix) quarantined:#216"
        );
    }
}
