//! Bind-group, shader-stage, color-attachment, and vertex caps.

use super::GpuLimits;

impl GpuLimits {
    /// `max_bind_groups` for the device.
    #[cfg(test)]
    #[inline]
    pub fn max_bind_groups(&self) -> u32 {
        self.wgpu.max_bind_groups
    }

    /// `max_bindings_per_bind_group` for the device.
    #[inline]
    pub fn max_bindings_per_bind_group(&self) -> u32 {
        self.wgpu.max_bindings_per_bind_group
    }

    /// `max_samplers_per_shader_stage` for the device.
    #[inline]
    pub fn max_samplers_per_shader_stage(&self) -> u32 {
        self.wgpu.max_samplers_per_shader_stage
    }

    /// `max_sampled_textures_per_shader_stage` for the device.
    #[inline]
    pub fn max_sampled_textures_per_shader_stage(&self) -> u32 {
        self.wgpu.max_sampled_textures_per_shader_stage
    }

    /// `max_storage_textures_per_shader_stage` for the device.
    #[cfg(test)]
    #[inline]
    pub fn max_storage_textures_per_shader_stage(&self) -> u32 {
        self.wgpu.max_storage_textures_per_shader_stage
    }

    /// `max_storage_buffers_per_shader_stage` for the device.
    #[cfg(test)]
    #[inline]
    pub fn max_storage_buffers_per_shader_stage(&self) -> u32 {
        self.wgpu.max_storage_buffers_per_shader_stage
    }

    /// `max_uniform_buffers_per_shader_stage` for the device.
    #[cfg(test)]
    #[inline]
    pub fn max_uniform_buffers_per_shader_stage(&self) -> u32 {
        self.wgpu.max_uniform_buffers_per_shader_stage
    }

    /// `max_color_attachments` for the device.
    #[cfg(test)]
    #[inline]
    pub fn max_color_attachments(&self) -> u32 {
        self.wgpu.max_color_attachments
    }

    /// `max_vertex_buffers` for the device.
    #[inline]
    pub fn max_vertex_buffers(&self) -> u32 {
        self.wgpu.max_vertex_buffers
    }

    /// `max_vertex_attributes` for the device.
    #[inline]
    pub fn max_vertex_attributes(&self) -> u32 {
        self.wgpu.max_vertex_attributes
    }
}
