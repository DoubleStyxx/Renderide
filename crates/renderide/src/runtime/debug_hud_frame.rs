//! Per-tick wiring from [`super::RendererRuntime`] to the backend [`crate::backend::RenderBackend`] debug HUD.

use std::time::Instant;

use crate::config::{DebugHudMainTab, RendererSettings};
use crate::diagnostics::DebugHudMetricInterest;
use crate::diagnostics::{note_settings_lock, note_snapshot_clone};
use crate::gpu::GpuContext;
use crate::hud_contract::DebugHudEncodeError;

use super::RendererRuntime;

fn debug_hud_metric_interest(settings: &RendererSettings) -> DebugHudMetricInterest {
    DebugHudMetricInterest::from_settings(settings)
}

impl RendererRuntime {
    /// Fills the active main-tab HUD snapshot when the main HUD has an active tab.
    fn capture_main_debug_hud_tab(
        &mut self,
        gpu: &GpuContext,
        now: Instant,
        interest: DebugHudMetricInterest,
        tab: DebugHudMainTab,
    ) {
        let backend_diag = match tab {
            DebugHudMainTab::Stats | DebugHudMainTab::ShaderRoutes | DebugHudMainTab::DrawState => {
                self.backend.snapshot_for_diagnostics(interest)
            }
            DebugHudMainTab::GpuMemory | DebugHudMainTab::GpuPasses => {
                crate::diagnostics::BackendDiagSnapshot::default()
            }
        };
        let host = if tab == DebugHudMainTab::Stats {
            self.diagnostics.host_hud.snapshot()
        } else {
            Default::default()
        };
        let next_refresh_in_secs = self.diagnostics.allocator_report_next_refresh_in_secs(now);
        let (ipc_pri_str, ipc_bg_str) = if tab == DebugHudMainTab::Stats {
            self.frontend.ipc_consecutive_outbound_drop_streaks()
        } else {
            (0, 0)
        };
        let frame_diag = crate::diagnostics::FrameDiagnosticsSnapshot::capture(
            crate::diagnostics::FrameDiagnosticsSnapshotCapture {
                main_tab: tab,
                host,
                last_submit_render_task_count: self.diagnostics.last_submit_render_task_count,
                pending_camera_readbacks: self.diagnostics.pending_camera_readbacks,
                completed_camera_readbacks: self.diagnostics.completed_camera_readbacks,
                failed_camera_readbacks: self.diagnostics.failed_camera_readbacks,
                backend: &backend_diag,
                ipc: crate::diagnostics::FrameDiagnosticsIpcQueues {
                    ipc_primary_outbound_drop_this_tick: self
                        .frontend
                        .ipc_outbound_primary_drop_this_tick(),
                    ipc_background_outbound_drop_this_tick: self
                        .frontend
                        .ipc_outbound_background_drop_this_tick(),
                    ipc_primary_consecutive_fail_streak: ipc_pri_str,
                    ipc_background_consecutive_fail_streak: ipc_bg_str,
                },
                xr: crate::diagnostics::XrRecoverableFailureCounts {
                    xr_wait_frame_failures: self.xr_stats.wait_frame_failures,
                    xr_locate_views_failures: self.xr_stats.locate_views_failures,
                },
                allocator: crate::diagnostics::GpuAllocatorHudRefresh {
                    gpu_allocator_totals: self.diagnostics.allocator_report_totals,
                    gpu_allocator_report: {
                        note_snapshot_clone();
                        self.diagnostics.allocator_report_hud.clone()
                    },
                    gpu_allocator_report_next_refresh_in_secs: next_refresh_in_secs,
                },
                frame_submit_apply_failures: self.diagnostics.frame_submit_apply_failures,
                unhandled_ipc_command_event_total: self.unhandled_ipc_command_event_total(),
            },
        );
        if tab == DebugHudMainTab::Stats {
            note_settings_lock();
            let msaa_requested = self
                .config
                .settings
                .read()
                .map(|s| s.rendering.msaa.as_count())
                .unwrap_or(1);
            let snapshot = crate::diagnostics::RendererInfoSnapshot::capture(
                crate::diagnostics::RendererInfoSnapshotCapture {
                    ipc_connected: self.is_ipc_connected(),
                    init_state: self.init_state(),
                    last_frame_index: self.last_frame_index(),
                    adapter_info: gpu.adapter_info(),
                    gpu_limits: gpu.limits().as_ref(),
                    surface_format: gpu.config_format(),
                    viewport_px: gpu.surface_extent_px(),
                    present_mode: gpu.present_mode(),
                    scene: &self.scene,
                    backend: &backend_diag,
                    gpu,
                    msaa_requested_samples: msaa_requested,
                },
            );
            self.backend.set_debug_hud_snapshot(snapshot);
        }
        self.backend.set_debug_hud_frame_diagnostics(frame_diag);
    }

    /// Copies debug HUD capture flags into the backend before the render graph runs.
    pub(crate) fn sync_debug_hud_diagnostics_from_settings(&mut self) {
        note_settings_lock();
        let mut flags = self
            .config
            .settings
            .read()
            .map(|s| debug_hud_metric_interest(&s))
            .unwrap_or_default();
        if super::frame_bench_draw_stats() {
            flags.force_world_mesh_draw_stats = true;
        }
        self.backend
            .set_debug_hud_per_view_config(flags.per_view_hud_config());
        self.backend
            .set_debug_hud_capture_graph_command_diagnostics(flags.wants_graph());
        self.backend
            .clear_debug_hud_current_view_texture_2d_asset_ids();
    }

    /// Updates debug HUD snapshots after [`crate::gpu::GpuContext::end_frame_timing`] for the winit tick.
    pub fn capture_debug_hud_after_frame_end(&mut self, gpu: &GpuContext) {
        self.capture_debug_hud_frame(Some(gpu));
    }

    /// Same capture as [`Self::capture_debug_hud_after_frame_end`], with `gpu` optional so the
    /// hidden-HUD counter test can drive the clear path without a device.
    ///
    /// Production always passes [`Some`]. Branches that sample the device no-op when `gpu` is
    /// [`None`]; the hidden path (`imgui_visible = false`) never enters those branches.
    pub(crate) fn capture_debug_hud_frame(&mut self, gpu: Option<&GpuContext>) {
        profiling::scope!("hud::capture_snapshot");
        let wall_ms = self.backend.debug_frame_time_ms();
        note_settings_lock();
        let flags = self
            .config
            .settings
            .read()
            .map(|s| debug_hud_metric_interest(&s))
            .unwrap_or_default();
        let now = Instant::now();
        self.capture_allocator_interest(gpu, flags, now);
        self.capture_frame_timing_interest(gpu, flags, wall_ms, now);
        self.capture_main_debug_interest(gpu, flags, now);
        self.capture_scene_transforms_interest(flags, now);
        self.capture_textures_interest(flags, now);
    }

    fn capture_allocator_interest(
        &mut self,
        gpu: Option<&GpuContext>,
        flags: DebugHudMetricInterest,
        now: Instant,
    ) {
        if !flags.wants_allocator_totals() {
            self.diagnostics.clear_allocator_report();
            return;
        }
        let Some(gpu) = gpu else {
            return;
        };
        self.diagnostics
            .refresh_gpu_allocator_report_hud(gpu, now, flags.wants_gpu_memory());
        if !flags.wants_gpu_memory() {
            self.diagnostics.clear_allocator_report_detail();
        }
    }

    fn capture_frame_timing_interest(
        &mut self,
        gpu: Option<&GpuContext>,
        flags: DebugHudMetricInterest,
        wall_ms: f64,
        now: Instant,
    ) {
        if !flags.frame_timing {
            self.backend.clear_debug_hud_frame_timing();
            return;
        }
        let Some(gpu) = gpu else {
            return;
        };
        let host = self.diagnostics.host_hud.snapshot();
        let frame_timing = crate::diagnostics::FrameTimingHudSnapshot::capture(
            crate::diagnostics::FrameTimingHudCapture {
                gpu,
                wall_frame_time_ms: wall_ms,
                host_frame_begin_to_submit: self.last_frame_begin_to_submit(),
                host_hud: &host,
                gpu_allocator: self.diagnostics.allocator_report_totals,
                history: &mut self.diagnostics.frame_time_history,
                ema: &mut self.diagnostics.frame_timing_ema,
                now,
            },
        );
        self.backend.set_debug_hud_frame_timing(frame_timing);
    }

    fn capture_main_debug_interest(
        &mut self,
        gpu: Option<&GpuContext>,
        flags: DebugHudMetricInterest,
        now: Instant,
    ) {
        if flags.wants_gpu_passes() {
            self.backend.clear_debug_hud_stats_snapshots();
            self.diagnostics.clear_main_hud_snapshot_timer();
            let Some(gpu) = gpu else {
                return;
            };
            let gpu_profiler_snapshot = gpu
                .latest_gpu_profiler_snapshot_handle()
                .lock()
                .map(|guard| {
                    note_snapshot_clone();
                    guard.clone()
                })
                .unwrap_or_default();
            self.backend
                .set_debug_hud_gpu_profiler_snapshot(gpu_profiler_snapshot);
            return;
        }
        if flags.wants_main_debug() {
            self.backend.clear_debug_hud_gpu_profiler_snapshot();
            if let Some(tab) = flags.main_tab
                && self.diagnostics.should_refresh_main_hud_snapshot(
                    now,
                    tab,
                    flags.main_hud_snapshot_signature(),
                )
                && let Some(gpu) = gpu
            {
                self.capture_main_debug_hud_tab(gpu, now, flags, tab);
            }
            return;
        }
        self.backend.clear_debug_hud_stats_snapshots();
        self.diagnostics.clear_main_hud_snapshot_timer();
        if flags.frame_timing {
            self.diagnostics.clear_allocator_report_detail();
        } else {
            self.diagnostics.clear_allocator_report();
        }
    }

    fn capture_scene_transforms_interest(&mut self, flags: DebugHudMetricInterest, now: Instant) {
        if flags.scene_transforms
            && self
                .diagnostics
                .should_refresh_scene_transforms_snapshot(now)
        {
            profiling::scope!("hud::capture_scene_transforms_snapshot");
            let scene_transforms =
                crate::diagnostics::SceneTransformsSnapshot::capture(&self.scene);
            self.backend
                .set_debug_hud_scene_transforms_snapshot(scene_transforms);
        } else if !flags.scene_transforms {
            self.backend.clear_debug_hud_scene_transforms_snapshot();
            self.diagnostics.clear_scene_transforms_snapshot_timer();
        }
    }

    fn capture_textures_interest(&mut self, flags: DebugHudMetricInterest, now: Instant) {
        if flags.textures && self.diagnostics.should_refresh_texture_debug_snapshot(now) {
            profiling::scope!("hud::capture_texture_debug_snapshot");
            let textures = crate::diagnostics::TextureDebugSnapshot::capture(
                self.backend.texture_pool(),
                self.backend.debug_hud_current_view_texture_2d_asset_ids(),
            );
            self.backend.set_debug_hud_texture_debug_snapshot(textures);
        } else if !flags.textures {
            self.backend.clear_debug_hud_texture_debug_snapshot();
            self.diagnostics.clear_texture_debug_snapshot_timer();
        }
    }

    /// Hidden-HUD encode early-out shared with [`Self::encode_debug_hud_overlay_on_surface`].
    ///
    /// Returns `true` when nothing visible would draw. Callers skip command-encoder creation.
    /// The hidden counter test drives this without a device; H7 un-ignores that test.
    pub(crate) fn skip_debug_hud_overlay_encode(&mut self) -> bool {
        if self.backend.debug_hud_has_visible_content() {
            false
        } else {
            self.backend.clear_debug_hud_input_capture();
            true
        }
    }

    /// Encodes the Dear ImGui debug overlay onto an acquired swapchain view (e.g. after the VR mirror blit).
    ///
    /// Uses the same composite path as the desktop render graph (`LoadOp::Load`). Caller must keep
    /// [`Self::set_debug_hud_frame_data`] in sync for this tick before encoding.
    pub(crate) fn encode_debug_hud_overlay_on_surface(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        backbuffer: &wgpu::TextureView,
    ) -> Result<(), DebugHudEncodeError> {
        if self.skip_debug_hud_overlay_encode() {
            return Ok(());
        }
        let device = gpu.device().as_ref();
        let extent = gpu.surface_extent_px();
        let q = gpu.queue().as_ref();
        self.backend.encode_debug_hud_overlay(
            device,
            q,
            encoder,
            backbuffer,
            extent,
            gpu.gpu_profiler(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::debug_hud_metric_interest;
    use crate::config::{DebugHudMainTab, RendererSettings};
    use crate::diagnostics::DebugHudMetricInterest;

    #[test]
    fn metric_interest_requires_visible_imgui() {
        let mut settings = RendererSettings::default();
        settings.debug.debug_hud_enabled = true;
        settings.debug.debug_hud_transforms = true;
        settings.debug.debug_hud_textures = true;
        settings.debug.hud.imgui_visible = false;

        assert_eq!(
            debug_hud_metric_interest(&settings),
            DebugHudMetricInterest::default()
        );
    }

    #[test]
    fn scene_transform_capture_ignores_retained_open_flag() {
        let mut settings = RendererSettings::default();
        settings.debug.debug_hud_transforms = true;
        settings.debug.hud.scene_transforms_open = false;

        assert!(debug_hud_metric_interest(&settings).scene_transforms);
    }

    #[test]
    fn texture_capture_ignores_retained_open_flag() {
        let mut settings = RendererSettings::default();
        settings.debug.debug_hud_textures = true;
        settings.debug.hud.texture_debug_open = false;

        assert!(debug_hud_metric_interest(&settings).textures);
    }

    #[test]
    fn main_tab_interest_follows_selected_open_tab_only() {
        let mut settings = RendererSettings::default();
        settings.debug.debug_hud_enabled = true;
        settings.debug.hud.main_tab = DebugHudMainTab::GpuPasses;

        let interest = debug_hud_metric_interest(&settings);

        assert_eq!(interest.main_tab, Some(DebugHudMainTab::GpuPasses));
        assert!(interest.wants_gpu_passes());
        assert!(!interest.wants_gpu_memory());
    }
}

#[cfg(test)]
mod hidden_zero_work {
    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};

    use crate::config::RendererSettings;
    use crate::diagnostics::hud_measure::{HudMeasureView, HudUi};
    use crate::diagnostics::{
        frame_hud_input, lock_hud_work_counters, reset_hud_work_counters,
        snapshot_hud_work_counters,
    };
    use crate::frontend::input::WindowInputAccumulator;

    use super::super::RendererRuntime;

    const HIDDEN_FRAMES: u32 = 300;

    /// H7 un-ignores this. Master still builds HUD input on hidden frames
    /// (`DebugHudInput::from_winit` via `frame_hud_input` in `app/driver/frame.rs`), so
    /// `input_builds` is non-zero. `settings_locks` stay above zero until H8 and are not asserted.
    #[test]
    #[ignore = "H7 un-ignores this: hidden frames still build DebugHudInput::from_winit (app/driver/frame.rs)"]
    fn hidden_frames_do_no_hud_work() {
        let _guard = lock_hud_work_counters();
        reset_hud_work_counters();
        let mut open = HudUi::from_measure_fixture(HudMeasureView::Stats);
        for _ in 0..3 {
            open.run_measure_frame();
        }
        let mut acc = WindowInputAccumulator::default();
        let _open_input = frame_hud_input(true, 1.0, &mut acc);
        let open_counts = snapshot_hud_work_counters();
        assert!(open_counts.ui_frames > 0);
        assert!(open_counts.draw_lists > 0);
        assert!(open_counts.fmt_calls > 0);
        assert!(open_counts.settings_locks > 0);
        assert!(open_counts.input_builds > 0);
        drop(open);

        reset_hud_work_counters();
        let mut settings = RendererSettings::default();
        settings.debug.hud.imgui_visible = false;
        let mut runtime = RendererRuntime::new(
            None,
            Arc::new(RwLock::new(settings)),
            PathBuf::from("hud-hidden-counter.toml"),
        );
        let mut acc = WindowInputAccumulator::default();
        for _ in 0..HIDDEN_FRAMES {
            let input = frame_hud_input(false, 1.0, &mut acc);
            runtime.set_debug_hud_input(input);
            runtime.sync_debug_hud_diagnostics_from_settings();
            runtime.capture_debug_hud_frame(None);
            assert!(runtime.skip_debug_hud_overlay_encode());
        }

        let counts = snapshot_hud_work_counters();
        assert!(
            counts.ui_frames == 0
                && counts.draw_lists == 0
                && counts.encodes == 0
                && counts.snapshot_clones == 0
                && counts.fmt_calls == 0
                && counts.input_builds == 0,
            "hidden HUD still did work over {HIDDEN_FRAMES} frames: \
             ui_frames={} draw_lists={} encodes={} snapshot_clones={} fmt_calls={} input_builds={} \
             settings_locks={}. input_builds is non-zero because app/driver/frame.rs still builds \
             DebugHudInput::from_winit on every frame (frame_hud_input ignores hud_visible until H7). \
             settings_locks are residual until H8 and are not part of this assert.",
            counts.ui_frames,
            counts.draw_lists,
            counts.encodes,
            counts.snapshot_clones,
            counts.fmt_calls,
            counts.input_builds,
            counts.settings_locks,
        );
    }
}
