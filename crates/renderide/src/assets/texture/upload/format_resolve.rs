//! Maps host [`SetTexture2DFormat`](crate::shared::SetTexture2DFormat) to [`wgpu::TextureFormat`] for new textures.
//!
//! **BC1 / BC3:** When [`wgpu::Features::TEXTURE_COMPRESSION_BC`] is present, these resolve to native
//! BC formats; otherwise CPU [`decode_mip_to_rgba8`](crate::assets::texture::decode::decode_mip_to_rgba8)
//! is used. **BC3nm** tangent-X-in-alpha is handled in WGSL (`normal_decode`).

use crate::shared::{ColorProfile, SetCubemapFormat, SetTexture2DFormat, SetTexture3DFormat};

use super::super::decode::needs_rgba8_decode_before_upload;
use super::super::format::pick_wgpu_storage_format;

/// Decides GPU storage format for a new 2D texture from host [`SetTexture2DFormat`].
///
/// Uses native compressed/uncompressed `wgpu` formats when supported; falls back to RGBA8 when
/// compression features are missing or the host layout needs swizzle ([`needs_rgba8_decode_before_upload`]).
pub fn resolve_texture2d_wgpu_format(
    device: &wgpu::Device,
    fmt: &SetTexture2DFormat,
) -> wgpu::TextureFormat {
    if needs_rgba8_decode_before_upload(device, fmt.format) {
        return rgba8_fallback_format(fmt.profile);
    }
    if let Some(f) = pick_wgpu_storage_format(device, fmt.format, fmt.profile) {
        return f;
    }
    rgba8_fallback_format(fmt.profile)
}

fn rgba8_fallback_format(profile: ColorProfile) -> wgpu::TextureFormat {
    match profile {
        ColorProfile::SRGB | ColorProfile::SRGBAlpha => wgpu::TextureFormat::Rgba8UnormSrgb,
        ColorProfile::Linear => wgpu::TextureFormat::Rgba8Unorm,
    }
}

/// Decides GPU storage format for a new 3D texture from host [`SetTexture3DFormat`].
pub fn resolve_texture3d_wgpu_format(
    device: &wgpu::Device,
    fmt: &SetTexture3DFormat,
) -> wgpu::TextureFormat {
    if needs_rgba8_decode_before_upload(device, fmt.format) {
        return rgba8_fallback_format(fmt.profile);
    }
    if let Some(f) = pick_wgpu_storage_format(device, fmt.format, fmt.profile) {
        return f;
    }
    rgba8_fallback_format(fmt.profile)
}

/// Decides GPU storage format for a new cubemap from host [`SetCubemapFormat`].
pub fn resolve_cubemap_wgpu_format(
    device: &wgpu::Device,
    fmt: &SetCubemapFormat,
) -> wgpu::TextureFormat {
    if needs_rgba8_decode_before_upload(device, fmt.format) {
        return rgba8_fallback_format(fmt.profile);
    }
    if let Some(f) = pick_wgpu_storage_format(device, fmt.format, fmt.profile) {
        return f;
    }
    rgba8_fallback_format(fmt.profile)
}
