//! End-to-end orchestration of the harness lifecycle: open IPC -> spawn renderer -> handshake ->
//! upload assets -> submit scene state -> wait for the renderer to write a fresh PNG -> shutdown.
//!
//! The implementation is split across focused submodules:
//!
//! - `config` -- public configuration and outcome types.
//! - `consts` -- centralized timing, asset-id, and tessellation constants.
//! - [`spawn`] -- renderer process spawn + RAII guard.
//! - `scene_state` -- scene-state SHM construction and first-submit pump.
//! - `scene_setup` -- per-template meshes, materials, and upload.
//! - [`png_readback`] -- PNG stability state machine + readback driver loop.
//! - `capture` -- first PNG wait plus property-update and shadow-move recaptures.
//! - `canvas_ui` -- coplanar and tilted UI quad scenes.
//! - `shutdown` -- graceful shutdown sequence.

use crate::error::HarnessError;
use crate::scene::mesh::Mesh;

use super::handshake::{DEFAULT_HANDSHAKE_TIMEOUT, run_handshake};
use super::ipc_setup::{DEFAULT_QUEUE_CAPACITY_BYTES, connect_session};
use super::lockstep::{FrameSubmitScalars, LockstepDriver};

mod bench_drive;
mod bench_scenes;
mod canvas_ui;
mod capture;
mod config;
mod consts;
mod custom_property;
mod double_buffer_stack;
mod exotic_pair;
mod gpu_ready;
mod negative_light;
mod scene_setup;
mod shadowed_caster;
mod warp_216_debug;

pub use bench_drive::{BenchDriveConfig, drive_bench};
pub mod png_readback;
mod scene_ids;
mod scene_state;
mod shutdown;
pub mod spawn;

pub use config::SceneSessionConfig;

use config::SceneSessionOutcome;

/// How a session replaces uploaded shaders before the golden frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CustomShaderAssignment {
    /// Every uploaded material is drawn as uploaded.
    None,
    /// `custom_property::install_and_assign` replaces the only material.
    PropertyUpdate,
    /// `exotic_pair::install_and_assign` replaces both materials.
    ExoticPair,
    /// `warp_216_debug::install_and_assign` replaces material 0. The backdrop stays.
    Warp216,
}

/// One shader still bound at capture, including the host variant bits.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DrawnShader {
    /// Unity shader asset filename, such as `PBSMetallic.shader`.
    pub shader_asset: &'static str,
    /// Host `_RenderideVariantBits`, when the case sets them.
    pub variant_bits: Option<u32>,
}

/// Shaders that remain on screen for `template` after custom-shader assignment.
///
/// Derived from [`build_scene_setup`] and [`custom_shader_assignment`], the same
/// assignment `run_session` performs before capture. A material with no shader
/// binding is the null fallback (`shaders/materials/null.wgsl`).
pub(crate) struct MaterialDraws {
    /// Shaders still bound at capture.
    pub shaders: Vec<DrawnShader>,
    /// Renderables whose material uploaded no shader.
    pub null_fallback: bool,
}

/// Shader assets drawn by `template`, after the session's custom-shader assignment.
pub(crate) fn material_draws(template: SessionTemplate) -> MaterialDraws {
    let assignment = custom_shader_assignment(&template);
    let setup = build_scene_setup(template);
    let mut shaders = Vec::new();
    let mut null_fallback = false;
    for (index, material) in setup.materials.iter().enumerate() {
        if assignment_skips_material(assignment, index) {
            continue;
        }
        let drawn = setup
            .scene
            .renderables
            .iter()
            .any(|renderable| renderable.material_asset_id == material.asset_id);
        if !drawn {
            continue;
        }
        match material.binding.as_ref() {
            Some(binding) => shaders.push(DrawnShader {
                shader_asset: binding.shader_name,
                variant_bits: binding.shader_variant_bits,
            }),
            None => null_fallback = true,
        }
    }
    shaders.sort_unstable();
    shaders.dedup();
    MaterialDraws {
        shaders,
        null_fallback,
    }
}

fn custom_shader_assignment(template: &SessionTemplate) -> CustomShaderAssignment {
    match template {
        SessionTemplate::CustomShaderPropertyUpdate => CustomShaderAssignment::PropertyUpdate,
        SessionTemplate::ExoticShaderPair => CustomShaderAssignment::ExoticPair,
        SessionTemplate::Warp216Debug { .. } => CustomShaderAssignment::Warp216,
        SessionTemplate::Sphere
        | SessionTemplate::Torus { .. }
        | SessionTemplate::MultiPrimitiveUnlitGrid { .. }
        | SessionTemplate::PbsLitMaterialMatrix
        | SessionTemplate::PbsMetallicPremultiply
        | SessionTemplate::ManyStaticMeshes
        | SessionTemplate::LightsShadows
        | SessionTemplate::AlphaCutoutMaskedQuads { .. }
        | SessionTemplate::GltfTexturedStaticMesh { .. }
        | SessionTemplate::CoplanarCanvasUiQuads
        | SessionTemplate::TiltedCanvasUiQuads
        | SessionTemplate::CanvasOffsetBiasQuad
        | SessionTemplate::TiltedCanvasDistanceBias
        | SessionTemplate::ShadowedCasterMove
        | SessionTemplate::ManyShadowedPointLights
        | SessionTemplate::DoubleBufferNothingOnSkybox
        | SessionTemplate::NegativeLightPair { .. }
        | SessionTemplate::SkinnedAvatars { .. } => CustomShaderAssignment::None,
    }
}

fn assignment_skips_material(assignment: CustomShaderAssignment, index: usize) -> bool {
    match assignment {
        CustomShaderAssignment::None => false,
        CustomShaderAssignment::PropertyUpdate | CustomShaderAssignment::ExoticPair => true,
        CustomShaderAssignment::Warp216 => index == 0,
    }
}
use scene_ids::{
    scene_material_id, scene_material_update_buffer_id, scene_mesh_id, scene_property_request_id,
    scene_shader_id, scene_texture_buffer_id, scene_texture_id, scene_update_batch_id,
};
use scene_setup::{
    Float4Binding, FloatBinding, MaterialAsset, MaterialBinding, MeshAsset, SceneSetup,
    TextureBinding, build_scene_setup, mesh_asset, pbs_material_for_scene, transform,
    unlit_color_material_for_scene, upload_scene_materials, upload_scene_meshes,
};
use scene_state::{
    SceneRenderable, SceneSubmission, default_camera_world_pose, directional_light, point_light,
};
use shutdown::request_shutdown_and_wait;
use spawn::spawn_renderer;

/// Selects which test scene the session uploads and renders.
#[derive(Clone, Debug)]
pub enum SessionTemplate {
    /// Original UV sphere baseline (no shader uploaded; renders on Null/checkerboard).
    Sphere,
    /// Procedural torus rendered through the embedded `unlit_default` shader with a texture.
    Torus {
        /// Caller-provided RGBA8 pixels for the `_Tex` binding.
        texture_rgba: Vec<u8>,
        /// Texture dimensions in pixels.
        texture_size: (u32, u32),
    },
    /// Multi-object unlit scene with cube, sphere, torus, and quad coverage.
    MultiPrimitiveUnlitGrid {
        /// Checker texture RGBA8 pixels.
        checker_rgba: Vec<u8>,
        /// UV ramp texture RGBA8 pixels.
        uv_ramp_rgba: Vec<u8>,
        /// Shared texture dimensions in pixels.
        texture_size: (u32, u32),
    },
    /// PBS material matrix lit by regular scene lights.
    PbsLitMaterialMatrix,
    /// One PBS metallic sphere with `_ALPHAPREMULTIPLY_ON`, One/OneMinusSrcAlpha, and queue 3000.
    PbsMetallicPremultiply,
    /// Frame-bench only: a 40x40 grid of 1,600 unlit draws sharing one mesh and material.
    ManyStaticMeshes,
    /// Frame-bench only: 64 lit spheres on a floor with one shadowed directional and four shadowed point lights.
    LightsShadows,
    /// Alpha-test quads driven by a generated alpha mask.
    AlphaCutoutMaskedQuads {
        /// Alpha mask RGBA8 pixels.
        mask_rgba: Vec<u8>,
        /// Texture dimensions in pixels.
        texture_size: (u32, u32),
    },
    /// Textured static mesh imported from a GLB fixture and rendered through Unlit.
    GltfTexturedStaticMesh {
        /// Mesh imported from the fixture.
        mesh: Mesh,
        /// Fixture texture RGBA8 pixels.
        texture_rgba: Vec<u8>,
        /// Texture dimensions in pixels.
        texture_size: (u32, u32),
    },
    /// Custom rainbow shader on one material. The session renders twice: once with `RAINBOW`
    /// off, then again after `_Color` and `RAINBOW` change. `_Speed` stays 0.
    CustomShaderPropertyUpdate,
    /// Poincare disk on the left quad and a gyroid on the right. Both materials set `_Speed` to 0.
    ExoticShaderPair,
    /// Two coplanar `UI/Unlit` quads. The later, larger quad must cover the earlier smaller one.
    CoplanarCanvasUiQuads,
    /// Two tilted canvases, each a parent plus an in-plane child, at two yaws.
    TiltedCanvasUiQuads,
    /// Opaque quad with a slightly farther UI quad whose Offset units must win the depth test.
    CanvasOffsetBiasQuad,
    /// Tilted canvas at 18 m. The later sibling is 0.15 mm behind on the plane and must win
    /// by the sibling-step NDC bias.
    TiltedCanvasDistanceBias,
    /// PBS floor and cube under a hard spot shadow, captured static and then moved.
    ShadowedCasterMove,
    /// Thirty hard-shadow point lights over a small caster grid. Opt-in frame bench only.
    ManyShadowedPointLights,
    /// Normal Skybox camera with a higher-depth double-buffered Nothing camera on one texture.
    /// The main view displays that texture.
    DoubleBufferNothingOnSkybox,
    /// #216 term-isolation sphere. The spec selects the rig, term, and channel.
    Warp216Debug {
        /// Term, channel, and light rig.
        spec: crate::scene_dsl::warp_216_debug::Spec,
    },
    /// PBS sphere, one positive directional, and an optional negative directional.
    NegativeLightPair {
        /// Spawns the renderer with physically based light upload.
        physically_based: bool,
        /// Includes the extra light.
        include_negative: bool,
        /// The extra light uses a negative color and a positive intensity.
        ///
        /// When false, the extra light uses a negative intensity and a positive color.
        negative_color: bool,
    },
    /// One-bone skinned avatars; `offscreen` puts 256 of 272 behind the camera.
    SkinnedAvatars {
        /// Whether the bulk of the avatars sit outside the camera frustum.
        offscreen: bool,
    },
}

/// Drives the full session end-to-end. The renderer process is killed on `Err` via [`Drop`] of
/// the spawned-renderer guard.
pub(super) fn run_session(
    cfg: &SceneSessionConfig,
    template: SessionTemplate,
) -> Result<SceneSessionOutcome, HarnessError> {
    if !cfg.renderer_path.exists() {
        return Err(HarnessError::RendererBinaryMissing(
            cfg.renderer_path.clone(),
        ));
    }

    let mut session = connect_session(DEFAULT_QUEUE_CAPACITY_BYTES)?;
    let prefix = session.shared_memory_prefix.clone();
    let backing_dir = session.tempdir_guard.path().to_path_buf();
    let template_label = template_label(&template);
    logger::info!(
        "Session: opened authority queues (prefix={prefix}, backing_dir={}, template={template_label})",
        backing_dir.display()
    );

    let mut spawned = spawn_renderer(cfg, &session.connection_params.queue_name, &backing_dir)?;

    let mut lockstep = LockstepDriver::new(FrameSubmitScalars::default());
    run_handshake(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        DEFAULT_HANDSHAKE_TIMEOUT,
    )?;

    double_buffer_stack::prepare_assets(&template, &mut session.queues, &mut lockstep)?;

    let assignment = custom_shader_assignment(&template);
    let property_recapture = matches!(assignment, CustomShaderAssignment::PropertyUpdate);
    let shadow_move = matches!(template, SessionTemplate::ShadowedCasterMove);
    let setup = build_scene_setup(template);
    let _uploaded_meshes = upload_scene_meshes(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        &backing_dir,
        &setup,
    )?;
    let _bound_materials = upload_scene_materials(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        &backing_dir,
        &setup,
    )?;
    match assignment {
        CustomShaderAssignment::None => {}
        CustomShaderAssignment::PropertyUpdate => {
            custom_property::install_and_assign(&mut session.queues, &mut lockstep, &setup)?;
        }
        CustomShaderAssignment::Warp216 => {
            warp_216_debug::install_and_assign(
                &mut session.queues,
                &mut lockstep,
                &prefix,
                &backing_dir,
                &setup,
            )?;
        }
        CustomShaderAssignment::ExoticPair => {
            exotic_pair::install_and_assign(&mut session.queues, &mut lockstep, &setup)?;
        }
    }

    let png_outcome = capture::capture_submitted_scene(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        &backing_dir,
        &setup,
        &mut spawned,
        cfg,
        property_recapture,
        shadow_move,
    )?;

    request_shutdown_and_wait(&mut session.queues, &mut spawned)?;

    Ok(png_outcome)
}

fn template_label(template: &SessionTemplate) -> &'static str {
    match template {
        SessionTemplate::Sphere => "Sphere",
        SessionTemplate::Torus { .. } => "Torus",
        SessionTemplate::MultiPrimitiveUnlitGrid { .. } => "MultiPrimitiveUnlitGrid",
        SessionTemplate::PbsLitMaterialMatrix => "PbsLitMaterialMatrix",
        SessionTemplate::PbsMetallicPremultiply => "PbsMetallicPremultiply",
        SessionTemplate::ManyStaticMeshes => "ManyStaticMeshes",
        SessionTemplate::LightsShadows => "LightsShadows",
        SessionTemplate::AlphaCutoutMaskedQuads { .. } => "AlphaCutoutMaskedQuads",
        SessionTemplate::GltfTexturedStaticMesh { .. } => "GltfTexturedStaticMesh",
        SessionTemplate::CustomShaderPropertyUpdate => "CustomShaderPropertyUpdate",
        SessionTemplate::ExoticShaderPair => "ExoticShaderPair",
        SessionTemplate::CoplanarCanvasUiQuads => "CoplanarCanvasUiQuads",
        SessionTemplate::TiltedCanvasUiQuads => "TiltedCanvasUiQuads",
        SessionTemplate::CanvasOffsetBiasQuad => "CanvasOffsetBiasQuad",
        SessionTemplate::TiltedCanvasDistanceBias => "TiltedCanvasDistanceBias",
        SessionTemplate::ShadowedCasterMove => "ShadowedCasterMove",
        SessionTemplate::ManyShadowedPointLights => "ManyShadowedPointLights",
        SessionTemplate::DoubleBufferNothingOnSkybox => "DoubleBufferNothingOnSkybox",
        SessionTemplate::NegativeLightPair { .. } => "NegativeLightPair",
        SessionTemplate::Warp216Debug { .. } => "Warp216Debug",
        SessionTemplate::SkinnedAvatars { .. } => "SkinnedAvatars",
    }
}

#[cfg(test)]
mod tests;
