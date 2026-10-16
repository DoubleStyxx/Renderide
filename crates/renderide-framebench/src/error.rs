//! Errors from the frame-benchmark runner and compare tool.

use thiserror::Error;

/// Failure while running a scenario or comparing two reports.
#[derive(Debug, Error)]
pub enum BenchError {
    /// The harness could not complete a scenario.
    #[error(transparent)]
    Harness(#[from] renderide_test::HarnessError),
    /// Filesystem failure while reading or writing a report.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A report was not valid JSON.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// A flag, scenario, or report field was unusable.
    #[error("{0}")]
    Message(String),
}
