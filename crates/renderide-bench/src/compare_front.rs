//! Front-door compare for criterion samples and framebench reports.
//!
//! Decision rule: one shared relative gate — the same percent thresholds and absolute-ms floor
//! as `renderide_framebench::relative_status` / `compare` and `scripts/ci/frame_bench_gate.py`
//! (the Python gate also applies Mann-Whitney on multi-run samples; B2 leaves `p_value` null).
//! Default [`renderide_framebench::DEFAULT_ABS_FLOOR_MS`] is 0 (O8 still open).

use std::collections::BTreeMap;
use std::path::Path;

use renderide_framebench::{
    DEFAULT_ABS_FLOOR_MS, DEFAULT_THRESHOLD_PERCENT, FrameBenchReport, RelativeStatus,
    relative_status, schema_supported,
};

use crate::error::BenchError;
use crate::report::{BenchCompareReport, COMPARE_SCHEMA, CompareRow, CpuSample, Verdict};

/// Default flag threshold (percent). Between flag and fail is report-only [`Verdict::Flag`].
pub const DEFAULT_FLAG_THRESHOLD_PERCENT: f64 = 5.0;

/// Compare knobs for the front door.
#[derive(Clone, Debug)]
pub struct FrontCompareOptions {
    /// Fail when a gated relative delta is strictly greater than this percent.
    pub fail_threshold: f64,
    /// Flag (report-only) when delta is strictly greater than this percent.
    pub flag_threshold: f64,
    /// Millisecond floor for frame ms metrics; 0 = off.
    pub abs_floor_ms: f64,
}

impl Default for FrontCompareOptions {
    fn default() -> Self {
        Self {
            fail_threshold: DEFAULT_THRESHOLD_PERCENT,
            flag_threshold: DEFAULT_FLAG_THRESHOLD_PERCENT,
            abs_floor_ms: DEFAULT_ABS_FLOOR_MS,
        }
    }
}

/// Compares two front-door run documents or raw frame/criterion inputs on disk.
///
/// `base` / `head` may be:
/// - `renderide-bench/1` run reports,
/// - framebench schema 1|2 reports,
/// - directories of criterion output (`*/new/estimates.json`).
///
/// # Errors
///
/// Returns [`BenchError::Usage`] when an input cannot be read or its schema is wrong.
pub fn compare_paths(
    base: &Path,
    head: &Path,
    options: &FrontCompareOptions,
) -> Result<BenchCompareReport, BenchError> {
    validate_options(options)?;
    let mut rows = Vec::new();
    let base_input = load_input(base)?;
    let head_input = load_input(head)?;

    match (&base_input, &head_input) {
        (Input::Run(b), Input::Run(h)) => {
            rows.extend(compare_cpu(&b.cpu, &h.cpu, options));
            if let (Some(bf), Some(hf)) = (&b.frame, &h.frame) {
                rows.extend(compare_frame(bf, hf, options)?);
            } else if b.frame.is_some() || h.frame.is_some() {
                rows.push(CompareRow {
                    suite: "frame".into(),
                    name: "*".into(),
                    metric: "cpu_frame_ms.p95".into(),
                    base: None,
                    head: None,
                    delta_pct: None,
                    p_value: None,
                    verdict: Verdict::Missing,
                });
            }
        }
        (Input::Frame(b), Input::Frame(h)) => {
            rows.extend(compare_frame(b, h, options)?);
        }
        (Input::Cpu(b), Input::Cpu(h)) => {
            rows.extend(compare_cpu(b, h, options));
        }
        _ => {
            return Err(BenchError::Usage(format!(
                "base ({}) and head ({}) are different input kinds; compare like with like",
                base.display(),
                head.display()
            )));
        }
    }

    let failed = rows.iter().any(|row| row.verdict == Verdict::Fail);
    Ok(BenchCompareReport {
        schema: COMPARE_SCHEMA.to_string(),
        fail_threshold: options.fail_threshold,
        flag_threshold: options.flag_threshold,
        abs_floor_ms: options.abs_floor_ms,
        decision_rule: decision_rule_doc().to_string(),
        rows,
        exit_code: if failed {
            crate::error::exit_code::REGRESSION
        } else {
            crate::error::exit_code::OK
        },
    })
}

/// Documented location of the shared decision rule.
pub fn decision_rule_doc() -> &'static str {
    "renderide_framebench::relative_status (+ scripts/ci/frame_bench_gate.py Mann-Whitney on multi-run CI); DEFAULT_ABS_FLOOR_MS=0 (O8 open)"
}

fn validate_options(options: &FrontCompareOptions) -> Result<(), BenchError> {
    if !options.fail_threshold.is_finite() || options.fail_threshold < 0.0 {
        return Err(BenchError::Usage(format!(
            "fail threshold must be a finite number >= 0, got {}",
            options.fail_threshold
        )));
    }
    if !options.flag_threshold.is_finite() || options.flag_threshold < 0.0 {
        return Err(BenchError::Usage(format!(
            "flag threshold must be a finite number >= 0, got {}",
            options.flag_threshold
        )));
    }
    if options.flag_threshold > options.fail_threshold {
        return Err(BenchError::Usage(format!(
            "flag threshold ({}) must be <= fail threshold ({})",
            options.flag_threshold, options.fail_threshold
        )));
    }
    if !options.abs_floor_ms.is_finite() || options.abs_floor_ms < 0.0 {
        return Err(BenchError::Usage(format!(
            "abs floor must be a finite number >= 0 ms, got {}",
            options.abs_floor_ms
        )));
    }
    Ok(())
}

enum Input {
    Run(Box<crate::report::BenchRunReport>),
    Frame(Box<FrameBenchReport>),
    Cpu(Vec<CpuSample>),
}

fn load_input(path: &Path) -> Result<Input, BenchError> {
    if path.is_dir() {
        let samples = crate::criterion_read::collect_cpu_samples(path)
            .map_err(|err| BenchError::Usage(format!("criterion dir {}: {err}", path.display())))?;
        return Ok(Input::Cpu(samples));
    }
    let bytes = std::fs::read(path)
        .map_err(|err| BenchError::Usage(format!("read {}: {err}", path.display())))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|err| BenchError::Usage(format!("json {}: {err}", path.display())))?;
    if value.get("schema").and_then(|s| s.as_str()) == Some(crate::report::RUN_SCHEMA) {
        let report: crate::report::BenchRunReport = serde_json::from_value(value)
            .map_err(|err| BenchError::Usage(format!("run report {}: {err}", path.display())))?;
        return Ok(Input::Run(Box::new(report)));
    }
    if let Some(schema) = value.get("schema").and_then(serde_json::Value::as_u64) {
        if schema_supported(schema as u32) {
            let report: FrameBenchReport = serde_json::from_value(value).map_err(|err| {
                BenchError::Usage(format!("frame report {}: {err}", path.display()))
            })?;
            if report.schema >= 2 && report.runner.is_none() {
                return Err(BenchError::Usage(format!(
                    "{} is a schema {} report without a `runner` object",
                    path.display(),
                    report.schema
                )));
            }
            return Ok(Input::Frame(Box::new(report)));
        }
        return Err(BenchError::Usage(format!(
            "{} has unsupported schema {schema}",
            path.display()
        )));
    }
    Err(BenchError::Usage(format!(
        "{} is not a renderide-bench run report, a framebench report, or a criterion directory",
        path.display()
    )))
}

fn compare_cpu(
    base: &[CpuSample],
    head: &[CpuSample],
    options: &FrontCompareOptions,
) -> Vec<CompareRow> {
    let mut map: BTreeMap<String, (Option<&CpuSample>, Option<&CpuSample>)> = BTreeMap::new();
    for sample in base {
        let key = format!("{}/{}", sample.group, sample.id);
        map.entry(key).or_default().0 = Some(sample);
    }
    for sample in head {
        let key = format!("{}/{}", sample.group, sample.id);
        map.entry(key).or_default().1 = Some(sample);
    }
    let mut rows = Vec::with_capacity(map.len());
    for (name, (b, h)) in map {
        let row = match (b, h) {
            (None, Some(h)) => CompareRow {
                suite: "cpu".into(),
                name,
                metric: "median_ns".into(),
                base: None,
                head: Some(h.median_ns),
                delta_pct: None,
                p_value: None,
                verdict: Verdict::New,
            },
            (Some(b), None) => CompareRow {
                suite: "cpu".into(),
                name,
                metric: "median_ns".into(),
                base: Some(b.median_ns),
                head: None,
                delta_pct: None,
                p_value: None,
                verdict: Verdict::Missing,
            },
            (Some(b), Some(h)) => {
                // Abs floor is defined in ms for frame metrics; CPU uses ns. Convert when set.
                let floor =
                    (options.abs_floor_ms > 0.0).then_some(options.abs_floor_ms * 1_000_000.0);
                row_from_values(
                    "cpu",
                    &name,
                    "median_ns",
                    b.median_ns,
                    h.median_ns,
                    options,
                    floor,
                )
            }
            (None, None) => {
                // BTreeMap entry always has a side; skip if both somehow absent.
                continue;
            }
        };
        rows.push(row);
    }
    rows
}

fn compare_frame(
    base: &FrameBenchReport,
    head: &FrameBenchReport,
    options: &FrontCompareOptions,
) -> Result<Vec<CompareRow>, BenchError> {
    if !schema_supported(base.schema) || !schema_supported(head.schema) {
        return Err(BenchError::Usage(format!(
            "expected frame schema 1 or 2, got base {} and head {}",
            base.schema, head.schema
        )));
    }
    let mut names: Vec<String> = base.scenarios.iter().map(|s| s.name.clone()).collect();
    for scenario in &head.scenarios {
        if !names.iter().any(|n| n == &scenario.name) {
            names.push(scenario.name.clone());
        }
    }
    names.sort();
    let floor = (options.abs_floor_ms > 0.0).then_some(options.abs_floor_ms);
    let mut rows = Vec::new();
    for name in names {
        let b = base.scenarios.iter().find(|s| s.name == name);
        let h = head.scenarios.iter().find(|s| s.name == name);
        match (b, h) {
            (None, Some(h)) => rows.push(CompareRow {
                suite: "frame".into(),
                name,
                metric: "cpu_frame_ms.p95".into(),
                base: None,
                head: Some(h.cpu_frame_ms.p95),
                delta_pct: None,
                p_value: None,
                verdict: Verdict::New,
            }),
            (Some(b), None) => rows.push(CompareRow {
                suite: "frame".into(),
                name,
                metric: "cpu_frame_ms.p95".into(),
                base: Some(b.cpu_frame_ms.p95),
                head: None,
                delta_pct: None,
                p_value: None,
                verdict: Verdict::Missing,
            }),
            (Some(b), Some(h)) => rows.push(row_from_values(
                "frame",
                &name,
                "cpu_frame_ms.p95",
                b.cpu_frame_ms.p95,
                h.cpu_frame_ms.p95,
                options,
                floor,
            )),
            (None, None) => {}
        }
    }
    Ok(rows)
}

fn row_from_values(
    suite: &str,
    name: &str,
    metric: &str,
    base: f64,
    head: f64,
    options: &FrontCompareOptions,
    abs_floor: Option<f64>,
) -> CompareRow {
    let (delta, status) = relative_status(base, head, true, options.fail_threshold, abs_floor);
    let verdict = match status {
        RelativeStatus::Ok => {
            // Split pass vs flag using the flag threshold on the same delta.
            match delta {
                Some(d) if d > options.flag_threshold => Verdict::Flag,
                _ => Verdict::Pass,
            }
        }
        RelativeStatus::OkAbsFloor => Verdict::Pass,
        RelativeStatus::Fail => Verdict::Fail,
        RelativeStatus::Undefined => Verdict::Missing,
    };
    CompareRow {
        suite: suite.into(),
        name: name.into(),
        metric: metric.into(),
        base: Some(base),
        head: Some(head),
        delta_pct: delta,
        p_value: None,
        verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::{FrontCompareOptions, compare_cpu, compare_paths};
    use crate::report::{CpuSample, Verdict};
    use renderide_framebench::DEFAULT_ABS_FLOOR_MS;
    use std::fs;

    fn sample(group: &str, id: &str, median_ns: f64) -> CpuSample {
        CpuSample {
            group: group.into(),
            id: id.into(),
            median_ns,
            ci95_ns: [median_ns * 0.99, median_ns * 1.01],
            throughput: None,
        }
    }

    #[test]
    fn default_abs_floor_is_zero() {
        assert_eq!(DEFAULT_ABS_FLOOR_MS, 0.0);
        assert_eq!(FrontCompareOptions::default().abs_floor_ms, 0.0);
    }

    #[test]
    fn cpu_threshold_flag_and_fail() {
        let options = FrontCompareOptions::default();
        let base = vec![sample("culling", "homogeneous_clip_aabb", 1000.0)];
        // +6% -> flag (above 5, at or below 10)
        let head = vec![sample("culling", "homogeneous_clip_aabb", 1060.0)];
        let rows = compare_cpu(&base, &head, &options);
        assert_eq!(rows[0].verdict, Verdict::Flag);
        // +11% -> fail
        let head = vec![sample("culling", "homogeneous_clip_aabb", 1110.0)];
        let rows = compare_cpu(&base, &head, &options);
        assert_eq!(rows[0].verdict, Verdict::Fail);
        // +10% exactly -> pass (relative_status: delta <= threshold)
        let head = vec![sample("culling", "homogeneous_clip_aabb", 1100.0)];
        let rows = compare_cpu(&base, &head, &options);
        assert_eq!(rows[0].verdict, Verdict::Flag); // 10 > flag 5, <= fail 10 -> Flag via Ok branch
    }

    #[test]
    fn cpu_abs_floor_when_set_clears_fail() {
        // +20% but only 0.04 ms (= 40_000 ns) with a 0.05 ms floor.
        let options = FrontCompareOptions {
            abs_floor_ms: 0.05,
            ..FrontCompareOptions::default()
        };
        let base = vec![sample("culling", "x", 200_000.0)];
        let head = vec![sample("culling", "x", 240_000.0)];
        let rows = compare_cpu(&base, &head, &options);
        assert_eq!(rows[0].verdict, Verdict::Pass);
        let options = FrontCompareOptions::default();
        let rows = compare_cpu(&base, &head, &options);
        assert_eq!(rows[0].verdict, Verdict::Fail);
    }

    #[test]
    fn compare_frame_fixtures_on_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("base.json");
        let head = dir.path().join("head.json");
        let mk = |p95: f64, schema: u32| {
            let mut doc = serde_json::json!({
                "schema": schema,
                "scale": "relative",
                "git_sha": "abc",
                "renderer_commit": "abc",
                "mode": "headless",
                "resolution": [256, 256],
                "warmup_frames": 10,
                "measured_frames": 20,
                "runs": 1,
                "adapter": {"name": "llvmpipe", "backend": "Vulkan", "device_type": "Cpu",
                            "driver": "llvmpipe", "driver_info": "Mesa"},
                "timestamp_query": false,
                "control_api": {"implemented": false, "transport": "localhost-tcp-jsonrpc", "methods": []},
                "scenarios": [{
                    "name": "unlit_sphere",
                    "cpu_frame_ms": {"p50": p95*0.5, "p95": p95, "p99": p95*1.1},
                    "draw_calls": {"p50": 1.0, "p95": 1.0, "p99": 1.0},
                    "upload_count": {"p50": 0.0, "p95": 0.0, "p99": 0.0},
                    "upload_bytes": {"p50": 0.0, "p95": 0.0, "p99": 0.0},
                    "gpu_passes_ms": null,
                    "per_run_cpu_frame_ms_p95": [p95],
                    "script": []
                }],
                "notes": []
            });
            if schema >= 2 {
                doc["runner"] = serde_json::json!({
                    "os": "linux", "cpu_model": "test",
                    "icd": {"path": null, "source": "none", "var": null},
                    "rayon_threads": 1, "rayon_threads_source": "default"
                });
            }
            doc
        };
        fs::write(&base, serde_json::to_vec(&mk(2.0, 1)).unwrap()).unwrap();
        fs::write(&head, serde_json::to_vec(&mk(2.5, 2)).unwrap()).unwrap();
        let report = compare_paths(&base, &head, &FrontCompareOptions::default()).expect("cmp");
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.rows[0].verdict, Verdict::Fail);

        fs::write(&head, serde_json::to_vec(&mk(2.0, 2)).unwrap()).unwrap();
        let report = compare_paths(&base, &head, &FrontCompareOptions::default()).expect("cmp");
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.rows[0].verdict, Verdict::Pass);

        // Tiny absolute delta fails with floor off, passes with floor on.
        fs::write(&base, serde_json::to_vec(&mk(0.2, 2)).unwrap()).unwrap();
        fs::write(&head, serde_json::to_vec(&mk(0.24, 2)).unwrap()).unwrap();
        let report = compare_paths(&base, &head, &FrontCompareOptions::default()).expect("cmp");
        assert_eq!(report.exit_code, 1);
        let floored = FrontCompareOptions {
            abs_floor_ms: 0.05,
            ..FrontCompareOptions::default()
        };
        let report = compare_paths(&base, &head, &floored).expect("cmp");
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.rows[0].verdict, Verdict::Pass);
    }
}
