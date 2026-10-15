//! Texture key validation against device limits.

use super::policy::texture_key_dims;
use super::{TextureKey, TransientPoolError};

/// Returns [`TransientPoolError::TextureExceedsLimits`] when `key` would exceed device limits.
pub(super) fn validate_texture_key(
    limits: &crate::gpu::GpuLimits,
    key: TextureKey,
    label: &'static str,
    usage: wgpu::TextureUsages,
) -> Result<(), TransientPoolError> {
    let (width, height, layers) = texture_key_dims(key);
    let dims_fit = match key.dimension {
        wgpu::TextureDimension::D3 => limits.texture_3d_fits(width, height, layers),
        _ => limits.texture_2d_fits(width, height) && limits.array_layers_fit(layers),
    };
    let requested_mips = key.mip_levels.max(1);
    let mips_fit = requested_mips <= 16 && requested_mips <= max_mip_levels_for_texture_key(key);
    if !dims_fit || !mips_fit {
        return Err(TransientPoolError::TextureExceedsLimits {
            label,
            width,
            height,
            layers,
            mip_levels: key.mip_levels,
        });
    }
    if !limits.texture_usage_supported(key.format, usage) {
        return Err(TransientPoolError::TextureUnsupportedUsage {
            label,
            format: key.format,
            usage,
        });
    }
    let sample_count = key.sample_count.max(1);
    let multisample_shape_fit = sample_count <= 1
        || (key.mip_levels.max(1) == 1
            && key.dimension == wgpu::TextureDimension::D2
            && usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            && !usage.contains(wgpu::TextureUsages::STORAGE_BINDING)
            && (layers <= 1 || limits.supports_multisample_array()));
    if !multisample_shape_fit || !limits.texture_sample_count_supported(key.format, sample_count) {
        return Err(TransientPoolError::TextureUnsupportedSampleCount {
            label,
            format: key.format,
            sample_count,
        });
    }
    Ok(())
}

fn max_mip_levels_for_texture_key(key: TextureKey) -> u32 {
    let (width, height, depth) = texture_key_dims(key);
    let max_axis = match key.dimension {
        wgpu::TextureDimension::D1 => width,
        wgpu::TextureDimension::D2 => width.max(height),
        wgpu::TextureDimension::D3 => width.max(height).max(depth),
    };
    u32::BITS - max_axis.max(1).leading_zeros()
}
