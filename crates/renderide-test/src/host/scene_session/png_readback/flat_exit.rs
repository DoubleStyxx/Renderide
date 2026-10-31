//! Early end of the #216 WARP flat-frame wait.
//!
//! The readiness wait still uses [`crate::golden::is_flat_image`]. K samples of
//! that flat image must match, and they must be separated by presented frames
//! so a clear color during WARP warm-up does not end the wait.

use std::time::{Duration, SystemTime};

use image::RgbaImage;

use super::super::gpu_ready::FrameReadiness;

/// Consecutive settled flat readbacks before the wait ends.
///
/// Three, not one. A single clear-color frame while WARP is still warming up
/// is not the stuck flat capture this quarantine covers.
pub(super) const QUARANTINE_FLAT_READBACKS: u32 = 3;

/// Presented frames required between those readbacks.
///
/// Two skips the immediately following presented frame. Polling the same PNG
/// twice, or taking the next frame, can still be that warm-up clear.
pub(super) const QUARANTINE_FLAT_FRAME_GAP: u32 = 2;

/// Wall-clock cap after the first settled flat frame. Other captures keep 300s.
///
/// The cap does not start at the beginning of the wait. A frame that is still
/// compiling is not the known flat capture, so that wait keeps the full budget.
pub(super) const QUARANTINE_FLAT_BACKSTOP: Duration = Duration::from_secs(60);

/// `flat_stable` when K identical settled frames were seen.
pub(super) const FLAT_STABLE_REASON: &str = "flat_stable";

/// Pass to [`flat_exit_budget`] for the ceiling that is not the 60s cap.
///
/// [`super::arm_flat_backstop`] uses this until the first settled flat frame.
/// A frame that is still compiling keeps the full WARP budget. The 60s cap
/// starts only after that settled frame.
pub(super) const NOT_THE_FLAT_BACKSTOP: bool = false;

/// Budget for one capture wait.
///
/// `applies` is a quarantined WARP case before the #216 expiry. Anything else
/// keeps `warp_budget` (300s on WARP, the caller's budget otherwise).
pub(super) fn flat_exit_budget(applies: bool, warp_budget: Duration, floor: Duration) -> Duration {
    let budget = if applies {
        QUARANTINE_FLAT_BACKSTOP
    } else {
        warp_budget
    };
    budget.max(floor)
}

/// Counts spaced, identical, settled flat readbacks.
#[derive(Debug, Default)]
pub(super) struct FlatExitWatch {
    presented: u32,
    last_mtime: Option<SystemTime>,
    streak: u32,
    last_counted_presented: Option<u32>,
    last_pixels: Option<Vec<u8>>,
}

/// Result of one stable-frame observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FlatExit {
    /// Keep waiting.
    Continue,
    /// K identical settled flat readbacks have been seen.
    Exit,
}

impl FlatExitWatch {
    /// `mtime` identifies one presented frame. The same mtime does not advance.
    ///
    /// A non-flat image resets the streak. A flat image that is not settled
    /// (pending is not 0, GPU-done is false, or dropped draws are not 0) does
    /// not count and does not reset. Flatness is [`crate::golden::is_flat_image`],
    /// the same check the capture records on the frame.
    pub(super) fn observe(
        &mut self,
        mtime: SystemTime,
        settled: bool,
        image: Option<&RgbaImage>,
    ) -> FlatExit {
        if self.last_mtime != Some(mtime) {
            self.presented = self.presented.saturating_add(1);
            self.last_mtime = Some(mtime);
        }
        let Some(image) = image else {
            self.reset();
            return FlatExit::Continue;
        };
        if !crate::golden::is_flat_image(image) {
            self.reset();
            return FlatExit::Continue;
        }
        if !settled {
            return FlatExit::Continue;
        }
        let spaced = self.last_counted_presented.is_none_or(|previous| {
            self.presented >= previous.saturating_add(QUARANTINE_FLAT_FRAME_GAP)
        });
        if !spaced {
            return FlatExit::Continue;
        }
        let pixels = image.as_raw();
        if self.last_pixels.as_deref() == Some(pixels.as_slice()) {
            self.streak = self.streak.saturating_add(1);
        } else {
            self.streak = 1;
            self.last_pixels = Some(pixels.clone());
        }
        self.last_counted_presented = Some(self.presented);
        if self.streak >= QUARANTINE_FLAT_READBACKS {
            FlatExit::Exit
        } else {
            FlatExit::Continue
        }
    }

    fn reset(&mut self) {
        self.streak = 0;
        self.last_counted_presented = None;
        self.last_pixels = None;
    }
}

/// Splits a finished wait into settle time and K-window time.
///
/// Settle time is the wait start until the first settled flat frame.
/// K-window time is that frame until the exit. `k_window` is the time since
/// the first settled flat frame. A window longer than `elapsed` is clamped.
pub(super) fn split_exit_ms(elapsed: Duration, k_window: Duration) -> (u64, u64) {
    let elapsed_ms = duration_ms(elapsed);
    let k_window_ms = duration_ms(k_window).min(elapsed_ms);
    (elapsed_ms.saturating_sub(k_window_ms), k_window_ms)
}

/// Log line for a #216 early exit, with settle time and K-window time apart.
pub(super) fn early_exit_log_line(
    case_name: &str,
    k: u32,
    settle: Duration,
    k_window: Duration,
    elapsed: Duration,
) -> String {
    format!(
        "Session: quarantine #216 early exit case={case_name} k={k} settle={settle:?} k_window={k_window:?} elapsed={elapsed:?} reason={FLAT_STABLE_REASON}"
    )
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Pending 0, GPU-done, and no dropped draws. That is the #216 flat signature.
pub(super) fn frame_is_settled(frame: &FrameReadiness) -> bool {
    frame.gpu_done && frame.pending == Some(0) && frame.dropped_draws == Some(0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use image::{Rgba, RgbaImage};

    use super::super::ObservedChunks;
    use super::*;
    use crate::scene_dsl::apply_quarantine;
    use crate::scene_dsl::case_is_quarantined_216;
    use crate::scene_dsl::suite::{SuiteCaseReport, SuiteReport};

    const CASES: &[&str] = &[
        "pbs_lit_material_matrix",
        "negative_light_parity",
        "negative_light_physically_based",
        "negative_color_physically_based",
    ];

    fn before_expiry() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000)
    }

    fn after_expiry() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_hours(1_139_568)
    }

    fn solid(color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(2, 2, Rgba(color))
    }

    fn settled() -> FrameReadiness {
        FrameReadiness {
            gpu_done: true,
            pending: Some(0),
            dropped_draws: Some(0),
            flat: true,
            stall_ms: 0,
        }
    }

    fn at(index: u32) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(u64::from(index) + 10)
    }

    #[test]
    fn flat_and_stable_exits_early_with_the_same_error_and_quarantine_label() {
        let image = solid([8, 8, 8, 255]);
        let mut watch = FlatExitWatch::default();
        let frame = settled();
        let mut decision = FlatExit::Continue;
        for index in 1..=5 {
            decision = watch.observe(at(index), frame_is_settled(&frame), Some(&image));
            if index < 5 {
                assert_eq!(decision, FlatExit::Continue);
            }
        }
        assert_eq!(decision, FlatExit::Exit);

        let mut chunks = ObservedChunks::default();
        chunks.note(Some("0"));
        let wait = Duration::from_millis(12_500);
        let path = PathBuf::from("out/pbs_lit_material_matrix/actual.png");
        let err = super::super::flat_or_unstable_error(
            &path,
            wait,
            &chunks,
            Some(super::super::ObservedSettleState {
                flat: true,
                pending: Some(0),
                gpu_done: true,
                dropped_draws: Some(0),
            }),
        );
        let twin = crate::HarnessError::FlatOrUnstableFrame {
            path: PathBuf::from("out/pbs_lit_material_matrix/actual.png"),
            wait,
            last_pending: 0,
            ready_chunks: 1,
            pending_chunks: 0,
            missing_chunks: 0,
            frame_count: 1,
            frame_pending: Some(0),
            gpu_done: true,
            dropped_draws: Some(0),
        };
        assert_eq!(err.to_string(), twin.to_string());
        assert!(err.to_string().contains("flat or unstable frame"));

        for name in CASES {
            let dir = tempfile::tempdir().expect("tempdir");
            let report_path = dir.path().join("report.json");
            let integration = crate::scene_dsl::cases::lookup(name).expect("case");
            let written = crate::scene_dsl::output::report_from_error(&integration, "x");
            std::fs::write(&report_path, serde_json::to_string(&written).expect("json"))
                .expect("report");
            let case = SuiteCaseReport::from_harness_error(
                *name,
                dir.path().to_path_buf(),
                report_path,
                &err,
            );
            let mut suite = SuiteReport::from_cases(vec![case]);
            apply_quarantine(&mut suite, before_expiry(), |_| true);
            let labeled = &suite.cases[0];
            if *name == "pbs_lit_material_matrix" {
                assert!(case_is_quarantined_216(labeled), "{name}");
                assert_eq!(
                    labeled.reason.as_deref(),
                    Some("quarantined:#216"),
                    "{name}"
                );
            } else {
                assert!(!case_is_quarantined_216(labeled), "{name}");
                assert!(labeled.reason.is_none(), "{name}");
            }
        }
    }

    #[test]
    fn flat_then_non_flat_before_k_does_not_exit() {
        let flat = solid([8, 8, 8, 255]);
        let mut mixed = flat.clone();
        mixed.put_pixel(1, 1, Rgba([200, 10, 10, 255]));
        let mut watch = FlatExitWatch::default();
        let frame = settled();
        assert_eq!(watch.observe(at(1), true, Some(&flat)), FlatExit::Continue);
        assert_eq!(watch.observe(at(3), true, Some(&flat)), FlatExit::Continue);
        assert_eq!(watch.observe(at(5), true, Some(&mixed)), FlatExit::Continue);
        assert_eq!(watch.observe(at(7), true, Some(&flat)), FlatExit::Continue);
        assert!(crate::golden::is_flat_image(&flat));
        assert!(!crate::golden::is_flat_image(&mixed));
        let _ = frame;
    }

    #[test]
    fn changing_flat_frames_do_not_exit_before_the_backstop() {
        let mut watch = FlatExitWatch::default();
        for index in 1..=12 {
            let color = u8::try_from(index).expect("color");
            let image = solid([color, color, color, 255]);
            assert_eq!(
                watch.observe(at(index), true, Some(&image)),
                FlatExit::Continue
            );
        }
        assert_eq!(
            flat_exit_budget(true, Duration::from_secs(300), Duration::from_secs(3)),
            QUARANTINE_FLAT_BACKSTOP
        );
        assert_eq!(
            flat_exit_budget(
                NOT_THE_FLAT_BACKSTOP,
                Duration::from_secs(300),
                Duration::from_secs(3)
            ),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn the_pre_settle_ceiling_is_the_full_warp_budget() {
        let warp = Duration::from_secs(300);
        let floor = Duration::from_secs(3);
        assert_eq!(flat_exit_budget(NOT_THE_FLAT_BACKSTOP, warp, floor), warp);
        assert_eq!(
            flat_exit_budget(true, warp, floor),
            QUARANTINE_FLAT_BACKSTOP
        );
    }

    #[test]
    fn settle_time_and_k_window_split_the_wait() {
        let elapsed = Duration::from_millis(12_500);
        let k_window = Duration::from_millis(8_500);
        assert_eq!(split_exit_ms(elapsed, k_window), (4_000, 8_500));
        assert_eq!(split_exit_ms(elapsed, Duration::ZERO), (12_500, 0));
        assert_eq!(split_exit_ms(elapsed, Duration::from_secs(30)), (0, 12_500));
        let (settle_ms, k_window_ms) = split_exit_ms(elapsed, k_window);
        let line = early_exit_log_line(
            "pbs_lit_material_matrix",
            QUARANTINE_FLAT_READBACKS,
            Duration::from_millis(settle_ms),
            Duration::from_millis(k_window_ms),
            elapsed,
        );
        assert!(line.contains("case=pbs_lit_material_matrix"), "{line}");
        assert!(line.contains("k=3"), "{line}");
        assert!(line.contains("settle=4s"), "{line}");
        assert!(line.contains("k_window=8.5s"), "{line}");
        assert!(line.contains("elapsed=12.5s"), "{line}");
        assert!(line.contains("reason=flat_stable"), "{line}");
    }

    #[test]
    fn a_non_warp_case_keeps_the_full_wait() {
        for name in CASES {
            assert!(!crate::scene_dsl::early_flat_exit_applies(
                name,
                false,
                before_expiry()
            ));
        }
        assert_eq!(
            flat_exit_budget(false, Duration::from_secs(300), Duration::from_secs(3)),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn a_non_quarantined_case_keeps_the_full_wait() {
        assert!(!crate::scene_dsl::early_flat_exit_applies(
            "unlit_sphere",
            true,
            before_expiry()
        ));
        assert_eq!(
            flat_exit_budget(false, Duration::from_secs(300), Duration::from_secs(3)),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn an_expired_quarantine_keeps_the_full_wait() {
        assert!(crate::scene_dsl::early_flat_exit_applies(
            "pbs_lit_material_matrix",
            true,
            before_expiry()
        ));
        assert!(!crate::scene_dsl::early_flat_exit_applies(
            "pbs_lit_material_matrix",
            true,
            after_expiry()
        ));
        for name in [
            "negative_light_parity",
            "negative_light_physically_based",
            "negative_color_physically_based",
        ] {
            assert!(!crate::scene_dsl::early_flat_exit_applies(
                name,
                true,
                before_expiry()
            ));
        }
        assert_eq!(
            flat_exit_budget(false, Duration::from_secs(300), Duration::from_secs(3)),
            Duration::from_secs(300)
        );
    }
}
