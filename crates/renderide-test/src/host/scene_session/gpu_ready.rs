//! GPU-done capture gate and the WARP readiness budget.
//!
//! A frame is a capture candidate only after its own submission has completed
//! (`gpu_done`), with pending=0 and zero dropped draws, and the image is not flat.
//! The settle counts distinct GPU-completed frames. Wall-clock time does not.

use std::time::{Duration, SystemTime};

/// Distinct GPU-completed frames required before a candidate is accepted.
pub(crate) const GPU_SETTLE_FRAMES: u32 = 2;

/// Readiness budget used when the selected adapter is WARP.
///
/// A ~127s submission stall (#216) fits inside 300s measured from the start of
/// the capture wait. Non-WARP captures keep the caller's budget (180s).
pub(crate) const WARP_READINESS_BUDGET: Duration = Duration::from_secs(300);

/// Facts the harness reads from one stable PNG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameReadiness {
    /// `renderide_gpu_done` text is exactly `1`.
    pub gpu_done: bool,
    /// Pending compiling draws. `None` was not recorded and is not zero.
    pub pending: Option<u64>,
    /// Draws skipped for lack of a pipeline. `None` is not zero.
    pub dropped_draws: Option<u64>,
    /// The PNG decodes to one color.
    pub flat: bool,
    /// `renderide_submit_stall_ms` for this frame. Missing is zero.
    pub stall_ms: u64,
}

impl FrameReadiness {
    /// Pending=0, zero dropped draws, GPU-done, and not flat.
    pub(crate) fn is_candidate(self) -> bool {
        self.gpu_done && self.pending == Some(0) && self.dropped_draws == Some(0) && !self.flat
    }
}

/// What the settle machine does with one observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettleDecision {
    /// This observation is not a capture candidate and does not count.
    Reject,
    /// Counted, or already counted, and still short of an acceptable candidate.
    Hold,
    /// The current frame is a candidate and two GPU-completed frames have settled.
    Accept,
}

/// Counts distinct GPU-completed frames. The same mtime never counts twice.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GpuFrameSettle {
    armed: bool,
    completed: u32,
    last_mtime: Option<SystemTime>,
    longest_stall_ms: u64,
}

impl GpuFrameSettle {
    /// Longest `renderide_submit_stall_ms` seen on any observation.
    pub(crate) const fn longest_stall_ms(&self) -> u64 {
        self.longest_stall_ms
    }

    /// Distinct GPU-completed frames counted so far.
    #[cfg(test)]
    pub(crate) const fn completed_frames(&self) -> u32 {
        self.completed
    }

    /// Updates the settle from one stable PNG.
    ///
    /// `gpu_done=false` never increments and never accepts. A flat or not-ready
    /// frame before the first candidate does not arm the settle. After the first
    /// candidate, each later distinct GPU-completed frame increments, including a
    /// flat one, but only a current candidate is accepted.
    pub(crate) fn observe(&mut self, mtime: SystemTime, frame: FrameReadiness) -> SettleDecision {
        self.longest_stall_ms = self.longest_stall_ms.max(frame.stall_ms);
        if !frame.gpu_done {
            return SettleDecision::Reject;
        }
        if self.last_mtime == Some(mtime) {
            return self.decision_for_current(frame);
        }
        if !self.armed {
            if !frame.is_candidate() {
                return SettleDecision::Reject;
            }
            self.armed = true;
            self.completed = 1;
            self.last_mtime = Some(mtime);
            return SettleDecision::Hold;
        }
        self.completed = self.completed.saturating_add(1);
        self.last_mtime = Some(mtime);
        self.decision_for_current(frame)
    }

    fn decision_for_current(&self, frame: FrameReadiness) -> SettleDecision {
        if frame.is_candidate() && self.completed >= GPU_SETTLE_FRAMES {
            SettleDecision::Accept
        } else if self.armed {
            SettleDecision::Hold
        } else {
            SettleDecision::Reject
        }
    }
}

/// Readiness budget for one capture.
///
/// WARP gets at least [`WARP_READINESS_BUDGET`], measured from the start of the
/// wait. Any other adapter keeps `base` unchanged.
pub(crate) const fn warp_readiness_budget(is_warp: bool, base: Duration) -> Duration {
    if is_warp {
        if base.as_secs() > WARP_READINESS_BUDGET.as_secs() {
            base
        } else {
            WARP_READINESS_BUDGET
        }
    } else {
        base
    }
}

/// Pending and dropped counts encoded in `renderide_pipeline_pending`.
///
/// The chunk is the sum of still-compiling lookups and dropped draws, so only
/// `0` records both as zero. A missing or unreadable chunk is not zero.
pub(crate) fn counts_from_pending_chunk(chunk: Option<&str>) -> (Option<u64>, Option<u64>) {
    match chunk.map(str::trim) {
        Some("0") => (Some(0), Some(0)),
        Some(text) => match text.parse::<u64>() {
            Ok(pending) => (Some(pending), None),
            Err(_) => (None, None),
        },
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{
        FrameReadiness, GpuFrameSettle, SettleDecision, counts_from_pending_chunk,
        warp_readiness_budget,
    };

    fn candidate(stall_ms: u64) -> FrameReadiness {
        FrameReadiness {
            gpu_done: true,
            pending: Some(0),
            dropped_draws: Some(0),
            flat: false,
            stall_ms,
        }
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn capture_is_rejected_while_gpu_done_is_false() {
        let mut settle = GpuFrameSettle::default();
        let mut frame = candidate(0);
        frame.gpu_done = false;
        assert_eq!(settle.observe(at(1), frame), SettleDecision::Reject);
        assert_eq!(settle.observe(at(2), frame), SettleDecision::Reject);
        assert_eq!(settle.completed_frames(), 0);
        assert!(!frame.is_candidate());
        // The first real GPU-done frame still only arms the settle.
        assert_eq!(settle.observe(at(3), candidate(4)), SettleDecision::Hold);
        assert_eq!(settle.completed_frames(), 1);
    }

    #[test]
    fn settle_counts_distinct_gpu_completed_frames_not_repeated_observations() {
        let mut settle = GpuFrameSettle::default();
        assert_eq!(settle.observe(at(1), candidate(10)), SettleDecision::Hold);
        assert_eq!(settle.observe(at(1), candidate(10)), SettleDecision::Hold);
        assert_eq!(settle.observe(at(1), candidate(12)), SettleDecision::Hold);
        assert_eq!(settle.completed_frames(), 1);
        assert_eq!(settle.longest_stall_ms(), 12);
        assert_eq!(settle.observe(at(2), candidate(3)), SettleDecision::Accept);
        assert_eq!(settle.completed_frames(), 2);
        assert_eq!(settle.longest_stall_ms(), 12);
    }

    #[test]
    fn flat_frames_before_the_first_candidate_do_not_arm() {
        let mut settle = GpuFrameSettle::default();
        let mut flat = candidate(1);
        flat.flat = true;
        assert_eq!(settle.observe(at(1), flat), SettleDecision::Reject);
        assert_eq!(settle.completed_frames(), 0);
        let mut pending = candidate(1);
        pending.pending = Some(4);
        pending.dropped_draws = None;
        assert_eq!(settle.observe(at(2), pending), SettleDecision::Reject);
        assert_eq!(settle.completed_frames(), 0);
    }

    #[test]
    fn a_flat_gpu_done_frame_after_arm_increments_but_is_not_accepted() {
        let mut settle = GpuFrameSettle::default();
        assert_eq!(settle.observe(at(1), candidate(1)), SettleDecision::Hold);
        let mut flat = candidate(40);
        flat.flat = true;
        assert_eq!(settle.observe(at(2), flat), SettleDecision::Hold);
        assert_eq!(settle.completed_frames(), 2);
        assert_eq!(settle.observe(at(3), candidate(5)), SettleDecision::Accept);
        assert_eq!(settle.longest_stall_ms(), 40);
    }

    #[test]
    fn zero_dropped_draws_are_required_for_a_candidate() {
        let mut dropped = candidate(0);
        dropped.dropped_draws = Some(1);
        assert!(!dropped.is_candidate());
        let mut settle = GpuFrameSettle::default();
        assert_eq!(settle.observe(at(1), dropped), SettleDecision::Reject);
        assert_eq!(settle.completed_frames(), 0);
        assert_eq!(counts_from_pending_chunk(Some("0")), (Some(0), Some(0)));
        assert_eq!(counts_from_pending_chunk(Some("4")), (Some(4), None));
        assert_eq!(counts_from_pending_chunk(None), (None, None));
    }

    #[test]
    fn warp_budget_applies_only_to_warp_adapter_info() {
        let warp_device = u32::from_str_radix("8c", 16).expect("device");
        assert_eq!(warp_device, 0x008c);
        let warp = crate::scene_dsl::adapter_is_warp(0x1414, warp_device);
        let padded = crate::scene_dsl::adapter_is_warp(0x1414, 0x008c);
        assert!(warp);
        assert!(padded);
        assert!(!crate::scene_dsl::adapter_is_warp(0x1414, 0x0001));
        assert!(!crate::scene_dsl::adapter_is_warp(0x10005, 0));
        let base = Duration::from_secs(180);
        assert_eq!(warp_readiness_budget(warp, base), Duration::from_secs(300));
        assert_eq!(
            warp_readiness_budget(false, base),
            base,
            "a non-WARP adapter keeps the 180s budget"
        );
        assert_eq!(
            warp_readiness_budget(true, Duration::from_secs(400)),
            Duration::from_secs(400)
        );
    }
}
