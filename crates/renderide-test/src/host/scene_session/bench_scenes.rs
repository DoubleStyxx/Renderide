//! Larger fixed scenes used only by `renderide-framebench`. They stay out of the golden suite.
//!
//! - `many_static_meshes`: draw-call, instancing, and CPU-cull pressure with no lights.
//! - `lights_shadows`: lit spheres with a shadowed directional light and shadowed point lights.

use glam::{Quat, Vec3};
use renderide_shared::shared::ShadowType;

use crate::scene::primitives::centered_bounds;
use crate::scene::sphere::generate_sphere;

use super::scene_state::{SceneLight, SceneRenderable, SceneSubmission};
use super::{
    SceneSetup, directional_light, mesh_asset, pbs_material_for_scene, point_light, scene_material_id,
    scene_mesh_id, transform, unlit_color_material_for_scene,
};

const GRID_SIDE: i32 = 40;
const LIGHT_TRANSFORM_BASE: i32 = 100_000;

/// 40x40 unlit spheres in one plane, one shared mesh and material (1,600 draws).
pub(super) fn many_static_meshes_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_sphere(8, 12),
        centered_bounds(Vec3::splat(1.05)),
    )];
    let materials = vec![unlit_color_material_for_scene(0, [0.35, 0.75, 0.95, 1.0])];
    let spacing = 1.1_f32;
    let half = (GRID_SIDE as f32 - 1.0) * 0.5;
    let mut renderables = Vec::with_capacity((GRID_SIDE * GRID_SIDE) as usize);
    for row in 0..GRID_SIDE {
        for col in 0..GRID_SIDE {
            let index = row * GRID_SIDE + col;
            let position = Vec3::new(
                (col as f32 - half) * spacing,
                (row as f32 - half) * spacing,
                0.0,
            );
            renderables.push(SceneRenderable {
                transform_id: index,
                pose: transform(position, Vec3::splat(0.45), Quat::IDENTITY),
                mesh_asset_id: scene_mesh_id(0),
                material_asset_id: scene_material_id(0),
                sorting_order: 0,
            });
        }
    }
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.0, -24.0), Vec3::ONE, Quat::IDENTITY),
            renderables,
            lights: Vec::new(),
        },
    }
}

/// 8x8 PBS spheres plus a floor-like flattened sphere, lit by one shadowed directional light and
/// four shadowed point lights.
pub(super) fn lights_shadows_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_sphere(16, 24),
        centered_bounds(Vec3::splat(1.05)),
    )];
    let materials = vec![
        pbs_material_for_scene(0, [0.85, 0.85, 0.9, 1.0], 0.0, 0.4),
        pbs_material_for_scene(1, [0.9, 0.35, 0.25, 1.0], 0.2, 0.7),
    ];
    let mut renderables = Vec::with_capacity(65);
    renderables.push(SceneRenderable {
        transform_id: 0,
        pose: transform(Vec3::new(0.0, -0.6, 0.0), Vec3::new(14.0, 0.1, 14.0), Quat::IDENTITY),
        mesh_asset_id: scene_mesh_id(0),
        material_asset_id: scene_material_id(0),
        sorting_order: 0,
    });
    for row in 0..8 {
        for col in 0..8 {
            let index = 1 + row * 8 + col;
            let position = Vec3::new((col as f32 - 3.5) * 1.5, 0.0, (row as f32 - 3.5) * 1.5);
            renderables.push(SceneRenderable {
                transform_id: index,
                pose: transform(position, Vec3::splat(0.5), Quat::IDENTITY),
                mesh_asset_id: scene_mesh_id(0),
                material_asset_id: scene_material_id(1),
                sorting_order: 0,
            });
        }
    }
    let mut sun = directional_light([1.0, 0.95, 0.85], 1.2);
    sun.shadow_type = ShadowType::Soft;
    sun.shadow_strength = 1.0;
    let mut lights = vec![SceneLight {
        transform_id: LIGHT_TRANSFORM_BASE,
        pose: transform(Vec3::new(0.0, 8.0, 0.0), Vec3::ONE, Quat::from_rotation_x(-1.0)),
        state: sun,
    }];
    for (i, (x, z)) in [(-4.0, -4.0), (4.0, -4.0), (-4.0, 4.0), (4.0, 4.0)]
        .into_iter()
        .enumerate()
    {
        let mut state = point_light([0.5, 0.65, 1.0], 5.0, 9.0);
        state.shadow_type = ShadowType::Soft;
        state.shadow_strength = 1.0;
        lights.push(SceneLight {
            transform_id: LIGHT_TRANSFORM_BASE + 1 + i as i32,
            pose: transform(Vec3::new(x, 2.5, z), Vec3::ONE, Quat::IDENTITY),
            state,
        });
    }
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 6.0, -14.0), Vec3::ONE, Quat::from_rotation_x(0.2618)),
            renderables,
            lights,
        },
    }
}
