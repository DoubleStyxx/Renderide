//! Repo task runner. Gauge owns the perf-gate shadow port; others add subcommands later.
#![deny(unsafe_code)]
#![allow(
    clippy::print_stdout,
    reason = "xtask is a CLI; stdout/stderr are the user interface"
)]
#![allow(
    clippy::print_stderr,
    reason = "xtask is a CLI; stdout/stderr are the user interface"
)]
#![allow(
    clippy::enum_variant_names,
    reason = "subcommand names mirror scripts/ci tool names for discoverability"
)]
#![allow(
    clippy::too_many_lines,
    reason = "faithful port of frame_bench_gate.py keeps decision logic colocated"
)]

mod bench_gate;
mod bench_pr_assemble;
mod bench_pr_echo_verdict;
mod bench_pr_force_unstable;
mod bench_pr_merge_confirm;
mod bench_pr_report;
mod shadow_diff;

use clap::{Parser, Subcommand};

/// Renderide `cargo xtask` entry point.
#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    about = "Renderide repo tasks (perf gate + future helpers)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Noise-aware frame bench gate (port of scripts/ci/frame_bench_gate.py).
    BenchGate(bench_gate::Args),
    /// Merge multi-round confirm gate.json under the post-drift two-clean-FAIL policy.
    BenchPrMergeConfirm(bench_pr_merge_confirm::Args),
    /// Stamp gate.json as inconclusive: unstable runner.
    BenchPrForceUnstable(bench_pr_force_unstable::Args),
    /// Print a one-line GITHUB_STEP_SUMMARY verdict for a gate.json.
    BenchPrEchoVerdict(bench_pr_echo_verdict::Args),
    /// Assemble bench-compare.json from gate.json (+ optional cpu compare).
    BenchPrAssemble(bench_pr_assemble::Args),
    /// Build the bench-pr combined summary and optional bench-compare.json.
    BenchPrReport(bench_pr_report::Args),
    /// Shadow-mode: run Rust gate on the same inputs and diff vs Python gate.json (non-blocking).
    BenchGateShadowDiff(shadow_diff::Args),
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::BenchGate(a) => bench_gate::run(a),
        Command::BenchPrMergeConfirm(a) => bench_pr_merge_confirm::run(a),
        Command::BenchPrForceUnstable(a) => bench_pr_force_unstable::run(a),
        Command::BenchPrEchoVerdict(a) => bench_pr_echo_verdict::run(a),
        Command::BenchPrAssemble(a) => bench_pr_assemble::run(a),
        Command::BenchPrReport(a) => bench_pr_report::run(a),
        Command::BenchGateShadowDiff(a) => shadow_diff::run(a),
    };
    std::process::exit(code);
}
