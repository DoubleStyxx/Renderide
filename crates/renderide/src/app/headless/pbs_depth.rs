//! Depth-attachment dump for the `pbs_lit` canary.
//!
//! The color PNG stays the harness input. This writes a sibling `depth.f32` and
//! logs how many texels in each vertical band are not the reverse-Z clear.

use std::path::Path;
use std::time::Duration;

use crate::gpu::GpuContext;
use crate::pbs_diag as pbs_capture;

/// Copies the headless depth attachment when the canary is armed.
///
/// A format wgpu cannot copy, or a failed map, is logged and does not fail the PNG.
pub(super) fn dump_depth(gpu: &GpuContext, output_path: &Path) {
    if !pbs_capture::enabled() {
        return;
    }
    let Some(depth) = gpu.headless_depth_texture() else {
        pbs_capture::note_depth("missing=1");
        return;
    };
    let format = depth.format();
    let extent = depth.size();
    if !matches!(
        format,
        wgpu::TextureFormat::Depth32Float | wgpu::TextureFormat::Depth32FloatStencil8
    ) {
        pbs_capture::note_depth(&format!(
            "copy=skipped format={format:?} size={}x{} clear={} compare={:?}",
            extent.width,
            extent.height,
            crate::gpu::MAIN_FORWARD_DEPTH_CLEAR,
            crate::gpu::MAIN_FORWARD_DEPTH_COMPARE
        ));
        return;
    }

    match read_depth_f32(gpu, depth, extent.width, extent.height) {
        Ok(samples) => {
            let summary = summarize(&samples, extent.width, extent.height, format);
            let path = output_path.with_extension("depth.f32");
            if let Err(err) = std::fs::write(&path, f32_bytes(&samples)) {
                pbs_capture::note_depth(&format!("{summary} write_error={err}"));
            } else {
                pbs_capture::note_depth(&format!("{summary} file={}", path.display()));
            }
        }
        Err(err) => {
            pbs_capture::note_depth(&format!(
                "copy=failed format={format:?} size={}x{} error={err}",
                extent.width, extent.height
            ));
        }
    }
}

fn read_depth_f32(
    gpu: &GpuContext,
    depth: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<f32>, String> {
    let tight = width.saturating_mul(4);
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = tight.div_ceil(alignment) * alignment;
    let buffer_size = u64::from(padded) * u64::from(height);
    let buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("renderide-pbs-depth-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    gpu.flush_driver();
    let mut encoder = gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("renderide-pbs-depth-readback"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: depth,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue().submit(std::iter::once(encoder.finish()));
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    gpu.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|err| format!("poll {err:?}"))?;
    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("map {err:?}")),
        Err(_) => return Err("map timeout".to_owned()),
    }
    let view = slice
        .get_mapped_range()
        .map_err(|err| format!("range {err}"))?;
    let mut samples = Vec::with_capacity((width as usize) * (height as usize));
    for row in 0..height as usize {
        let start = row * padded as usize;
        for x in 0..width as usize {
            let offset = start + x * 4;
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&view[offset..offset + 4]);
            samples.push(f32::from_le_bytes(bytes));
        }
    }
    drop(view);
    buffer.unmap();
    Ok(samples)
}

fn summarize(samples: &[f32], width: u32, height: u32, format: wgpu::TextureFormat) -> String {
    let mut min_d = f32::MAX;
    let mut max_d = f32::MIN;
    let mut nonclear = 0u32;
    let bands = [
        (0u32, 76u32),
        (77, 107),
        (108, 150),
        (151, 161),
        (162, 197),
        (198, 234),
        (235, u32::MAX),
    ];
    let mut band_counts = [0u32; 7];
    for (i, depth) in samples.iter().copied().enumerate() {
        min_d = min_d.min(depth);
        max_d = max_d.max(depth);
        if depth > 1.0e-6 {
            nonclear += 1;
            let y = (i as u32) / width.max(1);
            for (band, (lo, hi)) in bands.iter().enumerate() {
                if y >= *lo && y <= *hi {
                    band_counts[band] += 1;
                    break;
                }
            }
        }
    }
    format!(
        "format={format:?} size={width}x{height} clear={} compare={:?} nonclear={nonclear} min={min_d:.5} max={max_d:.5} y0-76={} y77-107={} y108-150={} y151-161={} y162-197={} y198-234={} y235-end={}",
        crate::gpu::MAIN_FORWARD_DEPTH_CLEAR,
        crate::gpu::MAIN_FORWARD_DEPTH_COMPARE,
        band_counts[0],
        band_counts[1],
        band_counts[2],
        band_counts[3],
        band_counts[4],
        band_counts[5],
        band_counts[6]
    )
}

fn f32_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}
