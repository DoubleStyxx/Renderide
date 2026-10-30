//! Relative compare of two frame-benchmark reports.
//!
//! A gated metric fails only when `(candidate - baseline) / baseline * 100` is greater than
//! the threshold. Equality with the threshold passes. A zero baseline is `n/a` and does not
//! fail. Absolute milliseconds are never a gate.
//!
//! Absolute floor (opt-in, off by default): with `--abs-floor-ms` above 0, a millisecond metric
//! (`cpu_frame_ms.*`, `gpu_passes_ms.*`) whose `candidate - baseline` is at most the floor never
//! fails, whatever its percent. [`DEFAULT_ABS_FLOOR_MS`] is 0 because a nonzero default is open
//! question O8 (plans/test-bench-rewrite.md): at ~0.2 ms lavapipe medians a 0.05 ms floor is a
//! 25% effective bar. `scripts/ci/frame_bench_gate.py` has the same flag and default, so both
//! tools share one rule.
//!
//! Schema 1 and schema 2 reports are both accepted, in any mix. A schema-2 report must carry
//! its `runner` block, as in the gate; a schema-2 report without one is an input error.

use crate::error::BenchError;
use crate::report::{FrameBenchReport, SUPPORTED_SCHEMAS, ScenarioReport, schema_supported};
use crate::stats::Percentiles;

/// Default relative threshold, in percent. Fail when the delta is strictly greater.
pub const DEFAULT_THRESHOLD_PERCENT: f64 = 10.0;

/// Default gated metric.
pub const DEFAULT_GATE_METRIC: &str = "cpu_frame_ms.p95";

/// Default absolute floor in milliseconds: 0, off.
///
/// `--abs-floor-ms` opts in. With a floor above 0, a ms-metric delta at or below it never
/// fails. O8 decides whether a nonzero default ships.
pub const DEFAULT_ABS_FLOOR_MS: f64 = 0.0;

/// Slack for floating-point noise at the floor boundary (`1.05 - 1.0` is `0.0500000…04`).
const ABS_FLOOR_EPSILON_MS: f64 = 1e-9;

/// Outcome of the shared relative gate (percent threshold plus optional absolute floor).
///
/// Used by `renderide-framebench compare`, `renderide-bench compare`, and documented as the
/// same percent/floor semantics as `scripts/ci/frame_bench_gate.py` (the Python gate also
/// runs Mann-Whitney on multi-run samples; that path stays in the script until B3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeStatus {
    /// Delta is at or below the threshold, or the metric is not gated.
    Ok,
    /// Delta is above the threshold in percent, but at or below the absolute floor.
    OkAbsFloor,
    /// Gated regression: above the threshold and (if set) above the absolute floor.
    Fail,
    /// Baseline is zero/non-finite, so percent is undefined.
    Undefined,
}

/// Shared relative decision used by framebench compare and the renderide-bench front door.
///
/// `abs_floor` is in the same units as `base`/`cand`. Pass `None` (or `Some(0.0)`) to disable
/// the floor. When `gated` is false, the status is always [`RelativeStatus::Ok`] or
/// [`RelativeStatus::Undefined`].
#[must_use]
pub fn relative_status(
    base: f64,
    cand: f64,
    gated: bool,
    threshold_percent: f64,
    abs_floor: Option<f64>,
) -> (Option<f64>, RelativeStatus) {
    let Some(delta) = relative_delta(base, cand) else {
        return (
            None,
            if gated {
                RelativeStatus::Undefined
            } else {
                RelativeStatus::Ok
            },
        );
    };
    if !gated {
        return (Some(delta), RelativeStatus::Ok);
    }
    if delta <= threshold_percent {
        return (Some(delta), RelativeStatus::Ok);
    }
    let floor = abs_floor.filter(|f| *f > 0.0);
    if floor.is_some_and(|f| below_abs_floor(base, cand, f)) {
        return (Some(delta), RelativeStatus::OkAbsFloor);
    }
    (Some(delta), RelativeStatus::Fail)
}

const INFO_METRICS: &[&str] = &[
    "cpu_frame_ms.p50",
    "cpu_frame_ms.p95",
    "cpu_frame_ms.p99",
    "draw_calls.p50",
    "upload_count.p50",
    "upload_bytes.p50",
];

/// Knobs for [`compare_reports`].
#[derive(Clone, Debug)]
pub struct CompareOptions {
    /// Fail when a gated relative delta is strictly greater than this percent.
    pub threshold_percent: f64,
    /// Metrics that can fail the process. Empty means [`DEFAULT_GATE_METRIC`].
    pub gate: Vec<String>,
    /// Millisecond deltas at or below this never fail. [`DEFAULT_ABS_FLOOR_MS`] (0, off) by
    /// default.
    pub abs_floor_ms: f64,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            threshold_percent: DEFAULT_THRESHOLD_PERCENT,
            gate: vec![DEFAULT_GATE_METRIC.to_string()],
            abs_floor_ms: DEFAULT_ABS_FLOOR_MS,
        }
    }
}

/// Markdown table plus whether any gated metric regressed.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareOutcome {
    /// Job-summary markdown.
    pub markdown: String,
    /// True when a gated metric regressed by more than the threshold.
    pub failed: bool,
}

/// Loads two schema-1 or schema-2 reports and compares them.
///
/// # Errors
///
/// Returns an error when a file is missing, a schema is not 1 or 2, or a gate metric path is
/// unknown.
pub fn compare_reports(
    baseline_path: &std::path::Path,
    candidate_path: &std::path::Path,
    options: &CompareOptions,
) -> Result<CompareOutcome, BenchError> {
    let baseline = load_report(baseline_path)?;
    let candidate = load_report(candidate_path)?;
    compare_loaded(&baseline, &candidate, options)
}

/// Compares two in-memory reports.
///
/// # Errors
///
/// Returns an error when a schema is unsupported, a schema-2 report has no `runner` block, a
/// gate metric path is unknown, or the absolute floor is negative or not finite.
pub fn compare_loaded(
    baseline: &FrameBenchReport,
    candidate: &FrameBenchReport,
    options: &CompareOptions,
) -> Result<CompareOutcome, BenchError> {
    if !schema_supported(baseline.schema) || !schema_supported(candidate.schema) {
        return Err(BenchError::Message(format!(
            "expected schema in {SUPPORTED_SCHEMAS:?}, got baseline {} and candidate {}",
            baseline.schema, candidate.schema
        )));
    }
    for (side, report) in [("baseline", baseline), ("candidate", candidate)] {
        if report.schema >= 2 && report.runner.is_none() {
            return Err(BenchError::Message(format!(
                "{side} is a schema {} report without a `runner` object",
                report.schema
            )));
        }
    }
    if !options.abs_floor_ms.is_finite() || options.abs_floor_ms < 0.0 {
        return Err(BenchError::Message(format!(
            "abs floor must be a finite number >= 0 ms, got {}",
            options.abs_floor_ms
        )));
    }
    let gate = if options.gate.is_empty() {
        vec![DEFAULT_GATE_METRIC.to_string()]
    } else {
        options.gate.clone()
    };
    for metric in &gate {
        if !metric_path_known(metric) {
            return Err(BenchError::Message(format!(
                "unknown gate metric `{metric}`"
            )));
        }
    }
    let metrics = display_metrics(&gate);
    let mut names = baseline
        .scenarios
        .iter()
        .map(|scenario| scenario.name.clone())
        .collect::<Vec<_>>();
    for scenario in &candidate.scenarios {
        if !names.iter().any(|name| name == &scenario.name) {
            names.push(scenario.name.clone());
        }
    }
    names.sort();

    let mut rows = Vec::new();
    let mut failed = false;
    for name in &names {
        for metric in &metrics {
            let row = compare_metric(
                baseline,
                candidate,
                name,
                metric,
                gate.iter().any(|gated| gated == metric),
                options.threshold_percent,
                options.abs_floor_ms,
            );
            if row.status == "FAIL" {
                failed = true;
            }
            rows.push(row);
        }
        rows.extend(gpu_info_rows(baseline, candidate, name, &gate));
    }

    Ok(CompareOutcome {
        markdown: render_markdown(
            baseline,
            candidate,
            options.threshold_percent,
            options.abs_floor_ms,
            &gate,
            &rows,
            failed,
        ),
        failed,
    })
}

fn load_report(path: &std::path::Path) -> Result<FrameBenchReport, BenchError> {
    let bytes = std::fs::read(path)
        .map_err(|err| BenchError::Message(format!("read {}: {err}", path.display())))?;
    let report: FrameBenchReport = serde_json::from_slice(&bytes)?;
    if !schema_supported(report.schema) {
        return Err(BenchError::Message(format!(
            "{} has schema {}, expected one of {SUPPORTED_SCHEMAS:?}",
            path.display(),
            report.schema
        )));
    }
    Ok(report)
}

struct Row {
    scenario: String,
    metric: String,
    baseline: String,
    candidate: String,
    delta: String,
    status: &'static str,
}

fn compare_metric(
    baseline: &FrameBenchReport,
    candidate: &FrameBenchReport,
    scenario: &str,
    metric: &str,
    gated: bool,
    threshold: f64,
    abs_floor_ms: f64,
) -> Row {
    let left = lookup(baseline, scenario, metric);
    let right = lookup(candidate, scenario, metric);
    let floor = metric_is_ms(metric).then_some(abs_floor_ms);
    let (delta, status) = match (left, right) {
        (Lookup::Value(base), Lookup::Value(cand)) => {
            let (_, _, delta, status) = status_for_values(base, cand, gated, threshold, floor);
            (delta, status)
        }
        (Lookup::Missing, _) | (_, Lookup::Missing) => ("n/a".to_string(), "missing"),
        _ => ("n/a".to_string(), if gated { "missing" } else { "info" }),
    };
    Row {
        scenario: scenario.to_string(),
        metric: metric.to_string(),
        baseline: format_lookup(left),
        candidate: format_lookup(right),
        delta,
        status,
    }
}

fn format_lookup(value: Lookup) -> String {
    match value {
        Lookup::Value(ms) => format_ms(ms),
        Lookup::Null => "null".to_string(),
        Lookup::Missing => "missing".to_string(),
    }
}

/// True for millisecond metrics, the only ones the absolute floor applies to.
fn metric_is_ms(metric: &str) -> bool {
    metric.starts_with("cpu_frame_ms.") || metric.starts_with("gpu_passes_ms.")
}

/// True when `cand - base` is at or below the absolute floor (with float slack).
fn below_abs_floor(base: f64, cand: f64, floor_ms: f64) -> bool {
    cand - base <= floor_ms + ABS_FLOOR_EPSILON_MS
}

fn status_for_values(
    base: f64,
    cand: f64,
    gated: bool,
    threshold: f64,
    floor_ms: Option<f64>,
) -> (String, String, String, &'static str) {
    let (delta, status) = relative_status(base, cand, gated, threshold, floor_ms);
    let status = match (gated, status) {
        (false, RelativeStatus::Ok | RelativeStatus::Undefined) => "info",
        (_, RelativeStatus::Ok) => "ok",
        (_, RelativeStatus::OkAbsFloor) => "ok (abs floor)",
        (_, RelativeStatus::Fail) => "FAIL",
        (_, RelativeStatus::Undefined) => "n/a",
    };
    let delta = delta.map_or_else(|| "n/a".to_string(), |d| format!("{d:+.2}%"));
    (format_ms(base), format_ms(cand), delta, status)
}

fn gpu_info_rows(
    baseline: &FrameBenchReport,
    candidate: &FrameBenchReport,
    scenario: &str,
    gate: &[String],
) -> Vec<Row> {
    let left = find_scenario(baseline, scenario).and_then(|item| item.gpu_passes_ms.as_ref());
    let right = find_scenario(candidate, scenario).and_then(|item| item.gpu_passes_ms.as_ref());
    match (left, right) {
        (None, None) => vec![Row {
            scenario: scenario.to_string(),
            metric: "gpu_passes_ms".to_string(),
            baseline: "null".to_string(),
            candidate: "null".to_string(),
            delta: "n/a".to_string(),
            status: "info",
        }],
        (Some(base), Some(cand)) => {
            let mut keys = base.keys().cloned().collect::<Vec<_>>();
            for key in cand.keys() {
                if !keys.iter().any(|existing| existing == key) {
                    keys.push(key.clone());
                }
            }
            keys.sort();
            keys.into_iter()
                .filter(|key| {
                    let metric = format!("gpu_passes_ms.{key}.p50");
                    !gate.iter().any(|gated| gated == &metric)
                })
                .map(|key| {
                    let metric = format!("gpu_passes_ms.{key}.p50");
                    let base_value = base.get(&key).map(|stats| stats.p50);
                    let cand_value = cand.get(&key).map(|stats| stats.p50);
                    let (baseline_text, candidate_text, delta, status) =
                        match (base_value, cand_value) {
                            (Some(base_ms), Some(cand_ms)) => {
                                let (_, _, delta, status) =
                                    status_for_values(base_ms, cand_ms, false, 0.0, None);
                                (format_ms(base_ms), format_ms(cand_ms), delta, status)
                            }
                            _ => (
                                base_value
                                    .map(format_ms)
                                    .unwrap_or_else(|| "missing".to_string()),
                                cand_value
                                    .map(format_ms)
                                    .unwrap_or_else(|| "missing".to_string()),
                                "n/a".to_string(),
                                "info",
                            ),
                        };
                    Row {
                        scenario: scenario.to_string(),
                        metric,
                        baseline: baseline_text,
                        candidate: candidate_text,
                        delta,
                        status,
                    }
                })
                .collect()
        }
        _ => vec![Row {
            scenario: scenario.to_string(),
            metric: "gpu_passes_ms".to_string(),
            baseline: if left.is_some() { "present" } else { "null" }.to_string(),
            candidate: if right.is_some() { "present" } else { "null" }.to_string(),
            delta: "n/a".to_string(),
            status: "info",
        }],
    }
}

#[derive(Clone, Copy)]
enum Lookup {
    Value(f64),
    Null,
    Missing,
}

fn lookup(report: &FrameBenchReport, scenario: &str, metric: &str) -> Lookup {
    let Some(scenario) = find_scenario(report, scenario) else {
        return Lookup::Missing;
    };
    match metric_value(scenario, metric) {
        MetricHit::Value(value) => Lookup::Value(value),
        MetricHit::Null => Lookup::Null,
        MetricHit::Absent => Lookup::Missing,
    }
}

fn find_scenario<'a>(report: &'a FrameBenchReport, name: &str) -> Option<&'a ScenarioReport> {
    report
        .scenarios
        .iter()
        .find(|scenario| scenario.name == name)
}

enum MetricHit {
    Value(f64),
    Null,
    Absent,
}

fn metric_value(scenario: &ScenarioReport, metric: &str) -> MetricHit {
    if let Some(pass) = metric.strip_prefix("gpu_passes_ms.") {
        return gpu_metric(scenario, pass);
    }
    let Some((name, which)) = metric.split_once('.') else {
        return MetricHit::Absent;
    };
    let Some(stats) = scalar_percentiles(scenario, name) else {
        return MetricHit::Absent;
    };
    match pick_percentile(stats, which) {
        Some(value) => MetricHit::Value(value),
        None => MetricHit::Absent,
    }
}

fn gpu_metric(scenario: &ScenarioReport, pass: &str) -> MetricHit {
    let Some((name, which)) = pass.rsplit_once('.') else {
        return MetricHit::Absent;
    };
    if !matches!(which, "p50" | "p95" | "p99") {
        return MetricHit::Absent;
    }
    let Some(map) = scenario.gpu_passes_ms.as_ref() else {
        return MetricHit::Null;
    };
    let Some(stats) = map.get(name) else {
        return MetricHit::Absent;
    };
    match pick_percentile(stats, which) {
        Some(value) => MetricHit::Value(value),
        None => MetricHit::Absent,
    }
}

fn scalar_percentiles<'a>(scenario: &'a ScenarioReport, name: &str) -> Option<&'a Percentiles> {
    match name {
        "cpu_frame_ms" => Some(&scenario.cpu_frame_ms),
        "draw_calls" => Some(&scenario.draw_calls),
        "upload_count" => Some(&scenario.upload_count),
        "upload_bytes" => Some(&scenario.upload_bytes),
        _ => None,
    }
}

fn pick_percentile(stats: &Percentiles, which: &str) -> Option<f64> {
    match which {
        "p50" => Some(stats.p50),
        "p95" => Some(stats.p95),
        "p99" => Some(stats.p99),
        _ => None,
    }
}

fn metric_path_known(metric: &str) -> bool {
    if metric.strip_prefix("gpu_passes_ms.").is_some() {
        let (_, which) = metric.rsplit_once('.').unwrap_or(("", ""));
        return matches!(which, "p50" | "p95" | "p99") && metric.matches('.').count() >= 2;
    }
    let Some((name, which)) = metric.split_once('.') else {
        return false;
    };
    matches!(
        name,
        "cpu_frame_ms" | "draw_calls" | "upload_count" | "upload_bytes"
    ) && matches!(which, "p50" | "p95" | "p99")
}

fn display_metrics(gate: &[String]) -> Vec<String> {
    let mut metrics = INFO_METRICS
        .iter()
        .map(|metric| (*metric).to_string())
        .collect::<Vec<_>>();
    for metric in gate {
        if !metrics.iter().any(|existing| existing == metric) {
            metrics.push(metric.clone());
        }
    }
    metrics
}

fn relative_delta(baseline: f64, candidate: f64) -> Option<f64> {
    if !baseline.is_finite() || !candidate.is_finite() || baseline == 0.0 {
        return None;
    }
    let delta = (candidate - baseline) / baseline * 100.0;
    delta.is_finite().then_some(delta)
}

fn format_ms(value: f64) -> String {
    format!("{value:.4}")
}

fn render_markdown(
    baseline: &FrameBenchReport,
    candidate: &FrameBenchReport,
    threshold: f64,
    abs_floor_ms: f64,
    gate: &[String],
    rows: &[Row],
    failed: bool,
) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    out.push_str("# Frame benchmark compare\n\n");
    out.push_str(
        "Absolute milliseconds are not a gate and are not comparable across machines, adapters, or lavapipe versus a discrete GPU. The gate is relative; an absolute floor, when set, only ever clears a row, never fails one.\n\n",
    );
    let _ = writeln!(out, "baseline_git: {}", baseline.git_sha);
    let _ = writeln!(out, "candidate_git: {}", candidate.git_sha);
    let _ = writeln!(out, "threshold: >{threshold:.4}% on gated metrics");
    if abs_floor_ms > 0.0 {
        let _ = writeln!(
            out,
            "abs_floor: ms deltas <= {abs_floor_ms:.4} ms never fail"
        );
    }
    let gated = gate.join(", ");
    let _ = write!(out, "gated: {gated}\n\n");
    out.push_str("| scenario | metric | baseline | candidate | delta | status |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        let scenario = cell(&row.scenario);
        let metric = cell(&row.metric);
        let baseline = cell(&row.baseline);
        let candidate = cell(&row.candidate);
        let delta = cell(&row.delta);
        let status = row.status;
        let _ = writeln!(
            out,
            "| {scenario} | {metric} | {baseline} | {candidate} | {delta} | {status} |"
        );
    }
    out.push('\n');
    if failed {
        out.push_str("result: fail\n");
    } else {
        out.push_str("result: pass\n");
    }
    out
}

fn cell(value: &str) -> String {
    value.replace('|', "/")
}

#[cfg(test)]
mod tests {
    use crate::compare::{CompareOptions, DEFAULT_ABS_FLOOR_MS, compare_loaded};
    use crate::report::{
        AdapterInfo, ControlApi, FrameBenchReport, SCHEMA, ScenarioReport, standard_notes,
    };
    use crate::runner_env::{IcdInfo, RunnerInfo};
    use crate::stats::Percentiles;

    fn percentiles(p50: f64, p95: f64, p99: f64) -> Percentiles {
        Percentiles { p50, p95, p99 }
    }

    fn scenario(name: &str, p95: f64) -> ScenarioReport {
        ScenarioReport {
            name: name.to_string(),
            cpu_frame_ms: percentiles(p95 * 0.5, p95, p95 * 1.1),
            draw_calls: percentiles(2.0, 2.0, 2.0),
            upload_count: percentiles(0.0, 0.0, 0.0),
            upload_bytes: percentiles(0.0, 0.0, 0.0),
            gpu_passes_ms: None,
            per_run_cpu_frame_ms_p95: vec![p95],
            script: Vec::new(),
        }
    }

    fn report(p95: f64) -> FrameBenchReport {
        FrameBenchReport {
            schema: SCHEMA,
            scale: "relative".to_string(),
            git_sha: "abc".to_string(),
            renderer_commit: "abc".to_string(),
            mode: "headless".to_string(),
            resolution: [256, 256],
            warmup_frames: 2,
            measured_frames: 4,
            runs: 1,
            adapter: AdapterInfo::empty(),
            runner: Some(RunnerInfo {
                os: "linux".to_string(),
                cpu_model: "AMD EPYC 7763 64-Core Processor".to_string(),
                icd: IcdInfo {
                    path: Some("/usr/share/vulkan/icd.d/lvp_icd.x86_64.json".to_string()),
                    source: "env".to_string(),
                    var: Some("VK_DRIVER_FILES".to_string()),
                },
                rayon_threads: 2,
                rayon_threads_source: "env".to_string(),
            }),
            timestamp_query: false,
            control_api: ControlApi::unimplemented(),
            scenarios: vec![scenario("unlit_sphere", p95)],
            notes: standard_notes(false),
        }
    }

    #[test]
    fn self_compare_passes() {
        let report = report(12.0);
        let outcome = compare_loaded(&report, &report, &CompareOptions::default()).expect("cmp");
        assert!(!outcome.failed);
        assert!(outcome.markdown.contains("result: pass"));
        assert!(outcome.markdown.contains("not a gate"));
    }

    #[test]
    fn exactly_ten_percent_does_not_fail() {
        let outcome = compare_loaded(&report(100.0), &report(110.0), &CompareOptions::default())
            .expect("cmp");
        assert!(!outcome.failed, "{}", outcome.markdown);
    }

    #[test]
    fn more_than_ten_percent_on_p95_fails() {
        let outcome = compare_loaded(&report(100.0), &report(111.0), &CompareOptions::default())
            .expect("cmp");
        assert!(outcome.failed, "{}", outcome.markdown);
        assert!(outcome.markdown.contains("result: fail"));
        assert!(outcome.markdown.contains("| FAIL |"));
    }

    #[test]
    fn zero_baseline_is_not_a_failure() {
        let outcome =
            compare_loaded(&report(0.0), &report(5.0), &CompareOptions::default()).expect("cmp");
        assert!(!outcome.failed, "{}", outcome.markdown);
        assert!(outcome.markdown.contains("| n/a |"));
    }

    #[test]
    fn null_gpu_does_not_fail_the_cpu_gate() {
        let outcome =
            compare_loaded(&report(10.0), &report(10.0), &CompareOptions::default()).expect("cmp");
        assert!(outcome.markdown.contains("gpu_passes_ms"));
        assert!(outcome.markdown.contains("| null | null |"));
        assert!(!outcome.failed);
    }

    fn schema1(p95: f64) -> FrameBenchReport {
        let mut report = report(p95);
        report.schema = 1;
        report.runner = None;
        report
    }

    #[test]
    fn schema_1_baseline_and_schema_2_candidate_compare() {
        let outcome = compare_loaded(&schema1(10.0), &report(10.0), &CompareOptions::default())
            .expect("mixed schemas");
        assert!(!outcome.failed, "{}", outcome.markdown);
        let outcome = compare_loaded(&report(10.0), &schema1(12.0), &CompareOptions::default())
            .expect("mixed schemas");
        assert!(outcome.failed, "{}", outcome.markdown);
    }

    #[test]
    fn unknown_schema_is_rejected() {
        let mut future = report(10.0);
        future.schema = 3;
        assert!(compare_loaded(&report(10.0), &future, &CompareOptions::default()).is_err());
        let mut zero = report(10.0);
        zero.schema = 0;
        assert!(compare_loaded(&zero, &report(10.0), &CompareOptions::default()).is_err());
    }

    fn floor(ms: f64) -> CompareOptions {
        CompareOptions {
            abs_floor_ms: ms,
            ..CompareOptions::default()
        }
    }

    #[test]
    fn schema_2_without_runner_is_rejected() {
        let mut bare = report(10.0);
        bare.runner = None;
        assert!(compare_loaded(&report(10.0), &bare, &CompareOptions::default()).is_err());
        assert!(compare_loaded(&bare, &report(10.0), &CompareOptions::default()).is_err());
    }

    #[test]
    fn delta_at_or_below_abs_floor_never_fails() {
        // +20% but only 0.04 ms, with the floor opted in at 0.05 ms.
        let outcome = compare_loaded(&report(0.2), &report(0.24), &floor(0.05)).expect("cmp");
        assert!(!outcome.failed, "{}", outcome.markdown);
        assert!(
            outcome.markdown.contains("| ok (abs floor) |"),
            "{}",
            outcome.markdown
        );
        assert!(
            outcome
                .markdown
                .contains("abs_floor: ms deltas <= 0.0500 ms never fail")
        );
        // +50% and exactly 0.05 ms: the floor is inclusive.
        let outcome = compare_loaded(&report(0.1), &report(0.15), &floor(0.05)).expect("cmp");
        assert!(!outcome.failed, "{}", outcome.markdown);
    }

    #[test]
    fn delta_above_abs_floor_still_fails() {
        let outcome = compare_loaded(&report(0.2), &report(0.2501), &floor(0.05)).expect("cmp");
        assert!(outcome.failed, "{}", outcome.markdown);
        let outcome = compare_loaded(&report(2.0), &report(2.4), &floor(0.05)).expect("cmp");
        assert!(outcome.failed, "{}", outcome.markdown);
    }

    #[test]
    fn default_abs_floor_is_off() {
        // O8 is open: the default is relative-only, so +20% at 0.2 ms (0.04 ms) fails.
        assert_eq!(DEFAULT_ABS_FLOOR_MS, 0.0);
        assert_eq!(CompareOptions::default().abs_floor_ms, 0.0);
        let outcome =
            compare_loaded(&report(0.2), &report(0.24), &CompareOptions::default()).expect("cmp");
        assert!(outcome.failed, "{}", outcome.markdown);
        assert!(
            !outcome.markdown.contains("abs_floor"),
            "{}",
            outcome.markdown
        );
        assert!(!outcome.markdown.contains("ok (abs floor)"));
        let outcome = compare_loaded(&report(0.2), &report(0.24), &floor(0.0)).expect("cmp");
        assert!(outcome.failed, "{}", outcome.markdown);
        assert!(compare_loaded(&report(0.2), &report(0.24), &floor(-0.01)).is_err());
        assert!(compare_loaded(&report(0.2), &report(0.24), &floor(f64::NAN)).is_err());
    }

    #[test]
    fn relative_status_matches_threshold_and_floor_vectors() {
        use crate::compare::{RelativeStatus, relative_status};
        // Equality with the threshold passes.
        assert_eq!(
            relative_status(100.0, 110.0, true, 10.0, None).1,
            RelativeStatus::Ok
        );
        assert_eq!(
            relative_status(100.0, 111.0, true, 10.0, None).1,
            RelativeStatus::Fail
        );
        // Abs floor off by default: +20% at 0.04 ms fails.
        assert_eq!(
            relative_status(0.2, 0.24, true, 10.0, Some(0.0)).1,
            RelativeStatus::Fail
        );
        assert_eq!(
            relative_status(0.2, 0.24, true, 10.0, Some(0.05)).1,
            RelativeStatus::OkAbsFloor
        );
        assert_eq!(
            relative_status(0.0, 5.0, true, 10.0, None).1,
            RelativeStatus::Undefined
        );
    }

    #[test]
    fn abs_floor_does_not_apply_to_counts() {
        let mut base = report(10.0);
        let mut cand = report(10.0);
        base.scenarios[0].draw_calls = percentiles(1.0, 1.0, 1.0);
        cand.scenarios[0].draw_calls = percentiles(1.02, 1.02, 1.02);
        let options = CompareOptions {
            gate: vec!["draw_calls.p50".to_string()],
            ..floor(0.05)
        };
        let outcome = compare_loaded(&base, &cand, &options).expect("cmp");
        assert!(
            !outcome.failed,
            "2% is under the threshold: {}",
            outcome.markdown
        );
        cand.scenarios[0].draw_calls = percentiles(1.04, 1.04, 1.04);
        let options = CompareOptions {
            threshold_percent: 3.0,
            ..options
        };
        let outcome = compare_loaded(&base, &cand, &options).expect("cmp");
        assert!(
            outcome.failed,
            "a 0.04 draw delta is not in ms: {}",
            outcome.markdown
        );
    }
}
