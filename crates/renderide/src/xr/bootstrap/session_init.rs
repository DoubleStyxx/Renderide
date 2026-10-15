//! OpenXR session creation, reference space, and controller input wiring.

use ash::vk::{self, Handle};
use openxr as xr;

use super::super::debug_utils::OpenxrDebugUtilsMessenger;
use super::super::input::{ManifestError, OpenxrInput, ProfileExtensionGates, load_manifest};
use super::super::session::{XrSessionState, XrSessionStateDescriptor};
use super::types::XrBootstrapError;

/// OpenXR session, reference space, optional controller actions, and [`XrSessionState`].
pub(super) struct OpenXrSessionBootstrapDescriptor<'a> {
    pub(super) xr_instance: xr::Instance,
    pub(super) openxr_debug_messenger: Option<OpenxrDebugUtilsMessenger>,
    pub(super) environment_blend_mode: xr::EnvironmentBlendMode,
    pub(super) xr_system_id: xr::SystemId,
    pub(super) vk_instance: &'a ash::Instance,
    pub(super) vk_physical_device: vk::PhysicalDevice,
    pub(super) vk_device: &'a ash::Device,
    pub(super) queue_family_index: u32,
    pub(super) profile_gates: ProfileExtensionGates,
}

/// Creates the OpenXR session, the STAGE reference space, the controller-action subsystem, and
/// wraps everything in [`XrSessionState`].
pub(super) fn openxr_session_state_and_input(
    desc: OpenXrSessionBootstrapDescriptor<'_>,
) -> Result<(XrSessionState, Option<OpenxrInput>), XrBootstrapError> {
    // SAFETY: `desc.vk_instance`/`vk_physical_device`/`vk_device` form a matched OpenXR-negotiated
    // Vulkan chain from `create_vulkan_logical_device_openxr`; `queue_family_index` was chosen
    // above to be a graphics-capable family on the selected device.
    let (session, frame_wait, frame_stream) = unsafe {
        desc.xr_instance.create_session::<xr::Vulkan>(
            desc.xr_system_id,
            &xr::vulkan::SessionCreateInfo {
                instance: desc.vk_instance.handle().as_raw() as xr::sys::platform::VkInstance,
                physical_device: desc.vk_physical_device.as_raw()
                    as xr::sys::platform::VkPhysicalDevice,
                device: desc.vk_device.handle().as_raw() as xr::sys::platform::VkDevice,
                queue_family_index: desc.queue_family_index,
                queue_index: 0,
            },
        )
    }
    .map_err(XrBootstrapError::OpenXr)?;
    let stage: xr::Space = session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .map_err(XrBootstrapError::OpenXr)?;

    let openxr_input = match load_manifest() {
        Ok((manifest, location)) => {
            logger::info!(
                "Loaded OpenXR action manifest from {} ({} profile(s))",
                location.root.display(),
                manifest.profiles.len()
            );
            match OpenxrInput::new(&desc.xr_instance, &session, &desc.profile_gates, &manifest) {
                Ok(i) => Some(i),
                Err(e) => {
                    logger::warn!(
                        "OpenXR controller input unavailable (continuing without actions): {e}"
                    );
                    None
                }
            }
        }
        Err(ManifestError::ActionsManifestMissing { ref searched }) => {
            logger::warn!(
                "OpenXR action manifest not found; searched: {}",
                searched.join(", ")
            );
            None
        }
        Err(e) => {
            logger::warn!("OpenXR action manifest load failed: {e}");
            None
        }
    };
    let xr_session = XrSessionState::new(XrSessionStateDescriptor {
        xr_instance: desc.xr_instance,
        openxr_debug_messenger: desc.openxr_debug_messenger,
        environment_blend_mode: desc.environment_blend_mode,
        session,
        frame_wait,
        frame_stream,
        stage,
    });
    Ok((xr_session, openxr_input))
}
