//! Two coplanar unlit quads in one LOD group, captured inside the cross-fade band.
//!
//! Both renderers share a transform and a mesh. LOD 0 is red and LOD 1 is blue. The fade width
//! covers almost the whole screen-height span, so the default camera lands mid-transition and the
//! stipple mixes both colors. Shadows stay off.

use glam::{Quat, Vec3};
use renderide_shared::shared::ShadowCastMode;

use crate::scene::primitives::{centered_bounds, generate_quad};

use super::scene_state::{SceneLodGroup, SceneLodLevel};
use super::{
    SceneRenderable, SceneSetup, SceneSubmission, default_camera_world_pose, mesh_asset,
    scene_material_id, scene_mesh_id, transform, unlit_color_material_for_scene,
};

/// Small enough that desktop mesh LOD bias (2) still lands inside the fade band.
const QUAD_SCALE: f32 = 0.7;

pub(super) fn scene_setup() -> SceneSetup {
    let pose = transform(Vec3::ZERO, Vec3::splat(QUAD_SCALE), Quat::IDENTITY);
    SceneSetup {
        meshes: vec![mesh_asset(
            0,
            generate_quad(),
            centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
        )],
        materials: vec![
            unlit_color_material_for_scene(0, [1.0, 0.1, 0.1, 1.0]),
            unlit_color_material_for_scene(1, [0.1, 0.2, 1.0, 1.0]),
        ],
        scene: SceneSubmission {
            camera_world_pose: default_camera_world_pose(),
            renderables: vec![
                SceneRenderable {
                    transform_id: 0,
                    pose,
                    mesh_asset_id: scene_mesh_id(0),
                    material_asset_id: scene_material_id(0),
                    sorting_order: 0,
                    shadow_cast_mode: ShadowCastMode::Off,
                },
                SceneRenderable {
                    transform_id: 0,
                    pose,
                    mesh_asset_id: scene_mesh_id(0),
                    material_asset_id: scene_material_id(1),
                    sorting_order: 1,
                    shadow_cast_mode: ShadowCastMode::Off,
                },
            ],
            lights: Vec::new(),
            lod_group: Some(SceneLodGroup {
                node_id: 0,
                cross_fade: true,
                animate_cross_fading: false,
                levels: vec![
                    SceneLodLevel {
                        screen_relative_transition_height: 0.02,
                        fade_transition_width: 1.0,
                        renderer_indices: vec![0],
                    },
                    SceneLodLevel {
                        screen_relative_transition_height: 0.0,
                        fade_transition_width: 0.0,
                        renderer_indices: vec![1],
                    },
                ],
            }),
        },
    }
}
