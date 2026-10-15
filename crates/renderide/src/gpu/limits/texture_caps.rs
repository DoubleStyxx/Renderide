//! Texture dimension, array layer, format, and edge-clamping helpers.

use super::{CUBEMAP_ARRAY_LAYERS, GpuLimits, MAX_RENDER_TEXTURE_EDGE};

impl GpuLimits {
    /// `max_texture_dimension_2d` for the device.
    #[inline]
    pub fn max_texture_dimension_2d(&self) -> u32 {
        self.wgpu.max_texture_dimension_2d
    }

    /// `max_texture_dimension_3d` for the device.
    #[inline]
    pub fn max_texture_dimension_3d(&self) -> u32 {
        self.wgpu.max_texture_dimension_3d
    }

    /// `max_texture_array_layers` for the device (cubemaps use [`CUBEMAP_ARRAY_LAYERS`]).
    #[inline]
    pub fn max_texture_array_layers(&self) -> u32 {
        self.wgpu.max_texture_array_layers
    }

    /// Returns `true` if `(w, h)` fits in [`Self::max_texture_dimension_2d`].
    #[must_use]
    #[inline]
    pub fn texture_2d_fits(&self, w: u32, h: u32) -> bool {
        let m = self.wgpu.max_texture_dimension_2d;
        w <= m && h <= m
    }

    /// Returns `true` if `(w, h, d)` fits in [`Self::max_texture_dimension_3d`].
    #[must_use]
    #[inline]
    pub fn texture_3d_fits(&self, w: u32, h: u32, d: u32) -> bool {
        let m = self.wgpu.max_texture_dimension_3d;
        w <= m && h <= m && d <= m
    }

    /// Returns `true` if `layers` fits in [`Self::max_texture_array_layers`].
    #[must_use]
    #[inline]
    pub fn array_layers_fit(&self, layers: u32) -> bool {
        layers <= self.wgpu.max_texture_array_layers
    }

    /// Returns `true` when [`Self::max_texture_array_layers`] is at least [`CUBEMAP_ARRAY_LAYERS`].
    #[must_use]
    #[inline]
    pub fn cubemap_fits_texture_array_layers(&self) -> bool {
        self.wgpu.max_texture_array_layers >= CUBEMAP_ARRAY_LAYERS
    }

    /// Returns `true` when multisampled 2D-array textures are valid on this device.
    #[must_use]
    #[inline]
    pub fn supports_multisample_array(&self) -> bool {
        self.features.contains(wgpu::Features::MULTISAMPLE_ARRAY)
    }

    /// Effective texture-format features used by this device for validation.
    #[must_use]
    #[inline]
    pub fn texture_format_features(
        &self,
        format: wgpu::TextureFormat,
    ) -> wgpu::TextureFormatFeatures {
        self.texture_format_features
            .get(&format)
            .copied()
            .unwrap_or_else(|| format.guaranteed_format_features(self.features))
    }

    /// Returns `true` when `usage` is allowed for `format` on this device.
    #[must_use]
    #[inline]
    pub fn texture_usage_supported(
        &self,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> bool {
        self.texture_format_features(format)
            .allowed_usages
            .contains(usage)
    }

    /// Returns `true` when `sample_count` is valid for `format` on this device.
    #[must_use]
    #[inline]
    pub fn texture_sample_count_supported(
        &self,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> bool {
        sample_count <= 1
            || self
                .texture_format_features(format)
                .flags
                .sample_count_supported(sample_count)
    }

    /// Clamps host edge length for render textures: `[4, min(MAX_RENDER_TEXTURE_EDGE, max_texture_dimension_2d)]`.
    #[inline]
    pub fn clamp_render_texture_edge(&self, edge: i32) -> u32 {
        let cap = self
            .wgpu
            .max_texture_dimension_2d
            .min(MAX_RENDER_TEXTURE_EDGE);
        edge.clamp(4, cap as i32) as u32
    }

    /// Clamps `edge` to `[1, max_texture_dimension_2d]`. Returns `None` when `edge == 0`.
    #[must_use]
    #[cfg(test)]
    #[inline]
    pub fn clamp_texture_2d_edge(&self, edge: u32) -> Option<u32> {
        if edge == 0 {
            return None;
        }
        Some(edge.min(self.wgpu.max_texture_dimension_2d))
    }
}
