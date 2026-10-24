//! Errors from the frame-benchmark runner and compare tool.

use thiserror::Error;

/// Failure while running a scenario or comparing two reports.
#[derive(Debug, Error)]
pub enum BenchError {
    /// The harness could not complete a scenario.
    ///
    /// Boxed so [`BenchError`] stays within clippy's `result_large_err` limit.
    /// The timed-out frame on `FlatOrUnstableFrame` makes `HarnessError` large
    /// enough on Windows to fail that lint when this variant stores it inline.
    #[error(transparent)]
    Harness(Box<renderide_test::HarnessError>),
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

impl From<renderide_test::HarnessError> for BenchError {
    fn from(err: renderide_test::HarnessError) -> Self {
        Self::Harness(Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::BenchError;
    use renderide_test::HarnessError;

    #[test]
    fn harness_error_is_boxed_and_still_displays_the_flat_timeout() {
        assert!(
            size_of::<BenchError>() <= 128,
            "BenchError is {} bytes",
            size_of::<BenchError>()
        );
        let err = BenchError::from(HarnessError::FlatOrUnstableFrame {
            path: PathBuf::from("actual.png"),
            wait: Duration::from_secs(300),
            last_pending: 0,
            ready_chunks: 1,
            pending_chunks: 0,
            missing_chunks: 0,
            frame_count: 2,
            frame_pending: Some(0),
            gpu_done: true,
            dropped_draws: Some(0),
        });
        let text = err.to_string();
        assert!(text.contains("flat or unstable"), "{text}");
        assert!(text.contains("last pending=0"), "{text}");
        assert!(matches!(err, BenchError::Harness(_)));
    }
}
