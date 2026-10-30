//! Errors and process exit codes for the `renderide-bench` front door.
//!
//! Exit codes match B1 (`renderide-framebench`) and the rewrite plan: 0 ok, 1 regression,
//! 2 usage/input, 3 run failure, 5 environment.

use thiserror::Error;

/// Process exit codes shared with `renderide-framebench` and `frame_bench_gate.py` (0/1/2).
pub mod exit_code {
    /// Run wrote its report, or compare found no gated regression.
    pub const OK: u8 = 0;
    /// Compare: at least one gated metric regressed.
    pub const REGRESSION: u8 = 1;
    /// Usage or input error (bad flag, unknown suite, unusable compare input). clap also exits 2.
    pub const USAGE: u8 = 2;
    /// A suite ran but produced no usable measurement (criterion or framebench failure).
    pub const RUN_FAILURE: u8 = 3;
    /// Environment: missing binary, spawn failure, or filesystem error.
    pub const ENVIRONMENT: u8 = 5;
}

/// Failure while listing, running, or comparing benches.
#[derive(Debug, Error)]
pub enum BenchError {
    /// Filesystem failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse or write failure.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// A flag, suite, or report field was unusable.
    #[error("{0}")]
    Usage(String),
    /// A suite ran but its output could not be used.
    #[error("{0}")]
    Run(String),
    /// The machine could not run the benchmark.
    #[error("{0}")]
    Env(String),
}

impl BenchError {
    /// Exit code when this error ends `run`.
    #[must_use]
    pub fn run_exit_code(&self) -> u8 {
        match self {
            Self::Io(_) | Self::Env(_) => exit_code::ENVIRONMENT,
            Self::Usage(_) => exit_code::USAGE,
            Self::Json(_) | Self::Run(_) => exit_code::RUN_FAILURE,
        }
    }

    /// Exit code when this error ends `compare` (every error is usage/input, like the gate).
    #[must_use]
    pub fn compare_exit_code(&self) -> u8 {
        exit_code::USAGE
    }

    /// Stable machine-readable kind for the `--json` error document.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self.run_exit_code() {
            exit_code::USAGE => "usage",
            exit_code::ENVIRONMENT => "environment",
            _ => "run_failure",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BenchError, exit_code};

    #[test]
    fn exit_code_mapping() {
        assert_eq!(
            BenchError::Usage("x".into()).run_exit_code(),
            exit_code::USAGE
        );
        assert_eq!(
            BenchError::Env("x".into()).run_exit_code(),
            exit_code::ENVIRONMENT
        );
        assert_eq!(
            BenchError::Run("x".into()).run_exit_code(),
            exit_code::RUN_FAILURE
        );
        assert_eq!(
            BenchError::Io(std::io::Error::other("x")).run_exit_code(),
            exit_code::ENVIRONMENT
        );
        assert_eq!(
            BenchError::Run("x".into()).compare_exit_code(),
            exit_code::USAGE
        );
    }
}
