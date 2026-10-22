//! CPU contract for `CustomShaderMaterial`: one material on an installed key, the other on
//! packaged Unlit, property writes on the assigned material, then clear.

use std::sync::Arc;

use crate::materials::custom_shader::{self, InstalledCustomShader};
use crate::materials::embedded::layout::StemEmbeddedPropertyIds;
use crate::materials::embedded::texture_pools::EmbeddedTexturePools;
use crate::materials::embedded::uniform_pack::{
    MaterialUniformDefaults, MaterialUniformPackMetadata, MaterialUniformValueSpaces,
    UniformPackTextureContext, build_embedded_uniform_bytes_with_material_defaults,
};
use crate::materials::host_data::{
    MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::wgsl_reflect::identifier_names::unescape_property_name;
use crate::materials::{RasterPipelineKind, ReflectedRasterLayout, reflect_raster_material_wgsl};

use super::common::{empty_texture_pools, lookup, read_f32_at, read_f32x4};

const RAINBOW_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_rainbow.wgsl");
const KEY: &str = "MaterialContractRainbow";
const ASSIGNED: i32 = 41_01;
const STOCK: i32 = 41_02;
const UNLIT: &str = "unlit_default";
/// Host Unlit `_COLOR` bit. Custom `RAINBOW` is bit 0, so this bit must not force the hue cycle.
const UNLIT_COLOR_BIT: u32 = 1 << 1;

struct Loaded {
    stem: String,
    ids: StemEmbeddedPropertyIds,
    registry: PropertyIdRegistry,
    defaults: MaterialUniformDefaults,
    reflected: ReflectedRasterLayout,
}

struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = custom_shader::remove(KEY);
        let _ = custom_shader::clear_material_override(ASSIGNED);
        let _ = custom_shader::clear_material_override(STOCK);
    }
}

#[test]
fn assign_packs_one_material_and_clear_restores_stock_unlit() {
    let _guard = Guard;
    let unlit = RasterPipelineKind::EmbeddedStem(Arc::from(UNLIT));
    let assigned_early = renderide_shared::format_custom_shader_assign(ASSIGNED, KEY);
    assert_eq!(
        assigned_early,
        format!("renderide-custom-wgsl:v1\nop=assign\nmaterial={ASSIGNED}\nkey={KEY}\n---\n")
    );
    apply_payload(&assigned_early);
    assert_eq!(
        custom_shader::apply_material_shader_override(ASSIGNED, unlit.clone()),
        unlit,
        "assign before install keeps the packaged route"
    );

    let installed = install();
    let generation = custom_shader::source_generation(&installed.default_stem);
    let custom = RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem.as_str()));
    assert_eq!(
        custom_shader::apply_material_shader_override(ASSIGNED, unlit.clone()),
        custom
    );
    assert_eq!(
        custom_shader::apply_material_shader_override(STOCK, unlit.clone()),
        unlit,
        "the other material stays on packaged Unlit"
    );

    let loaded = load(&installed);
    let mut store = assigned_store(&loaded);
    assert_assigned_uniforms(&loaded, &store, generation);
    store.set_material(
        ASSIGNED,
        loaded.registry.intern("RAINBOW"),
        MaterialPropertyValue::Float(0.0),
    );
    assert_eq!(
        keyword_mask(&loaded, &store, ASSIGNED),
        Some(0),
        "a zero RAINBOW float disables the keyword even when the Unlit color bit is set"
    );

    let clear = renderide_shared::format_custom_shader_clear(ASSIGNED);
    assert!(clear.contains(&format!("material={ASSIGNED}")));
    assert!(!clear.contains("key="));
    apply_payload(&clear);
    assert_eq!(
        custom_shader::apply_material_shader_override(ASSIGNED, unlit.clone()),
        unlit
    );
    assert_eq!(
        custom_shader::apply_material_shader_override(STOCK, unlit),
        RasterPipelineKind::EmbeddedStem(Arc::from(UNLIT))
    );
}

fn assert_assigned_uniforms(loaded: &Loaded, store: &MaterialPropertyStore, generation: u64) {
    let assigned_bytes = pack(loaded, store, ASSIGNED);
    let stock_bytes = pack(loaded, store, STOCK);
    assert_eq!(
        read_f32x4(&assigned_bytes, field_offset(loaded, "_Color")),
        [0.2, 0.4, 0.9, 1.0]
    );
    assert_eq!(
        read_f32_at(&assigned_bytes, field_offset(loaded, "_Speed")),
        0.0
    );
    assert_eq!(
        read_f32x4(&assigned_bytes, field_offset(loaded, "_MainTex_ST")),
        [1.0, 1.0, 0.0, 0.0]
    );
    assert_eq!(
        read_f32x4(&stock_bytes, field_offset(loaded, "_Color")),
        [1.0, 1.0, 1.0, 1.0],
        "the stock material does not inherit the assigned material's properties"
    );
    assert_eq!(
        read_f32_at(&stock_bytes, field_offset(loaded, "_Speed")),
        0.2
    );
    assert_eq!(keyword_mask(loaded, store, ASSIGNED), Some(1));
    assert_eq!(keyword_mask(loaded, store, STOCK), Some(0));
    let rainbow = custom_shader::variant_wgsl(&loaded.stem, 1)
        .expect("compose rainbow")
        .expect("rainbow variant");
    let plain = custom_shader::target_wgsl(&loaded.stem).expect("mask 0");
    assert!(
        rainbow.contains("frame_time.x"),
        "RAINBOW composes the hue cycle"
    );
    assert!(
        !plain.contains("frame_time.x"),
        "mask 0 drops the hue cycle"
    );
    assert_eq!(custom_shader::source_generation(&loaded.stem), generation);
}

fn assigned_store(loaded: &Loaded) -> MaterialPropertyStore {
    let mut store = MaterialPropertyStore::new();
    store.set_material(
        ASSIGNED,
        property_id(loaded, "_Color"),
        MaterialPropertyValue::Float4([0.2, 0.4, 0.9, 1.0]),
    );
    store.set_material(
        ASSIGNED,
        property_id(loaded, "_Speed"),
        MaterialPropertyValue::Float(0.0),
    );
    store.set_material(
        ASSIGNED,
        loaded.registry.intern("RAINBOW"),
        MaterialPropertyValue::Float(1.0),
    );
    assert_eq!(
        loaded.registry.intern("_Color"),
        property_id(loaded, "_Color")
    );
    store
}

fn install() -> InstalledCustomShader {
    let file = renderide_shared::format_custom_shader_payload(KEY, RAINBOW_WGSL);
    let payload = renderide_shared::parse_custom_shader_payload(&file)
        .expect("prefix")
        .expect("install header");
    assert!(matches!(
        payload.op,
        renderide_shared::CustomShaderOp::Install
    ));
    custom_shader::install(KEY, RAINBOW_WGSL).expect("install")
}

fn apply_payload(file: &str) {
    let payload = renderide_shared::parse_custom_shader_payload(file)
        .expect("prefix")
        .expect("header");
    let resolved = custom_shader::resolve_custom_shader_payload(Ok(payload), |_| None);
    assert!(!resolved.rejected);
    assert!(matches!(resolved.pipeline, RasterPipelineKind::Null));
}

fn load(installed: &InstalledCustomShader) -> Loaded {
    let wgsl = custom_shader::target_wgsl(&installed.default_stem).expect("composed wgsl");
    let reflected = reflect_raster_material_wgsl(wgsl.as_ref()).expect("reflect");
    let registry = PropertyIdRegistry::new();
    let ids = StemEmbeddedPropertyIds::build(&registry, &reflected);
    let defaults = defaults_for(&installed.default_stem, &reflected);
    Loaded {
        stem: installed.default_stem.clone(),
        ids,
        registry,
        defaults,
        reflected,
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

fn property_id(loaded: &Loaded, name: &str) -> i32 {
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

fn field_offset(loaded: &Loaded, name: &str) -> usize {
    loaded
        .reflected
        .material_uniform
        .as_ref()
        .expect("uniform block")
        .fields
        .iter()
        .find(|(field_name, _)| unescape_property_name(field_name) == name)
        .map(|(_, field)| field.offset as usize)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn pack(loaded: &Loaded, store: &MaterialPropertyStore, material_id: i32) -> Vec<u8> {
    let (texture, texture3d, cubemap, render_texture, video_texture) = empty_texture_pools();
    let pools = EmbeddedTexturePools {
        texture: &texture,
        texture3d: &texture3d,
        cubemap: &cubemap,
        render_texture: &render_texture,
        video_texture: &video_texture,
    };
    build_embedded_uniform_bytes_with_material_defaults(
        &loaded.reflected,
        &loaded.ids,
        &MaterialUniformPackMetadata {
            value_spaces: &MaterialUniformValueSpaces::default(),
            material_defaults: &loaded.defaults,
            quantize_toggle_floats: true,
        },
        store,
        lookup(material_id),
        &UniformPackTextureContext {
            pools: &pools,
            primary_texture_2d: -1,
        },
        None,
    )
    .expect("uniform bytes")
}

fn keyword_mask(loaded: &Loaded, store: &MaterialPropertyStore, material_id: i32) -> Option<u32> {
    custom_shader::keyword_mask(&loaded.stem, Some(UNLIT_COLOR_BIT), |name| {
        let property_id = loaded.registry.intern(name);
        match store.get_material(material_id, property_id) {
            Some(MaterialPropertyValue::Float(value)) => Some(*value),
            _ => None,
        }
    })
}
