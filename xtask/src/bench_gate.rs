//! Faithful Rust port of `scripts/ci/frame_bench_gate.py`.
#![allow(
    clippy::too_many_arguments,
    reason = "mirrors Python evaluate_confirm / decide signatures"
)]

use clap::Parser;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const SUPPORTED_SCHEMAS: [i64; 2] = [1, 2];
pub const DEFAULT_ALPHA: f64 = 0.05;
pub const DEFAULT_ABS_FLOOR_MS: f64 = 0.0;
pub const ABS_FLOOR_EPSILON_MS: f64 = 1e-9;
pub const EXACT_MAX_COMBINATIONS: u128 = 20_000;
pub const CONFIRM_MIN_N: usize = 12;
pub const BASE_DRIFT_PCT: f64 = 15.0;

#[derive(Debug, Clone)]
pub struct Row {
    pub scenario: String,
    pub base: Option<f64>,
    pub head: Option<f64>,
    pub delta: Option<f64>,
    pub p_value: Option<f64>,
    pub n_base: usize,
    pub n_head: usize,
    pub status: String,
    pub confirm_delta: Option<f64>,
    pub confirm_p: Option<f64>,
    pub confirm_n_base: usize,
    pub confirm_n_head: usize,
    pub base_drift_pct: Option<f64>,
    pub confirm_round: i32,
    pub deciding_round: Option<i32>,
}

impl Row {
    fn basic(
        scenario: String,
        base: Option<f64>,
        head: Option<f64>,
        delta: Option<f64>,
        p_value: Option<f64>,
        n_base: usize,
        n_head: usize,
        status: String,
    ) -> Self {
        Self {
            scenario,
            base,
            head,
            delta,
            p_value,
            n_base,
            n_head,
            status,
            confirm_delta: None,
            confirm_p: None,
            confirm_n_base: 0,
            confirm_n_head: 0,
            base_drift_pct: None,
            confirm_round: 1,
            deciding_round: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecideResult {
    pub verdict: String,
    pub reason: String,
    pub deciding_round: Option<i32>,
}

impl DecideResult {
    pub fn new(verdict: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            verdict: verdict.into(),
            reason: reason.into(),
            deciding_round: None,
        }
    }

    pub fn with_round(
        verdict: impl Into<String>,
        reason: impl Into<String>,
        deciding_round: Option<i32>,
    ) -> Self {
        Self {
            verdict: verdict.into(),
            reason: reason.into(),
            deciding_round,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InputError(pub String);

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<InputError> for String {
    fn from(e: InputError) -> Self {
        e.0
    }
}

/// CLI args mirroring `frame_bench_gate.py`.
#[derive(Debug, Parser)]
pub struct Args {
    /// First-pass base report; repeatable.
    #[arg(long = "base", value_name = "PATH")]
    pub base: Vec<PathBuf>,
    /// First-pass head report; repeatable.
    #[arg(long = "head", value_name = "PATH")]
    pub head: Vec<PathBuf>,
    /// Confirm-round base report.
    #[arg(long = "confirm-base", value_name = "PATH")]
    pub confirm_base: Vec<PathBuf>,
    /// Confirm-round head report.
    #[arg(long = "confirm-head", value_name = "PATH")]
    pub confirm_head: Vec<PathBuf>,
    /// Percent; first pass flags when delta > this.
    #[arg(long = "flag-threshold", default_value_t = 5.0)]
    pub flag_threshold: f64,
    /// Percent; the policy bar.
    #[arg(long = "fail-threshold", default_value_t = 10.0)]
    pub fail_threshold: f64,
    /// Deprecated alias of --fail-threshold.
    #[arg(long = "threshold")]
    pub legacy_threshold: Option<f64>,
    /// first pass or confirm.
    #[arg(long = "mode", value_parser = ["first", "confirm"], default_value = "first")]
    pub mode: String,
    /// Confirm: treat base drift as FAIL (inconclusive: base drift).
    #[arg(long = "fail-closed-drift")]
    pub fail_closed_drift: bool,
    /// Confirm attempt number recorded on rows.
    #[arg(long = "confirm-round", default_value_t = 1)]
    pub confirm_round: i32,
    /// Confirm mode: write scenario names with INCONCLUSIVE (base drift).
    #[arg(long = "drift-out")]
    pub drift_out: Option<PathBuf>,
    /// Confirm: Mann-Whitney one-sided level.
    #[arg(long = "alpha", default_value_t = DEFAULT_ALPHA)]
    pub alpha: f64,
    /// ms; head-minus-base median delta at or below this never flags or fails.
    #[arg(long = "abs-floor-ms", default_value_t = DEFAULT_ABS_FLOOR_MS)]
    pub abs_floor_ms: f64,
    /// Requested scenario head does not define.
    #[arg(long = "head-missing")]
    pub head_missing: Vec<String>,
    #[arg(long = "title", default_value = "Frame bench gate")]
    pub title: String,
    /// First mode: write flagged scenario names, one per line.
    #[arg(long = "suspects-out")]
    pub suspects_out: Option<PathBuf>,
    /// Optional: write machine-readable rows as JSON.
    #[arg(long = "json-out")]
    pub json_out: Option<PathBuf>,
}

pub fn run(mut args: Args) -> i32 {
    if let Some(legacy) = args.legacy_threshold {
        eprintln!(
            "frame_bench_gate: warning: --threshold is deprecated; using it as --fail-threshold {legacy}"
        );
        args.fail_threshold = legacy;
        if args.flag_threshold > args.fail_threshold {
            eprintln!(
                "frame_bench_gate: warning: --flag-threshold {} > --fail-threshold {}; lowering --flag-threshold",
                args.flag_threshold, args.fail_threshold
            );
            args.flag_threshold = args.fail_threshold;
        }
    }
    if args.head.is_empty() {
        eprintln!("frame_bench_gate: at least one --head report is required");
        return 2;
    }
    if !(0.0 < args.alpha && args.alpha < 1.0) {
        eprintln!("frame_bench_gate: --alpha must be in (0, 1)");
        return 2;
    }
    if !(args.abs_floor_ms.is_finite() && args.abs_floor_ms >= 0.0) {
        eprintln!("frame_bench_gate: --abs-floor-ms must be a finite number >= 0");
        return 2;
    }
    if !(0.0 <= args.flag_threshold && args.flag_threshold <= args.fail_threshold) {
        eprintln!("frame_bench_gate: need 0 <= --flag-threshold <= --fail-threshold");
        return 2;
    }
    if args.mode == "first" && (!args.confirm_base.is_empty() || !args.confirm_head.is_empty()) {
        eprintln!("frame_bench_gate: --confirm-base/--confirm-head need --mode confirm");
        return 2;
    }

    let (base, head, confirm_base, confirm_head) = match (|| -> Result<_, String> {
        let base = load(&args.base, false)?;
        let head = load(&args.head, true)?;
        let confirm_base = load(&args.confirm_base, false)?;
        let confirm_head = load(&args.confirm_head, true)?;
        check_base_medians("first-pass", &base)?;
        if args.mode == "confirm" {
            check_base_medians("confirm-round", &confirm_base)?;
            let mut pooled_base = base.clone();
            for (k, v) in &confirm_base {
                pooled_base
                    .entry(k.clone())
                    .or_default()
                    .extend(v.iter().copied());
            }
            check_base_medians("pooled", &pooled_base)?;
        }
        Ok((base, head, confirm_base, confirm_head))
    })() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("frame_bench_gate: {err}");
            return 2;
        }
    };

    let rows = if args.mode == "confirm" {
        evaluate_confirm(
            &base,
            &head,
            &confirm_base,
            &confirm_head,
            args.fail_threshold,
            &args.head_missing,
            args.alpha,
            args.abs_floor_ms,
            CONFIRM_MIN_N,
            BASE_DRIFT_PCT,
            args.fail_closed_drift,
            args.confirm_round,
        )
    } else {
        evaluate_first(
            &base,
            &head,
            args.flag_threshold,
            &args.head_missing,
            args.abs_floor_ms,
        )
    };

    let mut runners = Vec::new();
    if let Some(line) = runner_line("base", &args.base) {
        runners.push(line);
    }
    if let Some(line) = runner_line("head", &args.head) {
        runners.push(line);
    }
    let rendered = render(
        &rows,
        &args.title,
        args.flag_threshold,
        args.fail_threshold,
        &args.mode,
        args.alpha,
        args.abs_floor_ms,
        &runners,
    );
    let _ = io::stdout().write_all(rendered.as_bytes());

    let hit: Vec<String> = rows
        .iter()
        .filter(|r| is_failure(&r.status))
        .map(|r| r.scenario.clone())
        .collect();
    if let Some(path) = &args.suspects_out {
        let body = hit.iter().fold(String::new(), |mut s, n| {
            use std::fmt::Write as _;
            let _ = writeln!(s, "{n}");
            s
        });
        if let Err(e) = fs::write(path, body) {
            eprintln!("frame_bench_gate: {e}");
            return 2;
        }
    }
    if let Some(path) = &args.drift_out {
        let drifted: Vec<_> = rows
            .iter()
            .filter(|r| is_inconclusive_drift(&r.status))
            .map(|r| r.scenario.clone())
            .collect();
        let body = drifted.iter().fold(String::new(), |mut s, n| {
            use std::fmt::Write as _;
            let _ = writeln!(s, "{n}");
            s
        });
        if let Err(e) = fs::write(path, body) {
            eprintln!("frame_bench_gate: {e}");
            return 2;
        }
    }
    if let Some(path) = &args.json_out {
        let payload = gate_json_payload(
            &args.mode,
            args.flag_threshold,
            args.fail_threshold,
            args.alpha,
            args.abs_floor_ms,
            (args.mode == "confirm").then_some(args.confirm_round),
            &rows,
            i32::from(!hit.is_empty()),
        );
        if let Err(e) = write_json(path, &payload) {
            eprintln!("frame_bench_gate: {e}");
            return 2;
        }
    }
    let word = if args.mode == "confirm" {
        "fail"
    } else {
        "flagged"
    };
    if hit.is_empty() {
        println!("result: pass");
        0
    } else {
        println!("result: {word} ({})", hit.join(", "));
        1
    }
}

pub fn gate_json_payload(
    mode: &str,
    flag_threshold: f64,
    fail_threshold: f64,
    alpha: f64,
    abs_floor_ms: f64,
    confirm_round: Option<i32>,
    rows: &[Row],
    exit_code: i32,
) -> Value {
    let row_vals: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "scenario": r.scenario,
                "base": r.base,
                "head": r.head,
                "delta_pct": r.delta,
                "p_value": r.p_value,
                "n_base": r.n_base,
                "n_head": r.n_head,
                "confirm_delta_pct": r.confirm_delta,
                "confirm_p": r.confirm_p,
                "confirm_n_base": r.confirm_n_base,
                "confirm_n_head": r.confirm_n_head,
                "base_drift_pct": r.base_drift_pct,
                "confirm_round": r.confirm_round,
                "deciding_round": r.deciding_round,
                "status": r.status,
            })
        })
        .collect();
    json!({
        "schema": "frame-bench-gate/1",
        "mode": mode,
        "flag_threshold": flag_threshold,
        "fail_threshold": fail_threshold,
        "alpha": alpha,
        "abs_floor_ms": abs_floor_ms,
        "base_drift_pct_threshold": BASE_DRIFT_PCT,
        "confirm_round": confirm_round,
        "rows": row_vals,
        "exit_code": exit_code,
    })
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    text.push('\n');
    fs::write(path, text)
}

pub fn load(
    paths: &[PathBuf],
    require_scenarios: bool,
) -> Result<BTreeMap<String, Vec<f64>>, String> {
    let mut pooled: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for path in paths {
        let report = read_report(path)?;
        let scenarios = report
            .get("scenarios")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if require_scenarios && scenarios.is_empty() {
            return Err(format!(
                "{}: head report has zero scenarios",
                path.display()
            ));
        }
        for scenario in scenarios {
            let name = scenario
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{}: scenario missing name", path.display()))?
                .to_string();
            let samples = if let Some(arr) = scenario
                .get("per_run_cpu_frame_ms_p95")
                .and_then(|v| v.as_array())
            {
                arr.iter()
                    .map(|v| {
                        v.as_f64()
                            .ok_or_else(|| format!("{}: non-float sample", path.display()))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                let p95 = scenario
                    .pointer("/cpu_frame_ms/p95")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        format!(
                            "{}: scenario {name:?} missing cpu_frame_ms.p95",
                            path.display()
                        )
                    })?;
                vec![p95]
            };
            for v in &samples {
                if !v.is_finite() {
                    return Err(format!(
                        "{}: scenario {name:?} has a non-finite value ({v:?})",
                        path.display()
                    ));
                }
            }
            pooled.entry(name).or_default().extend(samples);
        }
    }
    Ok(pooled)
}

pub fn read_report(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let report: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let obj = report
        .as_object()
        .ok_or_else(|| format!("{}: report is not a JSON object", path.display()))?;
    if obj.contains_key("error") {
        let empty = json!({});
        let error = obj.get("error").cloned().unwrap_or(empty);
        return Err(format!(
            "{}: framebench error document (exit {:?}, {:?}): {:?}",
            path.display(),
            error.get("exit_code"),
            error.get("kind"),
            error.get("message")
        ));
    }
    let schema = obj.get("schema");
    let schema_ok = match schema {
        Some(Value::Number(n)) if n.as_i64() == Some(1) || n.as_i64() == Some(2) => true,
        // reject bool (serde may parse true as bool, not number)
        Some(Value::Bool(_)) => false,
        _ => false,
    };
    if !schema_ok {
        return Err(format!(
            "{}: schema {:?}, expected one of {:?}",
            path.display(),
            schema,
            SUPPORTED_SCHEMAS
        ));
    }
    let schema_n = schema.and_then(|v| v.as_i64()).unwrap_or(0);
    if schema_n >= 2 && !obj.get("runner").map(|v| v.is_object()).unwrap_or(false) {
        return Err(format!(
            "{}: schema {schema_n} report has no `runner` object",
            path.display()
        ));
    }
    Ok(report)
}

pub fn runner_line(label: &str, paths: &[PathBuf]) -> Option<String> {
    if let Some(path) = paths.iter().next() {
        let report = read_report(path).ok()?;
        let runner = report.get("runner")?.as_object()?;
        let icd = runner.get("icd").and_then(|v| v.as_object());
        let icd_text = icd
            .and_then(|i| i.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let source = icd
            .and_then(|i| i.get("var").or_else(|| i.get("source")))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let os = runner.get("os").and_then(|v| v.as_str()).unwrap_or("?");
        let cpu = runner
            .get("cpu_model")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let rayon = runner
            .get("rayon_threads")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into());
        let rayon_src = runner
            .get("rayon_threads_source")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        return Some(format!(
            "{label} runner: os={os} cpu=\"{cpu}\" icd={icd_text} ({source}) rayon_threads={rayon} ({rayon_src})"
        ));
    }
    None
}

pub fn median(values: &[f64]) -> f64 {
    // Match Python statistics.median: sort copy; odd → middle; even → mean of two middle.
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    assert!(n > 0);
    if n % 2 == 1 {
        v[n / 2]
    } else {
        f64::midpoint(v[n / 2 - 1], v[n / 2])
    }
}

pub fn below_abs_floor(base: &[f64], head: &[f64], abs_floor_ms: f64) -> bool {
    median(head) - median(base) <= abs_floor_ms + ABS_FLOOR_EPSILON_MS
}

pub fn check_base_medians(label: &str, base: &BTreeMap<String, Vec<f64>>) -> Result<(), String> {
    for (name, samples) in base {
        if !samples.is_empty() && median(samples) <= 0.0 {
            return Err(format!(
                "{label} base median for {name:?} is {:?}; must be > 0",
                median(samples)
            ));
        }
    }
    Ok(())
}

pub fn delta_percent(base: &[f64], head: &[f64]) -> Option<f64> {
    let base_value = median(base);
    if base_value <= 0.0 {
        return None;
    }
    Some((median(head) - base_value) / base_value * 100.0)
}

fn mid_ranks(values: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&i, &j| {
        values[i]
            .partial_cmp(&values[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0; values.len()];
    let mut ties = Vec::new();
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && values[order[j + 1]].to_bits() == values[order[i]].to_bits() {
            j += 1;
        }
        let rank = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            ranks[order[k]] = rank;
        }
        ties.push(j - i + 1);
        i = j + 1;
    }
    (ranks, ties)
}

fn comb_u128(n: u128, k: u128) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut acc = 1u128;
    for i in 0..k {
        acc = acc * (n - i) / (i + 1);
    }
    acc
}

/// One-sided Mann-Whitney U p-value for H1: head stochastically greater than base.
pub fn mann_whitney_greater(base: &[f64], head: &[f64]) -> f64 {
    let n_base = base.len();
    let n_head = head.len();
    if n_base == 0 || n_head == 0 {
        return 1.0;
    }
    let mut pooled = Vec::with_capacity(n_base + n_head);
    pooled.extend_from_slice(base);
    pooled.extend_from_slice(head);
    let (ranks, ties) = mid_ranks(&pooled);
    let offset = n_head as f64 * (n_head as f64 + 1.0) / 2.0;
    let u_obs: f64 = ranks[n_base..].iter().sum::<f64>() - offset;
    let n = n_base + n_head;
    if comb_u128(n as u128, n_head as u128) <= EXACT_MAX_COMBINATIONS {
        let mut hits = 0u128;
        let mut total = 0u128;
        let tolerance = 1e-9;
        // Enumerate combinations of ranks choosing n_head.
        let mut chosen = vec![0usize; n_head];
        fn enumerate(
            ranks: &[f64],
            n_head: usize,
            start: usize,
            depth: usize,
            chosen: &mut [usize],
            offset: f64,
            u_obs: f64,
            tolerance: f64,
            hits: &mut u128,
            total: &mut u128,
        ) {
            if depth == n_head {
                *total += 1;
                let sum: f64 = chosen.iter().map(|&i| ranks[i]).sum();
                if sum - offset >= u_obs - tolerance {
                    *hits += 1;
                }
                return;
            }
            // Need enough remaining elements.
            let remaining_slots = n_head - depth;
            for i in start..=ranks.len() - remaining_slots {
                chosen[depth] = i;
                enumerate(
                    ranks,
                    n_head,
                    i + 1,
                    depth + 1,
                    chosen,
                    offset,
                    u_obs,
                    tolerance,
                    hits,
                    total,
                );
            }
        }
        enumerate(
            &ranks,
            n_head,
            0,
            0,
            &mut chosen,
            offset,
            u_obs,
            tolerance,
            &mut hits,
            &mut total,
        );
        return hits as f64 / total as f64;
    }
    let mean = n_base as f64 * n_head as f64 / 2.0;
    let tie_term = ties
        .iter()
        .map(|&t| {
            let t = t as f64;
            t * t * t - t
        })
        .sum::<f64>()
        / (n as f64 * (n as f64 - 1.0));
    let variance = n_base as f64 * n_head as f64 / 12.0 * ((n as f64 + 1.0) - tie_term);
    if variance <= 0.0 {
        return 1.0;
    }
    let z = (u_obs - mean - 0.5) / variance.sqrt();
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

fn erfc(x: f64) -> f64 {
    // libm not always available; use std via no_std? Rust 1.99 has f64::erf in std?
    // Use complementary error function via libm or approximate.
    // Rust std: `x.cosh()` etc exist; erf is unstable. Use a well-known approximation.
    // Actually we can use `libm` crate - avoid new dep. Port from Python math.erfc.
    // Cephes-style erfc approximation:
    erfc_approx(x)
}

fn erfc_approx(x: f64) -> f64 {
    // Cody approximation for erfc; accurate enough for MW p-values.
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let ans = t
        * (-z.powi(2) - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 { ans } else { 2.0 - ans }
}

fn missing_rows(
    base: &BTreeMap<String, Vec<f64>>,
    head: &BTreeMap<String, Vec<f64>>,
    head_missing: &[String],
) -> (Vec<String>, Vec<Row>) {
    let mut both = Vec::new();
    let mut rows = Vec::new();
    let names: BTreeSet<_> = base.keys().chain(head.keys()).cloned().collect();
    for name in names {
        let head_samples = head.get(&name).cloned().unwrap_or_default();
        let base_samples = base.get(&name).cloned().unwrap_or_default();
        if head_samples.is_empty() {
            rows.push(Row::basic(
                name,
                Some(median(&base_samples)),
                None,
                None,
                None,
                base_samples.len(),
                0,
                "head N/A, skipped".into(),
            ));
        } else if base_samples.is_empty() {
            rows.push(Row::basic(
                name,
                None,
                Some(median(&head_samples)),
                None,
                None,
                0,
                head_samples.len(),
                "base N/A".into(),
            ));
        } else {
            both.push(name);
        }
    }
    let known: BTreeSet<_> = base.keys().chain(head.keys()).cloned().collect();
    let mut extra: Vec<_> = head_missing
        .iter()
        .filter(|n| !known.contains(*n))
        .cloned()
        .collect();
    extra.sort();
    for name in extra {
        rows.push(Row::basic(
            name,
            None,
            None,
            None,
            None,
            0,
            0,
            "head N/A, skipped".into(),
        ));
    }
    (both, rows)
}

fn order_rows(rows: Vec<Row>) -> Vec<Row> {
    let mut na_head = Vec::new();
    let mut rest = Vec::new();
    for r in rows {
        if r.status == "head N/A, skipped" && r.base.is_none() && r.n_base == 0 {
            na_head.push(r);
        } else {
            rest.push(r);
        }
    }
    rest.sort_by(|a, b| a.scenario.cmp(&b.scenario));
    na_head.sort_by(|a, b| a.scenario.cmp(&b.scenario));
    rest.extend(na_head);
    rest
}

pub fn evaluate_first(
    base: &BTreeMap<String, Vec<f64>>,
    head: &BTreeMap<String, Vec<f64>>,
    flag_threshold: f64,
    head_missing: &[String],
    abs_floor_ms: f64,
) -> Vec<Row> {
    let (both, mut rows) = missing_rows(base, head, head_missing);
    for name in both {
        let b = &base[&name];
        let h = &head[&name];
        let delta = delta_percent(b, h);
        let status = match delta {
            None => "n/a".to_string(),
            Some(d) if d <= flag_threshold => "ok".into(),
            Some(_) if below_abs_floor(b, h, abs_floor_ms) => "ok (abs floor)".into(),
            Some(_) => "FLAG".into(),
        };
        rows.push(Row::basic(
            name,
            Some(median(b)),
            Some(median(h)),
            delta,
            None,
            b.len(),
            h.len(),
            status,
        ));
    }
    order_rows(rows)
}

fn significant_regression(
    base: &[f64],
    head: &[f64],
    fail_threshold: f64,
    alpha: f64,
    abs_floor_ms: f64,
) -> (Option<f64>, Option<f64>, bool, bool) {
    if base.is_empty() || head.is_empty() {
        return (None, None, false, false);
    }
    let delta = delta_percent(base, head);
    let p_value = mann_whitney_greater(base, head);
    let relative = delta.is_some_and(|d| d > fail_threshold) && p_value < alpha;
    let floored = relative && below_abs_floor(base, head, abs_floor_ms);
    (delta, Some(p_value), relative && !floored, floored)
}

pub fn base_drift_percent(first_base: &[f64], confirm_base: &[f64]) -> Option<f64> {
    if first_base.is_empty() || confirm_base.is_empty() {
        return None;
    }
    let fb = median(first_base);
    if fb <= 0.0 {
        return None;
    }
    let cb = median(confirm_base);
    Some((cb - fb).abs() / fb * 100.0)
}

pub fn evaluate_confirm(
    first_base: &BTreeMap<String, Vec<f64>>,
    first_head: &BTreeMap<String, Vec<f64>>,
    confirm_base: &BTreeMap<String, Vec<f64>>,
    confirm_head: &BTreeMap<String, Vec<f64>>,
    fail_threshold: f64,
    head_missing: &[String],
    alpha: f64,
    abs_floor_ms: f64,
    confirm_min_n: usize,
    drift_threshold: f64,
    fail_closed_drift: bool,
    confirm_round: i32,
) -> Vec<Row> {
    let mut pooled_base = first_base.clone();
    let mut pooled_head = first_head.clone();
    for (k, v) in confirm_base {
        pooled_base
            .entry(k.clone())
            .or_default()
            .extend(v.iter().copied());
    }
    for (k, v) in confirm_head {
        pooled_head
            .entry(k.clone())
            .or_default()
            .extend(v.iter().copied());
    }
    let (both, mut rows) = missing_rows(&pooled_base, &pooled_head, head_missing);
    for name in both {
        let b = &pooled_base[&name];
        let h = &pooled_head[&name];
        let (delta, p_value, _pooled_fail, _pooled_floor) =
            significant_regression(b, h, fail_threshold, alpha, abs_floor_ms);
        let cb = confirm_base.get(&name).cloned().unwrap_or_default();
        let ch = confirm_head.get(&name).cloned().unwrap_or_default();
        let fb_samples = first_base.get(&name).cloned().unwrap_or_default();
        let drift = base_drift_percent(&fb_samples, &cb);
        let (c_delta, c_p, confirm_sig, confirm_floor) =
            significant_regression(&cb, &ch, fail_threshold, alpha, abs_floor_ms);
        let n_ok = cb.len() >= confirm_min_n && ch.len() >= confirm_min_n;
        let drifted = drift.is_some_and(|d| d > drift_threshold);
        let (status, deciding) = if delta.is_none() {
            ("n/a".to_string(), None)
        } else if drifted {
            (
                if fail_closed_drift {
                    "FAIL (inconclusive: base drift)".into()
                } else {
                    "INCONCLUSIVE (base drift)".into()
                },
                None,
            )
        } else if confirm_sig && n_ok {
            ("FAIL (confirm-only)".into(), Some(confirm_round))
        } else if confirm_sig && !n_ok {
            ("ok (confirm n<12)".into(), Some(confirm_round))
        } else if confirm_floor && n_ok {
            ("ok (abs floor)".into(), Some(confirm_round))
        } else {
            ("ok".into(), Some(confirm_round))
        };
        rows.push(Row {
            scenario: name,
            base: Some(median(b)),
            head: Some(median(h)),
            delta,
            p_value,
            n_base: b.len(),
            n_head: h.len(),
            status,
            confirm_delta: c_delta,
            confirm_p: c_p,
            confirm_n_base: cb.len(),
            confirm_n_head: ch.len(),
            base_drift_pct: drift,
            confirm_round,
            deciding_round: deciding,
        });
    }
    order_rows(rows)
}

pub fn is_failure(status: &str) -> bool {
    status.starts_with("FAIL") || status == "FLAG" || status.starts_with("INCONCLUSIVE")
}

pub fn is_inconclusive_drift(status: &str) -> bool {
    status.starts_with("INCONCLUSIVE (base drift)")
        || status.starts_with("FAIL (inconclusive: base drift)")
}

pub fn is_unstable_runner(status: &str) -> bool {
    status.starts_with("FAIL (inconclusive: unstable runner)")
}

pub fn confirm_status_kind(status: &str) -> &'static str {
    if is_unstable_runner(status) {
        "unstable"
    } else if is_inconclusive_drift(status) {
        "drift"
    } else if status.starts_with("FAIL") {
        "fail"
    } else {
        "pass"
    }
}

pub fn format_summary_verdict(action: &str, reason: &str) -> String {
    let action = action.trim();
    let reason = reason.trim();
    let mut line = match action {
        "pass" => "gate_action=pass".to_string(),
        "fail" => "gate_action=fail (perf)".to_string(),
        "need_confirm" => {
            "gate_action=need_confirm (need second consecutive drift-clean FAIL)".to_string()
        }
        "rerun" => {
            "gate_action=rerun (disagreement or recurring drift → full gate rerun once)".to_string()
        }
        "unstable" => "gate_action=unstable → inconclusive: unstable runner".to_string(),
        "" => "gate_action=unknown".to_string(),
        other => format!("gate_action={other}"),
    };
    if !reason.is_empty() {
        line = format!("{line}; verdict_reason={reason}");
    }
    line
}

pub fn post_drift_policy(round_kinds: &[&str], after_rerun: bool) -> DecideResult {
    if round_kinds.is_empty() {
        return DecideResult::new("pass", "no confirm rounds");
    }
    if round_kinds[0] != "drift" {
        if round_kinds[0] == "fail" {
            return DecideResult::with_round("fail", "confirm-only FAIL (no drift)", Some(1));
        }
        return DecideResult::with_round("pass", "confirm clear (no drift)", Some(1));
    }
    let mut clean_fail_streak = 0;
    let mut last_clean_round: Option<i32> = None;
    let mut saw_clean_fail = false;
    for (idx, kind) in round_kinds.iter().enumerate() {
        let round_no = (idx + 1) as i32;
        if *kind == "drift" {
            if round_no == 1 {
                continue;
            }
            if after_rerun {
                return DecideResult::with_round(
                    "unstable",
                    format!(
                        "recurring base drift on confirm round {round_no} after full-gate rerun"
                    ),
                    None,
                );
            }
            return DecideResult::with_round(
                "rerun",
                format!("recurring base drift on confirm round {round_no}"),
                None,
            );
        }
        if *kind == "fail" {
            if saw_clean_fail && last_clean_round == Some(round_no - 1) {
                return DecideResult::with_round(
                    "fail",
                    format!(
                        "two consecutive drift-clean confirm-only FAILs (rounds {}+{round_no})",
                        round_no - 1
                    ),
                    Some(round_no),
                );
            }
            saw_clean_fail = true;
            clean_fail_streak = 1;
            last_clean_round = Some(round_no);
            continue;
        }
        // pass
        if let Some(prev) = last_clean_round
            && saw_clean_fail
            && prev == round_no - 1
        {
            if after_rerun {
                return DecideResult::with_round(
                    "unstable",
                    format!(
                        "disagreement after drift (FAIL round {prev}, ok round {round_no}) after full-gate rerun"
                    ),
                    None,
                );
            }
            return DecideResult::with_round(
                "rerun",
                format!("disagreement after drift (FAIL round {prev}, ok round {round_no})"),
                None,
            );
        }
        return DecideResult::with_round(
            "pass",
            format!("drift-clean ok on confirm round {round_no}"),
            Some(round_no),
        );
    }
    if saw_clean_fail && clean_fail_streak == 1 {
        let round = last_clean_round.unwrap_or(0);
        return DecideResult::with_round(
            "need_confirm",
            format!(
                "one drift-clean FAIL on round {round}; need a second consecutive drift-clean confirm"
            ),
            None,
        );
    }
    DecideResult::with_round("fail", "base drift with no extra confirm supplied", None)
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "exercised by unit tests and future callers")
)]
pub fn decide(
    first_base: &[f64],
    first_head: &[f64],
    confirm_base: &[Vec<f64>],
    confirm_head: &[Vec<f64>],
    flag_threshold: f64,
    fail_threshold: f64,
    alpha: f64,
    abs_floor_ms: f64,
    confirm_min_n: usize,
    confirm_base_2: Option<&[Vec<f64>]>,
    confirm_head_2: Option<&[Vec<f64>]>,
    confirm_base_3: Option<&[Vec<f64>]>,
    confirm_head_3: Option<&[Vec<f64>]>,
    drift_threshold: f64,
    after_rerun: bool,
) -> String {
    decide_detail(
        first_base,
        first_head,
        confirm_base,
        confirm_head,
        flag_threshold,
        fail_threshold,
        alpha,
        abs_floor_ms,
        confirm_min_n,
        confirm_base_2,
        confirm_head_2,
        confirm_base_3,
        confirm_head_3,
        drift_threshold,
        after_rerun,
    )
    .verdict
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "exercised by unit tests and future callers")
)]
pub fn decide_detail(
    first_base: &[f64],
    first_head: &[f64],
    confirm_base: &[Vec<f64>],
    confirm_head: &[Vec<f64>],
    flag_threshold: f64,
    fail_threshold: f64,
    alpha: f64,
    abs_floor_ms: f64,
    confirm_min_n: usize,
    confirm_base_2: Option<&[Vec<f64>]>,
    confirm_head_2: Option<&[Vec<f64>]>,
    confirm_base_3: Option<&[Vec<f64>]>,
    confirm_head_3: Option<&[Vec<f64>]>,
    drift_threshold: f64,
    after_rerun: bool,
) -> DecideResult {
    let mut fb_map = BTreeMap::new();
    fb_map.insert("s".into(), first_base.to_vec());
    let mut fh_map = BTreeMap::new();
    fh_map.insert("s".into(), first_head.to_vec());

    if confirm_base.is_empty() && confirm_head.is_empty() {
        let first = evaluate_first(&fb_map, &fh_map, fail_threshold, &[], abs_floor_ms);
        let first = &first[0];
        if first.status == "FLAG" {
            return DecideResult::new("fail", "confirm_rounds=0 first-pass FAIL");
        }
        return DecideResult::new("pass", "confirm_rounds=0 first-pass clear");
    }
    let first = evaluate_first(&fb_map, &fh_map, flag_threshold, &[], abs_floor_ms);
    if first[0].status != "FLAG" {
        return DecideResult::new("pass", "first pass not flagged");
    }

    let mut rounds: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
    let flatten =
        |runs: &[Vec<f64>]| -> Vec<f64> { runs.iter().flat_map(|r| r.iter().copied()).collect() };
    rounds.push((flatten(confirm_base), flatten(confirm_head)));
    if let (Some(cb2), Some(ch2)) = (confirm_base_2, confirm_head_2) {
        rounds.push((flatten(cb2), flatten(ch2)));
    }
    if let (Some(cb3), Some(ch3)) = (confirm_base_3, confirm_head_3) {
        rounds.push((flatten(cb3), flatten(ch3)));
    }

    let mut kinds = Vec::new();
    for (i, (cb, ch)) in rounds.iter().enumerate() {
        let round_no = (i + 1) as i32;
        let mut cb_map = BTreeMap::new();
        cb_map.insert("s".into(), cb.clone());
        let mut ch_map = BTreeMap::new();
        ch_map.insert("s".into(), ch.clone());
        let row = evaluate_confirm(
            &fb_map,
            &fh_map,
            &cb_map,
            &ch_map,
            fail_threshold,
            &[],
            alpha,
            abs_floor_ms,
            confirm_min_n,
            drift_threshold,
            false,
            round_no,
        );
        kinds.push(confirm_status_kind(&row[0].status));
    }
    post_drift_policy(&kinds, after_rerun)
}

fn fmt_ms(value: Option<f64>) -> String {
    match value {
        None => "n/a".into(),
        Some(v) => format!("{v:.4}"),
    }
}
fn fmt_pct(value: Option<f64>) -> String {
    match value {
        None => "n/a".into(),
        Some(v) => format!("{v:+.2}%"),
    }
}
fn fmt_p(value: Option<f64>) -> String {
    match value {
        None => "n/a".into(),
        Some(v) => format!("{v:.4}"),
    }
}

pub fn render(
    rows: &[Row],
    title: &str,
    flag_threshold: f64,
    fail_threshold: f64,
    mode: &str,
    alpha: f64,
    abs_floor_ms: f64,
    runner_lines: &[String],
) -> String {
    let mut lines = vec![format!("### {title}"), String::new()];
    let floor_text = if abs_floor_ms > 0.0 {
        format!(
            " Absolute floor: a head-minus-base median delta <= {abs_floor_ms} ms never flags or fails."
        )
    } else {
        String::new()
    };
    if mode == "confirm" {
        lines.push(format!(
            "Gate: cpu_frame_ms.p95 (median of per-run p95). Confirm: FAIL only when confirm-rounds-only \
delta > {fail_threshold}% AND one-sided Mann-Whitney U p < {alpha} (head slower) with \
n>={CONFIRM_MIN_N} samples per side. Pooled delta/p are reported but do not decide FAIL. \
Confirm-base median drift > {BASE_DRIFT_PCT}% vs first-pass base → inconclusive. \
After drift: FAIL only if two consecutive drift-clean confirms both FAIL; \
disagreement/recurring drift → full gate rerun once; still unstable → \
inconclusive: unstable runner.{floor_text}"
        ));
        lines.push(String::new());
        lines.push(
            "| scenario | base p95 ms | head p95 ms | pooled delta | pooled runs base/head | pooled MW p \
| confirm-only delta | confirm runs base/head | confirm-only MW p | base drift % \
| confirm round | deciding round | status |"
                .into(),
        );
        lines.push("|---|---|---|---|---|---|---|---|---|---|---|---|---|".into());
        for r in rows {
            let drift_s = match r.base_drift_pct {
                None => "n/a".into(),
                Some(v) => format!("{v:.2}%"),
            };
            let dec_s = match r.deciding_round {
                None => "n/a".into(),
                Some(v) => v.to_string(),
            };
            lines.push(format!(
                "| {} | {} | {} | {} | {}/{} | {} | {} | {}/{} | {} | {} | {} | {} | {} |",
                r.scenario,
                fmt_ms(r.base),
                fmt_ms(r.head),
                fmt_pct(r.delta),
                r.n_base,
                r.n_head,
                fmt_p(r.p_value),
                fmt_pct(r.confirm_delta),
                r.confirm_n_base,
                r.confirm_n_head,
                fmt_p(r.confirm_p),
                drift_s,
                r.confirm_round,
                dec_s,
                r.status
            ));
        }
    } else {
        lines.push(format!(
            "Gate: cpu_frame_ms.p95 (median of per-run p95). First pass: FLAG when delta > {flag_threshold}% \
(no spread condition); flags are re-measured and fail only above {fail_threshold}%.{floor_text}"
        ));
        lines.push(String::new());
        lines.push(
            "| scenario | base p95 ms | head p95 ms | delta | runs base/head | status |".into(),
        );
        lines.push("|---|---|---|---|---|---|".into());
        for r in rows {
            lines.push(format!(
                "| {} | {} | {} | {} | {}/{} | {} |",
                r.scenario,
                fmt_ms(r.base),
                fmt_ms(r.head),
                fmt_pct(r.delta),
                r.n_base,
                r.n_head,
                r.status
            ));
        }
    }
    if !runner_lines.is_empty() {
        lines.push(String::new());
        for line in runner_lines {
            lines.push(format!("- {line}"));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests;
