//! One capture of two custom shaders side by side. Poincare is on the left, the gyroid on the
//! right. Both materials set `_Speed` to 0 so the frame does not depend on `frame_time`.

use glam::{Quat, Vec3};
use renderide_shared::shared::{ColorProfile, ShadowCastMode};
use renderide_shared::{format_custom_shader_assign, format_custom_shader_payload};

use crate::error::HarnessError;
use crate::scene::primitives::{centered_bounds, generate_quad};
use crate::scene::textures::checker_rgba;

use super::super::asset_upload::{DEFAULT_ASSET_UPLOAD_TIMEOUT, upload_shader_file};
use super::super::lockstep::LockstepDriver;
use super::consts::shader_variants;
use super::{
    Float4Binding, FloatBinding, MaterialAsset, MaterialBinding, SceneRenderable, SceneSetup,
    SceneSubmission, TextureBinding, mesh_asset, scene_material_id,
    scene_material_update_buffer_id, scene_property_request_id, scene_shader_id,
    scene_texture_buffer_id, scene_texture_id, scene_update_batch_id, transform,
};

const POINCARE_KEY: &str = "ExamplePoincare";
const GYROID_KEY: &str = "ExampleGyroid";
const POINCARE_INSTALL_ASSET_ID: i32 = 541;
const POINCARE_ASSIGN_ASSET_ID: i32 = 542;
const GYROID_INSTALL_ASSET_ID: i32 = 543;
const GYROID_ASSIGN_ASSET_ID: i32 = 544;
const QUAD_SCALE: f32 = 1.5;
const MAIN_TEX_ST: [f32; 4] = [1.0, 1.0, 0.0, 0.0];
const POINCARE_COLOR: [f32; 4] = [0.95, 0.78, 0.42, 1.0];
const GYROID_COLOR: [f32; 4] = [0.22, 0.72, 0.95, 1.0];
const SCHLAFLI: [f32; 4] = [7.0, 3.0, 0.0, 0.0];

const POINCARE_WGSL: &str =
    include_str!("../../../../../RenderideMod/Shaders/example_poincare.wgsl");
const GYROID_WGSL: &str = include_str!("../../../../../RenderideMod/Shaders/example_gyroid.wgsl");

pub(super) fn scene_setup() -> SceneSetup {
    let checker = checker_rgba(128, 128, 8, [240, 236, 220, 255], [28, 36, 58, 255]);
    let texture_size = (checker.width(), checker.height());
    let checker_bytes = checker.into_raw();
    SceneSetup {
        meshes: vec![mesh_asset(
            0,
            generate_quad(),
            centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
        )],
        materials: vec![
            custom_quad_material(CustomQuadMaterial {
                index: 0,
                texture: checker_bytes.clone(),
                texture_size,
                floats: vec![
                    FloatBinding {
                        property_name: "_Speed",
                        value: 0.0,
                    },
                    FloatBinding {
                        property_name: "TILE",
                        value: 1.0,
                    },
                ],
                float4s: vec![
                    Float4Binding {
                        property_name: "_Color",
                        value: POINCARE_COLOR,
                    },
                    Float4Binding {
                        property_name: "_Schlafli",
                        value: SCHLAFLI,
                    },
                ],
            }),
            custom_quad_material(CustomQuadMaterial {
                index: 1,
                texture: checker_bytes,
                texture_size,
                floats: vec![
                    FloatBinding {
                        property_name: "_Speed",
                        value: 0.0,
                    },
                    FloatBinding {
                        property_name: "_Frequency",
                        value: 6.0,
                    },
                    FloatBinding {
                        property_name: "GYROID",
                        value: 1.0,
                    },
                ],
                float4s: vec![Float4Binding {
                    property_name: "_Color",
                    value: GYROID_COLOR,
                }],
            }),
        ],
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.0, -3.4), Vec3::ONE, Quat::IDENTITY),
            renderables: vec![
                SceneRenderable {
                    transform_id: 0,
                    pose: transform(
                        Vec3::new(-0.82, 0.0, 0.0),
                        Vec3::splat(QUAD_SCALE),
                        Quat::IDENTITY,
                    ),
                    mesh_asset_id: super::scene_mesh_id(0),
                    material_asset_id: scene_material_id(0),
                    sorting_order: 0,
                    shadow_cast_mode: ShadowCastMode::Off,
                },
                SceneRenderable {
                    transform_id: 1,
                    pose: transform(
                        Vec3::new(0.82, 0.0, 0.0),
                        Vec3::splat(QUAD_SCALE),
                        Quat::IDENTITY,
                    ),
                    mesh_asset_id: super::scene_mesh_id(0),
                    material_asset_id: scene_material_id(1),
                    sorting_order: 0,
                    shadow_cast_mode: ShadowCastMode::Off,
                },
            ],
            lights: Vec::new(),
            lod_group: None,
        },
    }
}

struct CustomQuadMaterial {
    index: i32,
    texture: Vec<u8>,
    texture_size: (u32, u32),
    floats: Vec<FloatBinding>,
    float4s: Vec<Float4Binding>,
}

fn custom_quad_material(spec: CustomQuadMaterial) -> MaterialAsset {
    MaterialAsset {
        asset_id: scene_material_id(spec.index),
        update_batch_id: scene_update_batch_id(spec.index),
        property_request_id: scene_property_request_id(spec.index),
        update_base_buffer_id: scene_material_update_buffer_id(spec.index),
        binding: Some(MaterialBinding {
            shader_asset_id: scene_shader_id(spec.index),
            shader_name: "Unlit.shader",
            shader_variant_bits: Some(shader_variants::UNLIT_COLOR),
            textures: vec![TextureBinding {
                asset_id: scene_texture_id(spec.index),
                buffer_id: scene_texture_buffer_id(spec.index),
                property_name: "_MainTex",
                st_property_name: "_MainTex_ST",
                st_value: MAIN_TEX_ST,
                rgba_bytes: spec.texture,
                size: spec.texture_size,
                color_profile: ColorProfile::SRGB,
            }],
            floats: spec.floats,
            float4s: spec.float4s,
        }),
    }
}

/// Installs both programs and assigns each to its scene material.
pub(super) fn install_and_assign(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    setup: &SceneSetup,
) -> Result<(), HarnessError> {
    upload_shader_file(
        queues,
        lockstep,
        POINCARE_INSTALL_ASSET_ID,
        &format_custom_shader_payload(POINCARE_KEY, POINCARE_WGSL),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    upload_shader_file(
        queues,
        lockstep,
        POINCARE_ASSIGN_ASSET_ID,
        &format_custom_shader_assign(setup.materials[0].asset_id, POINCARE_KEY),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    upload_shader_file(
        queues,
        lockstep,
        GYROID_INSTALL_ASSET_ID,
        &format_custom_shader_payload(GYROID_KEY, GYROID_WGSL),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    upload_shader_file(
        queues,
        lockstep,
        GYROID_ASSIGN_ASSET_ID,
        &format_custom_shader_assign(setup.materials[1].asset_id, GYROID_KEY),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_materials_pin_speed_to_zero() {
        let setup = scene_setup();
        assert_eq!(setup.materials.len(), 2);
        for material in &setup.materials {
            let binding = material.binding.as_ref().expect("binding");
            assert_eq!(binding.shader_name, "Unlit.shader");
            assert_eq!(binding.textures[0].property_name, "_MainTex");
            let speed = binding
                .floats
                .iter()
                .find(|value| value.property_name == "_Speed")
                .expect("_Speed");
            assert_eq!(speed.value.to_bits(), 0.0f32.to_bits());
        }
        let left = setup.materials[0].binding.as_ref().expect("left");
        let right = setup.materials[1].binding.as_ref().expect("right");
        assert!(
            left.floats
                .iter()
                .any(|value| value.property_name == "TILE")
        );
        assert!(
            right
                .floats
                .iter()
                .any(|value| value.property_name == "GYROID")
        );
        assert_eq!(
            left.float4s
                .iter()
                .find(|value| value.property_name == "_Schlafli")
                .expect("_Schlafli")
                .value
                .map(f32::to_bits),
            SCHLAFLI.map(f32::to_bits)
        );
    }
}
