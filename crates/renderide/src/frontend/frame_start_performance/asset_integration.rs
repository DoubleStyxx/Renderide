//! Asset-integration counters and per-tick sample accumulated into the next
//! outgoing [`crate::shared::PerformanceState`].
//!
//! [`AssetIntegrationPerformanceSample`] is the cooperative drain shape the
//! runtime hands to [`super::state::FrameStartPerformanceState::record_asset_integration_stats`];
//! [`AssetIntegrationPerformanceState`] is the running accumulator emitted on
//! the next frame-start build.

use std::time::Duration;

/// Asset-integration counters accumulated until the next outgoing performance payload.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AssetIntegrationPerformanceState {
    pub(crate) integration_processing_time: f32,
    pub(crate) extra_particle_processing_time: f32,
    pub(crate) processed_asset_integrator_tasks: i32,
    pub(crate) integration_high_priority_tasks: i32,
    pub(crate) integration_tasks: i32,
    pub(crate) integration_render_tasks: i32,
    pub(crate) integration_particle_tasks: i32,
    pub(crate) processing_handle_waits: i32,
}

impl AssetIntegrationPerformanceState {
    /// Folds one cooperative drain into the running accumulator.
    pub(crate) fn accumulate(&mut self, sample: AssetIntegrationPerformanceSample) {
        self.integration_processing_time += sample.integration_elapsed.as_secs_f32();
        self.extra_particle_processing_time += sample.particle_elapsed.as_secs_f32();
        self.processed_asset_integrator_tasks = self
            .processed_asset_integrator_tasks
            .saturating_add(i32::try_from(sample.processed_tasks).unwrap_or(i32::MAX));
        self.integration_high_priority_tasks = self
            .integration_high_priority_tasks
            .saturating_add(i32::try_from(sample.high_priority_tasks).unwrap_or(i32::MAX));
        self.integration_tasks = self
            .integration_tasks
            .saturating_add(i32::try_from(sample.normal_priority_tasks).unwrap_or(i32::MAX));
        self.integration_render_tasks = self
            .integration_render_tasks
            .saturating_add(i32::try_from(sample.render_tasks).unwrap_or(i32::MAX));
        self.integration_particle_tasks = self
            .integration_particle_tasks
            .saturating_add(i32::try_from(sample.particle_tasks).unwrap_or(i32::MAX));
        self.processing_handle_waits = self
            .processing_handle_waits
            .saturating_add(sample.handle_waits);
    }

    /// Increments the wake-wait counter while the renderer waits for host submit.
    pub(crate) fn note_handle_wait(&mut self) {
        self.processing_handle_waits = self.processing_handle_waits.saturating_add(1);
    }
}

/// One cooperative asset-integration sample to fold into the next performance payload.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AssetIntegrationPerformanceSample {
    /// Non-particle integration time.
    pub(crate) integration_elapsed: Duration,
    /// Extra particle-lane integration time.
    pub(crate) particle_elapsed: Duration,
    /// Processed asset-integrator queue steps.
    pub(crate) processed_tasks: u32,
    /// Remaining high-priority upload tasks.
    pub(crate) high_priority_tasks: usize,
    /// Remaining normal-priority upload tasks.
    pub(crate) normal_priority_tasks: usize,
    /// Remaining render-lane tasks.
    pub(crate) render_tasks: usize,
    /// Remaining particle-lane tasks.
    pub(crate) particle_tasks: usize,
    /// Integration wait count while awaiting host frame submit.
    pub(crate) handle_waits: i32,
}
