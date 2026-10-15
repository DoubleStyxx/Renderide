//! Test-only builders shared by `gpu_pools` unit tests.

use glam::IVec2;

use crate::shared::{SetRenderTextureFormat, TextureFilterMode, TextureWrapMode};

/// Builds a host render-texture format row with the supplied wrap modes.
pub(crate) fn render_texture_format(
    wrap_u: TextureWrapMode,
    wrap_v: TextureWrapMode,
) -> SetRenderTextureFormat {
    SetRenderTextureFormat {
        asset_id: 42,
        size: IVec2::new(128, 64),
        depth: 24,
        filter_mode: TextureFilterMode::Bilinear,
        aniso_level: 8,
        wrap_u,
        wrap_v,
    }
}
