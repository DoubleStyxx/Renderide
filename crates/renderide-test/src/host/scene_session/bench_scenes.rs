//! Larger fixed scenes used only by `renderide-framebench`. They stay out of the golden suite.
//!
//! - `many_static_meshes`: draw-call, instancing, and CPU-cull pressure with no lights.
//! - `lights_shadows`: lit spheres with a shadowed directional light and shadowed point lights.
//! - `offscreen_skinned_avatars` / `onscreen_skinned_avatars`: one-bone skinned spheres, mostly
//!   behind the camera (or in view as the control), with a shadowed sun so every avatar still casts.
//! - `secondary_camera_copy`: the `unlit_sphere` scene plus two enabled secondary cameras that
//!   render into their own render textures, one with a `Depth` clear and one with `Nothing`
//!   (see [`secondary_camera_copy_rig`]).
//! - `secondary_camera_double_buffer`: the same sphere and pose, with two double-buffered
//!   cameras (`Depth` then `Nothing`, flags `1 | 4`) writing one 512² texture.

use glam::{Quat, Vec3};
use renderide_shared::shared::{CameraClearMode, RenderBoundingBox, ShadowCastMode, ShadowType};

use crate::scene::primitives::centered_bounds;
use crate::scene::sphere::generate_sphere;

use super::scene_state::{
    SceneCamera, SceneLight, SceneRenderable, SceneSkinned, SceneSubmission, next_free_transform_id,
};
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
            cameras: Vec::new(),
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
            cameras: Vec::new(),
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
            cameras: Vec::new(),
        },
    }
}

/// Render-texture asset id written by the `Depth`-clear secondary camera.
const SECONDARY_RT_DEPTH_CLEAR: i32 = 900;
/// Render-texture asset id written by the `Nothing`-clear secondary camera.
const SECONDARY_RT_NOTHING_CLEAR: i32 = 901;
/// Secondary render-texture size in pixels. Independent of the main view `--resolution`.
pub(super) const SECONDARY_RT_SIZE: (i32, i32) = (512, 512);

/// Render textures to allocate and cameras to submit for `secondary_camera_copy`.
pub(super) struct SecondaryCameraRig {
    /// Render-texture asset ids, all [`SECONDARY_RT_SIZE`] (`Rgba16Float` color + depth).
    pub render_textures: Vec<i32>,
    /// One `Depth`-clear and one `Nothing`-clear camera, one render texture each.
    pub cameras: Vec<SceneCamera>,
}

/// Two full-viewport, single-sample, non-double-buffered secondary cameras that share the main
/// camera pose (so they draw the same sphere) and render into their own render textures.
///
/// On fork master since 66421146 each of these non-clearing (`Depth` / `Nothing`) cameras seeds
/// its target with one full-target `Rgba16Float` previous-colour copy per frame (the
/// `WorldMeshForwardColorPreserve` pass); 0104fee7 is the pre-copy base that renders the same
/// draws with no copy, for an A/B. The main camera keeps the default `Skybox` clear.
pub(super) fn secondary_camera_copy_rig(scene: &SceneSubmission) -> SecondaryCameraRig {
    let first = next_free_transform_id(scene);
    let camera = |offset: i32, render_texture_asset_id, clear_mode, depth| {
        SceneCamera::secondary(
            first + offset,
            scene.camera_world_pose,
            render_texture_asset_id,
            clear_mode,
            depth,
        )
    };
    SecondaryCameraRig {
        render_textures: vec![SECONDARY_RT_DEPTH_CLEAR, SECONDARY_RT_NOTHING_CLEAR],
        cameras: vec![
            camera(0, SECONDARY_RT_DEPTH_CLEAR, CameraClearMode::Depth, 0.0),
            camera(1, SECONDARY_RT_NOTHING_CLEAR, CameraClearMode::Nothing, 1.0),
        ],
    }
}

/// Enabled (bit 0) and double-buffered (bit 2).
const DOUBLE_BUFFERED_ENABLED: u16 = 1 | (1 << 2);

/// Two stacked double-buffered secondary cameras writing one 512² render texture.
///
/// The `Depth` camera is depth 0, so it is the first writer and seeds history. The `Nothing`
/// camera is depth 1 and seeds the published image. Both use flags [`DOUBLE_BUFFERED_ENABLED`].
/// The pose is the scene camera pose, which the bench drive has already set.
pub(super) fn secondary_camera_double_buffer_rig(scene: &SceneSubmission) -> SecondaryCameraRig {
    let first = next_free_transform_id(scene);
    let camera = |offset: i32, clear_mode, depth| {
        let mut camera = SceneCamera::secondary(
            first + offset,
            scene.camera_world_pose,
            SECONDARY_RT_DEPTH_CLEAR,
            clear_mode,
            depth,
        );
        camera.flags = DOUBLE_BUFFERED_ENABLED;
        camera
    };
    SecondaryCameraRig {
        render_textures: vec![SECONDARY_RT_DEPTH_CLEAR],
        cameras: vec![
            camera(0, CameraClearMode::Depth, 0.0),
            camera(1, CameraClearMode::Nothing, 1.0),
        ],
    }
}
