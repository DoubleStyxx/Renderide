use std::sync::Arc;

use super::*;
use crate::materials::host_data::{
    MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::{MaterialRouter, RasterPipelineKind, UNITY_RENDER_QUEUE_TRANSPARENT};

#[test]
fn generated_billboard_mesh_preserves_compatible_source_pipeline() {
    let mut store = MaterialPropertyStore::new();
    store.set_shader_asset_for_material(7, 99);
    let dict = MaterialDictionary::new(&store);
    let mut router = MaterialRouter::new(RasterPipelineKind::Null);
    router.set_shader_pipeline(
        99,
        RasterPipelineKind::EmbeddedStem(Arc::from("pbsmetallic_default")),
    );
    let ids = MaterialPipelinePropertyIds::new(&PropertyIdRegistry::new());
    let resolved =
        resolve_material_batch(7, None, &dict, &router, &ids, ShaderPermutation::default());
    let mut key = batch_key_from_resolved(
        7,
        None,
        false,
        RasterFrontFace::Clockwise,
        RasterPrimitiveTopology::TriangleList,
        &resolved,
    );
    let mesh_asset_id = crate::particles::billboard_render_buffer_mesh_asset_id(3).unwrap();

    apply_render_buffer_mesh_pipeline_override(
        &mut key,
        mesh_asset_id,
        ShaderPermutation::default(),
    );

    let RasterPipelineKind::EmbeddedStem(stem) = &key.pipeline else {
        panic!("expected embedded source pipeline");
    };
    assert_eq!(stem.as_ref(), "pbsmetallic_default");
    assert!(key.uses_render_buffer_billboard);
    assert!(key.embedded_needs_tangent);
    assert!(!key.embedded_raw_tangent_payload);
    assert!(!key.embedded_raw_normal_payload);
}

#[test]
fn generated_billboard_mesh_preserves_source_blend_for_compatible_pipeline() {
    let registry = PropertyIdRegistry::new();
    let render_queue = registry.intern("_RenderQueue");
    let mut store = MaterialPropertyStore::new();
    store.set_shader_asset_for_material(7, 99);
    store.set_material(
        7,
        render_queue,
        MaterialPropertyValue::Float(UNITY_RENDER_QUEUE_TRANSPARENT as f32),
    );
    let dict = MaterialDictionary::new(&store);
    let mut router = MaterialRouter::new(RasterPipelineKind::Null);
    router.set_shader_pipeline(
        99,
        RasterPipelineKind::EmbeddedStem(Arc::from("pbsvertexcolortransparent_default")),
    );
    let ids = MaterialPipelinePropertyIds::new(&registry);
    let resolved =
        resolve_material_batch(7, None, &dict, &router, &ids, ShaderPermutation::default());
    let mut key = batch_key_from_resolved(
        7,
        None,
        false,
        RasterFrontFace::Clockwise,
        RasterPrimitiveTopology::TriangleList,
        &resolved,
    );
    let mesh_asset_id = crate::particles::billboard_render_buffer_mesh_asset_id(3).unwrap();

    assert_eq!(key.blend_mode, MaterialBlendMode::StemDefault);
    assert_eq!(
        key.transparent_class,
        TransparentMaterialClass::OrderedAlpha
    );

    apply_render_buffer_mesh_pipeline_override(
        &mut key,
        mesh_asset_id,
        ShaderPermutation::default(),
    );

    assert_eq!(key.blend_mode, MaterialBlendMode::StemDefault);
    assert_eq!(
        key.transparent_class,
        TransparentMaterialClass::OrderedAlpha
    );
}

#[test]
fn custom_keyword_float_and_variant_bits_set_specialization() {
    let _ = crate::materials::custom_shader::remove("kw-resolve");
    let installed = crate::materials::custom_shader::install(
        "kw-resolve",
        r#"
//#keyword _EMISSION
//#keyword _ALPHATEST_ON

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
#ifdef _EMISSION
    return rg::retain_globals_additive(vec4<f32>(1.0, 0.0, 0.0, 1.0));
#else
    return rg::retain_globals_additive(mat._Color);
#endif
}
"#,
    )
    .expect("install keyword shader");

    let registry = PropertyIdRegistry::new();
    let emission = registry.intern("_EMISSION");
    let mut store = MaterialPropertyStore::new();
    store.set_shader_asset_for_material(7, 99);
    store.set_material(7, emission, MaterialPropertyValue::Float(3.0));
    let dict = MaterialDictionary::new(&store);
    let mut router = MaterialRouter::new(RasterPipelineKind::Null);
    router.set_shader_route(
        99,
        RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem.as_str())),
        None,
        Some(0b10),
    );
    let ids = MaterialPipelinePropertyIds::new(&registry);
    let resolved = resolve_material_batch_with_registry(
        7,
        None,
        &dict,
        &router,
        &ids,
        ShaderPermutation::default(),
        Some(&registry),
    );
    assert_eq!(
        resolved.shader_specialization,
        MaterialShaderSpecializationKey::from_variant_bits(0b11)
    );
    let _ = crate::materials::custom_shader::remove("kw-resolve");
}

#[test]
fn per_material_override_replaces_only_the_assigned_material() {
    let key = "per-mat-resolve";
    let _ = crate::materials::custom_shader::remove(key);
    let _ = crate::materials::custom_shader::clear_material_override(88011);
    let _ = crate::materials::custom_shader::clear_material_override(88012);
    let installed = crate::materials::custom_shader::install(
        key,
        include_str!("../../../../../../RenderideMod/Shaders/example_solid.wgsl"),
    )
    .expect("install");
    let mut store = MaterialPropertyStore::new();
    store.set_shader_asset_for_material(88011, 99);
    store.set_shader_asset_for_material(88012, 99);
    let dict = MaterialDictionary::new(&store);
    let mut router = MaterialRouter::new(RasterPipelineKind::Null);
    router.set_shader_pipeline(
        99,
        RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default")),
    );
    let ids = MaterialPipelinePropertyIds::new(&PropertyIdRegistry::new());
    let assign = renderide_shared::parse_custom_shader_payload(
        &renderide_shared::format_custom_shader_assign(88011, key),
    )
    .expect("prefix")
    .expect("header");
    crate::materials::custom_shader::resolve_custom_shader_payload(Ok(assign), |_| None);

    let assigned = resolve_material_batch(
        88011,
        None,
        &dict,
        &router,
        &ids,
        ShaderPermutation::default(),
    );
    let stock = resolve_material_batch(
        88012,
        None,
        &dict,
        &router,
        &ids,
        ShaderPermutation::default(),
    );
    assert_eq!(
        assigned.pipeline,
        RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem.as_str()))
    );
    assert_eq!(
        stock.pipeline,
        RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default"))
    );
    assert_ne!(
        crate::materials::custom_shader::source_generation(&installed.default_stem),
        0
    );

    let clear = renderide_shared::parse_custom_shader_payload(
        &renderide_shared::format_custom_shader_clear(88011),
    )
    .expect("prefix")
    .expect("header");
    crate::materials::custom_shader::resolve_custom_shader_payload(Ok(clear), |_| None);
    let restored = resolve_material_batch(
        88011,
        None,
        &dict,
        &router,
        &ids,
        ShaderPermutation::default(),
    );
    assert_eq!(
        restored.pipeline,
        RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default"))
    );
    crate::materials::custom_shader::remove(key).expect("remove");
}

/// Host `_ALPHATEST` is bit 0, the same index as custom `RAINBOW`. An override must ignore it.
///
/// Ids stay off `4101`/`4102`. The uniform-pack contract test uses those, and both tests share
/// the process-global override map.
#[test]
fn per_material_override_ignores_host_alphatest_bit() {
    const KEY: &str = "rainbow-override-bits";
    const ASSIGNED: i32 = 4301;
    const NAMED: i32 = 4302;
    let _guard = OverrideBitGuard(KEY, ASSIGNED, NAMED);
    let installed = crate::materials::custom_shader::install(
        KEY,
        include_str!("../../../../../../RenderideMod/Shaders/example_rainbow.wgsl"),
    )
    .expect("install");
    let registry = PropertyIdRegistry::new();
    let rainbow = registry.intern("RAINBOW");
    let mut store = MaterialPropertyStore::new();
    store.set_shader_asset_for_material(ASSIGNED, 99);
    store.set_shader_asset_for_material(NAMED, 100);
    store.set_material(ASSIGNED, rainbow, MaterialPropertyValue::Float(0.0));
    store.set_material(NAMED, rainbow, MaterialPropertyValue::Float(0.0));
    let mut router = MaterialRouter::new(RasterPipelineKind::Null);
    router.set_shader_route(
        99,
        RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default")),
        None,
        Some(1),
    );
    router.set_shader_route(
        100,
        RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem.as_str())),
        None,
        Some(1),
    );
    let ids = MaterialPipelinePropertyIds::new(&registry);
    let assign = renderide_shared::parse_custom_shader_payload(
        &renderide_shared::format_custom_shader_assign(ASSIGNED, KEY),
    )
    .expect("prefix")
    .expect("header");
    crate::materials::custom_shader::resolve_custom_shader_payload(Ok(assign), |_| None);

    let off = resolve_bits(ASSIGNED, &store, &router, &ids, &registry);
    assert_eq!(off, 0, "RAINBOW float 0 stays off when host bit 0 is set");
    store.set_material(ASSIGNED, rainbow, MaterialPropertyValue::Float(1.0));
    let on = resolve_bits(ASSIGNED, &store, &router, &ids, &registry);
    assert_eq!(
        on, 1,
        "RAINBOW float 1 enables keyword 0 without the host bit"
    );
    let named = resolve_bits(NAMED, &store, &router, &ids, &registry);
    assert_eq!(named, 1, "a shader-name route still ORs host bit 0");
}

/// Drops the override-bit test's key and material ids when an assert fails.
struct OverrideBitGuard(&'static str, i32, i32);

impl Drop for OverrideBitGuard {
    fn drop(&mut self) {
        let _ = crate::materials::custom_shader::remove(self.0);
        let _ = crate::materials::custom_shader::clear_material_override(self.1);
        let _ = crate::materials::custom_shader::clear_material_override(self.2);
    }
}

fn resolve_bits(
    material_id: i32,
    store: &MaterialPropertyStore,
    router: &MaterialRouter,
    ids: &MaterialPipelinePropertyIds,
    registry: &PropertyIdRegistry,
) -> u32 {
    let dict = MaterialDictionary::new(store);
    let resolved = resolve_material_batch_with_registry(
        material_id,
        None,
        &dict,
        router,
        ids,
        ShaderPermutation::default(),
        Some(registry),
    );
    resolved.shader_specialization.static_variant_bits()
}
