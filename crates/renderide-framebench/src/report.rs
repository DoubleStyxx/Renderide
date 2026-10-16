//! Final frame-benchmark report. Schema version 1.
//!
//! Percentiles inside a run use linear interpolation. The published percentile is the median
//! of those per-run values. Absolute milliseconds are descriptive only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::scenario::{CONTROL_METHODS, CONTROL_TRANSPORT, ScriptStep};
use crate::stats::{Percentiles, median, percentiles};

/// Report schema written by this crate.
pub const SCHEMA: u32 = 1;

/// Adapter and driver strings copied from the renderer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdapterInfo {
    /// wgpu adapter name.
    pub name: String,
    /// wgpu backend debug name.
    pub backend: String,
    /// wgpu device type debug name.
    pub device_type: String,
    /// Driver name.
    pub driver: String,
    /// Driver info string.
    pub driver_info: String,
}

impl AdapterInfo {
    /// Empty adapter block used before the first sample.
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            backend: String::new(),
            device_type: String::new(),
            driver: String::new(),
            driver_info: String::new(),
        }
    }
}

/// Planned control API. `implemented` stays false; no socket is opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlApi {
    /// Always false in this crate.
    pub implemented: bool,
    /// `localhost-tcp-jsonrpc`.
    pub transport: String,
    /// `camera.set_pose`, `frames.wait`, `stats.query`.
    pub methods: Vec<String>,
}

impl ControlApi {
    /// The unimplemented control API descriptor stored in every report.
    pub fn unimplemented() -> Self {
        Self {
            implemented: false,
            transport: CONTROL_TRANSPORT.to_string(),
            methods: CONTROL_METHODS.into_iter().map(str::to_string).collect(),
        }
    }
}

/// One scenario after N runs have been reduced to a median.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioReport {
    /// Scenario name.
    pub name: String,
    /// CPU frame time, milliseconds.
    pub cpu_frame_ms: Percentiles,
    /// World-mesh draw count.
    pub draw_calls: Percentiles,
    /// Upload write count.
    pub upload_count: Percentiles,
    /// Upload payload bytes.
    pub upload_bytes: Percentiles,
    /// Per-pass GPU milliseconds. Null when any run has no timestamp data.
    pub gpu_passes_ms: Option<BTreeMap<String, Percentiles>>,
    /// Each run's CPU p95, before the median published in `cpu_frame_ms.p95`.
    pub per_run_cpu_frame_ms_p95: Vec<f64>,
    /// In-process stand-in for the control API.
    pub script: Vec<ScriptStep>,
}

/// One `renderide-framebench run` report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameBenchReport {
    /// Schema version. Currently 1.
    pub schema: u32,
    /// Always `relative`. Absolute milliseconds are not a gate.
    pub scale: String,
    /// `git rev-parse HEAD` from the runner, or `unknown`.
    pub git_sha: String,
    /// Renderer commit string embedded in the binary.
    pub renderer_commit: String,
    /// `headless` or `windowed`.
    pub mode: String,
    /// Requested framebuffer size, width then height.
    pub resolution: [u32; 2],
    /// Warmup frames discarded by the renderer.
    pub warmup_frames: u32,
    /// Measured frames per run.
    pub measured_frames: u32,
    /// Process invocations aggregated per scenario.
    pub runs: u32,
    /// Adapter from the first run.
    pub adapter: AdapterInfo,
    /// True when at least one scenario published GPU pass times.
    pub timestamp_query: bool,
    /// Unimplemented dev control API.
    pub control_api: ControlApi,
    /// One entry per requested scenario.
    pub scenarios: Vec<ScenarioReport>,
    /// How to read the numbers.
    pub notes: Vec<String>,
}

/// Measured frames from one renderer process.
#[derive(Clone, Debug)]
pub(crate) struct ScenarioRun {
    /// CPU frame milliseconds, one entry per measured frame.
    pub cpu_frame_ms: Vec<f64>,
    /// Draw counts.
    pub draw_calls: Vec<f64>,
    /// Upload counts.
    pub upload_count: Vec<f64>,
    /// Upload bytes.
    pub upload_bytes: Vec<f64>,
    /// Per-frame pass maps. `None` when the run has no usable timestamps.
    pub gpu_passes_ms: Option<Vec<BTreeMap<String, f64>>>,
    /// Adapter copied from the raw report.
    pub adapter: AdapterInfo,
    /// Renderer commit string.
    pub renderer_commit: String,
}

/// Reduces runs of one scenario to the median of each run's percentiles.
pub(crate) fn aggregate_scenario(
    name: &str,
    runs: &[ScenarioRun],
    script: Vec<ScriptStep>,
) -> Result<ScenarioReport, String> {
    if runs.is_empty() {
        return Err(format!("scenario {name} has no runs"));
    }
    let cpu = median_of(
        runs,
        |run| percentiles(&run.cpu_frame_ms),
        name,
        "cpu_frame_ms",
    )?;
    let draw_calls = median_of(runs, |run| percentiles(&run.draw_calls), name, "draw_calls")?;
    let upload_count = median_of(
        runs,
        |run| percentiles(&run.upload_count),
        name,
        "upload_count",
    )?;
    let upload_bytes = median_of(
        runs,
        |run| percentiles(&run.upload_bytes),
        name,
        "upload_bytes",
    )?;
    let per_run_cpu_frame_ms_p95 = runs
        .iter()
        .map(|run| percentiles(&run.cpu_frame_ms).map(|stats| stats.p95))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("scenario {name} is missing cpu samples"))?;
    Ok(ScenarioReport {
        name: name.to_string(),
        cpu_frame_ms: cpu,
        draw_calls,
        upload_count,
        upload_bytes,
        gpu_passes_ms: aggregate_gpu(runs),
        per_run_cpu_frame_ms_p95,
        script,
    })
}

fn median_of(
    runs: &[ScenarioRun],
    pick: impl Fn(&ScenarioRun) -> Option<Percentiles>,
    name: &str,
    metric: &str,
) -> Result<Percentiles, String> {
    let stats = runs
        .iter()
        .map(pick)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("scenario {name} is missing {metric} samples"))?;
    median_percentiles(&stats).ok_or_else(|| format!("scenario {name} could not median {metric}"))
}

fn median_percentiles(stats: &[Percentiles]) -> Option<Percentiles> {
    Some(Percentiles {
        p50: median(&stats.iter().map(|item| item.p50).collect::<Vec<_>>())?,
        p95: median(&stats.iter().map(|item| item.p95).collect::<Vec<_>>())?,
        p99: median(&stats.iter().map(|item| item.p99).collect::<Vec<_>>())?,
    })
}

fn aggregate_gpu(runs: &[ScenarioRun]) -> Option<BTreeMap<String, Percentiles>> {
    if runs.iter().any(|run| {
        run.gpu_passes_ms
            .as_ref()
            .is_none_or(|frames| frames.is_empty())
    }) {
        return None;
    }
    let frames = runs
        .iter()
        .filter_map(|run| run.gpu_passes_ms.as_ref())
        .collect::<Vec<_>>();
    let mut keys = frames
        .first()
        .and_then(|frames| frames.first())
        .map(|frame| frame.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.retain(|key| {
        frames.iter().all(|run_frames| {
            run_frames
                .iter()
                .all(|frame| frame.contains_key(key.as_str()))
        })
    });
    let mut out = BTreeMap::new();
    for key in keys {
        let per_run = frames
            .iter()
            .map(|run_frames| {
                let samples = run_frames
                    .iter()
                    .filter_map(|frame| frame.get(&key).copied())
                    .collect::<Vec<_>>();
                percentiles(&samples)
            })
            .collect::<Option<Vec<_>>>()?;
        out.insert(key, median_percentiles(&per_run)?);
    }
    (!out.is_empty()).then_some(out)
}

/// Notes stored on every report.
pub fn standard_notes(windowed: bool) -> Vec<String> {
    let mut notes = vec![
        "Absolute milliseconds are not a gate and are not comparable across machines, adapters, or lavapipe versus a discrete GPU.".to_string(),
        "Published percentiles are the median across runs of each run's percentile.".to_string(),
        "gpu_passes_ms is null when TIMESTAMP_QUERY is unavailable or a run failed to read it.".to_string(),
        "control_api.implemented is false. camera.set_pose, frames.wait, and stats.query are applied in process.".to_string(),
    ];
    if windowed {
        notes.push("Windowed runs do not pin the winit window size.".to_string());
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::{AdapterInfo, ScenarioRun, aggregate_scenario};

    fn run(
        cpu: Vec<f64>,
        gpu: Option<Vec<std::collections::BTreeMap<String, f64>>>,
    ) -> ScenarioRun {
        let n = cpu.len();
        ScenarioRun {
            cpu_frame_ms: cpu,
            draw_calls: vec![1.0; n],
            upload_count: vec![0.0; n],
            upload_bytes: vec![0.0; n],
            gpu_passes_ms: gpu,
            adapter: AdapterInfo::empty(),
            renderer_commit: "abc".to_string(),
        }
    }

    #[test]
    fn median_of_run_p95_is_not_a_pooled_sample() {
        let slow = run(vec![10.0, 20.0, 30.0], None);
        let fast = run(vec![1.0, 2.0, 3.0], None);
        let report = aggregate_scenario("unlit_sphere", &[slow, fast], Vec::new()).expect("agg");
        let slow_p95 = 29.0;
        let fast_p95 = 2.9;
        let expected = f64::midpoint(slow_p95, fast_p95);
        assert!(
            (report.cpu_frame_ms.p95 - expected).abs() < 1e-9,
            "{}",
            report.cpu_frame_ms.p95
        );
        assert_eq!(report.per_run_cpu_frame_ms_p95.len(), 2);
    }

    #[test]
    fn one_run_without_gpu_timestamps_nulls_the_scenario() {
        let with_gpu = run(
            vec![1.0, 2.0],
            Some(vec![std::collections::BTreeMap::from([(
                "color".to_string(),
                0.5,
            )])]),
        );
        let without = run(vec![1.0, 2.0], None);
        let report =
            aggregate_scenario("unlit_sphere", &[with_gpu, without], Vec::new()).expect("agg");
        assert!(report.gpu_passes_ms.is_none());
    }
}
