//! Reserved frame-global skybox specular sampling parameters.
//!
//! Pure data: packs sampling state ([`SkyboxSpecularUniformParams`]) into the trailing
//! `vec4<f32>` slot of [`crate::gpu::frame_globals::FrameGpuUniforms`].
//!
//! Direct skybox specular lighting is disabled. Skybox-authored specular IBL is supplied through
//! reflection probes, so this slot is preserved only for uniform layout stability.

/// Frame-global indicator for the disabled direct skybox specular source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkyboxSpecularSourceKind {
    /// No direct skybox specular cube is bound.
    Disabled,
    /// Reserved cubemap tag retained for frame-uniform compatibility.
    #[cfg(test)]
    Cubemap,
}

impl SkyboxSpecularSourceKind {
    /// Numeric tag consumed by WGSL.
    pub const fn to_f32(self) -> f32 {
        match self {
            Self::Disabled => 0.0,
            #[cfg(test)]
            Self::Cubemap => 1.0,
        }
    }
}

/// CPU-side parameters packed into
/// [`crate::gpu::frame_globals::FrameGpuUniforms::skybox_specular`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyboxSpecularUniformParams {
    /// Highest resident source mip available for the disabled direct skybox path.
    pub max_lod: f32,
    /// Whether the disabled direct skybox path has a source bound.
    pub enabled: bool,
    /// Reserved source kind (cube or disabled).
    pub source_kind: SkyboxSpecularSourceKind,
}

impl SkyboxSpecularUniformParams {
    /// Disabled direct skybox specular state.
    pub const fn disabled() -> Self {
        Self {
            max_lod: 0.0,
            enabled: false,
            source_kind: SkyboxSpecularSourceKind::Disabled,
        }
    }

    /// Builds enabled parameters from a resident cubemap mip count.
    #[cfg(test)]
    pub fn from_cubemap_resident_mips(mip_levels_resident: u32) -> Self {
        Self {
            max_lod: mip_levels_resident.saturating_sub(1) as f32,
            enabled: mip_levels_resident > 0,
            source_kind: if mip_levels_resident > 0 {
                SkyboxSpecularSourceKind::Cubemap
            } else {
                SkyboxSpecularSourceKind::Disabled
            },
        }
    }

    /// Packs parameters into the `vec4<f32>` layout consumed by WGSL.
    ///
    /// Layout: `.x` max LOD, `.y` enabled flag, `.z` source kind tag, `.w` reserved.
    pub fn to_vec4(self) -> [f32; 4] {
        [
            self.max_lod,
            if self.enabled { 1.0 } else { 0.0 },
            self.source_kind.to_f32(),
            0.0,
        ]
    }
}
