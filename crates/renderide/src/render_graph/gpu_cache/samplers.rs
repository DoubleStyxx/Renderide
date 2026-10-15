//! Sampler / view / uniform-buffer helpers shared by render-graph passes.

/// Creates a linear clamp sampler for fullscreen texture sampling.
pub(crate) fn create_linear_clamp_sampler(device: &wgpu::Device, label: &str) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(label),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// Creates a lazily cached uniform buffer descriptor with the renderer's standard flags.
pub(crate) fn create_uniform_buffer(
    device: &wgpu::Device,
    label: &'static str,
    size: u64,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    crate::profiling::note_resource_churn!(Buffer, "render_graph::gpu_cache_uniform_buffer");
    buffer
}

/// Number of array layers to expose when sampling a texture as `texture_2d_array`.
pub(crate) fn d2_array_layer_count(texture: &wgpu::Texture, multiview_stereo: bool) -> u32 {
    let layers_in_texture = texture.size().depth_or_array_layers.max(1);
    if multiview_stereo {
        2.min(layers_in_texture)
    } else {
        1
    }
}

/// Creates a sampled `D2Array` view over one or two layers of `texture`.
pub(crate) fn create_d2_array_view(
    texture: &wgpu::Texture,
    label: &str,
    multiview_stereo: bool,
) -> wgpu::TextureView {
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(label),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        array_layer_count: Some(d2_array_layer_count(texture, multiview_stereo)),
        ..Default::default()
    });
    crate::profiling::note_resource_churn!(TextureView, "render_graph::gpu_cache_d2_array_view");
    view
}
