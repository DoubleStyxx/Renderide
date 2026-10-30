//! wgpu half of the debug HUD.
//!
//! [`HudGpu`] owns the [`imgui_wgpu`] renderer. UI construction and draw-list
//! generation live on [`super::ui::HudUi`] and do not need a device.

use imgui::DrawData;
use imgui_wgpu::{Renderer as ImguiWgpuRenderer, RendererConfig};

use crate::hud_contract::DebugHudEncodeError;

use super::work_counters::note_encode;

/// ImGui's wgpu renderer and the load-on-top overlay pass.
pub(crate) struct HudGpu {
    renderer: ImguiWgpuRenderer,
}

impl HudGpu {
    /// Creates the renderer for `surface_format` and uploads the font atlas.
    pub(crate) fn new(
        imgui: &mut imgui::Context,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let mut renderer_config = RendererConfig::new();
        renderer_config.texture_format = surface_format;
        let renderer = ImguiWgpuRenderer::new(imgui, device, queue, renderer_config);
        Self { renderer }
    }

    /// Records the already-built draw list into a load-on-top pass over `backbuffer`.
    pub(crate) fn encode_pass(
        &mut self,
        draw_data: &DrawData,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        backbuffer: &wgpu::TextureView,
        profiler: Option<&crate::profiling::GpuProfilerHandle>,
    ) -> Result<(), DebugHudEncodeError> {
        profiling::scope!("hud::encode_imgui_wgpu");
        note_encode();
        let pass_query = profiler.map(|p| p.begin_pass_query("hud::imgui_wgpu_pass", encoder));
        let timestamp_writes = crate::profiling::render_pass_timestamp_writes(pass_query.as_ref());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgui-debug-hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: backbuffer,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes,
                multiview_mask: None,
            });
            self.renderer
                .render(draw_data, queue, device, &mut pass)
                .map_err(|e| DebugHudEncodeError::ImguiWgpu(e.to_string()))?;
        };
        if let (Some(p), Some(q)) = (profiler, pass_query) {
            p.end_query(encoder, q);
        }
        Ok(())
    }
}
