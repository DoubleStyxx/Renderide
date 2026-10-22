//! Ordering for handing an OpenXR swapchain image back to the compositor.
//!
//! `Queue::submit` returning only means the Vulkan submission was queued. Separate
//! `vkQueueSubmit` batches do not execute in submission order unless a semaphore or
//! fence chains them. wgpu orders its own batches with a relay semaphore, but
//! `xrReleaseSwapchainImage` on an out-of-process runtime (Monado, WiVRn) records a
//! barrier and submits it on the session queue without waiting on that semaphore.
//! RADV overlaps those submits, so the compositor can sample the image while the eye
//! copy is still running. That alternates finished frames with black or garbage and
//! flashes the headset.
//!
//! The projection path therefore polls the writing [`wgpu::SubmissionIndex`] to
//! completion before `xrReleaseSwapchainImage`. A timed-out wait still releases the
//! image: leaving it acquired would wedge the next `xrBeginFrame`. That one frame may
//! flash, and the timeout is logged.

use std::fmt;
use std::time::Duration;

use crate::log_throttle::LogThrottle;

/// How long the driver waits for the swapchain-writing submission to retire.
///
/// Matches the OpenXR finalize watchdog. A healthy VR frame finishes well inside this.
/// On timeout the image is still released so a stuck GPU cannot block the session.
pub(super) const SWAPCHAIN_GPU_VISIBILITY_TIMEOUT: Duration = Duration::from_millis(500);

static SWAPCHAIN_GPU_WAIT_LOG: LogThrottle = LogThrottle::new();

/// CPU-side phase of one projection swapchain handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SwapchainHandoffPhase {
    /// `Queue::submit` has returned. GPU writes may still be in flight.
    CpuSubmitted,
    /// The submission that wrote the image has completed, or this batch wrote nothing.
    GpuWritesVisible,
    /// The GPU wait failed. The image must still be released so the frame can close.
    GpuWaitTimedOut,
    /// `xrReleaseSwapchainImage` has returned for this frame.
    ImageReleased,
    /// `xrEndFrame` has returned for this frame.
    FrameEnded,
}

/// Transition applied to [`SwapchainHandoffPhase`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SwapchainHandoffEvent {
    /// App writes are finished, or this batch submitted no swapchain writes.
    GpuWritesVisible,
    /// [`wait_for_projection_writes`] did not observe GPU completion.
    GpuWaitTimedOut,
    /// `xrReleaseSwapchainImage` is about to run, or has just been decided.
    ReleaseImage,
    /// `xrEndFrame` is about to run, or has just been decided.
    EndFrame,
}

/// Illegal swapchain handoff transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SwapchainHandoffError {
    /// Release while the copy submission may still be executing.
    ReleaseBeforeGpuVisible,
    /// `xrEndFrame` while the acquired image has not been released.
    EndFrameBeforeRelease,
    /// A second GPU-visibility transition after the image was already resolved.
    DuplicateGpuWait,
    /// A second release after the image was already released.
    DuplicateRelease,
    /// Any event after `xrEndFrame` already closed the handoff.
    EventAfterFrameEnded,
}

/// Result of waiting for the projection batch's GPU writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectionGpuWait {
    /// This batch submitted no command buffers, so no image write is in flight.
    NotRequired,
    /// `Device::poll` reported the requested submission finished.
    Visible,
    /// The wait timed out or failed. Release still has to run.
    TimedOut,
}

/// Inputs for [`wait_for_projection_writes`].
pub(super) struct ProjectionGpuWaitRequest<'a> {
    /// Device whose fence backs [`Self::submission`].
    pub(super) device: &'a wgpu::Device,
    /// Submission that contains the eye copy (and optional depth transfer).
    pub(super) submission: &'a wgpu::SubmissionIndex,
    /// Command buffers in that submission.
    pub(super) command_buffers: usize,
    /// Frame sequence recorded on the finalize payload, for the timeout log.
    pub(super) frame_seq: u64,
    /// Acquired color image index, for the timeout log.
    pub(super) image_index: u32,
}

impl fmt::Display for SwapchainHandoffPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CpuSubmitted => "cpu_submitted",
            Self::GpuWritesVisible => "gpu_writes_visible",
            Self::GpuWaitTimedOut => "gpu_wait_timed_out",
            Self::ImageReleased => "image_released",
            Self::FrameEnded => "frame_ended",
        })
    }
}

impl fmt::Display for SwapchainHandoffEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::GpuWritesVisible => "gpu_writes_visible",
            Self::GpuWaitTimedOut => "gpu_wait_timed_out",
            Self::ReleaseImage => "release_image",
            Self::EndFrame => "end_frame",
        })
    }
}

impl fmt::Display for SwapchainHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ReleaseBeforeGpuVisible => {
                "xrReleaseSwapchainImage before the writing submission is GPU-visible"
            }
            Self::EndFrameBeforeRelease => "xrEndFrame before xrReleaseSwapchainImage",
            Self::DuplicateGpuWait => "duplicate GPU-visibility transition",
            Self::DuplicateRelease => "xrReleaseSwapchainImage twice",
            Self::EventAfterFrameEnded => "swapchain handoff event after xrEndFrame",
        })
    }
}

/// Returns whether a projection finalize must wait out the batch's submission.
///
/// An empty batch did not write the acquired image. Waiting on that empty submission
/// would not prove anything about a previous batch, and the projection copy is always
/// submitted in the same batch as the finalize that releases the image.
pub(super) const fn projection_finalize_requires_gpu_wait(command_buffers: usize) -> bool {
    command_buffers > 0
}

/// Legal event order for one projection finalize after `wait` has been observed.
///
/// [`crate::gpu::driver_thread::xr_finalize`] applies these three events in order,
/// around the real release and `xrEndFrame` calls. Tests replay the array so a
/// reorder that releases before the GPU wait fails without a headset.
pub(super) const fn projection_handoff_events(
    wait: ProjectionGpuWait,
) -> [SwapchainHandoffEvent; 3] {
    let gpu_event = match wait {
        ProjectionGpuWait::Visible | ProjectionGpuWait::NotRequired => {
            SwapchainHandoffEvent::GpuWritesVisible
        }
        ProjectionGpuWait::TimedOut => SwapchainHandoffEvent::GpuWaitTimedOut,
    };
    [
        gpu_event,
        SwapchainHandoffEvent::ReleaseImage,
        SwapchainHandoffEvent::EndFrame,
    ]
}

/// Advances one handoff step.
///
/// `ReleaseImage` is rejected from [`SwapchainHandoffPhase::CpuSubmitted`]. That is the
/// transition the compositor flash uses: release while the eye copy may still be running.
pub(super) const fn advance_swapchain_handoff(
    phase: SwapchainHandoffPhase,
    event: SwapchainHandoffEvent,
) -> Result<SwapchainHandoffPhase, SwapchainHandoffError> {
    use SwapchainHandoffError as Error;
    use SwapchainHandoffEvent as Event;
    use SwapchainHandoffPhase as Phase;
    match (phase, event) {
        (Phase::CpuSubmitted, Event::GpuWritesVisible) => Ok(Phase::GpuWritesVisible),
        (Phase::CpuSubmitted, Event::GpuWaitTimedOut) => Ok(Phase::GpuWaitTimedOut),
        (Phase::CpuSubmitted, Event::ReleaseImage) => Err(Error::ReleaseBeforeGpuVisible),
        (Phase::CpuSubmitted, Event::EndFrame) => Err(Error::EndFrameBeforeRelease),
        (Phase::GpuWritesVisible | Phase::GpuWaitTimedOut, Event::ReleaseImage) => {
            Ok(Phase::ImageReleased)
        }
        (Phase::GpuWritesVisible | Phase::GpuWaitTimedOut, Event::EndFrame) => {
            Err(Error::EndFrameBeforeRelease)
        }
        (
            Phase::GpuWritesVisible | Phase::GpuWaitTimedOut,
            Event::GpuWritesVisible | Event::GpuWaitTimedOut,
        ) => Err(Error::DuplicateGpuWait),
        (Phase::ImageReleased, Event::EndFrame) => Ok(Phase::FrameEnded),
        (Phase::ImageReleased, Event::ReleaseImage) => Err(Error::DuplicateRelease),
        (Phase::ImageReleased, Event::GpuWritesVisible | Event::GpuWaitTimedOut) => {
            Err(Error::DuplicateGpuWait)
        }
        (Phase::FrameEnded, _) => Err(Error::EventAfterFrameEnded),
    }
}

/// Applies `event`, logging and keeping `phase` when the transition is illegal.
///
/// The projection path only feeds [`projection_handoff_events`], which are legal.
/// A rejected transition is a programming error. The caller still performs the
/// OpenXR call: skipping `xrReleaseSwapchainImage` would leak the acquired image
/// and stall the next `xrBeginFrame`.
pub(super) fn commit_swapchain_handoff(
    phase: SwapchainHandoffPhase,
    event: SwapchainHandoffEvent,
) -> SwapchainHandoffPhase {
    match advance_swapchain_handoff(phase, event) {
        Ok(next) => next,
        Err(error) => {
            logger::error!("OpenXR swapchain handoff rejected {event} while {phase}: {error}");
            phase
        }
    }
}

/// Blocks until `request.submission` is GPU-visible, when this batch wrote the image.
///
/// The poll is a fence wait on the device. It must not run while the caller holds
/// [`crate::gpu::GpuQueueAccessGate`]: the main thread takes that gate around
/// `queue.write_buffer`, and this wait does not submit anything.
pub(super) fn wait_for_projection_writes(
    request: &ProjectionGpuWaitRequest<'_>,
) -> ProjectionGpuWait {
    if !projection_finalize_requires_gpu_wait(request.command_buffers) {
        return ProjectionGpuWait::NotRequired;
    }
    profiling::scope!("driver::xr_wait_gpu_visible");
    let poll_type = wgpu::PollType::Wait {
        submission_index: Some(request.submission.clone()),
        timeout: Some(SWAPCHAIN_GPU_VISIBILITY_TIMEOUT),
    };
    match request.device.poll(poll_type) {
        Ok(status) if status.wait_finished() => ProjectionGpuWait::Visible,
        Ok(status) => {
            log_swapchain_gpu_wait_failure(request, poll_status_label(status));
            ProjectionGpuWait::TimedOut
        }
        Err(error) => {
            log_swapchain_gpu_wait_failure(request, &error);
            ProjectionGpuWait::TimedOut
        }
    }
}

/// Stable label for a poll result that did not report the wait as finished.
fn poll_status_label(status: wgpu::PollStatus) -> &'static str {
    match status {
        wgpu::PollStatus::QueueEmpty => "queue_empty",
        wgpu::PollStatus::WaitSucceeded => "wait_succeeded",
        wgpu::PollStatus::Poll => "poll",
    }
}

/// Logs a throttled warning when the image will be released without GPU visibility.
fn log_swapchain_gpu_wait_failure(
    request: &ProjectionGpuWaitRequest<'_>,
    detail: impl fmt::Display,
) {
    let Some(occurrence) = SWAPCHAIN_GPU_WAIT_LOG.should_log(4, 64) else {
        return;
    };
    logger::warn!(
        "OpenXR swapchain GPU wait failed before xrReleaseSwapchainImage; releasing anyway so the frame can close. The compositor may flash this frame. occurrence={occurrence} detail={detail} frame_seq={} command_buffers={} image_index={}",
        request.frame_seq,
        request.command_buffers,
        request.image_index,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectionGpuWait, SwapchainHandoffError, SwapchainHandoffEvent, SwapchainHandoffPhase,
        advance_swapchain_handoff, projection_finalize_requires_gpu_wait,
        projection_handoff_events,
    };

    fn replay(
        events: [SwapchainHandoffEvent; 3],
    ) -> Result<SwapchainHandoffPhase, SwapchainHandoffError> {
        let mut phase = SwapchainHandoffPhase::CpuSubmitted;
        for event in events {
            phase = advance_swapchain_handoff(phase, event)?;
        }
        Ok(phase)
    }

    #[test]
    fn only_batches_that_submitted_writes_wait_for_the_gpu() {
        assert!(!projection_finalize_requires_gpu_wait(0));
        assert!(projection_finalize_requires_gpu_wait(1));
        assert!(projection_finalize_requires_gpu_wait(2));
    }

    #[test]
    fn production_order_ends_the_frame_after_a_visible_wait() {
        for wait in [ProjectionGpuWait::Visible, ProjectionGpuWait::NotRequired] {
            let phase = replay(projection_handoff_events(wait)).expect("legal handoff");
            assert_eq!(phase, SwapchainHandoffPhase::FrameEnded);
            assert_eq!(
                projection_handoff_events(wait)[0],
                SwapchainHandoffEvent::GpuWritesVisible
            );
        }
    }

    #[test]
    fn timed_out_wait_still_releases_and_ends_the_frame() {
        let events = projection_handoff_events(ProjectionGpuWait::TimedOut);
        assert_eq!(events[0], SwapchainHandoffEvent::GpuWaitTimedOut);
        let phase = replay(events).expect("timeout still closes the frame");
        assert_eq!(phase, SwapchainHandoffPhase::FrameEnded);
    }

    #[test]
    fn release_before_gpu_visibility_is_rejected() {
        let error = advance_swapchain_handoff(
            SwapchainHandoffPhase::CpuSubmitted,
            SwapchainHandoffEvent::ReleaseImage,
        )
        .expect_err("release races the copy");
        assert_eq!(error, SwapchainHandoffError::ReleaseBeforeGpuVisible);
    }

    #[test]
    fn end_frame_before_release_is_rejected() {
        let visible = advance_swapchain_handoff(
            SwapchainHandoffPhase::CpuSubmitted,
            SwapchainHandoffEvent::GpuWritesVisible,
        )
        .expect("writes visible");
        let error = advance_swapchain_handoff(visible, SwapchainHandoffEvent::EndFrame)
            .expect_err("end frame still needs release");
        assert_eq!(error, SwapchainHandoffError::EndFrameBeforeRelease);

        let timed_out = advance_swapchain_handoff(
            SwapchainHandoffPhase::CpuSubmitted,
            SwapchainHandoffEvent::GpuWaitTimedOut,
        )
        .expect("timeout resolved");
        let error = advance_swapchain_handoff(timed_out, SwapchainHandoffEvent::EndFrame)
            .expect_err("timeout still needs release before end frame");
        assert_eq!(error, SwapchainHandoffError::EndFrameBeforeRelease);
    }

    #[test]
    fn second_release_and_events_after_end_frame_are_rejected() {
        let released = advance_swapchain_handoff(
            SwapchainHandoffPhase::GpuWritesVisible,
            SwapchainHandoffEvent::ReleaseImage,
        )
        .expect("release after visibility");
        let error = advance_swapchain_handoff(released, SwapchainHandoffEvent::ReleaseImage)
            .expect_err("second release");
        assert_eq!(error, SwapchainHandoffError::DuplicateRelease);

        let ended = advance_swapchain_handoff(released, SwapchainHandoffEvent::EndFrame)
            .expect("end frame");
        let error = advance_swapchain_handoff(ended, SwapchainHandoffEvent::ReleaseImage)
            .expect_err("event after end frame");
        assert_eq!(error, SwapchainHandoffError::EventAfterFrameEnded);
    }
}
