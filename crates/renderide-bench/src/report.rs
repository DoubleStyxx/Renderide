//! JSON documents for `renderide-bench run` / `compare` / `list` (`--json`).

use serde::{Deserialize, Serialize};

use renderide_framebench::{FrameBenchReport, RunnerInfo};

/// Schema id for a successful or failed run document.
pub const RUN_SCHEMA: &str = "renderide-bench/1";
/// Schema id for a compare document.
pub const COMPARE_SCHEMA: &str = "renderide-bench-compare/1";
/// Schema id for a list document.
pub const LIST_SCHEMA: &str = "renderide-bench-list/1";

/// One criterion measurement collected from `estimates.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CpuSample {
    /// Criterion group name.
    pub group: String,
    /// Criterion function id.
    pub id: String,
    /// Median point estimate in nanoseconds.
    pub median_ns: f64,
    /// 95% CI bounds for the median, `[lower, upper]`, in nanoseconds.
    pub ci95_ns: [f64; 2],
    /// Optional throughput string from criterion (unused today).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<String>,
}

/// Front-door run report (`schema: renderide-bench/1`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchRunReport {
    /// Schema id.
    pub schema: String,
    /// Git SHA of the tree under test, when known.
    pub git_sha: String,
    /// Process exit code this document corresponds to.
    pub exit_code: u8,
    /// Runner environment (shared shape with framebench schema 2).
    pub runner: RunnerInfo,
    /// Embedded framebench report when `--suite frame|all` ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameBenchReport>,
    /// Criterion samples when `--suite cpu|all` ran.
    pub cpu: Vec<CpuSample>,
}

/// Compare verdict for one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// No regression (at or below the fail threshold, or cleared by the abs floor).
    Pass,
    /// Above the flag threshold but at or below the fail threshold (report-only).
    Flag,
    /// Above the fail threshold (and not cleared by the abs floor).
    Fail,
    /// Present on head only.
    New,
    /// Present on base only, or a gated metric could not be evaluated.
    Missing,
}

/// One compare row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompareRow {
    /// `cpu` or `frame`.
    pub suite: String,
    /// Scenario or `group/id`.
    pub name: String,
    /// Metric path (`cpu_frame_ms.p95` or `median_ns`).
    pub metric: String,
    /// Baseline value, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<f64>,
    /// Head value, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<f64>,
    /// Relative delta in percent, when defined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_pct: Option<f64>,
    /// Mann-Whitney p-value when available (always null in B2; reserved for B3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value: Option<f64>,
    /// Decision.
    pub verdict: Verdict,
}

/// Front-door compare report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchCompareReport {
    /// Schema id.
    pub schema: String,
    /// Fail threshold used (percent).
    pub fail_threshold: f64,
    /// Flag threshold used (percent).
    pub flag_threshold: f64,
    /// Absolute floor in milliseconds (0 = off). Applies to frame ms metrics only.
    pub abs_floor_ms: f64,
    /// Where the shared decision rule lives.
    pub decision_rule: String,
    /// Rows.
    pub rows: Vec<CompareRow>,
    /// Process exit code.
    pub exit_code: u8,
}

/// List document.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchListReport {
    /// Schema id.
    pub schema: String,
    /// CPU ids as `group/id`.
    pub cpu: Vec<String>,
    /// Frame scenario names.
    pub frame: Vec<String>,
}

/// `--json` error document.
#[derive(Clone, Debug, Serialize)]
pub struct ErrorDocument {
    /// Schema id (run schema, so callers can branch on one family).
    pub schema: String,
    /// Error body.
    pub error: ErrorBody,
    /// Runner info when available.
    pub runner: RunnerInfo,
}

/// Error payload.
#[derive(Clone, Debug, Serialize)]
pub struct ErrorBody {
    /// Exit code.
    pub exit_code: u8,
    /// Stable kind: `usage`, `environment`, or `run_failure`.
    pub kind: &'static str,
    /// Human message.
    pub message: String,
}
