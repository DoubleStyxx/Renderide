use super::consts::{asset_ids, shader_variants};
use super::scene_setup::material_property_names;
use super::{SessionTemplate, build_scene_setup, scene_material_id};

#[test]
fn torus_unlit_perlin_requests_texture_variant() {
    let setup = build_scene_setup(SessionTemplate::Torus {
        texture_rgba: vec![255, 255, 255, 255],
        texture_size: (1, 1),
    });
    let binding = setup.materials[0]
        .binding
        .as_ref()
        .expect("torus case must upload an unlit material");

    assert_eq!(binding.shader_name, "Unlit.shader");
    assert_eq!(
        binding.shader_variant_bits,
        Some(shader_variants::UNLIT_TEXTURE)
    );
}

#[test]
fn material_property_names_follow_payload_order() {
    let setup = build_scene_setup(SessionTemplate::Torus {
        texture_rgba: vec![255, 255, 255, 255],
        texture_size: (1, 1),
    });
    let binding = setup.materials[0].binding.as_ref().expect("binding");
    assert_eq!(
        material_property_names(binding),
        vec!["_Tex", "_Tex_ST", "_Color"]
    );
}

#[test]
fn torus_material_update_buffers_do_not_overlap_texture_buffer() {
    let material_buffers =
        asset_ids::MATERIAL_UPDATE_BASE_BUFFER..=asset_ids::MATERIAL_UPDATE_BASE_BUFFER + 3;

    assert!(!material_buffers.contains(&asset_ids::TEXTURE_DATA_BUFFER));
}

#[test]
fn pbs_premultiply_sets_keyword_blend_and_queue() {
    let setup = build_scene_setup(SessionTemplate::PbsMetallicPremultiply);
    let binding = setup.materials[0].binding.as_ref().expect("binding");
    assert_eq!(binding.shader_name, "PBSMetallic.shader");
    assert_eq!(
        binding.shader_variant_bits,
        Some(shader_variants::PBSMETALLIC_ALPHAPREMULTIPLY)
    );
    let floats: Vec<(&str, f32)> = binding
        .floats
        .iter()
        .map(|value| (value.property_name, value.value))
        .collect();
    assert!(floats.contains(&("_SrcBlend", 1.0)));
    assert!(floats.contains(&("_DstBlend", 10.0)));
    assert!(floats.contains(&("_ZWrite", 0.0)));
    assert!(floats.contains(&("_RenderQueue", 3000.0)));
    assert!((binding.float4s[0].value[3] - 0.45).abs() < f32::EPSILON);
}

#[test]
fn pbs_specular_premultiply_sets_keyword_blend_and_queue() {
    let setup = build_scene_setup(SessionTemplate::PbsSpecularPremultiply);
    let binding = setup.materials[0].binding.as_ref().expect("binding");
    assert_eq!(binding.shader_name, "PBSSpecular.shader");
    assert_eq!(
        binding.shader_variant_bits,
        Some(shader_variants::PBSSPECULAR_ALPHAPREMULTIPLY)
    );
    let floats: Vec<(&str, f32)> = binding
        .floats
        .iter()
        .map(|value| (value.property_name, value.value))
        .collect();
    assert!(floats.contains(&("_SrcBlend", 1.0)));
    assert!(floats.contains(&("_DstBlend", 10.0)));
    assert!(floats.contains(&("_ZWrite", 0.0)));
    assert!(floats.contains(&("_RenderQueue", 3000.0)));
    assert!(floats.contains(&("_Glossiness", 0.55)));
    assert_eq!(binding.float4s[0].property_name, "_Color");
    assert!((binding.float4s[0].value[3] - 0.45).abs() < f32::EPSILON);
    assert_eq!(binding.float4s[1].property_name, "_SpecColor");
    assert_eq!(binding.float4s[1].value, [0.8, 0.75, 0.65, 1.0]);
}

#[test]
fn pbs_matrix_builds_four_materials_against_one_mesh() {
    let setup = build_scene_setup(SessionTemplate::PbsLitMaterialMatrix);
    assert_eq!(setup.meshes.len(), 1);
    assert_eq!(setup.materials.len(), 4);
    assert_eq!(setup.scene.renderables.len(), 4);
    assert_eq!(setup.scene.lights.len(), 2);
    assert_eq!(
        setup.scene.renderables[3].material_asset_id,
        scene_material_id(3)
    );
}
