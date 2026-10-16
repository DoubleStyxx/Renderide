//! Command line for `renderide-framebench`.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI tool: stdout and stderr are the user-facing interface"
)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};

use crate::compare::{CompareOptions, DEFAULT_THRESHOLD_PERCENT, compare_reports};
use crate::error::BenchError;
use crate::runner::{RunConfig, resolve_renderer_path, run_bench};

/// Binary entry point.
pub fn cli_main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("renderide-framebench: {err}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: Cli) -> Result<ExitCode, BenchError> {
    match cli.command {
        Command::Run(opts) => {
            let output_path = resolve_output(&opts.output);
            let (width, height) = parse_resolution(&opts.resolution)?;
            let report = run_bench(&RunConfig {
                renderer_path: resolve_renderer_path(opts.renderer, opts.release),
                scenarios: opts.scenario,
                warmup_frames: opts.warmup,
                measured_frames: opts.frames,
                runs: opts.runs,
                width,
                height,
                timeout: Duration::from_secs(opts.timeout_seconds),
                verbose_renderer: opts.verbose_renderer,
                windowed: opts.windowed,
                output_path: output_path.clone(),
            })?;
            println!("{}", output_path.display());
            println!(
                "runs={} scenarios={} timestamp_query={}",
                report.runs,
                report.scenarios.len(),
                report.timestamp_query
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Compare(opts) => {
            let gate = if opts.gate.is_empty() {
                Vec::new()
            } else {
                opts.gate
            };
            let outcome = compare_reports(
                &opts.baseline,
                &opts.candidate,
                &CompareOptions {
                    threshold_percent: opts.threshold,
                    gate,
                },
            )?;
            print!("{}", outcome.markdown);
            if outcome.failed {
                Ok(ExitCode::from(1))
            } else {
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

fn resolve_output(path: &std::path::Path) -> PathBuf {
    if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        path.to_path_buf()
    } else {
        path.join("framebench.json")
    }
}

fn parse_resolution(text: &str) -> Result<(u32, u32), BenchError> {
    let Some((width, height)) = text.split_once(['x', 'X']) else {
        return Err(BenchError::Message(format!(
            "resolution `{text}` must look like 256x256"
        )));
    };
    let width = width
        .parse::<u32>()
        .map_err(|_err| BenchError::Message(format!("resolution width `{width}` is not a u32")))?;
    let height = height.parse::<u32>().map_err(|_err| {
        BenchError::Message(format!("resolution height `{height}` is not a u32"))
    })?;
    if width == 0 || height == 0 {
        return Err(BenchError::Message(
            "resolution width and height must be >= 1".to_string(),
        ));
    }
    Ok((width, height))
}

#[derive(Parser, Debug)]
#[command(
    name = "renderide-framebench",
    about = "Renderer frame benchmarks. Gates are relative percent only, never absolute milliseconds.",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run fixed scenarios and write one JSON report.
    Run(RunOpts),
    /// Compare two reports and exit 1 when a gated metric regresses.
    Compare(CompareOpts),
}

#[derive(Parser, Debug)]
struct RunOpts {
    /// Path to `renderide-renderer`. Overrides `--release`.
    #[arg(long)]
    renderer: Option<PathBuf>,
    /// Use `target/release/renderide-renderer` when `--renderer` is omitted.
    #[arg(long, default_value_t = false)]
    release: bool,
    /// Scenario name. Repeat for more than one. Default: unlit_sphere and multi_primitive_unlit.
    #[arg(long)]
    scenario: Vec<String>,
    /// Warmup frames discarded before measurement.
    #[arg(long, default_value_t = 8)]
    warmup: u32,
    /// Measured frames per run.
    #[arg(long, default_value_t = 32)]
    frames: u32,
    /// Renderer process invocations per scenario. The report stores the median of per-run percentiles.
    #[arg(long, default_value_t = 1)]
    runs: u32,
    /// JSON file, or a directory that receives `framebench.json`.
    #[arg(long, default_value = "results/framebench.json")]
    output: PathBuf,
    /// Headless framebuffer size, `WIDTHxHEIGHT`. Windowed mode does not pin the winit window.
    #[arg(long, default_value = "256x256")]
    resolution: String,
    /// Open a window instead of `--headless`.
    #[arg(long, default_value_t = false)]
    windowed: bool,
    /// Seconds to wait for the raw report after the scene submit.
    #[arg(long, default_value_t = 180)]
    timeout_seconds: u64,
    /// Inherit renderer stdout and stderr.
    #[arg(long, default_value_t = false)]
    verbose_renderer: bool,
}

#[derive(Parser, Debug)]
struct CompareOpts {
    /// Baseline report (for example master).
    #[arg(long)]
    baseline: PathBuf,
    /// Candidate report (for example the PR head).
    #[arg(long)]
    candidate: PathBuf,
    /// Fail when a gated relative delta is strictly greater than this percent.
    #[arg(long, default_value_t = DEFAULT_THRESHOLD_PERCENT)]
    threshold: f64,
    /// Metric to gate, repeatable. Default: cpu_frame_ms.p95.
    #[arg(long)]
    gate: Vec<String>,
}
