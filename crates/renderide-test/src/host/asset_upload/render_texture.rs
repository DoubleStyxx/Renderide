//! Render-texture allocation helper (`SetRenderTextureFormat` -> `RenderTextureResult`).

use std::time::Duration;

use glam::IVec2;
use renderide_shared::ipc::HostDualQueueIpc;
use renderide_shared::shared::{
    RendererCommand, SetRenderTextureFormat, TextureFilterMode, TextureWrapMode,
};

use crate::error::HarnessError;

use super::super::command_wait::wait_for_command;
use super::super::lockstep::LockstepDriver;

/// Unity depth-stencil bits requested for host render textures (the renderer always allocates a
/// depth attachment; this keeps the request explicit).
const RENDER_TEXTURE_DEPTH_BITS: i32 = 24;

/// Asks the renderer to allocate render texture `asset_id` (`width` x `height`) and waits for the
/// matching `RenderTextureResult`.
pub(in crate::host) fn create_render_texture(
    queues: &mut HostDualQueueIpc,
    lockstep: &mut LockstepDriver,
    asset_id: i32,
    size: (i32, i32),
    timeout: Duration,
) -> Result<(), HarnessError> {
    let format = SetRenderTextureFormat {
        size: IVec2::new(size.0, size.1),
        depth: RENDER_TEXTURE_DEPTH_BITS,
        filter_mode: TextureFilterMode::default(),
        aniso_level: 0,
        wrap_u: TextureWrapMode::Clamp,
        wrap_v: TextureWrapMode::Clamp,
        asset_id,
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
    )?;
    logger::info!(
        "AssetUpload: render texture {asset_id} {}x{} resident",
        size.0,
        size.1
    );
    Ok(())
}
