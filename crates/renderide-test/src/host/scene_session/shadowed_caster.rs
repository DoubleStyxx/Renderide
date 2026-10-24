//! Shadowed PBS captures.
//!
//! `ShadowedCasterMove` records a static hard-shadow frame, requires the next static frame to
//! match it pixel for pixel with zero atlas passes, then moves the cube. The pass log must show
//! one atlas submit for that move. `ManyShadowedPointLights` is the opt-in frame bench for the
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
/// Spot intensity kept below the blow-out that painted the upper frame white.
const SPOT_INTENSITY: f32 = 2.4;
/// Unshadowed fill. High enough to lift the floor, low enough that the spot shadow stays visible.
const FILL_INTENSITY: f32 = 0.22;
/// Floor extent in world units. The camera sits inside this quad so the foreground is not clear-black.
const FLOOR_EXTENT: f32 = 18.0;
const POINT_RESOLUTION: i32 = 128;
const POINT_LIGHT_COUNT: i32 = 30;
const CUBE_INDEX: usize = 1;
/// How many newer static frames to sample before giving up on a pixel match.
///
/// The first stable PNG can land while PBS and the shadow atlas are still compiling. Later
/// writes then differ. Two consecutive identical frames are the cache-hit evidence.
const STATIC_MATCH_ATTEMPTS: u32 = 8;
/// Lavapipe's settled shadow frame has several hundred colors. Two identical pre-light slabs
/// (DX12 WARP wrote a 48-color gray band) must not count as the cache hit.
const MIN_SETTLED_UNIQUE_COLORS: usize = 128;

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
            state: shadowed_spot_light(
                [1.0, 0.92, 0.78],
                SPOT_INTENSITY,
                6.0,
                60.0,
                SPOT_RESOLUTION,
            ),
        },
        SceneLight {
            transform_id: 3,
            pose: transform(
                Vec3::new(0.4, 2.4, -1.2),
                Vec3::ONE,
                Quat::from_rotation_arc(Vec3::Z, Vec3::new(0.15, -1.0, 0.35).normalize()),
            ),
            state: directional_light([0.75, 0.82, 1.0], FILL_INTENSITY),
        },
    ];
    scene.camera_world_pose = transform(
        Vec3::new(0.15, 1.55, -2.4),
        Vec3::ONE,
        Quat::from_rotation_x(0.62),
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
            cameras: Vec::new(),
            skinned: Vec::new(),
        },
    }
}

/// Copies the static PNG, requires the next static frame to match it, then moves the cube.
pub(super) fn recapture_after_move(
    mut recapture: ShadowMoveRecapture<'_>,
) -> Result<(), HarnessError> {
    let plain_path = recapture.output_path.with_file_name("plain.png");
    std::fs::copy(recapture.output_path, &plain_path)?;
    copy_atlas_pass_sidecar(recapture.output_path, &plain_path)?;
    logger::info!(
        "Session: copied static shadow PNG to {}",
        plain_path.display()
    );
    let static_total = wait_until_static_frames_match(&mut recapture, &plain_path)?;
    let log_offset = atlas_pass_log_len(recapture.output_path)?;

    let mut moved = recapture.setup.scene.clone();
    moved.renderables[CUBE_INDEX].pose = transform(
        Vec3::new(-0.55, 0.35, 0.55),
        Vec3::splat(0.7),
        Quat::IDENTITY,
    );
    recapture.scene.rewrite(&moved, recapture.lockstep)?;
    wait_for_fresh_png(&mut recapture)?;
    assert_pixels_changed(&plain_path, recapture.output_path)?;
    assert_move_recorded_one_atlas_pass(recapture.output_path, static_total, log_offset)?;
    Ok(())
}

fn floor_and_cube_scene(cube: Vec<SceneRenderable>) -> SceneSubmission {
    let mut renderables = vec![floor_renderable()];
    renderables.extend(cube);
    SceneSubmission {
        camera_world_pose: super::default_camera_world_pose(),
        renderables,
        lights: Vec::new(),
        cameras: Vec::new(),
        skinned: Vec::new(),
    }
}

fn floor_renderable() -> SceneRenderable {
    SceneRenderable {
        transform_id: 0,
        pose: transform(
            Vec3::ZERO,
            Vec3::new(FLOOR_EXTENT, FLOOR_EXTENT, 1.0),
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
) -> Result<u64, HarnessError> {
    let mut last_diffs = 0usize;
    let mut last_colors = 0usize;
    let mut last_passes = String::from("missing");
    for attempt in 1..=STATIC_MATCH_ATTEMPTS {
        wait_for_fresh_png(recapture)?;
        last_diffs = pixel_diff_count(plain_path, recapture.output_path)?;
        if last_diffs == 0 {
            let settled = crate::image_io::load_rgba(plain_path)?;
            crate::golden::reject_flat_image(&settled, plain_path)?;
            last_colors = unique_color_count(&settled);
            match read_atlas_pass_sidecar(recapture.output_path) {
                Ok(sample)
                    if last_colors >= MIN_SETTLED_UNIQUE_COLORS && sample.this_frame == 0 =>
                {
                    copy_atlas_pass_sidecar(recapture.output_path, plain_path)?;
                    logger::info!(
                        "Session: static shadow frames matched on attempt {attempt} ({last_colors} colors, 0 atlas passes, total {}, {})",
                        sample.total,
                        plain_path.display()
                    );
                    return Ok(sample.total);
                }
                Ok(sample) => {
                    last_passes =
                        format!("this_frame {} total {}", sample.this_frame, sample.total);
                    logger::info!(
                        "Session: static shadow frame {attempt} matched in {last_colors} colors with {last_passes}; waiting for a skipped atlas"
                    );
                }
                Err(err) => {
                    last_passes = err.to_string();
                    logger::info!(
                        "Session: static shadow frame {attempt} matched pixels but the atlas stat was not ready ({last_passes})"
                    );
                }
            }
        } else {
            logger::info!(
                "Session: static shadow frame {attempt} differed in {last_diffs} pixels; waiting for the atlas to settle"
            );
        }
        std::fs::copy(recapture.output_path, plain_path)?;
        copy_atlas_pass_sidecar(recapture.output_path, plain_path)?;
    }
    Err(HarnessError::ImageCompare(format!(
        "static shadow recapture was not a lit cache hit after {STATIC_MATCH_ATTEMPTS} frames (last pixel diff {last_diffs}, unique colors {last_colors}, atlas passes {last_passes}; need {MIN_SETTLED_UNIQUE_COLORS} colors and 0 atlas passes)"
    )))
}

struct AtlasPassSample {
    this_frame: u32,
    total: u64,
}

fn atlas_pass_sidecar(png: &Path) -> std::path::PathBuf {
    png.with_extension("atlas-passes")
}

fn atlas_pass_log(png: &Path) -> std::path::PathBuf {
    png.with_extension("atlas-pass-log")
}

fn read_atlas_pass_sidecar(png: &Path) -> Result<AtlasPassSample, HarnessError> {
    let sidecar = atlas_pass_sidecar(png);
    let png_mtime = std::fs::metadata(png)?.modified()?;
    let side_mtime = std::fs::metadata(&sidecar)?.modified()?;
    // A sidecar written for the next frame, before that frame's PNG rename, is newer than
    // the PNG still on disk. A finished pair has equal mtimes: the renderer stamps the sidecar
    // with the PNG mtime after rename, because the temp PNG is older than the sidecar write.
    if side_mtime > png_mtime {
        return Err(HarnessError::ImageCompare(
            "shadow atlas pass sidecar is newer than the PNG".to_string(),
        ));
    }
    let text = std::fs::read_to_string(&sidecar)?;
    parse_atlas_pass_line(text.trim()).ok_or_else(|| {
        HarnessError::ImageCompare(format!(
            "unreadable shadow atlas pass sidecar {}",
            sidecar.display()
        ))
    })
}

fn atlas_pass_log_len(png: &Path) -> Result<u64, HarnessError> {
    match std::fs::metadata(atlas_pass_log(png)) {
        Ok(meta) => Ok(meta.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
}

fn read_atlas_pass_log_since(
    png: &Path,
    offset: u64,
) -> Result<Vec<AtlasPassSample>, HarnessError> {
    let bytes = std::fs::read(atlas_pass_log(png))?;
    let start = usize::try_from(offset).unwrap_or(0).min(bytes.len());
    let text = String::from_utf8_lossy(&bytes[start..]);
    let ends_with_newline = text.ends_with('\n');
    let raw_lines: Vec<&str> = text.lines().collect();
    let mut samples = Vec::new();
    for (index, line) in raw_lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        match parse_atlas_pass_line(line) {
            Some(sample) => samples.push(sample),
            None if !ends_with_newline && index + 1 == raw_lines.len() => {}
            None => {
                return Err(HarnessError::ImageCompare(format!(
                    "unreadable shadow atlas pass log line {line:?}"
                )));
            }
        }
    }
    Ok(samples)
}

fn parse_atlas_pass_line(line: &str) -> Option<AtlasPassSample> {
    let mut parts = line.split_whitespace();
    let this_frame = parts.next()?.parse().ok()?;
    let total = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(AtlasPassSample { this_frame, total })
}

fn copy_atlas_pass_sidecar(from_png: &Path, to_png: &Path) -> Result<(), HarnessError> {
    let from = atlas_pass_sidecar(from_png);
    if from.is_file() {
        std::fs::copy(from, atlas_pass_sidecar(to_png))?;
    }
    Ok(())
}

fn assert_move_recorded_one_atlas_pass(
    png: &Path,
    static_total: u64,
    log_offset: u64,
) -> Result<(), HarnessError> {
    let samples = read_atlas_pass_log_since(png, log_offset)?;
    let renders = samples
        .iter()
        .filter(|sample| sample.this_frame == 1)
        .count();
    let total = samples.last().map(|sample| sample.total);
    let expected = static_total.saturating_add(1);
    if renders == 1 && total == Some(expected) {
        logger::info!(
            "Session: caster move submitted one shadow atlas pass (total {static_total} -> {expected})"
        );
        return Ok(());
    }
    Err(HarnessError::ImageCompare(format!(
        "moving the caster should submit one shadow atlas pass (static total {static_total}, renders after the move {renders}, last total {total:?})"
    )))
}

fn unique_color_count(image: &image::RgbaImage) -> usize {
    let mut colors = std::collections::HashSet::new();
    for pixel in image.pixels() {
        colors.insert(u32::from_le_bytes(pixel.0));
    }
    colors.len()
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
    fn atlas_pass_sidecar_matches_the_png_mtime() -> Result<(), HarnessError> {
        let dir = std::env::temp_dir().join(format!("shadow-atlas-mtime-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let png = dir.join("frame.png");
        std::fs::write(&png, b"png")?;
        let sidecar = atlas_pass_sidecar(&png);
        std::fs::write(&sidecar, "0 3\n")?;
        let png_mtime = std::fs::metadata(&png)?.modified()?;
        let earlier = png_mtime
            .checked_sub(Duration::from_secs(2))
            .unwrap_or(png_mtime);
        std::fs::File::options()
            .write(true)
            .open(&sidecar)?
            .set_modified(earlier)?;
        let sample = read_atlas_pass_sidecar(&png)?;
        assert_eq!(sample.this_frame, 0);
        assert_eq!(sample.total, 3);

        let later = png_mtime
            .checked_add(Duration::from_secs(2))
            .unwrap_or(png_mtime);
        std::fs::File::options()
            .write(true)
            .open(&sidecar)?
            .set_modified(later)?;
        match read_atlas_pass_sidecar(&png) {
            Ok(_) => {
                return Err(HarnessError::ImageCompare(
                    "newer sidecar was accepted".to_string(),
                ));
            }
            Err(err) => assert!(err.to_string().contains("newer than the PNG")),
        }
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn atlas_pass_line_parses_two_integers() {
        let sample = parse_atlas_pass_line("0 12");
        assert_eq!(
            sample.map(|sample| (sample.this_frame, sample.total)),
            Some((0, 12))
        );
        assert!(parse_atlas_pass_line("1").is_none());
        assert!(parse_atlas_pass_line("1 2 extra").is_none());
    }

    #[test]
    fn atlas_pass_log_keeps_the_move_after_a_later_cache_hit() -> Result<(), HarnessError> {
        let dir = std::env::temp_dir().join(format!("shadow-atlas-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let png = dir.join("frame.png");
        std::fs::write(&png, b"png")?;
        std::fs::write(atlas_pass_log(&png), "0 4\n1 5\n0 5\n")?;
        let samples = read_atlas_pass_log_since(&png, 4)?;
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].this_frame, 1);
        assert_eq!(samples[0].total, 5);
        assert_eq!(samples[1].this_frame, 0);
        assert_eq!(samples[1].total, 5);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn unique_color_count_rejects_a_two_tone_slab() {
        let mut image = image::RgbaImage::new(4, 4);
        for pixel in image.pixels_mut() {
            *pixel = image::Rgba([44, 44, 44, 255]);
        }
        image.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
        assert!(unique_color_count(&image) < MIN_SETTLED_UNIQUE_COLORS);
    }

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
