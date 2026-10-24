//! Copies an existing full-target render texture into HDR scene color.
//!
//! Forward shading writes a transient HDR image, and compose then blits that image over the
//! camera target. Loading the transient does not keep the render texture. Depth-only and
//! clear-none cameras therefore copy the current color in first, then load it. The double-buffered
//! camera that claimed this frame's swap copies its history image. A later camera on that texture
//! copies the published write image. Skybox and color cameras skip this pass.

use crate::frame_contract::RenderTextureSelfSampling;
use crate::passes::helpers::missing_pass_resource;
use crate::render_graph::context::EncoderPassCtx;
use crate::render_graph::error::{RenderPassError, SetupError};
use crate::render_graph::pass::{EncoderPass, PassBuilder};
use crate::render_graph::resources::{ImportedTextureHandle, TextureAccess, TextureHandle};

use super::clear_load::forward_clear_loads_for_view;

/// Encoder pass that seeds HDR scene color from the current full-target color texture.
#[derive(Debug)]
pub struct WorldMeshForwardColorPreservePass {
    frame_color: ImportedTextureHandle,
    scene_color_hdr: TextureHandle,
}

impl WorldMeshForwardColorPreservePass {
    /// Creates the preserve pass for the main graph's frame color and HDR scene color.
    pub fn new(frame_color: ImportedTextureHandle, scene_color_hdr: TextureHandle) -> Self {
        Self {
            frame_color,
            scene_color_hdr,
        }
    }
}

impl EncoderPass for WorldMeshForwardColorPreservePass {
    fn name(&self) -> &str {
        "WorldMeshForwardColorPreserve"
    }

    fn setup(&mut self, b: &mut PassBuilder<'_>) -> Result<(), SetupError> {
        b.encoder();
        b.import_texture(self.frame_color, TextureAccess::CopySrc);
        b.write_texture(self.scene_color_hdr, TextureAccess::CopyDst);
        Ok(())
    }

    fn should_record(&self, ctx: &EncoderPassCtx<'_, '_, '_>) -> Result<bool, RenderPassError> {
        Ok(forward_clear_loads_for_view(ctx.frame.view).seed_color)
    }

    fn record(&self, ctx: &mut EncoderPassCtx<'_, '_, '_>) -> Result<(), RenderPassError> {
        let extent = ctx.frame.view.viewport_px;
        let source = preserved_color_texture(
            ctx.frame
                .view
                .offscreen_write_target
                .render_texture_self_sampling(),
            ctx.frame.view.color_texture,
            ctx.frame.view.history_color_texture,
            ctx.frame.view.seed_published_color,
        )
        .ok_or_else(|| missing_pass_resource(self.name(), "preserved color texture"))?;
        let destination = ctx
            .graph_resources
            .transient_texture(self.scene_color_hdr)
            .ok_or_else(|| missing_pass_resource(self.name(), "scene color hdr"))?;
        let profiler = ctx.profiler;
        let query = profiler.map(|profiler| {
            profiler.begin_query("world_mesh_forward::color_preserve", ctx.encoder)
        });
        copy_preserved_scene_color(ctx.encoder, source, &destination.texture, extent);
        if let Some(query) = query
            && let Some(profiler) = profiler
        {
            profiler.end_query(ctx.encoder, query);
        }
        Ok(())
    }
}

/// Copies `extent_px` of `source` into `destination`.
///
/// A zero-sized view records nothing. The copy is clamped to both textures so a mismatched
/// extent cannot fail validation; full-target cameras size the HDR transient to the same viewport.
pub(crate) fn copy_preserved_scene_color(
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::Texture,
    destination: &wgpu::Texture,
    extent_px: (u32, u32),
) {
    let source_size = source.size();
    let destination_size = destination.size();
    let width = extent_px
        .0
        .min(source_size.width)
        .min(destination_size.width);
    let height = extent_px
        .1
        .min(source_size.height)
        .min(destination_size.height);
    if width == 0 || height == 0 {
        return;
    }
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: source,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: destination,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

/// Color image a Nothing or Depth camera copies into HDR scene color.
///
/// The double-buffered camera that claimed the swap returns `history` and never the write
/// image. A later camera on that texture returns `write`. Every other camera returns `write`.
/// `None` means this view has no image to preserve.
pub(crate) fn preserved_color_texture<'a>(
    self_sampling: Option<RenderTextureSelfSampling>,
    write: Option<&'a wgpu::Texture>,
    history: Option<&'a wgpu::Texture>,
    seed_published_color: bool,
) -> Option<&'a wgpu::Texture> {
    if seed_published_color {
        return write;
    }
    if self_sampling == Some(RenderTextureSelfSampling::AllowPreviousContents) {
        history
    } else {
        write
    }
}
