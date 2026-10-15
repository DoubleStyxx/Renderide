//! Shader-module construction helpers shared by render-graph passes.

/// Creates a WGSL shader module with the renderer's standard descriptor shape.
pub(crate) fn create_wgsl_shader_module(
    device: &wgpu::Device,
    label: &str,
    source: &str,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}
