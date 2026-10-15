//! Vulkan API version policy for OpenXR-backed wgpu initialization.

use ash::vk;
use openxr as xr;

use super::types::XrBootstrapError;

/// Converts an OpenXR [`xr::Version`] to a Vulkan `VkApplicationInfo::apiVersion` value.
fn xr_version_to_vulkan_api_version(xr: xr::Version) -> u32 {
    vk::make_api_version(0, u32::from(xr.major()), u32::from(xr.minor()), xr.patch())
}

/// Formats a packed Vulkan API version for diagnostics.
pub(super) fn format_vk_api_version(version: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(version),
        vk::api_version_minor(version),
        vk::api_version_patch(version)
    )
}

/// Picks a single Vulkan instance `apiVersion` that satisfies wgpu-hal (Vulkan **1.2+** for promoted
/// [`vkWaitSemaphores`] / timeline semaphores), the loader's reported instance version, and OpenXR
/// [`xr::graphics::vulkan::Requirements`].
///
/// Returns the highest version allowed by all constraints (typically `min(loader, OpenXR max,
/// project cap)`), which must be at least `max(1.2, OpenXR min)`.
pub(super) fn choose_vulkan_api_version_for_wgpu(
    loader_instance_version: u32,
    reqs: &<xr::Vulkan as xr::Graphics>::Requirements,
) -> Result<u32, XrBootstrapError> {
    const WGPU_MIN_VULKAN: u32 = vk::API_VERSION_1_2;
    const PROJECT_CAP_VULKAN: u32 = vk::API_VERSION_1_3;

    let xr_min_vk = xr_version_to_vulkan_api_version(reqs.min_api_version_supported);
    let xr_max_vk = xr_version_to_vulkan_api_version(reqs.max_api_version_supported);

    let floor = WGPU_MIN_VULKAN.max(xr_min_vk);
    let ceiling = loader_instance_version
        .min(xr_max_vk)
        .min(PROJECT_CAP_VULKAN);

    if floor > ceiling {
        return Err(XrBootstrapError::Message(format!(
            "No Vulkan API version works for wgpu + OpenXR: need at least {} (wgpu requires Vulkan 1.2+ for timeline semaphores), but loader and runtime allow at most {} (OpenXR max {}).",
            format_vk_api_version(floor),
            format_vk_api_version(ceiling),
            reqs.max_api_version_supported
        )));
    }

    Ok(ceiling)
}

#[cfg(test)]
mod tests {
    use super::*;

    type VulkanGraphicsRequirements = <xr::Vulkan as xr::Graphics>::Requirements;

    #[test]
    fn chooses_ceiling_when_loader_and_openxr_allow_1_3() {
        let reqs = VulkanGraphicsRequirements {
            min_api_version_supported: xr::Version::new(1, 0, 0),
            max_api_version_supported: xr::Version::new(1, 3, 0),
        };
        let v = choose_vulkan_api_version_for_wgpu(vk::API_VERSION_1_3, &reqs).unwrap();
        assert_eq!(v, vk::API_VERSION_1_3);
    }

    #[test]
    fn clamps_to_loader_when_openxr_allows_higher() {
        let reqs = VulkanGraphicsRequirements {
            min_api_version_supported: xr::Version::new(1, 0, 0),
            max_api_version_supported: xr::Version::new(1, 3, 0),
        };
        let v = choose_vulkan_api_version_for_wgpu(vk::API_VERSION_1_2, &reqs).unwrap();
        assert_eq!(v, vk::API_VERSION_1_2);
    }

    #[test]
    fn errors_when_openxr_max_below_wgpu_floor() {
        let reqs = VulkanGraphicsRequirements {
            min_api_version_supported: xr::Version::new(1, 0, 0),
            max_api_version_supported: xr::Version::new(1, 1, 0),
        };
        assert!(choose_vulkan_api_version_for_wgpu(vk::API_VERSION_1_3, &reqs).is_err());
    }
}
