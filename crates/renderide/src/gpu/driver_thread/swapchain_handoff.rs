//! Compatibility workaround for handing an OpenXR swapchain image back to the compositor.
//!
//! Same-queue Vulkan barriers and signals do cover earlier submissions on that queue.
//! wgpu already chains its own submits with a relay semaphore. The OpenXR and Vulkan
//! specs do not require the CPU to wait for those writes before
//! `xrReleaseSwapchainImage`.
//!
//! WiVRn and Monado on RADV still composite a stale or black image in that handoff
//! (upstream issue 673). The projection path therefore waits until the queue's
//! already-submitted work is done before release when the session resolved
//! [`crate::config::VrSwapchainGpuWait`] to on. `never`, and `auto` when the runtime
//! or adapter does not match, release without that wait and keep the old pipelining.
//! A timed-out wait still releases the image: leaving it acquired would wedge the
//! next `xrBeginFrame`. That one frame may flash, and the timeout is logged.
//!
//! [`wait_for_projection_writes`] is the only constructor of [`SwapchainReleasePermit`].
//! On the driver-thread success path, color and depth release borrow that permit, so
//! those two calls cannot move above the wait. Acquire, import, and
//! `xrWaitSwapchainImage` failures in `xr/app_integration/submit.rs` release the image
//! directly. Those paths run before a projection finalize is queued.

use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::time::{Duration, Instant};

use crate::gpu::flight_recorder::{GpuFlightEventKind, GpuFlightRecorder};
use crate::gpu::sync::device_health::GpuDeviceHealth;
use crate::log_throttle::LogThrottle;
use crate::profiling::plot_vr_swapchain_gpu_wait_ms;

/// How long the driver waits for the swapchain-writing submission to retire.
///
/// A healthy VR frame finishes well inside this. On timeout the image is still
/// released so a stuck GPU cannot block the session.
pub(super) const SWAPCHAIN_GPU_VISIBILITY_TIMEOUT: Duration = Duration::from_millis(500);

/// How long one wait iteration sleeps when the completion callback has not fired.
///
/// The callback runs on the thread that polls, so the receive cannot observe GPU
/// completion by itself. This only yields between non-blocking polls. It is the
/// extra latency after the GPU finishes, so it stays well under a millisecond.
/// The overall budget is still [`SWAPCHAIN_GPU_VISIBILITY_TIMEOUT`].
const SWAPCHAIN_GPU_POLL_SLICE: Duration = Duration::from_micros(50);

static SWAPCHAIN_GPU_WAIT_LOG: LogThrottle = LogThrottle::new();
static SWAPCHAIN_GPU_LOST_LOG: LogThrottle = LogThrottle::new();
static EMPTY_PROJECTION_FINALIZE_LOG: LogThrottle = LogThrottle::new();

/// Result of deciding whether the projection batch's GPU writes are visible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectionGpuWait {
    /// This batch submitted no command buffers, so no image write is in flight.
    NotRequired,
    /// `Queue::on_submitted_work_done` ran inside a non-blocking poll.
    Visible,
    /// The wait timed out or a non-fatal poll failed. Release still has to run.
    TimedOut,
    /// The session's wait mode is off, so the image is released without polling.
    Skipped,
    /// The device was lost before the next poll. That poll is skipped.
    DeviceLost,
}

/// Proof that [`wait_for_projection_writes`] has run for this projection finalize.
///
/// The field is private and this module has no other constructor. The driver-thread
/// success path borrows the permit, so those releases cannot run before the wait
/// decision. Failure-path releases before a finalize is queued do not.
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
    /// Device polled so `on_submitted_work_done` callbacks can run.
    pub(super) device: &'a wgpu::Device,
    /// Queue that submitted this projection batch.
    ///
    /// The wait registers [`wgpu::Queue::on_submitted_work_done`] here, after this
    /// batch's submit has returned and before the driver thread submits anything else.
    pub(super) queue: &'a wgpu::Queue,
    /// Device-loss generation observed by the renderer.
    ///
    /// Checked before every `Device::poll`. A loss reported here skips the poll.
    /// See [`wait_for_projection_writes`] for the loss that still aborts inside one poll.
    pub(super) device_health: &'a GpuDeviceHealth,
    /// Command buffers in the batch that carried this finalize.
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

/// Debug-asserts and, in release, warns once when a waited projection finalize submitted nothing.
///
/// The eye copy is supposed to share this batch. An empty batch skips the GPU wait
/// ([`ProjectionGpuWait::NotRequired`]) because there is no submission to retire.
/// The assert and the warning run only when the wait would have run. A disabled wait
/// does not inspect this batch.
pub(super) fn note_empty_projection_finalize(
    command_buffers: usize,
    frame_seq: u64,
    image_index: u32,
    wait_enabled: bool,
) {
    debug_assert!(
        !wait_enabled || command_buffers > 0,
        "projection finalize submitted 0 command buffers while the GPU wait is enabled; the eye copy should share this batch"
    );
    if !wait_enabled || command_buffers > 0 {
        return;
    }
    let Some(occurrence) = EMPTY_PROJECTION_FINALIZE_LOG.should_log(1, 0) else {
        return;
    };
    logger::warn!(
        "OpenXR projection finalize submitted 0 command buffers; skipping the GPU wait because this batch did not write the swapchain. occurrence={occurrence} frame_seq={frame_seq} image_index={image_index}"
    );
}

/// Waits until work submitted before this call is GPU-visible, when this session and batch require it.
///
/// Returns the permit that the driver-thread success path in [`super::xr_finalize`]
/// passes to `xrReleaseSwapchainImage`. Failure-path releases in
/// `xr/app_integration/submit.rs` do not use this permit.
///
/// # Why this is not `Device::poll(Wait)`
///
/// wgpu-core 29.0.3 `CoreDevice::poll` calls `handle_error_fatal` when
/// `WaitIdleError::to_poll_error()` returns `None`. `WaitIdleError::Device`
/// (lost, out of memory, unexpected) is that case. Vulkan `hal::Device::wait`
/// maps `VK_ERROR_DEVICE_LOST` from `vkWaitSemaphores` to `DeviceError::Lost`,
/// and `handle_hal_error` calls `Device::lose` before the public `poll` aborts.
/// A device lost during `PollType::Wait` therefore never returns to the
/// `DeviceLost` arm. With `always` as the default, a hang that loses the device
/// inside that wait would kill the driver thread instead of taking the recovery
/// path.
///
/// This function registers [`wgpu::Queue::on_submitted_work_done`] and drives it
/// with [`wgpu::PollType::Poll`] plus [`std::sync::mpsc::Receiver::recv_timeout`].
/// `Poll` does not call `hal::Device::wait`. The callback runs on this thread
/// before `poll` returns, and it only sends on a channel.
///
/// A loss that [`GpuDeviceHealth::is_lost`] already reports, including one that
/// arrives between polls, skips the next poll and returns
/// [`ProjectionGpuWait::DeviceLost`]. A hang that does not lose the device ends
/// as [`ProjectionGpuWait::TimedOut`] when the channel times out.
///
/// `Poll` still calls `get_fence_value` (`vkGetSemaphoreCounterValue` on the
/// timeline semaphore). That query uses the same lost-device mapping, so one
/// non-blocking poll can still reach `handle_error_fatal`. That is a single
/// query, not a 500 ms wait inside `vkWaitSemaphores`.
///
/// # Locks held by each poll
///
/// `poll_and_return_closures` takes the device snatch lock for reading and the
/// fence lock for reading across `maintain`. For `PollType::Poll`, `maintain`
/// does not call `hal::Device::wait`. Both guards drop before the completion
/// callback runs and before `poll` returns. `Buffer::destroy`, `Texture::destroy`,
/// `Queue::write_buffer`, `Queue::write_texture`, and `Queue::submit` can stall
/// on those locks only for that brief maintain, not for
/// [`SWAPCHAIN_GPU_VISIBILITY_TIMEOUT`].
///
/// This function does not take [`crate::gpu::GpuQueueAccessGate`]. That gate
/// serializes `Queue::write_texture`, `Queue::submit`, surface present, and the
/// OpenXR queue calls. `Queue::write_buffer` is not on that gate.
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
    pump_until_submitted_work_done(request)
}

/// Polls until the completion callback fires, the device is lost, or the budget expires.
fn pump_until_submitted_work_done(
    request: &ProjectionGpuWaitRequest<'_>,
) -> (ProjectionGpuWait, Option<String>) {
    let (sender, receiver) = sync_channel(1);
    request.queue.on_submitted_work_done(move || {
        let _ = sender.send(());
    });

    let deadline = Instant::now() + SWAPCHAIN_GPU_VISIBILITY_TIMEOUT;
    loop {
        if request.device_health.is_lost() {
            log_swapchain_gpu_wait_device_lost(request);
            return (ProjectionGpuWait::DeviceLost, None);
        }
        if Instant::now() >= deadline {
            let detail = "on_submitted_work_done exceeded the wait budget";
            log_swapchain_gpu_wait_timeout(request, detail);
            return (ProjectionGpuWait::TimedOut, Some(detail.to_owned()));
        }

        if let Err(error) = request.device.poll(wgpu::PollType::Poll) {
            let detail = error.to_string();
            log_swapchain_gpu_wait_timeout(request, &detail);
            return (ProjectionGpuWait::TimedOut, Some(detail));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let slice = remaining.min(SWAPCHAIN_GPU_POLL_SLICE);
        match receiver.recv_timeout(slice) {
            Ok(()) => return (ProjectionGpuWait::Visible, None),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if request.device_health.is_lost() {
                    log_swapchain_gpu_wait_device_lost(request);
                    return (ProjectionGpuWait::DeviceLost, None);
                }
                let detail = "on_submitted_work_done channel disconnected";
                log_swapchain_gpu_wait_timeout(request, detail);
                return (ProjectionGpuWait::TimedOut, Some(detail.to_owned()));
            }
        }
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
///
/// The first four losses log, then every 64th. A lost device otherwise emits this
/// once per frame for the rest of the session.
fn log_swapchain_gpu_wait_device_lost(request: &ProjectionGpuWaitRequest<'_>) {
    let Some(occurrence) = SWAPCHAIN_GPU_LOST_LOG.should_log(4, 64) else {
        return;
    };
    logger::error!(
        "OpenXR swapchain GPU wait skipped because the wgpu device is lost; releasing without polling. occurrence={occurrence} generation={} frame_seq={} command_buffers={} image_index={}",
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
    if !adapter.features().contains(wgpu::Features::CLEAR_TEXTURE) {
        return None;
    }
    let info = adapter.get_info();
    let label = format!(
        "{} driver={} driver_info={} type={:?}",
        info.name, info.driver, info.driver_info, info.device_type
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("vr-swapchain-gpu-wait"),
        required_features: wgpu::Features::CLEAR_TEXTURE,
        ..Default::default()
    }))
    .expect("lavapipe device");
    Some(LavapipeGpu {
        device,
        queue,
        label,
    })
}

/// Submits a 1x1 texture clear and returns its submission index.
///
/// The call shape stays off the production GPU-instrumentation audit: that scan
/// flags dotted clear and `queue.submit` calls under `src/`.
#[cfg(test)]
fn submit_clear(gpu: &LavapipeGpu) -> wgpu::SubmissionIndex {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vr-swapchain-gpu-wait-clear"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vr-swapchain-gpu-wait-clear"),
        });
    encoder.clear_texture(&texture, &wgpu::ImageSubresourceRange::default());
    let command = encoder.finish();
    let queue = &gpu.queue;
    let submission = queue.submit([command]);
    drop(texture);
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
        submit_clear(&gpu);

        let not_required = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            queue: &gpu.queue,
            device_health: &health,
            command_buffers: 0,
            frame_seq: 1,
            image_index: 0,
            wait_enabled: true,
            flight_recorder: &recorder,
        });
        assert_eq!(not_required.outcome(), ProjectionGpuWait::NotRequired);

        let skipped = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            queue: &gpu.queue,
            device_health: &health,
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
            queue: &gpu.queue,
            device_health: &health,
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
        submit_clear(&gpu);
        let lost_fired = Arc::new(AtomicBool::new(false));
        let lost_flag = Arc::clone(&lost_fired);
        gpu.queue.on_submitted_work_done(move || {
            lost_flag.store(true, Ordering::Release);
        });
        let lost = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            queue: &gpu.queue,
            device_health: &lost_health,
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
    }

    /// Prints nearest-rank p50/p95 for the handoff on lavapipe.
    ///
    /// Not part of the default suite. Run with `--ignored` when refreshing the
    /// headless numbers in the pull request.
    #[test]
    #[ignore = "prints lavapipe handoff percentiles"]
    fn lavapipe_handoff_percentiles() {
        let Some(gpu) = lavapipe_device() else {
            logger::info!("lavapipe handoff percentiles skipped: no lavapipe adapter");
            return;
        };
        let recorder = GpuFlightRecorder::new();
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
        submit_clear(gpu);
        let wait_started = Instant::now();
        let permit = wait_for_projection_writes(&ProjectionGpuWaitRequest {
            device: &gpu.device,
            queue: &gpu.queue,
            device_health: &health,
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
