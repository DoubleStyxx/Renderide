//! Uniform byte packing for embedded `@group(1)` material blocks (reflection-driven defaults
//! plus renderer-reserved fields).

use crate::materials::host_data::{MaterialPropertyLookupIds, MaterialPropertyStore};
use crate::materials::{ReflectedRasterLayout, ReflectedUniformField};

use crate::embedded_shaders::{EmbeddedMaterialDefaultKind, EmbeddedMaterialDefaultValue};

use super::layout::StemEmbeddedPropertyIds;
use super::texture_pools::EmbeddedTexturePools;
use super::texture_resolve::ResolvedTextureBinding;
use super::wrap_mode_bits::{WRAP_MODE_BITS_SUFFIX, sampler_wrap_mode_bits};

mod color_space;
mod helpers;
mod plan;

pub(crate) use plan::MaterialUniformPackPlan;

pub(crate) use crate::color_space::srgb_f32x4_rgb_to_linear as srgb_vec4_rgb_to_linear;
pub(crate) use color_space::MaterialUniformValueSpaces;
use helpers::shader_writer_unescaped_field_name;

/// Material uniform fallback values keyed by reflected WGSL field name.
pub(crate) type MaterialUniformDefaults = hashbrown::HashMap<String, EmbeddedMaterialDefaultValue>;

/// Source metadata used while packing material uniform bytes.
pub(crate) struct MaterialUniformPackMetadata<'a> {
    /// Per-field color-space metadata for reflected material uniforms.
    pub(crate) value_spaces: &'a MaterialUniformValueSpaces,
    /// Per-field fallback values declared by shader source metadata.
    pub(crate) material_defaults: &'a MaterialUniformDefaults,
}

/// Suffix convention that opts a uniform field in to host `mipmap_bias` population.
const LOD_BIAS_SUFFIX: &str = "_LodBias";
/// Suffix convention that opts a uniform field in to storage V-inversion population.
const STORAGE_V_INVERTED_SUFFIX: &str = "_StorageVInverted";
/// Suffix convention Unity uses for texture scale/offset uniforms.
const TEXTURE_TRANSFORM_SUFFIX: &str = "_ST";
/// Unity's implicit identity scale/offset value for unwritten texture transform uniforms.
const UNITY_TEXTURE_TRANSFORM_IDENTITY: [f32; 4] = [1.0, 1.0, 0.0, 0.0];

fn write_f32_at(buf: &mut [u8], field: &ReflectedUniformField, v: f32) {
    let off = field.offset as usize;
    if off + 4 <= buf.len() && field.size >= 4 {
        buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn write_u32_at(buf: &mut [u8], field: &ReflectedUniformField, v: u32) {
    let off = field.offset as usize;
    if off + 4 <= buf.len() && field.size >= 4 {
        buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn write_f32_slice_at(buf: &mut [u8], field: &ReflectedUniformField, values: &[f32]) {
    let off = field.offset as usize;
    let bytes = values.len().saturating_mul(4);
    if off + bytes <= buf.len() && field.size as usize >= bytes {
        for (i, value) in values.iter().enumerate() {
            let component_off = off + i * 4;
            buf[component_off..component_off + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
}

fn write_f32x4_at(buf: &mut [u8], field: &ReflectedUniformField, v: &[f32; 4]) {
    write_f32_slice_at(buf, field, v);
}

/// Element stride for a reflected uniform array. Non-array fields keep the historical 16-byte stride.
fn array_element_stride(field: &ReflectedUniformField) -> usize {
    let stride = field.array_stride as usize;
    if stride >= 4 { stride } else { 16 }
}

/// Writes a host `float4[]` material property into a reflected uniform array field.
fn write_f32x4_array_at(buf: &mut [u8], field: &ReflectedUniformField, values: &[[f32; 4]]) {
    let off = field.offset as usize;
    let stride = array_element_stride(field);
    let max_values = (field.size as usize) / stride;
    for (i, value) in values.iter().take(max_values).enumerate() {
        let elem_off = off + i * stride;
        if elem_off + 16 > buf.len() {
            return;
        }
        for (component, v) in value.iter().enumerate() {
            let component_off = elem_off + component * 4;
            buf[component_off..component_off + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
}

/// Writes a host `float[]` material property into a reflected uniform scalar array field.
fn write_f32_array_at(buf: &mut [u8], field: &ReflectedUniformField, values: &[f32]) {
    let off = field.offset as usize;
    let stride = array_element_stride(field);
    let max_values = (field.size as usize) / stride;
    for (i, value) in values.iter().take(max_values).enumerate() {
        let elem_off = off + i * stride;
        if elem_off + 4 > buf.len() {
            return;
        }
        buf[elem_off..elem_off + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn write_srgb_f32x4_array_at(buf: &mut [u8], field: &ReflectedUniformField, values: &[[f32; 4]]) {
    let off = field.offset as usize;
    let stride = array_element_stride(field);
    let max_values = (field.size as usize) / stride;
    for (i, value) in values.iter().take(max_values).enumerate() {
        let elem_off = off + i * stride;
        if elem_off + 16 > buf.len() {
            return;
        }
        for (component, v) in srgb_vec4_rgb_to_linear(*value).iter().enumerate() {
            let component_off = elem_off + component * 4;
            buf[component_off..component_off + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
}

/// Returns the reflected `vec4<f32>` fallback value for a missing host property.
fn missing_vec4_uniform_default(
    field_name: &str,
    material_defaults: &MaterialUniformDefaults,
) -> [f32; 4] {
    if let Some(default) = material_defaults.get(field_name)
        && default.kind == EmbeddedMaterialDefaultKind::Vec4
    {
        return default.values;
    }
    if shader_writer_unescaped_field_name(field_name).ends_with(TEXTURE_TRANSFORM_SUFFIX) {
        UNITY_TEXTURE_TRANSFORM_IDENTITY
    } else {
        [0.0; 4]
    }
}

/// Returns the reflected `f32` fallback value for a missing host property.
fn missing_f32_uniform_default(
    field_name: &str,
    material_defaults: &MaterialUniformDefaults,
) -> f32 {
    if let Some(default) = material_defaults.get(field_name)
        && default.kind == EmbeddedMaterialDefaultKind::Float
    {
        default.values[0]
    } else {
        0.0
    }
}

/// Auxiliary inputs required to populate texture-sourced uniform fields.
///
/// Threads resident texture pools into the packer so f32 fields following texture suffix
/// conventions can resolve their bound texture and read sampler/orientation metadata.
pub(crate) struct UniformPackTextureContext<'a> {
    /// Resident texture pools (2D / 3D / cubemap / render-texture).
    pub pools: &'a EmbeddedTexturePools<'a>,
    /// Primary 2D texture asset id for `_MainTex` / `_Tex` fallback (from [`crate::materials::embedded::texture_resolve::primary_texture_2d_asset_id`]).
    pub primary_texture_2d: i32,
}

/// Builds CPU bytes for the reflected material uniform block.
///
/// Every value comes from one of several sources, in priority order: texture storage-orientation
/// flags for fields following the [`STORAGE_V_INVERTED_SUFFIX`] convention, host-sourced sampler
/// state for fields following the [`LOD_BIAS_SUFFIX`] convention (`_<Tex>_LodBias`), the host's
/// property store (for host-declared properties), source-declared material defaults, Unity's
/// identity texture transform for missing `*_ST` vec4 fields, texture wrap bits for fields
/// following the [`WRAP_MODE_BITS_SUFFIX`] convention, or the renderer-reserved
/// `_RenderideVariantBits` variant bitfield. Other missing fields fall through to zero. Color-space
/// conversion applies only to host-sourced `float4` and `float4[]` values.
#[cfg(test)]
pub(crate) fn build_embedded_uniform_bytes(
    reflected: &ReflectedRasterLayout,
    ids: &StemEmbeddedPropertyIds,
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
    tex_ctx: &UniformPackTextureContext<'_>,
) -> Option<Vec<u8>> {
    build_embedded_uniform_bytes_with_value_spaces(
        reflected,
        ids,
        &MaterialUniformValueSpaces::default(),
        store,
        lookup,
        tex_ctx,
        None,
    )
}

/// Builds CPU bytes for the reflected material uniform block using explicit per-field value-space metadata.
#[cfg(test)]
pub(crate) fn build_embedded_uniform_bytes_with_value_spaces(
    reflected: &ReflectedRasterLayout,
    ids: &StemEmbeddedPropertyIds,
    value_spaces: &MaterialUniformValueSpaces,
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
    tex_ctx: &UniformPackTextureContext<'_>,
    shader_variant_bits: Option<u32>,
) -> Option<Vec<u8>> {
    build_embedded_uniform_bytes_with_material_defaults(
        reflected,
        ids,
        &MaterialUniformPackMetadata {
            value_spaces,
            material_defaults: &MaterialUniformDefaults::default(),
        },
        store,
        lookup,
        tex_ctx,
        shader_variant_bits,
    )
}

/// Builds CPU bytes for the reflected material uniform block using explicit value-space and default metadata.
pub(crate) fn build_embedded_uniform_bytes_with_material_defaults(
    reflected: &ReflectedRasterLayout,
    ids: &StemEmbeddedPropertyIds,
    metadata: &MaterialUniformPackMetadata<'_>,
    store: &MaterialPropertyStore,
    lookup: MaterialPropertyLookupIds,
    tex_ctx: &UniformPackTextureContext<'_>,
    shader_variant_bits: Option<u32>,
) -> Option<Vec<u8>> {
    profiling::scope!("materials::embedded_uniform_pack");
    let key = plan::uniform_plan_cache_key(reflected, ids, metadata.material_defaults);
    if let Some(cached) = metadata.value_spaces.cached_plan().get()
        && cached.key == key
    {
        let mut buf = Vec::new();
        return cached
            .plan
            .pack_into(store, lookup, tex_ctx, shader_variant_bits, &mut buf)
            .then_some(buf);
    }
    let plan = MaterialUniformPackPlan::compile(reflected, ids, metadata);
    let mut buf = Vec::new();
    let ready = plan.pack_into(store, lookup, tex_ctx, shader_variant_bits, &mut buf);
    let _ = metadata
        .value_spaces
        .cached_plan()
        .set(plan::CachedUniformPlan { key, plan });
    ready.then_some(buf)
}

/// Returns whether a resolved texture binding is a host-uploaded texture with V-inverted storage.
fn binding_storage_v_inverted_from_metadata(
    resolved: ResolvedTextureBinding,
    texture2d_storage_v_inverted: Option<bool>,
    cubemap_storage_v_inverted: Option<bool>,
) -> bool {
    match resolved {
        ResolvedTextureBinding::Texture2D { .. } => texture2d_storage_v_inverted.unwrap_or(false),
        ResolvedTextureBinding::Cubemap { .. } => cubemap_storage_v_inverted.unwrap_or(false),
        ResolvedTextureBinding::None
        | ResolvedTextureBinding::Texture3D { .. }
        | ResolvedTextureBinding::RenderTexture { .. }
        | ResolvedTextureBinding::VideoTexture { .. } => false,
    }
}

/// Returns whether a resolved texture binding is a host-uploaded texture with V-inverted storage.
fn binding_storage_v_inverted(
    resolved: ResolvedTextureBinding,
    tex_ctx: &UniformPackTextureContext<'_>,
) -> bool {
    let texture2d_storage_v_inverted = match resolved {
        ResolvedTextureBinding::Texture2D { asset_id } => tex_ctx
            .pools
            .texture
            .get(asset_id)
            .map(|t| t.storage_v_inverted),
        _ => None,
    };
    let cubemap_storage_v_inverted = match resolved {
        ResolvedTextureBinding::Cubemap { asset_id } => tex_ctx
            .pools
            .cubemap
            .get(asset_id)
            .map(|t| t.storage_v_inverted),
        _ => None,
    };
    binding_storage_v_inverted_from_metadata(
        resolved,
        texture2d_storage_v_inverted,
        cubemap_storage_v_inverted,
    )
}

/// Converts a storage V-inversion flag into the f32 convention used by explicit shader uniforms.
fn storage_v_inverted_flag_value(storage_v_inverted: bool) -> f32 {
    if storage_v_inverted { 1.0 } else { 0.0 }
}

/// Returns texture wrap bits for texture kinds whose sampler state exposes wrap modes.
fn binding_wrap_mode_bits_from_metadata(
    resolved: ResolvedTextureBinding,
    texture2d_wrap_mode_bits: Option<u32>,
    texture3d_wrap_mode_bits: Option<u32>,
    render_texture_wrap_mode_bits: Option<u32>,
    video_texture_wrap_mode_bits: Option<u32>,
) -> u32 {
    match resolved {
        ResolvedTextureBinding::Texture2D { .. } => texture2d_wrap_mode_bits.unwrap_or(0),
        ResolvedTextureBinding::Texture3D { .. } => texture3d_wrap_mode_bits.unwrap_or(0),
        ResolvedTextureBinding::RenderTexture { .. } => render_texture_wrap_mode_bits.unwrap_or(0),
        ResolvedTextureBinding::VideoTexture { .. } => video_texture_wrap_mode_bits.unwrap_or(0),
        ResolvedTextureBinding::None | ResolvedTextureBinding::Cubemap { .. } => 0,
    }
}

/// Returns texture wrap bits for a resolved binding from the resident texture pools.
fn binding_wrap_mode_bits(
    resolved: ResolvedTextureBinding,
    tex_ctx: &UniformPackTextureContext<'_>,
) -> u32 {
    let texture2d_wrap_mode_bits = match resolved {
        ResolvedTextureBinding::Texture2D { asset_id } => tex_ctx
            .pools
            .texture
            .get(asset_id)
            .map(|t| sampler_wrap_mode_bits(&t.sampler)),
        _ => None,
    };
    let texture3d_wrap_mode_bits = match resolved {
        ResolvedTextureBinding::Texture3D { asset_id } => tex_ctx
            .pools
            .texture3d
            .get(asset_id)
            .map(|t| sampler_wrap_mode_bits(&t.sampler)),
        _ => None,
    };
    let render_texture_wrap_mode_bits = match resolved {
        ResolvedTextureBinding::RenderTexture { asset_id } => tex_ctx
            .pools
            .render_texture
            .get(asset_id)
            .map(|t| sampler_wrap_mode_bits(&t.sampler)),
        _ => None,
    };
    let video_texture_wrap_mode_bits = match resolved {
        ResolvedTextureBinding::VideoTexture { asset_id } => tex_ctx
            .pools
            .video_texture
            .get(asset_id)
            .map(|t| sampler_wrap_mode_bits(&t.sampler)),
        _ => None,
    };
    binding_wrap_mode_bits_from_metadata(
        resolved,
        texture2d_wrap_mode_bits,
        texture3d_wrap_mode_bits,
        render_texture_wrap_mode_bits,
        video_texture_wrap_mode_bits,
    )
}

/// Returns the shader LOD bias for texture kinds whose wire properties expose mip bias.
fn binding_lod_bias_from_metadata(
    resolved: ResolvedTextureBinding,
    texture2d_mipmap_bias: Option<f32>,
    cubemap_mipmap_bias: Option<f32>,
) -> f32 {
    match resolved {
        ResolvedTextureBinding::Texture2D { .. } => texture2d_mipmap_bias.unwrap_or(0.0),
        ResolvedTextureBinding::Cubemap { .. } => cubemap_mipmap_bias.unwrap_or(0.0),
        ResolvedTextureBinding::None
        | ResolvedTextureBinding::Texture3D { .. }
        | ResolvedTextureBinding::RenderTexture { .. }
        | ResolvedTextureBinding::VideoTexture { .. } => 0.0,
    }
}

/// Returns the shader LOD bias for a resolved binding from the resident texture pools.
fn binding_lod_bias(
    resolved: ResolvedTextureBinding,
    tex_ctx: &UniformPackTextureContext<'_>,
) -> f32 {
    let texture2d_mipmap_bias = match resolved {
        ResolvedTextureBinding::Texture2D { asset_id } => tex_ctx
            .pools
            .texture
            .get(asset_id)
            .map(|t| t.sampler.mipmap_bias),
        _ => None,
    };
    let cubemap_mipmap_bias = match resolved {
        ResolvedTextureBinding::Cubemap { asset_id } => tex_ctx
            .pools
            .cubemap
            .get(asset_id)
            .map(|t| t.sampler.mipmap_bias),
        _ => None,
    };
    binding_lod_bias_from_metadata(resolved, texture2d_mipmap_bias, cubemap_mipmap_bias)
}

#[cfg(test)]
mod tests;
