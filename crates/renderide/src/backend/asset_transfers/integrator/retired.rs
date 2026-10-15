//! Retired GPU resources held alive across delayed-removal updates.

use crate::assets::mesh::GpuMesh;
use crate::gpu_pools::{
    GpuCubemap, GpuRenderTexture, GpuResource, GpuTexture2d, GpuTexture3d, GpuVideoTexture,
};

/// One retired GPU resource kept alive for the delayed-removal window.
#[derive(Debug)]
pub enum RetiredAssetResource {
    /// Removed resident mesh.
    Mesh(Box<GpuMesh>),
    /// Removed resident Texture2D.
    Texture2d(GpuTexture2d),
    /// Removed resident Texture3D.
    Texture3d(GpuTexture3d),
    /// Removed resident cubemap.
    Cubemap(GpuCubemap),
    /// Removed resident render texture.
    RenderTexture(GpuRenderTexture),
    /// Removed resident video texture.
    VideoTexture(GpuVideoTexture),
}

impl RetiredAssetResource {
    pub(super) fn resident_bytes(&self) -> u64 {
        match self {
            Self::Mesh(resource) => resource.resident_bytes(),
            Self::Texture2d(resource) => resource.resident_bytes(),
            Self::Texture3d(resource) => resource.resident_bytes(),
            Self::Cubemap(resource) => resource.resident_bytes(),
            Self::RenderTexture(resource) => resource.resident_bytes(),
            Self::VideoTexture(resource) => resource.resident_bytes(),
        }
    }
}
