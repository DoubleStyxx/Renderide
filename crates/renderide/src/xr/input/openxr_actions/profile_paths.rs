//! Resolved interaction profile paths used by per-frame controller profile detection.

use openxr as xr;

use super::super::manifest::Manifest;

/// Interaction profile paths resolved from the manifest for per-frame profile detection.
///
/// Missing entries remain `xr::Path::NULL` so path comparisons in
/// [`crate::xr::input::openxr_input::OpenxrInput::detect_profile`] safely return a non-match when a
/// specific profile is not in the manifest.
#[derive(Default)]
pub(in crate::xr::input) struct ResolvedProfilePaths {
    /// `/interaction_profiles/oculus/touch_controller`
    pub(in crate::xr::input) oculus_touch: xr::Path,
    /// `/interaction_profiles/valve/index_controller`
    pub(in crate::xr::input) valve_index: xr::Path,
    /// `/interaction_profiles/htc/vive_controller`
    pub(in crate::xr::input) htc_vive: xr::Path,
    /// `/interaction_profiles/microsoft/motion_controller`
    pub(in crate::xr::input) microsoft_motion: xr::Path,
    /// `/interaction_profiles/khr/generic_controller`
    pub(in crate::xr::input) generic_controller: xr::Path,
    /// `/interaction_profiles/khr/simple_controller`
    pub(in crate::xr::input) simple_controller: xr::Path,
    /// `/interaction_profiles/bytedance/pico4_controller`
    pub(in crate::xr::input) pico4_controller: xr::Path,
    /// `/interaction_profiles/bytedance/pico_neo3_controller`
    pub(in crate::xr::input) pico_neo3_controller: xr::Path,
    /// `/interaction_profiles/hp/mixed_reality_controller`
    pub(in crate::xr::input) hp_reverb_g2: xr::Path,
    /// `/interaction_profiles/samsung/odyssey_controller`
    pub(in crate::xr::input) samsung_odyssey: xr::Path,
    /// `/interaction_profiles/htc/vive_cosmos_controller`
    pub(in crate::xr::input) htc_vive_cosmos: xr::Path,
    /// `/interaction_profiles/htc/vive_focus3_controller`
    pub(in crate::xr::input) htc_vive_focus3: xr::Path,
    /// `/interaction_profiles/facebook/touch_controller_pro`
    pub(in crate::xr::input) meta_touch_pro: xr::Path,
    /// `/interaction_profiles/meta/touch_controller_plus`
    pub(in crate::xr::input) meta_touch_plus: xr::Path,
}

impl ResolvedProfilePaths {
    /// Resolves every profile path listed in the manifest through the OpenXR instance and stores
    /// the handles in the corresponding well-known field. Unknown profile paths are ignored; new
    /// profiles that need runtime detection must also be wired here plus in `detect_profile`.
    pub(in crate::xr::input) fn from_manifest(
        instance: &xr::Instance,
        manifest: &Manifest,
    ) -> Result<Self, xr::sys::Result> {
        let mut out = Self::default();
        for profile in &manifest.profiles {
            let path = instance.string_to_path(&profile.profile)?;
            match profile.profile.as_str() {
                "/interaction_profiles/oculus/touch_controller" => out.oculus_touch = path,
                "/interaction_profiles/valve/index_controller" => out.valve_index = path,
                "/interaction_profiles/htc/vive_controller" => out.htc_vive = path,
                "/interaction_profiles/microsoft/motion_controller" => {
                    out.microsoft_motion = path;
                }
                "/interaction_profiles/khr/generic_controller" => out.generic_controller = path,
                "/interaction_profiles/khr/simple_controller" => out.simple_controller = path,
                "/interaction_profiles/bytedance/pico4_controller" => out.pico4_controller = path,
                "/interaction_profiles/bytedance/pico_neo3_controller" => {
                    out.pico_neo3_controller = path;
                }
                "/interaction_profiles/hp/mixed_reality_controller" => out.hp_reverb_g2 = path,
                "/interaction_profiles/samsung/odyssey_controller" => {
                    out.samsung_odyssey = path;
                }
                "/interaction_profiles/htc/vive_cosmos_controller" => {
                    out.htc_vive_cosmos = path;
                }
                "/interaction_profiles/htc/vive_focus3_controller" => {
                    out.htc_vive_focus3 = path;
                }
                "/interaction_profiles/facebook/touch_controller_pro" => {
                    out.meta_touch_pro = path;
                }
                "/interaction_profiles/meta/touch_controller_plus" => {
                    out.meta_touch_plus = path;
                }
                _ => {}
            }
        }
        Ok(out)
    }
}
