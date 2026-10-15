//! Tiny helper for resolving the wgpu format a transient color attachment is bound to.

use crate::render_graph::context::GraphResolvedResources;
use crate::render_graph::resources::TextureHandle;

/// Resolves the wgpu format the transient color attachment is bound to this frame, falling back to
/// `default` when the texture has not been resolved yet (typically only during pass setup before
/// the first record).
pub(in crate::passes) fn transient_output_format_or(
    output: TextureHandle,
    graph_resources: &GraphResolvedResources,
    default: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    graph_resources
        .transient_texture(output)
        .map_or(default, |t| t.texture.format())
}
