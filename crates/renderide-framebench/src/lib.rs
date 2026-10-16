//! Renderer frame benchmarks.
//!
//! The runner launches fixed scenes through the headless harness and writes relative
//! percentiles. [`compare_reports`] fails only when a gated metric regresses by more than
//! the threshold percent. Absolute milliseconds are not a gate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cli;
mod compare;
mod error;
mod report;
mod runner;
mod scenario;
mod stats;

pub use cli::cli_main;
pub use compare::{
    CompareOptions, CompareOutcome, DEFAULT_GATE_METRIC, DEFAULT_THRESHOLD_PERCENT, compare_loaded,
    compare_reports,
};
pub use error::BenchError;
pub use report::{AdapterInfo, ControlApi, FrameBenchReport, ScenarioReport};
pub use runner::{RunConfig, resolve_renderer_path, run_bench};
pub use scenario::ScriptStep;
pub use stats::Percentiles;
