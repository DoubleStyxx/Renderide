//! Frame clock that holds evicted uniform offsets until submit completes.

use std::sync::atomic::{AtomicU64, Ordering};

/// Exact byte range returned by an evicted slot. Offsets stay aligned.
#[derive(Clone, Copy, Debug)]
pub(super) struct FreeSpan {
    pub(super) offset: u64,
    pub(super) size: u64,
}

/// Evicted range a recorded frame may still sample through `dynamic_offset`.
///
/// Promoted to [`FreeSpan`] only after the frame that last referenced it has finished
/// `Queue::submit` (`on_submitted_work_done`). Reusing it earlier lets a deferred
/// `write_buffer` overwrite bytes an already-recorded draw still reads.
#[derive(Clone, Copy, Debug)]
pub(super) struct PendingSpan {
    pub(super) offset: u64,
    pub(super) size: u64,
    /// Recording frame that retired this range. `0` means no frame had begun yet.
    pub(super) frame: u64,
}

/// Shared frame clock for every uniform-arena shard.
///
/// The recording thread advances `recording_frame`. The submit-done callback only
/// `fetch_max`es `completed_frame` (no wgpu calls: device-poll callbacks can deadlock).
/// Shards promote pending ranges when they next allocate on the recording thread.
#[derive(Debug)]
pub(in super::super) struct UniformArenaRetireClock {
    recording_frame: AtomicU64,
    completed_frame: AtomicU64,
    open_frame: AtomicU64,
}

impl UniformArenaRetireClock {
    pub(in super::super) fn new() -> Self {
        Self {
            recording_frame: AtomicU64::new(0),
            completed_frame: AtomicU64::new(0),
            open_frame: AtomicU64::new(0),
        }
    }

    /// Opens the next recording frame. The first frame is 1 so frame 0 never auto-promotes.
    pub(in super::super) fn begin_frame(&self) -> u64 {
        let frame = self.recording_frame.fetch_add(1, Ordering::Relaxed) + 1;
        self.open_frame.store(frame, Ordering::Relaxed);
        frame
    }

    pub(in super::super) fn mark_completed(&self, frame: u64) {
        self.completed_frame.fetch_max(frame, Ordering::Release);
    }

    pub(super) fn recording_frame(&self) -> u64 {
        self.recording_frame.load(Ordering::Relaxed)
    }

    pub(super) fn completed_frame(&self) -> u64 {
        self.completed_frame.load(Ordering::Acquire)
    }

    pub(super) fn open_frame(&self) -> u64 {
        self.open_frame.load(Ordering::Relaxed)
    }
}
