//! Cache key and dynamic-offset binding for the embedded material uniform arena.

use std::num::NonZeroU64;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in super::super) struct MaterialUniformCacheKey {
    pub(in super::super) stem_hash: u64,
    pub(in super::super) material_asset_id: i32,
    pub(in super::super) property_block_slot0: Option<i32>,
    /// Renderer-level `MaterialPropertyBlock` id (applies to every material on the renderer).
    pub(in super::super) renderer_property_block_id: Option<i32>,
    pub(in super::super) texture_2d_asset_id: i32,
    pub(in super::super) shader_variant_bits: Option<u32>,
    /// Custom-shader source generation. Packaged stems stay at zero.
    pub(in super::super) shader_source_generation: u64,
    /// Fold of bound-texture residency, view/allocation generation, and content generation.
    ///
    /// Same value as the bind-group `texture_bind_signature`. A reupload or mip residency change
    /// must not reuse a slot packed against the previous stamp.
    pub(in super::super) texture_residency_generation: u64,
}

/// GPU arena slot selected for a material uniform block.
#[derive(Clone)]
pub(in super::super) struct MaterialUniformArenaSlotBinding {
    pub(in super::super) buffer: Arc<wgpu::Buffer>,
    pub(in super::super) dynamic_offset: u32,
    pub(in super::super) size: NonZeroU64,
    pub(in super::super) buffer_generation: u64,
}
