//! Per-case output directory and JSON report layout for golden-image validation.
//!
//! Each case run writes into a dedicated subdirectory under a configurable test-output root
//! (default: `target/renderide-test-out/<case_name>/`). The fixed filenames are stable so
//! viewers and CI tooling can locate the artifacts deterministically.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::cases::IntegrationCase;
use super::host_info;
use super::side_by_side::SIDE_BY_SIDE_FILENAME;
use super::tolerance::ToleranceEvaluation;

/// Default name for the per-suite output directory beneath `target/`.
pub const DEFAULT_OUTPUT_SUBDIR: &str = "renderide-test-out";

/// Filenames written within a per-case output directory.
const ACTUAL_FILENAME: &str = "actual.png";

/// Path of the captured PNG inside a case artifact directory.
pub(crate) fn actual_png_in(artifacts_dir: &Path) -> PathBuf {
    artifacts_dir.join(ACTUAL_FILENAME)
}
const GOLDEN_FILENAME: &str = "golden.png";
const DIFF_FILENAME: &str = "diff.png";
const REPORT_FILENAME: &str = "report.json";

/// Sidecar written beside `actual.png` with the capture's stall and WARP budget.
pub(crate) const CAPTURE_STATS_FILENAME: &str = "capture-stats.json";

/// Paths used for one case run. Build with [`Self::for_case`]; do not construct fields by hand.
#[derive(Clone, Debug)]
pub struct CaseOutputLayout {
    /// Per-case directory containing all artifacts.
    pub root: PathBuf,
    /// PNG written by the renderer.
    pub actual_png: PathBuf,
    /// Copy of the golden PNG used for comparison (debug aid).
    pub golden_png_copy: PathBuf,
    /// Diff visualization. Written whenever both images load, including a pass.
    pub diff_png: PathBuf,
    /// `golden | actual | diff`, three times the actual width.
    pub side_by_side_png: PathBuf,
    /// JSON report describing the comparison criteria and outcome.
    pub report_json: PathBuf,
}

impl CaseOutputLayout {
    /// Builds the layout under `<output_root>/<case_name>/`.
    pub fn for_case(output_root: &Path, case_name: &str) -> Self {
        let root = output_root.join(case_name);
        Self {
            actual_png: root.join(ACTUAL_FILENAME),
            golden_png_copy: root.join(GOLDEN_FILENAME),
            diff_png: root.join(DIFF_FILENAME),
            side_by_side_png: root.join(SIDE_BY_SIDE_FILENAME),
            report_json: root.join(REPORT_FILENAME),
            root,
        }
    }

    /// Ensures the per-case directory exists and removes any stale artifacts left by a
    /// previous run so consumers see a clean slate.
    pub fn prepare(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let plain_png = self.actual_png.with_file_name("plain.png");
        let plain_diff = self.root.join("plain_diff.png");
        let capture_stats = self.actual_png.with_file_name(CAPTURE_STATS_FILENAME);
        for stale in [
            &self.actual_png,
            &self.golden_png_copy,
            &self.diff_png,
            &self.side_by_side_png,
            &self.report_json,
            &plain_png,
            &plain_diff,
            &capture_stats,
        ] {
            if stale.exists() {
                std::fs::remove_file(stale)?;
            }
        }
        Ok(())
    }
}

/// Default test-output root: `<workspace_root>/target/<DEFAULT_OUTPUT_SUBDIR>`.
pub fn default_output_root() -> PathBuf {
    workspace_target_dir().join(DEFAULT_OUTPUT_SUBDIR)
}

fn workspace_target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(|p| p.join("target"))
        .unwrap_or_else(|| PathBuf::from("target"))
}

/// Serialized per-case report shaped so a future HTML viewer can ingest it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseReport {
    /// Case identifier.
    pub name: String,
    /// Free-form description.
    pub description: String,
    /// Render target dimensions.
    pub resolution: (u32, u32),
    /// Tolerance evaluation, or `None` when the case did not produce a comparable image.
    pub evaluation: Option<ToleranceEvaluation>,
    /// Aggregate pass/fail.
    pub passed: bool,
    /// Optional error message when `passed = false` and `evaluation = None` (e.g. capture
    /// failure, harness error).
    pub error: Option<String>,
    /// `behavior` or `adapter_comparison`. Stamped after the renderer log names the adapter.
    #[serde(default = "default_behavior_gate")]
    pub gate: String,
    /// `expected failure (WARP, #212)` when that case is the known WARP mismatch.
    /// `unexpected pass (WARP, #212)` when WARP passes `pbs_lit` and `passed` stays true.
    /// Absent on a hard failure and on an ordinary pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Runner CPU model. Empty only for reports written before this field existed.
    #[serde(default)]
    pub cpu_model: String,
    /// Headless wgpu adapter line (name, device type, backend). `unavailable` when the
    /// renderer log has no matching capture.
    #[serde(default)]
    pub adapter: String,
    /// Longest `on_submitted_work_done` stall observed for this case, in milliseconds.
    #[serde(default)]
    pub longest_submission_stall_ms: u64,
    /// True when this case ran on WARP and the harness applied the WARP readiness budget.
    #[serde(default)]
    pub warp_readiness_budget_used: bool,
    /// `quarantined` when the #216 rule covers this case. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// `quarantined:#216` while that quarantine applies. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Set when a #216 WARP flat wait ended before the full readiness budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_exit: Option<EarlyExit>,
    /// `renderide-test.v2` once [`super::report_v2::stamp`] has run.
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Non-empty sentence for [`Self::reason`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_text: Option<String>,
    /// Conventional artifact filenames. Absolute paths stay on the output layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) artifacts: Option<super::report_v2::ArtifactNames>,
    /// Stall, WARP budget, and early exit, beside the flat fields of the same facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) capture: Option<super::report_v2::CaptureBlock>,
    /// CPU, structured adapter, and Vulkan ICD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runner: Option<super::report_v2::RunnerBlock>,
    /// Default 2×2 region stats when both images were loaded. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) region_stats: Option<Vec<super::region_stats::RegionStatRow>>,
}

/// Why a quarantined WARP flat wait stopped before the full budget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EarlyExit {
    /// Case directory name.
    pub case: String,
    /// Consecutive flat readbacks required.
    pub k: u32,
    /// Milliseconds from the start of the capture wait. Settle plus K-window.
    pub elapsed_ms: u64,
    /// Milliseconds from the wait start to the first settled flat frame.
    ///
    /// Zero on reports written before this field existed.
    #[serde(default)]
    pub settle_ms: u64,
    /// Milliseconds from the first settled flat frame to the exit.
    ///
    /// Zero on reports written before this field existed.
    #[serde(default)]
    pub k_window_ms: u64,
    /// `flat_stable` when K identical settled frames were seen.
    pub reason: String,
}

fn default_behavior_gate() -> String {
    crate::adapter_gate::BEHAVIOR.to_string()
}

fn default_schema() -> String {
    super::report_v2::SCHEMA.to_string()
}

/// Writes a `report.json` describing the comparison outcome.
///
/// Stamps the runner CPU and the adapter line for this case's PNG before serializing.
pub fn write_report(layout: &CaseOutputLayout, report: &mut CaseReport) -> io::Result<()> {
    let host = host_info::capture(&layout.actual_png);
    report.cpu_model = host.cpu_model;
    report.adapter = host.adapter;
    let (stall_ms, warp_budget_used, early_exit) = read_capture_stats(&layout.actual_png);
    report.longest_submission_stall_ms = stall_ms;
    report.warp_readiness_budget_used = warp_budget_used;
    if early_exit.is_some() {
        report.early_exit = early_exit;
    }
    super::report_v2::stamp(report, layout);
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| io::Error::other(format!("serialize report: {e}")))?;
    std::fs::write(&layout.report_json, json)
}

/// Builds a [`CaseReport`] from the comparison evaluation.
pub fn report_from_evaluation(case: &IntegrationCase, eval: ToleranceEvaluation) -> CaseReport {
    let passed = eval.passed;
    let error = super::channel_zero::failure_message(&eval.criteria);
    base_report(case, passed, Some(eval), error)
}

/// Builds a [`CaseReport`] for a case that failed before image comparison ran (e.g. renderer
/// crash, golden missing).
pub fn report_from_error(case: &IntegrationCase, message: impl Into<String>) -> CaseReport {
    base_report(case, false, None, Some(message.into()))
}

fn base_report(
    case: &IntegrationCase,
    passed: bool,
    evaluation: Option<ToleranceEvaluation>,
    error: Option<String>,
) -> CaseReport {
    CaseReport {
        name: case.name.clone(),
        description: case.description.clone(),
        resolution: case.resolution,
        passed,
        evaluation,
        error,
        gate: default_behavior_gate(),
        result: None,
        cpu_model: String::new(),
        adapter: String::new(),
        longest_submission_stall_ms: 0,
        warp_readiness_budget_used: false,
        status: None,
        reason: None,
        early_exit: None,
        schema: default_schema(),
        reason_text: None,
        artifacts: None,
        capture: None,
        runner: None,
        region_stats: None,
    }
}

fn read_capture_stats(actual_png: &Path) -> (u64, bool, Option<EarlyExit>) {
    let path = actual_png.with_file_name(CAPTURE_STATS_FILENAME);
    let Ok(bytes) = std::fs::read(path) else {
        return (0, false, None);
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return (0, false, None);
    };
    let stall_ms = value
        .get("longest_submission_stall_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let warp_budget_used = value
        .get("warp_readiness_budget_used")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let early_exit = value
        .get("early_exit")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok());
    (stall_ms, warp_budget_used, early_exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_case_builds_expected_filenames() {
        let layout = CaseOutputLayout::for_case(Path::new("/tmp/xyz"), "lit_zoo");
        assert_eq!(layout.root, PathBuf::from("/tmp/xyz/lit_zoo"));
        assert_eq!(
            layout.actual_png,
            PathBuf::from("/tmp/xyz/lit_zoo/actual.png")
        );
        assert_eq!(
            layout.golden_png_copy,
            PathBuf::from("/tmp/xyz/lit_zoo/golden.png")
        );
        assert_eq!(layout.diff_png, PathBuf::from("/tmp/xyz/lit_zoo/diff.png"));
        assert_eq!(
            layout.report_json,
            PathBuf::from("/tmp/xyz/lit_zoo/report.json")
        );
    }

    #[test]
    fn prepare_creates_dir_and_clears_stale_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let layout = CaseOutputLayout::for_case(dir.path(), "case_a");
        layout.prepare().expect("prepare 1");
        std::fs::write(&layout.actual_png, b"old").expect("seed stale");
        layout.prepare().expect("prepare 2");
        assert!(
            !layout.actual_png.exists(),
            "stale artifact must be cleared"
        );
        assert!(layout.root.is_dir());
    }

    #[test]
    fn report_serializes_to_json() {
        let case = crate::scene_dsl::cases::unlit_sphere();
        let mut report = report_from_error(&case, "boom");
        let layout =
            CaseOutputLayout::for_case(tempfile::tempdir().expect("tempdir").path(), &case.name);
        layout.prepare().expect("prepare");
        write_report(&layout, &mut report).expect("write report");
        let bytes = std::fs::read(&layout.report_json).expect("read report");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json");
        assert_eq!(json["name"], "unlit_sphere");
        assert_eq!(json["passed"], false);
        assert_eq!(json["error"], "boom");
        assert!(!json["cpu_model"].as_str().unwrap_or("").is_empty());
        assert!(json["adapter"].is_string());
        assert_eq!(json["longest_submission_stall_ms"], 0);
        assert_eq!(json["warp_readiness_budget_used"], false);
    }

    #[test]
    fn report_records_the_longest_submission_stall_and_warp_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let case = crate::scene_dsl::cases::unlit_sphere();
        let layout = CaseOutputLayout::for_case(dir.path(), &case.name);
        layout.prepare().expect("prepare");
        let stats = layout.actual_png.with_file_name(CAPTURE_STATS_FILENAME);
        std::fs::write(
            &stats,
            r#"{"longest_submission_stall_ms":127000,"warp_readiness_budget_used":true}"#,
        )
        .expect("sidecar");
        let mut report = report_from_error(&case, "boom");
        write_report(&layout, &mut report).expect("write report");
        let bytes = std::fs::read(&layout.report_json).expect("read report");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json");
        assert_eq!(json["longest_submission_stall_ms"], 127_000);
        assert_eq!(json["warp_readiness_budget_used"], true);
        assert_eq!(report.longest_submission_stall_ms, 127_000);
        assert!(report.warp_readiness_budget_used);
        assert!(json.get("early_exit").is_none());
    }

    #[test]
    fn report_records_a_quarantine_early_exit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let case = crate::scene_dsl::cases::unlit_sphere();
        let layout = CaseOutputLayout::for_case(dir.path(), &case.name);
        layout.prepare().expect("prepare");
        let stats = layout.actual_png.with_file_name(CAPTURE_STATS_FILENAME);
        std::fs::write(
            &stats,
            r#"{"longest_submission_stall_ms":1,"warp_readiness_budget_used":true,"early_exit":{"case":"pbs_lit_material_matrix","k":3,"elapsed_ms":12500,"reason":"flat_stable"}}"#,
        )
        .expect("sidecar");
        let mut report = report_from_error(&case, "boom");
        write_report(&layout, &mut report).expect("write report");
        let bytes = std::fs::read(&layout.report_json).expect("read report");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json");
        assert_eq!(json["early_exit"]["case"], "pbs_lit_material_matrix");
        assert_eq!(json["early_exit"]["k"], 3);
        assert_eq!(json["early_exit"]["elapsed_ms"], 12_500);
        assert_eq!(json["early_exit"]["settle_ms"], 0);
        assert_eq!(json["early_exit"]["k_window_ms"], 0);
        assert_eq!(json["early_exit"]["reason"], "flat_stable");
    }

    #[test]
    fn report_splits_settle_time_from_the_k_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let case = crate::scene_dsl::cases::unlit_sphere();
        let layout = CaseOutputLayout::for_case(dir.path(), &case.name);
        layout.prepare().expect("prepare");
        let stats = layout.actual_png.with_file_name(CAPTURE_STATS_FILENAME);
        std::fs::write(
            &stats,
            r#"{"longest_submission_stall_ms":1,"warp_readiness_budget_used":true,"early_exit":{"case":"pbs_lit_material_matrix","k":3,"elapsed_ms":12500,"settle_ms":4000,"k_window_ms":8500,"reason":"flat_stable"}}"#,
        )
        .expect("sidecar");
        let mut report = report_from_error(&case, "boom");
        write_report(&layout, &mut report).expect("write report");
        let bytes = std::fs::read(&layout.report_json).expect("read report");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json");
        assert_eq!(json["early_exit"]["settle_ms"], 4_000);
        assert_eq!(json["early_exit"]["k_window_ms"], 8_500);
        assert_eq!(json["early_exit"]["elapsed_ms"], 12_500);
        assert_eq!(
            json["early_exit"]["settle_ms"].as_u64().unwrap_or(0)
                + json["early_exit"]["k_window_ms"].as_u64().unwrap_or(0),
            12_500
        );
    }
}
