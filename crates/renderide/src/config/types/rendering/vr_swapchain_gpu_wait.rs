//! When the driver waits for the VR eye copy before `xrReleaseSwapchainImage`.
//!
//! Persisted as `[rendering] vr_swapchain_gpu_wait`. The default is `always` because
//! releasing the image while the copy is still running flashes the headset on
//! WiVRn/Monado with RADV. That flash is a seizure risk. `auto` enables the wait
//! only for that combination. `never` restores the previous pipelined release.
//!
//! The mode is resolved once, when the OpenXR session starts, from the runtime name
//! and the adapter the runtime selected. Changing it later requires a renderer restart.

use crate::labeled_enum;

/// AMD's PCI vendor id (`VkPhysicalDeviceProperties::vendorID`).
const AMD_VENDOR_ID: u32 = 0x1002;

labeled_enum! {
    /// VR swapchain GPU-wait policy persisted as `[rendering] vr_swapchain_gpu_wait`.
    ///
    /// `always` polls the eye-copy submission to completion before
    /// `xrReleaseSwapchainImage`. `auto` does that only for a Monado or WiVRn runtime
    /// on an AMD or RADV adapter. `never` releases without that CPU wait.
    pub enum VrSwapchainGpuWait: "VR swapchain GPU wait (`always` / `auto` / `never`)" {
        default => Always;

        /// Wait on every runtime and adapter. This is the default.
        Always => {
            persist: "always",
            label: "Always",
            aliases: ["on", "true", "1", "enabled"],
        },
        /// Wait only for Monado/WiVRn on an AMD or RADV adapter.
        Auto => {
            persist: "auto",
            label: "Auto (Monado/WiVRn on AMD/RADV)",
        },
        /// Do not wait. Restores the pipelined release from before the workaround.
        Never => {
            persist: "never",
            label: "Never",
            aliases: ["off", "false", "0", "disabled"],
        },
    }
}

/// Runtime and adapter names used to resolve [`VrSwapchainGpuWait::Auto`].
#[derive(Clone, Copy, Debug)]
pub struct VrSwapchainGpuWaitQuery<'a> {
    /// OpenXR runtime name (`xrGetInstanceProperties`).
    pub runtime_name: &'a str,
    /// wgpu adapter name.
    pub adapter_name: &'a str,
    /// wgpu adapter driver string.
    pub driver: &'a str,
    /// wgpu adapter driver-info string.
    pub driver_info: &'a str,
    /// Vulkan vendor id from the adapter.
    pub vendor_id: u32,
}

/// [`VrSwapchainGpuWait`] after runtime and adapter matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedVrSwapchainGpuWait {
    /// Configured mode.
    pub mode: VrSwapchainGpuWait,
    /// Whether this session polls before `xrReleaseSwapchainImage`.
    pub enabled: bool,
    /// Runtime name contains Monado or WiVRn.
    pub monado_family_runtime: bool,
    /// Adapter is AMD (`0x1002`) or reports RADV / Radeon.
    pub amd_or_radv_adapter: bool,
}

impl VrSwapchainGpuWait {
    /// Resolves this mode for one OpenXR session.
    ///
    /// `always` and `never` ignore the query. `auto` enables the wait only when the
    /// runtime is Monado or WiVRn and the adapter is AMD or RADV. That is the
    /// combination that flashed; same-queue barriers already cover other drivers.
    pub fn resolve(self, query: VrSwapchainGpuWaitQuery<'_>) -> ResolvedVrSwapchainGpuWait {
        let monado_family_runtime = runtime_is_monado_family(query.runtime_name);
        let amd_or_radv_adapter = adapter_is_amd_or_radv(
            query.adapter_name,
            query.driver,
            query.driver_info,
            query.vendor_id,
        );
        let enabled = match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => monado_family_runtime && amd_or_radv_adapter,
        };
        ResolvedVrSwapchainGpuWait {
            mode: self,
            enabled,
            monado_family_runtime,
            amd_or_radv_adapter,
        }
    }
}

impl ResolvedVrSwapchainGpuWait {
    /// Logs the mode, the effective on/off bit, and the signals that produced it.
    ///
    /// Call once, when the OpenXR session is created.
    pub fn log_session_start(self, query: VrSwapchainGpuWaitQuery<'_>) {
        logger::info!(
            "VR swapchain GPU wait: mode={} effective={} runtime='{}' adapter='{}' driver='{}' driver_info='{}' vendor=0x{:04x} monado_family={} amd_or_radv={}",
            self.mode.persist_str(),
            if self.enabled { "on" } else { "off" },
            query.runtime_name,
            query.adapter_name,
            query.driver,
            query.driver_info,
            query.vendor_id,
            self.monado_family_runtime,
            self.amd_or_radv_adapter,
        );
    }
}

/// Monado and WiVRn are the runtimes whose release submit raced the eye copy.
fn runtime_is_monado_family(runtime_name: &str) -> bool {
    let key = runtime_name.to_ascii_lowercase();
    key.contains("monado") || key.contains("wivrn")
}

/// AMD's vendor id, or a RADV / Radeon driver string.
fn adapter_is_amd_or_radv(name: &str, driver: &str, driver_info: &str, vendor_id: u32) -> bool {
    if vendor_id == AMD_VENDOR_ID {
        return true;
    }
    let driver_key = driver.to_ascii_lowercase();
    let info_key = driver_info.to_ascii_lowercase();
    let name_key = name.to_ascii_lowercase();
    contains_radv_or_radeon(&driver_key)
        || contains_radv_or_radeon(&info_key)
        || contains_radv_or_radeon(&name_key)
}

/// Matches the RADV driver and Radeon marketing names without treating every "amd" substring as a hit.
fn contains_radv_or_radeon(value: &str) -> bool {
    value.contains("radv") || value.contains("radeon")
}

#[cfg(test)]
mod tests {
    use super::{ResolvedVrSwapchainGpuWait, VrSwapchainGpuWait, VrSwapchainGpuWaitQuery};
    use crate::config::types::RendererSettings;

    fn query<'a>(
        runtime_name: &'a str,
        adapter_name: &'a str,
        driver: &'a str,
        vendor_id: u32,
    ) -> VrSwapchainGpuWaitQuery<'a> {
        VrSwapchainGpuWaitQuery {
            runtime_name,
            adapter_name,
            driver,
            driver_info: "",
            vendor_id,
        }
    }

    fn resolved(
        mode: VrSwapchainGpuWait,
        runtime_name: &str,
        driver: &str,
        vendor_id: u32,
    ) -> ResolvedVrSwapchainGpuWait {
        mode.resolve(query(runtime_name, "adapter", driver, vendor_id))
    }

    #[test]
    fn default_is_always() {
        assert_eq!(VrSwapchainGpuWait::default(), VrSwapchainGpuWait::Always);
        assert!(
            VrSwapchainGpuWait::Always
                .resolve(query("SteamVR", "NVIDIA", "nvidia", 0x10de))
                .enabled
        );
    }

    #[test]
    fn never_stays_off_for_the_broken_combination() {
        let decision = resolved(VrSwapchainGpuWait::Never, "WiVRn", "radv", 0x1002);
        assert!(!decision.enabled);
        assert!(decision.monado_family_runtime);
        assert!(decision.amd_or_radv_adapter);
    }

    #[test]
    fn auto_requires_both_monado_family_and_amd_or_radv() {
        assert!(resolved(VrSwapchainGpuWait::Auto, "WiVRn", "radv", 0x1002).enabled);
        assert!(resolved(VrSwapchainGpuWait::Auto, "Monado", "nvidia", 0x1002).enabled);
        assert!(!resolved(VrSwapchainGpuWait::Auto, "wivrn", "lavapipe", 0).amd_or_radv_adapter);
        assert!(!resolved(VrSwapchainGpuWait::Auto, "WiVRn", "nvidia", 0x10de).enabled);
        assert!(!resolved(VrSwapchainGpuWait::Auto, "SteamVR", "radv", 0x1002).enabled);
        assert!(!resolved(VrSwapchainGpuWait::Auto, "Oculus", "lvp", 0).enabled);
    }

    #[test]
    fn radv_string_matches_without_the_amd_vendor_id() {
        let decision = VrSwapchainGpuWait::Auto.resolve(VrSwapchainGpuWaitQuery {
            runtime_name: "Monado",
            adapter_name: "unknown",
            driver: "mesa",
            driver_info: "Mesa 25.2.8 (RADV)",
            vendor_id: 0,
        });
        assert!(decision.amd_or_radv_adapter);
        assert!(decision.enabled);
    }

    #[test]
    fn missing_key_defaults_to_always_and_tokens_round_trip() {
        let parsed: RendererSettings = toml::from_str("[rendering]\n").expect("section");
        assert_eq!(
            parsed.rendering.vr_swapchain_gpu_wait,
            VrSwapchainGpuWait::Always
        );
        for &mode in VrSwapchainGpuWait::ALL {
            let toml = format!(
                "[rendering]\nvr_swapchain_gpu_wait = \"{}\"\n",
                mode.persist_str()
            );
            let parsed: RendererSettings = toml::from_str(&toml).expect("token");
            assert_eq!(parsed.rendering.vr_swapchain_gpu_wait, mode);
        }
        let mut settings = RendererSettings::default();
        settings.rendering.vr_swapchain_gpu_wait = VrSwapchainGpuWait::Never;
        let toml = toml::to_string(&settings).expect("serialize");
        assert!(
            toml.contains("vr_swapchain_gpu_wait = \"never\""),
            "serialized TOML was: {toml}"
        );
    }
}
