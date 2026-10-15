//! Linear / clamp sampler factory shared by every IBL bake input.

/// Creates a linear filtered, clamp-to-edge sampler matching the convention used by every
/// IBL mip-0 producer, convolve, and downsample pass.
pub(super) fn create_linear_clamp_sampler(
    device: &wgpu::Device,
    label: &'static str,
) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}
