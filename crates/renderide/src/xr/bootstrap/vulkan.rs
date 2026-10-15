//! ash + Vulkan instance/device creation routed through OpenXR `KHR_vulkan_enable2`.
//!
//! Every call into OpenXR that needs a Vulkan loader pointer forwards through the
//! [`super::instance::vk_get_instance_proc_addr_shim`] so no fn-pointer transmute is required.

use std::ffi::c_void;

use ash::khr::timeline_semaphore as khr_timeline_semaphore;
use ash::vk::{self, Handle};
use openxr as xr;
use wgpu::hal;
use wgpu::hal::api::Vulkan as HalVulkan;
use wgpu::wgt;

use super::features::negotiate_wgpu_features;
use super::instance::{
    CACHED_VK_GET_INSTANCE_PROC_ADDR, VulkanGraphicsRequirements, vk_get_instance_proc_addr_shim,
};
use super::types::XrBootstrapError;
use super::version::choose_vulkan_api_version_for_wgpu;

/// Converts the fixed-size NUL-padded `device_name` from [`vk::PhysicalDeviceProperties`] to a
/// printable [`String`] without invoking unsafe `CStr` parsing.
pub(super) fn vk_physical_device_name(props: &vk::PhysicalDeviceProperties) -> String {
    let bytes: Vec<u8> = props
        .device_name
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Fails fast if neither [`vkWaitSemaphores`] (Vulkan 1.2 core) nor [`vkWaitSemaphoresKHR`] is
/// exported for the logical device. WGPU uses one of these for timeline-semaphore fences.
fn verify_device_has_wait_semaphores(
    vk_instance: &ash::Instance,
    device: vk::Device,
) -> Result<(), XrBootstrapError> {
    // SAFETY: `vk_instance` is a valid Vulkan instance and `device` is a `VkDevice` it created;
    // the C-string literal has static lifetime and is NUL-terminated.
    let addr_core = unsafe {
        (vk_instance.fp_v1_0().get_device_proc_addr)(device, c"vkWaitSemaphores".as_ptr())
    };
    // SAFETY: same preconditions as above.
    let addr_khr = unsafe {
        (vk_instance.fp_v1_0().get_device_proc_addr)(device, c"vkWaitSemaphoresKHR".as_ptr())
    };
    if addr_core.is_none() && addr_khr.is_none() {
        return Err(XrBootstrapError::Vulkan(
            "Vulkan device missing vkWaitSemaphores and vkWaitSemaphoresKHR; timeline semaphore support is required for wgpu."
                .into(),
        ));
    }
    Ok(())
}

/// Ash entry, Vulkan instance created via OpenXR, physical device, and wgpu-hal instance flags.
pub(super) struct OpenxrAshVkInstance {
    pub(super) vk_entry: ash::Entry,
    pub(super) vk_instance: ash::Instance,
    pub(super) vk_target_version: u32,
    pub(super) vk_physical_device: vk::PhysicalDevice,
    pub(super) extensions: Vec<&'static std::ffi::CStr>,
    pub(super) flags: wgt::InstanceFlags,
}

/// Creates [`ash::Instance`] and resolves the OpenXR-chosen physical device.
pub(super) fn create_openxr_vulkan_instance(
    xr_instance: &xr::Instance,
    xr_system_id: xr::SystemId,
    gpu_validation_layers: bool,
    reqs: &VulkanGraphicsRequirements,
) -> Result<OpenxrAshVkInstance, XrBootstrapError> {
    // SAFETY: `ash::Entry::load()` dynamically loads the platform's Vulkan loader. Relies on the
    // dynamic linker's standard search path to locate a compatible `libvulkan`.
    let vk_entry =
        unsafe { ash::Entry::load() }.map_err(|e| XrBootstrapError::Vulkan(e.to_string()))?;

    // SAFETY: `vk_entry` was just successfully loaded; calling
    // `vkEnumerateInstanceVersion` via its static function table is sound.
    let instance_api_version = match unsafe { vk_entry.try_enumerate_instance_version() } {
        Ok(Some(v)) => v,
        Ok(None) => vk::API_VERSION_1_0,
        Err(e) => {
            return Err(XrBootstrapError::Vulkan(format!(
                "try_enumerate_instance_version: {e}"
            )));
        }
    };

    let vk_target_version = choose_vulkan_api_version_for_wgpu(instance_api_version, reqs)?;

    let flags = crate::gpu::instance_flags_for_gpu_init(gpu_validation_layers);
    let extensions =
        hal::vulkan::Instance::desired_extensions(&vk_entry, instance_api_version, flags)
            .map_err(|e| XrBootstrapError::Vulkan(format!("desired_extensions: {e}")))?;

    let app_name = c"Renderide";
    let vk_app_info = vk::ApplicationInfo::default()
        .application_name(app_name)
        .application_version(1)
        .engine_name(app_name)
        .engine_version(1)
        .api_version(vk_target_version);

    let extensions_cstr: Vec<_> = extensions.iter().map(|s| s.as_ptr()).collect();
    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&vk_app_info)
        .enabled_extension_names(&extensions_cstr);

    let vk_instance =
        create_vulkan_instance_through_openxr(xr_instance, xr_system_id, &vk_entry, &create_info)?;
    let vk_physical_device =
        openxr_vulkan_physical_device(xr_instance, xr_system_id, &vk_instance)?;

    Ok(OpenxrAshVkInstance {
        vk_entry,
        vk_instance,
        vk_target_version,
        vk_physical_device,
        extensions,
        flags,
    })
}

fn create_vulkan_instance_through_openxr(
    xr_instance: &xr::Instance,
    xr_system_id: xr::SystemId,
    vk_entry: &ash::Entry,
    create_info: &vk::InstanceCreateInfo<'_>,
) -> Result<ash::Instance, XrBootstrapError> {
    // Install the loader entry pointer behind the OpenXR-shaped shim; subsequent OpenXR calls
    // forward through `vk_get_instance_proc_addr_shim` without any fn-pointer transmute.
    let _ = CACHED_VK_GET_INSTANCE_PROC_ADDR.set(vk_entry.static_fn().get_instance_proc_addr);
    // SAFETY: `xr_instance` and `xr_system_id` are valid; `create_info` is fully initialised and
    // borrowed for the call's duration only. The shim receives the OpenXR arguments and forwards
    // to the live `vkGetInstanceProcAddr` captured from `vk_entry`. The resulting raw `VkInstance`
    // handle is wrapped via `ash::Instance::load` so it shares the same loader function table.
    let instance = unsafe {
        let raw = xr_instance
            .create_vulkan_instance(
                xr_system_id,
                vk_get_instance_proc_addr_shim,
                core::ptr::from_ref(create_info).cast(),
            )?
            .map_err(vk::Result::from_raw)?;
        let handle = raw as usize as u64;
        ash::Instance::load(vk_entry.static_fn(), vk::Instance::from_raw(handle))
    };
    Ok(instance)
}

fn openxr_vulkan_physical_device(
    xr_instance: &xr::Instance,
    xr_system_id: xr::SystemId,
    vk_instance: &ash::Instance,
) -> Result<vk::PhysicalDevice, XrBootstrapError> {
    // SAFETY: `xr_instance` is live and `vk_instance` was created from the same
    // `(xr_instance, xr_system_id)` pair through `XR_KHR_vulkan_enable2`.
    let raw = unsafe {
        xr_instance
            .vulkan_graphics_device(xr_system_id, vk_instance.handle().as_raw() as *const c_void)?
    };
    Ok(vk::PhysicalDevice::from_raw(raw as usize as u64))
}

/// Vulkan device creation inputs for OpenXR `create_vulkan_device` + wgpu-hal negotiation.
pub(super) struct VulkanOpenXrDeviceCreateDescriptor<'a> {
    pub(super) xr_instance: &'a xr::Instance,
    pub(super) xr_system_id: xr::SystemId,
    pub(super) vk_instance: &'a ash::Instance,
    pub(super) vk_physical_device: vk::PhysicalDevice,
    pub(super) queue_family_index: u32,
    pub(super) wgpu_exposed: &'a hal::ExposedAdapter<HalVulkan>,
    pub(super) vk_device_properties: &'a vk::PhysicalDeviceProperties,
}

/// Creates the Vulkan logical device through OpenXR using wgpu-hal feature negotiation.
pub(super) fn create_vulkan_logical_device_openxr(
    desc: VulkanOpenXrDeviceCreateDescriptor<'_>,
) -> Result<(wgt::Features, Vec<&'static std::ffi::CStr>, ash::Device), XrBootstrapError> {
    let wgpu_features = negotiate_wgpu_features(desc.wgpu_exposed.features);

    let mut enabled_device_extensions = desc
        .wgpu_exposed
        .adapter
        .required_device_extensions(wgpu_features);

    if desc.vk_device_properties.api_version >= vk::API_VERSION_1_2
        && desc
            .wgpu_exposed
            .adapter
            .physical_device_capabilities()
            .supports_extension(khr_timeline_semaphore::NAME)
        && !enabled_device_extensions
            .iter()
            .copied()
            .any(|e| e == khr_timeline_semaphore::NAME)
    {
        enabled_device_extensions.push(khr_timeline_semaphore::NAME);
    }

    let mut enabled_phd_features = desc
        .wgpu_exposed
        .adapter
        .physical_device_features(&enabled_device_extensions, wgpu_features);

    let family_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(desc.queue_family_index)
        .queue_priorities(&[1.0f32]);
    let str_pointers: Vec<_> = enabled_device_extensions
        .iter()
        .map(|e| e.as_ptr())
        .collect();
    let pre_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&family_info))
        .enabled_extension_names(&str_pointers);
    let device_create_info = enabled_phd_features.add_to_device_create(pre_info);

    let vk_device = create_vulkan_device_through_openxr(&desc, &device_create_info)?;

    Ok((wgpu_features, enabled_device_extensions, vk_device))
}

fn create_vulkan_device_through_openxr(
    desc: &VulkanOpenXrDeviceCreateDescriptor<'_>,
    device_create_info: &vk::DeviceCreateInfo<'_>,
) -> Result<ash::Device, XrBootstrapError> {
    // SAFETY: `desc` contains the matched OpenXR/Vulkan instance, system id, physical device, and
    // device-create chain produced above; `device_create_info` references data that outlives this
    // call. The OpenXR-shaped shim forwards to the cached Vulkan loader entry without transmuting
    // function-pointer types. `ash::Device::load` ties the new `VkDevice` to `desc.vk_instance`'s
    // function table.
    let device = unsafe {
        let raw = desc
            .xr_instance
            .create_vulkan_device(
                desc.xr_system_id,
                vk_get_instance_proc_addr_shim,
                desc.vk_physical_device.as_raw() as *const c_void,
                core::ptr::from_ref(device_create_info).cast(),
            )?
            .map_err(vk::Result::from_raw)?;
        let device_handle = vk::Device::from_raw(raw as usize as u64);
        verify_device_has_wait_semaphores(desc.vk_instance, device_handle)?;
        ash::Device::load(desc.vk_instance.fp_v1_0(), device_handle)
    };
    Ok(device)
}
