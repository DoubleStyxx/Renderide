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
use super::gpu_ready::{self, FrameReadiness, GpuFrameSettle, SettleDecision};

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
/// `1` only after that frame's submission reached `on_submitted_work_done`.
const GPU_DONE_TEXT_KEYWORD: &str = "renderide_gpu_done";
/// Milliseconds spent waiting for that submission inside the headless readback.
const SUBMIT_STALL_TEXT_KEYWORD: &str = "renderide_submit_stall_ms";

/// Whether this PNG's own `tEXt` chunk says the frame had no compiling or skipped draws.
///
/// The count is inside the PNG, so a later frame cannot publish `0` next to an older image.
/// A missing chunk, a non-numeric chunk, or a non-zero count is not ready. A Null fallback
/// drawn while the requested shader is still compiling is a positive count. A draw skipped
/// because no pipeline was available is a positive count too.
#[cfg(test)]
fn pipeline_frame_is_ready(path: &Path) -> bool {
    decode_stable_png(path)
        .text(PIPELINE_PENDING_TEXT_KEYWORD)
        .is_some_and(|text| text.trim() == "0")
}

/// One open of a stable PNG: every `tEXt` chunk and the RGBA pixels, when they decode.
struct DecodedPng {
    texts: Vec<(String, String)>,
    image: Option<image::RgbaImage>,
}

impl DecodedPng {
    fn missing() -> Self {
        Self {
            texts: Vec::new(),
            image: None,
        }
    }

    fn text(&self, keyword: &str) -> Option<&str> {
        self.texts
            .iter()
            .find(|(key, _)| key == keyword)
            .map(|(_, text)| text.as_str())
    }
}

/// Reads the file once. A failed decode has no chunks and is not flat.
fn decode_stable_png(path: &Path) -> DecodedPng {
    let Ok(bytes) = std::fs::read(path) else {
        return DecodedPng::missing();
    };
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let Ok(mut reader) = decoder.read_info() else {
        return DecodedPng::missing();
    };
    let texts = reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .map(|chunk| (chunk.keyword.clone(), chunk.text.clone()))
        .collect();
    let Some(size) = reader.output_buffer_size() else {
        return DecodedPng { texts, image: None };
    };
    let mut buf = vec![0u8; size];
    let Ok(output) = reader.next_frame(&mut buf) else {
        return DecodedPng { texts, image: None };
    };
    DecodedPng {
        texts,
        image: rgba8_image(&output, &buf),
    }
}

/// Headless captures are 8-bit RGBA with packed rows. Anything else is not a flat image.
fn rgba8_image(output: &png::OutputInfo, buf: &[u8]) -> Option<image::RgbaImage> {
    if output.color_type != png::ColorType::Rgba || output.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let bytes = output.buffer_size();
    if buf.len() < bytes || output.line_size != output.width as usize * 4 {
        return None;
    }
    image::RgbaImage::from_raw(output.width, output.height, buf[..bytes].to_vec())
}

/// Readiness plus the pending chunk text, from a single decode.
struct DecodedFrame {
    readiness: FrameReadiness,
    pending_text: Option<String>,
    image: Option<image::RgbaImage>,
}

fn read_frame_readiness(path: &Path) -> DecodedFrame {
    let decoded = decode_stable_png(path);
    let pending_text = decoded
        .text(PIPELINE_PENDING_TEXT_KEYWORD)
        .map(str::to_string);
    let (pending, dropped_draws) = gpu_ready::counts_from_pending_chunk(pending_text.as_deref());
    let gpu_done = decoded
        .text(GPU_DONE_TEXT_KEYWORD)
        .is_some_and(|text| text.trim() == "1");
    let stall_ms = decoded
        .text(SUBMIT_STALL_TEXT_KEYWORD)
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let flat = decoded
        .image
        .as_ref()
        .is_some_and(crate::golden::is_flat_image);
    DecodedFrame {
        readiness: FrameReadiness {
            gpu_done,
            pending,
            dropped_draws,
            flat,
            stall_ms,
        },
        pending_text,
        image: decoded.image,
    }
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
    let wait_started = Instant::now();
    let mut last_log_at = Instant::now();
    let mut warp = WarpBudgetWatch {
        last_check: wait_started
            .checked_sub(timing::LOG_INTERVAL)
            .unwrap_or(wait_started),
        decided: false,
        used: false,
        flat_backstop: false,
        deadline: wait_started + wait_timing.overall_timeout.max(deadline_floor),
    };
    let mut progress = CaptureProgress::default();
    let mut settle = GpuFrameSettle::default();
    let case_name = case_name_from_output(output_path);
    let mut stats = CaptureStatsWriter::new(
        output_path.with_file_name(crate::scene_dsl::output::CAPTURE_STATS_FILENAME),
    );

    while Instant::now() < warp.deadline {
        extend_warp_budget(
            output_path,
            &wait_timing,
            wait_started,
            deadline_floor,
            &mut warp,
        );
        arm_flat_backstop(&case_name, wait_started, deadline_floor, &mut warp);
        progress.flat_armed = warp.flat_backstop;
        stats.sync(settle.longest_stall_ms(), warp.used);
        let _ = lockstep.tick(queues);

        if let Ok(Some(status)) = renderer.try_wait() {
            return Err(HarnessError::AssetAckTimeout(
                wait_started.elapsed(),
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
            match settle_stable_frame(
                output_path,
                &wait_timing,
                &mut state,
                &mut progress,
                &mut settle,
                &mut stats,
                warp.used,
            ) {
                StableStep::Accept => {
                    return Ok(SceneSessionOutcome {
                        png_path: output_path.to_path_buf(),
                    });
                }
                StableStep::EarlyExit => {
                    return finish_flat_early_exit(
                        output_path,
                        &case_name,
                        wait_started,
                        &settle,
                        &mut stats,
                        warp.used,
                        &progress,
                    );
                }
                StableStep::Continue => {}
            }
            std::thread::sleep(timing::POLL_INTERVAL);
            continue;
        }

        log_still_waiting(&mut last_log_at, &wait_timing, &state);
        std::thread::sleep(timing::POLL_INTERVAL);
    }

    let wait = if warp.flat_backstop {
        flat_exit::flat_exit_budget(true, Duration::ZERO, deadline_floor)
    } else {
        reported_wait(wait_timing.overall_timeout, warp.used, deadline_floor)
    };
    stats.sync(settle.longest_stall_ms(), warp.used);
    timeout_outcome(
        output_path,
        wait,
        &progress.chunks,
        progress.pending_pipeline_seen,
        progress.flat_candidate_seen,
        progress.last_observed,
    )
}

/// Flags and chunk counts gathered while a capture is still waiting.
#[derive(Default)]
struct CaptureProgress {
    flat_candidate_seen: bool,
    pending_pipeline_seen: bool,
    /// The compiling-or-skipped line has its own flag. A flat frame with
    /// `pending > 0` still sets `pending_pipeline_seen` for timeout classification,
    /// and that must not swallow the line the Linux stall grep looks for.
    compiling_or_skipped_logged: bool,
    gpu_not_done_logged: bool,
    settle_hold_logged: bool,
    /// The #216 60s backstop is active for this wait.
    flat_armed: bool,
    flat_watch: flat_exit::FlatExitWatch,
    last_observed: Option<ObservedSettleState>,
    chunks: ObservedChunks,
}

/// What one stable PNG does to the capture wait.
enum StableStep {
    /// This frame is not the capture. Keep polling.
    Continue,
    /// Two settled non-flat frames were accepted.
    Accept,
    /// K identical settled flat readbacks ended a #216 wait.
    EarlyExit,
}

/// Returns how this stable PNG affects the capture.
fn settle_stable_frame(
    output_path: &Path,
    wait_timing: &PngStabilityWaitTiming,
    state: &mut PngStabilityState,
    progress: &mut CaptureProgress,
    settle: &mut GpuFrameSettle,
    stats: &mut CaptureStatsWriter,
    warp_used: bool,
) -> StableStep {
    let Some(mtime) = state.last_seen_mtime() else {
        state.reject_candidate();
        return StableStep::Continue;
    };
    let decoded = read_frame_readiness(output_path);
    let _ = note_stable_frame(
        output_path,
        decoded.pending_text.as_deref(),
        &decoded.readiness,
        progress,
    );
    stats.sync(settle.longest_stall_ms(), warp_used);
    let completed_before = settle.completed_frames();
    let decision = settle.observe(mtime, decoded.readiness);
    let _ = log_settle_rejection(
        mtime,
        &decoded.readiness,
        decision,
        completed_before > 0,
        progress,
    );
    stats.sync(settle.longest_stall_ms(), warp_used);
    if progress.flat_armed
        && progress.flat_watch.observe(
            mtime,
            flat_exit::frame_is_settled(&decoded.readiness),
            decoded.image.as_ref(),
        ) == flat_exit::FlatExit::Exit
    {
        return StableStep::EarlyExit;
    }
    if decision == SettleDecision::Accept {
        logger::info!(
            "Session: fresh PNG stabilized at {} after {:?}",
            output_path.display(),
            wait_timing.scene_submit_instant.elapsed()
        );
        return StableStep::Accept;
    }
    if decision == SettleDecision::Hold && !progress.settle_hold_logged {
        logger::info!("Session: GPU-completed frame recorded; waiting for a later settled frame");
        progress.settle_hold_logged = true;
    }
    state.reject_candidate();
    StableStep::Continue
}

fn timeout_outcome(
    output_path: &Path,
    wait: Duration,
    chunks: &ObservedChunks,
    pending_pipeline_seen: bool,
    flat_candidate_seen: bool,
    last_observed: Option<ObservedSettleState>,
) -> Result<SceneSessionOutcome, HarnessError> {
    match classify_capture_timeout(chunks, pending_pipeline_seen, flat_candidate_seen) {
        CaptureTimeout::FlatOrUnstable => Err(flat_or_unstable_error(
            output_path,
            wait,
            chunks,
            last_observed,
        )),
        CaptureTimeout::PipelineNotReady => Err(HarnessError::PipelineNotReady {
            path: PathBuf::from(output_path),
            wait,
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
            wait,
        }),
    }
}

fn flat_or_unstable_error(
    output_path: &Path,
    wait: Duration,
    chunks: &ObservedChunks,
    last_observed: Option<ObservedSettleState>,
) -> HarnessError {
    let (frame_pending, gpu_done, dropped_draws) = match last_observed {
        Some(observed) => (observed.pending, observed.gpu_done, observed.dropped_draws),
        None => (None, false, None),
    };
    HarnessError::FlatOrUnstableFrame {
        path: PathBuf::from(output_path),
        wait,
        last_pending: chunks.last_pending.unwrap_or(0),
        ready_chunks: chunks.ready,
        pending_chunks: chunks.pending,
        missing_chunks: chunks.missing,
        frame_count: chunks.frames,
        frame_pending,
        gpu_done,
        dropped_draws,
    }
}

/// Deadline state for one capture wait. WARP extends it once, from `wait_started`.
struct WarpBudgetWatch {
    last_check: Instant,
    decided: bool,
    used: bool,
    /// This wait is a quarantined WARP case still inside the #216 expiry.
    flat_backstop: bool,
    deadline: Instant,
}

/// Grows the deadline once, from the start of the wait, when the selected adapter is WARP.
fn extend_warp_budget(
    output_path: &Path,
    wait_timing: &PngStabilityWaitTiming,
    wait_started: Instant,
    deadline_floor: Duration,
    watch: &mut WarpBudgetWatch,
) {
    if watch.decided || watch.last_check.elapsed() < timing::LOG_INTERVAL {
        return;
    }
    watch.last_check = Instant::now();
    let Some(is_warp) = crate::scene_dsl::output_warp_decision(output_path) else {
        return;
    };
    watch.decided = true;
    if !is_warp {
        return;
    }
    let budget = gpu_ready::warp_readiness_budget(true, wait_timing.overall_timeout);
    let extended = wait_started + budget.max(deadline_floor);
    if extended > watch.deadline {
        watch.deadline = extended;
    }
    watch.used = true;
    logger::info!(
        "Session: WARP adapter; readiness budget is {:?} (#216, #214)",
        budget
    );
}

/// Readiness facts that distinguish a flat stretch from an alternating one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservedSettleState {
    flat: bool,
    pending: Option<u64>,
    gpu_done: bool,
    dropped_draws: Option<u64>,
}

impl ObservedSettleState {
    fn from_frame(frame: &FrameReadiness) -> Self {
        Self {
            flat: frame.flat,
            pending: frame.pending,
            gpu_done: frame.gpu_done,
            dropped_draws: frame.dropped_draws,
        }
    }
}

/// Logs a settle rejection when its readiness state differs from the last observation.
///
/// Repeated identical frames produce one line. `good` then `flat` then `good`
/// then `flat` produces a line on each return to `flat`, with `reset` set when
/// a counted frame was cleared. `stall_ms` is printed but does not by itself
/// open a new line. Returns that line, or `None` when this observation did not log.
fn log_settle_rejection(
    mtime: SystemTime,
    frame: &FrameReadiness,
    decision: SettleDecision,
    had_counted_frame: bool,
    progress: &mut CaptureProgress,
) -> Option<String> {
    let observed = ObservedSettleState::from_frame(frame);
    let changed = progress.last_observed != Some(observed);
    progress.last_observed = Some(observed);
    if decision != SettleDecision::Reject || !changed {
        return None;
    }
    let reset = had_counted_frame;
    let pending = format_count(frame.pending);
    let dropped = format_count(frame.dropped_draws);
    let line = format!(
        "Session: settle reject mtime={} flat={} pending={pending} gpu_done={} dropped={dropped} stall_ms={} reset={reset}",
        mtime_millis(mtime),
        frame.flat,
        frame.gpu_done,
        frame.stall_ms,
    );
    logger::info!("{line}");
    Some(line)
}

fn mtime_millis(mtime: SystemTime) -> u128 {
    mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

fn format_count(count: Option<u64>) -> String {
    match count {
        Some(count) => count.to_string(),
        None => "none".to_string(),
    }
}

fn reported_wait(overall: Duration, warp_budget_used: bool, deadline_floor: Duration) -> Duration {
    let budget = if warp_budget_used {
        gpu_ready::warp_readiness_budget(true, overall)
    } else {
        overall
    };
    budget.max(deadline_floor)
}

/// Records the stable frame and logs the same rejection lines the Linux suite greps for.
///
/// `pending_text` is the chunk already decoded with the frame. This does not reopen the PNG.
/// Returns the compiling-or-skipped line when this call is the one that logs it.
fn note_stable_frame(
    output_path: &Path,
    pending_text: Option<&str>,
    frame: &FrameReadiness,
    progress: &mut CaptureProgress,
) -> Option<String> {
    let ready = pending_text.is_some_and(|text| text.trim() == "0");
    let mut logged = None;
    if frame.flat {
        if !progress.flat_candidate_seen {
            logger::info!(
                "Session: stable PNG is flat; waiting for a later frame (pipelines may still be compiling)"
            );
        }
        progress.flat_candidate_seen = true;
        progress.pending_pipeline_seen |= !ready;
    } else if !ready {
        // Pipelines compile asynchronously. A draw whose pipeline is still pending,
        // or that was skipped because no pipeline was available, is stamped on the
        // PNG (materials/registry.rs). Keep pumping for a later write.
        // `compiling_or_skipped_logged` is not `pending_pipeline_seen`: a flat frame
        // with pending>0 sets that flag and must not suppress this line.
        if !progress.compiling_or_skipped_logged {
            let line = compiling_or_skipped_line(output_path);
            logger::info!("{line}");
            progress.compiling_or_skipped_logged = true;
            logged = Some(line);
        }
        progress.pending_pipeline_seen = true;
    } else if !frame.gpu_done && !progress.gpu_not_done_logged {
        logger::info!("Session: stable PNG gpu submission is not done; waiting for a later frame");
        progress.gpu_not_done_logged = true;
    }
    progress.chunks.note(pending_text);
    logged
}

/// Linux CI greps this line, including the stalled case's PNG path.
fn compiling_or_skipped_line(output_path: &Path) -> String {
    let shown = output_path.display();
    format!(
        "Session: stable PNG {shown} still has compiling or skipped material draws; waiting for a ready frame"
    )
}

/// Writes the stall and WARP-budget flag beside the PNG on every exit, including timeout.
struct CaptureStatsWriter {
    path: PathBuf,
    longest_submission_stall_ms: u64,
    warp_readiness_budget_used: bool,
    early_exit: Option<crate::scene_dsl::output::EarlyExit>,
}

impl CaptureStatsWriter {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            longest_submission_stall_ms: 0,
            warp_readiness_budget_used: false,
            early_exit: None,
        }
    }

    fn set_early_exit(&mut self, early_exit: crate::scene_dsl::output::EarlyExit) {
        self.early_exit = Some(early_exit);
    }

    fn sync(&mut self, stall_ms: u64, warp_budget_used: bool) {
        self.longest_submission_stall_ms = self.longest_submission_stall_ms.max(stall_ms);
        self.warp_readiness_budget_used = warp_budget_used;
    }
}

impl Drop for CaptureStatsWriter {
    fn drop(&mut self) {
        let mut payload = serde_json::json!({
            "longest_submission_stall_ms": self.longest_submission_stall_ms,
            "warp_readiness_budget_used": self.warp_readiness_budget_used,
        });
        if let Some(early_exit) = &self.early_exit {
            payload["early_exit"] = serde_json::json!({
                "case": early_exit.case,
                "k": early_exit.k,
                "elapsed_ms": early_exit.elapsed_ms,
                "reason": early_exit.reason,
            });
        }
        let Ok(json) = serde_json::to_string_pretty(&payload) else {
            return;
        };
        if let Err(err) = std::fs::write(&self.path, json) {
            logger::warn!("capture stats write {}: {err}", self.path.display());
        }
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

fn log_still_waiting(
    last_log_at: &mut Instant,
    wait_timing: &PngStabilityWaitTiming,
    state: &PngStabilityState,
) {
    if last_log_at.elapsed() < timing::LOG_INTERVAL {
        return;
    }
    let now = Instant::now();
    logger::info!(
        "Session: still waiting for fresh PNG (elapsed={:?}, scene_window_open={}, last_mtime={:?})",
        wait_timing.scene_submit_instant.elapsed(),
        state.render_window_open(now),
        state.last_seen_mtime()
    );
    *last_log_at = now;
}

fn arm_flat_backstop(
    case_name: &str,
    wait_started: Instant,
    deadline_floor: Duration,
    warp: &mut WarpBudgetWatch,
) {
    if !warp.used || !crate::scene_dsl::early_flat_exit_applies(case_name, true, SystemTime::now())
    {
        return;
    }
    warp.deadline =
        wait_started + flat_exit::flat_exit_budget(true, Duration::ZERO, deadline_floor);
    warp.flat_backstop = true;
}

fn finish_flat_early_exit(
    output_path: &Path,
    case_name: &str,
    wait_started: Instant,
    settle: &GpuFrameSettle,
    stats: &mut CaptureStatsWriter,
    warp_used: bool,
    progress: &CaptureProgress,
) -> Result<SceneSessionOutcome, HarnessError> {
    let elapsed = wait_started.elapsed();
    logger::info!(
        "Session: quarantine #216 early exit case={case_name} k={} elapsed={elapsed:?} reason={}",
        flat_exit::QUARANTINE_FLAT_READBACKS,
        flat_exit::FLAT_STABLE_REASON
    );
    stats.set_early_exit(crate::scene_dsl::output::EarlyExit {
        case: case_name.to_string(),
        k: flat_exit::QUARANTINE_FLAT_READBACKS,
        elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        reason: flat_exit::FLAT_STABLE_REASON.to_string(),
    });
    stats.sync(settle.longest_stall_ms(), warp_used);
    Err(flat_or_unstable_error(
        output_path,
        elapsed,
        &progress.chunks,
        progress.last_observed,
    ))
}

fn case_name_from_output(output_path: &Path) -> String {
    output_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

mod flat_exit;

#[cfg(test)]
mod tests;
