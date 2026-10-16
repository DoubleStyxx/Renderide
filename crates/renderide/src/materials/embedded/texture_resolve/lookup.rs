//! Texture asset id resolution for embedded `@group(1)` texture bindings.
//!
//! Host materials reference textures through property ids whose values are packed `(asset_id,
//! kind)` tuples. The helpers in this module unpack those values, apply the primary-texture
//! fallback for `_MainTex` / `_Tex` slots, and return one [`ResolvedTextureBinding`] per
//! reflected texture entry.

use std::hash::{Hash, Hasher};

use crate::assets::texture::{
    HostTextureAssetKind, texture2d_asset_id_from_packed, unpack_host_texture_packed,
};
use crate::materials::ReflectedRasterLayout;
use crate::materials::host_data::{
    MaterialPropertyLookupIds, MaterialPropertyStore, MaterialPropertyValue,
};

use super::super::layout::{StemEmbeddedPropertyIds, shader_writer_unescaped_property_name};

/// Resolved GPU texture binding for a material property (packed host id or primary fallback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedTextureBinding {
    /// No texture or unsupported packed type.
    None,
    /// [`crate::gpu_pools::TexturePool`] entry (unpacked 2D asset id).
    Texture2D {
        /// Unpacked asset id within the 2D pool.
        asset_id: i32,
    },
    /// [`crate::gpu_pools::Texture3dPool`] entry (unpacked 3D asset id).
    Texture3D {
        /// Unpacked asset id within the 3D pool.
        asset_id: i32,
    },
    /// [`crate::gpu_pools::CubemapPool`] entry (unpacked cubemap asset id).
    Cubemap {
        /// Unpacked asset id within the cubemap pool.
        asset_id: i32,
    },
    /// [`crate::gpu_pools::RenderTexturePool`] entry (unpacked render-texture asset id).
    RenderTexture {
        /// Unpacked asset id within the render-texture pool.
        asset_id: i32,
    },
    /// [`crate::gpu_pools::VideoTexturePool`] entry (unpacked 2D asset id).
    VideoTexture {
        /// Unpacked asset id within the video-texture pool.
        asset_id: i32,
    },
}

/// Placeholder texel color to bind for an unset or nonresident 2D texture slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefaultTextureColor {
    /// Use the default opaque white texture.
    White,
    /// Use the default opaque black texture.
    Black,
    /// Use the default flat tangent-space normal texture `(0.5, 0.5, 1.0)`.
    FlatNormal,
}

impl ResolvedTextureBinding {
    /// Hashes this binding into `hasher` using a discriminant-tagged scheme that distinguishes
    /// between texture pool kinds even when asset ids collide between pools.
    pub(crate) fn hash_for_signature(self, hasher: &mut impl Hasher) {
        match self {
            ResolvedTextureBinding::None => {
                0u8.hash(hasher);
            }
            ResolvedTextureBinding::Texture2D { asset_id } => {
                1u8.hash(hasher);
                asset_id.hash(hasher);
            }
            ResolvedTextureBinding::Texture3D { asset_id } => {
                3u8.hash(hasher);
                asset_id.hash(hasher);
            }
            ResolvedTextureBinding::Cubemap { asset_id } => {
                4u8.hash(hasher);
                asset_id.hash(hasher);
            }
            ResolvedTextureBinding::RenderTexture { asset_id } => {
                2u8.hash(hasher);
                asset_id.hash(hasher);
            }
            ResolvedTextureBinding::VideoTexture { asset_id } => {
                5u8.hash(hasher);
                asset_id.hash(hasher);
            }
        }
    }
}

/// Property ids to try for one reflected texture binding, in priority order:
/// the exact WGSL global name first, followed by host-side aliases such as
/// `_MaskTex` -> `MaskTexture`.
pub(crate) fn texture_property_ids_for_binding(
    ids: &StemEmbeddedPropertyIds,
    binding: u32,
) -> &[i32] {
    // aliases are built once per stem, no tiny Vec per texture bind.
    ids.texture_binding_property_ids
        .get(&binding)
        .map_or(&[], |pids| pids.as_ref())
}

fn first_material_texture_binding(reflected: &ReflectedRasterLayout) -> Option<u32> {
    reflected
        .material_entries
        .iter()
        .find(|entry| matches!(entry.ty, wgpu::BindingType::Texture { .. }))
        .map(|entry| entry.binding)
}

/// Resolves the primary 2D texture asset id from the first reflected material texture slot.
pub(crate) fn primary_texture_2d_asset_id(
    reflected: &ReflectedRasterLayout,
    ids: &StemEmbeddedPropertyIds,
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
) -> i32 {
    let Some(binding) = first_material_texture_binding(reflected) else {
        return -1;
    };
    for &pid in texture_property_ids_for_binding(ids, binding) {
        if let Some(MaterialPropertyValue::Texture(packed)) = store.get_merged(lookup, pid) {
            return texture2d_asset_id_from_packed(*packed).unwrap_or(-1);
        }
    }
    -1
}

/// Whether `host_name` is the canonical primary-texture name for which we should fall back to
/// the bound primary texture when no explicit binding is present.
///
/// Only `_MainTex` and `_Tex` are accepted: the host writes one of these from every primary
/// texture call (`_MainTex` everywhere except `UnlitMaterial` which uses `_Tex`).
pub(crate) fn should_fallback_to_primary_texture(host_name: &str) -> bool {
    let host_name = shader_writer_unescaped_property_name(host_name);
    matches!(host_name, "_MainTex" | "_Tex")
}

/// Returns the compatibility 2D placeholder color for a reflected host texture name.
///
/// Embedded shaders with `//#texture_default` directives bypass this heuristic. It remains as a
/// fallback for custom or unannotated shader slots, preserving the old channel-name convention:
/// metallic and packed-normal channels default to black, tangent-space normal maps default to a
/// flat normal, and every other slot defaults to white.
pub(crate) fn default_2d_texture_color_for_host(host_name: &str) -> DefaultTextureColor {
    let host_name = shader_writer_unescaped_property_name(host_name);
    match host_name {
        "_MetallicMap" | "_MetallicMap1" | "_MetallicMap2" | "_MetallicMap3"
        | "_MetallicGloss01" | "_MetallicGloss23" | "_PackedNormalMap01" | "_PackedNormalMap23" => {
            DefaultTextureColor::Black
        }
        "_BumpMap" | "_NormalMap" | "_NormalMap0" | "_NormalMap1" | "_DetailNormalMap" => {
            DefaultTextureColor::FlatNormal
        }
        _ => DefaultTextureColor::White,
    }
}

/// Maps one merged property value to a texture binding. Non-texture values are [`ResolvedTextureBinding::None`].
pub(crate) fn resolved_texture_binding_from_property(
    value: Option<&MaterialPropertyValue>,
) -> ResolvedTextureBinding {
    match value {
        Some(MaterialPropertyValue::Texture(packed)) => match unpack_host_texture_packed(*packed) {
            Some((id, HostTextureAssetKind::Texture2D)) => {
                ResolvedTextureBinding::Texture2D { asset_id: id }
            }
            Some((id, HostTextureAssetKind::Texture3D)) => {
                ResolvedTextureBinding::Texture3D { asset_id: id }
            }
            Some((id, HostTextureAssetKind::Cubemap)) => {
                ResolvedTextureBinding::Cubemap { asset_id: id }
            }
            Some((id, HostTextureAssetKind::RenderTexture)) => {
                ResolvedTextureBinding::RenderTexture { asset_id: id }
            }
            Some((id, HostTextureAssetKind::VideoTexture)) => {
                ResolvedTextureBinding::VideoTexture { asset_id: id }
            }
            _ => ResolvedTextureBinding::None,
        },
        _ => ResolvedTextureBinding::None,
    }
}

fn texture_property_binding(
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
    property_id: i32,
) -> ResolvedTextureBinding {
    resolved_texture_binding_from_property(store.get_merged(lookup, property_id))
}

/// Which resident pool view satisfies a reflected `@group(1)` texture dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SampledTextureSource {
    /// `texture_2d` from a 2D texture asset.
    Texture2D,
    /// `texture_3d` from a 3D texture asset.
    Texture3D,
    /// `texture_cube` from a cubemap asset.
    Cubemap,
    /// `texture_2d_array` from the cubemap asset's face array view.
    CubemapArray,
    /// `texture_2d` from a render texture.
    RenderTexture,
    /// `texture_2d` from a video texture.
    VideoTexture,
}

/// Maps a reflected view dimension and a host texture property onto one pool view.
///
/// `texture_2d` reads 2D, render, and video assets. `texture_cube` reads cubemaps.
/// `texture_2d_array` reads the cubemap face array. Other pairs do not bind.
pub(crate) fn sampled_texture_source(
    view_dimension: wgpu::TextureViewDimension,
    binding: ResolvedTextureBinding,
) -> Option<SampledTextureSource> {
    match (view_dimension, binding) {
        (_, ResolvedTextureBinding::None) => None,
        (wgpu::TextureViewDimension::D2, ResolvedTextureBinding::Texture2D { .. }) => {
            Some(SampledTextureSource::Texture2D)
        }
        (wgpu::TextureViewDimension::D3, ResolvedTextureBinding::Texture3D { .. }) => {
            Some(SampledTextureSource::Texture3D)
        }
        (wgpu::TextureViewDimension::Cube, ResolvedTextureBinding::Cubemap { .. }) => {
            Some(SampledTextureSource::Cubemap)
        }
        (wgpu::TextureViewDimension::D2Array, ResolvedTextureBinding::Cubemap { .. }) => {
            Some(SampledTextureSource::CubemapArray)
        }
        (wgpu::TextureViewDimension::D2, ResolvedTextureBinding::RenderTexture { .. }) => {
            Some(SampledTextureSource::RenderTexture)
        }
        (wgpu::TextureViewDimension::D2, ResolvedTextureBinding::VideoTexture { .. }) => {
            Some(SampledTextureSource::VideoTexture)
        }
        _ => None,
    }
}

/// Pairs a sampler binding with the texture it samples.
///
/// The sampler global `_MainTex_sampler` pairs with the texture global `_MainTex` even when the
/// bindings are not adjacent. A sampler that does not follow that name falls back to the texture
/// at `binding - 1`. `None` means the bind uses the default sampler.
pub(crate) fn paired_texture_binding_for_sampler(
    reflected: &ReflectedRasterLayout,
    sampler_binding: u32,
) -> Option<u32> {
    if let Some(name) = reflected.material_group1_names.get(&sampler_binding) {
        let name = shader_writer_unescaped_property_name(name);
        if let Some(texture_name) = name.strip_suffix("_sampler")
            && let Some(binding) = texture_binding_named(reflected, texture_name)
        {
            return Some(binding);
        }
    }
    let adjacent = sampler_binding.saturating_sub(1);
    if adjacent != sampler_binding && is_texture_binding(reflected, adjacent) {
        return Some(adjacent);
    }
    None
}

fn texture_binding_named(reflected: &ReflectedRasterLayout, texture_name: &str) -> Option<u32> {
    reflected.material_entries.iter().find_map(|entry| {
        if !matches!(entry.ty, wgpu::BindingType::Texture { .. }) {
            return None;
        }
        let name = reflected.material_group1_names.get(&entry.binding)?;
        (shader_writer_unescaped_property_name(name) == texture_name).then_some(entry.binding)
    })
}

fn is_texture_binding(reflected: &ReflectedRasterLayout, binding: u32) -> bool {
    reflected.material_entries.iter().any(|entry| {
        entry.binding == binding && matches!(entry.ty, wgpu::BindingType::Texture { .. })
    })
}

/// Resolves resident texture binding for a host property name, with primary-texture fallback for 2D-only slots.
pub(crate) fn resolved_texture_binding_for_host(
    host_name: &str,
    texture_property_ids: &[i32],
    primary_texture_2d: i32,
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
) -> ResolvedTextureBinding {
    for &texture_property_id in texture_property_ids {
        let b = texture_property_binding(store, lookup, texture_property_id);
        if !matches!(b, ResolvedTextureBinding::None) {
            return b;
        }
    }
    if should_fallback_to_primary_texture(host_name) && primary_texture_2d >= 0 {
        return ResolvedTextureBinding::Texture2D {
            asset_id: primary_texture_2d,
        };
    }
    ResolvedTextureBinding::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use hashbrown::HashMap;

    use crate::materials::embedded::layout::StemEmbeddedPropertyIds;
    use crate::materials::host_data::PropertyIdRegistry;

    fn lookup(material_id: i32) -> MaterialPropertyLookupIds {
        MaterialPropertyLookupIds {
            material_asset_id: material_id,
            mesh_property_block_slot0: None,
            mesh_renderer_property_block_id: None,
        }
    }

    fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }
    }

    fn reflected_with_textures(
        names: &[(u32, &str)],
    ) -> (
        ReflectedRasterLayout,
        StemEmbeddedPropertyIds,
        PropertyIdRegistry,
    ) {
        let registry = PropertyIdRegistry::new();
        let mut texture_binding_property_ids = HashMap::new();
        let mut material_group1_names = HashMap::new();
        let mut material_entries = Vec::new();
        for &(binding, name) in names {
            let pid = registry.intern(name);
            texture_binding_property_ids.insert(binding, Arc::from(vec![pid].into_boxed_slice()));
            material_group1_names.insert(binding, name.to_string());
            material_entries.push(texture_entry(binding));
        }
        (
            ReflectedRasterLayout {
                layout_fingerprint: 0,
                material_entries,
                per_draw_entries: Vec::new(),
                material_uniform: None,
                material_group1_names,
                vs_vertex_inputs: Vec::new(),
                vs_max_vertex_location: None,
                uses_scene_depth_snapshot: false,
                uses_scene_color_snapshot: false,
                requires_intersection_pass: false,
            },
            StemEmbeddedPropertyIds {
                uniform_field_ids: HashMap::new(),
                texture_binding_property_ids,
            },
            registry,
        )
    }

    /// Packs a host texture id using the same bit layout as `IdPacker<TextureAssetType>`.
    fn pack_host_texture(asset_id: i32, kind: HostTextureAssetKind) -> i32 {
        ((asset_id as u32) | ((kind as u32) << 29)) as i32
    }

    fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        }
    }

    fn reflected_handles(
        entries: Vec<wgpu::BindGroupLayoutEntry>,
        names: &[(u32, &str)],
    ) -> ReflectedRasterLayout {
        let mut material_group1_names = HashMap::new();
        for &(binding, name) in names {
            material_group1_names.insert(binding, name.to_string());
        }
        ReflectedRasterLayout {
            layout_fingerprint: 0,
            material_entries: entries,
            per_draw_entries: Vec::new(),
            material_uniform: None,
            material_group1_names,
            vs_vertex_inputs: Vec::new(),
            vs_max_vertex_location: None,
            uses_scene_depth_snapshot: false,
            uses_scene_color_snapshot: false,
            requires_intersection_pass: false,
        }
    }

    #[test]
    fn sampler_pairs_by_name_then_adjacent_binding() {
        let reflected = reflected_handles(
            vec![
                texture_entry(1),
                texture_entry(4),
                sampler_entry(5),
                sampler_entry(6),
                sampler_entry(8),
            ],
            &[
                (1, "_MainTex"),
                (4, "_Cube"),
                (5, "_Other_sampler"),
                (6, "_MainTex_sampler"),
                (8, "_Loose"),
            ],
        );
        assert_eq!(paired_texture_binding_for_sampler(&reflected, 6), Some(1));
        assert_eq!(paired_texture_binding_for_sampler(&reflected, 5), Some(4));
        assert_eq!(paired_texture_binding_for_sampler(&reflected, 8), None);
    }

    #[test]
    fn sampled_texture_source_maps_2d_cube_and_array() {
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::D2,
                ResolvedTextureBinding::Texture2D { asset_id: 1 }
            ),
            Some(SampledTextureSource::Texture2D)
        );
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::Cube,
                ResolvedTextureBinding::Cubemap { asset_id: 2 }
            ),
            Some(SampledTextureSource::Cubemap)
        );
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::D2Array,
                ResolvedTextureBinding::Cubemap { asset_id: 2 }
            ),
            Some(SampledTextureSource::CubemapArray)
        );
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::D2Array,
                ResolvedTextureBinding::Texture2D { asset_id: 1 }
            ),
            None
        );
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::D3,
                ResolvedTextureBinding::Texture3D { asset_id: 3 }
            ),
            Some(SampledTextureSource::Texture3D)
        );
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::D2,
                ResolvedTextureBinding::RenderTexture { asset_id: 4 }
            ),
            Some(SampledTextureSource::RenderTexture)
        );
        assert_eq!(
            sampled_texture_source(
                wgpu::TextureViewDimension::D2,
                ResolvedTextureBinding::VideoTexture { asset_id: 5 }
            ),
            Some(SampledTextureSource::VideoTexture)
        );
        assert_eq!(
            sampled_texture_source(wgpu::TextureViewDimension::D2, ResolvedTextureBinding::None),
            None
        );
    }

    #[test]
    fn resolved_texture_binding_uses_alias_property_id() {
        let mut store = MaterialPropertyStore::new();
        let exact_mask_tex_pid = 10;
        let alias_mask_texture_pid = 11;
        store.set_material(
            4,
            alias_mask_texture_pid,
            MaterialPropertyValue::Texture(123),
        );

        assert_eq!(
            resolved_texture_binding_for_host(
                "_MaskTex",
                &[exact_mask_tex_pid, alias_mask_texture_pid],
                -1,
                &store,
                lookup(4),
            ),
            ResolvedTextureBinding::Texture2D { asset_id: 123 }
        );
    }

    #[test]
    fn resolved_texture_binding_prefers_exact_property_id_over_alias() {
        let mut store = MaterialPropertyStore::new();
        let exact_tex_pid = 20;
        let alias_texture_pid = 21;
        store.set_material(5, exact_tex_pid, MaterialPropertyValue::Texture(200));
        store.set_material(5, alias_texture_pid, MaterialPropertyValue::Texture(201));

        assert_eq!(
            resolved_texture_binding_for_host(
                "_Tex",
                &[exact_tex_pid, alias_texture_pid],
                -1,
                &store,
                lookup(5),
            ),
            ResolvedTextureBinding::Texture2D { asset_id: 200 }
        );
    }

    #[test]
    fn resolved_texture_binding_accepts_video_texture_property() {
        let mut store = MaterialPropertyStore::new();
        let main_tex_pid = 30;
        store.set_material(
            8,
            main_tex_pid,
            MaterialPropertyValue::Texture(pack_host_texture(
                44,
                HostTextureAssetKind::VideoTexture,
            )),
        );

        assert_eq!(
            resolved_texture_binding_for_host("_MainTex", &[main_tex_pid], -1, &store, lookup(8)),
            ResolvedTextureBinding::VideoTexture { asset_id: 44 }
        );
    }

    #[test]
    fn primary_texture_fallback_strips_naga_oil_suffix() {
        assert!(should_fallback_to_primary_texture(
            "_MainTexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJ4GSZLYMU5DU5DPN5XDEX"
        ));
    }

    #[test]
    fn metallic_variant_maps_use_black_default_texture() {
        for host_name in [
            "_MetallicMap",
            "_MetallicMap1",
            "_MetallicMap2",
            "_MetallicMap3",
            "_MetallicGloss01",
            "_MetallicGloss23",
            "_MetallicMap1_",
            "_MetallicMapX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJ4GSZLYMU5DU5DPN5XDEX",
        ] {
            assert_eq!(
                default_2d_texture_color_for_host(host_name),
                DefaultTextureColor::Black,
                "{host_name} should bind the black placeholder"
            );
        }
    }

    #[test]
    fn packed_normal_maps_use_black_default_texture() {
        // PBSColorSplat ships `_PackedNormalMap01/23 = "black" {}`: the encoding packs derivative
        // deltas around zero, so a black texel is the no-op default and a flat-normal value would
        // bias the surface.
        for host_name in [
            "_PackedNormalMap01",
            "_PackedNormalMap23",
            "_PackedNormalMap01X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJ4GSZLYMU5DU5DPN5XDEX",
        ] {
            assert_eq!(
                default_2d_texture_color_for_host(host_name),
                DefaultTextureColor::Black,
                "{host_name} should bind the black placeholder"
            );
        }
    }

    #[test]
    fn normal_map_slots_use_flat_normal_default_texture() {
        for host_name in [
            "_BumpMap",
            "_NormalMap",
            "_NormalMap0",
            "_NormalMap1",
            "_DetailNormalMap",
            "_BumpMapX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJ4GSZLYMU5DU5DPN5XDEX",
        ] {
            assert_eq!(
                default_2d_texture_color_for_host(host_name),
                DefaultTextureColor::FlatNormal,
                "{host_name} should bind the flat-normal placeholder"
            );
        }
    }

    #[test]
    fn standard_metallic_gloss_map_keeps_white_default_texture() {
        for host_name in [
            "_MetallicGlossMap",
            "_OcclusionMap",
            "_SpecGlossMap",
            "_EmissionMap",
            "_MainTex",
            "_Tex",
        ] {
            assert_eq!(
                default_2d_texture_color_for_host(host_name),
                DefaultTextureColor::White,
                "{host_name} should bind the white placeholder"
            );
        }
    }

    #[test]
    fn primary_texture_ignores_later_non_primary_maps() {
        let (reflected, ids, registry) =
            reflected_with_textures(&[(1, "_MainTex"), (9, "_OcclusionMap")]);
        let mut store = MaterialPropertyStore::new();
        let occlusion = registry.intern("_OcclusionMap");
        store.set_material(6, occlusion, MaterialPropertyValue::Texture(77));

        assert_eq!(
            primary_texture_2d_asset_id(&reflected, &ids, &store, lookup(6)),
            -1
        );
        assert_eq!(
            resolved_texture_binding_for_host(
                "_MainTex",
                texture_property_ids_for_binding(&ids, 1),
                primary_texture_2d_asset_id(&reflected, &ids, &store, lookup(6)),
                &store,
                lookup(6),
            ),
            ResolvedTextureBinding::None
        );

        let main = registry.intern("_MainTex");
        store.set_material(6, main, MaterialPropertyValue::Texture(88));
        assert_eq!(
            primary_texture_2d_asset_id(&reflected, &ids, &store, lookup(6)),
            88
        );
    }
}
