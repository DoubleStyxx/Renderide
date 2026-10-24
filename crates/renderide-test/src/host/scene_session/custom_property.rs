//! Two-capture custom-shader session: install `example_rainbow.wgsl`, assign it to one material,
//! render with `RAINBOW` off, then update `_Color` and `RAINBOW` and render again.
//!
//! `_Speed` stays 0 so the hue cycle does not depend on frame time. The stock shader on the
//! material stays `Unlit.shader`, which samples `_Tex` rather than `_MainTex`, so a missed
//! assign cannot reproduce the custom golden.

use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant, SystemTime};

use glam::{Quat, Vec3};
use renderide_shared::shared::{ColorProfile, ShadowCastMode};
use renderide_shared::{format_custom_shader_assign, format_custom_shader_payload};

use crate::error::HarnessError;
use crate::scene::primitives::{centered_bounds, generate_quad};
use crate::scene::textures::checker_rgba;

use super::super::asset_upload::{
    BoundMaterial, DEFAULT_ASSET_UPLOAD_TIMEOUT, MaterialBatchRequest, MaterialUpdateOp,
    PropertyIdLookup, apply_material_batch, request_property_ids, upload_shader_file,
};
use super::super::lockstep::LockstepDriver;
use super::consts::shader_variants;
use super::png_readback::{PngStabilityWaitTiming, run_lockstep_until_png_stable};
use super::{
    Float4Binding, FloatBinding, MaterialAsset, MaterialBinding, SceneRenderable, SceneSetup,
    SceneSubmission, TextureBinding, mesh_asset, scene_material_id,
    scene_material_update_buffer_id, scene_property_request_id, scene_shader_id,
    scene_texture_buffer_id, scene_texture_id, scene_update_batch_id, transform,
};

/// Shader key sent on the install and assign payloads. The renderer normalizes it.
const SHADER_KEY: &str = "ExampleRainbow";
/// Positive `ShaderUpload.assetId` for the install so the harness can wait for the ack.
const INSTALL_ASSET_ID: i32 = 521;
/// Positive carrier `ShaderUpload.assetId` for the assign. The material id is the header.
const ASSIGN_ASSET_ID: i32 = 522;
/// Material-update slot for the second batch. Slot 0 owns buffers 220..=223.
const PROPERTY_UPDATE_SLOT: i32 = 1;
/// Quad scale at the default 60 degree camera so the surface covers most of the frame.
const QUAD_SCALE: f32 = 2.6;
const PLAIN_COLOR: [f32; 4] = [1.0, 0.25, 0.2, 1.0];
const UPDATED_COLOR: [f32; 4] = [0.15, 0.45, 1.0, 1.0];
const MAIN_TEX_ST: [f32; 4] = [2.0, 2.0, 0.25, 0.0];
/// Second-wait interval. The stability floor is `interval * 2`, which leaves time for the
/// `RAINBOW` variant to compile before a PNG is accepted.
const RECAPTURE_INTERVAL: Duration = Duration::from_secs(3);

const RAINBOW_WGSL: &str = include_str!("../../../../../RenderideMod/Shaders/example_rainbow.wgsl");

/// Scene uploaded before the custom shader is assigned.
pub(super) fn scene_setup() -> SceneSetup {
    let checker = checker_rgba(128, 128, 8, [245, 245, 245, 255], [24, 34, 54, 255]);
    let texture_size = (checker.width(), checker.height());
    SceneSetup {
        meshes: vec![mesh_asset(
            0,
            generate_quad(),
            centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
        )],
        materials: vec![MaterialAsset {
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
                    rgba_bytes: checker.into_raw(),
                    size: texture_size,
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
                    value: PLAIN_COLOR,
                }],
            }),
        }],
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.0, -3.0), Vec3::ONE, Quat::IDENTITY),
            renderables: vec![SceneRenderable {
                transform_id: 0,
                pose: transform(Vec3::ZERO, Vec3::splat(QUAD_SCALE), Quat::IDENTITY),
                mesh_asset_id: super::scene_mesh_id(0),
                material_asset_id: scene_material_id(0),
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::Off,
            }],
            lights: Vec::new(),
            lod_group: None,
        },
    }
}

/// Installs the rainbow program and assigns it to the scene material.
pub(super) fn install_and_assign(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    setup: &SceneSetup,
) -> Result<(), HarnessError> {
    let material_asset_id = setup.materials[0].asset_id;
    let plain = install_file();
    upload_shader_file(
        queues,
        lockstep,
        INSTALL_ASSET_ID,
        &plain,
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    let assign = format_custom_shader_assign(material_asset_id, SHADER_KEY);
    upload_shader_file(
        queues,
        lockstep,
        ASSIGN_ASSET_ID,
        &assign,
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    Ok(())
}

/// Inputs for the second capture after the property batch.
pub(super) struct PropertyRecapture<'a> {
    /// Host command queues.
    pub queues: &'a mut renderide_shared::ipc::HostDualQueueIpc,
    /// Lockstep driver kept pumping while acknowledgements are pending.
    pub lockstep: &'a mut LockstepDriver,
    /// Shared-memory prefix for the material-update buffers.
    pub prefix: &'a str,
    /// Per-session backing directory for those buffers.
    pub backing_dir: &'a Path,
    /// Scene whose first material receives the property batch.
    pub setup: &'a SceneSetup,
    /// Renderer PNG path. The pre-update copy is `plain.png` beside it.
    pub output_path: &'a Path,
    /// Deadline for the second stable PNG.
    pub timeout: Duration,
    /// Live renderer process.
    pub renderer: &'a mut Child,
}

/// Copies the pre-update PNG, applies the property batch, and waits for a newer PNG.
pub(super) fn recapture_after_property_update(
    recapture: PropertyRecapture<'_>,
) -> Result<BoundMaterial, HarnessError> {
    let plain_path = recapture.output_path.with_file_name("plain.png");
    std::fs::copy(recapture.output_path, &plain_path)?;
    logger::info!("Session: copied pre-update PNG to {}", plain_path.display());

    let material_asset_id = recapture.setup.materials[0].asset_id;
    let names = ["_Color", "_Speed", "RAINBOW"];
    let property_ids = request_property_ids(
        recapture.queues,
        recapture.lockstep,
        PropertyIdLookup {
            request_id: scene_property_request_id(PROPERTY_UPDATE_SLOT),
            names: &names,
            timeout: DEFAULT_ASSET_UPLOAD_TIMEOUT,
        },
    )?;
    if property_ids.len() != names.len() {
        return Err(HarnessError::QueueOptions(format!(
            "expected {} property ids, got {}",
            names.len(),
            property_ids.len()
        )));
    }

    let ops = [
        MaterialUpdateOp::SelectTarget { material_asset_id },
        MaterialUpdateOp::SetFloat4 {
            property_id: property_ids[0],
            value: UPDATED_COLOR,
        },
        MaterialUpdateOp::SetFloat {
            property_id: property_ids[1],
            value: 0.0,
        },
        MaterialUpdateOp::SetFloat {
            property_id: property_ids[2],
            value: 1.0,
        },
        MaterialUpdateOp::UpdateBatchEnd,
    ];
    let bound = apply_material_batch(
        recapture.queues,
        recapture.lockstep,
        MaterialBatchRequest {
            shared_memory_prefix: recapture.prefix,
            backing_dir: recapture.backing_dir,
            base_buffer_id: scene_material_update_buffer_id(PROPERTY_UPDATE_SLOT),
            update_batch_id: scene_update_batch_id(PROPERTY_UPDATE_SLOT),
            material_update_count: 1,
            ops: &ops,
            timeout: DEFAULT_ASSET_UPLOAD_TIMEOUT,
        },
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
            interval: RECAPTURE_INTERVAL,
            non_flat_skips: 0,
        },
        recapture.renderer,
    )?;
    Ok(bound)
}

fn install_file() -> String {
    format_custom_shader_payload(SHADER_KEY, RAINBOW_WGSL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_payload_is_the_rainbow_example() {
        let file = install_file();
        assert!(file.starts_with("renderide-custom-wgsl:v1\nkey=ExampleRainbow\n---\n"));
        assert!(file.contains("//#keyword RAINBOW"));
        assert!(file.contains("_MainTex_ST"));
        assert!(file.contains("_Speed"));
    }

    #[test]
    fn assign_payload_names_the_scene_material() {
        assert_eq!(
            format_custom_shader_assign(scene_material_id(0), SHADER_KEY),
            "renderide-custom-wgsl:v1\nop=assign\nmaterial=120\nkey=ExampleRainbow\n---\n"
        );
    }

    #[test]
    fn scene_keeps_unlit_and_binds_main_tex() {
        let setup = scene_setup();
        let binding = setup.materials[0].binding.as_ref().expect("binding");
        assert_eq!(binding.shader_name, "Unlit.shader");
        assert_eq!(
            binding.shader_variant_bits,
            Some(shader_variants::UNLIT_COLOR)
        );
        assert_eq!(binding.textures[0].property_name, "_MainTex");
        assert_eq!(
            binding.textures[0].st_value.map(f32::to_bits),
            MAIN_TEX_ST.map(f32::to_bits)
        );
        assert_eq!(
            binding
                .floats
                .iter()
                .map(|value| (value.property_name, value.value.to_bits()))
                .collect::<Vec<_>>(),
            vec![("_Speed", 0.0f32.to_bits()), ("RAINBOW", 0.0f32.to_bits())]
        );
        assert_eq!(
            binding.float4s[0].value.map(f32::to_bits),
            PLAIN_COLOR.map(f32::to_bits)
        );
        assert_eq!(setup.materials[0].update_base_buffer_id, 220);
        assert_eq!(scene_material_update_buffer_id(PROPERTY_UPDATE_SLOT), 224);
    }
}
