//! Coplanar and tilted UI quad scenes for the canvas depth-order and NDC-bias goldens.

use glam::{Quat, Vec3};
use renderide_shared::shared::ShadowCastMode;

use crate::scene::primitives::{centered_bounds, generate_quad};

use super::scene_state::default_camera_world_pose;
use super::{
    Float4Binding, FloatBinding, MaterialAsset, MaterialBinding, SceneRenderable, SceneSetup,
    SceneSubmission, mesh_asset, scene_material_id, scene_material_update_buffer_id, scene_mesh_id,
    scene_property_request_id, scene_shader_id, scene_update_batch_id, transform,
    unlit_color_material_for_scene,
};

/// Two coplanar UI quads. The later, larger quad covers the earlier smaller one.
pub(super) fn coplanar_canvas_ui_quads_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_quad(),
        centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
    )];
    let materials = vec![
        ui_unlit_color_material(0, [1.0, 0.05, 0.05, 1.0]),
        ui_unlit_color_material(1, [0.05, 0.2, 1.0, 1.0]),
    ];
    let renderables = [(0, Vec3::new(0.4, 0.4, 1.0)), (1, Vec3::ONE)]
        .into_iter()
        .map(|(index, scale)| SceneRenderable {
            transform_id: index,
            pose: transform(Vec3::ZERO, scale, Quat::IDENTITY),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(index),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        })
        .collect();
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables,
            lights: Vec::new(),
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

/// Tilted parent and child quads at +35° and −40° yaw.
pub(super) fn tilted_canvas_ui_quads_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_quad(),
        centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
    )];
    let materials = vec![
        ui_unlit_color_material(0, [1.0, 0.05, 0.05, 1.0]),
        ui_unlit_color_material(1, [0.05, 0.2, 1.0, 1.0]),
    ];
    // +35° and −40° are both past 30°. These in-plane X offsets move each child center
    // farther along camera +Z (positive world Z). The center-depth key then draws the child
    // first and the parent covers it.
    let mut renderables =
        tilted_canvas_pair(Vec3::new(-1.0, 0.0, 0.0), 35.0_f32.to_radians(), -0.22, 0);
    renderables.extend(tilted_canvas_pair(
        Vec3::new(1.0, 0.0, 0.0),
        -40.0_f32.to_radians(),
        0.22,
        2,
    ));
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables,
            lights: Vec::new(),
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

fn tilted_canvas_pair(
    origin: Vec3,
    yaw: f32,
    child_local_x: f32,
    transform_base: i32,
) -> Vec<SceneRenderable> {
    let rotation = Quat::from_rotation_y(yaw);
    let child_position = origin + rotation * Vec3::new(child_local_x, 0.0, 0.0);
    vec![
        SceneRenderable {
            transform_id: transform_base,
            pose: transform(origin, Vec3::new(1.7, 1.7, 1.0), rotation),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(0),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
        SceneRenderable {
            transform_id: transform_base + 1,
            pose: transform(child_position, Vec3::new(0.9, 0.9, 1.0), rotation),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(1),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
    ]
}

/// Opaque quad with a UI quad 0.6 mm farther and `_OffsetUnits = -64`.
pub(super) fn canvas_offset_bias_quad_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_quad(),
        centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
    )];
    let materials = vec![
        unlit_color_material_for_scene(0, [1.0, 0.05, 0.05, 1.0]),
        ui_unlit_offset_material(1, [0.05, 0.2, 1.0, 1.0], -64.0),
    ];
    let renderables = vec![
        SceneRenderable {
            transform_id: 0,
            pose: transform(Vec3::ZERO, Vec3::new(1.1, 1.1, 1.0), Quat::IDENTITY),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(0),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
        SceneRenderable {
            transform_id: 1,
            pose: transform(
                Vec3::new(0.0, 0.0, 0.0006),
                Vec3::new(0.45, 0.45, 1.0),
                Quat::IDENTITY,
            ),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(1),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
    ];
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables,
            lights: Vec::new(),
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

/// Tilted siblings at 18 m. The later child is 0.15 mm behind on the plane.
pub(super) fn tilted_canvas_distance_bias_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_quad(),
        centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
    )];
    let materials = vec![
        ui_unlit_color_material(0, [1.0, 0.05, 0.05, 1.0]),
        ui_unlit_color_material(1, [0.05, 0.2, 1.0, 1.0]),
    ];
    // Center is 18 m from the camera at z = -3. A 60° yaw makes a 0.15 mm normal nudge
    // about 0.30 mm along the view ray: more than one D24 ulp and less than one 4-ulp step.
    // The pair stays inside one 0.5 mm plane quantum, so the later child gets step 1.
    let yaw = 60.0_f32.to_radians();
    let rotation = Quat::from_rotation_y(yaw);
    let parent = Vec3::new(0.0, 0.0, 15.0);
    let child = parent + rotation * Vec3::new(0.0, 0.0, 0.00015);
    let renderables = vec![
        SceneRenderable {
            transform_id: 0,
            pose: transform(parent, Vec3::new(7.0, 7.0, 1.0), rotation),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(0),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
        SceneRenderable {
            transform_id: 1,
            pose: transform(child, Vec3::new(2.2, 2.2, 1.0), rotation),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(1),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        },
    ];
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables,
            lights: Vec::new(),
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

fn ui_unlit_color_material(index: i32, tint: [f32; 4]) -> MaterialAsset {
    MaterialAsset {
        asset_id: scene_material_id(index),
        update_batch_id: scene_update_batch_id(index),
        property_request_id: scene_property_request_id(index),
        update_base_buffer_id: scene_material_update_buffer_id(index),
        binding: Some(MaterialBinding {
            shader_asset_id: scene_shader_id(index),
            shader_name: "UI_Unlit.shader",
            shader_variant_bits: None,
            textures: Vec::new(),
            floats: vec![FloatBinding {
                property_name: "_ZWrite",
                value: 1.0,
            }],
            float4s: vec![Float4Binding {
                property_name: "_Tint",
                value: tint,
            }],
        }),
    }
}

fn ui_unlit_offset_material(index: i32, tint: [f32; 4], offset_units: f32) -> MaterialAsset {
    let mut material = ui_unlit_color_material(index, tint);
    if let Some(binding) = material.binding.as_mut() {
        binding.floats.push(FloatBinding {
            property_name: "_OffsetUnits",
            value: offset_units,
        });
    }
    material
}
