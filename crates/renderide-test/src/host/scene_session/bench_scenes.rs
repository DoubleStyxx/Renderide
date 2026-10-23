//! Larger fixed scenes used only by `renderide-framebench`. They stay out of the golden suite.
//!
//! - `many_static_meshes`: draw-call, instancing, and CPU-cull pressure with no lights.
//! - `lights_shadows`: lit spheres with a shadowed directional light and shadowed point lights.
//! - `offscreen_skinned_avatars` / `onscreen_skinned_avatars`: one-bone skinned spheres, mostly
//!   behind the camera (or in view as the control), with a shadowed sun so every avatar still casts.

use glam::{Quat, Vec3};
use renderide_shared::shared::{RenderBoundingBox, ShadowCastMode, ShadowType};

use crate::scene::primitives::centered_bounds;
use crate::scene::sphere::generate_sphere;

use super::scene_state::{SceneLight, SceneRenderable, SceneSkinned, SceneSubmission};
use super::{
    SceneSetup, directional_light, mesh_asset, pbs_material_for_scene, point_light,
    scene_material_id, scene_mesh_id, transform, unlit_color_material_for_scene,
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
                shadow_cast_mode: ShadowCastMode::Off,
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
            skinned: Vec::new(),
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
        pose: transform(
            Vec3::new(0.0, -0.6, 0.0),
            Vec3::new(14.0, 0.1, 14.0),
            Quat::IDENTITY,
        ),
        mesh_asset_id: scene_mesh_id(0),
        material_asset_id: scene_material_id(0),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::Off,
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
                shadow_cast_mode: ShadowCastMode::On,
            });
        }
    }
    let mut sun = directional_light([1.0, 0.95, 0.85], 1.2);
    sun.shadow_type = ShadowType::Soft;
    sun.shadow_strength = 1.0;
    let mut lights = vec![SceneLight {
        transform_id: LIGHT_TRANSFORM_BASE,
        pose: transform(
            Vec3::new(0.0, 8.0, 0.0),
            Vec3::ONE,
            Quat::from_rotation_x(-1.0),
        ),
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
            camera_world_pose: transform(
                Vec3::new(0.0, 6.0, -14.0),
                Vec3::ONE,
                Quat::from_rotation_x(0.2618),
            ),
            renderables,
            lights,
            skinned: Vec::new(),
        },
    }
}

/// Avatars placed in the camera frustum.
const VISIBLE_AVATARS: i32 = 16;
/// Extra avatars placed behind the camera (off-screen) or in front of it (control).
const EXTRA_AVATARS: i32 = 256;
const AVATAR_TRANSFORM_BASE: i32 = 1_000;

/// A floor, 16 visible one-bone skinned spheres and 256 extras that are either behind the camera
/// (`offscreen = true`: outside the frustum but inside shadow distance) or in view (control).
///
/// Every avatar casts shadows from one soft-shadowed directional light, so the shadow pass keeps
/// all 272 casters regardless of camera culling. Each avatar uses one bone (its own node),
/// identity bind pose, and host-sent posed bounds, so `culling` can use posed bounds.
pub(super) fn skinned_avatars_setup(offscreen: bool) -> SceneSetup {
    let mut skinned_mesh = mesh_asset(
        0,
        generate_sphere(12, 16),
        centered_bounds(Vec3::splat(1.05)),
    );
    skinned_mesh.skinned = true;
    let floor_mesh = mesh_asset(
        1,
        generate_sphere(16, 24),
        centered_bounds(Vec3::splat(1.05)),
    );
    let meshes = vec![skinned_mesh, floor_mesh];
    let materials = vec![
        pbs_material_for_scene(0, [0.85, 0.85, 0.9, 1.0], 0.0, 0.4),
        pbs_material_for_scene(1, [0.3, 0.7, 0.45, 1.0], 0.1, 0.6),
    ];
    let renderables = vec![SceneRenderable {
        transform_id: 0,
        pose: transform(
            Vec3::new(0.0, -0.6, 0.0),
            Vec3::new(40.0, 0.1, 60.0),
            Quat::IDENTITY,
        ),
        mesh_asset_id: scene_mesh_id(1),
        material_asset_id: scene_material_id(0),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::Off,
    }];
    let posed_bounds = RenderBoundingBox {
        center: Vec3::ZERO,
        extents: Vec3::splat(1.05),
    };
    let mut skinned = Vec::with_capacity((VISIBLE_AVATARS + EXTRA_AVATARS) as usize);
    let mut push_avatar = |index: i32, position: Vec3| {
        skinned.push(SceneSkinned {
            transform_id: AVATAR_TRANSFORM_BASE + index,
            pose: transform(position, Vec3::splat(0.5), Quat::IDENTITY),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(1),
            shadow_cast_mode: ShadowCastMode::On,
            posed_bounds,
        });
    };
    for i in 0..VISIBLE_AVATARS {
        let (row, col) = (i / 4, i % 4);
        push_avatar(
            i,
            Vec3::new((col as f32 - 1.5) * 1.8, 0.0, (row as f32 - 1.5) * 1.8),
        );
    }
    for i in 0..EXTRA_AVATARS {
        let (row, col) = (i / 16, i % 16);
        let x = (col as f32 - 7.5) * 1.6;
        let z = if offscreen {
            -18.0 - row as f32 * 1.6
        } else {
            6.0 + row as f32 * 1.6
        };
        push_avatar(VISIBLE_AVATARS + i, Vec3::new(x, 0.0, z));
    }
    let mut sun = directional_light([1.0, 0.95, 0.85], 1.2);
    sun.shadow_type = ShadowType::Soft;
    sun.shadow_strength = 1.0;
    let lights = vec![SceneLight {
        transform_id: LIGHT_TRANSFORM_BASE,
        pose: transform(
            Vec3::new(0.0, 8.0, 0.0),
            Vec3::ONE,
            Quat::from_rotation_x(-1.0),
        ),
        state: sun,
    }];
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: transform(
                Vec3::new(0.0, 4.0, -12.0),
                Vec3::ONE,
                Quat::from_rotation_x(0.2),
            ),
            renderables,
            lights,
            skinned,
        },
    }
}
