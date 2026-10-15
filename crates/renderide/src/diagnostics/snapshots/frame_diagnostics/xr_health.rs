//! Recoverable OpenXR error counts fragment of [`super::FrameDiagnosticsSnapshot`].
//!
//! [`XrRecoverableFailureCounts`] doubles as the capture-input type and the snapshot fragment
//! storage; capture is the identity transform.

/// Cumulative recoverable OpenXR errors surfaced on the Frame diagnostics HUD.
#[derive(Clone, Copy, Debug, Default)]
pub struct XrRecoverableFailureCounts {
    /// Cumulative OpenXR `wait_frame` errors (recoverable).
    pub xr_wait_frame_failures: u64,
    /// Cumulative OpenXR `locate_views` errors while rendering was expected (recoverable).
    pub xr_locate_views_failures: u64,
}
