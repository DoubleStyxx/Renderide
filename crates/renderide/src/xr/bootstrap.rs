//! Create a Vulkan instance and device via OpenXR `KHR_vulkan_enable2`, then wrap with wgpu.
//!
//! Vulkan validation layers follow the same rules as desktop GPU init:
//! [`crate::config::DebugSettings::gpu_validation_layers`] in `config.toml` (and
//! `RENDERIDE_GPU_VALIDATION`), plus `WGPU_*` env overrides via
//! [`crate::gpu::instance_flags_for_gpu_init`].
//!
//! ## Layout
//!
//! - **`instance`** -- OpenXR entry loader, instance creation with extension negotiation, and
//!   the `vkGetInstanceProcAddr` shim used by every OpenXR call into Vulkan.
//! - **`vulkan`** -- ash Vulkan instance + logical device creation through OpenXR
//!   `KHR_vulkan_enable2`.
//! - **`extensions`** -- data-driven OpenXR extension negotiation table consumed by `instance`.
//! - **`features`** -- data-driven wgpu feature negotiation table consumed by `vulkan`.
//! - **`wgpu_assembly`** -- wgpu-hal Vulkan adapter / device assembly and final packaging into
//!   [`XrWgpuHandles`].
//! - **`session_init`** -- OpenXR session, reference space, and controller-input wiring.
//! - **`init`** -- public [`init_wgpu_openxr`] orchestrator.
//! - **`types`** / **`version`** -- shared error and Vulkan-version policy.

mod extensions;
mod features;
mod init;
mod instance;
mod session_init;
mod types;
mod version;
mod vulkan;
mod wgpu_assembly;

pub use init::init_wgpu_openxr;
pub use types::XrWgpuHandles;
