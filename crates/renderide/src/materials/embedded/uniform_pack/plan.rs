//! Compiled material-uniform pack plan.
//!
//! Stem layout build classifies each reflected field once (property id, sRGB, default, texture
//! suffix). A dirty material then walks that list and hoists the three property-map probes out of
//! the per-field loop.

use crate::materials::embedded::layout::StemEmbeddedPropertyIds;
use crate::materials::embedded::texture_resolve::{
    resolved_texture_binding_from_property, should_fallback_to_primary_texture,
    texture_property_ids_for_binding,
};
use crate::materials::host_data::{
    MaterialPropertyLookupIds, MaterialPropertyStore, MaterialPropertyValue,
};
use crate::materials::{ReflectedRasterLayout, ReflectedUniformField, ReflectedUniformScalarKind};

use super::{
    LOD_BIAS_SUFFIX, MaterialUniformPackMetadata, STORAGE_V_INVERTED_SUFFIX,
    UniformPackTextureContext, WRAP_MODE_BITS_SUFFIX, binding_lod_bias, binding_storage_v_inverted,
    binding_wrap_mode_bits, missing_f32_uniform_default, missing_vec4_uniform_default,
    shader_writer_unescaped_field_name, storage_v_inverted_flag_value, write_f32_array_at,
    write_f32_at, write_f32_slice_at, write_f32x4_array_at, write_f32x4_at,
    write_srgb_f32x4_array_at, write_u32_at,
};

const KIND_VEC4: u8 = 0;
const KIND_F32: u8 = 1;
const KIND_LOD_BIAS: u8 = 2;
const KIND_STORAGE_V: u8 = 3;
const KIND_WRAP_BITS: u8 = 4;
const KIND_VARIANT_BITS: u8 = 5;
const KIND_ZERO_U32: u8 = 6;
const KIND_HOST_ARRAY: u8 = 7;
const KIND_VEC2: u8 = 8;
const KIND_VEC3: u8 = 9;
const KIND_F32_ARRAY: u8 = 10;
const KIND_VEC4_ARRAY: u8 = 11;
const FLAG_SRGB: u8 = 1;
const RENDERIDE_VARIANT_BITS_FIELD: &str = "_RenderideVariantBits";

#[derive(Debug)]
struct PlannedTexture {
    property_ids: Vec<i32>,
    fallback_primary: bool,
}

#[derive(Debug)]
struct PlannedField {
    offset: u32,
    size: u32,
    array_stride: u32,
    pid: i32,
    /// Default bits, vec4-default index, or planned-texture index. Meaning follows `kind`.
    aux: u32,
    flags: u8,
    kind: u8,
}

/// Field recipes for one reflected material uniform block.
#[derive(Debug)]
pub(crate) struct MaterialUniformPackPlan {
    total_size: usize,
    fields: Vec<PlannedField>,
    textures: Vec<PlannedTexture>,
    vec4_defaults: Vec<[f32; 4]>,
    /// `false` when the reflected block is missing or a field has no property id.
    ready: bool,
}

/// Identity of the layout a [`MaterialUniformPackPlan`] was compiled from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UniformPlanCacheKey {
    reflected: usize,
    ids: usize,
    defaults: usize,
}

/// Plan cached on the value-space object that was first packed with `key`.
#[derive(Debug)]
pub(super) struct CachedUniformPlan {
    pub(super) key: UniformPlanCacheKey,
    pub(super) plan: MaterialUniformPackPlan,
}

pub(super) fn uniform_plan_cache_key(
    reflected: &ReflectedRasterLayout,
    ids: &StemEmbeddedPropertyIds,
    defaults: &super::MaterialUniformDefaults,
) -> UniformPlanCacheKey {
    UniformPlanCacheKey {
        reflected: std::ptr::from_ref(reflected) as usize,
        ids: std::ptr::from_ref(ids) as usize,
        defaults: std::ptr::from_ref(defaults) as usize,
    }
}

impl MaterialUniformPackPlan {
    /// Classifies `reflected` once. An unready plan means the block is missing or a field has no
    /// property id; [`Self::pack_into`] then returns `false`.
    pub(crate) fn compile(
        reflected: &ReflectedRasterLayout,
        ids: &StemEmbeddedPropertyIds,
        metadata: &MaterialUniformPackMetadata<'_>,
    ) -> Self {
        let Some(uniform) = reflected.material_uniform.as_ref() else {
            return Self::unready();
        };
        let mut fields = Vec::with_capacity(uniform.fields.len());
        let mut textures = Vec::new();
        let mut vec4_defaults = Vec::new();
        let mut ctx = ClassifyCtx {
            reflected,
            ids,
            metadata,
            textures: &mut textures,
            vec4_defaults: &mut vec4_defaults,
        };
        for (field_name, field) in &uniform.fields {
            let Some(pid) = ctx.ids.uniform_field_ids.get(field_name).copied() else {
                return Self::unready();
            };
            fields.push(classify_field(field_name, field, pid, &mut ctx));
        }
        fields.sort_by_key(|field| field.offset);
        Self {
            total_size: usize::try_from(uniform.total_size).unwrap_or(0),
            fields,
            textures,
            vec4_defaults,
            ready: true,
        }
    }

    /// `true` when [`Self::compile`] saw a complete uniform block.
    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    /// Writes the uniform block into `out`, reusing its allocation. Returns `false` when this
    /// plan was not compiled from a complete uniform block.
    pub(crate) fn pack_into(
        &self,
        store: &MaterialPropertyStore,
        lookup: MaterialPropertyLookupIds,
        tex_ctx: &UniformPackTextureContext<'_>,
        shader_variant_bits: Option<u32>,
        out: &mut Vec<u8>,
    ) -> bool {
        if !self.ready {
            return false;
        }
        out.clear();
        out.resize(self.total_size, 0);
        let maps = store.merged_maps(lookup);
        for field in &self.fields {
            self.write_field(field, &maps, tex_ctx, shader_variant_bits, out);
        }
        true
    }

    fn unready() -> Self {
        Self {
            total_size: 0,
            fields: Vec::new(),
            textures: Vec::new(),
            vec4_defaults: Vec::new(),
            ready: false,
        }
    }

    fn write_field(
        &self,
        field: &PlannedField,
        maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
        tex_ctx: &UniformPackTextureContext<'_>,
        shader_variant_bits: Option<u32>,
        out: &mut [u8],
    ) {
        let reflected = ReflectedUniformField {
            offset: field.offset,
            size: field.size,
            kind: ReflectedUniformScalarKind::F32,
            array_stride: field.array_stride,
        };
        match field.kind {
            KIND_VEC4 => self.write_vec4(field, maps, &reflected, out),
            KIND_VEC2 => write_vec_prefix(field, maps, &reflected, 2, out),
            KIND_VEC3 => write_vec_prefix(field, maps, &reflected, 3, out),
            KIND_F32 => write_f32_at(out, &reflected, f32_property(field, maps)),
            KIND_LOD_BIAS => write_f32_at(
                out,
                &reflected,
                binding_lod_bias(self.resolve_texture(field.aux, maps, tex_ctx), tex_ctx),
            ),
            KIND_STORAGE_V => write_f32_at(
                out,
                &reflected,
                storage_v_inverted_flag_value(binding_storage_v_inverted(
                    self.resolve_texture(field.aux, maps, tex_ctx),
                    tex_ctx,
                )),
            ),
            KIND_WRAP_BITS => write_u32_at(
                out,
                &reflected,
                binding_wrap_mode_bits(self.resolve_texture(field.aux, maps, tex_ctx), tex_ctx),
            ),
            KIND_VARIANT_BITS => write_u32_at(out, &reflected, shader_variant_bits.unwrap_or(0)),
            KIND_ZERO_U32 => write_u32_at(out, &reflected, 0),
            KIND_HOST_ARRAY => write_host_array(field, maps, &reflected, out),
            KIND_F32_ARRAY => {
                if let Some(MaterialPropertyValue::FloatArray(values)) = maps.get(field.pid) {
                    write_f32_array_at(out, &reflected, values);
                }
            }
            KIND_VEC4_ARRAY => write_vec4_array(field, maps, &reflected, out),
            _ => {}
        }
    }

    fn write_vec4(
        &self,
        field: &PlannedField,
        maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
        reflected: &ReflectedUniformField,
        out: &mut [u8],
    ) {
        let value = if let Some(MaterialPropertyValue::Float4(color)) = maps.get(field.pid) {
            if field.flags & FLAG_SRGB != 0 {
                super::srgb_vec4_rgb_to_linear(*color)
            } else {
                *color
            }
        } else {
            self.vec4_defaults
                .get(usize::try_from(field.aux).unwrap_or(0))
                .copied()
                .unwrap_or([0.0; 4])
        };
        write_f32x4_at(out, reflected, &value);
    }

    fn resolve_texture(
        &self,
        texture_index: u32,
        maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
        tex_ctx: &UniformPackTextureContext<'_>,
    ) -> crate::materials::embedded::texture_resolve::ResolvedTextureBinding {
        let Some(planned) = self
            .textures
            .get(usize::try_from(texture_index).unwrap_or(0))
        else {
            return crate::materials::embedded::texture_resolve::ResolvedTextureBinding::None;
        };
        for &pid in &planned.property_ids {
            let binding = resolved_texture_binding_from_property(maps.get(pid));
            if !matches!(
                binding,
                crate::materials::embedded::texture_resolve::ResolvedTextureBinding::None
            ) {
                return binding;
            }
        }
        if planned.fallback_primary && tex_ctx.primary_texture_2d >= 0 {
            crate::materials::embedded::texture_resolve::ResolvedTextureBinding::Texture2D {
                asset_id: tex_ctx.primary_texture_2d,
            }
        } else {
            crate::materials::embedded::texture_resolve::ResolvedTextureBinding::None
        }
    }
}

fn classify_vec_prefix(mut planned: PlannedField, kind: u8) -> PlannedField {
    planned.kind = kind;
    planned
}

fn classify_host_array(mut planned: PlannedField, kind: u8, srgb: bool) -> PlannedField {
    planned.kind = kind;
    if srgb {
        planned.flags = FLAG_SRGB;
    }
    planned
}

fn write_vec_prefix(
    field: &PlannedField,
    maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
    reflected: &ReflectedUniformField,
    components: usize,
    out: &mut [u8],
) {
    let values = if let Some(MaterialPropertyValue::Float4(value)) = maps.get(field.pid) {
        *value
    } else {
        [0.0; 4]
    };
    write_f32_slice_at(out, reflected, &values[..components]);
}

fn write_vec4_array(
    field: &PlannedField,
    maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
    reflected: &ReflectedUniformField,
    out: &mut [u8],
) {
    if let Some(MaterialPropertyValue::Float4Array(values)) = maps.get(field.pid) {
        if field.flags & FLAG_SRGB != 0 {
            write_srgb_f32x4_array_at(out, reflected, values);
        } else {
            write_f32x4_array_at(out, reflected, values);
        }
    }
}

fn f32_property(
    field: &PlannedField,
    maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
) -> f32 {
    if let Some(MaterialPropertyValue::Float(value)) = maps.get(field.pid) {
        *value
    } else {
        f32::from_bits(field.aux)
    }
}

fn write_host_array(
    field: &PlannedField,
    maps: &crate::materials::host_data::MergedPropertyMaps<'_>,
    reflected: &ReflectedUniformField,
    out: &mut [u8],
) {
    if let Some(MaterialPropertyValue::FloatArray(values)) = maps.get(field.pid) {
        write_f32_array_at(out, reflected, values);
    } else if let Some(MaterialPropertyValue::Float4Array(values)) = maps.get(field.pid) {
        if field.flags & FLAG_SRGB != 0 {
            write_srgb_f32x4_array_at(out, reflected, values);
        } else {
            write_f32x4_array_at(out, reflected, values);
        }
    }
}

struct ClassifyCtx<'a> {
    reflected: &'a ReflectedRasterLayout,
    ids: &'a StemEmbeddedPropertyIds,
    metadata: &'a MaterialUniformPackMetadata<'a>,
    textures: &'a mut Vec<PlannedTexture>,
    vec4_defaults: &'a mut Vec<[f32; 4]>,
}

fn classify_field(
    field_name: &str,
    field: &ReflectedUniformField,
    pid: i32,
    ctx: &mut ClassifyCtx<'_>,
) -> PlannedField {
    let base = PlannedField {
        offset: field.offset,
        size: field.size,
        array_stride: field.array_stride,
        pid,
        aux: 0,
        flags: 0,
        kind: KIND_ZERO_U32,
    };
    match field.kind {
        ReflectedUniformScalarKind::Vec4 => classify_vec4(base, field_name, ctx),
        ReflectedUniformScalarKind::Vec2 => classify_vec_prefix(base, KIND_VEC2),
        ReflectedUniformScalarKind::Vec3 => classify_vec_prefix(base, KIND_VEC3),
        ReflectedUniformScalarKind::F32 => classify_f32(base, field_name, ctx),
        ReflectedUniformScalarKind::F32Array => classify_host_array(base, KIND_F32_ARRAY, false),
        ReflectedUniformScalarKind::Vec4Array => classify_host_array(
            base,
            KIND_VEC4_ARRAY,
            ctx.metadata.value_spaces.is_srgb_vec4_array(field_name),
        ),
        ReflectedUniformScalarKind::U32 => classify_u32(base, field_name, ctx),
        ReflectedUniformScalarKind::Unsupported => {
            let mut planned = base;
            planned.kind = KIND_HOST_ARRAY;
            if ctx.metadata.value_spaces.is_srgb_vec4_array(field_name) {
                planned.flags = FLAG_SRGB;
            }
            planned
        }
    }
}

fn classify_vec4(
    mut planned: PlannedField,
    field_name: &str,
    ctx: &mut ClassifyCtx<'_>,
) -> PlannedField {
    let default = missing_vec4_uniform_default(field_name, ctx.metadata.material_defaults);
    planned.aux = u32::try_from(ctx.vec4_defaults.len()).unwrap_or(0);
    ctx.vec4_defaults.push(default);
    planned.kind = KIND_VEC4;
    if ctx.metadata.value_spaces.is_srgb_vec4(field_name) {
        planned.flags = FLAG_SRGB;
    }
    planned
}

fn classify_f32(
    mut planned: PlannedField,
    field_name: &str,
    ctx: &mut ClassifyCtx<'_>,
) -> PlannedField {
    if let Some(texture_index) = push_texture_suffix(field_name, STORAGE_V_INVERTED_SUFFIX, ctx) {
        planned.kind = KIND_STORAGE_V;
        planned.aux = texture_index;
        return planned;
    }
    if let Some(texture_index) = push_texture_suffix(field_name, LOD_BIAS_SUFFIX, ctx) {
        planned.kind = KIND_LOD_BIAS;
        planned.aux = texture_index;
        return planned;
    }
    planned.kind = KIND_F32;
    planned.aux = missing_f32_uniform_default(field_name, ctx.metadata.material_defaults).to_bits();
    planned
}

fn classify_u32(
    mut planned: PlannedField,
    field_name: &str,
    ctx: &mut ClassifyCtx<'_>,
) -> PlannedField {
    if let Some(texture_index) = push_texture_suffix(field_name, WRAP_MODE_BITS_SUFFIX, ctx) {
        planned.kind = KIND_WRAP_BITS;
        planned.aux = texture_index;
        return planned;
    }
    if shader_writer_unescaped_field_name(field_name) == RENDERIDE_VARIANT_BITS_FIELD {
        planned.kind = KIND_VARIANT_BITS;
    }
    planned
}

/// Records a texture-suffix field when the stripped name is a group(1) texture.
///
/// `None` means the suffix does not apply, matching the runtime fall-through to a plain property.
fn push_texture_suffix(field_name: &str, suffix: &str, ctx: &mut ClassifyCtx<'_>) -> Option<u32> {
    let unescaped = shader_writer_unescaped_field_name(field_name);
    let tex_name = unescaped.strip_suffix(suffix)?;
    let (&binding, host_name) = ctx
        .reflected
        .material_group1_names
        .iter()
        .find(|(_, name)| name.as_str() == tex_name)?;
    let pids = texture_property_ids_for_binding(ctx.ids, binding);
    let planned = PlannedTexture {
        property_ids: pids.to_vec(),
        fallback_primary: !pids.is_empty() && should_fallback_to_primary_texture(host_name),
    };
    let index = u32::try_from(ctx.textures.len()).ok()?;
    ctx.textures.push(planned);
    Some(index)
}
