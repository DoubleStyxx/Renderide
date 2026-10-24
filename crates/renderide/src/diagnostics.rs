//! Dear ImGui diagnostics: **Frame timing** ([`crate::config::DebugSettings::debug_hud_frame_timing`]),
//! **Feedback / Bug Report** ([`crate::config::DebugSettings::debug_hud_links`]),
//! **Renderide debug** ([`crate::config::DebugSettings::debug_hud_enabled`]: Stats / Visibility / Graph / Assets / Shader routes / Draw state / GPU memory / GPU passes),
//! **Scene transforms** ([`crate::config::DebugSettings::debug_hud_transforms`]),
//! and **Textures** ([`crate::config::DebugSettings::debug_hud_textures`]).
//!
//! Also hosts the cooperative renderer hang/hitch detector ([`Watchdog`]).

mod hud;
pub(crate) mod log_once;
mod per_view;
mod snapshots;
mod watchdog;

pub(crate) use hud::DebugHudOverlayContext;
pub(crate) use hud::input::frame_hud_input;
pub(crate) use hud::work_counters::{note_settings_lock, note_snapshot_clone};
#[cfg(test)]
pub(crate) use hud::work_counters::{
    reset as reset_hud_work_counters, snapshot as snapshot_hud_work_counters,
    test_lock as lock_hud_work_counters,
};
pub use hud::{
    DebugHud, DebugHudInput, DebugHudMetricInterest, sanitize_input_state_for_imgui_host,
};

/// CPU HUD entry points for the steady-state allocation test.
///
/// Not part of the renderer process API. The integration test drives [`HudUi`] with fixed
/// fixtures and no wgpu device.
#[doc(hidden)]
pub mod hud_measure {
    pub use super::hud::{HudMeasureView, HudUi};
}
pub use snapshots::{
    AssetDiagnosticsSnapshot, BackendDiagSnapshot, FrameDiagnosticsIpcQueues,
    FrameDiagnosticsSnapshot, FrameDiagnosticsSnapshotCapture, FrameTimeHistory, FrameTimingEma,
    FrameTimingHudCapture, FrameTimingHudSnapshot, FrameTimingOnePercentStats,
    FrameUploadArenaSnapshot, GpuAllocatorHud, GpuAllocatorHudRefresh, GpuAllocatorReportHud,
    HostHudGatherer, LightDiagnosticsSnapshot, RendererInfoSnapshot, RendererInfoSnapshotCapture,
    SceneTransformsSnapshot, ShaderRouteSnapshot, TextureDebugSnapshot, XrRecoverableFailureCounts,
};
pub use watchdog::{Heartbeat, Watchdog, WatchdogPause};
