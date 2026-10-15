//! Storage and uniform buffer offset alignment helpers.

use super::GpuLimits;

impl GpuLimits {
    /// `min_storage_buffer_offset_alignment` for dynamic storage offsets (e.g. per-draw slab).
    #[inline]
    pub fn min_storage_buffer_offset_alignment(&self) -> u32 {
        self.wgpu.min_storage_buffer_offset_alignment
    }

    /// `min_uniform_buffer_offset_alignment` for dynamic uniform offsets.
    #[inline]
    pub fn min_uniform_buffer_offset_alignment(&self) -> u32 {
        self.wgpu.min_uniform_buffer_offset_alignment
    }

    /// Rounds `n` up to a multiple of [`Self::min_storage_buffer_offset_alignment`].
    #[cfg(test)]
    #[inline]
    pub fn align_storage_offset(&self, n: u64) -> u64 {
        let align = u64::from(self.wgpu.min_storage_buffer_offset_alignment).max(1);
        n.div_ceil(align) * align
    }

    /// Rounds `n` up to a multiple of [`Self::min_uniform_buffer_offset_alignment`].
    #[cfg(test)]
    #[inline]
    pub fn align_uniform_offset(&self, n: u64) -> u64 {
        let align = u64::from(self.wgpu.min_uniform_buffer_offset_alignment).max(1);
        n.div_ceil(align) * align
    }
}
