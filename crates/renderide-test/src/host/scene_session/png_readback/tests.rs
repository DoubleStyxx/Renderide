//! Unit tests for the PNG stability state machine and frame readiness decode.

use std::time::{Duration, Instant, SystemTime};

use super::{
    CaptureTimeout, ObservedChunks, PngObservation, PngStabilityState, PngStabilityVerdict,
    PngStabilityWaitTiming, classify_capture_timeout,
};

fn chunks_with_last(last_pending: Option<u64>, frames: u64) -> ObservedChunks {
    let mut chunks = ObservedChunks::default();
    match last_pending {
        Some(0) => chunks.note(Some("0")),
        Some(count) => {
            let text = count.to_string();
            chunks.note(Some(&text));
        }
        None => chunks.note(None),
    }
    chunks.frames = frames;
    chunks
}

#[test]
fn reported_wait_includes_the_deadline_floor() {
    let floor = Duration::from_millis(3500);
    let short = Duration::from_millis(500);
    assert_eq!(super::reported_wait(short, false, floor), floor);
    let base = Duration::from_secs(180);
    assert_eq!(super::reported_wait(base, false, floor), base);
    assert_eq!(
        super::reported_wait(base, true, floor),
        Duration::from_secs(300)
    );
}

#[test]
fn non_warp_adapter_timeout_still_reports_flat_or_unstable() {
    let budget = super::super::gpu_ready::warp_readiness_budget(false, Duration::from_secs(180));
    assert_eq!(budget, Duration::from_secs(180));
    let ready = chunks_with_last(Some(0), 5);
    assert_eq!(
        classify_capture_timeout(&ready, false, true),
        CaptureTimeout::FlatOrUnstable
    );
    let err = crate::error::HarnessError::FlatOrUnstableFrame {
        path: std::path::PathBuf::from("actual.png"),
        wait: budget,
        last_pending: 0,
        ready_chunks: ready.ready,
        pending_chunks: ready.pending,
        missing_chunks: ready.missing,
        frame_count: ready.frames,
        frame_pending: Some(0),
        gpu_done: true,
        dropped_draws: Some(0),
    };
    let text = err.to_string();
    assert!(text.contains("flat or unstable"), "{text}");
    assert!(text.contains("last pending=0"), "{text}");
    assert!(text.contains("frames="), "{text}");
    assert!(!text.contains("pipeline-pending"), "{text}");
}

#[test]
fn timeout_with_pending_zero_is_flat_or_unstable_not_pipeline_not_ready() {
    let ready = chunks_with_last(Some(0), 5);
    assert_eq!(
        classify_capture_timeout(&ready, true, true),
        CaptureTimeout::FlatOrUnstable
    );
    assert_eq!(
        classify_capture_timeout(&ready, false, true),
        CaptureTimeout::FlatOrUnstable
    );

    let still_pending = chunks_with_last(Some(3), 2);
    assert_eq!(
        classify_capture_timeout(&still_pending, true, false),
        CaptureTimeout::PipelineNotReady
    );

    let unseen = ObservedChunks::default();
    assert_eq!(
        classify_capture_timeout(&unseen, false, false),
        CaptureTimeout::Missing
    );
}

/// Builds a timing where `interval=1s` -> `min_wall_after_submit=2s`
/// (since `interval*2 = 2s > MIN_WALL_AFTER_SUBMIT_FLOOR = 1.5s`).
fn make_timing(
    scene_submitted_at: SystemTime,
    scene_submit_instant: Instant,
) -> PngStabilityWaitTiming {
    PngStabilityWaitTiming {
        scene_submitted_at,
        scene_submit_instant,
        overall_timeout: Duration::from_secs(10),
        interval: Duration::from_secs(1),
    }
}

fn fresh_state() -> (PngStabilityState, Instant, SystemTime) {
    let i0 = Instant::now();
    let t0 = SystemTime::now();
    let state = PngStabilityState::new(&make_timing(t0, i0));
    (state, i0, t0)
}

#[test]
fn pending_before_render_window_opens() {
    let (mut s, i0, t0) = fresh_state();
    let now = i0 + Duration::from_millis(500);
    let mtime = t0 + Duration::from_millis(100);
    let v = s.observe(now, PngObservation::Present { mtime, size: 1024 });
    assert_eq!(v, PngStabilityVerdict::Pending);
}

#[test]
fn pending_when_mtime_not_advanced_past_scene() {
    let (mut s, i0, t0) = fresh_state();
    let now = i0 + Duration::from_millis(2500);
    let mtime = t0 - Duration::from_millis(100);
    let v = s.observe(now, PngObservation::Present { mtime, size: 1024 });
    assert_eq!(v, PngStabilityVerdict::Pending);
}

#[test]
fn pending_when_size_zero() {
    let (mut s, i0, t0) = fresh_state();
    let now = i0 + Duration::from_millis(2500);
    let mtime = t0 + Duration::from_millis(100);
    let v = s.observe(now, PngObservation::Present { mtime, size: 0 });
    assert_eq!(v, PngStabilityVerdict::Pending);
}

#[test]
fn pending_within_stability_window() {
    let (mut s, i0, t0) = fresh_state();
    let mtime = t0 + Duration::from_millis(2200);
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2300),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Pending
    );
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2400),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Pending
    );
}

#[test]
fn stable_after_window_elapses() {
    let (mut s, i0, t0) = fresh_state();
    let mtime = t0 + Duration::from_millis(2200);
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2300),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Pending
    );
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2500),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Stable
    );
}

#[test]
fn stable_since_resets_when_mtime_changes() {
    let (mut s, i0, t0) = fresh_state();
    let mtime1 = t0 + Duration::from_millis(2200);
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2300),
            PngObservation::Present {
                mtime: mtime1,
                size: 1024,
            }
        ),
        PngStabilityVerdict::Pending
    );

    let mtime2 = t0 + Duration::from_millis(2400);
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2450),
            PngObservation::Present {
                mtime: mtime2,
                size: 1024,
            }
        ),
        PngStabilityVerdict::Pending
    );
    // Past the original `stable_since` by >200ms, but mtime changed so the window restarted.
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2600),
            PngObservation::Present {
                mtime: mtime2,
                size: 1024,
            }
        ),
        PngStabilityVerdict::Pending
    );
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2650),
            PngObservation::Present {
                mtime: mtime2,
                size: 1024,
            }
        ),
        PngStabilityVerdict::Stable
    );
}

#[test]
fn reject_candidate_reopens_the_window_for_the_same_mtime() {
    let (mut s, i0, t0) = fresh_state();
    let mtime = t0 + Duration::from_millis(2200);
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2300),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Pending
    );
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2600),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Stable
    );
    s.reject_candidate();
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2610),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Pending
    );
    assert_eq!(
        s.observe(
            i0 + Duration::from_millis(2900),
            PngObservation::Present { mtime, size: 1024 }
        ),
        PngStabilityVerdict::Stable
    );
}

#[test]
fn missing_file_keeps_pending() {
    let (mut s, i0, _t0) = fresh_state();
    let now = i0 + Duration::from_millis(2500);
    assert_eq!(
        s.observe(now, PngObservation::Missing),
        PngStabilityVerdict::Pending
    );
    assert!(s.last_seen_mtime().is_none());
}

#[test]
fn repeated_flat_rejections_log_once_and_alternation_logs_again() {
    let flat = super::FrameReadiness {
        gpu_done: true,
        pending: Some(0),
        dropped_draws: Some(0),
        flat: true,
        stall_ms: 153,
    };
    let good = super::FrameReadiness {
        flat: false,
        stall_ms: 10,
        ..flat
    };
    let mut progress = super::CaptureProgress::default();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let first = settle_log(
        &mut progress,
        t0,
        &flat,
        super::SettleDecision::Reject,
        false,
    )
    .expect("first flat reject");
    assert!(first.contains("reset=false"), "{first}");
    assert!(first.contains("flat=true"), "{first}");

    assert!(
        settle_log(
            &mut progress,
            t0 + Duration::from_secs(1),
            &flat,
            super::SettleDecision::Reject,
            false
        )
        .is_none(),
        "an identical flat reject does not log again"
    );
    assert!(
        settle_log(
            &mut progress,
            t0 + Duration::from_secs(2),
            &good,
            super::SettleDecision::Hold,
            false
        )
        .is_none(),
        "a held good frame does not log a rejection"
    );

    let again = settle_log(
        &mut progress,
        t0 + Duration::from_secs(3),
        &flat,
        super::SettleDecision::Reject,
        true,
    )
    .expect("return to flat");
    assert!(again.contains("reset=true"), "{again}");
}

fn settle_log(
    progress: &mut super::CaptureProgress,
    mtime: SystemTime,
    frame: &super::FrameReadiness,
    decision: super::SettleDecision,
    had_counted_frame: bool,
) -> Option<String> {
    super::log_settle_rejection(mtime, frame, decision, had_counted_frame, progress)
}

#[test]
fn flat_pending_frame_does_not_suppress_the_compiling_line() {
    let path = std::path::Path::new("target/renderide-test-out/pbs_lit_material_matrix/actual.png");
    let mut progress = super::CaptureProgress::default();
    let flat_pending = super::FrameReadiness {
        gpu_done: false,
        pending: Some(4),
        dropped_draws: None,
        flat: true,
        stall_ms: 12,
    };
    assert!(super::note_stable_frame(path, Some("4"), &flat_pending, &mut progress).is_none());
    assert!(progress.pending_pipeline_seen);
    assert!(!progress.compiling_or_skipped_logged);

    let compiling = super::FrameReadiness {
        flat: false,
        pending: Some(2),
        ..flat_pending
    };
    let line = super::note_stable_frame(path, Some("2"), &compiling, &mut progress)
        .expect("compiling line");
    let shown = path.display();
    assert_eq!(line, super::compiling_or_skipped_line(path));
    assert!(
        line.contains(&format!(
            "{shown} still has compiling or skipped material draws"
        )),
        "{line}"
    );
    assert!(
        line.contains("pbs_lit_material_matrix/actual.png"),
        "{line}"
    );
    assert!(progress.compiling_or_skipped_logged);
    assert!(super::note_stable_frame(path, Some("2"), &compiling, &mut progress).is_none());
}

#[test]
fn timeout_copies_the_observed_frame_and_fails_closed_without_one() {
    let path = std::path::Path::new("target/renderide-test-out/pbs_lit_material_matrix/actual.png");
    let chunks = chunks_with_last(Some(0), 3);
    let observed = super::ObservedSettleState {
        flat: true,
        pending: Some(4),
        gpu_done: true,
        dropped_draws: None,
    };
    let err = super::timeout_outcome(
        path,
        Duration::from_secs(300),
        &chunks,
        true,
        true,
        Some(observed),
    )
    .expect_err("timeout");
    match err {
        crate::HarnessError::FlatOrUnstableFrame {
            frame_pending,
            gpu_done,
            dropped_draws,
            last_pending,
            ..
        } => {
            assert_eq!(frame_pending, Some(4));
            assert!(gpu_done);
            assert_eq!(dropped_draws, None);
            assert_eq!(last_pending, 0);
        }
        other => panic!("expected flat or unstable, got {other}"),
    }

    let unseen = super::timeout_outcome(path, Duration::from_secs(300), &chunks, false, true, None)
        .expect_err("unobserved");
    let text = unseen.to_string();
    assert!(text.contains("flat or unstable"), "{text}");
    assert!(text.contains("last pending=0"), "{text}");
    assert!(!text.contains("pipeline-pending"), "{text}");
    match unseen {
        crate::HarnessError::FlatOrUnstableFrame {
            frame_pending,
            gpu_done,
            dropped_draws,
            ..
        } => {
            assert_eq!(frame_pending, None);
            assert!(!gpu_done);
            assert_eq!(dropped_draws, None);
        }
        other => panic!("expected flat or unstable, got {other}"),
    }
}

#[test]
fn read_frame_readiness_matches_a_renderer_png() {
    let dir = tempfile::tempdir().expect("tempdir");
    let png = dir.path().join("actual.png");
    write_renderer_png(
        &png,
        RendererStamp {
            pending: "0",
            gpu_done: "1",
            stall_ms: "153",
            pixels: &[255, 0, 0, 255, 0, 0, 255, 255],
        },
    );
    let decoded = super::read_frame_readiness(&png);
    assert_eq!(
        decoded.readiness,
        super::FrameReadiness {
            gpu_done: true,
            pending: Some(0),
            dropped_draws: Some(0),
            flat: false,
            stall_ms: 153,
        }
    );
    assert_eq!(decoded.pending_text.as_deref(), Some("0"));

    write_renderer_png(
        &png,
        RendererStamp {
            pending: "4",
            gpu_done: "0",
            stall_ms: "12",
            pixels: &[0, 0, 0, 255, 0, 0, 0, 255],
        },
    );
    let flat = super::read_frame_readiness(&png);
    assert!(!flat.readiness.gpu_done);
    assert_eq!(flat.readiness.pending, Some(4));
    assert_eq!(flat.readiness.dropped_draws, None);
    assert!(flat.readiness.flat);
    assert_eq!(flat.readiness.stall_ms, 12);
}

struct RendererStamp<'a> {
    pending: &'a str,
    gpu_done: &'a str,
    stall_ms: &'a str,
    pixels: &'a [u8],
}

fn write_renderer_png(path: &std::path::Path, stamp: RendererStamp<'_>) {
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, 2, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .add_text_chunk(
            super::PIPELINE_PENDING_TEXT_KEYWORD.to_string(),
            stamp.pending.to_string(),
        )
        .expect("pending chunk");
    encoder
        .add_text_chunk(
            super::GPU_DONE_TEXT_KEYWORD.to_string(),
            stamp.gpu_done.to_string(),
        )
        .expect("gpu_done chunk");
    encoder
        .add_text_chunk(
            super::SUBMIT_STALL_TEXT_KEYWORD.to_string(),
            stamp.stall_ms.to_string(),
        )
        .expect("stall chunk");
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(stamp.pixels).expect("pixels");
    writer.finish().expect("finish");
}

#[test]
fn pipeline_text_chunk_zero_is_ready_and_anything_else_waits() {
    let dir = tempfile::tempdir().expect("tempdir");
    let png = dir.path().join("actual.png");
    assert!(!super::pipeline_frame_is_ready(&png));
    write_pending_png(&png, "2");
    assert!(!super::pipeline_frame_is_ready(&png));
    write_pending_png(&png, "0");
    assert!(super::pipeline_frame_is_ready(&png));
    // A sidecar must not override the count stored in the PNG.
    std::fs::write(png.with_extension("pipeline-pending"), "0\n").expect("sidecar");
    write_pending_png(&png, "4");
    assert!(!super::pipeline_frame_is_ready(&png));
    write_pending_png(&png, "nope");
    assert!(!super::pipeline_frame_is_ready(&png));
}

fn write_pending_png(path: &std::path::Path, pending: &str) {
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .add_text_chunk(
            super::PIPELINE_PENDING_TEXT_KEYWORD.to_string(),
            pending.to_string(),
        )
        .expect("text chunk");
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[0, 0, 0, 255]).expect("pixels");
    writer.finish().expect("finish");
}
