//! Shared GPU texture allocation helpers for resident texture pools.

use std::fmt::Display;
use std::sync::Arc;

/// View configuration for a newly allocated sampled texture.
pub(crate) struct TextureViewInit<'a> {
    /// Optional debug label for the view.
    pub label: Option<&'a str>,
    /// Optional explicit view dimension.
    pub dimension: Option<wgpu::TextureViewDimension>,
}

/// Allocation descriptor for resident sampled textures.
pub(crate) struct SampledTextureAllocation<'a> {
    /// Debug label for the texture.
    pub label: &'a str,
    /// Texture extent.
    pub size: wgpu::Extent3d,
    /// Number of mip levels.
    pub mip_level_count: u32,
    /// Texture dimension.
    pub dimension: wgpu::TextureDimension,
    /// Storage format.
    pub format: wgpu::TextureFormat,
    /// Initial default view shape.
    pub view: TextureViewInit<'a>,
}

/// Creates resident texture storage and its default binding view.
pub(crate) fn create_sampled_copy_dst_texture(
    device: &wgpu::Device,
    desc: SampledTextureAllocation<'_>,
) -> (Arc<wgpu::Texture>, Arc<wgpu::TextureView>) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(desc.label),
        size: desc.size,
        mip_level_count: desc.mip_level_count,
        sample_count: 1,
        dimension: desc.dimension,
        format: desc.format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: desc.view.label,
        dimension: desc.view.dimension,
        ..Default::default()
    });
    crate::profiling::note_resource_churn!(TextureView, "gpu_pools::sampled_copy_dst_texture_view");
    (Arc::new(texture), Arc::new(view))
}

/// Validates that every axis in `axes` is within `max_dim`. Emits one
/// `logger::warn!` and returns `false` on the first violation. `kind` is the
/// asset-kind label (e.g. `"texture"`, `"cubemap"`, `"texture3d"`),
/// `extent_label` is the noun used in the warning preamble (`"format size"`
/// for textures, `"face size"` for cubemaps), `dims` is the formatted
/// dimensions for the warning (e.g. `"128x64"`, `"32"`, `"16x16x16"`), and
/// `max_dim_name` is the limit name reported (`"max_texture_dimension_2d"` or
/// `"max_texture_dimension_3d"`).
pub(crate) fn validate_texture_extent(
    asset_id: i32,
    kind: &str,
    extent_label: &str,
    dims: &dyn Display,
    axes: &[u32],
    max_dim: u32,
    max_dim_name: &str,
) -> bool {
    for &axis in axes {
        if axis > max_dim {
            logger::warn!(
                "{kind} {asset_id}: {extent_label} {dims} exceeds {max_dim_name} ({max_dim}); GPU texture not created"
            );
            return false;
        }
    }
    true
}

/// Clamps `requested` to `legal`, emitting a `logger::warn!` once when
/// clamping kicks in. `kind` is the asset-kind label and `dim_phrase` is the
/// formatted dimensions used in the warning (e.g. `"128x64"`, `"face size 32"`,
/// `"16x16x16"`).
pub(crate) fn clamp_texture_mip_count(
    asset_id: i32,
    kind: &str,
    dim_phrase: &dyn Display,
    requested: u32,
    legal: u32,
) -> u32 {
    let clamped = requested.min(legal);
    if requested > legal {
        logger::warn!(
            "{kind} {asset_id}: host requested {requested} mips for {dim_phrase}; clamping to legal mip count {legal}"
        );
    }
    clamped
}
