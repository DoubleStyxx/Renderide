//! Human and JSON output for the harness CLI.
//!
//! This is the only module allowed to write the process stdout.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI print module: stdout and stderr are the user-facing interface"
)]

use crate::scene_dsl::suite::SuiteReport;

/// Where a finished command should write.
pub(super) struct Presentation<'a> {
    /// `--json`: stdout is one document and human lines go to stderr.
    pub json: bool,
    /// `check-suite` deprecation note.
    pub deprecation: bool,
    /// Extra stderr lines (bless skips, one-colour warnings).
    pub notes: &'a [String],
    /// Suite lines. Stdout unless `json` is set.
    pub human: &'a [String],
    /// Suite report printed as the only stdout document when `json` is set.
    pub report: Option<&'a SuiteReport>,
    /// `env:` line. Always stderr, with no prefix, so stdout stays the suite text or one JSON document.
    pub env_line: Option<&'a str>,
}

/// A failure before command dispatch, written to stderr.
pub(super) fn startup_error(message: &str) {
    eprintln!("renderide-test: {message}");
}

/// Writes the command result.
pub(super) fn emit(presentation: &Presentation<'_>) {
    if let Some(line) = presentation.env_line {
        eprintln!("{line}");
    }
    if presentation.deprecation {
        eprintln!("renderide-test: `check-suite` is deprecated; use `run`. Output is unchanged.");
    }
    for note in presentation.notes {
        eprintln!("renderide-test: {note}");
    }
    if presentation.json {
        if let Some(report) = presentation.report {
            match serde_json::to_string(report) {
                Ok(json) => println!("{json}"),
                Err(err) => eprintln!("renderide-test: serialize report: {err}"),
            }
        }
        for line in presentation.human {
            eprintln!("{line}");
        }
        return;
    }
    for line in presentation.human {
        println!("{line}");
    }
}

pub(super) fn suite_lines(
    outcome: &crate::scene_dsl::suite::SuiteRunOutcome,
    result_label: &str,
    print_adapter: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(warning) = outcome.report.quarantine_warning.as_deref() {
        lines.push(crate::scene_dsl::warn_line(warning));
    }
    lines.extend(case_lines(&outcome.report, print_adapter));
    lines.push(format!("Suite report: {}", outcome.report_path.display()));
    lines.push(format_suite_result(&outcome.report, result_label));
    if print_adapter {
        lines.push(check_suite_gate_summary(&outcome.report));
    }
    lines.push(crate::scene_dsl::suite::SUITE_READINESS_SUMMARY.to_string());
    lines
}

/// `list` output. `--json` is one document on stdout.
pub(super) fn emit_list(json: bool, names: &[String]) {
    if json {
        let value = serde_json::json!({ "scenes": names });
        match serde_json::to_string(&value) {
            Ok(text) => println!("{text}"),
            Err(err) => eprintln!("renderide-test: serialize list: {err}"),
        }
        return;
    }
    for name in names {
        println!("{name}");
    }
}

fn case_lines(report: &SuiteReport, print_adapter: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for case in &report.cases {
        if crate::scene_dsl::warp_pbs_is_expected(case) {
            lines.push(format!(
                "EXPECTED FAIL {} ({}) {}",
                case.name,
                case.artifacts_dir.display(),
                crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL
            ));
            push_adapter_line(&mut lines, case, print_adapter);
        } else if crate::scene_dsl::case_is_quarantined_216(case) {
            lines.push(crate::scene_dsl::quarantined_line(
                &case.name,
                &case.artifacts_dir,
                case.reason.as_deref().unwrap_or(""),
            ));
            push_adapter_line(&mut lines, case, print_adapter);
        } else if crate::scene_dsl::warp_pbs_is_unexpected_pass(case) {
            lines.push(crate::scene_dsl::warp_unexpected_pass_line(
                &case.name,
                &case.artifacts_dir,
            ));
            push_adapter_line(&mut lines, case, print_adapter);
        } else {
            let one = [print_case(case)];
            let printed = if print_adapter {
                crate::adapter_gate::format_check_suite(&one)
            } else {
                crate::adapter_gate::format_update_suite(&one)
            };
            lines.extend(printed.case_lines);
        }
    }
    lines
}

fn push_adapter_line(
    lines: &mut Vec<String>,
    case: &crate::scene_dsl::suite::SuiteCaseReport,
    print_adapter: bool,
) {
    if print_adapter && case.gate == crate::adapter_gate::ADAPTER_COMPARISON {
        lines.push(crate::adapter_gate::adapter_line(
            &case.name,
            &case.artifacts_dir,
            &case.adapter,
        ));
    }
}

fn format_suite_result(report: &SuiteReport, result_label: &str) -> String {
    let mut notes = Vec::new();
    if report.expected_failures > 0 {
        notes.push(format!(
            "{} {}",
            report.expected_failures,
            crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL
        ));
    }
    if report.unexpected_passes > 0 {
        notes.push(format!(
            "{} {}",
            report.unexpected_passes,
            crate::scene_dsl::WARP_UNEXPECTED_PASS_LABEL
        ));
    }
    if report.quarantined > 0 {
        notes.push(format!("{} quarantined", report.quarantined));
    }
    let head = format!(
        "Suite result: {}/{} {result_label}",
        report.passed, report.total
    );
    if notes.is_empty() {
        head
    } else {
        format!("{head}, {}", notes.join(", "))
    }
}

fn check_suite_gate_summary(report: &SuiteReport) -> String {
    let passing = report
        .cases
        .iter()
        .filter(|case| case.passed)
        .map(print_case)
        .collect::<Vec<_>>();
    crate::adapter_gate::format_check_suite(&passing).summary
}

fn print_case(
    case: &crate::scene_dsl::suite::SuiteCaseReport,
) -> crate::adapter_gate::PrintCase<'_> {
    crate::adapter_gate::PrintCase {
        name: &case.name,
        passed: case.passed,
        artifacts_dir: &case.artifacts_dir,
        error: case.error.as_deref(),
        gate: if case.gate == crate::adapter_gate::ADAPTER_COMPARISON {
            crate::adapter_gate::Gate::AdapterComparison
        } else {
            crate::adapter_gate::Gate::Behavior
        },
        adapter: &case.adapter,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::scene_dsl::suite::{SuiteCaseReport, SuiteReport};

    fn case(
        name: &str,
        passed: bool,
        result: Option<&str>,
        status: Option<&str>,
        reason: Option<&str>,
    ) -> SuiteCaseReport {
        let mut case =
            SuiteCaseReport::plain(name, passed, PathBuf::from(name), PathBuf::from(name), None);
        case.result = result.map(str::to_string);
        case.status = status.map(str::to_string);
        case.reason = reason.map(str::to_string);
        case
    }

    #[test]
    fn suite_result_line_counts_quarantine_apart_from_expected_failures() {
        use crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL;

        let pass = case("a", true, None, None, None);
        let expected = case("b", false, Some(WARP_EXPECTED_FAILURE_LABEL), None, None);
        let quarantined = case(
            "c",
            false,
            None,
            Some("quarantined"),
            Some("quarantined:#216"),
        );
        let passes = SuiteReport::from_cases(vec![pass.clone()]);
        assert_eq!(
            format_suite_result(&passes, "passed"),
            "Suite result: 1/1 passed"
        );
        let with_expected = SuiteReport::from_cases(vec![pass.clone(), expected.clone()]);
        assert_eq!(
            format_suite_result(&with_expected, "passed"),
            "Suite result: 1/2 passed, 1 expected failure (WARP, #212)"
        );
        let with_quarantine = SuiteReport::from_cases(vec![pass.clone(), quarantined.clone()]);
        assert_eq!(
            format_suite_result(&with_quarantine, "passed"),
            "Suite result: 1/2 passed, 1 quarantined"
        );
        let both = SuiteReport::from_cases(vec![pass.clone(), expected, quarantined]);
        assert_eq!(
            format_suite_result(&both, "passed"),
            "Suite result: 1/3 passed, 1 expected failure (WARP, #212), 1 quarantined"
        );
        assert_eq!(
            check_suite_gate_summary(&with_expected),
            "Suite result: 1 behavior + 0 adapter comparison"
        );
        let mut adapter_case = case("d", true, None, None, None);
        adapter_case.gate = crate::adapter_gate::ADAPTER_COMPARISON.to_string();
        let split = SuiteReport::from_cases(vec![pass, adapter_case]);
        assert_eq!(
            check_suite_gate_summary(&split),
            "Suite result: 1 behavior + 1 adapter comparison"
        );
        let mut unexpected = case(
            "e",
            true,
            Some(crate::scene_dsl::WARP_UNEXPECTED_PASS_LABEL),
            None,
            None,
        );
        unexpected.gate = crate::adapter_gate::ADAPTER_COMPARISON.to_string();
        let with_unexpected = SuiteReport::from_cases(vec![unexpected]);
        assert_eq!(
            format_suite_result(&with_unexpected, "passed"),
            "Suite result: 1/1 passed, 1 unexpected pass (WARP, #212)"
        );
        assert_eq!(with_unexpected.failed, 0);
        assert!(with_unexpected.passed());
    }

    #[test]
    fn adapter_line_prints_on_every_outcome() {
        use crate::scene_dsl::{WARP_EXPECTED_FAILURE_LABEL, WARP_UNEXPECTED_PASS_LABEL};

        fn labeled(
            name: &str,
            passed: bool,
            result: Option<&str>,
            status: Option<&str>,
            reason: Option<&str>,
        ) -> SuiteCaseReport {
            let mut case = SuiteCaseReport::plain(
                name,
                passed,
                PathBuf::from("out").join(name),
                PathBuf::from(name),
                (!passed).then(|| "case failed".to_string()),
            );
            case.gate = crate::adapter_gate::ADAPTER_COMPARISON.to_string();
            case.adapter = "Microsoft Basic Render Driver type=Cpu backend=Dx12".to_string();
            case.result = result.map(str::to_string);
            case.status = status.map(str::to_string);
            case.reason = reason.map(str::to_string);
            case
        }

        let cases = vec![
            labeled("pass_case", true, None, None, None),
            labeled("fail_case", false, None, None, None),
            labeled(
                "expected_case",
                false,
                Some(WARP_EXPECTED_FAILURE_LABEL),
                None,
                None,
            ),
            labeled(
                "quarantine_case",
                false,
                None,
                Some("quarantined"),
                Some("quarantined:#216"),
            ),
            labeled(
                "unexpected_case",
                true,
                Some(WARP_UNEXPECTED_PASS_LABEL),
                None,
                None,
            ),
        ];
        let lines = case_lines(&SuiteReport::from_cases(cases), true);
        for name in [
            "pass_case",
            "fail_case",
            "expected_case",
            "quarantine_case",
            "unexpected_case",
        ] {
            assert!(
                lines.iter().any(|line| {
                    line.starts_with("ADAPTER ")
                        && line.contains(name)
                        && line.contains("Microsoft Basic Render Driver")
                }),
                "{name} missing from {lines:?}"
            );
        }
        assert!(lines.iter().any(|line| line.starts_with("EXPECTED FAIL ")));
        assert!(lines.iter().any(|line| line.starts_with("QUARANTINED ")));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("UNEXPECTED PASS ") && line.contains("#212"))
        );
        assert!(lines.iter().any(|line| line.starts_with("FAIL ")));
        let update = case_lines(
            &SuiteReport::from_cases(vec![labeled("pass_case", true, None, None, None)]),
            false,
        );
        assert!(update.iter().all(|line| !line.contains("ADAPTER")));
    }
}
