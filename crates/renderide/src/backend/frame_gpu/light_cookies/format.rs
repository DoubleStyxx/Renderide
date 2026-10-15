use crate::gpu::{
    GpuLimits, LIGHT_COOKIE_WRAP_MODE_CLAMP, LIGHT_COOKIE_WRAP_MODE_MASK,
    LIGHT_COOKIE_WRAP_MODE_MIRROR, LIGHT_COOKIE_WRAP_MODE_MIRROR_ONCE,
    LIGHT_COOKIE_WRAP_MODE_REPEAT, LIGHT_COOKIE_WRAP_U_SHIFT, LIGHT_COOKIE_WRAP_V_SHIFT,
};
use crate::gpu_pools::{GpuCubemap, SamplerState};
use crate::shared::{TextureFormat, TextureWrapMode};

/// Scalar storage format used for light-cookie atlas layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LightCookieAtlasFormat {
    /// Signed half-float scalar storage.
    R16Float,
    /// Signed half-float RGBA storage used when scalar render targets are unavailable.
    Rgba16Float,
    /// Unsigned normalized scalar fallback.
    R8Unorm,
}

impl LightCookieAtlasFormat {
    /// Returns the wgpu texture format.
    pub(super) const fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::R16Float => wgpu::TextureFormat::R16Float,
            Self::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
            Self::R8Unorm => wgpu::TextureFormat::R8Unorm,
        }
    }

    /// Returns bytes per texel for CPU fallback-layer writes.
    pub(super) const fn bytes_per_texel(self) -> u32 {
        match self {
            Self::R16Float => 2,
            Self::Rgba16Float => 8,
            Self::R8Unorm => 1,
        }
    }
}

/// Channel read from a source texture into the scalar cookie atlas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LightCookieSourceChannel {
    /// Source red channel.
    Red,
    /// Source alpha channel.
    Alpha,
}

/// Sampler/layout mode used for source cookie blits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LightCookieSourceSampling {
    /// Filterable source texture and filtering sampler.
    Filtering,
    /// Unfilterable float source texture and non-filtering sampler.
    NonFiltering,
}

/// Resolved source texture view plus sampling policy.
#[derive(Clone, Copy)]
pub(super) struct LightCookieSource<'a> {
    /// Source texture view.
    pub(super) view: &'a wgpu::TextureView,
    /// Source channel copied into the scalar atlas.
    pub(super) channel: LightCookieSourceChannel,
    /// Source sampler/layout mode.
    pub(super) sampling: LightCookieSourceSampling,
}

/// Resolved point-cookie source cubemap plus sampling policy.
#[derive(Clone, Copy)]
pub(super) struct LightCookiePointSource<'a> {
    /// Source cubemap resource.
    pub(super) cubemap: &'a GpuCubemap,
    /// Source channel copied into the scalar atlas.
    pub(super) channel: LightCookieSourceChannel,
    /// Source sampler/layout mode.
    pub(super) sampling: LightCookieSourceSampling,
}

/// Selects the scalar texture format used by light-cookie atlases.
pub(super) fn select_light_cookie_atlas_format(limits: &GpuLimits) -> LightCookieAtlasFormat {
    if light_cookie_atlas_format_supported(limits, LightCookieAtlasFormat::R16Float) {
        return LightCookieAtlasFormat::R16Float;
    }
    if light_cookie_atlas_format_supported(limits, LightCookieAtlasFormat::Rgba16Float) {
        logger::warn!(
            "signed scalar light-cookie atlas format R16Float is unavailable; using Rgba16Float HDR cookie storage"
        );
        return LightCookieAtlasFormat::Rgba16Float;
    }
    logger::warn!(
        "HDR light-cookie atlas formats are unavailable; falling back to unsigned R8Unorm cookies"
    );
    LightCookieAtlasFormat::R8Unorm
}

/// Returns whether the device can use `format` for sampled scalar light-cookie atlases.
pub(super) fn light_cookie_atlas_format_supported(
    limits: &GpuLimits,
    format: LightCookieAtlasFormat,
) -> bool {
    let features = limits.texture_format_features(format.wgpu());
    features.allowed_usages.contains(
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST,
    ) && features
        .flags
        .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
}

/// Returns the source channel that carries the scalar cookie value.
pub(super) fn source_channel_for_host_format(format: TextureFormat) -> LightCookieSourceChannel {
    match format {
        TextureFormat::ARGB32
        | TextureFormat::RGBA32
        | TextureFormat::BGRA32
        | TextureFormat::RGBAHalf
        | TextureFormat::ARGBHalf
        | TextureFormat::RGBAFloat
        | TextureFormat::ARGBFloat
        | TextureFormat::BC2
        | TextureFormat::BC3
        | TextureFormat::BC7
        | TextureFormat::ETC2RGBA1
        | TextureFormat::ETC2RGBA8
        | TextureFormat::ASTC4x4
        | TextureFormat::ASTC5x5
        | TextureFormat::ASTC6x6
        | TextureFormat::ASTC8x8
        | TextureFormat::ASTC10x10
        | TextureFormat::ASTC12x12 => LightCookieSourceChannel::Alpha,
        TextureFormat::Unknown
        | TextureFormat::Alpha8
        | TextureFormat::R8
        | TextureFormat::RGB24
        | TextureFormat::RGB565
        | TextureFormat::BGR565
        | TextureFormat::RHalf
        | TextureFormat::RGHalf
        | TextureFormat::RFloat
        | TextureFormat::RGFloat
        | TextureFormat::BC1
        | TextureFormat::BC4
        | TextureFormat::BC5
        | TextureFormat::BC6H
        | TextureFormat::ETC2RGB => LightCookieSourceChannel::Red,
    }
}

/// Packs U/V sampler wrap modes for 2D cookie shader addressing.
pub(super) fn light_cookie_wrap_bits(sampler: &SamplerState) -> u32 {
    pack_wrap_mode(sampler.wrap_u, LIGHT_COOKIE_WRAP_U_SHIFT)
        | pack_wrap_mode(sampler.wrap_v, LIGHT_COOKIE_WRAP_V_SHIFT)
}

/// Packs one wrap mode into the shader metadata bitfield.
fn pack_wrap_mode(mode: TextureWrapMode, shift: u32) -> u32 {
    (wrap_mode_bits(mode) & LIGHT_COOKIE_WRAP_MODE_MASK) << shift
}

/// Converts a host texture wrap mode to the compact shader enum.
fn wrap_mode_bits(mode: TextureWrapMode) -> u32 {
    match mode {
        TextureWrapMode::Repeat => LIGHT_COOKIE_WRAP_MODE_REPEAT,
        TextureWrapMode::Clamp => LIGHT_COOKIE_WRAP_MODE_CLAMP,
        TextureWrapMode::Mirror => LIGHT_COOKIE_WRAP_MODE_MIRROR,
        TextureWrapMode::MirrorOnce => LIGHT_COOKIE_WRAP_MODE_MIRROR_ONCE,
    }
}

/// Returns the source sampling mode supported by `format`.
pub(super) fn source_sampling_for_limits(
    limits: &GpuLimits,
    format: wgpu::TextureFormat,
) -> Option<LightCookieSourceSampling> {
    let features = limits.texture_format_features(format);
    if !features
        .allowed_usages
        .contains(wgpu::TextureUsages::TEXTURE_BINDING)
    {
        return None;
    }
    if features
        .flags
        .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
    {
        return Some(LightCookieSourceSampling::Filtering);
    }
    Some(LightCookieSourceSampling::NonFiltering)
}
