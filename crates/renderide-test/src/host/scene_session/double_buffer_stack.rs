//! Headless capture of a normal Skybox camera with a later double-buffered Nothing camera.
//!
//! Both cameras write one render texture. The main view only displays that texture, so the
//! golden is the shared target after the stacked clears. On the execution-order head the Skybox
//! is the first writer and the Nothing camera copies this frame's published image. Before that
//! fix the Nothing camera copies history over the Skybox.

use glam::{Quat, Vec3, Vec4};
use renderide_shared::ipc::HostDualQueueIpc;
use renderide_shared::shared::{
    CameraClearMode, CameraProjection, CameraState, RenderRect, ShadowCastMode,
};

use crate::error::HarnessError;
use crate::scene::primitives::{centered_bounds, generate_quad};
use crate::scene::sphere::generate_sphere;

use super::super::asset_upload::{pack_render_texture_handle, upload_render_texture};
use super::super::lockstep::LockstepDriver;
use super::consts::shader_variants;
use super::scene_state::{SceneCamera, default_camera_world_pose};
use super::{
    Float4Binding, MaterialAsset, MaterialBinding, SceneRenderable, SceneSetup, SceneSubmission,
    SessionTemplate, TextureBinding, mesh_asset, scene_material_id,
    scene_material_update_buffer_id, scene_mesh_id, scene_property_request_id, scene_shader_id,
    scene_update_batch_id, transform, unlit_color_material_for_scene,
};
use renderide_shared::shared::ColorProfile;

/// Render-texture asset both stacked cameras write and the main-view quad samples.
const RENDER_TEXTURE_ASSET: i32 = 900;
/// Edge length of the render texture and of the case framebuffer.
const RENDER_TEXTURE_EDGE: i32 = 256;
/// Sphere transform. Both cameras selective-render this id and skip the display quad.
const SPHERE_TRANSFORM: i32 = 0;
/// Full-screen quad that shows the render texture in the captured main view.
const QUAD_TRANSFORM: i32 = 1;
/// Shared pose for the two render-texture cameras. Matches the main-view eye.
const CAMERA_TRANSFORM: i32 = 2;

/// Uploads the shared render texture when this session is the stacked-camera case.
pub(super) fn prepare_assets(
    template: &SessionTemplate,
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
) -> Result<(), HarnessError> {
    if !matches!(template, SessionTemplate::DoubleBufferNothingOnSkybox) {
        return Ok(());
    }
    upload_render_texture(
        queues,
        lockstep,
        RENDER_TEXTURE_ASSET,
        (RENDER_TEXTURE_EDGE, RENDER_TEXTURE_EDGE),
    )
}

/// Skybox camera at depth 0 and a double-buffered Nothing camera at a higher depth.
pub(super) fn scene_setup() -> SceneSetup {
    let meshes = vec![
        mesh_asset(
            0,
            generate_sphere(16, 24),
            centered_bounds(Vec3::splat(1.05)),
        ),
        mesh_asset(
            1,
            generate_quad(),
            centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
        ),
    ];
    let materials = vec![
        unlit_color_material_for_scene(0, [1.0, 0.15, 0.05, 1.0]),
        render_texture_quad_material(),
    ];
    let renderables = vec![
        SceneRenderable {
            transform_id: SPHERE_TRANSFORM,
            pose: transform(Vec3::new(0.35, 0.0, 1.2), Vec3::splat(0.4), Quat::IDENTITY),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(0),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
        SceneRenderable {
            transform_id: QUAD_TRANSFORM,
            pose: transform(Vec3::new(0.0, 0.0, -2.15), Vec3::splat(8.0), Quat::IDENTITY),
            mesh_asset_id: scene_mesh_id(1),
            material_asset_id: scene_material_id(1),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
    ];
    let camera_pose = default_camera_world_pose();
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: camera_pose,
            renderables,
            lights: Vec::new(),
            cameras: vec![
                stacked_camera(0.0, CameraClearMode::Skybox, false),
                stacked_camera(1.0, CameraClearMode::Nothing, true),
            ],
        },
    }
}

fn render_texture_quad_material() -> MaterialAsset {
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
                asset_id: RENDER_TEXTURE_ASSET,
                buffer_id: 0,
                property_name: "_Tex",
                st_property_name: "_Tex_ST",
                st_value: [1.0, 1.0, 0.0, 0.0],
                rgba_bytes: Vec::new(),
                size: (0, 0),
                color_profile: ColorProfile::Linear,
                packed_handle: Some(pack_render_texture_handle(RENDER_TEXTURE_ASSET)),
            }],
            floats: Vec::new(),
            float4s: vec![Float4Binding {
                property_name: "_Color",
                value: [1.0, 1.0, 1.0, 1.0],
            }],
        }),
    }
}

fn stacked_camera(depth: f32, clear_mode: CameraClearMode, double_buffered: bool) -> SceneCamera {
    let mut flags = 1_u16;
    if double_buffered {
        flags |= 1 << 2;
    }
    let background_color = if clear_mode == CameraClearMode::Skybox {
        Vec4::new(0.15, 0.75, 0.95, 1.0)
    } else {
        Vec4::ZERO
    };
    SceneCamera {
        transform_id: CAMERA_TRANSFORM,
        pose: default_camera_world_pose(),
        selective_transform_ids: vec![SPHERE_TRANSFORM],
        state: CameraState {
            field_of_view: 60.0,
            orthographic_size: 1.0,
            near_clip: 0.1,
            far_clip: 100.0,
            background_color,
            viewport: RenderRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            depth,
            render_texture_asset_id: RENDER_TEXTURE_ASSET,
            clear_mode,
            projection: CameraProjection::Perspective,
            flags,
            ..CameraState::default()
        },
    }
}
