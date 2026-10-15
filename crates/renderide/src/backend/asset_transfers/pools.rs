//! Resident GPU asset pools owned by the asset-transfer facade.

use crate::gpu_pools::{
    CubemapPool, MeshPool, RenderTexturePool, Texture3dPool, TexturePool, VideoTexturePool,
};

/// GPU-resident pools populated by host asset uploads.
pub(crate) struct ResidentAssetPools {
    /// Resident meshes.
    pub(crate) mesh_pool: MeshPool,
    /// Resident 2D textures.
    pub(crate) texture_pool: TexturePool,
    /// Resident 3D textures.
    pub(crate) texture3d_pool: Texture3dPool,
    /// Resident cubemaps.
    pub(crate) cubemap_pool: CubemapPool,
    /// Resident host render textures.
    pub(crate) render_texture_pool: RenderTexturePool,
    /// Resident video texture placeholders and decoded frame views.
    pub(crate) video_texture_pool: VideoTexturePool,
}

impl Default for ResidentAssetPools {
    fn default() -> Self {
        Self {
            mesh_pool: MeshPool::default_pool(),
            texture_pool: TexturePool::default_pool(),
            texture3d_pool: Texture3dPool::default_pool(),
            cubemap_pool: CubemapPool::default_pool(),
            render_texture_pool: RenderTexturePool::new(),
            video_texture_pool: VideoTexturePool::new(),
        }
    }
}
