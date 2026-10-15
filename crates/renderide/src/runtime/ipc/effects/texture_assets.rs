//! Texture-asset and render-texture command effects on [`RendererRuntime`].

use crate::frontend::dispatch::command_dispatch::RunningCommandEffect;
use crate::shared::{
    SetCubemapData, SetCubemapFormat, SetCubemapProperties, SetTexture2DData, SetTexture2DFormat,
    SetTexture2DProperties, SetTexture3DData, SetTexture3DFormat, SetTexture3DProperties,
};

use super::super::super::RendererRuntime;

impl RendererRuntime {
    pub(in crate::runtime) fn apply_texture_asset_effect(&mut self, effect: RunningCommandEffect) {
        match effect {
            RunningCommandEffect::SetTexture2DFormat(f) => self.dispatch_texture_2d_format(f),
            RunningCommandEffect::SetTexture2DProperties(p) => {
                self.dispatch_texture_2d_properties(p);
            }
            RunningCommandEffect::SetTexture2DData(d) => self.dispatch_texture_2d_data(d),
            RunningCommandEffect::UnloadTexture2D(u) => self.backend.on_unload_texture_2d(u),
            RunningCommandEffect::SetTexture3DFormat(f) => self.dispatch_texture_3d_format(f),
            RunningCommandEffect::SetTexture3DProperties(p) => {
                self.dispatch_texture_3d_properties(p);
            }
            RunningCommandEffect::SetTexture3DData(d) => self.dispatch_texture_3d_data(d),
            RunningCommandEffect::UnloadTexture3D(u) => self.backend.on_unload_texture_3d(u),
            RunningCommandEffect::SetCubemapFormat(f) => self.dispatch_cubemap_format(f),
            RunningCommandEffect::SetCubemapProperties(p) => self.dispatch_cubemap_properties(p),
            RunningCommandEffect::SetCubemapData(d) => self.dispatch_cubemap_data(d),
            RunningCommandEffect::UnloadCubemap(u) => self.backend.on_unload_cubemap(u),
            RunningCommandEffect::SetRenderTextureFormat(f) => self
                .backend
                .on_set_render_texture_format(f, self.frontend.ipc_mut()),
            RunningCommandEffect::UnloadRenderTexture(u) => {
                self.backend.on_unload_render_texture(u);
            }
            _ => {}
        }
    }

    fn dispatch_texture_2d_format(&mut self, f: SetTexture2DFormat) {
        self.backend
            .on_set_texture_2d_format(f, self.frontend.ipc_mut());
    }

    fn dispatch_texture_2d_properties(&mut self, p: SetTexture2DProperties) {
        self.backend
            .on_set_texture_2d_properties(p, self.frontend.ipc_mut());
    }

    fn dispatch_texture_2d_data(&mut self, d: SetTexture2DData) {
        let (shm, ipc) = self.frontend.transport_pair_mut();
        self.backend.on_set_texture_2d_data(d, shm, ipc);
    }

    fn dispatch_texture_3d_format(&mut self, f: SetTexture3DFormat) {
        self.backend
            .on_set_texture_3d_format(f, self.frontend.ipc_mut());
    }

    fn dispatch_texture_3d_properties(&mut self, p: SetTexture3DProperties) {
        self.backend
            .on_set_texture_3d_properties(p, self.frontend.ipc_mut());
    }

    fn dispatch_texture_3d_data(&mut self, d: SetTexture3DData) {
        let (shm, ipc) = self.frontend.transport_pair_mut();
        self.backend.on_set_texture_3d_data(d, shm, ipc);
    }

    fn dispatch_cubemap_format(&mut self, f: SetCubemapFormat) {
        self.backend
            .on_set_cubemap_format(f, self.frontend.ipc_mut());
    }

    fn dispatch_cubemap_properties(&mut self, p: SetCubemapProperties) {
        self.backend
            .on_set_cubemap_properties(p, self.frontend.ipc_mut());
    }

    fn dispatch_cubemap_data(&mut self, d: SetCubemapData) {
        let (shm, ipc) = self.frontend.transport_pair_mut();
        self.backend.on_set_cubemap_data(d, shm, ipc);
    }
}
