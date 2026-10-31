//! Parallel runner for the registered headless scene suite.
//!
//! The suite command is the intended entry point for GPU-backed golden-image validation. It
//! executes each selected case through [`super::runner::run_integration_case`], writes every
//! existing per-case artifact, and emits a suite-level JSON report for CI artifact collection.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::HarnessError;
use crate::image_io::load_rgba;

use super::cases::{IntegrationCase, lookup, registry};
use super::host_info;
use super::output::{CaseOutputLayout, report_from_evaluation, write_report};
use super::runner::{CaseRunOutcome, RunnerConfig, run_integration_case};

/// Suite-level report filename written under the configured output root.
pub const SUITE_REPORT_FILENAME: &str = "suite-report.json";

/// Suite summary. WARP stalls stay bounded (#214); the black frame is the clear (#216).
pub const SUITE_READINESS_SUMMARY: &str = "Each capture waits until that frame's GPU submission is done. WARP submission stalls stay bounded (#214); a black pbs_lit frame is the clear color until that submission completes (#216).";

fn default_summary() -> String {
    SUITE_READINESS_SUMMARY.to_string()
}

/// Suite cases assigned to one harness worker chunk.
const SUITE_PARALLEL_CHUNK_CASES: usize = 1;
/// Suite case count required before the harness fans out.
const SUITE_PARALLEL_MIN_CASES: usize = SUITE_PARALLEL_CHUNK_CASES * 2;

/// Inputs for [`run_suite`].
#[derive(Clone, Debug)]
pub struct SuiteConfig {
    /// Cases to execute. Use [`select_cases`] to resolve CLI case names.
    pub cases: Vec<IntegrationCase>,
    /// Common runner configuration shared by every case.
    pub runner: RunnerConfig,
    /// Maximum number of cases to run concurrently. Values below one are clamped to one.
    pub jobs: usize,
}

/// Outcome of a completed suite run.
#[derive(Clone, Debug)]
pub struct SuiteRunOutcome {
    /// Aggregated suite report.
    pub report: SuiteReport,
    /// Path to the written suite-level report.
    pub report_path: PathBuf,
}

impl SuiteRunOutcome {
    /// Returns whether every executed case passed.
    pub fn passed(&self) -> bool {
        self.report.passed()
    }
}

/// Suite-level JSON report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuiteReport {
    /// Number of cases executed.
    pub total: usize,
    /// Number of passing cases.
    pub passed: usize,
    /// Number of failing cases. An expected WARP failure or a #216 quarantine is not counted here.
    pub failed: usize,
    /// Cases reported as `expected failure (WARP, #212)`. These are not passes.
    #[serde(default)]
    pub expected_failures: usize,
    /// WARP passes of `pbs_lit_material_matrix`. Counted here and still inside `passed`.
    #[serde(default)]
    pub unexpected_passes: usize,
    /// Cases recorded as `quarantined` with reason `quarantined:#216`.
    /// Not a pass and not an expected failure.
    #[serde(default)]
    pub quarantined: usize,
    /// Per-case results, in registry or CLI selection order.
    pub cases: Vec<SuiteCaseReport>,
    /// Points at the bounded WARP stall (#214) and the black clear captured during it (#216).
    #[serde(default = "default_summary")]
    pub summary: String,
    /// Set inside the last 48 hours before the #216 quarantine expires.
    ///
    /// Written to `suite-report.json` when present. Absent after expiry and
    /// while more than 48 hours remain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine_warning: Option<String>,
}

impl SuiteReport {
    /// Builds an aggregate report from per-case results.
    pub fn from_cases(cases: Vec<SuiteCaseReport>) -> Self {
        let mut report = Self {
            total: 0,
            passed: 0,
            failed: 0,
            expected_failures: 0,
            unexpected_passes: 0,
            quarantined: 0,
            cases,
            summary: SUITE_READINESS_SUMMARY.to_string(),
            quarantine_warning: None,
        };
        report.recompute();
        report
    }

    /// Recounts pass, failure, expected-failure, and quarantine totals from `cases`.
    pub fn recompute(&mut self) {
        self.total = self.cases.len();
        self.passed = self.cases.iter().filter(|case| case.passed).count();
        self.expected_failures = self
            .cases
            .iter()
            .filter(|case| super::warp_pbs::case_is_expected_failure(case))
            .count();
        self.unexpected_passes = self
            .cases
            .iter()
            .filter(|case| super::warp_pbs::case_is_unexpected_pass(case))
            .count();
        self.quarantined = self
            .cases
            .iter()
            .filter(|case| {
                super::quarantine_216::case_is_quarantined(case)
                    && !super::warp_pbs::case_is_expected_failure(case)
            })
            .count();
        self.failed = self.total.saturating_sub(
            self.passed
                .saturating_add(self.expected_failures)
                .saturating_add(self.quarantined),
        );
    }

    /// Returns whether every case passed, was an expected WARP failure, or was quarantined.
    ///
    /// An expected failure and a quarantine keep `passed == false` on that case.
    /// Neither fails the suite.
    pub const fn passed(&self) -> bool {
        self.failed == 0
    }
}

/// Why a harness timeout failed, when the suite recorded one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuiteFailureKind {
    /// Timed out on a flat or unstable frame.
    FlatOrUnstableFrame,
    /// Timed out while material draws were still compiling or skipped.
    PipelineNotReady,
    /// No stable PNG arrived before the deadline.
    PngOutputMissing,
}

/// Per-case suite report entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuiteCaseReport {
    /// Case identifier.
    pub name: String,
    /// Whether the case passed.
    pub passed: bool,
    /// Per-case artifact directory.
    pub artifacts_dir: PathBuf,
    /// Per-case JSON report path.
    pub report_json: PathBuf,
    /// Error summary when the case failed before or during comparison.
    pub error: Option<String>,
    /// `expected failure (WARP, #212)` when that case is the known WARP mismatch.
    /// `unexpected pass (WARP, #212)` when WARP passes `pbs_lit`. Absent on a
    /// hard failure and on an ordinary pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// `quarantined` when the #216 rule covers this case. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// `quarantined:#216` for the original flat-frame rule.
    /// `WARP lit shading untrusted` for a negative-light case on WARP.
    /// Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Timed-out harness failure, when this case has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<SuiteFailureKind>,
    /// Pending count on the frame the harness timed out on.
    ///
    /// `None` means that frame did not record a count. It is not zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_pending: Option<u64>,
    /// GPU-done flag from the frame the harness timed out on.
    ///
    /// `None` means no frame was observed. It is not `false` and it is not done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_done: Option<bool>,
    /// Dropped draws on the frame the harness timed out on. `None` is not zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped_draws: Option<u64>,
    /// The case finished an image comparison.
    ///
    /// A harness error leaves this false. The negative-light WARP rule reads it
    /// so a tolerance result is quarantined and a spawn failure is not.
    #[serde(default)]
    pub image_compared: bool,
    /// `behavior` or `adapter_comparison`.
    #[serde(default = "default_behavior_gate")]
    pub gate: String,
    /// Selected adapter label. Empty until the renderer log is read.
    #[serde(default)]
    pub adapter: String,
}

fn default_behavior_gate() -> String {
    crate::adapter_gate::BEHAVIOR.to_string()
}

impl SuiteCaseReport {
    /// A finished case with no quarantine label and no timed-out frame facts.
    pub(crate) fn plain(
        name: impl Into<String>,
        passed: bool,
        artifacts_dir: PathBuf,
        report_json: PathBuf,
        error: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            passed,
            artifacts_dir,
            report_json,
            error,
            result: None,
            status: None,
            reason: None,
            failure_kind: None,
            frame_pending: None,
            gpu_done: None,
            dropped_draws: None,
            gate: default_behavior_gate(),
            adapter: String::new(),
            image_compared: false,
        }
    }

    /// Copies the timeout kind and the timed-out frame from `err`.
    ///
    /// Pending and GPU-done come from that error, not from a later read of `actual.png`.
    pub(crate) fn from_harness_error(
        name: impl Into<String>,
        artifacts_dir: PathBuf,
        report_json: PathBuf,
        err: &HarnessError,
    ) -> Self {
        let mut case = Self::plain(
            name,
            false,
            artifacts_dir,
            report_json,
            Some(err.to_string()),
        );
        match err {
            HarnessError::FlatOrUnstableFrame {
                frame_pending,
                gpu_done,
                dropped_draws,
                ..
            } => {
                case.failure_kind = Some(SuiteFailureKind::FlatOrUnstableFrame);
                case.frame_pending = *frame_pending;
                case.gpu_done = Some(*gpu_done);
                case.dropped_draws = *dropped_draws;
            }
            HarnessError::PipelineNotReady { .. } => {
                case.failure_kind = Some(SuiteFailureKind::PipelineNotReady);
            }
            HarnessError::PngOutputMissing { .. } => {
                case.failure_kind = Some(SuiteFailureKind::PngOutputMissing);
            }
            _ => {}
        }
        case
    }
}

/// Resolves requested case names against the registry.
///
/// When `requested` is empty, every registered case is returned. Duplicate requested names are
/// ignored after their first occurrence so parallel execution never races on one output folder.
pub fn select_cases(requested: &[String]) -> Result<Vec<IntegrationCase>, HarnessError> {
    select_cases_in(requested, None)
}

/// [`select_cases`] plus an opt-in group.
///
/// `None` is the default registry, and a debug case name still resolves. `Some(group)`
/// limits the pool to that group, including when `requested` is empty.
pub fn select_cases_in(
    requested: &[String],
    group: Option<&str>,
) -> Result<Vec<IntegrationCase>, HarnessError> {
    let grouped = match group {
        None => None,
        Some(name) if name == super::warp_216_debug::GROUP => Some(super::warp_216_debug::cases()),
        Some(name) => return Err(HarnessError::UnknownGroup(name.to_string())),
    };
    if requested.is_empty() {
        return Ok(grouped.unwrap_or_else(registry));
    }

    let mut selected = Vec::new();
    for name in requested {
        if selected
            .iter()
            .any(|case: &IntegrationCase| case.name == *name)
        {
            continue;
        }
        let found = if let Some(pool) = &grouped {
            pool.iter().find(|case| case.name == *name).cloned()
        } else {
            lookup(name).or_else(|| super::warp_216_debug::lookup(name))
        };
        let Some(case) = found else {
            return Err(HarnessError::UnknownCase(name.clone()));
        };
        selected.push(case);
    }
    Ok(selected)
}

/// Runs every configured case in parallel and writes the suite-level report.
pub fn run_suite(config: SuiteConfig) -> Result<SuiteRunOutcome, HarnessError> {
    run_suite_with(config, run_case_for_suite, true)
}

/// Runs every configured case and promotes each non-flat capture into its committed golden.
pub fn update_suite(config: SuiteConfig) -> Result<SuiteRunOutcome, HarnessError> {
    run_suite_with(config, run_case_for_update, false)
}

fn run_suite_with(
    config: SuiteConfig,
    run_case: fn(&IntegrationCase, &RunnerConfig) -> SuiteCaseReport,
    check_suite: bool,
) -> Result<SuiteRunOutcome, HarnessError> {
    host_info::log_suite_host();
    std::fs::create_dir_all(&config.runner.output_root)?;
    let jobs = config.jobs.max(1).min(config.cases.len().max(1));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .map_err(|e| HarnessError::QueueOptions(format!("build suite thread pool: {e}")))?;

    let runner = &config.runner;
    let case_reports =
        if jobs >= SUITE_PARALLEL_MIN_CASES && config.cases.len() >= SUITE_PARALLEL_MIN_CASES {
            pool.install(|| {
                config
                    .cases
                    .par_iter()
                    .with_min_len(SUITE_PARALLEL_CHUNK_CASES)
                    .map(|case| run_case(case, runner))
                    .collect::<Vec<_>>()
            })
        } else {
            config
                .cases
                .iter()
                .map(|case| run_case(case, runner))
                .collect::<Vec<_>>()
        };

    let mut report = SuiteReport::from_cases(case_reports);
    apply_suite_labels(
        &mut report,
        check_suite,
        SystemTime::now(),
        super::quarantine_216::capture_is_warp,
    );
    let report_path = write_suite_report(&config.runner.output_root, &report)?;
    Ok(SuiteRunOutcome {
        report,
        report_path,
    })
}

/// The #212 label applies to both commands. The #216 quarantine is check-suite only,
/// so a failed `update-suite` cannot exit 0 by counting a flat frame as quarantined.
fn apply_suite_labels(
    report: &mut SuiteReport,
    check_suite: bool,
    now: SystemTime,
    is_warp: impl Fn(&Path) -> bool,
) {
    super::warp_pbs::apply_expected_failure(report);
    if check_suite {
        super::quarantine_216::apply_quarantine(report, now, is_warp);
    }
}

fn run_case_for_suite(case: &IntegrationCase, runner: &RunnerConfig) -> SuiteCaseReport {
    let fallback_layout = CaseOutputLayout::for_case(&runner.output_root, &case.name);
    match run_integration_case(case, runner) {
        Ok(outcome) => report_from_outcome(outcome),
        Err(err) => SuiteCaseReport::from_harness_error(
            case.name.clone(),
            fallback_layout.root,
            fallback_layout.report_json,
            &err,
        ),
    }
}

fn run_case_for_update(case: &IntegrationCase, runner: &RunnerConfig) -> SuiteCaseReport {
    let fallback_layout = CaseOutputLayout::for_case(&runner.output_root, &case.name);
    match run_integration_case(case, runner) {
        Ok(outcome) => report_from_update_outcome(case, outcome),
        Err(err) => SuiteCaseReport::from_harness_error(
            case.name.clone(),
            fallback_layout.root,
            fallback_layout.report_json,
            &err,
        ),
    }
}

fn report_from_update_outcome(case: &IntegrationCase, outcome: CaseRunOutcome) -> SuiteCaseReport {
    match promote_case_golden(case, &outcome) {
        Ok(report) => report,
        Err(err) => {
            let mut report = SuiteCaseReport::from_harness_error(
                case.name.clone(),
                outcome.layout.root,
                outcome.layout.report_json,
                &err,
            );
            report.error = Some(format!("golden update: {err}"));
            report
        }
    }
}

fn promote_case_golden(
    case: &IntegrationCase,
    outcome: &CaseRunOutcome,
) -> Result<SuiteCaseReport, HarnessError> {
    let actual = load_rgba(&outcome.layout.actual_png)?;
    let eval = case
        .tolerance
        .evaluate(&actual, &actual)
        .map_err(|msg| HarnessError::ImageCompare(format!("post-update self-compare: {msg}")))?;
    if !eval.passed {
        let mut report = report_from_evaluation(case, eval);
        write_report(&outcome.layout, &mut report)
            .map_err(|e| HarnessError::QueueOptions(format!("write update report: {e}")))?;
        return Err(HarnessError::ImageCompare(
            "post-update self-compare failed coverage gates".to_string(),
        ));
    }

    crate::golden::generate(&outcome.layout.actual_png, &case.golden_path)?;
    if let Some(plain_golden) = case.plain_capture_golden() {
        let plain_actual = outcome.layout.actual_png.with_file_name("plain.png");
        let plain = load_rgba(&plain_actual)?;
        let eval = case.tolerance.evaluate(&plain, &plain).map_err(|msg| {
            HarnessError::ImageCompare(format!("plain post-update self-compare: {msg}"))
        })?;
        if !eval.passed {
            return Err(HarnessError::ImageCompare(
                "plain post-update self-compare failed coverage gates".to_string(),
            ));
        }
        crate::golden::generate(&plain_actual, &plain_golden)?;
    }
    if let Err(e) = std::fs::copy(&outcome.layout.actual_png, &outcome.layout.golden_png_copy) {
        logger::warn!("suite update: failed to refresh artifact golden copy: {e}");
    }

    let mut report = report_from_evaluation(case, eval);
    write_report(&outcome.layout, &mut report)
        .map_err(|e| HarnessError::QueueOptions(format!("write update report: {e}")))?;
    Ok(SuiteCaseReport::plain(
        report.name,
        true,
        outcome.layout.root.clone(),
        outcome.layout.report_json.clone(),
        None,
    ))
}

fn report_from_outcome(outcome: CaseRunOutcome) -> SuiteCaseReport {
    let error = if outcome.report.passed {
        None
    } else {
        outcome
            .report
            .error
            .clone()
            .or_else(|| Some("case failed tolerance".to_string()))
    };
    let mut case = SuiteCaseReport::plain(
        outcome.report.name,
        outcome.report.passed,
        outcome.layout.root,
        outcome.layout.report_json,
        error,
    );
    case.image_compared = outcome.report.evaluation.is_some();
    case
}

/// Writes `suite-report.json`, including the #216 expiry warning when one is set.
pub(crate) fn write_suite_report(
    output_root: &Path,
    report: &SuiteReport,
) -> Result<PathBuf, HarnessError> {
    let report_path = output_root.join(SUITE_REPORT_FILENAME);
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| HarnessError::QueueOptions(format!("serialize suite report: {e}")))?;
    std::fs::write(&report_path, json)?;
    Ok(report_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case_report(name: &str, passed: bool) -> SuiteCaseReport {
        SuiteCaseReport::plain(
            name,
            passed,
            PathBuf::from("target/renderide-test-out").join(name),
            PathBuf::from("target/renderide-test-out")
                .join(name)
                .join("report.json"),
            (!passed).then(|| "failed".to_string()),
        )
    }

    #[test]
    fn select_cases_empty_returns_registry() {
        let cases = select_cases(&[]).expect("select registry");
        assert_eq!(cases.len(), registry().len());
    }

    #[test]
    fn select_cases_resolves_named_subset_in_order() {
        let requested = vec!["torus_unlit_perlin".to_string(), "unlit_sphere".to_string()];
        let cases = select_cases(&requested).expect("select cases");
        let names: Vec<_> = cases.into_iter().map(|case| case.name).collect();
        assert_eq!(names, requested);
    }

    #[test]
    fn select_cases_deduplicates_names() {
        let requested = vec!["unlit_sphere".to_string(), "unlit_sphere".to_string()];
        let cases = select_cases(&requested).expect("select cases");
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].name, "unlit_sphere");
    }

    #[test]
    fn select_cases_rejects_unknown_name() {
        let requested = vec!["missing_case".to_string()];
        let err = select_cases(&requested).expect_err("unknown case");
        assert!(matches!(err, HarnessError::UnknownCase(name) if name == "missing_case"));
    }

    #[test]
    fn suite_report_aggregates_counts() {
        let report = SuiteReport::from_cases(vec![
            case_report("a", true),
            case_report("b", false),
            case_report("c", true),
        ]);
        assert_eq!(report.total, 3);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 1);
        assert!(!report.passed());
    }

    #[test]
    fn write_suite_report_serializes_expected_shape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report = SuiteReport::from_cases(vec![case_report("a", true)]);
        let path = write_suite_report(dir.path(), &report).expect("write suite report");
        assert_eq!(path, dir.path().join(SUITE_REPORT_FILENAME));
        let bytes = std::fs::read(path).expect("read report");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(json["total"], 1);
        assert_eq!(json["passed"], 1);
        assert_eq!(json["failed"], 0);
        assert_eq!(json["quarantined"], 0);
        assert_eq!(json["cases"][0]["name"], "a");
        let summary = json["summary"].as_str().unwrap_or("");
        assert!(summary.contains("#216"), "{summary}");
        assert!(summary.contains("#214"), "{summary}");
        assert!(json.get("quarantine_warning").is_none());
    }

    #[test]
    fn older_suite_json_without_quarantine_fields_still_parses() {
        let raw = r#"{
            "total": 1,
            "passed": 0,
            "failed": 1,
            "cases": [{
                "name": "a",
                "passed": false,
                "artifacts_dir": "out/a",
                "report_json": "out/a/report.json",
                "error": "failed"
            }],
            "summary": "s"
        }"#;
        let report: SuiteReport = serde_json::from_str(raw).expect("parse old suite report");
        assert_eq!(report.quarantined, 0);
        assert_eq!(report.expected_failures, 0);
        assert!(report.quarantine_warning.is_none());
        assert!(report.cases[0].status.is_none());
        assert!(report.cases[0].reason.is_none());
        assert!(report.cases[0].result.is_none());
        assert!(report.cases[0].failure_kind.is_none());
        assert!(report.cases[0].frame_pending.is_none());
        assert!(report.cases[0].gpu_done.is_none());
        assert!(report.cases[0].dropped_draws.is_none());
    }

    #[test]
    fn write_suite_report_writes_the_quarantine_warning() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![case_report("a", false)]);
        let warning = "quarantine #216 expires at 2026-11-14T23:59:59Z".to_string();
        report.quarantine_warning = Some(warning.clone());
        let path = write_suite_report(dir.path(), &report).expect("write suite report");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(SUITE_REPORT_FILENAME)
        );
        let bytes = std::fs::read(path).expect("read report");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(json["quarantine_warning"], warning);
    }

    fn quarantinable_case(dir: &Path) -> SuiteCaseReport {
        let artifacts = dir.join("pbs_lit_material_matrix");
        std::fs::create_dir_all(&artifacts).expect("artifacts");
        let report_json = artifacts.join("report.json");
        let integration = crate::scene_dsl::cases::pbs_lit_material_matrix();
        let case_report =
            crate::scene_dsl::output::report_from_error(&integration, "flat or unstable");
        std::fs::write(
            &report_json,
            serde_json::to_string_pretty(&case_report).expect("report json"),
        )
        .expect("write report");
        let mut case = SuiteCaseReport::plain(
            "pbs_lit_material_matrix",
            false,
            artifacts,
            report_json,
            Some("flat or unstable".to_string()),
        );
        case.failure_kind = Some(SuiteFailureKind::FlatOrUnstableFrame);
        case.frame_pending = Some(0);
        case.gpu_done = Some(true);
        case.dropped_draws = Some(0);
        case
    }

    #[test]
    fn quarantine_applies_on_check_suite_and_not_on_update_suite() {
        let now = SystemTime::UNIX_EPOCH;
        let check_dir = tempfile::tempdir().expect("check dir");
        let mut check = SuiteReport::from_cases(vec![quarantinable_case(check_dir.path())]);
        apply_suite_labels(&mut check, true, now, |_| true);
        assert_eq!(check.quarantined, 1);
        assert_eq!(check.failed, 0);
        assert!(check.passed());
        assert_eq!(check.cases[0].status.as_deref(), Some("quarantined"));

        let update_dir = tempfile::tempdir().expect("update dir");
        let mut update = SuiteReport::from_cases(vec![quarantinable_case(update_dir.path())]);
        apply_suite_labels(&mut update, false, now, |_| true);
        assert_eq!(update.quarantined, 0);
        assert_eq!(update.failed, 1);
        assert!(!update.passed());
        assert!(update.cases[0].status.is_none());
        assert!(update.cases[0].reason.is_none());
    }

    #[test]
    fn harness_timeout_copies_the_timed_out_frame() {
        let flat = HarnessError::FlatOrUnstableFrame {
            path: PathBuf::from("actual.png"),
            wait: std::time::Duration::from_secs(300),
            last_pending: 0,
            ready_chunks: 1,
            pending_chunks: 0,
            missing_chunks: 0,
            frame_count: 1,
            frame_pending: Some(4),
            gpu_done: true,
            dropped_draws: None,
        };
        let case = SuiteCaseReport::from_harness_error(
            "pbs_lit_material_matrix",
            PathBuf::from("out"),
            PathBuf::from("out/report.json"),
            &flat,
        );
        assert_eq!(
            case.failure_kind,
            Some(SuiteFailureKind::FlatOrUnstableFrame)
        );
        assert_eq!(case.frame_pending, Some(4));
        assert_eq!(case.gpu_done, Some(true));
        assert_eq!(case.dropped_draws, None);
        let text = case.error.expect("error");
        assert!(text.contains("last pending=0"), "{text}");
        assert!(text.contains("flat or unstable"), "{text}");
        assert!(!text.contains("pipeline-pending"), "{text}");

        let missing = HarnessError::PngOutputMissing {
            path: PathBuf::from("actual.png"),
            wait: std::time::Duration::from_secs(180),
        };
        let case = SuiteCaseReport::from_harness_error(
            "pbs_lit_material_matrix",
            PathBuf::from("out"),
            PathBuf::from("out/report.json"),
            &missing,
        );
        assert_eq!(case.failure_kind, Some(SuiteFailureKind::PngOutputMissing));
        assert!(case.frame_pending.is_none());
        assert!(case.gpu_done.is_none());
        assert!(case.dropped_draws.is_none());
    }
}
