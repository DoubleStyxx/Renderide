//! Process exit codes for `renderide-test`.
//!
//! Expected failures and quarantines are 0. A mixed run uses the highest
//! priority among the failing cases: 5, then 3, then 4, then 6, then 1.
//! Usage errors exit 2 before a suite report exists.

use crate::scene_dsl::suite::{SuiteCaseReport, SuiteFailureKind, SuiteReport};

/// Every case passed. Expected failures and quarantines are allowed.
pub const EXIT_OK: u8 = 0;
/// Image gate: mismatch, region floor, region colour, coverage, channel-zero, or region-mean.
pub const EXIT_IMAGE: u8 = 1;
/// Bad flag or unknown scene. Clap uses this for usage errors.
pub const EXIT_USAGE: u8 = 2;
/// Capture or run failure.
pub const EXIT_CAPTURE: u8 = 3;
/// Golden missing, or bless rejected a flat or coverage-failing capture.
pub const EXIT_GOLDEN: u8 = 4;
/// Renderer missing, spawn failed, or no ICD or adapter.
pub const EXIT_ENVIRONMENT: u8 = 5;
/// Flaky under `--repeat`, or a blessed capture failed against its new golden.
pub const EXIT_UNSTABLE: u8 = 6;

/// Highest priority first. 0 and 2 are not mixed into a suite result.
const PRIORITY: [u8; 5] = [
    EXIT_ENVIRONMENT,
    EXIT_CAPTURE,
    EXIT_GOLDEN,
    EXIT_UNSTABLE,
    EXIT_IMAGE,
];

/// Suite exit code from per-case codes.
pub fn combine(codes: &[u8]) -> u8 {
    for code in PRIORITY {
        if codes.contains(&code) {
            return code;
        }
    }
    EXIT_OK
}

/// This case's exit code. Expected failures and quarantines stay 0.
pub fn case_code(case: &SuiteCaseReport) -> u8 {
    if case.passed
        || crate::scene_dsl::warp_pbs_is_expected(case)
        || crate::scene_dsl::case_is_quarantined_216(case)
    {
        return EXIT_OK;
    }
    code_from_failure(case)
}

/// Writes each case code and the suite code. The suite code is what the process returns.
pub fn stamp(report: &mut SuiteReport) {
    let mut codes = Vec::with_capacity(report.cases.len());
    for case in &mut report.cases {
        let code = case_code(case);
        case.exit_code = code;
        codes.push(code);
    }
    report.exit_code = combine(&codes);
}

fn code_from_failure(case: &SuiteCaseReport) -> u8 {
    let error = case.error.as_deref().unwrap_or("");
    if is_environment(error) {
        return EXIT_ENVIRONMENT;
    }
    if is_capture(error, case.failure_kind) {
        return EXIT_CAPTURE;
    }
    if is_golden(error) {
        return EXIT_GOLDEN;
    }
    if is_unstable(error) {
        return EXIT_UNSTABLE;
    }
    EXIT_IMAGE
}

fn is_environment(error: &str) -> bool {
    error.contains("renderer binary not found")
        || error.contains("spawn renderer")
        || error.contains("no ICD")
        || error.contains("no adapter")
}

fn is_capture(error: &str, kind: Option<SuiteFailureKind>) -> bool {
    matches!(
        kind,
        Some(
            SuiteFailureKind::PipelineNotReady
                | SuiteFailureKind::FlatOrUnstableFrame
                | SuiteFailureKind::PngOutputMissing
        )
    ) || error.contains("pipeline-pending")
        || error.contains("flat or unstable")
        || error.contains("handshake timed out")
        || error.contains("expected fresh PNG")
        || error.contains("timed out")
        || error.contains("renderer exited")
}

fn is_golden(error: &str) -> bool {
    error.contains("golden_flat")
        || error.contains("golden image not found")
        || error.contains("golden missing")
        || error.contains("bless_refused")
}

fn is_unstable(error: &str) -> bool {
    error.contains("unstable") || error.contains("flaky")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL;
    use crate::scene_dsl::suite::SuiteCaseReport;

    fn failed(error: &str) -> SuiteCaseReport {
        let mut case = SuiteCaseReport::plain(
            "case",
            false,
            PathBuf::from("out"),
            PathBuf::from("out/report.json"),
            Some(error.to_string()),
        );
        case.image_compared = true;
        case
    }

    #[test]
    fn each_outcome_maps_to_its_code() {
        assert_eq!(case_code(&failed("case failed tolerance")), EXIT_IMAGE);
        assert_eq!(
            case_code(&failed("case failed tolerance; channel-zero: G is near 0")),
            EXIT_IMAGE
        );
        assert_eq!(
            case_code(&failed(
                "case failed tolerance; region-mean: region column 0"
            )),
            EXIT_IMAGE
        );
        assert_eq!(
            case_code(&failed("headless PNG stayed pipeline-pending for 60s")),
            EXIT_CAPTURE
        );
        let mut flat = failed("flat or unstable frame");
        flat.failure_kind = Some(SuiteFailureKind::FlatOrUnstableFrame);
        assert_eq!(case_code(&flat), EXIT_CAPTURE);
        assert_eq!(
            case_code(&failed("handshake timed out after 15s")),
            EXIT_CAPTURE
        );
        assert_eq!(
            case_code(&failed("golden image not found at x")),
            EXIT_GOLDEN
        );
        assert_eq!(
            case_code(&failed("golden_flat: capture failed coverage")),
            EXIT_GOLDEN
        );
        assert_eq!(
            case_code(&failed("bless_refused: pass --force to overwrite")),
            EXIT_GOLDEN
        );
        assert_eq!(
            case_code(&failed("renderer binary not found at /missing")),
            EXIT_ENVIRONMENT
        );
        assert_eq!(
            case_code(&failed("spawn renderer process: permission denied")),
            EXIT_ENVIRONMENT
        );
        assert_eq!(
            case_code(&failed("no ICD or adapter selected")),
            EXIT_ENVIRONMENT
        );
        assert_eq!(
            case_code(&failed("unstable: new golden did not match")),
            EXIT_UNSTABLE
        );
        assert_eq!(case_code(&failed("flaky: repeat disagreed")), EXIT_UNSTABLE);
    }

    #[test]
    fn expected_failures_and_quarantines_stay_zero() {
        let mut expected = failed("case failed tolerance");
        expected.result = Some(WARP_EXPECTED_FAILURE_LABEL.to_string());
        assert_eq!(case_code(&expected), EXIT_OK);
        let mut quarantined = failed("flat or unstable frame");
        quarantined.status = Some("quarantined".to_string());
        quarantined.reason = Some("quarantined:#216".to_string());
        assert_eq!(case_code(&quarantined), EXIT_OK);
        let mut passed = failed("ignored");
        passed.passed = true;
        passed.error = None;
        assert_eq!(case_code(&passed), EXIT_OK);
    }

    #[test]
    fn mixed_failures_use_priority_order() {
        assert_eq!(combine(&[EXIT_OK, EXIT_OK]), EXIT_OK);
        assert_eq!(combine(&[EXIT_IMAGE, EXIT_OK]), EXIT_IMAGE);
        assert_eq!(combine(&[EXIT_IMAGE, EXIT_UNSTABLE]), EXIT_UNSTABLE);
        assert_eq!(combine(&[EXIT_UNSTABLE, EXIT_GOLDEN]), EXIT_GOLDEN);
        assert_eq!(combine(&[EXIT_GOLDEN, EXIT_CAPTURE]), EXIT_CAPTURE);
        assert_eq!(combine(&[EXIT_CAPTURE, EXIT_ENVIRONMENT]), EXIT_ENVIRONMENT);
        assert_eq!(
            combine(&[
                EXIT_IMAGE,
                EXIT_UNSTABLE,
                EXIT_GOLDEN,
                EXIT_CAPTURE,
                EXIT_ENVIRONMENT
            ]),
            EXIT_ENVIRONMENT
        );
    }

    #[test]
    fn stamp_writes_the_same_code_the_process_returns() {
        let mut report = SuiteReport::from_cases(vec![
            failed("case failed tolerance; channel-zero: G"),
            failed("renderer binary not found at /nope"),
        ]);
        stamp(&mut report);
        assert_eq!(report.cases[0].exit_code, EXIT_IMAGE);
        assert_eq!(report.cases[1].exit_code, EXIT_ENVIRONMENT);
        assert_eq!(report.exit_code, EXIT_ENVIRONMENT);
    }
}
