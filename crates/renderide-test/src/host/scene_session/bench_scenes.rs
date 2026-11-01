//! Larger fixed scenes used only by `renderide-framebench`. They stay out of the golden suite.
//!
//! - `many_static_meshes`: draw-call, instancing, and CPU-cull pressure with no lights.
//! - `lights_shadows`: lit spheres with a shadowed directional light and shadowed point lights.
//! - `offscreen_skinned_avatars` / `onscreen_skinned_avatars`: one-bone skinned spheres, mostly
//!   behind the camera (or in view as the control), with a shadowed sun so every avatar still casts.
//! - `many_instances`: GPU-driven judge floor — 100×100 unlit spheres (10,000 draws), one mesh and
//!   material. Stresses draw-prep, CPU cull, and instancing headroom Vertex phases must beat.
//! - `occlusion_heavy`: GPU-driven judge floor — large near occluders in front of thousands of
//!   spheres. Stresses temporal Hi-Z / occlusion (previous-frame pyramid) after warmup.
//! - `material_variety`: GPU-driven judge floor — 64 distinct materials (48 PBS + 16 Unlit) on a
//!   shared mesh. Stresses material bind groups and pipeline-family variety.
//! - `secondary_camera_copy`: the `unlit_sphere` scene plus two enabled secondary cameras that
//!   render into their own render textures, one with a `Depth` clear and one with `Nothing`
//!   (see [`secondary_camera_copy_rig`]).
//! - `secondary_camera_double_buffer`: the same sphere and pose, with two double-buffered
//!   cameras (`Depth` then `Nothing`, flags `1 | 4`) writing one 512² texture.
//!
//! Pass order matches the Unity/Renderide forward graph (shadows → depth prepass → opaque →
//! transparent → `hi_z_build`); see `CONTRIBUTING.md` § passes and Renderite.Unity
//! `CameraPortal`/`CameraController` shadow + forward opaque staging
//! (`/workspace/reference/Renderite` @ 7ccfd2ee).

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

/// 100×100 unlit spheres (10,000 draws), one shared mesh and material. Judge floor for
/// GPU-driven / instancing work: submitted draw count stays at 10k when nothing is culled.
const MANY_INSTANCES_SIDE: i32 = 100;

/// Frame-bench only: 10,000 unlit draws in one plane.
pub(super) fn many_instances_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_sphere(6, 8),
        centered_bounds(Vec3::splat(1.05)),
    )];
    let materials = vec![unlit_color_material_for_scene(0, [0.45, 0.7, 0.95, 1.0])];
    let spacing = 0.55_f32;
    let half = (MANY_INSTANCES_SIDE as f32 - 1.0) * 0.5;
    let mut renderables = Vec::with_capacity((MANY_INSTANCES_SIDE * MANY_INSTANCES_SIDE) as usize);
    for row in 0..MANY_INSTANCES_SIDE {
        for col in 0..MANY_INSTANCES_SIDE {
            let index = row * MANY_INSTANCES_SIDE + col;
            let position = Vec3::new(
                (col as f32 - half) * spacing,
                (row as f32 - half) * spacing,
                0.0,
            );
            renderables.push(SceneRenderable {
                transform_id: index,
                pose: transform(position, Vec3::splat(0.22), Quat::IDENTITY),
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
            camera_world_pose: transform(Vec3::new(0.0, 0.0, -55.0), Vec3::ONE, Quat::IDENTITY),
            renderables,
            lights: Vec::new(),
            skinned: Vec::new(),
            cameras: Vec::new(),
        },
    }
}

/// Hidden-sphere grid behind the occluder wall.
const OCCLUSION_HIDDEN_SIDE: i32 = 64;
/// Visible spheres kept clear of the occluder so the scene is never zero-draw.
const OCCLUSION_VISIBLE: i32 = 16;
const OCCLUSION_TRANSFORM_BASE: i32 = 10;

/// Large near wall plus 4,096 spheres behind it and 16 visible accents. After warmup the
/// previous-frame Hi-Z pyramid should cull most of the hidden grid; `draw_calls` is the signal.
pub(super) fn occlusion_heavy_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_sphere(8, 12),
        centered_bounds(Vec3::splat(1.05)),
    )];
    let materials = vec![
        unlit_color_material_for_scene(0, [0.12, 0.12, 0.14, 1.0]),
        unlit_color_material_for_scene(1, [0.85, 0.55, 0.25, 1.0]),
        unlit_color_material_for_scene(2, [0.35, 0.8, 0.55, 1.0]),
    ];
    let mut renderables = Vec::with_capacity(
        2 + (OCCLUSION_HIDDEN_SIDE * OCCLUSION_HIDDEN_SIDE) as usize + OCCLUSION_VISIBLE as usize,
    );
    // Floor (not an occluder for the grid; keeps depth continuity).
    renderables.push(SceneRenderable {
        transform_id: 0,
        pose: transform(
            Vec3::new(0.0, -1.2, 8.0),
            Vec3::new(40.0, 0.08, 40.0),
            Quat::IDENTITY,
        ),
        mesh_asset_id: scene_mesh_id(0),
        material_asset_id: scene_material_id(0),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::Off,
    });
    // Near occluder wall covering the central FOV (writes opaque depth before Hi-Z build).
    renderables.push(SceneRenderable {
        transform_id: 1,
        pose: transform(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(18.0, 10.0, 0.4),
            Quat::IDENTITY,
        ),
        mesh_asset_id: scene_mesh_id(0),
        material_asset_id: scene_material_id(0),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::Off,
    });
    let half = (OCCLUSION_HIDDEN_SIDE as f32 - 1.0) * 0.5;
    let spacing = 0.55_f32;
    for row in 0..OCCLUSION_HIDDEN_SIDE {
        for col in 0..OCCLUSION_HIDDEN_SIDE {
            let index = OCCLUSION_TRANSFORM_BASE + row * OCCLUSION_HIDDEN_SIDE + col;
            // Behind the wall along +Z (camera looks from -Z).
            let position = Vec3::new(
                (col as f32 - half) * spacing,
                (row as f32 - half) * spacing * 0.35 + 1.5,
                4.0 + (row as f32) * 0.08,
            );
            renderables.push(SceneRenderable {
                transform_id: index,
                pose: transform(position, Vec3::splat(0.2), Quat::IDENTITY),
                mesh_asset_id: scene_mesh_id(0),
                material_asset_id: scene_material_id(1),
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::Off,
            });
        }
    }
    let visible_base = OCCLUSION_TRANSFORM_BASE + OCCLUSION_HIDDEN_SIDE * OCCLUSION_HIDDEN_SIDE;
    for i in 0..OCCLUSION_VISIBLE {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let position = Vec3::new(
            side * (10.5 + (i / 2) as f32 * 0.7),
            0.5,
            -2.0 + (i as f32) * 0.3,
        );
        renderables.push(SceneRenderable {
            transform_id: visible_base + i,
            pose: transform(position, Vec3::splat(0.35), Quat::IDENTITY),
            mesh_asset_id: scene_mesh_id(0),
            material_asset_id: scene_material_id(2),
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::Off,
        });
    }
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: transform(
                Vec3::new(0.0, 3.0, -18.0),
                Vec3::ONE,
                Quat::from_rotation_x(0.12),
            ),
            renderables,
            lights: Vec::new(),
            skinned: Vec::new(),
            cameras: Vec::new(),
        },
    }
}

/// Distinct PBS + Unlit materials on one shared mesh (64 draws). Indices stay under 100 so
/// material / shader asset id ranges do not collide (`SCENE_MATERIAL_BASE` / `SCENE_SHADER_BASE`).
const MATERIAL_VARIETY_COUNT: i32 = 64;
const MATERIAL_VARIETY_PBS: i32 = 48;

/// Frame-bench only: 64 unique materials (48 PBSMetallic + 16 Unlit color) in an 8×8 grid.
pub(super) fn material_variety_setup() -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_sphere(10, 14),
        centered_bounds(Vec3::splat(1.05)),
    )];
    let mut materials = Vec::with_capacity(MATERIAL_VARIETY_COUNT as usize);
    for i in 0..MATERIAL_VARIETY_PBS {
        let t = i as f32 / (MATERIAL_VARIETY_PBS as f32);
        let color = [
            0.2 + 0.75 * t,
            0.15 + 0.55 * (1.0 - t),
            0.35 + 0.45 * ((i % 7) as f32 / 6.0),
            1.0,
        ];
        let metallic = (i % 5) as f32 / 4.0;
        let glossiness = 0.2 + 0.7 * ((i % 8) as f32 / 7.0);
        materials.push(pbs_material_for_scene(
            i as usize, color, metallic, glossiness,
        ));
    }
    for i in MATERIAL_VARIETY_PBS..MATERIAL_VARIETY_COUNT {
        let u = (i - MATERIAL_VARIETY_PBS) as f32 / 15.0;
        let color = [0.9 * (1.0 - u), 0.3 + 0.5 * u, 0.85, 1.0];
        materials.push(unlit_color_material_for_scene(i, color));
    }
    let side = 8_i32;
    let half = (side as f32 - 1.0) * 0.5;
    let spacing = 1.35_f32;
    let mut renderables = Vec::with_capacity(MATERIAL_VARIETY_COUNT as usize);
    for row in 0..side {
        for col in 0..side {
            let index = row * side + col;
            let position = Vec3::new(
                (col as f32 - half) * spacing,
                (row as f32 - half) * spacing,
                0.0,
            );
            renderables.push(SceneRenderable {
                transform_id: index,
                pose: transform(position, Vec3::splat(0.45), Quat::IDENTITY),
                mesh_asset_id: scene_mesh_id(0),
                material_asset_id: scene_material_id(index),
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::Off,
            });
        }
    }
    let lights = vec![SceneLight {
        transform_id: LIGHT_TRANSFORM_BASE,
        pose: transform(
            Vec3::new(0.0, 8.0, -4.0),
            Vec3::ONE,
            Quat::from_rotation_x(-0.8),
        ),
        state: directional_light([1.0, 0.95, 0.88], 1.3),
    }];
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.0, -14.0), Vec3::ONE, Quat::IDENTITY),
            renderables,
            lights,
            skinned: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::{
        MANY_INSTANCES_SIDE, MATERIAL_VARIETY_COUNT, MATERIAL_VARIETY_PBS, OCCLUSION_HIDDEN_SIDE,
        OCCLUSION_VISIBLE, many_instances_setup, material_variety_setup, occlusion_heavy_setup,
    };

    #[test]
    fn many_instances_submits_ten_thousand_draws() {
        let setup = many_instances_setup();
        assert_eq!(
            setup.scene.renderables.len(),
            (MANY_INSTANCES_SIDE * MANY_INSTANCES_SIDE) as usize
        );
        assert_eq!(setup.materials.len(), 1);
        assert!(setup.scene.lights.is_empty());
    }

    #[test]
    fn occlusion_heavy_has_wall_and_hidden_grid() {
        let setup = occlusion_heavy_setup();
        let expected = 2
            + (OCCLUSION_HIDDEN_SIDE * OCCLUSION_HIDDEN_SIDE) as usize
            + OCCLUSION_VISIBLE as usize;
        assert_eq!(setup.scene.renderables.len(), expected);
        assert_eq!(setup.materials.len(), 3);
    }

    #[test]
    fn material_variety_has_sixty_four_distinct_materials() {
        let setup = material_variety_setup();
        assert_eq!(setup.materials.len(), MATERIAL_VARIETY_COUNT as usize);
        assert_eq!(
            setup.scene.renderables.len(),
            MATERIAL_VARIETY_COUNT as usize
        );
        let mut ids: Vec<_> = setup
            .materials
            .iter()
            .map(|material| material.asset_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MATERIAL_VARIETY_COUNT as usize);
        assert_eq!(MATERIAL_VARIETY_PBS, 48);
        assert_eq!(setup.scene.lights.len(), 1);
    }
}
