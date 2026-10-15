//! Bind group / uniform buffer scaffolding shared by every IBL compute pass.

use bytemuck::Pod;
use wgpu::util::DeviceExt;

/// Creates a `UNIFORM` buffer initialized from a [`Pod`] value.
pub(super) fn make_uniform_buffer<T: Pod>(
    device: &wgpu::Device,
    label: &'static str,
    value: &T,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(value),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

/// Builds a bind group for the analytic mip-0 producer: `[uniform, storage_output]`.
pub(super) fn build_storage_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    uniform: &wgpu::Buffer,
    dst: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(dst),
            },
        ],
    })
}

/// Builds a bind group for source-pyramid downsample passes: `[uniform, source_view, storage_output]`.
pub(super) fn build_input_output_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    uniform: &wgpu::Buffer,
    src: &wgpu::TextureView,
    dst: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(src),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(dst),
            },
        ],
    })
}

/// Builds a bind group for sampled producers and the convolve passes:
/// `[uniform, source_view, sampler, storage_output]`.
pub(super) fn build_sampled_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    uniform: &wgpu::Buffer,
    src: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    dst: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(src),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(dst),
            },
        ],
    })
}
