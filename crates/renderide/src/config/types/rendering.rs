//! Rendering toggles, MSAA, presentation mode, and scene-color format. Persisted as
//! `[rendering]`. Each enum lives in its own submodule and is generated through the shared
//! [`crate::labeled_enum`] macro so adding a new mode is a single declaration with the
//! canonical persist string, label, and any aliases.

mod graphics_api;
mod msaa;
mod presentation_mode;
mod scene_color;
mod vr_swapchain_gpu_wait;

pub use graphics_api::GraphicsApiSetting;
pub use msaa::MsaaSampleCount;
pub use presentation_mode::PresentationModeSetting;
pub use scene_color::SceneColorFormat;
pub use vr_swapchain_gpu_wait::{
    ResolvedVrSwapchainGpuWait, VrSwapchainGpuWait, VrSwapchainGpuWaitQuery,
};

use serde::{Deserialize, Serialize};

/// Rendering toggles and scalars. Persisted as `[rendering]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderingSettings {
    /// Swapchain presentation mode applied live without restart through
    /// [`crate::gpu::GpuContext::set_present_mode`].
    pub presentation_mode: PresentationModeSetting,
    /// Startup graphics API preference. `Auto` preserves wgpu's default backend discovery; an
    /// explicit API constrains the first adapter-selection attempt and falls back to automatic
    /// selection if that API has no compatible adapter. Applied only when the GPU stack is created,
    /// so changes require a renderer restart.
    #[serde(rename = "graphics_api", default)]
    pub graphics_api: GraphicsApiSetting,
    /// Whether the OpenXR driver waits for the eye copy before releasing the swapchain image.
    ///
    /// Resolved once at OpenXR session start. `always` is the default because the WiVRn/Monado
    /// on RADV flash is a seizure risk. Changes require a renderer restart.
    #[serde(default)]
    pub vr_swapchain_gpu_wait: VrSwapchainGpuWait,
    /// Wall-clock budget per frame for cooperative mesh/texture integration
    /// ([`crate::runtime::RendererRuntime::run_asset_integration`]), in milliseconds.
    #[serde(rename = "asset_integration_budget_ms")]
    pub asset_integration_budget_ms: u32,
    /// Extra post-main budget for dynamic buffer / particle integration, in milliseconds.
    #[serde(rename = "asset_particle_integration_budget_ms")]
    pub asset_particle_integration_budget_ms: u32,
    /// Multisample anti-aliasing for forward rendering. Effective sample count is clamped to the
    /// GPU's supported maximum for the target format. Main-window, VR, CameraRenderTask, and host
    /// RenderTexture camera outputs use this tier; reflection-probe utility captures stay 1x.
    pub msaa: MsaaSampleCount,
    /// Format for the **scene-color** HDR target the forward pass renders into before
    /// [`crate::passes::SceneColorComposePass`] writes the displayable target.
    ///
    /// This is intermediate precision/range (e.g. [`SceneColorFormat::Rgba16Float`]), not the OS
    /// swapchain HDR mode.
    #[serde(rename = "scene_color_format")]
    pub scene_color_format: SceneColorFormat,
    /// Opt-in physically based light upload.
    ///
    /// When `true`, packing clamps each channel of `color * intensity` so the product is
    /// non-negative. That removes darkening from a negative intensity or a negative color
    /// channel. A negative color times a negative intensity stays, because that product is
    /// positive. A negative intensity with no negative color left is stored as zero. Range,
    /// shadows, and the other light fields stay as the host authored them.
    /// The default is `false` so Resonite parity stays intact: Renderite's Unity renderer
    /// keeps negative intensity and negative color, and those lights darken the scene. A
    /// missing `physically_based` key deserializes to `false`, so existing config files stay
    /// on that parity path.
    #[serde(default)]
    pub physically_based: bool,
}

impl Default for RenderingSettings {
    fn default() -> Self {
        Self {
            presentation_mode: PresentationModeSetting::default(),
            graphics_api: GraphicsApiSetting::default(),
            vr_swapchain_gpu_wait: VrSwapchainGpuWait::default(),
            asset_integration_budget_ms: 2,
            asset_particle_integration_budget_ms: 4,
            msaa: MsaaSampleCount::default(),
            scene_color_format: SceneColorFormat::default(),
            physically_based: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RenderingSettings;
    use crate::config::types::RendererSettings;

    #[test]
    fn physically_based_defaults_to_false() {
        assert!(!RenderingSettings::default().physically_based);
        assert!(!RendererSettings::default().rendering.physically_based);
    }

    /// A config written before this key existed must stay on the Resonite parity path.
    #[test]
    fn missing_physically_based_key_deserializes_to_false() {
        let settings: RendererSettings =
            toml::from_str("[rendering]\nmsaa = \"x4\"\n").expect("deserialize");

        assert!(!settings.rendering.physically_based);
    }

    #[test]
    fn physically_based_round_trips_through_toml() {
        let mut settings = RendererSettings::default();
        settings.rendering.physically_based = true;

        let text = toml::to_string(&settings).expect("serialize");
        let decoded: RendererSettings = toml::from_str(&text).expect("deserialize");

        assert!(
            text.contains("physically_based = true"),
            "serialized settings missing the flag:\n{text}"
        );
        assert!(decoded.rendering.physically_based);
    }
}
