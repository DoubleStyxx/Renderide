//! Host-visible video ready/audio-track comparison helpers.

use renderide_shared::{VideoAudioTrack, VideoTextureReady};

/// Compares a `VideoTextureReady` message to another.
pub(super) fn video_texture_ready_eq(a: &VideoTextureReady, b: &VideoTextureReady) -> bool {
    a.asset_id == b.asset_id
        && a.has_alpha == b.has_alpha
        && a.instance_changed == b.instance_changed
        && a.size == b.size
        && a.length.to_bits() == b.length.to_bits()
        && a.playback_engine == b.playback_engine
        && video_audio_tracks_eq(&a.audio_tracks, &b.audio_tracks)
}

/// Compares audio track slices.
pub(super) fn video_audio_tracks_eq(a: &[VideoAudioTrack], b: &[VideoAudioTrack]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a_track, b_track)| video_audio_track_eq(a_track, b_track))
}

/// Compares a `VideoAudioTrack` to another.
pub(super) fn video_audio_track_eq(a: &VideoAudioTrack, b: &VideoAudioTrack) -> bool {
    a.sample_rate == b.sample_rate
        && a.index == b.index
        && a.name == b.name
        && a.language_code == b.language_code
        && a.channel_count == b.channel_count
}

#[cfg(test)]
mod tests {
    use glam::IVec2;

    use super::*;

    fn track(index: i32) -> VideoAudioTrack {
        VideoAudioTrack {
            index,
            channel_count: 2,
            sample_rate: 48_000,
            language_code: Some(String::from("en")),
            name: Some(format!("Track {index}")),
        }
    }

    fn ready(length: f64, tracks: Vec<VideoAudioTrack>) -> VideoTextureReady {
        VideoTextureReady {
            length,
            size: IVec2::new(320, 240),
            has_alpha: false,
            asset_id: 9,
            instance_changed: true,
            playback_engine: Some(String::from("test")),
            audio_tracks: tracks,
        }
    }

    #[test]
    fn ready_messages_compare_full_payload() {
        let a = ready(1.5, vec![track(0)]);
        let b = ready(1.5, vec![track(0)]);

        assert!(video_texture_ready_eq(&a, &b));
    }

    #[test]
    fn ready_messages_reject_size_and_alpha_differences() {
        let a = ready(1.5, Vec::new());
        let mut b = ready(1.5, Vec::new());
        b.size = IVec2::new(640, 480);
        assert!(!video_texture_ready_eq(&a, &b));

        b = ready(1.5, Vec::new());
        b.has_alpha = true;
        assert!(!video_texture_ready_eq(&a, &b));
    }

    #[test]
    fn audio_track_comparison_rejects_metadata_differences() {
        let a = track(0);
        let mut b = track(0);
        b.language_code = Some(String::from("ja"));
        assert!(!video_audio_track_eq(&a, &b));

        b = track(0);
        b.channel_count = 6;
        assert!(!video_audio_track_eq(&a, &b));
    }
}
