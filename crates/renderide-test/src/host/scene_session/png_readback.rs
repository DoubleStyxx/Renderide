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
    /// How many non-flat stable PNGs to ignore before accepting one.
    ///
    /// Async pipeline compiles can present one shader while the next is still building.
    /// A clear frame is already rejected. This count rejects that many later colored
    /// frames so the capture waits for the following write.
    pub non_flat_skips: u32,
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

    /// Rejects the current stable candidate so only a strictly newer PNG write can become
    /// `Stable`. Used when the candidate is a flat clear-only frame rendered before async
    /// material pipelines finished compiling.
    pub fn reject_candidate(&mut self) {
        self.stable_since = None;
    }
}

/// Whether the PNG at `path` decodes to a flat single-color image.
fn png_is_flat(path: &Path) -> bool {
    crate::image_io::load_rgba(path).is_ok_and(|img| crate::golden::is_flat_image(&img))
}

/// What to do with one mtime-stable PNG.
enum StablePng {
    /// Two successive writes matched. Safe to compare against the golden.
    Accept,
    /// Flat, partial, or not yet repeated. Wait for a newer write.
    KeepWaiting,
}

/// Rejects a flat or one-off frame. A repeated non-flat image is the steady capture.
fn consider_stable_png(
    path: &Path,
    repeated: &mut Option<Vec<u8>>,
    flat_seen: &mut bool,
) -> StablePng {
    if png_is_flat(path) {
        if !*flat_seen {
            logger::info!(
                "Session: stable PNG is flat; waiting for a later frame (pipelines may still be compiling)"
            );
        }
        *flat_seen = true;
        *repeated = None;
        return StablePng::KeepWaiting;
    }
    let bytes = std::fs::read(path).unwrap_or_default();
    if bytes.is_empty() {
        return StablePng::KeepWaiting;
    }
    if repeated.as_ref().is_some_and(|previous| previous == &bytes) {
        return StablePng::Accept;
    }
    logger::info!(
        "Session: PNG not yet repeated; waiting for the next write at {}",
        path.display()
    );
    *repeated = Some(bytes);
    StablePng::KeepWaiting
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
    // A frame can sit unchanged for the 200ms mtime window and still be replaced at the next
    // render interval. The first non-flat write is sometimes a pre-warmup image (one LOD, or the
    // null shader while the real pipeline compiles). Accept only once two successive writes match.
    let mut repeated_png: Option<Vec<u8>> = None;
    let mut non_flat_skips = wait_timing.non_flat_skips;

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
            // The renderer also withholds a PNG while a draw pipeline is still compiling.
            // A remaining one-off frame (single LOD before the fade weights land) must not win
            // just because its mtime sat still for 200ms between 500ms writes.
            match consider_stable_png(output_path, &mut repeated_png, &mut flat_candidate_seen) {
                StablePng::Accept => {
                    if non_flat_skips > 0 {
                        non_flat_skips -= 1;
                        logger::info!(
                            "Session: stable PNG is only a partial compile; waiting for a newer frame"
                        );
                        // The next write must match a new pair. Leaving the skipped bytes in
                        // `repeated_png` would accept the following identical partial immediately.
                        repeated_png = None;
                        state.reject_candidate();
                        std::thread::sleep(timing::POLL_INTERVAL);
                        continue;
                    }
                    logger::info!(
                        "Session: fresh PNG stabilized at {} after {:?}",
                        output_path.display(),
                        wait_timing.scene_submit_instant.elapsed()
                    );
                    return Ok(SceneSessionOutcome {
                        png_path: output_path.to_path_buf(),
                    });
                }
                StablePng::KeepWaiting => {
                    state.reject_candidate();
                    std::thread::sleep(timing::POLL_INTERVAL);
                    continue;
                }
            }
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

    if flat_candidate_seen {
        // Never rendered anything but a flat frame: hand it to the golden check so the
        // failure is reported as "flat single color" rather than "missing PNG".
        return Ok(SceneSessionOutcome {
            png_path: output_path.to_path_buf(),
        });
    }

    Err(HarnessError::PngOutputMissing {
        path: PathBuf::from(output_path),
        wait: wait_timing.overall_timeout,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant, SystemTime};

    use super::{PngObservation, PngStabilityState, PngStabilityVerdict, PngStabilityWaitTiming};

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
            non_flat_skips: 0,
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
    fn missing_file_keeps_pending() {
        let (mut s, i0, _t0) = fresh_state();
        let now = i0 + Duration::from_millis(2500);
        assert_eq!(
            s.observe(now, PngObservation::Missing),
            PngStabilityVerdict::Pending
        );
        assert!(s.last_seen_mtime().is_none());
    }
}
