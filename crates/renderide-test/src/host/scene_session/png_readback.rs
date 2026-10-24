//! PNG stability state machine and the readback driver loop.
//!
//! Decision logic is encapsulated in [`PngStabilityState::observe`] so the per-tick I/O loop in
//! `run_lockstep_until_png_stable` is small and the state transitions are unit-testable in
//! isolation from the renderer / filesystem.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant, SystemTime};

use renderide_shared::ipc::HostDualQueueIpc;

use crate::error::HarnessError;

use super::super::lockstep::LockstepDriver;
use super::config::SceneSessionOutcome;
use super::consts::timing;

/// Wall-clock + monotonic anchors for one readback wait.
pub struct PngStabilityWaitTiming {
    /// `SystemTime` when the scene was submitted; the PNG `mtime` must exceed this.
    pub scene_submitted_at: SystemTime,
    /// Monotonic instant at scene submit; used for the "wait at least N intervals" gate.
    pub scene_submit_instant: Instant,
    /// Wall-clock budget for the entire wait-until-stable-PNG loop.
    pub overall_timeout: Duration,
    /// Renderer's configured PNG write interval.
    pub interval: Duration,
}

/// Filesystem snapshot of the renderer's PNG output, supplied to [`PngStabilityState::observe`].
#[derive(Clone, Copy, Debug)]
pub enum PngObservation {
    /// File does not exist or its metadata could not be read.
    Missing,
    /// File exists with the given modification time and byte length.
    Present {
        /// File modification time.
        mtime: SystemTime,
        /// File size in bytes.
        size: u64,
    },
}

/// Decision returned by [`PngStabilityState::observe`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PngStabilityVerdict {
    /// Keep polling; not yet a fresh, stable PNG.
    Pending,
    /// PNG is stable past scene-submit and safe to return.
    Stable,
}

/// Pure state machine for "has the renderer produced a fresh, stable PNG?"
///
/// Each call to [`Self::observe`] is deterministic given `(now, observation)`; the outer loop
/// owns all I/O. This separation keeps the decision logic unit-testable.
pub struct PngStabilityState {
    /// `SystemTime` baseline; observed `mtime` must strictly exceed this to count as scene-fresh.
    scene_submitted_at: SystemTime,
    /// Monotonic baseline; the render-window opens once `now - scene_submit_instant` exceeds
    /// [`Self::min_wall_after_submit`].
    scene_submit_instant: Instant,
    /// Floor on the post-submit wait before any PNG mtime is accepted.
    min_wall_after_submit: Duration,
    /// PNG mtime must remain unchanged for this duration before transitioning to `Stable`.
    stability_window: Duration,
    /// Mtime most recently considered as a stability candidate.
    last_seen_mtime: Option<SystemTime>,
    /// Monotonic instant at which `last_seen_mtime` was first observed.
    stable_since: Option<Instant>,
}

impl PngStabilityState {
    /// Constructs the state from a [`PngStabilityWaitTiming`] anchor pair.
    ///
    /// `min_wall_after_submit` is set to `interval * 2`, floored at
    /// `timing::MIN_WALL_AFTER_SUBMIT_FLOOR`, so callers configured with very short
    /// renderer intervals still wait long enough for slow software rendering to apply-then-render.
    pub fn new(timing: &PngStabilityWaitTiming) -> Self {
        let min_wall_after_submit = (timing.interval * 2).max(timing::MIN_WALL_AFTER_SUBMIT_FLOOR);
        Self {
            scene_submitted_at: timing.scene_submitted_at,
            scene_submit_instant: timing.scene_submit_instant,
            min_wall_after_submit,
            stability_window: timing::STABILITY_WINDOW,
            last_seen_mtime: None,
            stable_since: None,
        }
    }

    /// Floor on the post-submit wait before any PNG is accepted; exposed for deadline computation.
    pub const fn min_wall_after_submit(&self) -> Duration {
        self.min_wall_after_submit
    }

    /// Whether enough wall-clock has elapsed past scene submit for ANY PNG to be considered
    /// scene-fresh. Exposed for log-line context.
    pub fn render_window_open(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.scene_submit_instant) >= self.min_wall_after_submit
    }

    /// Mtime most recently considered as a stability candidate (`None` if none seen yet).
    /// Exposed for log-line context.
    pub const fn last_seen_mtime(&self) -> Option<SystemTime> {
        self.last_seen_mtime
    }

    /// Updates internal state from a fresh observation and returns the new verdict.
    pub fn observe(&mut self, now: Instant, observation: PngObservation) -> PngStabilityVerdict {
        let (mtime, size) = match observation {
            PngObservation::Missing => return PngStabilityVerdict::Pending,
            PngObservation::Present { mtime, size } => (mtime, size),
        };
        if size == 0 {
            return PngStabilityVerdict::Pending;
        }
        if !self.render_window_open(now) {
            return PngStabilityVerdict::Pending;
        }
        if mtime <= self.scene_submitted_at {
            return PngStabilityVerdict::Pending;
        }

        if self.last_seen_mtime == Some(mtime) {
            if let Some(since) = self.stable_since
                && now.saturating_duration_since(since) >= self.stability_window
            {
                return PngStabilityVerdict::Stable;
            }
            return PngStabilityVerdict::Pending;
        }

        self.last_seen_mtime = Some(mtime);
        self.stable_since = Some(now);
        PngStabilityVerdict::Pending
    }

    /// Drops the current stability window so the same PNG can be judged again.
    ///
    /// Clearing only `stable_since` left `last_seen_mtime` set, and the next observation of
    /// that same mtime never reopened the window. A frame that became ready without a newer
    /// write, or a write whose mtime did not change, then waited out the whole capture.
    pub fn reject_candidate(&mut self) {
        self.stable_since = None;
        self.last_seen_mtime = None;
    }
}

/// Keyword of the PNG `tEXt` chunk written by the headless PNG encoder.
const PIPELINE_PENDING_TEXT_KEYWORD: &str = "renderide_pipeline_pending";

/// Whether this PNG's own `tEXt` chunk says the frame had no compiling or skipped draws.
///
/// The count is inside the PNG, so a later frame cannot publish `0` next to an older image.
/// A missing chunk, a non-numeric chunk, or a non-zero count is not ready. A Null fallback
/// drawn while the requested shader is still compiling is a positive count. A draw skipped
/// because no pipeline was available is a positive count too.
fn pipeline_frame_is_ready(path: &Path) -> bool {
    pipeline_pending_text(path).is_some_and(|text| text.trim() == "0")
}

fn pipeline_pending_text(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let reader = decoder.read_info().ok()?;
    reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .find(|chunk| chunk.keyword == PIPELINE_PENDING_TEXT_KEYWORD)
        .map(|chunk| chunk.text.clone())
}

/// Whether the PNG at `path` decodes to a flat single-color image.
fn png_is_flat(path: &Path) -> bool {
    crate::image_io::load_rgba(path).is_ok_and(|img| crate::golden::is_flat_image(&img))
}

/// Reads the current observation of `output_path`. Maps any I/O failure to
/// [`PngObservation::Missing`].
fn read_png_observation(output_path: &Path) -> PngObservation {
    match std::fs::metadata(output_path) {
        Ok(meta) => match meta.modified() {
            Ok(mtime) => PngObservation::Present {
                mtime,
                size: meta.len(),
            },
            Err(_) => PngObservation::Missing,
        },
        Err(_) => PngObservation::Missing,
    }
}

/// Drains the lockstep, pumps the [`PngStabilityState`] from filesystem observations, and returns
/// once the PNG is stable. Bails if the renderer exits or the deadline expires.
pub(super) fn run_lockstep_until_png_stable(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    output_path: &Path,
    wait_timing: PngStabilityWaitTiming,
    renderer: &mut Child,
) -> Result<SceneSessionOutcome, HarnessError> {
    let mut state = PngStabilityState::new(&wait_timing);
    let deadline_floor = state.min_wall_after_submit() + timing::PNG_DEADLINE_SLACK;
    let deadline = Instant::now() + wait_timing.overall_timeout.max(deadline_floor);
    let mut last_log_at = Instant::now();
    let mut flat_candidate_seen = false;
    let mut pending_pipeline_seen = false;
    let mut chunks = ObservedChunks::default();

    while Instant::now() < deadline {
        let _ = lockstep.tick(queues);

        if let Ok(Some(status)) = renderer.try_wait() {
            return Err(HarnessError::AssetAckTimeout(
                deadline.elapsed(),
                if status.success() {
                    "renderer exited cleanly before producing PNG"
                } else {
                    "renderer exited with failure before producing PNG"
                },
            ));
        }

        let now = Instant::now();
        if matches!(
            state.observe(now, read_png_observation(output_path)),
            PngStabilityVerdict::Stable
        ) {
            // Pipelines compile asynchronously. A draw whose pipeline is still pending,
            // or that was skipped because no pipeline was available, is stamped on the
            // PNG (materials/registry.rs). Keep pumping for a later write.
            if png_is_flat(output_path) {
                if !flat_candidate_seen {
                    logger::info!(
                        "Session: stable PNG is flat; waiting for a later frame (pipelines may still be compiling)"
                    );
                }
                flat_candidate_seen = true;
                let pending_text = pipeline_pending_text(output_path);
                pending_pipeline_seen |= !pending_text
                    .as_deref()
                    .is_some_and(|text| text.trim() == "0");
                chunks.note(pending_text.as_deref());
                state.reject_candidate();
                std::thread::sleep(timing::POLL_INTERVAL);
                continue;
            }
            if !pipeline_frame_is_ready(output_path) {
                if !pending_pipeline_seen {
                    logger::info!(
                        "Session: stable PNG still has compiling or skipped material draws; waiting for a ready frame"
                    );
                }
                pending_pipeline_seen = true;
                chunks.note(pipeline_pending_text(output_path).as_deref());
                state.reject_candidate();
                std::thread::sleep(timing::POLL_INTERVAL);
                continue;
            }
            chunks.note(pipeline_pending_text(output_path).as_deref());
            logger::info!(
                "Session: fresh PNG stabilized at {} after {:?}",
                output_path.display(),
                wait_timing.scene_submit_instant.elapsed()
            );
            return Ok(SceneSessionOutcome {
                png_path: output_path.to_path_buf(),
            });
        }

        if last_log_at.elapsed() >= timing::LOG_INTERVAL {
            let now = Instant::now();
            logger::info!(
                "Session: still waiting for fresh PNG (elapsed={:?}, scene_window_open={}, last_mtime={:?})",
                wait_timing.scene_submit_instant.elapsed(),
                state.render_window_open(now),
                state.last_seen_mtime()
            );
            last_log_at = now;
        }
        std::thread::sleep(timing::POLL_INTERVAL);
    }

    match classify_capture_timeout(&chunks, pending_pipeline_seen, flat_candidate_seen) {
        CaptureTimeout::FlatOrUnstable => Err(HarnessError::FlatOrUnstableFrame {
            path: PathBuf::from(output_path),
            wait: wait_timing.overall_timeout,
            last_pending: chunks.last_pending.unwrap_or(0),
            ready_chunks: chunks.ready,
            pending_chunks: chunks.pending,
            missing_chunks: chunks.missing,
            frame_count: chunks.frames,
        }),
        CaptureTimeout::PipelineNotReady => Err(HarnessError::PipelineNotReady {
            path: PathBuf::from(output_path),
            wait: wait_timing.overall_timeout,
        }),
        CaptureTimeout::FlatHandedOff => {
            // Never rendered anything but a flat frame, and the pending chunk was not
            // zero: hand it to the golden check so the failure is "flat single color".
            Ok(SceneSessionOutcome {
                png_path: output_path.to_path_buf(),
            })
        }
        CaptureTimeout::Missing => Err(HarnessError::PngOutputMissing {
            path: PathBuf::from(output_path),
            wait: wait_timing.overall_timeout,
        }),
    }
}

/// Stable-frame pending chunks seen before the capture deadline.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ObservedChunks {
    frames: u64,
    ready: u64,
    pending: u64,
    missing: u64,
    last_pending: Option<u64>,
}

impl ObservedChunks {
    fn note(&mut self, text: Option<&str>) {
        self.frames = self.frames.saturating_add(1);
        match text
            .map(str::trim)
            .and_then(|text| text.parse::<u64>().ok())
        {
            Some(0) => {
                self.ready = self.ready.saturating_add(1);
                self.last_pending = Some(0);
            }
            Some(count) => {
                self.pending = self.pending.saturating_add(1);
                self.last_pending = Some(count);
            }
            None => {
                self.missing = self.missing.saturating_add(1);
                self.last_pending = None;
            }
        }
    }
}

/// How a capture that reached its deadline is reported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureTimeout {
    /// The last frame's pending count was zero. The image was flat or still changing.
    FlatOrUnstable,
    /// A frame was still compiling or had skipped draws when the deadline hit.
    PipelineNotReady,
    /// Only flat frames were seen, and none of them recorded pending=0.
    FlatHandedOff,
    /// No stable PNG arrived.
    Missing,
}

/// A timeout whose last pending count is zero is a flat or unstable frame, not
/// [`HarnessError::PipelineNotReady`].
fn classify_capture_timeout(
    chunks: &ObservedChunks,
    pending_pipeline_seen: bool,
    flat_candidate_seen: bool,
) -> CaptureTimeout {
    if chunks.frames > 0 && chunks.last_pending == Some(0) {
        return CaptureTimeout::FlatOrUnstable;
    }
    if pending_pipeline_seen {
        return CaptureTimeout::PipelineNotReady;
    }
    if flat_candidate_seen {
        return CaptureTimeout::FlatHandedOff;
    }
    CaptureTimeout::Missing
}

#[cfg(test)]
mod tests {
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
}
