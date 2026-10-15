//! Mutex that serialises access to the Vulkan queue shared by wgpu and OpenXR.
//!
//! # wgpu-core lock ordering
//!
//! `Queue::write_texture` acquires the destination texture's `initialization_status`
//! `RwLock` (write) and then, with that guard still live, acquires `device.trackers`.
//! `Queue::submit` does the opposite: `trackers` first, then texture initialization takes
//! `initialization_status` write for every texture referenced by the baked command buffers.
//! With `write_texture` on the main thread and `submit` on the
//! [`super::driver_thread::DriverThread`], the two inner locks form an ABBA cycle -- observed as
//! a futex hang with the main thread parked in
//! `Queue::write_texture` and the driver parked in
//! `BakedCommands::initialize_texture_memory`.
//!
//! `Queue::write_buffer` (the asymmetric cousin) takes `trackers` first and
//! `initialization_status` second with no nesting, so it is not part of this cycle and is left
//! ungated.
//!
//! # OpenXR queue ownership
//!
//! OpenXR's Vulkan binding requires external synchronization for calls that may access the bound
//! `VkQueue`. Renderide binds OpenXR to the same Vulkan queue that backs [`wgpu::Queue`], so the
//! gate is also held around `xrBeginFrame`, `xrAcquireSwapchainImage`,
//! `xrReleaseSwapchainImage`, and `xrEndFrame`. Vulkan surface presentation also reaches the
//! queue via `vkQueuePresentKHR`, so [`wgpu::SurfaceTexture::present`] is serialized through the
//! same gate.
//!
//! # Scope
//!
//! The gate is held around `Queue::write_texture` call sites in the asset texture upload
//! path, decoded video texture callbacks, the driver thread's `Queue::submit`,
//! `SurfaceTexture::present`, and the narrow OpenXR calls listed above. Long waits such as `xrWaitFrame`,
//! `xrWaitSwapchainImage`, view location, and input sync stay outside the gate so
//! compositor stalls do not block unrelated GPU submissions.

use std::sync::Arc;

use parking_lot::Mutex;

/// Queue-gate acquisition policy for operations that can safely yield when the queue is busy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuQueueAccessMode {
    /// Wait until the shared queue gate can be acquired.
    #[default]
    Blocking,
    /// Return immediately when another queue owner currently holds the gate.
    NonBlocking,
}

/// Shared mutex acquired before operations that may access the renderer's Vulkan queue.
///
/// Instantiated once by [`super::GpuContext`] and cloned into the
/// [`super::driver_thread::DriverThread`], the texture asset upload path, and OpenXR
/// frame submission.
#[derive(Clone, Default)]
pub struct GpuQueueAccessGate {
    inner: Arc<Mutex<()>>,
}

impl GpuQueueAccessGate {
    /// Creates an uncontended gate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Locks the gate for the duration of the returned guard. Call immediately before
    /// [`wgpu::Queue::write_texture`], [`wgpu::Queue::submit`],
    /// [`wgpu::SurfaceTexture::present`], or an OpenXR queue-access call and drop the guard as
    /// soon as that call returns.
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.inner.lock()
    }

    /// Attempts to lock the gate without waiting for the current owner.
    pub fn try_lock(&self) -> Option<parking_lot::MutexGuard<'_, ()>> {
        self.inner.try_lock()
    }

    /// Locks according to `mode`, returning `None` only for contended non-blocking access.
    pub fn lock_for(&self, mode: GpuQueueAccessMode) -> Option<parking_lot::MutexGuard<'_, ()>> {
        match mode {
            GpuQueueAccessMode::Blocking => Some(self.lock()),
            GpuQueueAccessMode::NonBlocking => self.try_lock(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GpuQueueAccessGate;

    #[test]
    fn try_lock_returns_none_when_gate_is_held() {
        let gate = GpuQueueAccessGate::new();
        let _held = gate.lock();

        assert!(gate.try_lock().is_none());
    }

    #[test]
    fn try_lock_succeeds_after_guard_drops() {
        let gate = GpuQueueAccessGate::new();
        {
            let _held = gate.lock();
        }

        assert!(gate.try_lock().is_some());
    }
}
