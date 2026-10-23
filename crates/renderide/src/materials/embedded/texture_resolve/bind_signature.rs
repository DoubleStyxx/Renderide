//! Hashes the per-frame signature used to invalidate the embedded `@group(1)` bind cache.
//!
//! [`texture_bind_signature`] visits every reflected texture entry and folds in the resolved
//! binding plus the live texture-pool state (view generation, mip residency, sampler state,
//! V-flip flag). When the resulting hash changes, the cached `wgpu::BindGroup` for the host
//! material has to be rebuilt.

use std::hash::{Hash, Hasher};

use ahash::AHasher;

use crate::frame_contract::OffscreenWriteTarget;
use crate::gpu_pools::SamplerState;
use crate::materials::ReflectedRasterLayout;
use crate::materials::host_data::{MaterialPropertyLookupIds, MaterialPropertyStore};

use super::super::layout::StemEmbeddedPropertyIds;
use super::super::texture_pools::EmbeddedTexturePools;
use super::lookup::{
    ResolvedTextureBinding, resolved_texture_binding_for_host, texture_property_ids_for_binding,
};

/// Hashes residency and the generations that identify the bound `wgpu::TextureView`.
///
/// `view_generation` is the allocation/view id (`view_generation` or cubemap
/// `allocation_generation`). `content_generation` bumps on texel reupload into that
/// allocation. A bind-group key that omits either can reuse a group whose view or
/// sampler LOD clamp no longer matches the pool.
pub(crate) fn hash_texture_residency_generation(
    hasher: &mut impl Hasher,
    resident: bool,
    view_generation: u64,
    content_generation: u64,
    mip_levels_resident: u32,
) {
    resident.hash(hasher);
    view_generation.hash(hasher);
    content_generation.hash(hasher);
    mip_levels_resident.hash(hasher);
}

/// Hashes the mode-affecting fields of a unified [`SamplerState`] for bind-cache invalidation.
pub(crate) fn hash_sampler_state(state: &SamplerState, h: &mut impl Hasher) {
    (state.filter_mode as i32).hash(h);
    state.aniso_level.hash(h);
    (state.wrap_u as i32).hash(h);
    (state.wrap_v as i32).hash(h);
    (state.wrap_w as i32).hash(h);
    state.mipmap_bias.to_bits().hash(h);
}

/// Fingerprint for bind cache invalidation when texture views or residency change.
///
/// When `offscreen_write_target` suppresses a host render texture, that render-texture id is
/// treated as non-resident so same-target material sampling stays masked.
pub(crate) fn texture_bind_signature(
    reflected: &ReflectedRasterLayout,
    ids: &StemEmbeddedPropertyIds,
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
    pools: &EmbeddedTexturePools<'_>,
    primary_texture_2d: i32,
    offscreen_write_target: OffscreenWriteTarget,
) -> u64 {
    let mut h = AHasher::default();
    offscreen_write_target.hash(&mut h);
    for entry in &reflected.material_entries {
        if !matches!(entry.ty, wgpu::BindingType::Texture { .. }) {
            continue;
        }
        let Some(name) = reflected.material_group1_names.get(&entry.binding) else {
            continue;
        };
        let texture_pids = texture_property_ids_for_binding(ids, entry.binding);
        if texture_pids.is_empty() {
            continue;
        }
        let binding = resolved_texture_binding_for_host(
            name.as_str(),
            texture_pids,
            primary_texture_2d,
            store,
            lookup,
        );
        hash_texture_entry_signature_contribution(
            &mut h,
            entry.binding,
            name.as_str(),
            binding,
            pools,
            offscreen_write_target,
        );
    }
    h.finish()
}

/// Hashes one reflected texture entry's contribution to [`texture_bind_signature`].
///
/// A snapshot pass that captures texture views and samplers in lockstep with signature
/// computation calls this so that the cache entry and its key are produced from the same
/// pool read.
pub(crate) fn hash_texture_entry_signature_contribution(
    hasher: &mut impl Hasher,
    entry_binding: u32,
    host_name: &str,
    binding: ResolvedTextureBinding,
    pools: &EmbeddedTexturePools<'_>,
    offscreen_write_target: OffscreenWriteTarget,
) {
    entry_binding.hash(hasher);
    host_name.hash(hasher);
    binding.hash_for_signature(hasher);
    match binding {
        ResolvedTextureBinding::None => false.hash(hasher),
        ResolvedTextureBinding::Texture2D { asset_id } => {
            if let Some(t) = pools.texture.get(asset_id) {
                hash_texture_residency_generation(
                    hasher,
                    t.mip_levels_resident > 0,
                    t.view_generation,
                    t.content_generation,
                    t.mip_levels_resident,
                );
                t.storage_v_inverted.hash(hasher);
                hash_sampler_state(&t.sampler, hasher);
            } else {
                false.hash(hasher);
            }
        }
        ResolvedTextureBinding::Texture3D { asset_id } => {
            if let Some(t) = pools.texture3d.get(asset_id) {
                hash_texture_residency_generation(
                    hasher,
                    t.mip_levels_resident > 0,
                    t.view_generation,
                    t.content_generation,
                    t.mip_levels_resident,
                );
                hash_sampler_state(&t.sampler, hasher);
            } else {
                false.hash(hasher);
            }
        }
        ResolvedTextureBinding::Cubemap { asset_id } => {
            if let Some(t) = pools.cubemap.get(asset_id) {
                hash_texture_residency_generation(
                    hasher,
                    t.mip_levels_resident > 0,
                    t.allocation_generation,
                    t.content_generation,
                    t.mip_levels_resident,
                );
                t.storage_v_inverted.hash(hasher);
                hash_sampler_state(&t.sampler, hasher);
            } else {
                false.hash(hasher);
            }
        }
        ResolvedTextureBinding::RenderTexture { asset_id } => {
            if offscreen_write_target.suppresses_render_texture_sampling(asset_id) {
                false.hash(hasher);
            } else if let Some(t) = pools.render_texture.get(asset_id) {
                let allow_previous =
                    offscreen_write_target.allows_previous_render_texture_contents(asset_id);
                t.is_sampleable().hash(hasher);
                t.sampled_color_view_generation(allow_previous).hash(hasher);
                hash_sampler_state(&t.sampler, hasher);
            } else {
                false.hash(hasher);
            }
        }
        ResolvedTextureBinding::VideoTexture { asset_id } => {
            if let Some(t) = pools.video_texture.get(asset_id) {
                t.is_sampleable().hash(hasher);
                t.view_generation.hash(hasher);
                hash_sampler_state(&t.sampler, hasher);
            } else {
                false.hash(hasher);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shared::{TextureFilterMode, TextureWrapMode};

    /// Hashes the same sampler fields used by `texture_bind_signature` for 2D/render textures.
    fn sampler_signature_for(state: &SamplerState) -> u64 {
        let mut hasher = AHasher::default();
        hash_sampler_state(state, &mut hasher);
        hasher.finish()
    }

    /// Builds a 2D sampler state with the supplied U/V wrap modes.
    fn texture2d_sampler_state(wrap_u: TextureWrapMode, wrap_v: TextureWrapMode) -> SamplerState {
        SamplerState {
            filter_mode: TextureFilterMode::Bilinear,
            aniso_level: 8,
            wrap_u,
            wrap_v,
            wrap_w: TextureWrapMode::default(),
            mipmap_bias: 0.0,
        }
    }

    /// Changing U wrap changes the sampler portion of the material bind signature.
    #[test]
    fn bind_signature_sampler_hash_distinguishes_render_texture_wrap_u() {
        let repeat = texture2d_sampler_state(TextureWrapMode::Repeat, TextureWrapMode::Clamp);
        let clamp = texture2d_sampler_state(TextureWrapMode::Clamp, TextureWrapMode::Clamp);

        assert_ne!(
            sampler_signature_for(&repeat),
            sampler_signature_for(&clamp)
        );
    }

    fn residency_generation_hash(view_generation: u64, content_generation: u64, mips: u32) -> u64 {
        let mut hasher = AHasher::default();
        hash_texture_residency_generation(
            &mut hasher,
            true,
            view_generation,
            content_generation,
            mips,
        );
        hasher.finish()
    }

    /// View reallocation, texel reupload, and mip residency each change the bind signature.
    #[test]
    fn residency_generation_hash_tracks_reupload_and_mip_residency() {
        let base = residency_generation_hash(4, 1, 6);
        assert_ne!(base, residency_generation_hash(5, 1, 6));
        assert_ne!(base, residency_generation_hash(4, 2, 6));
        assert_ne!(base, residency_generation_hash(4, 1, 5));
    }

    /// Changing V wrap changes the sampler portion of the material bind signature.
    #[test]
    fn bind_signature_sampler_hash_distinguishes_render_texture_wrap_v() {
        let repeat = texture2d_sampler_state(TextureWrapMode::Clamp, TextureWrapMode::Repeat);
        let clamp = texture2d_sampler_state(TextureWrapMode::Clamp, TextureWrapMode::Clamp);

        assert_ne!(
            sampler_signature_for(&repeat),
            sampler_signature_for(&clamp)
        );
    }
}
