//! Dear ImGui overlay for developer diagnostics.
//!
//! The **Frame timing** window shows FPS, CPU/GPU frame intervals, RAM/VRAM, and a frametime graph.
//! **Feedback / Bug Report** shows quick links for reporting issues and joining discussion.
//! **[`crate::config::DebugSettings::debug_hud_frame_timing`]** toggles the **Frame timing** window (default on).
//! **[`crate::config::DebugSettings::debug_hud_links`]** toggles the **Feedback / Bug Report** links panel.
//! **[`crate::config::DebugSettings::debug_hud_enabled`]** toggles **Renderide debug** (Stats / Shader routes / Draw state / GPU memory / GPU passes).
//! **[`crate::config::DebugSettings::debug_hud_transforms`]** toggles the **Scene transforms** window.
//! **[`crate::config::DebugSettings::debug_hud_textures`]** toggles the **Textures** window.
//!
//! The overlay is split so the UI can run without a GPU:
//!
//! - [`ui::HudUi`] owns the ImGui context, snapshots, and draw-list generation.
//! - [`gpu::HudGpu`] owns the [`imgui_wgpu`] renderer and the load-on-top pass.
//! - [`DebugHud`] is the facade the rest of the renderer calls. It composes the two.
//!
//! HUD-rendering infrastructure lives in submodules:
//!
//! - [`layout`]: declarative window placement (`Viewport`, `WindowSlot`) plus
//!   the stacked-column constants and helpers used by the anchored HUD windows.
//! - [`state`]: [`state::HudUiState`], the per-tab view state and filters owned by
//!   [`DebugHud`].
//! - [`view`]: rendering algebra (`HudWindow`, `TabView`).
//! - [`registry`]: static-dispatch [`registry::DebugWindow`] enum + [`registry::OverlayFeatureFlags`].
//! - [`fmt`]: right-aligned numeric formatters and byte-compaction helpers.
//! - [`input`]: HUD input transport plus per-frame ImGui IO bridge.
//! - [`windows`]: concrete window/tab impls.

pub mod fmt;
pub mod input;
pub mod layout;
pub mod metrics;
pub(crate) mod persistence;
pub mod registry;
pub mod state;
pub mod view;
pub mod windows;

mod fixture;
mod gpu;
mod ui;
pub(crate) mod work_counters;

#[cfg(test)]
mod zero_work;

pub use crate::hud_contract::DebugHudEncodeError;
pub use fixture::HudMeasureView;
pub use input::{DebugHudInput, sanitize_input_state_for_imgui_host};
pub use metrics::DebugHudMetricInterest;
pub use ui::HudUi;

/// GPU target and encoder state for one debug-HUD overlay encode.
pub(crate) struct DebugHudOverlayContext<'a, 'encoder> {
    /// WGPU device used by the ImGui renderer.
    pub(crate) device: &'a wgpu::Device,
    /// WGPU queue used by the ImGui renderer for texture updates.
    pub(crate) queue: &'a wgpu::Queue,
    /// Command encoder receiving the HUD render pass.
    pub(crate) encoder: &'encoder mut wgpu::CommandEncoder,
    /// Swapchain or offscreen color view the HUD should composite over.
    pub(crate) backbuffer: &'a wgpu::TextureView,
    /// Pixel extent of the target surface.
    pub(crate) extent: (u32, u32),
    /// Optional GPU profiler for pass timestamp instrumentation.
    pub(crate) profiler: Option<&'a crate::profiling::GpuProfilerHandle>,
}

use self::gpu::HudGpu;

/// Dear ImGui overlay: feedback links, frame timing, renderer stats, shader routes, scene transforms, and config UI.
///
/// The CPU UI ([`HudUi`]) and the wgpu renderer ([`HudGpu`]) are separate so draw-list
/// tests do not need a device. Callers still use this facade.
pub struct DebugHud {
    ui: HudUi,
    gpu: HudGpu,
}

impl DebugHud {
    /// Builds ImGui and the wgpu render backend for the swapchain format.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        renderer_settings: crate::config::RendererSettingsHandle,
        config_save_path: std::path::PathBuf,
        suppress_renderer_config_disk_writes: bool,
    ) -> Self {
        let mut ui = HudUi::new(
            renderer_settings,
            config_save_path,
            suppress_renderer_config_disk_writes,
        );
        let gpu = HudGpu::new(ui.imgui_mut(), device, queue, surface_format);
        Self { ui, gpu }
    }

    /// Replaces the GPU profiler snapshot shown in the **GPU passes** tab.
    ///
    /// Called once per winit tick from the HUD update path with the latest snapshot read out of
    /// [`crate::gpu::GpuContext::latest_gpu_profiler_snapshot_handle`].
    pub fn set_gpu_profiler_snapshot(&mut self, snapshot: crate::profiling::GpuProfilerSnapshot) {
        self.ui.set_gpu_profiler_snapshot(snapshot);
    }

    /// Clears the **GPU passes** tab payload.
    pub fn clear_gpu_profiler_snapshot(&mut self) {
        self.ui.clear_gpu_profiler_snapshot();
    }

    /// Stores [`crate::diagnostics::FrameTimingHudSnapshot`] for the **Frame timing** window.
    pub fn set_frame_timing(&mut self, sample: crate::diagnostics::FrameTimingHudSnapshot) {
        self.ui.set_frame_timing(sample);
    }

    /// Clears the **Frame timing** window payload.
    pub fn clear_frame_timing(&mut self) {
        self.ui.clear_frame_timing();
    }

    /// Stores [`crate::diagnostics::RendererInfoSnapshot`] for the **Stats** tab (IPC, adapter, scene, materials, graph).
    pub fn set_snapshot(&mut self, sample: crate::diagnostics::RendererInfoSnapshot) {
        self.ui.set_snapshot(sample);
    }

    /// Stores [`crate::diagnostics::FrameDiagnosticsSnapshot`] for timing, host/allocator, draws, textures, shader routes, and GPU memory tab data.
    pub fn set_frame_diagnostics(&mut self, sample: crate::diagnostics::FrameDiagnosticsSnapshot) {
        self.ui.set_frame_diagnostics(sample);
    }

    /// Stores per-render-space world transform rows for the **Scene transforms** window.
    pub fn set_scene_transforms_snapshot(
        &mut self,
        sample: crate::diagnostics::SceneTransformsSnapshot,
    ) {
        self.ui.set_scene_transforms_snapshot(sample);
    }

    /// Clears the **Scene transforms** window payload.
    pub fn clear_scene_transforms_snapshot(&mut self) {
        self.ui.clear_scene_transforms_snapshot();
    }

    /// Stores texture pool rows for the **Textures** window.
    pub fn set_texture_debug_snapshot(&mut self, sample: crate::diagnostics::TextureDebugSnapshot) {
        self.ui.set_texture_debug_snapshot(sample);
    }

    /// Clears main debug tab payloads only (not frame timing or scene transforms).
    pub fn clear_stats_hud_payloads(&mut self) {
        self.ui.clear_stats_hud_payloads();
    }

    /// Clears the **Textures** HUD payload.
    pub fn clear_texture_debug_snapshot(&mut self) {
        self.ui.clear_texture_debug_snapshot();
    }

    /// Returns `true` when at least one HUD window will draw something this frame.
    ///
    /// Used by the render-graph executor to skip the entire HUD command encoder + GPU profiler
    /// query wrap when ImGui is hidden. Skipping is safe even when the HUD has been open in prior
    /// frames: ImGui's per-frame state lives on [`HudUi`] and is only consumed when
    /// [`Self::encode_overlay`] runs, so dropping a frame's encode does not corrupt later frames'
    /// drawing. When ImGui is visible, **Renderer config** always draws so configuration remains
    /// reachable.
    pub fn has_visible_content(&self) -> bool {
        self.ui.has_visible_content()
    }

    /// Records ImGui into `encoder` as a load-on-top pass over `backbuffer`.
    ///
    /// Iterates [`registry::DebugWindow::ALL`] and dispatches to the matching
    /// [`view::HudWindow`] impl per variant; [`registry::DebugWindow::enabled`] gates each
    /// window from [`registry::OverlayFeatureFlags::from_settings`]. Adding a new window means
    /// adding an enum variant + one match arm; the encode path stays a single for-loop.
    pub fn encode_overlay(
        &mut self,
        target: DebugHudOverlayContext<'_, '_>,
        input: &DebugHudInput,
    ) -> Result<(bool, bool), DebugHudEncodeError> {
        profiling::scope!("hud::encode_overlay");
        let DebugHudOverlayContext {
            device,
            queue,
            encoder,
            backbuffer,
            extent,
            profiler,
        } = target;
        self.ui.build_ui(extent, input);
        let pass_result = {
            let draw_data = self.ui.render_draw_data();
            self.gpu
                .encode_pass(draw_data, device, queue, encoder, backbuffer, profiler)
        };
        let result = pass_result.map(|()| self.ui.want_capture());
        self.ui.finish_frame();
        result
    }
}
