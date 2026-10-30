//! PBS sphere lit by one positive directional light and an optional negative directional.
//!
//! Parity mode keeps the negative intensity, so that light darkens the hemisphere it faces.
//! Physically based upload clamps that intensity to zero, which must match this scene with
//! the negative light omitted.

use glam::{Quat, Vec3};
use renderide_shared::shared::ShadowCastMode;

use crate::scene::primitives::centered_bounds;
use crate::scene::sphere::generate_sphere;

use super::scene_state::{SceneLight, SceneRenderable, SceneSubmission, directional_light};
use super::{
    SceneSetup, mesh_asset, pbs_material_for_scene, scene_material_id, scene_mesh_id, transform,
};

/// Key-light intensity. Strong enough to shade the sphere without blowing out the frame.
const POSITIVE_INTENSITY: f32 = 1.05;
/// Fill intensity. Negative so parity mode darkens the front-right of the sphere.
const NEGATIVE_INTENSITY: f32 = -1.45;

/// Builds the negative-light sphere. `include_negative` adds the darkening directional.
pub(super) fn scene_setup(include_negative: bool) -> SceneSetup {
    let meshes = vec![mesh_asset(
        0,
        generate_sphere(18, 28),
        centered_bounds(Vec3::splat(1.05)),
    )];
    let materials = vec![pbs_material_for_scene(0, [0.84, 0.8, 0.74, 1.0], 0.0, 0.32)];
    let mut lights = vec![SceneLight {
        transform_id: 10,
        pose: transform(
            Vec3::new(-1.2, 2.0, -1.4),
            Vec3::ONE,
            Quat::from_rotation_x(-0.65),
        ),
        state: directional_light([1.0, 0.96, 0.9], POSITIVE_INTENSITY),
    }];
    if include_negative {
        // Local +Z is the travel direction. Aim it from the camera toward the front-right
        // so the visible disk darkens on that side, not only on the silhouette.
        let travel = Vec3::new(-0.55, -0.15, 0.82).normalize();
        lights.push(SceneLight {
            transform_id: 11,
            pose: transform(
                Vec3::ZERO,
                Vec3::ONE,
                Quat::from_rotation_arc(Vec3::Z, travel),
            ),
            state: directional_light([0.75, 0.85, 1.0], NEGATIVE_INTENSITY),
        });
    }
    SceneSetup {
        meshes,
        materials,
        scene: SceneSubmission {
            camera_world_pose: transform(Vec3::new(0.0, 0.15, -3.1), Vec3::ONE, Quat::IDENTITY),
            renderables: vec![SceneRenderable {
                transform_id: 0,
                pose: transform(Vec3::ZERO, Vec3::splat(0.78), Quat::IDENTITY),
                mesh_asset_id: scene_mesh_id(0),
                material_asset_id: scene_material_id(0),
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::Off,
            }],
            lights,
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::scene_setup;

    #[test]
    fn scene_pairs_one_positive_light_with_one_negative_light() {
        let scene = scene_setup(true);
        assert_eq!(scene.scene.lights.len(), 2);
        assert!(scene.scene.lights[0].state.intensity > 0.0);
        assert!(scene.scene.lights[1].state.intensity < 0.0);
        assert_eq!(scene.scene.renderables.len(), 1);
    }

    #[test]
    fn scene_without_the_negative_light_keeps_only_the_positive_one() {
        let scene = scene_setup(false);
        assert_eq!(scene.scene.lights.len(), 1);
        assert!(scene.scene.lights[0].state.intensity > 0.0);
    }
}
