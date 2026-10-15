//! Buffer size and binding-size caps with matching `_fits` predicates.

use super::GpuLimits;

impl GpuLimits {
    /// `max_buffer_size` for the device.
    #[inline]
    pub fn max_buffer_size(&self) -> u64 {
        self.wgpu.max_buffer_size
    }

    /// `max_storage_buffer_binding_size` for the device.
    #[inline]
    pub fn max_storage_buffer_binding_size(&self) -> u64 {
        self.wgpu.max_storage_buffer_binding_size
    }

    /// `max_uniform_buffer_binding_size` for the device.
    #[inline]
    pub fn max_uniform_buffer_binding_size(&self) -> u64 {
        self.wgpu.max_uniform_buffer_binding_size
    }

    /// Returns `true` if `bytes` fits in [`Self::max_buffer_size`].
    #[must_use]
    #[inline]
    pub fn buffer_size_fits(&self, bytes: u64) -> bool {
        bytes <= self.wgpu.max_buffer_size
    }

    /// Returns `true` if `bytes` fits in [`Self::max_storage_buffer_binding_size`].
    #[must_use]
    #[inline]
    pub fn storage_binding_fits(&self, bytes: u64) -> bool {
        bytes <= self.wgpu.max_storage_buffer_binding_size
    }

    /// Returns `true` if `bytes` fits in [`Self::max_uniform_buffer_binding_size`].
    #[must_use]
    #[inline]
    pub fn uniform_binding_fits(&self, bytes: u64) -> bool {
        bytes <= self.wgpu.max_uniform_buffer_binding_size
    }
}
