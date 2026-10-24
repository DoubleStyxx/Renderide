//! Drives one frame-benchmark scenario through the existing headless host harness.
//!
//! The renderer records samples itself (`--frame-bench-out`). This module uploads a fixed scene,
//! applies one camera pose, and pumps lockstep until that report is complete. Scripted camera
//! paths stay a caller concern: the pose fields are the in-process stand-in for `camera.set_pose`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use glam::{Quat, Vec3};
use renderide_shared::shared::{QualityConfig, RendererCommand};

use crate::error::HarnessError;
use crate::scene::textures::{checker_rgba, uv_ramp_rgba};

use super::super::asset_upload::{DEFAULT_ASSET_UPLOAD_TIMEOUT, create_render_texture};
use super::super::handshake::{DEFAULT_HANDSHAKE_TIMEOUT, run_handshake};
use super::super::ipc_setup::{DEFAULT_QUEUE_CAPACITY_BYTES, connect_session};
use super::super::lockstep::{FrameSubmitScalars, LockstepDriver};
use super::bench_scenes::{
    SECONDARY_RT_SIZE, secondary_camera_copy_rig, secondary_camera_double_buffer_rig,
};
use super::consts::timing;
use super::scene_state::{build_scene_state_with_cameras, ensure_scene_submitted};
use super::shutdown::request_shutdown_and_wait;
use super::spawn::SpawnedRenderer;
use super::{
    SessionTemplate, build_scene_setup, transform, upload_scene_materials, upload_scene_meshes,
};

/// Scenario that adds two render-texture secondary cameras to the `unlit_sphere` scene.
const SECONDARY_CAMERA_COPY: &str = "secondary_camera_copy";
/// Two stacked double-buffered cameras (Depth, then Nothing) writing one 512² texture.
const SECONDARY_CAMERA_DOUBLE_BUFFER: &str = "secondary_camera_double_buffer";

/// One scenario invocation. The caller owns warmup, measured-frame, and pose choices.
#[derive(Clone, Debug)]
pub struct BenchDriveConfig {
    /// Path to the `renderide-renderer` binary.
    pub renderer_path: PathBuf,
    /// `unlit_sphere`, `multi_primitive_unlit`, `pbs_lit_material_matrix`, `many_static_meshes`,
    /// `lights_shadows`, `many_shadowed_point_lights`, `offscreen_skinned_avatars`,
    /// `onscreen_skinned_avatars`, `secondary_camera_copy`, or `secondary_camera_double_buffer`.
    pub scenario: String,
    /// Raw per-frame JSON path the renderer writes when the measured window completes.
    pub report_path: PathBuf,
    /// Headless render width. Ignored for windowed runs.
    pub width: u32,
    /// Headless render height. Ignored for windowed runs.
    pub height: u32,
    /// Frames recorded then discarded before the measured window.
    pub warmup_frames: u32,
    /// Frames stored in the raw report.
    pub measured_frames: u32,
    /// Deadline for the report after the scene submit, not including handshake.
    pub timeout: Duration,
    /// Inherit the renderer stdout and stderr when set.
    pub verbose_renderer: bool,
    /// Skip `--headless`. The winit window size is not pinned.
    pub windowed: bool,
    /// World-space camera position applied before the first submit.
    pub camera_position: [f32; 3],
    /// Camera rotation quaternion, xyzw.
    pub camera_rotation_xyzw: [f32; 4],
}

/// Spawns the renderer, uploads `cfg.scenario`, and waits for a complete raw report.
///
/// # Errors
///
/// Returns [`HarnessError::UnknownBenchScenario`] for an unknown name, and the usual harness
/// errors when the renderer binary, handshake, upload, or report wait fails.
pub fn drive_bench(cfg: &BenchDriveConfig) -> Result<(), HarnessError> {
    let template = scenario_template(&cfg.scenario)?;
    if !cfg.renderer_path.exists() {
        return Err(HarnessError::RendererBinaryMissing(
            cfg.renderer_path.clone(),
        ));
    }

    let mut session = connect_session(DEFAULT_QUEUE_CAPACITY_BYTES)?;
    let prefix = session.shared_memory_prefix.clone();
    let backing_dir = session.tempdir_guard.path().to_path_buf();
    let png_dir = tempfile::Builder::new()
        .prefix("renderide-framebench-png-")
        .tempdir()?;
    let png_path = png_dir.path().join("unused.png");

    let mut spawned = spawn_bench_renderer(
        cfg,
        &session.connection_params.queue_name,
        &backing_dir,
        &png_path,
    )?;
    let mut lockstep = LockstepDriver::new(FrameSubmitScalars::default());
    run_handshake(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        DEFAULT_HANDSHAKE_TIMEOUT,
    )?;
    publish_many_light_shadow_quality(&mut session.queues, &cfg.scenario)?;

    let mut setup = build_scene_setup(template);
    apply_camera(&mut setup, cfg);
    let _meshes = upload_scene_meshes(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        &backing_dir,
        &setup,
    )?;
    let _materials = upload_scene_materials(
        &mut session.queues,
        &mut lockstep,
        &prefix,
        &backing_dir,
        &setup,
    )?;
    let rig = if cfg.scenario == SECONDARY_CAMERA_COPY {
        Some(secondary_camera_copy_rig(&setup.scene))
    } else if cfg.scenario == SECONDARY_CAMERA_DOUBLE_BUFFER {
        Some(secondary_camera_double_buffer_rig(&setup.scene))
    } else {
        None
    };
    let cameras = if let Some(rig) = rig {
        for asset_id in &rig.render_textures {
            create_render_texture(
                &mut session.queues,
                &mut lockstep,
                *asset_id,
                SECONDARY_RT_SIZE,
                DEFAULT_ASSET_UPLOAD_TIMEOUT,
            )?;
        }
        rig.cameras
    } else {
        Vec::new()
    };
    let scene = build_scene_state_with_cameras(
        &prefix,
        &backing_dir,
        &setup.scene,
        &cameras,
        &mut lockstep,
    )?;
    let _submit = ensure_scene_submitted(&mut session.queues, &mut lockstep, cfg.timeout)?;

    let child = spawned
        .child
        .as_mut()
        .ok_or_else(|| HarnessError::BenchReportMissing {
            path: cfg.report_path.clone(),
            wait: Duration::ZERO,
            renderer_state: "spawn did not keep a child".to_string(),
        })?;
    wait_for_complete_report(
        &mut session.queues,
        &mut lockstep,
        &cfg.report_path,
        cfg.timeout,
        child,
    )?;
    drop(scene);
    request_shutdown_and_wait(&mut session.queues, &mut spawned)?;
    Ok(())
}

fn scenario_template(name: &str) -> Result<SessionTemplate, HarnessError> {
    match name {
        "unlit_sphere" | SECONDARY_CAMERA_COPY | SECONDARY_CAMERA_DOUBLE_BUFFER => {
            Ok(SessionTemplate::Sphere)
        }
        "pbs_lit_material_matrix" => Ok(SessionTemplate::PbsLitMaterialMatrix),
        "many_static_meshes" => Ok(SessionTemplate::ManyStaticMeshes),
        "lights_shadows" => Ok(SessionTemplate::LightsShadows),
        "multi_primitive_unlit" => {
            let checker = checker_rgba(256, 256, 8, [245, 245, 245, 255], [24, 34, 54, 255]);
            let uv_ramp = uv_ramp_rgba(256, 256);
            Ok(SessionTemplate::MultiPrimitiveUnlitGrid {
                checker_rgba: checker.into_raw(),
                uv_ramp_rgba: uv_ramp.into_raw(),
                texture_size: (256, 256),
            })
        }
        "many_shadowed_point_lights" => Ok(SessionTemplate::ManyShadowedPointLights),
        "offscreen_skinned_avatars" => Ok(SessionTemplate::SkinnedAvatars { offscreen: true }),
        "onscreen_skinned_avatars" => Ok(SessionTemplate::SkinnedAvatars { offscreen: false }),
        other => Err(HarnessError::UnknownBenchScenario(other.to_string())),
    }
}

fn publish_many_light_shadow_quality(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    scenario: &str,
) -> Result<(), HarnessError> {
    if !matches!(
        scenario,
        "many_shadowed_point_lights" | "offscreen_skinned_avatars" | "onscreen_skinned_avatars"
    ) {
        return Ok(());
    }
    let sent = queues.send_primary(RendererCommand::QualityConfig(QualityConfig {
        per_pixel_lights: 32,
        shadow_distance: 30.0,
        ..QualityConfig::default()
    }));
    if sent {
        return Ok(());
    }
    Err(HarnessError::QueueOptions(
        "send_primary(QualityConfig) returned false (queue full?)".to_string(),
    ))
}

fn apply_camera(setup: &mut super::SceneSetup, cfg: &BenchDriveConfig) {
    let [x, y, z, w] = cfg.camera_rotation_xyzw;
    setup.scene.camera_world_pose = transform(
        Vec3::from_array(cfg.camera_position),
        Vec3::ONE,
        Quat::from_xyzw(x, y, z, w),
    );
}

fn spawn_bench_renderer(
    cfg: &BenchDriveConfig,
    queue_name: &str,
    backing_dir: &Path,
    png_path: &Path,
) -> Result<SpawnedRenderer, HarnessError> {
    let args = bench_renderer_args(cfg, queue_name, png_path);
    let mut cmd = Command::new(&cfg.renderer_path);
    cmd.args(&args);
    cmd.env("RENDERIDE_INTERPROCESS_DIR", backing_dir);
    if cfg.verbose_renderer {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.env("RENDERIDE_LOG_TEE_TERMINAL", "0");
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    logger::info!(
        "Spawning frame bench renderer: {} {}",
        cfg.renderer_path.display(),
        args.join(" ")
    );
    let child = cmd.spawn().map_err(HarnessError::SpawnRenderer)?;
    Ok(SpawnedRenderer { child: Some(child) })
}

pub(super) fn bench_renderer_args(
    cfg: &BenchDriveConfig,
    queue_name: &str,
    png_path: &Path,
) -> Vec<String> {
    let mut args = Vec::new();
    if !cfg.windowed {
        args.extend([
            "--headless".to_string(),
            "--headless-output".to_string(),
            png_path.display().to_string(),
            "--headless-resolution".to_string(),
            format!("{}x{}", cfg.width, cfg.height),
            "--headless-interval-ms".to_string(),
            "1".to_string(),
        ]);
    }
    args.extend([
        "-QueueName".to_string(),
        queue_name.to_string(),
        "-QueueCapacity".to_string(),
        DEFAULT_QUEUE_CAPACITY_BYTES.to_string(),
        "-LogLevel".to_string(),
        "info".to_string(),
        "--ignore-config".to_string(),
        "--frame-bench-out".to_string(),
        cfg.report_path.display().to_string(),
        "--frame-bench-warmup".to_string(),
        cfg.warmup_frames.to_string(),
        "--frame-bench-frames".to_string(),
        cfg.measured_frames.to_string(),
    ]);
    args
}

fn wait_for_complete_report(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    report_path: &Path,
    timeout: Duration,
    renderer: &mut Child,
) -> Result<(), HarnessError> {
    let started = Instant::now();
    let deadline = started + timeout;
    let mut last_log = Instant::now();
    loop {
        let _ = lockstep.tick(queues);
        if report_is_complete(report_path) {
            logger::info!(
                "frame bench report complete at {} after {:?}",
                report_path.display(),
                started.elapsed()
            );
            return Ok(());
        }
        if let Ok(Some(status)) = renderer.try_wait() {
            return Err(HarnessError::BenchReportMissing {
                path: report_path.to_path_buf(),
                wait: started.elapsed(),
                renderer_state: format!("exited {status}"),
            });
        }
        if Instant::now() >= deadline {
            return Err(HarnessError::BenchReportMissing {
                path: report_path.to_path_buf(),
                wait: timeout,
                renderer_state: "running".to_string(),
            });
        }
        if last_log.elapsed() >= timing::LOG_INTERVAL {
            logger::info!(
                "frame bench still waiting for {} (elapsed={:?})",
                report_path.display(),
                started.elapsed()
            );
            last_log = Instant::now();
        }
        std::thread::sleep(timing::POLL_INTERVAL);
    }
}

fn report_is_complete(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    value.get("complete").and_then(serde_json::Value::as_bool) == Some(true)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{
        BenchDriveConfig, SECONDARY_CAMERA_COPY, SECONDARY_CAMERA_DOUBLE_BUFFER, SessionTemplate,
        bench_renderer_args, build_scene_setup, drive_bench, scenario_template,
        secondary_camera_copy_rig, secondary_camera_double_buffer_rig,
    };

    fn cfg(scenario: &str, windowed: bool) -> BenchDriveConfig {
        BenchDriveConfig {
            renderer_path: PathBuf::from("missing-renderer"),
            scenario: scenario.to_string(),
            report_path: PathBuf::from("out/raw.json"),
            width: 256,
            height: 128,
            warmup_frames: 2,
            measured_frames: 4,
            timeout: Duration::from_secs(5),
            verbose_renderer: false,
            windowed,
            camera_position: [0.0, 0.0, -3.0],
            camera_rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn secondary_camera_copy_adds_depth_and_nothing_cameras_to_the_sphere_scene() {
        assert!(matches!(
            scenario_template(SECONDARY_CAMERA_COPY),
            Ok(SessionTemplate::Sphere)
        ));
        let setup = build_scene_setup(SessionTemplate::Sphere);
        let rig = secondary_camera_copy_rig(&setup.scene);
        assert_eq!(rig.render_textures.len(), 2);
        let clears: Vec<_> = rig.cameras.iter().map(|c| c.clear_mode).collect();
        assert_eq!(
            clears,
            [
                renderide_shared::shared::CameraClearMode::Depth,
                renderide_shared::shared::CameraClearMode::Nothing
            ]
        );
        for (camera, rt) in rig.cameras.iter().zip(&rig.render_textures) {
            assert_eq!(camera.render_texture_asset_id, *rt);
            assert_eq!(camera.flags, 1);
            assert!(
                setup
                    .scene
                    .renderables
                    .iter()
                    .all(|r| r.transform_id != camera.transform_id)
            );
        }
    }

    #[test]
    fn secondary_camera_double_buffer_stacks_depth_then_nothing_on_one_texture() {
        assert!(matches!(
            scenario_template(SECONDARY_CAMERA_DOUBLE_BUFFER),
            Ok(SessionTemplate::Sphere)
        ));
        let setup = build_scene_setup(SessionTemplate::Sphere);
        let rig = secondary_camera_double_buffer_rig(&setup.scene);
        assert_eq!(rig.render_textures, vec![900]);
        assert_eq!(rig.cameras.len(), 2);
        assert_eq!(
            rig.cameras[0].clear_mode,
            renderide_shared::shared::CameraClearMode::Depth
        );
        assert_eq!(
            rig.cameras[1].clear_mode,
            renderide_shared::shared::CameraClearMode::Nothing
        );
        assert_eq!(rig.cameras[0].depth, 0.0);
        assert_eq!(rig.cameras[1].depth, 1.0);
        for camera in &rig.cameras {
            assert_eq!(camera.render_texture_asset_id, 900);
            assert_eq!(camera.flags, 1 | (1 << 2));
            assert!(camera.selective_transform_ids.is_empty());
        }
    }

    #[test]
    fn unknown_scenario_is_rejected_before_spawn() {
        let err = drive_bench(&cfg("nope", false)).expect_err("unknown");
        let message = err.to_string();
        assert!(message.contains("nope"), "{message}");
    }

    #[test]
    fn headless_args_pin_resolution_and_frame_bench_flags() {
        let args = bench_renderer_args(
            &cfg("unlit_sphere", false),
            "queue-a",
            PathBuf::from("p.png").as_path(),
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--headless-resolution", "256x128"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--headless-interval-ms", "1"])
        );
        assert!(args.windows(2).any(|pair| pair == ["-LogLevel", "info"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--frame-bench-out", "out/raw.json"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--frame-bench-warmup", "2"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--frame-bench-frames", "4"])
        );
        assert!(args.iter().any(|arg| arg == "--ignore-config"));
    }

    #[test]
    fn windowed_args_omit_headless_flags() {
        let args = bench_renderer_args(
            &cfg("unlit_sphere", true),
            "queue-a",
            PathBuf::from("p.png").as_path(),
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg == "--headless" || arg.starts_with("--headless-"))
        );
        assert!(args.iter().any(|arg| arg == "--frame-bench-out"));
    }
}
