use std::sync::Arc;

use crate::gpu::GpuLimits;
use crate::skybox::ibl_cache::SkyboxIblKey;

pub(super) struct ReflectionProbeAtlas {
    pub(super) texture: Arc<wgpu::Texture>,
    pub(super) face_size: u32,
    pub(super) mip_levels: u32,
    pub(super) capacity: u16,
    pub(super) keys: Vec<Option<SkyboxIblKey>>,
}

pub(super) struct AtlasCopyJob {
    pub(super) slot: u16,
    pub(super) texture: Arc<wgpu::Texture>,
    pub(super) mip_levels: u32,
}

pub(super) fn max_atlas_slots(limits: &GpuLimits) -> u16 {
    (limits.max_texture_array_layers() / 6)
        .min(u32::from(u16::MAX))
        .max(1) as u16
}
