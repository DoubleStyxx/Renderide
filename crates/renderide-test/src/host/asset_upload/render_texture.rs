//! Render-texture format registration for headless cases that sample a camera target.

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

/// Registers one host render texture and waits for `RenderTextureResult`.
pub(in crate::host) fn upload_render_texture(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    asset_id: i32,
    size: (i32, i32),
) -> Result<(), HarnessError> {
    let format = SetRenderTextureFormat {
        asset_id,
        size: IVec2::new(size.0, size.1),
        depth: 24,
        filter_mode: TextureFilterMode::Point,
        aniso_level: 1,
        wrap_u: TextureWrapMode::Clamp,
        wrap_v: TextureWrapMode::Clamp,
    };
    if !queues.send_background(RendererCommand::SetRenderTextureFormat(format)) {
        return Err(HarnessError::QueueOptions(
            "send_background(SetRenderTextureFormat) returned false (queue full?)".to_string(),
        ));
    }
    logger::info!(
        "AssetUpload: sent SetRenderTextureFormat(asset_id={asset_id}, {}x{})",
        size.0,
        size.1
    );
    wait_for_render_texture_result(queues, lockstep, asset_id, DEFAULT_ASSET_UPLOAD_TIMEOUT)?;
    logger::info!("AssetUpload: received RenderTextureResult(asset_id={asset_id})");
    Ok(())
}

fn wait_for_render_texture_result(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    asset_id: i32,
    timeout: Duration,
) -> Result<(), HarnessError> {
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
