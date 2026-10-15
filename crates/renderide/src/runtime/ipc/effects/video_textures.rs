//! Video-texture command effects on [`RendererRuntime`].

use crate::frontend::dispatch::command_dispatch::RunningCommandEffect;

use super::super::super::RendererRuntime;

impl RendererRuntime {
    pub(in crate::runtime) fn apply_video_texture_effect(&mut self, effect: RunningCommandEffect) {
        match effect {
            RunningCommandEffect::VideoTextureLoad(l) => self.backend.on_video_texture_load(l),
            RunningCommandEffect::VideoTextureUpdate(u) => self.backend.on_video_texture_update(u),
            RunningCommandEffect::VideoTextureProperties(p) => {
                self.backend.on_video_texture_properties(p);
            }
            RunningCommandEffect::VideoTextureStartAudioTrack(s) => {
                self.backend.on_video_texture_start_audio_track(s);
            }
            RunningCommandEffect::UnloadVideoTexture(u) => self.backend.on_unload_video_texture(u),
            _ => {}
        }
    }
}
