//! Custom-shader material properties: reflection, uniform layout, defaults, and updates.

use super::common::{empty_texture_pools, lookup, read_f32_at, read_f32x4};
use super::super::{
    MaterialUniformDefaults, MaterialUniformPackMetadata, MaterialUniformValueSpaces,
    UniformPackTextureContext, build_embedded_uniform_bytes_with_material_defaults,
};
use crate::embedded_shaders::{EmbeddedMaterialDefaultKind, EmbeddedMaterialDefaultValue};
use crate::materials::custom_shader::{self, InstalledCustomShader};
use crate::materials::embedded::layout::StemEmbeddedPropertyIds;
use crate::materials::embedded::texture_pools::EmbeddedTexturePools;
use crate::materials::embedded::texture_resolve::{
    DefaultTextureColor, ResolvedTextureBinding, default_2d_texture_color_for_host,
    resolved_texture_binding_for_host, texture_property_ids_for_binding,
};
use crate::materials::host_data::{
    MaterialPropertyLookupIds, MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::wgsl_reflect::identifier_names::unescape_property_name;
use crate::materials::{
    ReflectedRasterLayout, ReflectedUniformField, ReflectedUniformScalarKind,
    reflect_raster_material_wgsl,
};
use crate::render_contract::RasterPipelineKind;

const RAINBOW_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_rainbow.wgsl");

const LAYOUT_WGSL: &str = r#"
#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Dir: vec3<f32>,
    _Speed: f32,
    _Color: vec4<f32>,
    _Tints: array<vec4<f32>, 4>,
    _Weights: array<f32, 3>,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    let tint = mat._Tints[0] + mat._Tints[1] + mat._Tints[2] + mat._Tints[3];
    let weight = mat._Weights[0] + mat._Weights[1] + mat._Weights[2];
    let dir = vec4<f32>(mat._Dir, mat._Speed + weight);
    return rg::retain_globals_additive(mat._Color + tint + dir);
}
"#;

const I32_WGSL: &str = r#"
#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Bad: i32,
}

@group(1) @binding(0) var<uniform> mat: Material;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    return rg::retain_globals_additive(vec4<f32>(f32(mat._Bad), 0.0, 0.0, 1.0));
}
"#;

const MISMATCHED_DEFAULT_WGSL: &str = r#"
//#mat_default _Color float 0.5

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> mat: Material;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    return rg::retain_globals_additive(mat._Color);
}
"#;

const UNPAIRED_TEXTURE_WGSL: &str = r#"
#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    return rg::retain_globals_additive(mat._Color);
}
"#;

struct RemoveKey(&'static str);

impl RemoveKey {
    fn prepare(key: &'static str) -> Self {
        let _ = custom_shader::remove(key);
        Self(key)
    }
}

impl Drop for RemoveKey {
    fn drop(&mut self) {
        let _ = custom_shader::remove(self.0);
    }
}

struct LoadedMaterial {
    reflected: ReflectedRasterLayout,
    ids: StemEmbeddedPropertyIds,
    registry: PropertyIdRegistry,
    defaults: MaterialUniformDefaults,
}

fn install_key(key: &'static str, wgsl: &str) -> InstalledCustomShader {
    custom_shader::install(key, wgsl).unwrap_or_else(|error| panic!("install {key}: {error}"))
}

fn load_material(installed: &InstalledCustomShader) -> LoadedMaterial {
    let wgsl = custom_shader::target_wgsl(&installed.default_stem).expect("composed wgsl");
    let reflected = reflect_raster_material_wgsl(wgsl.as_ref()).expect("reflect composed wgsl");
    let registry = PropertyIdRegistry::new();
    let ids = StemEmbeddedPropertyIds::build(&registry, &reflected);
    let defaults = defaults_for(&installed.default_stem, &reflected);
    LoadedMaterial {
        reflected,
        ids,
        registry,
        defaults,
    }
}

fn defaults_for(stem: &str, reflected: &ReflectedRasterLayout) -> MaterialUniformDefaults {
    let mut defaults = MaterialUniformDefaults::default();
    let Some(listed) = custom_shader::material_uniform_defaults(stem) else {
        return defaults;
    };
    let Some(uniform) = reflected.material_uniform.as_ref() else {
        return defaults;
    };
    for default in listed.iter() {
        for (field_name, _) in &uniform.fields {
            if unescape_property_name(field_name) == default.property.as_ref() {
                defaults.insert(field_name.clone(), default.value);
            }
        }
    }
    defaults
}

fn field<'a>(loaded: &'a LoadedMaterial, name: &str) -> &'a ReflectedUniformField {
    loaded
        .reflected
        .material_uniform
        .as_ref()
        .expect("uniform block")
        .fields
        .iter()
        .find(|(field_name, _)| unescape_property_name(field_name) == name)
        .map(|(_, field)| field)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn property_id(loaded: &LoadedMaterial, name: &str) -> i32 {
    let field_name = loaded
        .reflected
        .material_uniform
        .as_ref()
        .expect("uniform block")
        .fields
        .keys()
        .find(|field_name| unescape_property_name(field_name) == name)
        .unwrap_or_else(|| panic!("missing field {name}"));
    *loaded
        .ids
        .uniform_field_ids
        .get(field_name)
        .unwrap_or_else(|| panic!("missing property id {name}"))
}

fn pack(loaded: &LoadedMaterial, store: &MaterialPropertyStore, material_id: i32) -> Vec<u8> {
    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_texture_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };
    let tex_ctx = UniformPackTextureContext {
        pools: &pools,
        primary_texture_2d: -1,
    };
    build_embedded_uniform_bytes_with_material_defaults(
        &loaded.reflected,
        &loaded.ids,
        &MaterialUniformPackMetadata {
            value_spaces: &MaterialUniformValueSpaces::default(),
            material_defaults: &loaded.defaults,
        },
        store,
        lookup(material_id),
        &tex_ctx,
        None,
    )
    .expect("uniform bytes")
}

fn listed_default(stem: &str, property: &str) -> EmbeddedMaterialDefaultValue {
    custom_shader::material_uniform_defaults(stem)
        .expect("defaults")
        .iter()
        .find(|default| default.property.as_ref() == property)
        .unwrap_or_else(|| panic!("missing default {property}"))
        .value
}

#[test]
fn rainbow_maps_group1_names_to_host_properties() {
    let _guard = RemoveKey::prepare("prop-rainbow-map");
    let installed = install_key("prop-rainbow-map", RAINBOW_WGSL);
    let loaded = load_material(&installed);

    let color = field(&loaded, "_Color");
    let speed = field(&loaded, "_Speed");
    assert_eq!(color.kind, ReflectedUniformScalarKind::Vec4);
    assert_eq!(speed.kind, ReflectedUniformScalarKind::F32);
    assert_eq!(property_id(&loaded, "_Color"), loaded.registry.intern("_Color"));
    assert_eq!(property_id(&loaded, "_Speed"), loaded.registry.intern("_Speed"));

    let (&binding, _) = loaded
        .reflected
        .material_group1_names
        .iter()
        .find(|(_, name)| unescape_property_name(name) == "_MainTex")
        .expect("_MainTex binding");
    let pids = texture_property_ids_for_binding(&loaded.ids, binding);
    assert!(pids.contains(&loaded.registry.intern("_MainTex")));
    assert!(pids.contains(&loaded.registry.intern("_Tex")));
    let sampler_binding = binding + 1;
    let sampler_name = loaded
        .reflected
        .material_group1_names
        .get(&sampler_binding)
        .map(|name| unescape_property_name(name));
    assert_eq!(sampler_name, Some("_MainTex_sampler"));
}

#[test]
fn packs_vec3_f32_vec4_arrays_and_mixed_types_at_uniform_alignment() {
    let _guard = RemoveKey::prepare("prop-layout");
    let installed = install_key("prop-layout", LAYOUT_WGSL);
    let loaded = load_material(&installed);
    let uniform = loaded
        .reflected
        .material_uniform
        .as_ref()
        .expect("uniform block");

    let dir = field(&loaded, "_Dir");
    let speed = field(&loaded, "_Speed");
    let color = field(&loaded, "_Color");
    let tints = field(&loaded, "_Tints");
    let weights = field(&loaded, "_Weights");

    assert_eq!(dir.offset, 0);
    assert_eq!(dir.size, 12);
    assert_eq!(dir.kind, ReflectedUniformScalarKind::Vec3);
    assert_eq!(speed.offset, 12, "f32 after vec3 stays at offset 12");
    assert_eq!(speed.size, 4);
    assert_eq!(speed.kind, ReflectedUniformScalarKind::F32);
    assert_eq!(color.offset, 16);
    assert_eq!(color.kind, ReflectedUniformScalarKind::Vec4);
    assert_eq!(tints.offset, 32);
    assert_eq!(tints.kind, ReflectedUniformScalarKind::Vec4Array);
    assert_eq!(tints.array_stride, 16);
    assert_eq!(tints.size, 64);
    assert_eq!(weights.offset, 96);
    assert_eq!(weights.kind, ReflectedUniformScalarKind::F32Array);
    assert_eq!(weights.array_stride, 16);
    assert_eq!(weights.size, 48);
    assert_eq!(uniform.total_size, 144);

    let mut store = MaterialPropertyStore::new();
    let material_id = 41;
    store.set_material(
        material_id,
        property_id(&loaded, "_Dir"),
        MaterialPropertyValue::Float4([1.0, 2.0, 3.0, 9.0]),
    );
    store.set_material(
        material_id,
        property_id(&loaded, "_Speed"),
        MaterialPropertyValue::Float(0.5),
    );
    store.set_material(
        material_id,
        property_id(&loaded, "_Color"),
        MaterialPropertyValue::Float4([4.0, 5.0, 6.0, 7.0]),
    );
    store.set_material(
        material_id,
        property_id(&loaded, "_Tints"),
        MaterialPropertyValue::Float4Array(vec![[8.0, 0.0, 0.0, 1.0], [0.0, 8.0, 0.0, 1.0]]),
    );
    store.set_material(
        material_id,
        property_id(&loaded, "_Weights"),
        MaterialPropertyValue::FloatArray(vec![0.25, 0.5]),
    );

    let bytes = pack(&loaded, &store, material_id);
    assert_eq!(read_f32_at(&bytes, 0), 1.0);
    assert_eq!(read_f32_at(&bytes, 4), 2.0);
    assert_eq!(read_f32_at(&bytes, 8), 3.0);
    assert_eq!(read_f32_at(&bytes, 12), 0.5);
    assert_eq!(read_f32x4(&bytes, 16), [4.0, 5.0, 6.0, 7.0]);
    assert_eq!(read_f32x4(&bytes, 32), [8.0, 0.0, 0.0, 1.0]);
    assert_eq!(read_f32x4(&bytes, 48), [0.0, 8.0, 0.0, 1.0]);
    assert_eq!(read_f32x4(&bytes, 64), [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(read_f32_at(&bytes, 96), 0.25);
    assert_eq!(read_f32_at(&bytes, 112), 0.5);
    assert_eq!(read_f32_at(&bytes, 128), 0.0);
}

#[test]
fn unset_properties_use_mat_default_zero_or_white() {
    let _guard = RemoveKey::prepare("prop-defaults");
    let installed = install_key("prop-defaults", RAINBOW_WGSL);
    let loaded = load_material(&installed);
    let bytes = pack(&loaded, &MaterialPropertyStore::new(), 8);

    let color = listed_default(&installed.default_stem, "_Color");
    let speed = listed_default(&installed.default_stem, "_Speed");
    assert_eq!(color.kind, EmbeddedMaterialDefaultKind::Vec4);
    assert_eq!(color.values, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(speed.kind, EmbeddedMaterialDefaultKind::Float);
    assert_eq!(speed.values[0], 0.2);
    assert_eq!(
        read_f32x4(&bytes, field(&loaded, "_Color").offset as usize),
        [1.0, 1.0, 1.0, 1.0]
    );
    assert_eq!(
        read_f32_at(&bytes, field(&loaded, "_Speed").offset as usize),
        0.2
    );

    let _layout_guard = RemoveKey::prepare("prop-defaults-vec3");
    let layout = install_key("prop-defaults-vec3", LAYOUT_WGSL);
    let layout_loaded = load_material(&layout);
    let zeroed = pack(&layout_loaded, &MaterialPropertyStore::new(), 9);
    assert_eq!(read_f32_at(&zeroed, 0), 0.0);
    assert_eq!(read_f32_at(&zeroed, 4), 0.0);
    assert_eq!(read_f32_at(&zeroed, 8), 0.0);
    assert_eq!(read_f32_at(&zeroed, 12), 0.0);

    let (&binding, _) = layout_loaded
        .reflected
        .material_group1_names
        .iter()
        .find(|(_, name)| unescape_property_name(name) == "_MainTex")
        .expect("_MainTex");
    let pids = texture_property_ids_for_binding(&layout_loaded.ids, binding);
    let resolved = resolved_texture_binding_for_host(
        "_MainTex",
        pids,
        -1,
        &MaterialPropertyStore::new(),
        MaterialPropertyLookupIds {
            material_asset_id: 9,
            mesh_property_block_slot0: None,
            mesh_renderer_property_block_id: None,
        },
    );
    assert_eq!(resolved, ResolvedTextureBinding::None);
    assert_eq!(
        default_2d_texture_color_for_host("_MainTex"),
        DefaultTextureColor::White
    );
}

#[test]
fn property_update_changes_uniform_bytes_without_shader_generation_change() {
    let _guard = RemoveKey::prepare("prop-update");
    let installed = install_key("prop-update", RAINBOW_WGSL);
    let loaded = load_material(&installed);
    let generation = custom_shader::source_generation(&installed.default_stem);
    let before = pack(&loaded, &MaterialPropertyStore::new(), 15);
    assert_eq!(
        read_f32_at(&before, field(&loaded, "_Speed").offset as usize),
        0.2
    );

    let mut store = MaterialPropertyStore::new();
    store.set_material(
        15,
        property_id(&loaded, "_Speed"),
        MaterialPropertyValue::Float(1.5),
    );
    store.set_material(
        15,
        property_id(&loaded, "_Color"),
        MaterialPropertyValue::Float4([0.25, 0.5, 0.75, 1.0]),
    );
    let after = pack(&loaded, &store, 15);

    assert_ne!(before, after);
    assert_eq!(
        read_f32_at(&after, field(&loaded, "_Speed").offset as usize),
        1.5
    );
    assert_eq!(
        read_f32x4(&after, field(&loaded, "_Color").offset as usize),
        [0.25, 0.5, 0.75, 1.0]
    );
    assert_eq!(
        custom_shader::source_generation(&installed.default_stem),
        generation,
        "a property write must not bump shader_source_generation or rebuild the pipeline"
    );
}

#[test]
fn rejects_unsupported_types_mismatched_defaults_and_unpaired_textures() {
    let _keep = RemoveKey::prepare("prop-keep");
    let kept = install_key("prop-keep", RAINBOW_WGSL);
    let generation = custom_shader::source_generation(&kept.default_stem);

    let unsupported = custom_shader::install("prop-keep", I32_WGSL).expect_err("i32");
    assert!(
        unsupported.contains("unsupported"),
        "i32 should be rejected: {unsupported}"
    );
    assert!(custom_shader::target_wgsl(&kept.default_stem).is_some());
    assert_eq!(
        custom_shader::source_generation(&kept.default_stem),
        generation
    );

    let mismatched =
        custom_shader::install("prop-mismatch", MISMATCHED_DEFAULT_WGSL).expect_err("default");
    assert!(
        mismatched.contains("does not match"),
        "float default on vec4 should be rejected: {mismatched}"
    );
    assert!(custom_shader::default_stem_for_asset_name("prop-mismatch").is_none());

    let unpaired =
        custom_shader::install("prop-unpaired", UNPAIRED_TEXTURE_WGSL).expect_err("sampler");
    assert!(
        unpaired.contains("sampler"),
        "texture without sampler should be rejected: {unpaired}"
    );

    let resolved = custom_shader::resolve_custom_shader_payload(
        Ok(renderide_shared::custom_shader_ipc::CustomShaderPayload {
            key: "prop-rejected-upload".to_string(),
            remove: false,
            wgsl: I32_WGSL.to_string(),
        }),
        |_| None,
    );
    assert!(resolved.rejected);
    assert!(matches!(resolved.pipeline, RasterPipelineKind::Null));
    assert!(resolved.retarget_shader_key.is_none());
    assert!(custom_shader::default_stem_for_asset_name("prop-rejected-upload").is_none());
}
