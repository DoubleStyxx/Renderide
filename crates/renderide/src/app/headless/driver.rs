//! Headless offscreen driver loop.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::crash_context::{self, RenderMode, TargetMode, TickPhase};
use crate::gpu::GpuContext;
use crate::ipc::HeadlessParams;
use crate::run_error::RunError;
use crate::runtime::{RendererRuntime, TickOutcome};
use crate::shared::InputState;

use super::super::bootstrap::{ExternalShutdownCoordinator, GpuStartupConfig};
use super::super::exit::RunExit;
use super::readback;
use super::schedule::{HeadlessSchedule, HeadlessTickKind};

/// When `1` or `true`, headless PNG writes also emit the atlas-pass sidecar and append-only log.
///
/// The scene harness sets this. A normal headless capture writes only the PNG.
const SHADOW_ATLAS_PASS_LOG_ENV: &str = "RENDERIDE_SHADOW_ATLAS_PASS_LOG";
/// Sleep granularity inside the headless tick loop.
const HEADLESS_TICK_SLEEP: Duration = Duration::from_millis(5);
/// Maximum time headless mode waits for cooperative resource shutdown.
const HEADLESS_GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Runs the renderer in headless offscreen mode until shutdown or fatal IPC.
pub(crate) fn run_headless(
    runtime: &mut RendererRuntime,
    params: HeadlessParams,
    external_shutdown: Option<ExternalShutdownCoordinator>,
    main_heartbeat: Option<crate::diagnostics::Heartbeat>,
    gpu_config: GpuStartupConfig,
) -> Result<RunExit, RunError> {
    let mut driver = HeadlessDriver::new(
        runtime,
        params,
        external_shutdown,
        main_heartbeat,
        gpu_config,
    )?;
    driver.run()
}

struct HeadlessDriver<'a> {
    runtime: &'a mut RendererRuntime,
    params: HeadlessParams,
    external_shutdown: Option<ExternalShutdownCoordinator>,
    main_heartbeat: Option<crate::diagnostics::Heartbeat>,
    gpu: GpuContext,
    schedule: HeadlessSchedule,
    frames_written: u64,
    /// Start of the tick currently being measured for the control API.
    #[cfg(feature = "control-api")]
    control_tick_started: Instant,
}

impl<'a> HeadlessDriver<'a> {
    fn new(
        runtime: &'a mut RendererRuntime,
        params: HeadlessParams,
        external_shutdown: Option<ExternalShutdownCoordinator>,
        main_heartbeat: Option<crate::diagnostics::Heartbeat>,
        gpu_config: GpuStartupConfig,
    ) -> Result<Self, RunError> {
        crate::debug_scene_color::set_readback_enabled(params.scene_color_readback);
        logger::info!(
            "Headless mode: output={} size={}x{} interval_ms={} scene_color_readback={}",
            params.output_path.display(),
            params.width,
            params.height,
            params.interval_ms,
            params.scene_color_readback,
        );
        crate::gpu::warp_black_diag::arm_frame_log(&params.output_path);
        crate::gpu::warp_light_probe::arm(&params.output_path);
        crate::gpu::warp_material_bind_probe::arm(&params.output_path);
        crash_context::set_target_mode(TargetMode::Headless);
        crash_context::set_render_mode(RenderMode::Headless);

        let gpu = pollster::block_on(GpuContext::new_headless(
            params.width,
            params.height,
            gpu_config.max_frame_latency,
            gpu_config.gpu_validation_layers,
            gpu_config.power_preference,
            gpu_config.graphics_api,
        ))
        .map_err(RunError::gpu)?;
        crate::gpu::warp_black_diag::configure_for_adapter(gpu.adapter_info());
        runtime.attach_gpu(&gpu);
        #[cfg(feature = "control-api")]
        runtime.publish_control_meta(&gpu);
        if crate::app::frame_bench::active() {
            crate::gpu::frame_bench_queries::install(gpu.device(), gpu.queue());
        }
        let schedule = HeadlessSchedule::new(params.interval_ms, Instant::now());

        Ok(Self {
            runtime,
            params,
            external_shutdown,
            main_heartbeat,
            gpu,
            schedule,
            frames_written: 0,
            #[cfg(feature = "control-api")]
            control_tick_started: Instant::now(),
        })
    }

    fn run(&mut self) -> Result<RunExit, RunError> {
        loop {
            if let Some(heartbeat) = self.main_heartbeat.as_ref() {
                heartbeat.pet();
            }
            if self.external_shutdown_requested() {
                logger::info!("Headless: external shutdown requested, exiting");
                self.drain_graceful_shutdown();
                break;
            }

            let now = Instant::now();
            crash_context::set_tick_phase(TickPhase::Headless);
            self.runtime.tick_frame_wall_clock_begin(now);
            let tick_kind = self.schedule.tick_kind(now);
            #[cfg(feature = "control-api")]
            {
                self.control_tick_started = Instant::now();
            }
            let outcome = self.run_tick_for_bench(tick_kind);
            let render_skipped = outcome.render_skipped;
            if let Some(exit) = self.handle_tick_outcome(outcome) {
                return Ok(exit);
            }
            if crate::app::frame_bench::active() && self.runtime.shutdown_requested() {
                logger::info!("Headless: frame bench report written, exiting");
                self.drain_graceful_shutdown();
                return Ok(RunExit::Clean);
            }
            self.after_tick(tick_kind, render_skipped);
        }

        Ok(RunExit::Clean)
    }

    fn external_shutdown_requested(&self) -> bool {
        self.external_shutdown
            .as_ref()
            .is_some_and(|coord| coord.requested.load(Ordering::Relaxed))
    }

    fn run_tick_for_bench(&mut self, kind: HeadlessTickKind) -> TickOutcome {
        let measure = crate::app::frame_bench::active() && kind == HeadlessTickKind::FullFrame;
        if measure {
            self.gpu.begin_frame_timing(Instant::now());
        }
        let outcome = self.run_tick(kind);
        if measure {
            crate::app::frame_bench::finish_headless_full_frame(&self.gpu, self.runtime, &outcome);
        }
        outcome
    }

    fn run_tick(&mut self, kind: HeadlessTickKind) -> TickOutcome {
        let inputs = self.tick_inputs();
        match kind {
            HeadlessTickKind::FullFrame => {
                profiling::scope!("headless::full_frame");
                let outcome = self.runtime.tick_one_frame(&mut self.gpu, inputs);
                self.schedule.complete_full_frame(Instant::now());
                outcome
            }
            HeadlessTickKind::LockstepOnly => {
                profiling::scope!("headless::lockstep_tick");
                self.runtime
                    .tick_one_frame_lockstep_only(Some(&mut self.gpu), inputs)
            }
        }
    }

    fn tick_inputs(&self) -> InputState {
        #[cfg(feature = "control-api")]
        {
            self.runtime.control_input_state()
        }
        #[cfg(not(feature = "control-api"))]
        {
            InputState::default()
        }
    }

    fn handle_tick_outcome(&mut self, outcome: TickOutcome) -> Option<RunExit> {
        if outcome.shutdown_requested {
            logger::info!("Headless: host shutdown requested, exiting");
            self.runtime
                .log_compact_renderer_summary("headless-host-shutdown");
            self.drain_graceful_shutdown();
            return Some(RunExit::Clean);
        }
        if outcome.fatal_error {
            logger::error!("Headless: fatal IPC error, exiting");
            self.runtime
                .log_compact_renderer_summary("headless-fatal-ipc");
            crate::profiling::flush_resource_churn_plots();
            crate::profiling::emit_frame_mark();
            return Some(RunExit::Code(4));
        }
        if let Some(err) = outcome.graph_error {
            let kind = crate::render_graph::graph_error_kind(&err);
            crash_context::set_last_graph_error(kind);
            logger::warn!("Headless: render graph error this tick: {err:?}");
        }
        None
    }

    fn drain_graceful_shutdown(&mut self) {
        profiling::scope!("headless::graceful_shutdown");
        self.gpu.wait_for_previous_present();
        self.runtime.begin_graceful_shutdown();

        let deadline = Instant::now() + HEADLESS_GRACEFUL_SHUTDOWN_TIMEOUT;
        while !self.runtime.graceful_shutdown_complete() {
            if Instant::now() >= deadline {
                logger::warn!(
                    "Headless graceful shutdown timed out after {}ms; exiting",
                    HEADLESS_GRACEFUL_SHUTDOWN_TIMEOUT.as_millis()
                );
                break;
            }
            std::thread::sleep(HEADLESS_TICK_SLEEP);
        }
    }

    fn after_tick(&mut self, kind: HeadlessTickKind, render_skipped: bool) {
        #[cfg(feature = "control-api")]
        self.note_control_presented(kind, render_skipped);
        if kind == HeadlessTickKind::FullFrame && !render_skipped {
            self.maybe_write_frame_png();
        } else {
            crate::gpu::warp_black_diag::discard_frame_log();
        }
        self.runtime
            .tick_frame_render_time_end(self.gpu.last_completed_gpu_render_time_seconds());
        crate::profiling::flush_resource_churn_plots();
        crate::profiling::emit_frame_mark();
        std::thread::sleep(HEADLESS_TICK_SLEEP);
    }

    #[cfg(feature = "control-api")]
    fn note_control_presented(&self, kind: HeadlessTickKind, render_skipped: bool) {
        let full_frame = kind == HeadlessTickKind::FullFrame;
        if !crate::control::headless_tick_presents(full_frame, render_skipped) {
            return;
        }
        let Some(session) = self.runtime.control_session() else {
            return;
        };
        let cpu_frame_ms = self.control_tick_started.elapsed().as_secs_f64() * 1000.0;
        crate::control::finish_presented_frame(
            &session,
            &self.gpu,
            cpu_frame_ms,
            crate::app::control_capture_readback,
        );
    }

    fn maybe_write_frame_png(&mut self) {
        crate::gpu::warp_black_diag::flush_frame_log();
        if crate::app::frame_bench::active() {
            return;
        }
        if self.gpu.headless_color_texture().is_none() {
            return;
        }
        let atlas_passes =
            shadow_atlas_pass_log_enabled().then(|| self.runtime.shadow_atlas_pass_counts());
        let pending_pipeline_draws = crate::gpu::warp_black_diag::pending_for_publish(
            self.gpu.adapter_info(),
            self.runtime.take_pending_pipeline_draws(),
        );
        if let Err(e) = readback::readback_and_write_png_atomically(
            &self.gpu,
            &self.params.output_path,
            atlas_passes,
            pending_pipeline_draws,
        ) {
            logger::warn!("Headless PNG write failed: {e}");
        } else {
            self.frames_written = self.frames_written.saturating_add(1);
            logger::trace!("Headless wrote PNG #{}", self.frames_written);
        }
    }
}

fn shadow_atlas_pass_log_enabled() -> bool {
    std::env::var(SHADOW_ATLAS_PASS_LOG_ENV)
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}
