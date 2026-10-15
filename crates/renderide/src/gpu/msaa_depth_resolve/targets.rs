//! View bundles consumed by [`super::MsaaDepthResolveResources`] encoders.
//!
//! Pure data: the encoder methods take these structs by value to keep the call sites
//! readable when many texture views are involved (mono compute input + R32 intermediate +
//! single-sample destination, or stereo per-eye + array intermediate + multiview destination).

/// Single-view (desktop) MSAA depth resolve: sampled views and destination depth format.
pub struct MsaaDepthResolveMonoTargets<'a> {
    /// Multisampled depth texture view (compute input).
    pub msaa_depth_view: &'a wgpu::TextureView,
    /// R32Float intermediate (compute output, blit source).
    pub r32_view: &'a wgpu::TextureView,
    /// Single-sample depth attachment to clear and blit into.
    pub dst_depth_view: &'a wgpu::TextureView,
    /// Format of `dst_depth_view` (selects blit pipeline).
    pub dst_depth_format: wgpu::TextureFormat,
}

/// Stereo multiview MSAA depth resolve: per-eye views, array sample view, and destination.
pub struct MsaaDepthResolveStereoTargets<'a> {
    /// Two single-layer MSAA depth views (one dispatch each).
    pub msaa_depth_layer_views: [&'a wgpu::TextureView; 2],
    /// Two single-layer R32Float storage views.
    pub r32_layer_views: [&'a wgpu::TextureView; 2],
    /// `D2Array` view of the R32 intermediate for the multiview blit.
    pub r32_array_view: &'a wgpu::TextureView,
    /// Two-layer depth attachment view.
    pub dst_depth_view: &'a wgpu::TextureView,
    /// Format of `dst_depth_view` (selects stereo blit pipeline).
    pub dst_depth_format: wgpu::TextureFormat,
}
