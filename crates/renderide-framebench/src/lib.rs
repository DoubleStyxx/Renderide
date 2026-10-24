//! Renderer frame benchmarks.
//!
//! The runner launches fixed scenes through the headless harness and writes relative
//! percentiles. [`compare_reports`] fails only when a gated metric regresses by more than
//! the threshold percent. Absolute milliseconds are not a gate; deltas at or below
//! [`ABS_FLOOR_MS`] never fail. Reports are schema 2 ([`SCHEMA`]), a superset of schema 1 with
//! a [`RunnerInfo`] block. Exit codes are in [`exit_code`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cli;
mod compare;
mod error;
mod report;
mod runner;
mod runner_env;
mod scenario;
mod stats;

pub use cli::cli_main;
pub use compare::{
    ABS_FLOOR_MS, CompareOptions, CompareOutcome, DEFAULT_GATE_METRIC, DEFAULT_THRESHOLD_PERCENT,
    compare_loaded, compare_reports,
};
pub use error::{BenchError, exit_code};
pub use report::{
    AdapterInfo, ControlApi, FrameBenchReport, SCHEMA, SUPPORTED_SCHEMAS, ScenarioReport,
    schema_supported,
};
pub use runner::{RunConfig, resolve_renderer_path, run_bench};
pub use runner_env::{IcdInfo, RunnerInfo, detect as detect_runner, env_line};
pub use scenario::ScriptStep;
pub use stats::Percentiles;
