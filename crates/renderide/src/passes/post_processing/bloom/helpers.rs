//! Small shared helpers for the bloom pass implementations.

use crate::render_graph::context::GraphResolvedResources;

/// Resolves the color attachment format for a transient handle; falls back to the bloom texture
/// format (`Rg11b10Ufloat`) when the handle has no current mapping (graph build error).
pub(super) fn attachment_format(
    graph_resources: &GraphResolvedResources,
    handle: crate::render_graph::resources::TextureHandle,
) -> wgpu::TextureFormat {
    graph_resources
        .transient_texture(handle)
        .map_or(wgpu::TextureFormat::Rg11b10Ufloat, |t| t.texture.format())
}
