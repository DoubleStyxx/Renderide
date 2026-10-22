//! Shadowed PBS captures.
//!
//! `ShadowedCasterMove` records a static hard-shadow frame, requires the next static frame to
//! match it pixel for pixel, then moves the cube. The pixel check is the atlas-skip evidence:
//! after the first stable frame the planner reuses the shadow map, and a deterministic renderer
//! must keep writing the same image. `ManyShadowedPointLights` is the opt-in frame bench for the
//! many-local-light case and is not a golden.

use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant, SystemTime};

use glam::{Quat, Vec3};
use renderide_shared::shared::ShadowCastMode;

use crate::error::HarnessError;
use crate::scene::primitives::{centered_bounds, generate_cube, generate_quad};

use super::super::lockstep::LockstepDriver;
use super::png_readback::{PngStabilityWaitTiming, run_lockstep_until_png_stable};
use super::scene_state::{
    SceneLight, SceneState, directional_light, shadowed_point_light, shadowed_spot_light,
};
use super::{
    SceneRenderable, SceneSetup, SceneSubmission, mesh_asset, pbs_material_for_scene,
    scene_mesh_id, transform,
};

const SPOT_RESOLUTION: i32 = 256;
const POINT_RESOLUTION: i32 = 128;
const POINT_LIGHT_COUNT: i32 = 30;
const CUBE_INDEX: usize = 1;
/// How many newer static frames to sample before giving up on a pixel match.
///
/// The first stable PNG can land while PBS and the shadow atlas are still compiling. Later
/// writes then differ. Two consecutive identical frames are the cache-hit evidence.
const STATIC_MATCH_ATTEMPTS: u32 = 8;

/// Inputs for the static-identity check and the caster move.
pub(super) struct ShadowMoveRecapture<'a> {
    /// Host command queues.
    pub queues: &'a mut renderide_shared::ipc::HostDualQueueIpc,
    /// Lockstep driver kept pumping while the renderer writes PNGs.
    pub lockstep: &'a mut LockstepDriver,
    /// Live scene shared memory. The rewrite sends a pose-only delta.
    pub scene: &'a mut SceneState,
    /// Static scene. The cube is renderable index 1.
    pub setup: &'a SceneSetup,
    /// Renderer PNG path. The static copy is `plain.png` beside it.
    pub output_path: &'a Path,
    /// Deadline for each later stable PNG.
    pub timeout: Duration,
    /// Renderer PNG interval, used as the stability floor.
    pub interval: Duration,
    /// Live renderer process.
    pub renderer: &'a mut Child,
}

/// Floor, cube, hard spot, and an unshadowed directional fill.
pub(super) fn scene_setup() -> SceneSetup {
    let mut scene = floor_and_cube_scene(vec![SceneRenderable {
        transform_id: 1,
        pose: transform(Vec3::new(0.35, 0.35, 0.1), Vec3::splat(0.7), Quat::IDENTITY),
        mesh_asset_id: scene_mesh_id(1),
        material_asset_id: super::scene_material_id(1),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::On,
    }]);
    let aim = (Vec3::new(0.35, 0.35, 0.1) - Vec3::new(-1.2, 1.9, 0.2)).normalize();
    scene.lights = vec![
        SceneLight {
            transform_id: 2,
            pose: transform(
                Vec3::new(-1.2, 1.9, 0.2),
                Vec3::ONE,
                Quat::from_rotation_arc(Vec3::Z, aim),
            ),
            state: shadowed_spot_light([1.0, 0.92, 0.78], 10.0, 6.0, 60.0, SPOT_RESOLUTION),
        },
        SceneLight {
            transform_id: 3,
            pose: transform(
                Vec3::new(0.4, 2.4, -1.2),
                Vec3::ONE,
                Quat::from_rotation_arc(Vec3::Z, Vec3::new(0.15, -1.0, 0.35).normalize()),
            ),
            state: directional_light([0.75, 0.82, 1.0], 0.55),
        },
    ];
    scene.camera_world_pose = transform(
        Vec3::new(0.2, 1.2, -3.2),
        Vec3::ONE,
        Quat::from_rotation_x(0.22),
    );
    SceneSetup {
        meshes: caster_meshes(),
        materials: caster_materials(),
        scene,
    }
}

/// Eight casting cubes under thirty hard point lights. Shadow tiles stay at 128 so lavapipe can
/// finish a short measured window.
pub(super) fn many_point_lights_setup() -> SceneSetup {
    let mut renderables = vec![floor_renderable()];
    let xs = [-1.5, -0.5, 0.5, 1.5];
    let zs = [-0.4, 0.4];
    let mut transform_id = 1i32;
    for z in zs {
        for x in xs {
            renderables.push(SceneRenderable {
                transform_id,
                pose: transform(Vec3::new(x, 0.28, z), Vec3::splat(0.5), Quat::IDENTITY),
                mesh_asset_id: scene_mesh_id(1),
                material_asset_id: super::scene_material_id(1),
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::On,
            });
            transform_id += 1;
        }
    }
    let mut lights = Vec::with_capacity(POINT_LIGHT_COUNT as usize);
    let mut light_id = 20i32;
    for y in [1.6_f32, 2.6] {
        for z in [-1.0_f32, 0.0, 1.0] {
            for x in [-2.0_f32, -1.0, 0.0, 1.0, 2.0] {
                lights.push(SceneLight {
                    transform_id: light_id,
                    pose: transform(Vec3::new(x, y, z), Vec3::ONE, Quat::IDENTITY),
                    state: shadowed_point_light([1.0, 0.94, 0.82], 4.0, 6.0, POINT_RESOLUTION),
                });
                light_id += 1;
            }
        }
    }
    debug_assert_eq!(lights.len(), POINT_LIGHT_COUNT as usize);
    SceneSetup {
        meshes: caster_meshes(),
        materials: caster_materials(),
        scene: SceneSubmission {
            camera_world_pose: transform(
                Vec3::new(0.0, 2.2, -5.5),
                Vec3::ONE,
                Quat::from_rotation_x(0.35),
            ),
            renderables,
            lights,
        },
    }
}

/// Copies the static PNG, requires the next static frame to match it, then moves the cube.
pub(super) fn recapture_after_move(
    mut recapture: ShadowMoveRecapture<'_>,
) -> Result<(), HarnessError> {
    let plain_path = recapture.output_path.with_file_name("plain.png");
    std::fs::copy(recapture.output_path, &plain_path)?;
    logger::info!(
        "Session: copied static shadow PNG to {}",
        plain_path.display()
    );
    wait_until_static_frames_match(&mut recapture, &plain_path)?;

    let mut moved = recapture.setup.scene.clone();
    moved.renderables[CUBE_INDEX].pose = transform(
        Vec3::new(-0.55, 0.35, 0.55),
        Vec3::splat(0.7),
        Quat::IDENTITY,
    );
    recapture.scene.rewrite(&moved, recapture.lockstep)?;
    wait_for_fresh_png(&mut recapture)?;
    assert_pixels_changed(&plain_path, recapture.output_path)?;
    Ok(())
}

fn floor_and_cube_scene(cube: Vec<SceneRenderable>) -> SceneSubmission {
    let mut renderables = vec![floor_renderable()];
    renderables.extend(cube);
    SceneSubmission {
        camera_world_pose: super::default_camera_world_pose(),
        renderables,
        lights: Vec::new(),
    }
}

fn floor_renderable() -> SceneRenderable {
    SceneRenderable {
        transform_id: 0,
        pose: transform(
            Vec3::ZERO,
            Vec3::new(3.4, 3.4, 1.0),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        mesh_asset_id: scene_mesh_id(0),
        material_asset_id: super::scene_material_id(0),
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::Off,
    }
}

fn caster_meshes() -> Vec<super::MeshAsset> {
    vec![
        mesh_asset(
            0,
            generate_quad(),
            centered_bounds(Vec3::new(0.55, 0.55, 0.05)),
        ),
        mesh_asset(1, generate_cube(), centered_bounds(Vec3::splat(0.55))),
    ]
}

fn caster_materials() -> Vec<super::MaterialAsset> {
    vec![
        pbs_material_for_scene(0, [0.72, 0.7, 0.66, 1.0], 0.0, 0.22),
        pbs_material_for_scene(1, [0.86, 0.24, 0.16, 1.0], 0.0, 0.45),
    ]
}

fn wait_for_fresh_png(recapture: &mut ShadowMoveRecapture<'_>) -> Result<(), HarnessError> {
    run_lockstep_until_png_stable(
        recapture.queues,
        recapture.lockstep,
        recapture.output_path,
        PngStabilityWaitTiming {
            scene_submitted_at: SystemTime::now(),
            scene_submit_instant: Instant::now(),
            overall_timeout: recapture.timeout,
            interval: recapture.interval,
        },
        recapture.renderer,
    )?;
    Ok(())
}

fn wait_until_static_frames_match(
    recapture: &mut ShadowMoveRecapture<'_>,
    plain_path: &Path,
) -> Result<(), HarnessError> {
    let mut last_diffs = 0usize;
    for attempt in 1..=STATIC_MATCH_ATTEMPTS {
        wait_for_fresh_png(recapture)?;
        last_diffs = pixel_diff_count(plain_path, recapture.output_path)?;
        if last_diffs == 0 {
            let settled = crate::image_io::load_rgba(plain_path)?;
            crate::golden::reject_flat_image(&settled, plain_path)?;
            logger::info!(
                "Session: static shadow frames matched on attempt {attempt} ({})",
                plain_path.display()
            );
            return Ok(());
        }
        logger::info!(
            "Session: static shadow frame {attempt} differed in {last_diffs} pixels; waiting for the atlas to settle"
        );
        std::fs::copy(recapture.output_path, plain_path)?;
    }
    Err(HarnessError::ImageCompare(format!(
        "static shadow recapture still differed in {last_diffs} pixels after {STATIC_MATCH_ATTEMPTS} frames; a cache-hit frame must stay pixel-identical"
    )))
}

fn pixel_diff_count(before: &Path, after: &Path) -> Result<usize, HarnessError> {
    let left = crate::image_io::load_rgba(before)?;
    let right = crate::image_io::load_rgba(after)?;
    if left.dimensions() != right.dimensions() {
        return Err(HarnessError::ImageCompare(format!(
            "static shadow frames have different dimensions {:?} vs {:?}",
            left.dimensions(),
            right.dimensions()
        )));
    }
    Ok(left
        .pixels()
        .zip(right.pixels())
        .filter(|(a, b)| a != b)
        .count())
}

fn assert_pixels_changed(before: &Path, after: &Path) -> Result<(), HarnessError> {
    let left = crate::image_io::load_rgba(before)?;
    let right = crate::image_io::load_rgba(after)?;
    crate::golden::reject_flat_image(&right, after)?;
    if left.as_raw() != right.as_raw() {
        return Ok(());
    }
    Err(HarnessError::ImageCompare(
        "moving the shadow caster left the capture unchanged".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_cube_keeps_the_same_renderable_and_light_counts() {
        let setup = scene_setup();
        let mut moved = setup.scene.clone();
        moved.renderables[CUBE_INDEX].pose.position.x = -0.55;
        assert_eq!(setup.scene.renderables.len(), moved.renderables.len());
        assert_eq!(setup.scene.lights.len(), moved.lights.len());
        assert_ne!(
            setup.scene.renderables[CUBE_INDEX].pose.position.x,
            moved.renderables[CUBE_INDEX].pose.position.x
        );
        assert_eq!(
            setup.scene.lights[0].state.shadow_map_resolution_override,
            SPOT_RESOLUTION
        );
    }

    #[test]
    fn many_point_light_scene_has_thirty_shadowed_lights() {
        let setup = many_point_lights_setup();
        assert_eq!(setup.scene.lights.len(), 30);
        assert!(setup.scene.lights.iter().all(|light| {
            light.state.shadow_strength > 0.0 && light.state.shadow_map_resolution_override == 128
        }));
        assert!(
            setup
                .scene
                .renderables
                .iter()
                .skip(1)
                .all(|renderable| renderable.shadow_cast_mode == ShadowCastMode::On)
        );
    }
}
