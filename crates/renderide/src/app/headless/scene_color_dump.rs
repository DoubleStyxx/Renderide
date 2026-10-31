//! Copies the stashed HDR scene-color texture beside the headless PNG.
//!
//! Called only when [`crate::debug_scene_color::readback_enabled`] is set, and only
//! from the headless PNG path. The files land before the PNG rename so a stable
//! `actual.png` already has its float sidecar.

use std::path::Path;

use crate::debug_scene_color::{
    encode_rgba16f_file, json_sidecar_path, raw_sidecar_path, scene_color_stats_json,
    stashed_scene_color,
};
use crate::gpu::GpuContext;

use super::readback::{HeadlessReadbackError, await_buffer_map, confirm_frame_submission_done};

/// Writes `*.scene_color.rgba16f` and `*.scene_color.json` next to `png`.
pub(super) fn write_scene_color_sidecars(
    gpu: &GpuContext,
    png: &Path,
) -> Result<(), HeadlessReadbackError> {
    let texture = stashed_scene_color().ok_or_else(|| {
        HeadlessReadbackError::Encode("scene color texture was not stashed".to_string())
    })?;
    if texture.format() != wgpu::TextureFormat::Rgba16Float {
        return Err(HeadlessReadbackError::Encode(format!(
            "scene color format is {:?}, expected Rgba16Float",
            texture.format()
        )));
    }
    let extent = texture.size();
    let width = extent.width;
    let height = extent.height;
    if width == 0 || height == 0 {
        return Err(HeadlessReadbackError::EmptyExtent);
    }
    let tight_stride = width.saturating_mul(8);
    let padded_stride = tight_stride.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer_size = u64::from(padded_stride) * u64::from(height);
    if buffer_size > gpu.limits().max_buffer_size() {
        return Err(HeadlessReadbackError::BufferSizeExceedsLimit {
            size: buffer_size,
            max: gpu.limits().max_buffer_size(),
        });
    }

    let _stall = confirm_frame_submission_done(gpu)?;
    let readback = gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("renderide-scene-color-debug-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    copy_scene_color(gpu, &texture, &readback, width, height, padded_stride)?;
    let slice = readback.slice(..);
    await_buffer_map(slice, gpu.device())?;
    let tight = {
        let view = slice
            .get_mapped_range()
            .map_err(|err| HeadlessReadbackError::Map(err.to_string()))?;
        tight_rows(&view, height, padded_stride, tight_stride)?
    };
    readback.unmap();

    let raw = encode_rgba16f_file(width, height, &tight);
    let json = scene_color_stats_json(width, height, &tight);
    let json_text = serde_json::to_string_pretty(&json)
        .map_err(|err| HeadlessReadbackError::Encode(err.to_string()))?;
    write_atomic(&raw_sidecar_path(png), &raw)?;
    write_atomic(json_sidecar_path(png).as_path(), json_text.as_bytes())?;
    Ok(())
}

fn copy_scene_color(
    gpu: &GpuContext,
    texture: &wgpu::Texture,
    readback: &wgpu::Buffer,
    width: u32,
    height: u32,
    padded_stride: u32,
) -> Result<(), HeadlessReadbackError> {
    gpu.flush_driver();
    let mut encoder = gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("renderide-scene-color-debug-readback"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let command_buffer = encoder.finish();
    gpu.queue().submit(std::iter::once(command_buffer));
    Ok(())
}

fn tight_rows(
    bytes: &[u8],
    height: u32,
    padded_stride: u32,
    tight_stride: u32,
) -> Result<Vec<u8>, HeadlessReadbackError> {
    let mut tight = vec![0u8; tight_stride as usize * height as usize];
    for row in 0..height as usize {
        let src = row * padded_stride as usize;
        let dst = row * tight_stride as usize;
        let src_end = src + tight_stride as usize;
        let source = bytes
            .get(src..src_end)
            .ok_or(HeadlessReadbackError::EncodeBufferSize)?;
        tight[dst..dst + tight_stride as usize].copy_from_slice(source);
    }
    Ok(tight)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), HeadlessReadbackError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(HeadlessReadbackError::Io)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(HeadlessReadbackError::Io)?;
    std::fs::rename(&tmp, path).map_err(HeadlessReadbackError::Io)
}
