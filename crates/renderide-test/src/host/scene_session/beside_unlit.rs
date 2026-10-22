//! Rainbow custom shader on the left quad and packaged Unlit on the right.
//!
//! `RAINBOW` stays off so the left quad is `_MainTex * _Color` without waiting on a keyword
//! variant. The left material's host shader is Unlit with only the color bit, and its texture
//! is `_MainTex`, which packaged Unlit does not sample. A missed assign draws a flat color.
//! The second capture clears the left material and that quad becomes the flat Unlit color.
//! The right quad is textured Unlit on `_Tex` for the whole session.

use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant, SystemTime};

use glam::{Quat, Vec3};
use renderide_shared::shared::ColorProfile;
use renderide_shared::{
    format_custom_shader_assign, format_custom_shader_clear, format_custom_shader_payload,
};

use crate::error::HarnessError;
use crate::scene::primitives::{centered_bounds, generate_quad};
use crate::scene::textures::checker_rgba;

use super::super::asset_upload::{DEFAULT_ASSET_UPLOAD_TIMEOUT, upload_shader_file};
use super::super::lockstep::LockstepDriver;
use super::consts::shader_variants;
use super::png_readback::{PngStabilityWaitTiming, run_lockstep_until_png_stable};
use super::{
    Float4Binding, FloatBinding, MaterialAsset, MaterialBinding, SceneRenderable, SceneSetup,
    SceneSubmission, TextureBinding, mesh_asset, scene_material_id,
    scene_material_update_buffer_id, scene_property_request_id, scene_shader_id,
    scene_texture_buffer_id, scene_texture_id, scene_update_batch_id, transform,
};

const SHADER_KEY: &str = "Rainbow";
const INSTALL_ASSET_ID: i32 = 561;
const ASSIGN_ASSET_ID: i32 = 562;
const CLEAR_ASSET_ID: i32 = 563;
const QUAD_SCALE: f32 = 1.5;
const LEFT_COLOR: [f32; 4] = [1.0, 0.35, 0.2, 1.0];
const RIGHT_COLOR: [f32; 4] = [0.25, 0.55, 1.0, 1.0];
const MAIN_TEX_ST: [f32; 4] = [2.0, 2.0, 0.0, 0.0];
/// Clear swaps the left quad back to packaged Unlit. Two renderer intervals cover that.
const CLEAR_INTERVAL: Duration = Duration::from_secs(2);

const RAINBOW_WGSL: &str = include_str!("../../../../../RenderideMod/Shaders/example_rainbow.wgsl");

pub(super) fn scene_setup() -> SceneSetup {
    let left_checker = checker_rgba(128, 128, 8, [250, 236, 210, 255], [120, 36, 28, 255]);
    let right_checker = checker_rgba(128, 128, 16, [230, 240, 250, 255], [18, 42, 96, 255]);
    let left_size = (left_checker.width(), left_checker.height());
    let right_size = (right_checker.width(), right_checker.height());
    SceneSetup {
        meshes: vec![mesh_asset(
            0,
            generate_quad(),
            centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
        )],
        materials: vec![
            left_material(left_checker.into_raw(), left_size),
            right_material(right_checker.into_raw(), right_size),
        ],
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.0, -3.4), Vec3::ONE, Quat::IDENTITY),
            renderables: vec![
                quad_renderable(0, Vec3::new(-0.82, 0.0, 0.0)),
                quad_renderable(1, Vec3::new(0.82, 0.0, 0.0)),
            ],
            lights: Vec::new(),
        },
    }
}

fn quad_renderable(index: i32, translation: Vec3) -> SceneRenderable {
    SceneRenderable {
        transform_id: index,
        pose: transform(translation, Vec3::splat(QUAD_SCALE), Quat::IDENTITY),
        mesh_asset_id: super::scene_mesh_id(0),
        material_asset_id: scene_material_id(index),
        sorting_order: 0,
    }
}

fn left_material(rgba: Vec<u8>, size: (u32, u32)) -> MaterialAsset {
    MaterialAsset {
        asset_id: scene_material_id(0),
        update_batch_id: scene_update_batch_id(0),
        property_request_id: scene_property_request_id(0),
        update_base_buffer_id: scene_material_update_buffer_id(0),
        binding: Some(MaterialBinding {
            shader_asset_id: scene_shader_id(0),
            shader_name: "Unlit.shader",
            shader_variant_bits: Some(shader_variants::UNLIT_COLOR),
            textures: vec![TextureBinding {
                asset_id: scene_texture_id(0),
                buffer_id: scene_texture_buffer_id(0),
                property_name: "_MainTex",
                st_property_name: "_MainTex_ST",
                st_value: MAIN_TEX_ST,
                rgba_bytes: rgba,
                size,
                color_profile: ColorProfile::SRGB,
            }],
            floats: vec![
                FloatBinding {
                    property_name: "_Speed",
                    value: 0.0,
                },
                FloatBinding {
                    property_name: "RAINBOW",
                    value: 0.0,
                },
            ],
            float4s: vec![Float4Binding {
                property_name: "_Color",
                value: LEFT_COLOR,
            }],
        }),
    }
}

fn right_material(rgba: Vec<u8>, size: (u32, u32)) -> MaterialAsset {
    MaterialAsset {
        asset_id: scene_material_id(1),
        update_batch_id: scene_update_batch_id(1),
        property_request_id: scene_property_request_id(1),
        update_base_buffer_id: scene_material_update_buffer_id(1),
        binding: Some(MaterialBinding {
            shader_asset_id: scene_shader_id(1),
            shader_name: "Unlit.shader",
            shader_variant_bits: Some(
                shader_variants::UNLIT_TEXTURE | shader_variants::UNLIT_COLOR,
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
            }],
            floats: Vec::new(),
            float4s: vec![Float4Binding {
                property_name: "_Color",
                value: RIGHT_COLOR,
            }],
        }),
    }
}

/// Installs rainbow and assigns it to the left material only.
pub(super) fn install_and_assign(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    setup: &SceneSetup,
) -> Result<(), HarnessError> {
    let left = setup.materials[0].asset_id;
    upload_shader_file(
        queues,
        lockstep,
        INSTALL_ASSET_ID,
        &format_custom_shader_payload(SHADER_KEY, RAINBOW_WGSL),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    upload_shader_file(
        queues,
        lockstep,
        ASSIGN_ASSET_ID,
        &format_custom_shader_assign(left, SHADER_KEY),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    Ok(())
}

/// Inputs for the capture after `ClearMaterial`.
pub(super) struct ClearRecapture<'a> {
    /// Host command queues.
    pub queues: &'a mut renderide_shared::ipc::HostDualQueueIpc,
    /// Lockstep driver kept pumping while the clear is acknowledged.
    pub lockstep: &'a mut LockstepDriver,
    /// Scene whose first material is cleared.
    pub setup: &'a SceneSetup,
    /// Renderer PNG path. The pre-clear copy is `plain.png` beside it.
    pub output_path: &'a Path,
    /// Deadline for the second stable PNG.
    pub timeout: Duration,
    /// Live renderer process.
    pub renderer: &'a mut Child,
}

/// Copies the assigned frame, clears the left material, and waits for a newer PNG.
pub(super) fn recapture_after_clear(recapture: ClearRecapture<'_>) -> Result<(), HarnessError> {
    let plain_path = recapture.output_path.with_file_name("plain.png");
    std::fs::copy(recapture.output_path, &plain_path)?;
    logger::info!("Session: copied pre-clear PNG to {}", plain_path.display());

    let left = recapture.setup.materials[0].asset_id;
    upload_shader_file(
        recapture.queues,
        recapture.lockstep,
        CLEAR_ASSET_ID,
        &format_custom_shader_clear(left),
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;

    let scene_submitted_at = SystemTime::now();
    let scene_submit_instant = Instant::now();
    run_lockstep_until_png_stable(
        recapture.queues,
        recapture.lockstep,
        recapture.output_path,
        PngStabilityWaitTiming {
            scene_submitted_at,
            scene_submit_instant,
            overall_timeout: recapture.timeout,
            interval: CLEAR_INTERVAL,
        },
        recapture.renderer,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_material_is_custom_and_right_stays_packaged_unlit() {
        let setup = scene_setup();
        let left = setup.materials[0].binding.as_ref().expect("left");
        let right = setup.materials[1].binding.as_ref().expect("right");
        assert_eq!(left.shader_name, "Unlit.shader");
        assert_eq!(right.shader_name, "Unlit.shader");
        assert_ne!(left.shader_asset_id, right.shader_asset_id);
        assert_eq!(left.textures[0].property_name, "_MainTex");
        assert_eq!(right.textures[0].property_name, "_Tex");
        assert_eq!(left.shader_variant_bits, Some(shader_variants::UNLIT_COLOR));
        assert!(
            left.floats
                .iter()
                .any(|value| value.property_name == "RAINBOW" && value.value == 0.0)
        );
        assert_eq!(
            format_custom_shader_assign(setup.materials[0].asset_id, SHADER_KEY),
            "renderide-custom-wgsl:v1\nop=assign\nmaterial=120\nkey=Rainbow\n---\n"
        );
        let clear = format_custom_shader_clear(setup.materials[0].asset_id);
        assert_eq!(
            clear,
            "renderide-custom-wgsl:v1\nop=clear\nmaterial=120\n---\n"
        );
        assert_ne!(setup.materials[0].asset_id, setup.materials[1].asset_id);
    }
}
