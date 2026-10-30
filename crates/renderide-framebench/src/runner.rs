//! Scenario runner. Each run is a fresh renderer process. The published percentile is the
//! median of the per-run percentiles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::error::BenchError;
use crate::report::{
    AdapterInfo, ControlApi, FrameBenchReport, RAW_SCHEMA, SCHEMA, ScenarioRun, aggregate_scenario,
    standard_notes,
};
use crate::runner_env::RunnerInfo;
use crate::scenario;

/// Inputs for [`run_bench`].
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// `renderide-renderer` binary.
    pub renderer_path: PathBuf,
    /// Scenario names. Empty selects [`scenario::DEFAULT_SCENARIOS`].
    pub scenarios: Vec<String>,
    /// Warmup frames passed to the renderer.
    pub warmup_frames: u32,
    /// Measured frames passed to the renderer.
    pub measured_frames: u32,
    /// Process invocations per scenario. Must be at least 1.
    pub runs: u32,
    /// Headless width.
    pub width: u32,
    /// Headless height.
    pub height: u32,
    /// Report wait after scene submit.
    pub timeout: Duration,
    /// Inherit renderer logs.
    pub verbose_renderer: bool,
    /// With `verbose_renderer`, send the renderer's stdout to this process's stderr so stdout
    /// carries only the `--json` document.
    pub renderer_stdout_to_stderr: bool,
    /// Skip headless flags. Window size is not pinned.
    pub windowed: bool,
    /// Final JSON path.
    pub output_path: PathBuf,
}

/// Runs every scenario and writes one schema-2 report.
///
/// `runner` is stored as the report's `runner` block; callers normally pass
/// [`crate::runner_env::detect`].
///
/// # Errors
///
/// Returns an error when a scenario name, frame count, or raw report is invalid, or when the
/// harness fails. [`BenchError::run_exit_code`] classifies it.
pub fn run_bench(cfg: &RunConfig, runner: RunnerInfo) -> Result<FrameBenchReport, BenchError> {
    validate(cfg)?;
    for name in &cfg.scenarios {
        if scenario::by_name(name).is_none() {
            return Err(BenchError::Usage(format!("unknown scenario `{name}`")));
        }
    }
    let _ = renderide_test::logging::init_renderer_test_logging()
        .map_err(|err| BenchError::Env(format!("logging: {err}")))?;
    let names = if cfg.scenarios.is_empty() {
        scenario::DEFAULT_SCENARIOS
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        cfg.scenarios.clone()
    };
    let mut scenarios = Vec::with_capacity(names.len());
    let mut adapter = AdapterInfo::empty();
    let mut renderer_commit = String::new();
    let mut saw_adapter = false;
    for name in &names {
        let scenario = scenario::by_name(name)
            .ok_or_else(|| BenchError::Usage(format!("unknown scenario `{name}`")))?;
        let steps = scenario::script(scenario, cfg.warmup_frames, cfg.measured_frames);
        let (position, rotation) = scenario::pose_from_script(&steps)
            .ok_or_else(|| BenchError::Run(format!("scenario {name} has no camera.set_pose")))?;
        let mut runs = Vec::with_capacity(usize::try_from(cfg.runs).unwrap_or(1));
        for index in 0..cfg.runs {
            let run = run_once(cfg, scenario.name, index, position, rotation)?;
            if !saw_adapter {
                adapter = run.adapter.clone();
                renderer_commit.clone_from(&run.renderer_commit);
                saw_adapter = true;
            } else if run.renderer_commit != renderer_commit {
                return Err(BenchError::Run(
                    "renderer commit changed between runs".to_string(),
                ));
            }
            runs.push(run);
        }
        scenarios.push(aggregate_scenario(scenario.name, &runs, steps).map_err(BenchError::Run)?);
    }
    let report = FrameBenchReport {
        schema: SCHEMA,
        scale: "relative".to_string(),
        git_sha: git_sha(),
        renderer_commit,
        mode: if cfg.windowed {
            "windowed".to_string()
        } else {
            "headless".to_string()
        },
        resolution: [cfg.width, cfg.height],
        warmup_frames: cfg.warmup_frames,
        measured_frames: cfg.measured_frames,
        runs: cfg.runs,
        adapter,
        runner: Some(runner),
        timestamp_query: scenarios
            .iter()
            .any(|scenario| scenario.gpu_passes_ms.is_some()),
        control_api: ControlApi::unimplemented(),
        scenarios,
        notes: standard_notes(cfg.windowed),
    };
    write_report(&cfg.output_path, &report)?;
    Ok(report)
}

fn validate(cfg: &RunConfig) -> Result<(), BenchError> {
    if cfg.runs == 0 {
        return Err(BenchError::Usage("runs must be >= 1".to_string()));
    }
    if cfg.measured_frames == 0 {
        return Err(BenchError::Usage(
            "measured frames must be >= 1".to_string(),
        ));
    }
    if cfg.width == 0 || cfg.height == 0 {
        return Err(BenchError::Usage(
            "resolution width and height must be >= 1".to_string(),
        ));
    }
    Ok(())
}

fn run_once(
    cfg: &RunConfig,
    name: &str,
    index: u32,
    position: [f32; 3],
    rotation: [f32; 4],
) -> Result<ScenarioRun, BenchError> {
    let raw_dir = tempfile::Builder::new()
        .prefix("renderide-framebench-raw-")
        .tempdir()?;
    let raw_path = raw_dir.path().join(format!("{name}-run{index}.json"));
    renderide_test::host::scene_session::drive_bench(
        &renderide_test::host::scene_session::BenchDriveConfig {
            renderer_path: cfg.renderer_path.clone(),
            scenario: name.to_string(),
            report_path: raw_path.clone(),
            width: cfg.width,
            height: cfg.height,
            warmup_frames: cfg.warmup_frames,
            measured_frames: cfg.measured_frames,
            timeout: cfg.timeout,
            verbose_renderer: cfg.verbose_renderer,
            renderer_stdout_to_stderr: cfg.renderer_stdout_to_stderr,
            windowed: cfg.windowed,
            camera_position: position,
            camera_rotation_xyzw: rotation,
        },
    )?;
    parse_raw(&raw_path, cfg)
}

fn parse_raw(path: &Path, cfg: &RunConfig) -> Result<ScenarioRun, BenchError> {
    let bytes = std::fs::read(path)
        .map_err(|err| BenchError::Run(format!("read raw report {}: {err}", path.display())))?;
    let raw: RawReport = serde_json::from_slice(&bytes)?;
    if raw.schema != RAW_SCHEMA || !raw.complete {
        return Err(BenchError::Run(format!(
            "{} is not a complete schema {RAW_SCHEMA} raw report",
            path.display()
        )));
    }
    if raw.warmup_frames != cfg.warmup_frames || raw.measured_frames != cfg.measured_frames {
        return Err(BenchError::Run(format!(
            "{} recorded warmup/measured {}/{}, expected {}/{}",
            path.display(),
            raw.warmup_frames,
            raw.measured_frames,
            cfg.warmup_frames,
            cfg.measured_frames
        )));
    }
    let expected = usize::try_from(cfg.measured_frames).unwrap_or(usize::MAX);
    if raw.frames.len() != expected {
        return Err(BenchError::Run(format!(
            "{} has {} frames, expected {expected}",
            path.display(),
            raw.frames.len()
        )));
    }
    let cpu_frame_ms = raw.frames.iter().map(|frame| frame.cpu_frame_ms).collect();
    let draw_calls = counts(path, raw.frames.iter().map(|frame| frame.draw_calls))?;
    let upload_count = counts(path, raw.frames.iter().map(|frame| frame.upload_count))?;
    let upload_bytes = counts(path, raw.frames.iter().map(|frame| frame.upload_bytes))?;
    let gpu_passes_ms = raw
        .timestamp_query
        .then(|| gpu_frames(&raw.frames))
        .flatten();
    Ok(ScenarioRun {
        cpu_frame_ms,
        draw_calls,
        upload_count,
        upload_bytes,
        gpu_passes_ms,
        adapter: raw.adapter,
        renderer_commit: raw.renderer_commit,
    })
}

fn counts(path: &Path, values: impl Iterator<Item = u64>) -> Result<Vec<f64>, BenchError> {
    values
        .map(|value| {
            u32::try_from(value).map(f64::from).map_err(|_err| {
                BenchError::Run(format!("{} count {value} exceeds u32", path.display()))
            })
        })
        .collect()
}

fn gpu_frames(frames: &[RawFrame]) -> Option<Vec<BTreeMap<String, f64>>> {
    if frames
        .iter()
        .any(|frame| frame.gpu_passes_ms.as_ref().is_none_or(BTreeMap::is_empty))
    {
        return None;
    }
    Some(
        frames
            .iter()
            .filter_map(|frame| frame.gpu_passes_ms.clone())
            .collect(),
    )
}

fn write_report(path: &Path, report: &FrameBenchReport) -> Result<(), BenchError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(report)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn git_sha() -> String {
    let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
    else {
        return "unknown".to_string();
    };
    if !output.status.success() {
        return "unknown".to_string();
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        "unknown".to_string()
    } else {
        sha
    }
}

/// Resolves the renderer binary from an explicit path, a sibling of this executable, or
/// `target/{debug,release}/renderide-renderer`.
pub fn resolve_renderer_path(explicit: Option<PathBuf>, release: bool) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(renderer_file_name());
        if sibling.is_file() {
            return sibling;
        }
    }
    workspace_root()
        .join("target")
        .join(if release { "release" } else { "debug" })
        .join(renderer_file_name())
}

fn renderer_file_name() -> &'static str {
    if cfg!(windows) {
        "renderide-renderer.exe"
    } else {
        "renderide-renderer"
    }
}

fn workspace_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates/renderide").is_dir() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[derive(Deserialize)]
struct RawReport {
    schema: u32,
    complete: bool,
    warmup_frames: u32,
    measured_frames: u32,
    timestamp_query: bool,
    #[serde(default)]
    renderer_commit: String,
    adapter: AdapterInfo,
    frames: Vec<RawFrame>,
}

#[derive(Deserialize)]
struct RawFrame {
    cpu_frame_ms: f64,
    draw_calls: u64,
    upload_count: u64,
    upload_bytes: u64,
    gpu_passes_ms: Option<BTreeMap<String, f64>>,
}
