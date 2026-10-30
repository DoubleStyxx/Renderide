//! `renderide-bench run` for CPU (criterion) and frame (framebench) suites.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use renderide_framebench::{
    FrameBenchReport, RunConfig, RunnerInfo, detect_runner, resolve_renderer_path, run_bench,
};

use crate::catalog::CPU_BENCH_IDS;
use crate::criterion_read::collect_cpu_samples;
use crate::error::BenchError;
use crate::report::{BenchRunReport, CpuSample, RUN_SCHEMA};

/// Which suites to execute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suite {
    /// Criterion micro-benches only.
    Cpu,
    /// Framebench only.
    Frame,
    /// Both.
    All,
}

impl Suite {
    /// Parses `cpu`, `frame`, or `all`.
    ///
    /// # Errors
    ///
    /// Returns [`BenchError::Usage`] for any other string.
    pub fn parse(text: &str) -> Result<Self, BenchError> {
        match text {
            "cpu" => Ok(Self::Cpu),
            "frame" => Ok(Self::Frame),
            "all" => Ok(Self::All),
            other => Err(BenchError::Usage(format!(
                "unknown suite `{other}`; expected cpu, frame, or all"
            ))),
        }
    }
}

/// Options for [`run_suites`].
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Suite selection.
    pub suite: Suite,
    /// Optional scenario filter for the frame suite.
    pub scenarios: Vec<String>,
    /// Framebench warmup frames.
    pub warmup: u32,
    /// Framebench measured frames.
    pub frames: u32,
    /// Framebench runs per scenario.
    pub runs: u32,
    /// Framebuffer size `WxH`.
    pub resolution: String,
    /// Optional renderer path.
    pub renderer: Option<PathBuf>,
    /// Prefer release renderer when `--renderer` is omitted.
    pub release: bool,
    /// Seconds to wait for a framebench raw report.
    pub timeout_seconds: u64,
    /// Inherit renderer logs.
    pub verbose_renderer: bool,
    /// Windowed framebench.
    pub windowed: bool,
    /// Optional output path for the front-door JSON (and framebench file when frame runs).
    pub output: Option<PathBuf>,
    /// When set, skip `cargo bench` and only collect from this criterion directory.
    pub criterion_dir: Option<PathBuf>,
    /// Extra args after `--` for `cargo bench` (for example `--quick`).
    pub cargo_bench_args: Vec<String>,
    /// Working directory for `cargo bench` (repo root). Defaults to cwd.
    pub workspace_root: Option<PathBuf>,
}

/// Runs the requested suites and returns a [`BenchRunReport`].
///
/// # Errors
///
/// Propagates usage, environment, and run failures from either suite.
pub fn run_suites(options: &RunOptions, runner: RunnerInfo) -> Result<BenchRunReport, BenchError> {
    let mut cpu = Vec::new();
    let mut frame = None;
    let git_sha = git_sha(options.workspace_root.as_deref());

    if matches!(options.suite, Suite::Cpu | Suite::All) {
        cpu = run_cpu(options)?;
    }
    if matches!(options.suite, Suite::Frame | Suite::All) {
        frame = Some(run_frame(options, runner.clone())?);
    }

    Ok(BenchRunReport {
        schema: RUN_SCHEMA.to_string(),
        git_sha,
        exit_code: crate::error::exit_code::OK,
        runner,
        frame,
        cpu,
    })
}

fn run_cpu(options: &RunOptions) -> Result<Vec<CpuSample>, BenchError> {
    let criterion_dir = if let Some(dir) = &options.criterion_dir {
        dir.clone()
    } else {
        let root = options
            .workspace_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        invoke_cargo_bench(&root, &options.cargo_bench_args)?;
        criterion_dir_for(&root)
    };
    let samples = collect_cpu_samples(&criterion_dir)?;
    if samples.is_empty() {
        return Err(BenchError::Run(format!(
            "no criterion estimates under {}",
            criterion_dir.display()
        )));
    }
    Ok(samples)
}

/// Returns whether every catalog CPU id is present in `samples`.
#[must_use]
pub fn cpu_catalog_complete(samples: &[CpuSample]) -> bool {
    CPU_BENCH_IDS.iter().all(|want| {
        samples
            .iter()
            .any(|s| s.group == want.group && s.id == want.id)
    })
}

fn invoke_cargo_bench(root: &Path, extra: &[String]) -> Result<(), BenchError> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .args([
            "bench",
            "-p",
            "renderide-bench",
            "--bench",
            "cpu_hot",
            "--bench",
            "transparent_sort",
        ])
        .env("CARGO_TERM_COLOR", "never");
    if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
        cmd.env("CARGO_TARGET_DIR", target);
    }
    if !extra.is_empty() {
        cmd.arg("--").args(extra);
    }
    let status = cmd
        .status()
        .map_err(|err| BenchError::Env(format!("spawn cargo bench: {err}")))?;
    if !status.success() {
        return Err(BenchError::Run(format!("cargo bench failed with {status}")));
    }
    Ok(())
}

fn criterion_dir_for(root: &Path) -> PathBuf {
    if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(target).join("criterion");
    }
    root.join("target").join("criterion")
}

fn run_frame(options: &RunOptions, runner: RunnerInfo) -> Result<FrameBenchReport, BenchError> {
    let (width, height) = parse_resolution(&options.resolution)?;
    if options.runs == 0 {
        return Err(BenchError::Usage("runs must be >= 1".into()));
    }
    let output_path = options
        .output
        .clone()
        .map(|path| {
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                // When the front door writes one combined JSON, put the raw frame report beside it.
                path.with_file_name("framebench.json")
            } else {
                path.join("framebench.json")
            }
        })
        .unwrap_or_else(|| PathBuf::from("results/framebench.json"));
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| BenchError::Env(format!("create {}: {err}", parent.display())))?;
    }
    run_bench(
        &RunConfig {
            renderer_path: resolve_renderer_path(options.renderer.clone(), options.release),
            scenarios: options.scenarios.clone(),
            warmup_frames: options.warmup,
            measured_frames: options.frames,
            runs: options.runs,
            width,
            height,
            timeout: Duration::from_secs(options.timeout_seconds),
            verbose_renderer: options.verbose_renderer,
            renderer_stdout_to_stderr: true,
            windowed: options.windowed,
            output_path,
        },
        runner,
    )
    .map_err(map_frame_error)
}

fn map_frame_error(err: renderide_framebench::BenchError) -> BenchError {
    use renderide_framebench::exit_code as fe;
    match err.run_exit_code() {
        fe::USAGE => BenchError::Usage(err.to_string()),
        fe::ENVIRONMENT => BenchError::Env(err.to_string()),
        _ => BenchError::Run(err.to_string()),
    }
}

fn parse_resolution(text: &str) -> Result<(u32, u32), BenchError> {
    let Some((width, height)) = text.split_once(['x', 'X']) else {
        return Err(BenchError::Usage(format!(
            "resolution `{text}` must look like 256x256"
        )));
    };
    let width = width
        .parse::<u32>()
        .map_err(|_err| BenchError::Usage(format!("resolution width `{width}` is not a u32")))?;
    let height = height
        .parse::<u32>()
        .map_err(|_err| BenchError::Usage(format!("resolution height `{height}` is not a u32")))?;
    if width == 0 || height == 0 {
        return Err(BenchError::Usage(
            "resolution width and height must be >= 1".into(),
        ));
    }
    Ok((width, height))
}

fn git_sha(root: Option<&Path>) -> String {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "HEAD"]);
    if let Some(root) = root {
        cmd.current_dir(root);
    }
    cmd.output()
        .ok()
        .and_then(|out| {
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Detects runner info for the front door (re-export shape from framebench).
#[must_use]
pub fn detect() -> RunnerInfo {
    detect_runner()
}

#[cfg(test)]
mod tests {
    use super::{Suite, cpu_catalog_complete, parse_resolution};
    use crate::catalog::CPU_BENCH_IDS;
    use crate::report::CpuSample;

    #[test]
    fn suite_parse() {
        assert_eq!(Suite::parse("cpu").unwrap(), Suite::Cpu);
        assert_eq!(Suite::parse("frame").unwrap(), Suite::Frame);
        assert_eq!(Suite::parse("all").unwrap(), Suite::All);
        assert!(Suite::parse("gpu").is_err());
    }

    #[test]
    fn resolution_parse() {
        assert_eq!(parse_resolution("256x256").unwrap(), (256, 256));
        assert!(parse_resolution("256").is_err());
        assert!(parse_resolution("0x256").is_err());
    }

    #[test]
    fn catalog_complete_requires_every_id() {
        let samples: Vec<_> = CPU_BENCH_IDS
            .iter()
            .map(|id| CpuSample {
                group: id.group.into(),
                id: id.id.into(),
                median_ns: 1.0,
                ci95_ns: [1.0, 1.0],
                throughput: None,
            })
            .collect();
        assert!(cpu_catalog_complete(&samples));
        assert!(!cpu_catalog_complete(&samples[..samples.len() - 1]));
    }
}
