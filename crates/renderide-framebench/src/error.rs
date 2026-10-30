//! Errors from the frame-benchmark runner and compare tool, and their process exit codes.

use thiserror::Error;

use renderide_test::HarnessError;

/// Process exit codes shared by `run` and `compare`.
///
/// The numbering follows the bench rewrite plan's table for both tools. When several things go
/// wrong, the first error ends the process, so there is no priority merge here.
pub mod exit_code {
    /// Run wrote its report, or compare found no gated regression.
    pub const OK: u8 = 0;
    /// Compare: at least one gated metric regressed.
    pub const REGRESSION: u8 = 1;
    /// Usage or input error: a bad flag value, an unknown scenario, or an unusable compare input.
    /// clap also exits 2 for flags it rejects itself.
    pub const USAGE: u8 = 2;
    /// The renderer ran but produced no usable measurement (timeout, crash, bad raw report).
    pub const RUN_FAILURE: u8 = 3;
    /// Environment: renderer binary missing, spawn failure, or a filesystem error.
    pub const ENVIRONMENT: u8 = 5;
}

/// Failure while running a scenario or comparing two reports.
#[derive(Debug, Error)]
pub enum BenchError {
    /// The harness could not complete a scenario.
    #[error(transparent)]
    Harness(#[from] HarnessError),
    /// Filesystem failure while reading or writing a report.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A report was not valid JSON.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// A flag, scenario, or report field was unusable.
    #[error("{0}")]
    Message(String),
    /// A flag value or scenario name was rejected before anything ran.
    #[error("{0}")]
    Usage(String),
    /// The renderer ran but its output could not be used.
    #[error("{0}")]
    Run(String),
    /// The machine could not run the benchmark (logging, filesystem, missing tools).
    #[error("{0}")]
    Env(String),
}

impl BenchError {
    /// Exit code for this error when it ends `renderide-framebench run`.
    ///
    /// `compare` maps every error to [`exit_code::USAGE`] instead, like `frame_bench_gate.py`'s
    /// input errors.
    pub fn run_exit_code(&self) -> u8 {
        match self {
            Self::Harness(err) => harness_exit_code(err),
            Self::Io(_) | Self::Env(_) => exit_code::ENVIRONMENT,
            Self::Usage(_) => exit_code::USAGE,
            Self::Json(_) | Self::Message(_) | Self::Run(_) => exit_code::RUN_FAILURE,
        }
    }

    /// Stable machine-readable kind for the `--json` error document.
    pub fn kind(&self) -> &'static str {
        match self.run_exit_code() {
            exit_code::USAGE => "usage",
            exit_code::ENVIRONMENT => "environment",
            _ => "run_failure",
        }
    }
}

fn harness_exit_code(err: &HarnessError) -> u8 {
    match err {
        HarnessError::RendererBinaryMissing(_)
        | HarnessError::SpawnRenderer(_)
        | HarnessError::Io(_) => exit_code::ENVIRONMENT,
        HarnessError::UnknownBenchScenario(_) | HarnessError::UnknownCase(_) => exit_code::USAGE,
        HarnessError::QueueOptions(_)
        | HarnessError::HandshakeTimeout(_)
        | HarnessError::AssetAckTimeout(..)
        | HarnessError::PngOutputMissing { .. }
        | HarnessError::PipelineNotReady { .. }
        | HarnessError::FlatOrUnstableFrame { .. }
        | HarnessError::PngRead { .. }
        | HarnessError::PngWrite { .. }
        | HarnessError::FlatImage { .. }
        | HarnessError::GoldenMissing(_)
        | HarnessError::GoldenMismatch { .. }
        | HarnessError::GltfFixture { .. }
        | HarnessError::ImageCompare(_)
        | HarnessError::BenchReportMissing { .. }
        | HarnessError::SuiteFailed { .. } => exit_code::RUN_FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use renderide_test::HarnessError;

    use super::{BenchError, exit_code};

    #[test]
    fn environment_errors_exit_5() {
        for err in [
            BenchError::Harness(HarnessError::RendererBinaryMissing(PathBuf::from("/nope"))),
            BenchError::Harness(HarnessError::SpawnRenderer(std::io::Error::other("x"))),
            BenchError::Io(std::io::Error::other("disk full")),
            BenchError::Env("logging".to_string()),
        ] {
            assert_eq!(err.run_exit_code(), exit_code::ENVIRONMENT, "{err}");
            assert_eq!(err.kind(), "environment");
        }
    }

    #[test]
    fn run_failures_exit_3() {
        for err in [
            BenchError::Harness(HarnessError::HandshakeTimeout(Duration::from_secs(1))),
            BenchError::Harness(HarnessError::PipelineNotReady {
                path: PathBuf::from("capture.png"),
                wait: Duration::from_secs(1),
            }),
            BenchError::Harness(HarnessError::FlatOrUnstableFrame {
                path: PathBuf::from("capture.png"),
                wait: Duration::from_secs(1),
                last_pending: 0,
                ready_chunks: 1,
                pending_chunks: 0,
                missing_chunks: 0,
                frame_count: 1,
            }),
            BenchError::Harness(HarnessError::BenchReportMissing {
                path: PathBuf::from("r.json"),
                wait: Duration::from_secs(1),
                renderer_state: "exited".to_string(),
            }),
            BenchError::Run("incomplete raw report".to_string()),
            BenchError::Message("legacy".to_string()),
        ] {
            assert_eq!(err.run_exit_code(), exit_code::RUN_FAILURE, "{err}");
            assert_eq!(err.kind(), "run_failure");
        }
    }

    #[test]
    fn usage_errors_exit_2() {
        for err in [
            BenchError::Usage("runs must be >= 1".to_string()),
            BenchError::Harness(HarnessError::UnknownBenchScenario("nope".to_string())),
        ] {
            assert_eq!(err.run_exit_code(), exit_code::USAGE, "{err}");
            assert_eq!(err.kind(), "usage");
        }
    }
}
