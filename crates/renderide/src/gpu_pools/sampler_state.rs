//! Unified texture sampler state shared by every resident texture pool.
//!
//! `SamplerState` collapses the previous per-kind sampler structs into one product type with
//! `wrap_u/v/w`. 2D bind paths ignore `wrap_w`; cubemap bind paths force `wrap_u/v` to repeat
//! through [`Self::from_cubemap_props`]. Builders convert from each host property struct so
//! pool-side construction stays a single function call.

use renderide_shared::VideoTextureProperties;

use crate::shared::{
    SetCubemapProperties, SetRenderTextureFormat, SetTexture2DProperties, SetTexture3DProperties,
    TextureFilterMode, TextureWrapMode,
};

/// Sampler-related fields mirrored from host texture properties.
///
/// `wrap_w` is always present so a single struct covers 2D, 3D, cubemap, render-texture, and
/// video-texture sampling. Pools that do not author a third axis fall back to
/// [`TextureWrapMode::default`].
#[derive(Clone, Debug)]
pub struct SamplerState {
    /// Min/mag filter from host.
    pub filter_mode: TextureFilterMode,
    /// Anisotropic filtering level (clamped to non-negative).
    pub aniso_level: i32,
    /// U address mode.
    pub wrap_u: TextureWrapMode,
    /// V address mode.
    pub wrap_v: TextureWrapMode,
    /// W address mode (only consumed by 3D bind paths; defaults to [`TextureWrapMode::default`]).
    pub wrap_w: TextureWrapMode,
    /// Mip bias applied when sampling.
    pub mipmap_bias: f32,
}

impl Default for SamplerState {
    fn default() -> Self {
        Self {
            filter_mode: TextureFilterMode::default(),
            aniso_level: 1,
            wrap_u: TextureWrapMode::default(),
            wrap_v: TextureWrapMode::default(),
            wrap_w: TextureWrapMode::default(),
            mipmap_bias: 0.0,
        }
    }
}

impl SamplerState {
    /// Builds sampler state from optional [`SetTexture2DProperties`].
    pub fn from_texture2d_props(props: Option<&SetTexture2DProperties>) -> Self {
        let Some(p) = props else {
            return Self::default();
        };
        Self {
            filter_mode: p.filter_mode,
            aniso_level: p.aniso_level.max(0),
            wrap_u: p.wrap_u,
            wrap_v: p.wrap_v,
            wrap_w: TextureWrapMode::default(),
            mipmap_bias: p.mipmap_bias,
        }
    }

    /// Builds sampler state from optional [`SetTexture3DProperties`]; `mipmap_bias` is zero
    /// because the host 3D properties wire format does not carry it.
    pub fn from_texture3d_props(props: Option<&SetTexture3DProperties>) -> Self {
        let Some(p) = props else {
            return Self::default();
        };
        Self {
            filter_mode: p.filter_mode,
            aniso_level: p.aniso_level.max(0),
            wrap_u: p.wrap_u,
            wrap_v: p.wrap_v,
            wrap_w: p.wrap_w,
            mipmap_bias: 0.0,
        }
    }

    /// Builds sampler state from optional [`SetCubemapProperties`]. Cubemaps always sample
    /// with [`TextureWrapMode::Repeat`] on both axes since the host properties do not carry
    /// wrap modes.
    pub fn from_cubemap_props(props: Option<&SetCubemapProperties>) -> Self {
        let Some(p) = props else {
            return Self {
                wrap_u: TextureWrapMode::Repeat,
                wrap_v: TextureWrapMode::Repeat,
                ..Self::default()
            };
        };
        Self {
            filter_mode: p.filter_mode,
            aniso_level: p.aniso_level.max(0),
            wrap_u: TextureWrapMode::Repeat,
            wrap_v: TextureWrapMode::Repeat,
            wrap_w: TextureWrapMode::default(),
            mipmap_bias: p.mipmap_bias,
        }
    }

    /// Builds sampler state from a host [`SetRenderTextureFormat`]. Negative anisotropy is
    /// clamped to zero before sampler creation.
    pub fn from_render_texture_format(fmt: &SetRenderTextureFormat) -> Self {
        Self {
            filter_mode: fmt.filter_mode,
            aniso_level: fmt.aniso_level.max(0),
            wrap_u: fmt.wrap_u,
            wrap_v: fmt.wrap_v,
            wrap_w: TextureWrapMode::default(),
            mipmap_bias: 0.0,
        }
    }

    /// Builds sampler state from host [`VideoTextureProperties`]; negative anisotropy clamped.
    pub fn from_video_props(props: &VideoTextureProperties) -> Self {
        Self {
            filter_mode: props.filter_mode,
            aniso_level: props.aniso_level.max(0),
            wrap_u: props.wrap_u,
            wrap_v: props.wrap_v,
            wrap_w: TextureWrapMode::default(),
            mipmap_bias: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SamplerState;
    use crate::gpu_pools::test_support::render_texture_format;
    use crate::shared::{TextureFilterMode, TextureWrapMode};
    use renderide_shared::VideoTextureProperties;

    #[test]
    fn render_texture_preserves_host_wrap_modes() {
        let fmt = render_texture_format(TextureWrapMode::Mirror, TextureWrapMode::Clamp);
        let s = SamplerState::from_render_texture_format(&fmt);
        assert_eq!(s.wrap_u, TextureWrapMode::Mirror);
        assert_eq!(s.wrap_v, TextureWrapMode::Clamp);
    }

    #[test]
    fn render_texture_clamps_negative_anisotropy() {
        let mut fmt = render_texture_format(TextureWrapMode::Clamp, TextureWrapMode::Clamp);
        fmt.aniso_level = -4;
        let s = SamplerState::from_render_texture_format(&fmt);
        assert_eq!(s.aniso_level, 0);
    }

    #[test]
    fn video_props_clamp_negative_anisotropy() {
        let props = VideoTextureProperties {
            filter_mode: TextureFilterMode::Anisotropic,
            aniso_level: -4,
            wrap_u: TextureWrapMode::Mirror,
            wrap_v: TextureWrapMode::Clamp,
            asset_id: 12,
        };
        let s = SamplerState::from_video_props(&props);
        assert_eq!(s.filter_mode, TextureFilterMode::Anisotropic);
        assert_eq!(s.aniso_level, 0);
        assert_eq!(s.wrap_u, TextureWrapMode::Mirror);
        assert_eq!(s.wrap_v, TextureWrapMode::Clamp);
        assert_eq!(s.mipmap_bias, 0.0);
    }

    #[test]
    fn cubemap_forces_repeat_wrap() {
        let s = SamplerState::from_cubemap_props(None);
        assert_eq!(s.wrap_u, TextureWrapMode::Repeat);
        assert_eq!(s.wrap_v, TextureWrapMode::Repeat);
    }
}
