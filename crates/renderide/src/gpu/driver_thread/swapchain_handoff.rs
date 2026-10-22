//! Compatibility workaround for handing an OpenXR swapchain image back to the compositor.
//!
//! Same-queue Vulkan barriers and signals do cover earlier submissions on that queue.
//! wgpu already chains its own submits with a relay semaphore. The OpenXR and Vulkan
//! specs do not require the CPU to wait for those writes before
//! `xrReleaseSwapchainImage`.
//!
//! WiVRn and Monado on RADV still composite a stale or black image in that handoff
//! (upstream issue 673). The projection path therefore polls the writing submission
//! to completion before release when the session resolved
//! [`crate::config::VrSwapchainGpuWait`] to on. `never`, and `auto` when the runtime
//! or adapter does not match, release without that poll and keep the old pipelining.
//! A timed-out wait still releases the image: leaving it acquired would wedge the
//! next `xrBeginFrame`. That one frame may flash, and the timeout is logged.
//!
//! [`wait_for_projection_writes`] is the only constructor of [`SwapchainReleasePermit`].
//! Both swapchain releases borrow the permit, so neither call can be moved above the
//! wait.

use std::time::{Duration, Instant};

use crate::gpu::flight_recorder::{GpuFlightEventKind, GpuFlightRecorder};
use crate::gpu::sync::device_health::GpuDeviceHealth;
use crate::log_throttle::LogThrottle;
use crate::profiling::plot_vr_swapchain_gpu_wait_ms;

/// How long the driver waits for the swapchain-writing submission to retire.
///
/// A healthy VR frame finishes well inside this. On timeout the image is still
/// released so a stuck GPU cannot block the session. This is also the bound on the
/// wgpu lock stall documented on [`wait_for_projection_writes`].
pub(super) const SWAPCHAIN_GPU_VISIBILITY_TIMEOUT: Duration = Duration::from_millis(500);

static SWAPCHAIN_GPU_WAIT_LOG: LogThrottle = LogThrottle::new();
static EMPTY_PROJECTION_FINALIZE_LOG: LogThrottle = LogThrottle::new();

/// Result of deciding whether the projection batch's GPU writes are visible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectionGpuWait {
    /// This batch submitted no command buffers, so no image write is in flight.
    NotRequired,
    /// `Device::poll` reported the requested submission finished.
    Visible,
    /// The wait timed out or the poll failed. Release still has to run.
    TimedOut,
    /// The session's wait mode is off, so the image is released without polling.
    Skipped,
    /// The device was already lost. The poll is skipped so wgpu does not abort.
    DeviceLost,
}

/// Proof that [`wait_for_projection_writes`] has run for this projection finalize.
///
/// The field is private and this module has no other constructor. Release functions
/// borrow the permit, so they cannot run before the wait decision.
pub(super) struct SwapchainReleasePermit {
    outcome: ProjectionGpuWait,
}

impl SwapchainReleasePermit {
    /// Reports how the wait was resolved.
    pub(super) fn outcome(&self) -> ProjectionGpuWait {
        self.outcome
    }
}

/// Inputs for [`wait_for_projection_writes`].
pub(super) struct ProjectionGpuWaitRequest<'a> {
    /// Device whose fence backs [`Self::submission`].
    pub(super) device: &'a wgpu::Device,
    /// Device-loss generation observed by the renderer.
    ///
    /// Checked before `Device::poll`. wgpu-core 29.0.3 turns a lost-device wait into
    /// a fatal error instead of [`wgpu::PollError`].
    pub(super) device_health: &'a GpuDeviceHealth,
    /// Submission that contains the eye copy (and optional depth transfer).
    pub(super) submission: &'a wgpu::SubmissionIndex,
    /// Command buffers in that submission.
    pub(super) command_buffers: usize,
    /// Frame sequence recorded on the finalize payload, for logs and the flight recorder.
    pub(super) frame_seq: u64,
    /// Acquired color image index, for logs and the flight recorder.
    pub(super) image_index: u32,
    /// Session-start resolution of `vr_swapchain_gpu_wait`.
    pub(super) wait_enabled: bool,
    /// Recorder that receives the timeout event.
    pub(super) flight_recorder: &'a GpuFlightRecorder,
}

/// Returns whether a projection finalize must wait out the batch's submission.
///
/// An empty batch did not write the acquired image. Waiting on that empty submission
/// would not prove anything about a previous batch, and the projection copy is
/// submitted in the same batch as the finalize that releases the image.
pub(super) const fn projection_finalize_requires_gpu_wait(command_buffers: usize) -> bool {
    command_buffers > 0
}

/// Debug-asserts and, in release, warns once when a projection finalize submitted nothing.
///
/// The eye copy is supposed to share this batch. An empty batch skips the GPU wait
/// ([`ProjectionGpuWait::NotRequired`]) because there is no submission to retire.
pub(super) fn note_empty_projection_finalize(
    command_buffers: usize,
    frame_seq: u64,
    image_index: u32,
) {
    debug_assert!(
        command_buffers > 0,
        "projection finalize submitted 0 command buffers; the eye copy should share this batch"
    );
    if command_buffers > 0 {
        return;
    }
    let Some(occurrence) = EMPTY_PROJECTION_FINALIZE_LOG.should_log(1, 0) else {
        return;
    };
    logger::warn!(
        "OpenXR projection finalize submitted 0 command buffers; skipping the GPU wait because this batch did not write the swapchain. occurrence={occurrence} frame_seq={frame_seq} image_index={image_index}"
    );
}

/// Blocks until `request.submission` is GPU-visible, when this session and batch require it.
///
/// Returns the permit that [`super::xr_finalize`] must pass to `xrReleaseSwapchainImage`.
///
/// # wgpu-core 29.0.3 locks held across the wait
///
/// `Device::poll(Wait)` reaches `poll_and_return_closures`, which takes the device
/// snatch lock for reading and the fence lock for reading, then calls `maintain`.
/// `maintain` calls `hal::Device::wait` before dropping either guard. Both locks are
/// held for the whole wait, bounded by [`SWAPCHAIN_GPU_VISIBILITY_TIMEOUT`].
///
/// While they are held:
/// - `Buffer::destroy` and `Texture::destroy` take the snatch lock for writing, so a
///   main-thread `destroy()` stalls until the wait returns.
/// - `Queue::write_buffer` and `Queue::write_texture` take the snatch lock for
///   reading. They share that lock with the poll. They stall when a snatch writer
///   is already queued, because parking_lot blocks new readers for a waiting writer.
/// - `Queue::submit` takes the snatch lock for reading and then the fence lock for
///   writing, so a submit stalls until the poll drops its fence read guard.
///
/// This function does not take [`crate::gpu::GpuQueueAccessGate`]. That gate
/// serializes `Queue::write_texture`, `Queue::submit`, surface present, and the
/// OpenXR queue calls. `Queue::write_buffer` is not on that gate. The stall above
/// is the wgpu snatch and fence locks, not the queue gate.
///
/// A device that [`GpuDeviceHealth::is_lost`] already reports is not polled. wgpu's
/// public `poll` treats device-loss as fatal (`WaitIdleError::Device` is not a
/// [`wgpu::PollError`]) and would abort the driver thread.
pub(super) fn wait_for_projection_writes(
    request: &ProjectionGpuWaitRequest<'_>,
) -> SwapchainReleasePermit {
    let started = Instant::now();
    let (outcome, timeout_detail) = resolve_projection_gpu_wait(request);
    let waited = started.elapsed();
    plot_vr_swapchain_gpu_wait_ms(waited);
    if let Some(detail) = timeout_detail {
        let waited_ms = u32::try_from(waited.as_millis()).unwrap_or(u32::MAX);
        request
            .flight_recorder
            .record(GpuFlightEventKind::SwapchainGpuWaitTimedOut {
                frame_seq: request.frame_seq,
                image_index: request.image_index,
                command_buffers: request.command_buffers,
                waited_ms,
                detail,
            });
    }
    SwapchainReleasePermit { outcome }
}

/// Chooses the wait outcome and, on timeout, the flight-recorder detail.
fn resolve_projection_gpu_wait(
    request: &ProjectionGpuWaitRequest<'_>,
) -> (ProjectionGpuWait, Option<String>) {
    if !request.wait_enabled {
        return (ProjectionGpuWait::Skipped, None);
    }
    if !projection_finalize_requires_gpu_wait(request.command_buffers) {
        return (ProjectionGpuWait::NotRequired, None);
    }
    if request.device_health.is_lost() {
        log_swapchain_gpu_wait_device_lost(request);
        return (ProjectionGpuWait::DeviceLost, None);
    }

    profiling::scope!("driver::xr_wait_gpu_visible");
    let poll_type = wgpu::PollType::Wait {
        submission_index: Some(request.submission.clone()),
        timeout: Some(SWAPCHAIN_GPU_VISIBILITY_TIMEOUT),
    };
    match request.device.poll(poll_type) {
        Ok(status) if status.wait_finished() => (ProjectionGpuWait::Visible, None),
        Ok(status) => {
            let detail = poll_status_label(status).to_owned();
            log_swapchain_gpu_wait_timeout(request, &detail);
            (ProjectionGpuWait::TimedOut, Some(detail))
        }
        Err(error) => {
            let detail = error.to_string();
            log_swapchain_gpu_wait_timeout(request, &detail);
            (ProjectionGpuWait::TimedOut, Some(detail))
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
fn log_swapchain_gpu_wait_timeout(request: &ProjectionGpuWaitRequest<'_>, detail: &str) {
    let Some(occurrence) = SWAPCHAIN_GPU_WAIT_LOG.should_log(4, 64) else {
        return;
    };
    logger::warn!(
        "OpenXR swapchain GPU wait timed out before xrReleaseSwapchainImage; releasing anyway so the frame can close. The compositor may flash this frame. occurrence={occurrence} detail={detail} frame_seq={} command_buffers={} image_index={}",
        request.frame_seq,
        request.command_buffers,
        request.image_index,
    );
}

/// Logs device-loss separately from a timeout. The image is still released.
fn log_swapchain_gpu_wait_device_lost(request: &ProjectionGpuWaitRequest<'_>) {
    logger::error!(
        "OpenXR swapchain GPU wait skipped because the wgpu device is already lost; releasing without polling. generation={} frame_seq={} command_buffers={} image_index={}",
        request.device_health.lost_generation(),
        request.frame_seq,
        request.command_buffers,
        request.image_index,
    );
}

/// Nearest-rank percentile. `samples` is sorted in place.
#[cfg(test)]
fn percentile(samples: &mut [f64], fraction: f64) -> f64 {
    assert!(!samples.is_empty());
    samples.sort_by(f64::total_cmp);
    let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
    samples[index.min(samples.len() - 1)]
}

/// Headless Vulkan device used by the lavapipe wait test.
#[cfg(test)]
struct LavapipeGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    label: String,
}

/// Opens a lavapipe (or other CPU Vulkan) device when the loader can see one.
#[cfg(test)]
fn lavapipe_device() -> Option<LavapipeGpu> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(descriptor);
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    let adapter = adapters.into_iter().find(|adapter| {
        let info = adapter.get_info();
        let driver = info.driver.to_ascii_lowercase();
        let name = info.name.to_ascii_lowercase();
        info.device_type == wgpu::DeviceType::Cpu
            || driver.contains("llvmpipe")
            || driver.contains("lvp")
            || name.contains("llvmpipe")
            || name.contains("lavapipe")
    })?;
    let info = adapter.get_info();
    let label = format!(
        "{} driver={} driver_info={} type={:?}",
        info.name, info.driver, info.driver_info, info.device_type
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("vr-swapchain-gpu-wait"),
        ..Default::default()
    }))
    .expect("lavapipe device");
    Some(LavapipeGpu {
        device,
        queue,
        label,
    })
}

/// Submits a 4-byte buffer clear and returns its submission index.
#[cfg(test)]
fn submit_clear(gpu: &LavapipeGpu) -> wgpu::SubmissionIndex {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vr-swapchain-gpu-wait-clear"),
        size: 4,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vr-swapchain-gpu-wait-clear"),
        });
    encoder.clear_buffer(&buffer, 0, None);
    let command = encoder.finish();
    let submission = gpu.queue.submit([command]);
    drop(buffer);
    submission
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    use super::{
        LavapipeGpu, ProjectionGpuWait, ProjectionGpuWaitRequest, lavapipe_device,
        projection_finalize_requires_gpu_wait, submit_clear, wait_for_projection_writes,
    };
    use crate::gpu::flight_recorder::GpuFlightRecorder;
    use crate::gpu::sync::device_health::GpuDeviceHealth;

    #[test]
    fn only_batches_that_submitted_writes_require_a_gpu_wait() {
        assert!(!projection_finalize_requires_gpu_wait(0));
        assert!(projection_finalize_requires_gpu_wait(1));
        assert!(projection_finalize_requires_gpu_wait(2));
    }

    #[test]
    fn lavapipe_wait_observes_a_real_submission() {
        let Some(gpu) = lavapipe_device() else {
            logger::info!("lavapipe swapchain wait test skipped: no lavapipe adapter");
            return;
        };
        let recorder = GpuFlightRecorder::new();
        let health = GpuDeviceHealth::new();
        let submission = submit_clear(&gpu);

        let not_required = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            device_health: &health,
            submission: &submission,
            command_buffers: 0,
            frame_seq: 1,
            image_index: 0,
            wait_enabled: true,
            flight_recorder: &recorder,
        });
        assert_eq!(not_required.outcome(), ProjectionGpuWait::NotRequired);

        let skipped = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            device_health: &health,
            submission: &submission,
            command_buffers: 1,
            frame_seq: 2,
            image_index: 1,
            wait_enabled: false,
            flight_recorder: &recorder,
        });
        assert_eq!(skipped.outcome(), ProjectionGpuWait::Skipped);

        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        gpu.queue.on_submitted_work_done(move || {
            flag.store(true, Ordering::Release);
        });
        let visible = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            device_health: &health,
            submission: &submission,
            command_buffers: 1,
            frame_seq: 3,
            image_index: 2,
            wait_enabled: true,
            flight_recorder: &recorder,
        });
        assert_eq!(visible.outcome(), ProjectionGpuWait::Visible);
        assert!(
            fired.load(Ordering::Acquire),
            "completion callback must have run inside the wait"
        );

        let lost_health = GpuDeviceHealth::new();
        lost_health.mark_lost("lavapipe test");
        let lost_submission = submit_clear(&gpu);
        let lost_fired = Arc::new(AtomicBool::new(false));
        let lost_flag = Arc::clone(&lost_fired);
        gpu.queue.on_submitted_work_done(move || {
            lost_flag.store(true, Ordering::Release);
        });
        let lost = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            device_health: &lost_health,
            submission: &lost_submission,
            command_buffers: 1,
            frame_seq: 4,
            image_index: 3,
            wait_enabled: true,
            flight_recorder: &recorder,
        });
        assert_eq!(lost.outcome(), ProjectionGpuWait::DeviceLost);
        assert!(
            !lost_fired.load(Ordering::Acquire),
            "device-lost must not poll, so the new callback stays pending"
        );

        let mut always_wait_ms = Vec::new();
        let mut always_cpu_ms = Vec::new();
        let mut never_wait_ms = Vec::new();
        let mut never_cpu_ms = Vec::new();
        for _ in 0..48 {
            let (cpu_ms, wait_ms) = time_handoff(&gpu, &recorder, true);
            always_cpu_ms.push(cpu_ms);
            always_wait_ms.push(wait_ms);
            let (cpu_ms, wait_ms) = time_handoff(&gpu, &recorder, false);
            never_cpu_ms.push(cpu_ms);
            never_wait_ms.push(wait_ms);
        }
        eprintln!(
            "lavapipe handoff adapter={} always_cpu_ms p50={:.4} p95={:.4} always_wait_ms p50={:.4} p95={:.4} never_cpu_ms p50={:.4} p95={:.4} never_wait_ms p50={:.4} p95={:.4}",
            gpu.label,
            super::percentile(&mut always_cpu_ms, 0.50),
            super::percentile(&mut always_cpu_ms, 0.95),
            super::percentile(&mut always_wait_ms, 0.50),
            super::percentile(&mut always_wait_ms, 0.95),
            super::percentile(&mut never_cpu_ms, 0.50),
            super::percentile(&mut never_cpu_ms, 0.95),
            super::percentile(&mut never_wait_ms, 0.50),
            super::percentile(&mut never_wait_ms, 0.95),
        );
    }

    /// Times submit plus the wait decision on a healthy device.
    fn time_handoff(
        gpu: &LavapipeGpu,
        recorder: &GpuFlightRecorder,
        wait_enabled: bool,
    ) -> (f64, f64) {
        let health = GpuDeviceHealth::new();
        let started = Instant::now();
        let submission = submit_clear(gpu);
        let wait_started = Instant::now();
        let permit = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            device_health: &health,
            submission: &submission,
            command_buffers: 1,
            frame_seq: 0,
            image_index: 0,
            wait_enabled,
            flight_recorder: recorder,
        });
        let wait_ms = wait_started.elapsed().as_secs_f64() * 1000.0;
        let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
        let expected = if wait_enabled {
            ProjectionGpuWait::Visible
        } else {
            ProjectionGpuWait::Skipped
        };
        assert_eq!(permit.outcome(), expected);
        (cpu_ms, wait_ms)
    }
}
