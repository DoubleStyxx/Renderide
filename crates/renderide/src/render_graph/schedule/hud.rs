//! Debug HUD snapshot for compiled render-graph schedules.

use super::{FrameSchedule, ScheduleStep};

/// CPU-side snapshot of a [`FrameSchedule`] for the debug HUD.
///
/// Captured once per graph build/rebuild and surfaced in the diagnostics overlay so developers
/// can see pass count, wave layout, and phase distribution at a glance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScheduleHudSnapshot {
    /// Total retained pass count.
    pub pass_count: usize,
    /// Total Kahn waves.
    pub wave_count: usize,
    /// Number of [`crate::render_graph::pass::PassPhase::FrameGlobal`] passes.
    pub frame_global_count: usize,
    /// Number of [`crate::render_graph::pass::PassPhase::PerView`] passes.
    pub per_view_count: usize,
    /// Pass count per wave (`waves[w].len()`).
    pub passes_per_wave: Vec<usize>,
    /// Retained dependency edge count between scheduled steps.
    pub dependency_edge_count: usize,
    /// Conservative merge groups detected at compile time.
    pub render_pass_merge_group_count: usize,
    /// Merge groups planned for materialized recording.
    pub render_pass_materialization_group_count: usize,
    /// Scheduler units that can record in a worker batch.
    pub parallel_recording_unit_count: usize,
    /// Scheduler batches that record more than one unit in parallel.
    pub parallel_recording_batch_count: usize,
}

impl ScheduleHudSnapshot {
    /// Builds a snapshot from a [`FrameSchedule`].
    pub fn from_schedule(schedule: &FrameSchedule) -> Self {
        Self {
            pass_count: schedule.pass_count(),
            wave_count: schedule.wave_count(),
            frame_global_count: schedule.frame_global_steps().count(),
            per_view_count: schedule.per_view_steps().count(),
            passes_per_wave: schedule.wave_steps().map(<[ScheduleStep]>::len).collect(),
            dependency_edge_count: schedule.dependency_edges.len(),
            render_pass_merge_group_count: schedule.render_pass_merge_groups.len(),
            render_pass_materialization_group_count: schedule
                .render_pass_materialization_plan
                .groups
                .len(),
            parallel_recording_unit_count: schedule.recording_plan.parallel_unit_count(),
            parallel_recording_batch_count: schedule.recording_plan.parallel_batch_count(),
        }
    }
}
