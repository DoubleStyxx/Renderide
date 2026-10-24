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
use serde::Serialize;

use crate::compare::{
    CompareOptions, DEFAULT_ABS_FLOOR_MS, DEFAULT_THRESHOLD_PERCENT, compare_reports,
};
use crate::error::{BenchError, exit_code};
use crate::report::SCHEMA;
use crate::runner::{RunConfig, resolve_renderer_path, run_bench};
use crate::runner_env::{self, RunnerInfo};

/// Binary entry point. Exit codes are listed in [`exit_code`].
pub fn cli_main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return ExitCode::from(parse_error(&err)),
    };
    ExitCode::from(match cli.command {
        Command::Run(opts) => run_command(opts),
        Command::Compare(opts) => compare_command(opts),
    })
}

/// Clap parse failure. `--help` and `--version` print to stdout and exit 0, as with
/// `Cli::parse`. A usage error prints clap's message on stderr and exits 2; when `--json` is on
/// the command line, stdout also gets one error document (`kind: "usage"`), so `--json` callers
/// always get a document on a usage error.
fn parse_error(err: &clap::Error) -> u8 {
    if !err.use_stderr() {
        let _ = err.print();
        return exit_code::OK;
    }
    let _ = err.print();
    if std::env::args_os().any(|arg| arg == "--json") {
        let message = err.render().to_string();
        let usage = BenchError::Usage(message.trim().to_string());
        print_error_document(&usage, exit_code::USAGE, runner_env::detect());
    }
    exit_code::USAGE
}

fn run_command(opts: RunOpts) -> u8 {
    let json = opts.json;
    let runner = runner_env::detect();
    eprintln!("{}", runner_env::env_line(&runner));
    let output_path = resolve_output(&opts.output);
    let result = parse_resolution(&opts.resolution).and_then(|(width, height)| {
        run_bench(
            &RunConfig {
                renderer_path: resolve_renderer_path(opts.renderer, opts.release),
                scenarios: opts.scenario,
                warmup_frames: opts.warmup,
                measured_frames: opts.frames,
                runs: opts.runs,
                width,
                height,
                timeout: Duration::from_secs(opts.timeout_seconds),
                verbose_renderer: opts.verbose_renderer,
                renderer_stdout_to_stderr: json,
                windowed: opts.windowed,
                output_path: output_path.clone(),
            },
            runner.clone(),
        )
    });
    match result {
        Ok(report) => {
            let summary = format!(
                "runs={} scenarios={} timestamp_query={}",
                report.runs,
                report.scenarios.len(),
                report.timestamp_query
            );
            if json {
                // `--json`: stdout is exactly one JSON document (the report also on disk).
                match serde_json::to_string_pretty(&report) {
                    Ok(body) => println!("{body}"),
                    Err(err) => {
                        eprintln!("renderide-framebench: json: {err}");
                        return exit_code::ENVIRONMENT;
                    }
                }
                eprintln!("{}", output_path.display());
                eprintln!("{summary}");
            } else {
                println!("{}", output_path.display());
                println!("{summary}");
            }
            exit_code::OK
        }
        Err(err) => {
            let code = err.run_exit_code();
            eprintln!("renderide-framebench: {err} (exit {code})");
            if json {
                print_error_document(&err, code, runner);
            }
            code
        }
    }
}

/// `--json` failure document: same `schema` and `runner` as a report, plus `error`, no
/// `scenarios`. `frame_bench_gate.py` rejects it as an input error.
#[derive(Serialize)]
struct ErrorDocument {
    schema: u32,
    error: ErrorBody,
    runner: RunnerInfo,
}

#[derive(Serialize)]
struct ErrorBody {
    exit_code: u8,
    kind: &'static str,
    message: String,
}

fn print_error_document(err: &BenchError, code: u8, runner: RunnerInfo) {
    let doc = ErrorDocument {
        schema: SCHEMA,
        error: ErrorBody {
            exit_code: code,
            kind: err.kind(),
            message: err.to_string(),
        },
        runner,
    };
    match serde_json::to_string_pretty(&doc) {
        Ok(body) => println!("{body}"),
        Err(json_err) => eprintln!("renderide-framebench: json: {json_err}"),
    }
}

fn compare_command(opts: CompareOpts) -> u8 {
    let outcome = compare_reports(
        &opts.baseline,
        &opts.candidate,
        &CompareOptions {
            threshold_percent: opts.threshold,
            gate: opts.gate,
            abs_floor_ms: opts.abs_floor_ms,
        },
    );
    match outcome {
        Ok(outcome) => {
            print!("{}", outcome.markdown);
            if outcome.failed {
                exit_code::REGRESSION
            } else {
                exit_code::OK
            }
        }
        Err(err) => {
            // Unreadable or invalid inputs, like frame_bench_gate.py's exit 2.
            eprintln!("renderide-framebench: {err} (exit {})", exit_code::USAGE);
            exit_code::USAGE
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
            "resolution width and height must be >= 1".to_string(),
        ));
    }
    Ok((width, height))
}

#[derive(Parser, Debug)]
#[command(
    name = "renderide-framebench",
    about = "Renderer frame benchmarks. Gates are relative percent; an opt-in absolute ms floor (--abs-floor-ms) can clear small deltas.",
    after_help = "Exit codes: 0 ok, 1 compare found a gated regression, 2 usage or input error, \
3 run failure (renderer ran, no usable measurement), 5 environment (renderer missing, spawn or \
filesystem failure).",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run fixed scenarios and write one JSON report (schema 2).
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
    /// Inherit renderer stdout and stderr. With `--json`, renderer stdout goes to stderr.
    #[arg(long, default_value_t = false)]
    verbose_renderer: bool,
    /// Print the report as the only stdout output (human lines go to stderr). On failure,
    /// stdout is one error document `{schema, error{exit_code, kind, message}, runner}`.
    /// `--output` is still written on success.
    #[arg(long, default_value_t = false)]
    json: bool,
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
    /// Millisecond-metric deltas at or below this never fail, whatever their percent.
    /// Default 0 (off); opt-in until open question O8 decides a default.
    #[arg(long, default_value_t = DEFAULT_ABS_FLOOR_MS)]
    abs_floor_ms: f64,
}
