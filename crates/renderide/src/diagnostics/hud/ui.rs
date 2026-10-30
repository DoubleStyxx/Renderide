//! CPU half of the debug HUD: ImGui context, snapshots, and draw-list generation.
//!
//! [`HudUi`] does not own a wgpu device. [`super::gpu::HudGpu`] consumes the draw
//! list produced by [`HudUi::render_draw_data`]. Allocation tests drive [`HudUi`]
//! alone.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use imgui::{Condition, Context, FontConfig, FontSource};

use crate::config::{DebugHudSettings, RendererSettingsHandle, save_renderer_settings};
use crate::diagnostics::snapshots::{
    FrameDiagnosticsSnapshot, FrameTimingHudSnapshot, RendererInfoSnapshot,
    SceneTransformsSnapshot, TextureDebugSnapshot,
};

use super::fixture::{self, HudMeasureView};
use super::input::{DebugHudInput, apply_input};
use super::layout::Viewport;
use super::persistence::{
    imgui_ini_path_from_config_save_path, read_nonempty_text, write_nonempty_text_atomic,
};
use super::registry::{DebugWindow, OverlayFeatureFlags};
use super::state::HudUiState;
use super::view::HudWindow;
use super::windows::feedback::FeedbackWindow;
use super::windows::frame_timing::FrameTimingWindow;
use super::windows::main_debug::{MainDebugWindow, MainDebugWindowData};
use super::windows::renderer_config::{RendererConfigData, RendererConfigWindow};
use super::windows::scene_transforms::SceneTransformsWindow;
use super::windows::texture_debug::TextureDebugWindow;
use super::work_counters::{note_draw_list, note_settings_lock, note_ui_frame};

const IMGUI_INI_SAVE_RATE_SECS: f32 = 0.25;

/// Fixed viewport used by the steady-state allocation fixture.
pub const MEASURE_VIEWPORT_PX: (u32, u32) = (1920, 1080);
/// UI scale used by the steady-state allocation fixture.
pub const MEASURE_UI_SCALE: f32 = 1.0;

/// ImGui context, HUD snapshots, and semantic UI state. No wgpu types.
pub struct HudUi {
    imgui: Context,
    last_frame_at: Instant,
    /// When set, frame delta is this value instead of wall time.
    ///
    /// Production leaves this `None`. The allocation fixture sets it so steady-state
    /// counts do not depend on how fast the test process schedules frames.
    frame_delta_override: Option<Duration>,
    /// Lightweight FPS / wall / CPU-submit / GPU-idle metrics ([`FrameTimingHudSnapshot`]).
    frame_timing: Option<FrameTimingHudSnapshot>,
    latest: Option<RendererInfoSnapshot>,
    /// Per-frame timing, draws, host metrics, shader routes, and GPU allocator detail ([`FrameDiagnosticsSnapshot`]).
    frame_diagnostics: Option<FrameDiagnosticsSnapshot>,
    /// Per-frame world transform listing for the **Scene transforms** window.
    scene_transforms: SceneTransformsSnapshot,
    /// Per-frame texture pool listing for the **Textures** window.
    texture_debug: TextureDebugSnapshot,
    /// Per-tab view state and filter toggles.
    ui_state: HudUiState,
    /// Live settings + persistence target for the **Renderer config** window.
    renderer_settings: RendererSettingsHandle,
    config_save_path: PathBuf,
    /// Sidecar path for Dear ImGui's raw window layout settings.
    imgui_ini_path: PathBuf,
    /// When `true`, do not write `config.toml` from the overlay (startup Figment extract failed).
    suppress_renderer_config_disk_writes: bool,
    /// Most recent flattened per-pass GPU timings and query stats for the **GPU passes** tab.
    gpu_profiler_snapshot: crate::profiling::GpuProfilerSnapshot,
}

impl HudUi {
    /// Builds the ImGui context, default font, and persisted UI state. Does not touch wgpu.
    pub(crate) fn new(
        renderer_settings: RendererSettingsHandle,
        config_save_path: PathBuf,
        suppress_renderer_config_disk_writes: bool,
    ) -> Self {
        let hud_settings = renderer_settings
            .read()
            .map(|g| g.debug.hud.clone())
            .unwrap_or_else(|_| DebugHudSettings::default());

        let mut imgui = Context::create();
        imgui.set_ini_filename(None);
        imgui.set_log_filename(None);
        {
            let io = imgui.io_mut();
            io.config_windows_move_from_title_bar_only = true;
            io.config_drag_click_to_input_text = true;
            io.font_global_scale = hud_settings.resolved_ui_scale();
            io.ini_saving_rate = IMGUI_INI_SAVE_RATE_SECS;
        }
        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                oversample_h: 2,
                pixel_snap_h: true,
                size_pixels: 14.0,
                ..FontConfig::default()
            }),
        }]);

        let imgui_ini_path = imgui_ini_path_from_config_save_path(&config_save_path);
        Self::load_imgui_ini_if_enabled(&mut imgui, &imgui_ini_path, &hud_settings);

        Self {
            imgui,
            last_frame_at: Instant::now(),
            frame_delta_override: None,
            frame_timing: None,
            latest: None,
            frame_diagnostics: None,
            scene_transforms: SceneTransformsSnapshot::default(),
            texture_debug: TextureDebugSnapshot::default(),
            ui_state: HudUiState::from_settings(&hud_settings),
            renderer_settings,
            config_save_path,
            imgui_ini_path,
            suppress_renderer_config_disk_writes,
            gpu_profiler_snapshot: crate::profiling::GpuProfilerSnapshot::default(),
        }
    }

    /// HUD UI loaded with the fixed H0a fixture for `view`.
    ///
    /// Viewport is [`MEASURE_VIEWPORT_PX`], scale is [`MEASURE_UI_SCALE`], and the
    /// font is ImGui's default 14 px ProggyClean atlas. Layout ini saving is off so
    /// the steady-state allocation count is the widget and formatter work.
    pub fn from_measure_fixture(view: HudMeasureView) -> Self {
        let settings = Arc::new(RwLock::new(fixture::settings_for(view)));
        let mut ui = Self::new(settings, PathBuf::from("hud-measure-fixture.toml"), true);
        ui.frame_delta_override = Some(Duration::from_millis(16));
        // Production builds this atlas inside [`super::gpu::HudGpu::new`]. The
        // allocation test has no device, so build it here before draw lists.
        let _atlas = ui.imgui.fonts().build_rgba32_texture();
        fixture::install(&mut ui, view);
        ui
    }

    /// One CPU HUD frame at [`MEASURE_VIEWPORT_PX`] with the fixed fixture input.
    ///
    /// Hidden views return after the visibility read and do not call `imgui.frame()`
    /// or `render()`. Open views build the UI, the draw list, and run the same
    /// post-frame settings persist as [`super::DebugHud::encode_overlay`].
    pub fn run_measure_frame(&mut self) {
        if !self.has_visible_content() {
            return;
        }
        let input = DebugHudInput {
            cursor_px: [960.0, 540.0],
            window_focused: true,
            mouse_active: true,
            ..DebugHudInput::default()
        };
        self.build_ui(MEASURE_VIEWPORT_PX, &input);
        let _draw_data = self.render_draw_data();
        self.finish_frame();
    }

    pub(crate) fn imgui_mut(&mut self) -> &mut Context {
        &mut self.imgui
    }

    /// Replaces the GPU profiler snapshot shown in the **GPU passes** tab.
    pub(crate) fn set_gpu_profiler_snapshot(
        &mut self,
        snapshot: crate::profiling::GpuProfilerSnapshot,
    ) {
        self.gpu_profiler_snapshot = snapshot;
    }

    /// Clears the **GPU passes** tab payload.
    pub(crate) fn clear_gpu_profiler_snapshot(&mut self) {
        self.gpu_profiler_snapshot = crate::profiling::GpuProfilerSnapshot::default();
    }

    /// Stores [`FrameTimingHudSnapshot`] for the **Frame timing** window.
    pub(crate) fn set_frame_timing(&mut self, sample: FrameTimingHudSnapshot) {
        self.frame_timing = Some(sample);
    }

    /// Clears the **Frame timing** window payload.
    pub(crate) fn clear_frame_timing(&mut self) {
        self.frame_timing = None;
    }

    /// Stores [`RendererInfoSnapshot`] for the **Stats** tab.
    pub(crate) fn set_snapshot(&mut self, sample: RendererInfoSnapshot) {
        self.latest = Some(sample);
    }

    /// Stores [`FrameDiagnosticsSnapshot`] for the main debug tabs.
    pub(crate) fn set_frame_diagnostics(&mut self, sample: FrameDiagnosticsSnapshot) {
        self.frame_diagnostics = Some(sample);
    }

    /// Stores per-render-space world transform rows for the **Scene transforms** window.
    pub(crate) fn set_scene_transforms_snapshot(&mut self, sample: SceneTransformsSnapshot) {
        self.scene_transforms = sample;
    }

    /// Clears the **Scene transforms** window payload.
    pub(crate) fn clear_scene_transforms_snapshot(&mut self) {
        self.scene_transforms = SceneTransformsSnapshot::default();
    }

    /// Stores texture pool rows for the **Textures** window.
    pub(crate) fn set_texture_debug_snapshot(&mut self, sample: TextureDebugSnapshot) {
        self.texture_debug = sample;
    }

    /// Clears main debug tab payloads only (not [`Self::frame_timing`] or scene transforms).
    pub(crate) fn clear_stats_hud_payloads(&mut self) {
        self.latest = None;
        self.frame_diagnostics = None;
        self.clear_gpu_profiler_snapshot();
    }

    /// Clears the **Textures** HUD payload.
    pub(crate) fn clear_texture_debug_snapshot(&mut self) {
        self.texture_debug = TextureDebugSnapshot::default();
    }

    /// Returns `true` when at least one HUD window will draw something this frame.
    pub(crate) fn has_visible_content(&self) -> bool {
        let flags = OverlayFeatureFlags::from_settings(&self.renderer_settings);
        flags.imgui_visible
    }

    /// Mouse and keyboard capture flags from the ImGui context after a frame.
    pub(crate) fn want_capture(&self) -> (bool, bool) {
        let io = self.imgui.io();
        (io.want_capture_mouse, io.want_capture_keyboard)
    }

    /// Updates IO, builds every enabled window, and leaves the ImGui frame open for [`Self::render_draw_data`].
    pub(crate) fn build_ui(&mut self, (width, height): (u32, u32), input: &DebugHudInput) {
        self.apply_overlay_frame_io((width, height), input);

        let flags = OverlayFeatureFlags::from_settings(&self.renderer_settings);
        let viewport = Viewport { width, height };

        note_ui_frame();
        // `self.imgui.frame()` already holds a mutable borrow on `self.imgui`, so the dispatch
        // loop cannot also take `&mut self`. Field-disjoint borrows let each match arm borrow
        // exactly the snapshot it needs while sharing `&mut self.ui_state`.
        let ui = self.imgui.frame();
        let ui_state = &mut self.ui_state;
        let frame_timing = self.frame_timing.as_ref();
        let renderer_info = self.latest.as_ref();
        let frame_diagnostics = self.frame_diagnostics.as_ref();
        let gpu_profiler_snapshot = &self.gpu_profiler_snapshot;
        let scene_transforms = &self.scene_transforms;
        let texture_debug = &self.texture_debug;
        let renderer_settings = &self.renderer_settings;
        let config_save_path = self.config_save_path.as_path();
        let suppress_renderer_config_disk_writes = self.suppress_renderer_config_disk_writes;

        for &window in DebugWindow::ALL {
            if !window.enabled(flags) {
                continue;
            }
            match window {
                DebugWindow::FrameTiming => {
                    render_window(ui, viewport, &FrameTimingWindow, frame_timing, ui_state);
                }
                DebugWindow::Feedback => {
                    render_window(ui, viewport, &FeedbackWindow, (), ui_state);
                }
                DebugWindow::Main => render_window(
                    ui,
                    viewport,
                    &MainDebugWindow,
                    MainDebugWindowData {
                        renderer_info,
                        frame_diagnostics,
                        gpu_profiler_snapshot,
                    },
                    ui_state,
                ),
                DebugWindow::SceneTransforms => render_window(
                    ui,
                    viewport,
                    &SceneTransformsWindow,
                    scene_transforms,
                    ui_state,
                ),
                DebugWindow::Textures => {
                    render_window(ui, viewport, &TextureDebugWindow, texture_debug, ui_state);
                }
                DebugWindow::RendererConfig => render_window(
                    ui,
                    viewport,
                    &RendererConfigWindow,
                    RendererConfigData {
                        settings: renderer_settings,
                        save_path: config_save_path,
                        suppress_renderer_config_disk_writes,
                    },
                    ui_state,
                ),
            }
        }
    }

    /// Builds the ImGui draw list for the frame started by [`Self::build_ui`].
    pub(crate) fn render_draw_data(&mut self) -> &imgui::DrawData {
        note_draw_list();
        self.imgui.render()
    }

    /// Persists semantic UI state and flushes a requested ini save.
    ///
    /// Runs even when the GPU pass fails, matching the previous `encode_overlay` order.
    pub(crate) fn finish_frame(&mut self) {
        self.persist_ui_state_to_config_if_changed();
        self.save_imgui_ini_if_requested();
    }

    /// Updates ImGui delta time, display size, and injects [`DebugHudInput`] for this frame.
    fn apply_overlay_frame_io(&mut self, (width, height): (u32, u32), input: &DebugHudInput) {
        profiling::scope!("hud::apply_input");
        let delta = match self.frame_delta_override {
            Some(delta) => delta,
            None => self.last_frame_at.elapsed().max(Duration::from_millis(1)),
        };
        self.last_frame_at = Instant::now();
        let hud_settings = self.current_hud_settings();

        let io = self.imgui.io_mut();
        io.display_size = [width as f32, height as f32];
        io.display_framebuffer_scale = [1.0, 1.0];
        io.font_global_scale = hud_settings.resolved_ui_scale();
        io.update_delta_time(delta);
        apply_input(io, input);
    }

    fn current_hud_settings(&self) -> DebugHudSettings {
        note_settings_lock();
        self.renderer_settings
            .read()
            .map(|g| g.debug.hud.clone())
            .unwrap_or_else(|_| DebugHudSettings::default())
    }

    fn load_imgui_ini_if_enabled(
        imgui: &mut Context,
        imgui_ini_path: &Path,
        hud_settings: &DebugHudSettings,
    ) {
        if !hud_settings.persist_layout {
            return;
        }

        match read_nonempty_text(imgui_ini_path) {
            Ok(Some(data)) => imgui.load_ini_settings(&data),
            Ok(None) => {
                logger::debug!(
                    "Ignoring empty ImGui layout file at {}",
                    imgui_ini_path.display()
                );
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                logger::warn!(
                    "Failed to read ImGui layout from {}: {e}",
                    imgui_ini_path.display()
                );
            }
        }
    }

    fn persist_ui_state_to_config_if_changed(&self) {
        note_settings_lock();
        let Ok(mut g) = self.renderer_settings.write() else {
            logger::warn!("Failed to persist HUD state: renderer settings store is unavailable");
            return;
        };

        if !self.ui_state.write_to_settings(&mut g.debug.hud) {
            return;
        }

        if self.suppress_renderer_config_disk_writes {
            logger::error!(
                "Refusing to save renderer config to {}: disk writes suppressed after startup extract failure",
                self.config_save_path.display()
            );
            return;
        }

        if let Err(e) = save_renderer_settings(&self.config_save_path, &g) {
            logger::warn!(
                "Failed to save renderer config to {}: {e}",
                self.config_save_path.display()
            );
        }
    }

    fn save_imgui_ini_now(&mut self) {
        if self.current_hud_settings().persist_layout {
            let mut contents = String::new();
            self.imgui.save_ini_settings(&mut contents);
            match write_nonempty_text_atomic(&self.imgui_ini_path, &contents) {
                Ok(true) => {}
                Ok(false) => {
                    logger::debug!(
                        "Skipping empty ImGui layout save to {}",
                        self.imgui_ini_path.display()
                    );
                }
                Err(e) => {
                    logger::warn!(
                        "Failed to save ImGui layout to {}: {e}",
                        self.imgui_ini_path.display()
                    );
                }
            }
        }

        self.imgui.io_mut().want_save_ini_settings = false;
    }

    fn save_imgui_ini_if_requested(&mut self) {
        if self.imgui.io().want_save_ini_settings {
            self.save_imgui_ini_now();
        }
    }
}

impl Drop for HudUi {
    fn drop(&mut self) {
        self.save_imgui_ini_now();
    }
}

/// Renders one [`HudWindow`] in the standard ImGui envelope (position, size, flags, bg alpha,
/// body).
fn render_window<W>(
    ui: &imgui::Ui,
    viewport: Viewport,
    window: &W,
    data: W::Data<'_>,
    state: &mut W::State,
) where
    W: HudWindow,
{
    profiling::scope!("hud::render_window");
    let slot = window.anchor(viewport);
    ui.window(window.title())
        .position(slot.position, Condition::FirstUseEver)
        .size(slot.size, Condition::FirstUseEver)
        .size_constraints(slot.size_min, slot.size_max)
        .bg_alpha(window.bg_alpha())
        .flags(window.flags())
        .build(|| window.body(ui, data, state));
}
