//! First PNG capture, plus the property-update and shadow-move recaptures.

use std::time::{Duration, Instant, SystemTime};

use crate::error::HarnessError;

use super::png_readback::{PngStabilityWaitTiming, run_lockstep_until_png_stable};
use super::scene_state::{build_scene_state, ensure_scene_submitted};
use super::{
    LockstepDriver, SceneSessionConfig, SceneSessionOutcome, SceneSetup, custom_property,
    shadowed_caster, spawn,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the capture keeps the same session handles the PNG wait already threads through"
)]
/// Submits the scene, waits for a stable PNG, then runs any extra captures.
pub(super) fn capture_submitted_scene(
    queues: &mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    prefix: &str,
    backing_dir: &std::path::Path,
    setup: &SceneSetup,
    spawned: &mut spawn::SpawnedRenderer,
    cfg: &SceneSessionConfig,
    property_recapture: bool,
    shadow_move: bool,
) -> Result<SceneSessionOutcome, HarnessError> {
    let mut scene = build_scene_state(prefix, backing_dir, &setup.scene, lockstep)?;
    let scene_submit_index = ensure_scene_submitted(queues, lockstep, cfg.timeout)?;
    let scene_submitted_at = SystemTime::now();
    let scene_submit_instant = Instant::now();
    logger::info!(
        "Session: scene submitted at frame_index={scene_submit_index}, mtime_baseline={scene_submitted_at:?}; waiting for fresh PNG"
    );
    #[expect(
        clippy::expect_used,
        reason = "child set immediately above by spawn_renderer"
    )]
    let renderer = spawned.child.as_mut().expect("child set");
    let png_outcome = run_lockstep_until_png_stable(
        queues,
        lockstep,
        &cfg.output_path,
        PngStabilityWaitTiming {
            scene_submitted_at,
            scene_submit_instant,
            overall_timeout: cfg.timeout,
            interval: Duration::from_millis(cfg.interval_ms.max(1)),
        },
        renderer,
    )?;
    finish_extra_captures(
        ExtraCaptures {
            queues,
            lockstep,
            prefix,
            backing_dir,
            setup,
            scene: &mut scene,
            cfg,
            renderer,
        },
        property_recapture,
        shadow_move,
    )?;
    drop(scene);
    Ok(png_outcome)
}

struct ExtraCaptures<'a> {
    queues: &'a mut renderide_shared::ipc::HostDualQueueIpc,
    lockstep: &'a mut LockstepDriver,
    prefix: &'a str,
    backing_dir: &'a std::path::Path,
    setup: &'a SceneSetup,
    scene: &'a mut super::scene_state::SceneState,
    cfg: &'a SceneSessionConfig,
    renderer: &'a mut std::process::Child,
}

fn finish_extra_captures(
    capture: ExtraCaptures<'_>,
    property_recapture: bool,
    shadow_move: bool,
) -> Result<(), HarnessError> {
    if property_recapture {
        custom_property::recapture_after_property_update(custom_property::PropertyRecapture {
            queues: capture.queues,
            lockstep: capture.lockstep,
            prefix: capture.prefix,
            backing_dir: capture.backing_dir,
            setup: capture.setup,
            output_path: &capture.cfg.output_path,
            timeout: capture.cfg.timeout,
            renderer: capture.renderer,
        })?;
    }
    if shadow_move {
        shadowed_caster::recapture_after_move(shadowed_caster::ShadowMoveRecapture {
            queues: capture.queues,
            lockstep: capture.lockstep,
            scene: capture.scene,
            setup: capture.setup,
            output_path: &capture.cfg.output_path,
            timeout: capture.cfg.timeout,
            interval: Duration::from_millis(capture.cfg.interval_ms.max(1)),
            renderer: capture.renderer,
        })?;
    }
    Ok(())
}
