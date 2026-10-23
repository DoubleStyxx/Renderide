//! Renderer half of the dynamic-variable uniform contract.
//!
//! The mod maps `float` to `SetFloat`, `float2` / `float3` / `float4` to `SetFloat4` (missing
//! lanes zero), `colorX` to `SetColor` (which profiles, then `SetFloat4`), and
//! `IAssetProvider<ITexture2D>` to `SetTexture`. This packs those host values into the example
//! programs. A property write must not change `shader_source_generation`.

use super::super::{
    MaterialUniformDefaults, MaterialUniformPackMetadata, MaterialUniformValueSpaces,
    UniformPackTextureContext, build_embedded_uniform_bytes_with_material_defaults,
};
use super::common::{empty_texture_pools, lookup, read_f32_at, read_f32x4};
use crate::materials::custom_shader::{self, InstalledCustomShader};
use crate::materials::embedded::layout::StemEmbeddedPropertyIds;
use crate::materials::embedded::texture_pools::EmbeddedTexturePools;
use crate::materials::embedded::texture_resolve::{
    ResolvedTextureBinding, resolved_texture_binding_for_host, texture_property_ids_for_binding,
};
use crate::materials::host_data::{
    MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::wgsl_reflect::identifier_names::unescape_property_name;
use crate::materials::{
    ReflectedRasterLayout, ReflectedUniformField, ReflectedUniformScalarKind,
    reflect_raster_material_wgsl,
};

const SOLID_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_solid.wgsl");
const RAINBOW_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_rainbow.wgsl");
const POINCARE_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_poincare.wgsl");
const GYROID_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_gyroid.wgsl");
const HOLOGRAM_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_hologram.wgsl");
const HIEROGLYPH_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_hieroglyph.wgsl");
const NEBULA_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_nebula.wgsl");
const DISPERSION_WGSL: &str =
    include_str!("../../../../../../../RenderideMod/Shaders/example_dispersion.wgsl");

struct DynVarShaderContract {
    key: &'static str,
    source: &'static str,
    floats: &'static [&'static str],
    vec4s: &'static [&'static str],
    has_main_tex: bool,
}

const CONTRACTS: &[DynVarShaderContract] = &[
    DynVarShaderContract {
        key: "dynvar-reflect-solid",
        source: SOLID_WGSL,
        floats: &[],
        vec4s: &["_Color"],
        has_main_tex: false,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-rainbow",
        source: RAINBOW_WGSL,
        floats: &["_Speed"],
        vec4s: &["_Color", "_MainTex_ST"],
        has_main_tex: true,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-poincare",
        source: POINCARE_WGSL,
        floats: &["_Speed"],
        vec4s: &["_Color", "_MainTex_ST", "_Schlafli"],
        has_main_tex: true,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-gyroid",
        source: GYROID_WGSL,
        floats: &["_Frequency", "_Speed"],
        vec4s: &["_Color", "_MainTex_ST"],
        has_main_tex: true,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-hologram",
        source: HOLOGRAM_WGSL,
        floats: &["_Fresnel", "_Speed"],
        vec4s: &["_Color", "_MainTex_ST", "_Scan"],
        has_main_tex: true,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-hieroglyph",
        source: HIEROGLYPH_WGSL,
        floats: &["_Emission", "_Speed"],
        vec4s: &["_Color", "_MainTex_ST", "_Grid"],
        has_main_tex: true,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-nebula",
        source: NEBULA_WGSL,
        floats: &["_Speed"],
        vec4s: &["_Color", "_MainTex_ST", "_Warp"],
        has_main_tex: true,
    },
    DynVarShaderContract {
        key: "dynvar-reflect-dispersion",
        source: DISPERSION_WGSL,
        floats: &["_Spread", "_Speed"],
        vec4s: &["_Color", "_MainTex_ST"],
        has_main_tex: true,
    },
];

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
            quantize_toggle_floats: true,
        },
        store,
        lookup(material_id),
        &tex_ctx,
        None,
    )
    .expect("uniform bytes")
}

fn binding_named(loaded: &LoadedMaterial, name: &str) -> u32 {
    loaded
        .reflected
        .material_group1_names
        .iter()
        .find(|(_, binding_name)| unescape_property_name(binding_name) == name)
        .map(|(&binding, _)| binding)
        .unwrap_or_else(|| panic!("missing binding {name}"))
}

fn assert_texture_2d(loaded: &LoadedMaterial, name: &str) {
    let binding = binding_named(loaded, name);
    let entry = loaded
        .reflected
        .material_entries
        .iter()
        .find(|entry| entry.binding == binding)
        .unwrap_or_else(|| panic!("missing layout entry {name}"));
    match entry.ty {
        wgpu::BindingType::Texture {
            view_dimension: wgpu::TextureViewDimension::D2,
            ..
        } => {}
        other => panic!("{name} is {other:?}, not texture_2d"),
    }
}

fn assert_sampler(loaded: &LoadedMaterial, name: &str) {
    let binding = binding_named(loaded, name);
    let entry = loaded
        .reflected
        .material_entries
        .iter()
        .find(|entry| entry.binding == binding)
        .unwrap_or_else(|| panic!("missing layout entry {name}"));
    assert!(
        matches!(entry.ty, wgpu::BindingType::Sampler(..)),
        "{name} is {:?}, not a sampler",
        entry.ty
    );
}

#[test]
fn example_shaders_reflect_dynvar_uniform_names() {
    for contract in CONTRACTS {
        let _guard = RemoveKey::prepare(contract.key);
        let installed = install_key(contract.key, contract.source);
        let loaded = load_material(&installed);
        for name in contract.floats {
            assert_eq!(
                field(&loaded, name).kind,
                ReflectedUniformScalarKind::F32,
                "{} {name}",
                contract.key
            );
        }
        for name in contract.vec4s {
            assert_eq!(
                field(&loaded, name).kind,
                ReflectedUniformScalarKind::Vec4,
                "{} {name}",
                contract.key
            );
        }
        let uniforms = loaded
            .reflected
            .material_uniform
            .as_ref()
            .expect("uniform block");
        assert!(
            uniforms
                .fields
                .keys()
                .all(|name| unescape_property_name(name) != "_MainTex_sampler"),
            "_MainTex_sampler is not a uniform"
        );
        if contract.has_main_tex {
            assert_texture_2d(&loaded, "_MainTex");
            assert_sampler(&loaded, "_MainTex_sampler");
        }
    }
}

fn assert_f32(bytes: &[u8], loaded: &LoadedMaterial, name: &str, value: f32) {
    assert_eq!(
        read_f32_at(bytes, field(loaded, name).offset as usize),
        value,
        "{name}"
    );
}

fn assert_vec4(bytes: &[u8], loaded: &LoadedMaterial, name: &str, value: [f32; 4]) {
    assert_eq!(
        read_f32x4(bytes, field(loaded, name).offset as usize),
        value,
        "{name}"
    );
}

fn hologram_dynvar_store(loaded: &LoadedMaterial, material_id: i32) -> MaterialPropertyStore {
    let mut store = MaterialPropertyStore::new();
    // colorX arrives as the float4 SetColor already profiled. float2/float3 arrive padded.
    store.set_material(
        material_id,
        property_id(loaded, "_Color"),
        MaterialPropertyValue::Float4([0.2, 0.4, 0.6, 1.0]),
    );
    store.set_material(
        material_id,
        property_id(loaded, "_Scan"),
        MaterialPropertyValue::Float4([8.0, 0.25, 0.0, 0.0]),
    );
    store.set_material(
        material_id,
        property_id(loaded, "_Fresnel"),
        MaterialPropertyValue::Float(1.25),
    );
    store.set_material(
        material_id,
        property_id(loaded, "_Speed"),
        MaterialPropertyValue::Float(0.5),
    );
    store.set_material(
        material_id,
        property_id(loaded, "_MainTex_ST"),
        MaterialPropertyValue::Float4([2.0, 2.0, 0.1, 0.0]),
    );
    let texture_ids =
        texture_property_ids_for_binding(&loaded.ids, binding_named(loaded, "_MainTex"));
    let texture_property = texture_ids
        .iter()
        .copied()
        .find(|id| *id == loaded.registry.intern("_MainTex"))
        .expect("_MainTex property id");
    // Texture2D is type tag 0, so the packed handle is the asset id.
    store.set_material(
        material_id,
        texture_property,
        MaterialPropertyValue::Texture(42),
    );
    store
}

fn assert_hologram_defaults(bytes: &[u8], loaded: &LoadedMaterial) {
    assert_vec4(bytes, loaded, "_Color", [0.75, 0.9, 1.0, 1.0]);
    assert_vec4(bytes, loaded, "_Scan", [42.0, 0.55, 0.0, 0.0]);
    assert_f32(bytes, loaded, "_Fresnel", 3.0);
    assert_f32(bytes, loaded, "_Speed", 0.35);
    assert_vec4(bytes, loaded, "_MainTex_ST", [1.0, 1.0, 0.0, 0.0]);
}

fn assert_hologram_dynvars(bytes: &[u8], loaded: &LoadedMaterial) {
    assert_vec4(bytes, loaded, "_Color", [0.2, 0.4, 0.6, 1.0]);
    assert_vec4(bytes, loaded, "_Scan", [8.0, 0.25, 0.0, 0.0]);
    assert_f32(bytes, loaded, "_Fresnel", 1.25);
    assert_f32(bytes, loaded, "_Speed", 0.5);
    assert_vec4(bytes, loaded, "_MainTex_ST", [2.0, 2.0, 0.1, 0.0]);
}

#[test]
fn dynvar_property_writes_update_hologram_without_a_new_shader_generation() {
    let _guard = RemoveKey::prepare("dynvar-pack-hologram");
    let installed = install_key("dynvar-pack-hologram", HOLOGRAM_WGSL);
    let loaded = load_material(&installed);
    let generation = installed.generation;
    assert_eq!(
        custom_shader::source_generation(&installed.default_stem),
        generation
    );

    let material_id = 38;
    let defaults = pack(&loaded, &MaterialPropertyStore::new(), material_id);
    assert_hologram_defaults(&defaults, &loaded);

    let store = hologram_dynvar_store(&loaded, material_id);
    let written = pack(&loaded, &store, material_id);
    assert_hologram_dynvars(&written, &loaded);
    let texture_ids =
        texture_property_ids_for_binding(&loaded.ids, binding_named(&loaded, "_MainTex"));
    assert_eq!(
        resolved_texture_binding_for_host("_MainTex", texture_ids, -1, &store, lookup(material_id)),
        ResolvedTextureBinding::Texture2D { asset_id: 42 }
    );
    assert_eq!(
        custom_shader::source_generation(&installed.default_stem),
        generation,
        "a dynvar property write must not rebuild the shader"
    );

    let cleared = pack(&loaded, &MaterialPropertyStore::new(), material_id);
    assert_eq!(cleared, defaults);
}
