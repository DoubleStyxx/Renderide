//! Error types for the renderide-test harness.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

/// Top-level harness error.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// The renderer binary could not be located on disk.
    #[error(
        "renderer binary not found at {0}; build with `cargo build -p renderide` (use `--profile dev-fast` or `--release` as needed)"
    )]
    RendererBinaryMissing(PathBuf),
    /// Spawning the renderer process failed.
    #[error("spawn renderer process: {0}")]
    SpawnRenderer(#[source] std::io::Error),
    /// Building `interprocess::QueueOptions` failed (capacity invalid, etc.).
    #[error("queue options invalid: {0}")]
    QueueOptions(String),
    /// The handshake never completed within the configured timeout.
    #[error("handshake timed out after {0:?}")]
    HandshakeTimeout(Duration),
    /// An asset upload acknowledgement never arrived.
    #[error("asset ack timed out after {0:?} ({1})")]
    AssetAckTimeout(Duration, &'static str),
    /// PNG output never appeared / never refreshed within the configured wait.
    #[error("expected fresh PNG output at {path} within {wait:?}")]
    PngOutputMissing {
        /// Output PNG path the renderer was instructed to write.
        path: PathBuf,
        /// Maximum wall-clock wait before giving up.
        wait: Duration,
    },
    /// A stable PNG arrived, but every candidate frame still had a compiling or skipped material draw.
    #[error(
        "headless PNG at {path} stayed pipeline-pending for {wait:?}; capture refused frames whose material draws were still compiling or were skipped because no pipeline was available"
    )]
    PipelineNotReady {
        /// Output PNG path the renderer was instructed to write.
        path: PathBuf,
        /// Maximum wall-clock wait before giving up.
        wait: Duration,
    },
    /// The capture timed out on a frame whose pending count was already zero.
    ///
    /// The image was flat or still changing, so this is not a pipeline that never
    /// became ready. `last_pending` is the last `renderide_pipeline_pending` value.
    #[error(
        "headless PNG at {path} timed out after {wait:?} on a flat or unstable frame; last pending={last_pending}; chunk stats: ready={ready_chunks} pending={pending_chunks} missing={missing_chunks}; frames={frame_count}"
    )]
    FlatOrUnstableFrame {
        /// Output PNG path the renderer was instructed to write.
        path: PathBuf,
        /// Maximum wall-clock wait before giving up.
        wait: Duration,
        /// Last pending-pipeline count read from the PNG chunk.
        last_pending: u64,
        /// Stable frames whose chunk was `0`.
        ready_chunks: u64,
        /// Stable frames whose chunk was greater than zero.
        pending_chunks: u64,
        /// Stable frames with no numeric pending chunk.
        missing_chunks: u64,
        /// Stable frames observed before the deadline.
        frame_count: u64,
        /// Pending count on the frame the harness timed out on.
        ///
        /// `None` means no frame was observed. It is not zero.
        frame_pending: Option<u64>,
        /// Whether that frame's GPU submission had finished.
        ///
        /// False when no frame was observed.
        gpu_done: bool,
        /// Dropped draws on that frame. `None` is not zero.
        dropped_draws: Option<u64>,
    },
    /// Reading or decoding the PNG output failed.
    #[error("read png {path}: {source}")]
    PngRead {
        /// Path that failed to load.
        path: PathBuf,
        /// Underlying image crate error.
        #[source]
        source: image::ImageError,
    },
    /// Writing the diff or actual PNG to disk failed.
    #[error("write png {path}: {source}")]
    PngWrite {
        /// Output path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: image::ImageError,
    },
    /// Rendered image has no per-channel variation (clear-only or nearly flat); geometry did not draw.
    #[error(
        "rendered image at {path} is a flat single color {color:?}; the renderer produced no draws"
    )]
    FlatImage {
        /// Path to the offending PNG.
        path: PathBuf,
        /// Sample RGBA (typically the first pixel).
        color: [u8; 4],
    },
    /// Golden image is missing on disk; bless it with `run --bless`.
    #[error(
        "golden image not found at {0}; run `renderide-test run --bless --case <name>` to create it, or add `--force` to overwrite"
    )]
    GoldenMissing(PathBuf),
    /// Perceptual diff failed against the configured threshold.
    #[error(
        "perceptual diff failed: SSIM={score:.4} below threshold {threshold:.4}; diff written to {diff_path}"
    )]
    GoldenMismatch {
        /// Computed SSIM score.
        score: f64,
        /// Required minimum SSIM score.
        threshold: f64,
        /// Path of the saved diff visualization.
        diff_path: PathBuf,
    },
    /// Generic IO failure (file copy, rename, etc.).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Loading or converting a GLB fixture failed.
    #[error("load GLB fixture {path}: {message}")]
    GltfFixture {
        /// Fixture path being loaded.
        path: PathBuf,
        /// Human-readable failure reason.
        message: String,
    },
    /// `image-compare` failed to compute a similarity score.
    #[error("image-compare: {0}")]
    ImageCompare(String),
    /// A requested named scene case does not exist in the suite registry.
    #[error("unknown integration case `{0}`")]
    UnknownCase(String),
    /// `--group` does not name a known opt-in case group.
    #[error("unknown suite group `{0}`")]
    UnknownGroup(String),
    /// The frame benchmark did not write a complete report before the deadline.
    #[error("frame bench report missing at {path} after {wait:?} (renderer: {renderer_state})")]
    BenchReportMissing {
        /// Report path the renderer was asked to write.
        path: PathBuf,
        /// How long the harness waited.
        wait: Duration,
        /// Child process state when the wait ended.
        renderer_state: String,
    },
    /// Unknown frame-benchmark scenario name.
    #[error("unknown frame bench scenario `{0}`")]
    UnknownBenchScenario(String),
    /// One or more cases in the headless suite failed.
    #[error(
        "headless suite failed: {failed}/{total} cases failed; report written to {report_path}"
    )]
    SuiteFailed {
        /// Number of failed cases.
        failed: usize,
        /// Number of cases executed.
        total: usize,
        /// Path to the suite-level JSON report.
        report_path: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::HarnessError;

    #[test]
    fn display_renderer_binary_missing() {
        let p = PathBuf::from("/no/renderide");
        let e = HarnessError::RendererBinaryMissing(p.clone());
        let s = e.to_string();
        assert!(s.contains("renderer binary not found"));
        assert!(s.contains(p.to_string_lossy().as_ref()));
    }

    #[test]
    fn display_golden_missing() {
        let p = PathBuf::from("missing.png");
        let e = HarnessError::GoldenMissing(p);
        assert!(e.to_string().contains("golden image not found"));
        assert!(e.to_string().contains("run --bless"));
        assert!(e.to_string().contains("--force"));
    }

    #[test]
    fn display_golden_mismatch_includes_scores_and_paths() {
        let e = HarnessError::GoldenMismatch {
            score: 0.5,
            threshold: 0.95,
            diff_path: PathBuf::from("target/diff.png"),
        };
        let s = e.to_string();
        assert!(s.contains("SSIM=0.5000"));
        assert!(s.contains("0.9500"));
        assert!(s.contains("diff.png"));
    }

    #[test]
    fn display_flat_or_unstable_includes_pending_chunk_stats_and_frames() {
        let e = HarnessError::FlatOrUnstableFrame {
            path: PathBuf::from("target/headless.png"),
            wait: Duration::from_secs(180),
            last_pending: 0,
            ready_chunks: 4,
            pending_chunks: 1,
            missing_chunks: 0,
            frame_count: 5,
            frame_pending: Some(0),
            gpu_done: true,
            dropped_draws: Some(0),
        };
        let s = e.to_string();
        assert!(s.contains("flat or unstable"));
        assert!(s.contains("last pending=0"));
        assert!(s.contains("ready=4"));
        assert!(s.contains("pending=1"));
        assert!(s.contains("missing=0"));
        assert!(s.contains("frames=5"));
        assert!(s.contains("headless.png"));
        assert!(!s.contains("pipeline-pending"));
    }

    #[test]
    fn display_pipeline_not_ready_includes_path_and_wait() {
        let e = HarnessError::PipelineNotReady {
            path: PathBuf::from("target/headless.png"),
            wait: Duration::from_secs(9),
        };
        let s = e.to_string();
        assert!(s.contains("headless.png"));
        assert!(s.contains("pipeline-pending"));
        assert!(s.contains('9'));
    }

    #[test]
    fn display_png_output_missing_includes_path_and_wait() {
        let e = HarnessError::PngOutputMissing {
            path: PathBuf::from("target/headless.png"),
            wait: Duration::from_secs(7),
        };
        let s = e.to_string();
        assert!(s.contains("headless.png"));
        assert!(s.contains("expected fresh PNG output"));
        assert!(s.contains('7'));
    }

    #[test]
    fn display_flat_image_includes_path_and_color() {
        let e = HarnessError::FlatImage {
            path: PathBuf::from("flat.png"),
            color: [10, 20, 30, 40],
        };
        let s = e.to_string();
        assert!(s.contains("flat.png"));
        assert!(s.contains("flat single color"));
        for component in ["10", "20", "30", "40"] {
            assert!(
                s.contains(component),
                "missing component {component} in {s}"
            );
        }
    }

    #[test]
    fn display_asset_ack_timeout_includes_duration_and_reason() {
        let e = HarnessError::AssetAckTimeout(Duration::from_millis(2500), "mesh upload");
        let s = e.to_string();
        assert!(s.contains("asset ack timed out"));
        assert!(s.contains("mesh upload"));
    }

    #[test]
    fn display_handshake_timeout_includes_label() {
        let e = HarnessError::HandshakeTimeout(Duration::from_secs(15));
        let s = e.to_string();
        assert!(s.contains("handshake timed out"));
    }

    #[test]
    fn display_png_read_includes_path() {
        let img_err = image::ImageError::IoError(std::io::Error::other("decode failed"));
        let e = HarnessError::PngRead {
            path: PathBuf::from("a.png"),
            source: img_err,
        };
        let s = e.to_string();
        assert!(s.contains("a.png"));
        assert!(s.starts_with("read png"));
    }

    #[test]
    fn display_png_write_includes_path() {
        let img_err = image::ImageError::IoError(std::io::Error::other("write failed"));
        let e = HarnessError::PngWrite {
            path: PathBuf::from("b.png"),
            source: img_err,
        };
        let s = e.to_string();
        assert!(s.contains("b.png"));
        assert!(s.starts_with("write png"));
    }

    #[test]
    fn display_image_compare_includes_inner_message() {
        let e = HarnessError::ImageCompare("rank deficient".to_string());
        let s = e.to_string();
        assert!(s.contains("rank deficient"));
        assert!(s.starts_with("image-compare"));
    }

    #[test]
    fn display_queue_options_includes_inner_message() {
        let e = HarnessError::QueueOptions("capacity overflow".to_string());
        let s = e.to_string();
        assert!(s.contains("capacity overflow"));
        assert!(s.contains("queue options invalid"));
    }

    #[test]
    fn display_spawn_renderer_includes_io_error() {
        let io_err = std::io::Error::other("permission denied");
        let e = HarnessError::SpawnRenderer(io_err);
        let s = e.to_string();
        assert!(s.contains("permission denied"));
        assert!(s.contains("spawn renderer process"));
    }

    #[test]
    fn display_gltf_fixture_includes_path_and_message() {
        let e = HarnessError::GltfFixture {
            path: PathBuf::from("fixtures/model.glb"),
            message: "missing POSITION".to_string(),
        };
        let s = e.to_string();
        assert!(s.contains("fixtures/model.glb"));
        assert!(s.contains("missing POSITION"));
    }

    #[test]
    fn display_unknown_case_includes_name() {
        let e = HarnessError::UnknownCase("missing_case".to_string());
        let s = e.to_string();
        assert!(s.contains("unknown integration case"));
        assert!(s.contains("missing_case"));
    }

    #[test]
    fn display_suite_failed_includes_counts_and_report() {
        let e = HarnessError::SuiteFailed {
            failed: 2,
            total: 5,
            report_path: PathBuf::from("target/renderide-test-out/suite-report.json"),
        };
        let s = e.to_string();
        assert!(s.contains("2/5"));
        assert!(s.contains("suite-report.json"));
    }
}
