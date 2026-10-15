//! Tests for 2D mip-chain upload conversion.
//!
//! After the unified-orientation refactor the renderer never row-flips host data on ingest,
//! so every host upload (uncompressed and compressed alike) lands identically to
//! the bytes we received and reports `storage_v_inverted = true` (Unity V=0 bottom orientation).

use super::super::super::layout::mip_byte_len;
use super::super::mip_write_common::MipUploadFormatCtx;
use super::conversion::mip_src_to_upload_pixels;
use crate::shared::TextureFormat;

fn upload_ctx(fmt_format: TextureFormat, wgpu_format: wgpu::TextureFormat) -> MipUploadFormatCtx {
    MipUploadFormatCtx {
        asset_id: 77,
        fmt_format,
        wgpu_format,
        needs_rgba8_decode: false,
    }
}

#[test]
fn bc7_flip_y_uploads_bytes_unchanged() {
    let raw: Vec<u8> = (0..64).collect();
    let pixels = mip_src_to_upload_pixels(
        upload_ctx(TextureFormat::BC7, wgpu::TextureFormat::Bc7RgbaUnorm),
        8,
        8,
        true,
        &raw,
        0,
    )
    .expect("bc7 upload");

    assert_eq!(pixels.bytes, raw);
    assert!(pixels.storage_v_inverted);
}

#[test]
fn native_compressed_flip_y_keeps_bytes_intact() {
    for (host_format, wgpu_format) in [
        (TextureFormat::BC1, wgpu::TextureFormat::Bc1RgbaUnorm),
        (TextureFormat::BC3, wgpu::TextureFormat::Bc3RgbaUnorm),
        (TextureFormat::BC6H, wgpu::TextureFormat::Bc6hRgbUfloat),
        (TextureFormat::BC7, wgpu::TextureFormat::Bc7RgbaUnorm),
        (TextureFormat::ETC2RGB, wgpu::TextureFormat::Etc2Rgb8Unorm),
        (
            TextureFormat::ETC2RGBA1,
            wgpu::TextureFormat::Etc2Rgb8A1Unorm,
        ),
        (
            TextureFormat::ETC2RGBA8,
            wgpu::TextureFormat::Etc2Rgba8Unorm,
        ),
    ] {
        let len = mip_byte_len(host_format, 8, 8).expect("compressed mip byte length");
        let raw: Vec<u8> = (0..len).map(|i| i as u8).collect();
        let pixels =
            mip_src_to_upload_pixels(upload_ctx(host_format, wgpu_format), 8, 8, true, &raw, 0)
                .expect("native compressed upload");

        assert_eq!(
            pixels.bytes, raw,
            "{host_format:?} bytes should stay intact"
        );
        assert!(
            pixels.storage_v_inverted,
            "{host_format:?} bytes are in Unity orientation"
        );
    }
}

#[test]
fn rgba8_flip_y_uploads_bytes_unchanged() {
    let raw: Vec<u8> = (0..(4 * 4 * 4)).map(|i| i as u8).collect();
    let pixels = mip_src_to_upload_pixels(
        upload_ctx(TextureFormat::RGBA32, wgpu::TextureFormat::Rgba8Unorm),
        4,
        4,
        true,
        &raw,
        0,
    )
    .expect("rgba8 upload");

    assert_eq!(pixels.bytes, raw);
    assert!(pixels.storage_v_inverted);
}
