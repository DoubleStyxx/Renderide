//! Playback clock math and GStreamer time conversion for video textures.

use renderide_shared::VideoTextureUpdate;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum tolerated seek drift while video is actively playing.
const PLAYING_SEEK_DRIFT_SECONDS: f64 = 1.0;

/// Maximum tolerated seek drift while video is paused.
const PAUSED_SEEK_DRIFT_SECONDS: f64 = 0.01;

/// Host-visible interpretation of the current media duration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MediaDuration {
    /// Seekable media with a positive finite duration in seconds.
    Finite {
        /// Duration in seconds.
        seconds: f64,
    },
    /// Stream-like media without a stable timeline length.
    Stream,
}

impl MediaDuration {
    /// Converts an optional GStreamer clock duration into host-visible duration semantics.
    pub(super) fn from_clock_time(duration: Option<gstreamer::ClockTime>) -> Self {
        match duration {
            Some(duration) if duration != gstreamer::ClockTime::ZERO => Self::Finite {
                seconds: duration.nseconds() as f64 / 1_000_000_000.0,
            },
            _ => Self::Stream,
        }
    }

    /// Returns the length value sent through `VideoTextureReady`.
    pub(super) fn ready_length_seconds(self) -> f64 {
        match self {
            Self::Finite { seconds } => seconds,
            Self::Stream => f64::INFINITY,
        }
    }

    /// Returns whether traditional timeline seeking is valid for this media.
    pub(super) fn supports_timeline_seeking(self) -> bool {
        matches!(self, Self::Finite { .. })
    }

    /// Returns whether this media should report `VideoTextureClockErrorState`.
    pub(super) fn reports_clock_error(self) -> bool {
        self.supports_timeline_seeking()
    }
}

/// Returns the pipeline state implied by the host update.
pub(super) fn target_state_for_update(update: &VideoTextureUpdate) -> gstreamer::State {
    if update.play {
        gstreamer::State::Playing
    } else {
        gstreamer::State::Paused
    }
}

/// Returns how far the current playback position may drift before seeking.
pub(super) fn max_seek_drift_seconds(update: &VideoTextureUpdate) -> f64 {
    if update.play {
        PLAYING_SEEK_DRIFT_SECONDS
    } else {
        PAUSED_SEEK_DRIFT_SECONDS
    }
}

/// Returns `true` when GStreamer should seek to the host clock position.
pub(super) fn should_seek_to_host_position(
    current_seconds: f64,
    update: &VideoTextureUpdate,
) -> bool {
    (current_seconds - update.position).abs() > max_seek_drift_seconds(update)
}

/// Converts host seconds to a bounded GStreamer clock time.
pub(super) fn clock_time_from_seconds(seconds: f64) -> gstreamer::ClockTime {
    if !seconds.is_finite() || seconds <= 0.0 {
        return gstreamer::ClockTime::ZERO;
    }
    let max_nanos = gstreamer::ClockTime::MAX.nseconds();
    let requested_nanos = seconds * 1_000_000_000.0;
    let nanos = if requested_nanos >= max_nanos as f64 {
        max_nanos
    } else {
        requested_nanos as u64
    };
    gstreamer::ClockTime::from_nseconds(nanos)
}

/// Returns the host-expected playback position right now, given the last received update.
///
/// Host-expected `AdjustedPosition` calculation:
/// `position + (now - decoded_time).total_seconds()`, with no play-state guard.
pub(super) fn adjusted_host_position(update: &VideoTextureUpdate, now_nanos: i128) -> f64 {
    let elapsed_nanos = now_nanos - update.decoded_time;
    update.position + (elapsed_nanos as f64) / 1_000_000_000.0
}

/// Returns the current wall-clock time as nanoseconds since the UNIX epoch.
pub(super) fn unix_nanos_now() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}
