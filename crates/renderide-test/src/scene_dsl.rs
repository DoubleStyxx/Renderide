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
pub(crate) mod channel_zero;
pub mod host_info;
pub mod output;
mod quarantine_216;
pub mod region_color;
pub(crate) mod region_stats;
pub(crate) mod report_v2;
pub mod runner;
pub(crate) mod side_by_side;
pub mod suite;
pub mod tolerance;
pub mod warp_216_debug;
mod warp_pbs;

/// Labels matching WARP cases. The flat-exit tests use this with an injected clock.
#[cfg(test)]
pub(crate) use quarantine_216::apply_quarantine;
/// Whether a suite case is the time-limited #216 quarantine and not a pass.
pub(crate) use quarantine_216::case_is_quarantined as case_is_quarantined_216;
/// Whether this WARP case may end a flat-frame wait before the full budget.
pub(crate) use quarantine_216::early_flat_exit_applies;
/// Loud `QUARANTINED` line for one quarantined case.
pub(crate) use quarantine_216::quarantined_line;
/// Loud `WARN` line while the #216 quarantine is inside its last 48 hours.
pub(crate) use quarantine_216::warn_line;
/// Summary text for the known WARP `pbs_lit` image mismatch.
pub(crate) use warp_pbs::EXPECTED_FAILURE_LABEL as WARP_EXPECTED_FAILURE_LABEL;
/// Summary text when WARP passes `pbs_lit`. The suite still passes.
pub(crate) use warp_pbs::UNEXPECTED_PASS_LABEL as WARP_UNEXPECTED_PASS_LABEL;
/// Vendor `0x1414` and device `0x008c`, including a log that prints `0x8c`.
pub(crate) use warp_pbs::adapter_is_warp;
/// Whether a suite case is that expected failure and not a pass.
pub(crate) use warp_pbs::case_is_expected_failure as warp_pbs_is_expected;
/// Whether a suite case passed on WARP and carries the unexpected-pass label.
pub(crate) use warp_pbs::case_is_unexpected_pass as warp_pbs_is_unexpected_pass;
/// `None` until the renderer log for this PNG has a selected-adapter line.
pub(crate) use warp_pbs::output_warp_decision;
/// Loud `UNEXPECTED PASS` line for that WARP pass.
pub(crate) use warp_pbs::unexpected_pass_line as warp_unexpected_pass_line;

pub use cases::{CaseTemplate, IntegrationCase};
pub use output::CaseOutputLayout;
pub use runner::run_integration_case;
pub use suite::{
    SuiteCaseReport, SuiteConfig, SuiteFailureKind, SuiteReport, SuiteRunOutcome, run_suite,
};
pub use tolerance::{Combine, CriterionResult, Tolerance, ToleranceEvaluation};
