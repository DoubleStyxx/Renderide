//! Adapter selection, feature negotiation, MSAA support discovery, and device creation.
//!
//! All construction-time decisions for [`crate::gpu::GpuContext`] live here. The
//! submodules separate pure scoring/policy ([`selection::power_preference_score`],
//! [`msaa_support::clamp_msaa_request_to_supported`]) from IO-bearing wrappers
//! ([`selection::select_adapter`], [`device::request_device_for_adapter`]) so the
//! policy can be exercised by unit tests without a live wgpu instance.

pub(crate) mod device;
pub(crate) mod features;
pub(crate) mod msaa_support;
pub(crate) mod selection;

#[cfg(test)]
mod tests;
