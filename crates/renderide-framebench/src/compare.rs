//! Relative compare of two frame-benchmark reports.
//!
//! A gated metric fails only when `(candidate - baseline) / baseline * 100` is greater than
//! the threshold. Equality with the threshold passes. A zero baseline is `n/a` and does not
//! fail. Absolute milliseconds are never a gate.

use crate::error::BenchError;
use crate::report::{FrameBenchReport, SCHEMA, ScenarioReport};
use crate::stats::Percentiles;

/// Default relative threshold, in percent. Fail when the delta is strictly greater.
pub const DEFAULT_THRESHOLD_PERCENT: f64 = 10.0;

/// Default gated metric.
pub const DEFAULT_GATE_METRIC: &str = "cpu_frame_ms.p95";

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
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            threshold_percent: DEFAULT_THRESHOLD_PERCENT,
            gate: vec![DEFAULT_GATE_METRIC.to_string()],
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

/// Loads two schema-1 reports and compares them.
///
/// # Errors
///
/// Returns an error when a file is missing, the schema is not 1, or a gate metric path is unknown.
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
/// Returns an error when a gate metric path is unknown.
pub fn compare_loaded(
    baseline: &FrameBenchReport,
    candidate: &FrameBenchReport,
    options: &CompareOptions,
) -> Result<CompareOutcome, BenchError> {
    if baseline.schema != SCHEMA || candidate.schema != SCHEMA {
        return Err(BenchError::Message(format!(
            "expected schema {SCHEMA}, got baseline {} and candidate {}",
            baseline.schema, candidate.schema
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
    if report.schema != SCHEMA {
        return Err(BenchError::Message(format!(
            "{} has schema {}, expected {SCHEMA}",
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
) -> Row {
    let left = lookup(baseline, scenario, metric);
    let right = lookup(candidate, scenario, metric);
    let (delta, status) = match (left, right) {
        (Lookup::Value(base), Lookup::Value(cand)) => {
            let (_, _, delta, status) = status_for_values(base, cand, gated, threshold);
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

fn status_for_values(
    base: f64,
    cand: f64,
    gated: bool,
    threshold: f64,
) -> (String, String, String, &'static str) {
    let Some(delta) = relative_delta(base, cand) else {
        return (
            format_ms(base),
            format_ms(cand),
            "n/a".to_string(),
            if gated { "n/a" } else { "info" },
        );
    };
    let status = if !gated {
        "info"
    } else if delta > threshold {
        "FAIL"
    } else {
        "ok"
    };
    (
        format_ms(base),
        format_ms(cand),
        format!("{delta:+.2}%"),
        status,
    )
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
                                    status_for_values(base_ms, cand_ms, false, 0.0);
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
    gate: &[String],
    rows: &[Row],
    failed: bool,
) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    out.push_str("# Frame benchmark compare\n\n");
    out.push_str(
        "Absolute milliseconds are not a gate and are not comparable across machines, adapters, or lavapipe versus a discrete GPU. The gate is relative only.\n\n",
    );
    let _ = writeln!(out, "baseline_git: {}", baseline.git_sha);
    let _ = writeln!(out, "candidate_git: {}", candidate.git_sha);
    let _ = writeln!(out, "threshold: >{threshold:.4}% on gated metrics");
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
    use crate::compare::{CompareOptions, compare_loaded};
    use crate::report::{
        AdapterInfo, ControlApi, FrameBenchReport, SCHEMA, ScenarioReport, standard_notes,
    };
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
}
