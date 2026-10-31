//! One sphere, one directional or point light, and a gradient backdrop for #216.
//!
//! The sphere material is the debug shader in `warp_216_term_debug.wgsl`. The backdrop
//! is unlit and textured so a zero lighting term still leaves a non-flat, non-black frame.

use std::path::Path;

use glam::{Quat, Vec3};
use renderide_shared::shared::{ColorProfile, ShadowCastMode};
use renderide_shared::{format_custom_shader_assign, format_custom_shader_payload};

use crate::scene::primitives::{centered_bounds, generate_quad};
use crate::scene::sphere::generate_sphere;
use crate::scene_dsl::warp_216_debug::{LightRig, LightingTerm, OutputChannel, Spec};

use super::super::asset_upload::{
    DEFAULT_ASSET_UPLOAD_TIMEOUT, MaterialBatchRequest, MaterialUpdateOp, apply_material_batch,
    upload_shader_file,
};
use super::super::lockstep::LockstepDriver;
use super::negative_light;
use super::scene_ids::{
    scene_material_id, scene_material_update_buffer_id, scene_mesh_id, scene_property_request_id,
    scene_shader_id, scene_texture_buffer_id, scene_texture_id, scene_update_batch_id,
};
use super::scene_setup::{
    Float4Binding, FloatBinding, MaterialAsset, MaterialBinding, SceneSetup, TextureBinding,
    mesh_asset, transform,
};
use super::scene_state::{
    SceneLight, SceneRenderable, SceneSubmission, directional_light, point_light,
};

/// Shader key on the install and assign payloads.
const SHADER_KEY: &str = "Warp216TermDebug";
/// Positive install id so the harness can wait for the ack.
const INSTALL_ASSET_ID: i32 = 621;
/// Positive assign carrier. The material id is in the payload header.
const ASSIGN_ASSET_ID: i32 = 622;

const SPHERE_COLOR: [f32; 4] = [0.72, 0.38, 0.22, 1.0];
const SPHERE_METALLIC: f32 = 0.0;
const SPHERE_GLOSSINESS: f32 = 0.62;
/// Directional colour. Channels differ so a dropped channel is visible in the composed frame.
const DIRECTIONAL_COLOR: [f32; 3] = [1.0, 0.82, 0.45];
const DIRECTIONAL_INTENSITY: f32 = 1.6;
/// Point colour. Blue-heavy, distinct from the directional rig.
const POINT_COLOR: [f32; 3] = [0.35, 0.72, 1.0];
const POINT_INTENSITY: f32 = 8.0;
const POINT_RANGE: f32 = 8.0;

const DEBUG_WGSL: &str = include_str!("warp_216_term_debug.wgsl");

/// Sphere, gradient backdrop, and the rig selected by `spec`.
pub(super) fn scene_setup(spec: Spec) -> SceneSetup {
    let mut setup = match spec.rig {
        LightRig::NegativePair | LightRig::NegativeParity => {
            negative_light::scene_setup(true, false)
        }
        LightRig::PositiveOnly => negative_light::scene_setup(false, false),
        LightRig::Directional => lit_setup(vec![directional_scene_light()]),
        LightRig::Point => lit_setup(vec![point_scene_light()]),
        LightRig::None => lit_setup(Vec::new()),
    };
    setup.materials[0] = debug_material(spec);
    setup.meshes.push(mesh_asset(
        1,
        generate_quad(),
        centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
    ));
    setup.materials.push(backdrop_material());
    setup.scene.renderables.push(backdrop_renderable());
    setup.scene.camera_world_pose =
        transform(Vec3::new(0.0, 0.15, -3.1), Vec3::ONE, Quat::IDENTITY);
    setup
}

/// Installs the debug program and assigns it to the sphere material.
pub(super) fn install_and_assign(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    prefix: &str,
    backing_dir: &Path,
    setup: &SceneSetup,
) -> Result<(), crate::error::HarnessError> {
    let material_asset_id = setup.materials[0].asset_id;
    upload_shader_file(
        queues,
        lockstep,
        INSTALL_ASSET_ID,
        &format_custom_shader_payload(SHADER_KEY, DEBUG_WGSL),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    upload_shader_file(
        queues,
        lockstep,
        ASSIGN_ASSET_ID,
        &format_custom_shader_assign(material_asset_id, SHADER_KEY),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    // The install registers the custom stem on this shader asset. Point the sphere
    // material at it so the draw does not stay on the placeholder Unlit route.
    let ops = [
        MaterialUpdateOp::SelectTarget { material_asset_id },
        MaterialUpdateOp::SetShader {
            shader_asset_id: INSTALL_ASSET_ID,
        },
        MaterialUpdateOp::UpdateBatchEnd,
    ];
    apply_material_batch(
        queues,
        lockstep,
        MaterialBatchRequest {
            shared_memory_prefix: prefix,
            backing_dir,
            base_buffer_id: scene_material_update_buffer_id(2),
            update_batch_id: scene_update_batch_id(2),
            material_update_count: 1,
            ops: &ops,
            timeout: DEFAULT_ASSET_UPLOAD_TIMEOUT,
        },
    )?;
    Ok(())
}

fn lit_setup(lights: Vec<SceneLight>) -> SceneSetup {
    SceneSetup {
        meshes: vec![mesh_asset(
            0,
            generate_sphere(18, 28),
            centered_bounds(Vec3::splat(1.05)),
        )],
        materials: vec![debug_material(Spec {
            rig: LightRig::Directional,
            term: LightingTerm::Composed,
            channel: OutputChannel::Raw,
        })],
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.15, -3.1), Vec3::ONE, Quat::IDENTITY),
            renderables: vec![sphere_renderable()],
            lights,
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

fn sphere_renderable() -> SceneRenderable {
    SceneRenderable {
        transform_id: 0,
        pose: transform(Vec3::ZERO, Vec3::splat(0.78), Quat::IDENTITY),
        mesh_asset_id: scene_mesh_id(0),
        material_asset_id: scene_material_id(0),
        sorting_order: 1,
        shadow_cast_mode: ShadowCastMode::Off,
    }
}

fn backdrop_renderable() -> SceneRenderable {
    SceneRenderable {
        transform_id: 1,
        pose: transform(
            Vec3::new(0.0, 0.0, 1.6),
            Vec3::new(8.0, 8.0, 1.0),
            Quat::IDENTITY,
        ),
        mesh_asset_id: scene_mesh_id(1),
        material_asset_id: scene_material_id(1),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::Off,
    }
}

fn directional_scene_light() -> SceneLight {
    SceneLight {
        transform_id: 10,
        pose: transform(
            Vec3::new(-1.2, 2.0, -1.4),
            Vec3::ONE,
            Quat::from_rotation_x(-0.65),
        ),
        state: directional_light(DIRECTIONAL_COLOR, DIRECTIONAL_INTENSITY),
    }
}

fn point_scene_light() -> SceneLight {
    SceneLight {
        transform_id: 10,
        pose: transform(Vec3::new(0.85, 1.15, -1.35), Vec3::ONE, Quat::IDENTITY),
        state: point_light(POINT_COLOR, POINT_INTENSITY, POINT_RANGE),
    }
}

fn debug_material(spec: Spec) -> MaterialAsset {
    MaterialAsset {
        asset_id: scene_material_id(0),
        update_batch_id: scene_update_batch_id(0),
        property_request_id: scene_property_request_id(0),
        update_base_buffer_id: scene_material_update_buffer_id(0),
        binding: Some(MaterialBinding {
            shader_asset_id: scene_shader_id(0),
            shader_name: "Unlit.shader",
            shader_variant_bits: None,
            textures: Vec::new(),
            floats: vec![
                FloatBinding {
                    property_name: "_Metallic",
                    value: SPHERE_METALLIC,
                },
                FloatBinding {
                    property_name: "_Glossiness",
                    value: SPHERE_GLOSSINESS,
                },
                FloatBinding {
                    property_name: "_TermId",
                    value: spec.term_id(),
                },
                FloatBinding {
                    property_name: "_Channel",
                    value: spec.channel_id(),
                },
                FloatBinding {
                    property_name: "_TERM_DEBUG",
                    value: 1.0,
                },
            ],
            float4s: vec![Float4Binding {
                property_name: "_Color",
                value: SPHERE_COLOR,
            }],
        }),
    }
}

fn backdrop_material() -> MaterialAsset {
    let (rgba, size) = backdrop_rgba();
    MaterialAsset {
        asset_id: scene_material_id(1),
        update_batch_id: scene_update_batch_id(1),
        property_request_id: scene_property_request_id(1),
        update_base_buffer_id: scene_material_update_buffer_id(1),
        binding: Some(MaterialBinding {
            shader_asset_id: scene_shader_id(1),
            shader_name: "Unlit.shader",
            shader_variant_bits: Some(
                super::consts::shader_variants::UNLIT_TEXTURE
                    | super::consts::shader_variants::UNLIT_COLOR,
            ),
            textures: vec![TextureBinding {
                asset_id: scene_texture_id(1),
                buffer_id: scene_texture_buffer_id(1),
                property_name: "_Tex",
                st_property_name: "_Tex_ST",
                st_value: [1.0, 1.0, 0.0, 0.0],
                rgba_bytes: rgba,
                size,
                color_profile: ColorProfile::SRGB,
                packed_handle: None,
            }],
            floats: Vec::new(),
            float4s: vec![Float4Binding {
                property_name: "_Color",
                value: [1.0, 1.0, 1.0, 1.0],
            }],
        }),
    }
}

/// 32×32 gradient. R follows x, G follows y, B is a 4-pixel checker.
///
/// No random seed: the formula is the pattern. Values stay off black and off magenta.
pub(super) fn backdrop_rgba() -> (Vec<u8>, (u32, u32)) {
    const WIDTH: u32 = 32;
    const HEIGHT: u32 = 32;
    let mut bytes = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let red = 28 + x * 6;
            let green = 48 + y * 5;
            let blue = if (x / 4 + y / 4) % 2 == 0 { 72 } else { 168 };
            bytes.extend_from_slice(&[red as u8, green as u8, blue, 255]);
        }
    }
    (bytes, (WIDTH, HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::{backdrop_rgba, scene_setup};
    use crate::scene_dsl::warp_216_debug::{LightRig, LightingTerm, OutputChannel, Spec};

    fn float_named(setup: &super::SceneSetup, name: &str) -> f32 {
        setup.materials[0]
            .binding
            .as_ref()
            .expect("binding")
            .floats
            .iter()
            .find(|float| float.property_name == name)
            .map(|float| float.value)
            .unwrap_or_else(|| panic!("missing {name}"))
    }

    #[test]
    fn directional_scene_is_one_sphere_one_light_and_a_backdrop() {
        let setup = scene_setup(Spec {
            rig: LightRig::Directional,
            term: LightingTerm::Diffuse,
            channel: OutputChannel::G,
        });
        assert_eq!(setup.scene.renderables.len(), 2);
        assert_eq!(setup.scene.lights.len(), 1);
        assert!(setup.scene.lights[0].state.intensity > 0.0);
        assert_eq!(float_named(&setup, "_TermId"), LightingTerm::Diffuse.id());
        assert_eq!(float_named(&setup, "_Channel"), OutputChannel::G.id());
        assert_eq!(float_named(&setup, "_TERM_DEBUG"), 1.0);
        assert_eq!(
            setup.scene.camera_world_pose.position,
            glam::Vec3::new(0.0, 0.15, -3.1)
        );
    }

    #[test]
    fn point_rig_is_one_point_light() {
        let setup = scene_setup(Spec {
            rig: LightRig::Point,
            term: LightingTerm::Composed,
            channel: OutputChannel::Raw,
        });
        assert_eq!(setup.scene.lights.len(), 1);
        assert!(setup.scene.lights[0].state.range > 1.0);
    }

    #[test]
    fn negative_pair_keeps_the_negative_intensity_and_positive_only_does_not() {
        let negative = scene_setup(Spec {
            rig: LightRig::NegativePair,
            term: LightingTerm::Composed,
            channel: OutputChannel::Raw,
        });
        assert_eq!(negative.scene.lights.len(), 2);
        assert_eq!(negative.scene.lights[1].state.intensity, -1.45);
        let parity = scene_setup(Spec {
            rig: LightRig::NegativeParity,
            term: LightingTerm::Composed,
            channel: OutputChannel::Raw,
        });
        assert_eq!(parity.scene.lights.len(), 2);
        assert_eq!(parity.scene.lights[1].state.intensity, -1.45);
        let positive = scene_setup(Spec {
            rig: LightRig::PositiveOnly,
            term: LightingTerm::Composed,
            channel: OutputChannel::Raw,
        });
        assert_eq!(positive.scene.lights.len(), 1);
        assert!(positive.scene.lights[0].state.intensity > 0.0);
    }

    #[test]
    fn constant_scene_has_no_lights() {
        let setup = scene_setup(Spec {
            rig: LightRig::None,
            term: LightingTerm::Constant,
            channel: OutputChannel::Raw,
        });
        assert!(setup.scene.lights.is_empty());
        assert_eq!(float_named(&setup, "_TermId"), LightingTerm::Constant.id());
    }

    #[test]
    fn backdrop_is_not_flat_black_or_magenta() {
        let (bytes, (width, height)) = backdrop_rgba();
        assert_eq!((width, height), (32, 32));
        let mut min = [255u8; 3];
        let mut max = [0u8; 3];
        for pixel in bytes.as_chunks::<4>().0 {
            for channel in 0..3 {
                min[channel] = min[channel].min(pixel[channel]);
                max[channel] = max[channel].max(pixel[channel]);
            }
            assert_ne!(&pixel[..3], &[255, 0, 255]);
            assert_ne!(&pixel[..3], &[0, 0, 0]);
        }
        for channel in 0..3 {
            assert!(max[channel] - min[channel] > 1, "channel {channel} is flat");
        }
    }
}
