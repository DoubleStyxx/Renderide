//! Modular scene-driven integration test surface.
//!
//! Each integration test case is described by an [`cases::IntegrationCase`] (name, golden
//! path, render resolution, comparison [`tolerance::Tolerance`]) and dispatches to one of the
//! built-in scene templates that drive the harness end-to-end. New cases are added by
//! authoring a builder in [`cases`] and wiring its template into
//! [`runner::run_integration_case`].
//!
//! This is the modular spine for the integration suite: golden comparison, output layout, and
//! report emission all flow through this module. The intent is to grow the suite by adding
//! cases and templates rather than copy-pasting end-to-end orchestration.

pub mod band_count;
pub mod cases;
pub mod host_info;
pub mod output;
pub mod region_color;
pub mod runner;
pub mod suite;
pub mod tolerance;
mod warp_pbs;

/// Summary text for the known WARP `pbs_lit` image mismatch.
pub(crate) use warp_pbs::EXPECTED_FAILURE_LABEL as WARP_EXPECTED_FAILURE_LABEL;
/// Vendor `0x1414` and device `0x008c`, including a log that prints `0x8c`.
#[cfg(test)]
pub(crate) use warp_pbs::adapter_is_warp;
/// Whether a suite case is that expected failure and not a pass.
pub(crate) use warp_pbs::case_is_expected_failure as warp_pbs_is_expected;
/// `None` until the renderer log for this PNG has a selected-adapter line.
pub(crate) use warp_pbs::output_warp_decision;

pub use cases::{CaseTemplate, IntegrationCase};
pub use output::CaseOutputLayout;
pub use runner::run_integration_case;
pub use suite::{SuiteCaseReport, SuiteConfig, SuiteReport, SuiteRunOutcome, run_suite};
pub use tolerance::{Combine, CriterionResult, Tolerance, ToleranceEvaluation};
