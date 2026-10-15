//! [`GpuMsaa`] sub-handle owned by [`super::GpuContext`].
//!
//! Holds the device's supported MSAA tier lists (desktop and stereo) discovered at
//! construction by [`crate::gpu::adapter::msaa_support::MsaaSupport`], plus the effective
//! sample counts the runtime selects each tick by clamping the user request through
//! [`crate::gpu::adapter::msaa_support::clamp_msaa_request_to_supported`].

use crate::gpu::adapter::msaa_support::{MsaaSupport, clamp_msaa_request_to_supported};

/// MSAA tier state for the desktop window and the stereo OpenXR path.
#[derive(Debug)]
pub struct GpuMsaa {
    /// MSAA tiers supported for the configured surface color format and forward depth/stencil
    /// format (sorted ascending: 2, 4, ...). Empty means MSAA is unavailable.
    supported_sample_counts: Vec<u32>,
    /// MSAA tiers supported for **2D array** color + forward depth/stencil format on the OpenXR
    /// path (sorted ascending). Empty when the adapter lacks [`wgpu::Features::MULTISAMPLE_ARRAY`]
    /// or [`wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`], which silently clamps the
    /// stereo request to `1` (MSAA off).
    supported_sample_counts_stereo: Vec<u32>,
    /// Effective swapchain MSAA sample count this frame (1 = off).
    effective: u32,
    /// Requested stereo MSAA (from settings) before clamping; set each XR frame by the runtime.
    requested_stereo: u32,
    /// Effective stereo MSAA sample count (1 = off).
    effective_stereo: u32,
}

impl GpuMsaa {
    /// Builds an MSAA tier state from the adapter probe captured at GPU construction.
    pub(super) fn new(support: MsaaSupport) -> Self {
        Self {
            supported_sample_counts: support.desktop,
            supported_sample_counts_stereo: support.stereo,
            effective: 1,
            requested_stereo: 1,
            effective_stereo: 1,
        }
    }

    /// Adapter-reported maximum MSAA sample count for the swapchain color format and depth.
    pub fn msaa_max_sample_count(&self) -> u32 {
        self.supported_sample_counts.last().copied().unwrap_or(1)
    }

    /// Adapter-reported maximum MSAA sample count for **2D array** color + depth (stereo / OpenXR path).
    ///
    /// Returns `1` when the device lacks [`wgpu::Features::MULTISAMPLE_ARRAY`] or
    /// [`wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`], in which case the stereo forward
    /// path silently falls back to no MSAA.
    pub fn msaa_max_sample_count_stereo(&self) -> u32 {
        self.supported_sample_counts_stereo
            .last()
            .copied()
            .unwrap_or(1)
    }

    /// Effective MSAA sample count for the main window this frame (after [`Self::set_swapchain_msaa_requested`]).
    pub fn swapchain_msaa_effective(&self) -> u32 {
        self.effective
    }

    /// Effective stereo MSAA sample count for the OpenXR path this frame (after
    /// [`Self::set_swapchain_msaa_requested_stereo`]). `1` = off.
    pub fn swapchain_msaa_effective_stereo(&self) -> u32 {
        self.effective_stereo
    }

    /// Sets requested MSAA for the desktop swapchain path; values are rounded to a **format-valid**
    /// tier ([`Self::msaa_max_sample_count`]), not merely capped by the maximum tier.
    ///
    /// Call each frame before graph execution (from [`crate::config::RenderingSettings::msaa`]).
    pub fn set_swapchain_msaa_requested(&mut self, requested: u32) {
        self.effective = clamp_msaa_request_to_supported(requested, &self.supported_sample_counts);
    }

    /// Sets requested MSAA for the OpenXR stereo path; clamps to a format-valid tier against the
    /// stereo supported list. When `MULTISAMPLE_ARRAY` is unavailable the stereo list is empty and
    /// the effective count silently becomes `1`.
    ///
    /// Call each XR frame before graph execution (from [`crate::config::RenderingSettings::msaa`]).
    pub fn set_swapchain_msaa_requested_stereo(&mut self, requested: u32) {
        let requested = requested.max(1);
        let effective =
            clamp_msaa_request_to_supported(requested, &self.supported_sample_counts_stereo);
        if self.requested_stereo != requested || self.effective_stereo != effective {
            if requested > 1 && effective != requested {
                logger::info!(
                    "VR MSAA clamped: requested {}x -> effective {}x (supported={:?})",
                    requested,
                    effective,
                    self.supported_sample_counts_stereo
                );
            }
            self.requested_stereo = requested;
            self.effective_stereo = effective;
        }
    }
}
