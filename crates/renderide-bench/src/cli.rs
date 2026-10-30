//! Command line for the `renderide-bench` front door.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI tool: stdout and stderr are the user-facing interface"
)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use renderide_framebench::{DEFAULT_ABS_FLOOR_MS, DEFAULT_THRESHOLD_PERCENT, env_line};

use crate::catalog::{CPU_BENCH_IDS, FRAME_SCENARIOS};
use crate::compare_front::{
    DEFAULT_FLAG_THRESHOLD_PERCENT, FrontCompareOptions, compare_paths, decision_rule_doc,
};
use crate::error::{BenchError, exit_code};
use crate::report::{BenchListReport, ErrorBody, ErrorDocument, LIST_SCHEMA, RUN_SCHEMA};
use crate::run_suite::{RunOptions, Suite, detect, run_suites};

/// Binary entry point. Exit codes are listed in [`exit_code`].
pub fn cli_main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return ExitCode::from(parse_error(&err)),
    };
    ExitCode::from(match cli.command {
        Command::Run(opts) => run_command(opts),
        Command::Compare(opts) => compare_command(opts),
        Command::List(opts) => list_command(opts),
    })
}

fn parse_error(err: &clap::Error) -> u8 {
    if !err.use_stderr() {
        let _ = err.print();
        return exit_code::OK;
    }
    let _ = err.print();
    if std::env::args_os().any(|arg| arg == "--json") {
        let message = err.render().to_string();
        let usage = BenchError::Usage(message.trim().to_string());
        print_error_document(&usage, exit_code::USAGE);
    }
    exit_code::USAGE
}

fn run_command(opts: RunOpts) -> u8 {
    let json = opts.json;
    let runner = detect();
    eprintln!("{}", env_line(&runner));
    eprintln!("decision_rule: {}", decision_rule_doc());
    let suite = match Suite::parse(&opts.suite) {
        Ok(suite) => suite,
        Err(err) => {
            eprintln!("renderide-bench: {err} (exit {})", exit_code::USAGE);
            if json {
                print_error_document(&err, exit_code::USAGE);
            }
            return exit_code::USAGE;
        }
    };
    let options = RunOptions {
        suite,
        scenarios: opts.scenario,
        warmup: opts.warmup,
        frames: opts.frames,
        runs: opts.runs,
        resolution: opts.resolution,
        renderer: opts.renderer,
        release: opts.release,
        timeout_seconds: opts.timeout_seconds,
        verbose_renderer: opts.verbose_renderer,
        windowed: opts.windowed,
        output: opts.output.clone(),
        criterion_dir: opts.criterion_dir,
        cargo_bench_args: opts.cargo_bench_args,
        workspace_root: None,
    };
    match run_suites(&options, runner) {
        Ok(mut report) => {
            if let Some(path) = &opts.output
                && let Err(err) = write_json(path, &report)
            {
                eprintln!("renderide-bench: {err} (exit {})", exit_code::ENVIRONMENT);
                if json {
                    print_error_document(&err, exit_code::ENVIRONMENT);
                }
                return exit_code::ENVIRONMENT;
            }
            report.exit_code = exit_code::OK;
            if json {
                match serde_json::to_string_pretty(&report) {
                    Ok(body) => println!("{body}"),
                    Err(err) => {
                        eprintln!("renderide-bench: json: {err}");
                        return exit_code::ENVIRONMENT;
                    }
                }
            } else {
                println!(
                    "suite={} cpu_samples={} frame_scenarios={}",
                    opts.suite,
                    report.cpu.len(),
                    report
                        .frame
                        .as_ref()
                        .map_or(0, |frame| frame.scenarios.len())
                );
            }
            exit_code::OK
        }
        Err(err) => {
            let code = err.run_exit_code();
            eprintln!("renderide-bench: {err} (exit {code})");
            if json {
                print_error_document(&err, code);
            }
            code
        }
    }
}

fn compare_command(opts: CompareOpts) -> u8 {
    let options = FrontCompareOptions {
        fail_threshold: opts.fail_threshold,
        flag_threshold: opts.flag_threshold,
        abs_floor_ms: opts.abs_floor_ms,
    };
    match compare_paths(&opts.base, &opts.head, &options) {
        Ok(report) => {
            if opts.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(body) => println!("{body}"),
                    Err(err) => {
                        eprintln!("renderide-bench: json: {err}");
                        return exit_code::ENVIRONMENT;
                    }
                }
            } else {
                print_compare_markdown(&report);
            }
            report.exit_code
        }
        Err(err) => {
            let code = err.compare_exit_code();
            eprintln!("renderide-bench: {err} (exit {code})");
            if opts.json {
                print_error_document(&err, code);
            }
            code
        }
    }
}

fn list_command(opts: ListOpts) -> u8 {
    let report = BenchListReport {
        schema: LIST_SCHEMA.to_string(),
        cpu: CPU_BENCH_IDS
            .iter()
            .map(super::catalog::CpuBenchId::path)
            .collect(),
        frame: FRAME_SCENARIOS.iter().map(|s| (*s).to_string()).collect(),
    };
    if opts.json {
        match serde_json::to_string_pretty(&report) {
            Ok(body) => println!("{body}"),
            Err(err) => {
                eprintln!("renderide-bench: json: {err}");
                return exit_code::ENVIRONMENT;
            }
        }
    } else {
        println!("cpu:");
        for id in &report.cpu {
            println!("  {id}");
        }
        println!("frame:");
        for name in &report.frame {
            println!("  {name}");
        }
    }
    exit_code::OK
}

fn print_compare_markdown(report: &crate::report::BenchCompareReport) {
    println!("# Bench compare\n");
    println!("fail_threshold: >{:.4}%", report.fail_threshold);
    println!("flag_threshold: >{:.4}%", report.flag_threshold);
    if report.abs_floor_ms > 0.0 {
        println!(
            "abs_floor: ms deltas <= {:.4} ms never fail",
            report.abs_floor_ms
        );
    }
    println!("decision_rule: {}\n", report.decision_rule);
    println!("| suite | name | metric | base | head | delta | verdict |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for row in &report.rows {
        let base = row.base.map_or_else(|| "n/a".into(), |v| format!("{v:.4}"));
        let head = row.head.map_or_else(|| "n/a".into(), |v| format!("{v:.4}"));
        let delta = row
            .delta_pct
            .map_or_else(|| "n/a".into(), |v| format!("{v:+.2}%"));
        let verdict = match row.verdict {
            crate::report::Verdict::Pass => "pass",
            crate::report::Verdict::Flag => "flag",
            crate::report::Verdict::Fail => "fail",
            crate::report::Verdict::New => "new",
            crate::report::Verdict::Missing => "missing",
        };
        println!(
            "| {} | {} | {} | {base} | {head} | {delta} | {verdict} |",
            row.suite, row.name, row.metric
        );
    }
    println!();
    if report.exit_code == exit_code::REGRESSION {
        println!("result: fail");
    } else {
        println!("result: pass");
    }
}

fn write_json(
    path: &std::path::Path,
    report: &crate::report::BenchRunReport,
) -> Result<(), BenchError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(BenchError::Io)?;
    }
    let body = serde_json::to_vec_pretty(report)?;
    std::fs::write(path, body).map_err(BenchError::Io)
}

fn print_error_document(err: &BenchError, code: u8) {
    let doc = ErrorDocument {
        schema: RUN_SCHEMA.to_string(),
        error: ErrorBody {
            exit_code: code,
            kind: err.kind(),
            message: err.to_string(),
        },
        runner: detect(),
    };
    match serde_json::to_string_pretty(&doc) {
        Ok(body) => println!("{body}"),
        Err(json_err) => eprintln!("renderide-bench: json: {json_err}"),
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "renderide-bench",
    about = "Front door for criterion CPU benches and renderide-framebench. One decision rule shared with framebench compare and frame_bench_gate.py.",
    after_help = "Exit codes: 0 ok, 1 compare found a gated regression, 2 usage or input error, \
3 run failure, 5 environment. Absolute floor default is 0 (O8 open).",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run CPU and/or frame suites and emit a renderide-bench/1 document.
    Run(RunOpts),
    /// Compare two reports or criterion trees.
    Compare(CompareOpts),
    /// List known CPU ids and frame scenarios.
    List(ListOpts),
}

#[derive(Parser, Debug)]
struct RunOpts {
    /// Suite: `cpu`, `frame`, or `all`.
    #[arg(long, default_value = "cpu")]
    suite: String,
    /// Frame scenario name (repeatable). Default: framebench defaults.
    #[arg(long)]
    scenario: Vec<String>,
    /// Print one JSON document on stdout; human lines go to stderr.
    #[arg(long, default_value_t = false)]
    json: bool,
    /// Optional path to write the run document.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Framebench warmup frames.
    #[arg(long, default_value_t = 8)]
    warmup: u32,
    /// Framebench measured frames.
    #[arg(long, default_value_t = 32)]
    frames: u32,
    /// Framebench runs per scenario.
    #[arg(long, default_value_t = 1)]
    runs: u32,
    /// Framebuffer size `WIDTHxHEIGHT`.
    #[arg(long, default_value = "256x256")]
    resolution: String,
    /// Path to `renderide-renderer`.
    #[arg(long)]
    renderer: Option<PathBuf>,
    /// Prefer the release renderer when `--renderer` is omitted.
    #[arg(long, default_value_t = false)]
    release: bool,
    /// Seconds to wait for a framebench raw report.
    #[arg(long, default_value_t = 180)]
    timeout_seconds: u64,
    /// Inherit renderer stdout/stderr (renderer stdout goes to stderr).
    #[arg(long, default_value_t = false)]
    verbose_renderer: bool,
    /// Open a window instead of headless framebench.
    #[arg(long, default_value_t = false)]
    windowed: bool,
    /// Collect CPU samples from this criterion directory without running `cargo bench`.
    #[arg(long)]
    criterion_dir: Option<PathBuf>,
    /// Extra args forwarded to `cargo bench` after `--` (for example `--quick`).
    #[arg(long = "bench-arg", value_name = "ARG")]
    cargo_bench_args: Vec<String>,
}

#[derive(Parser, Debug)]
struct CompareOpts {
    /// Baseline run report, framebench report, or criterion directory.
    #[arg(long)]
    base: PathBuf,
    /// Head run report, framebench report, or criterion directory.
    #[arg(long)]
    head: PathBuf,
    /// Print one JSON document on stdout.
    #[arg(long, default_value_t = false)]
    json: bool,
    /// Fail when a gated relative delta is strictly greater than this percent.
    #[arg(long, default_value_t = DEFAULT_THRESHOLD_PERCENT)]
    fail_threshold: f64,
    /// Flag (report-only) when delta is strictly greater than this percent.
    #[arg(long, default_value_t = DEFAULT_FLAG_THRESHOLD_PERCENT)]
    flag_threshold: f64,
    /// Millisecond deltas at or below this never fail. Default 0 (off); O8 open.
    #[arg(long, default_value_t = DEFAULT_ABS_FLOOR_MS)]
    abs_floor_ms: f64,
}

#[derive(Parser, Debug)]
struct ListOpts {
    /// Print one JSON document on stdout.
    #[arg(long, default_value_t = false)]
    json: bool,
}
