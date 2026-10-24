//! Render-texture allocation (`SetRenderTextureFormat` -> `RenderTextureResult`).

use std::time::Duration;

use glam::IVec2;
use renderide_shared::ipc::HostDualQueueIpc;
use renderide_shared::shared::{
    RendererCommand, SetRenderTextureFormat, TextureFilterMode, TextureWrapMode,
};

use crate::error::HarnessError;

use super::super::command_wait::wait_for_command;
use super::super::lockstep::LockstepDriver;
use super::DEFAULT_ASSET_UPLOAD_TIMEOUT;

/// Unity depth-stencil bits requested for host render textures (the renderer always allocates a
/// depth attachment; this keeps the request explicit).
const RENDER_TEXTURE_DEPTH_BITS: i32 = 24;

/// Asks the renderer to allocate render texture `asset_id` (`width` x `height`) and waits for the
/// matching `RenderTextureResult`.
///
/// Frame-bench uses the host default filter and zero anisotropy. The stacked-camera golden uses
/// [`upload_render_texture`], which asks for a point filter so the capture stays deterministic.
pub(in crate::host) fn create_render_texture(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    asset_id: i32,
    size: (i32, i32),
    timeout: Duration,
) -> Result<(), HarnessError> {
    send_render_texture(
        queues,
        lockstep,
        asset_id,
        size,
        TextureFilterMode::default(),
        0,
        timeout,
    )?;
    logger::info!(
        "AssetUpload: render texture {asset_id} {}x{} resident",
        size.0,
        size.1
    );
    Ok(())
}

/// Registers one host render texture with a point filter and waits for `RenderTextureResult`.
pub(in crate::host) fn upload_render_texture(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    asset_id: i32,
    size: (i32, i32),
) -> Result<(), HarnessError> {
    logger::info!(
        "AssetUpload: sent SetRenderTextureFormat(asset_id={asset_id}, {}x{})",
        size.0,
        size.1
    );
    send_render_texture(
        queues,
        lockstep,
        asset_id,
        size,
        TextureFilterMode::Point,
        1,
        DEFAULT_ASSET_UPLOAD_TIMEOUT,
    )?;
    logger::info!("AssetUpload: received RenderTextureResult(asset_id={asset_id})");
    Ok(())
}

fn send_render_texture(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    asset_id: i32,
    size: (i32, i32),
    filter_mode: TextureFilterMode,
    aniso_level: i32,
    timeout: Duration,
) -> Result<(), HarnessError> {
    let format = SetRenderTextureFormat {
        asset_id,
        size: IVec2::new(size.0, size.1),
        depth: RENDER_TEXTURE_DEPTH_BITS,
        filter_mode,
        aniso_level,
        wrap_u: TextureWrapMode::Clamp,
        wrap_v: TextureWrapMode::Clamp,
    };
    if !queues.send_background(RendererCommand::SetRenderTextureFormat(format)) {
        return Err(HarnessError::QueueOptions(
            "send_background(SetRenderTextureFormat) returned false (queue full?)".to_string(),
        ));
    }
    wait_for_command(
        queues,
        lockstep,
        timeout,
        |wait| HarnessError::AssetAckTimeout(wait, "RenderTextureResult never arrived"),
        |msg| match msg {
            RendererCommand::RenderTextureResult(result) if result.asset_id == asset_id => Some(()),
            _ => None,
        },
    )
}
