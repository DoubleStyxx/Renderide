//! Unit tests for [`super::layout`] (mip byte sizing).

use glam::IVec2;

use super::layout::{
    block_extent, bytes_per_compressed_block, clamp_host_texture_mip_count,
    estimate_gpu_cubemap_bytes, estimate_gpu_texture_bytes, estimate_gpu_texture3d_bytes,
    host_format_is_compressed, host_mip_payload_byte_offset, legal_texture2d_mip_level_count,
    legal_texture3d_mip_level_count, mip_byte_len, mip_compressed_byte_len,
    mip_dimensions_at_level, mip_dimensions_at_level_3d, mip_tight_bytes_per_texel,
    mip_uncompressed_byte_len, validate_mip_upload_layout,
};
use crate::shared::{SetTexture2DData, TextureFormat};

#[test]
fn validate_mip_layout_accepts_contiguous_payload() {
    let mut d = SetTexture2DData::default();
    d.data.length = 4 * 4 * 4 + 2 * 2 * 4; // rgba32 mip0 + mip1
    d.mip_map_sizes = vec![IVec2::new(4, 4), IVec2::new(2, 2)];
    d.mip_starts = vec![0, 64];
    assert!(validate_mip_upload_layout(TextureFormat::RGBA32, &d).is_ok());
}

#[test]
fn validate_mip_layout_rejects_overflow() {
    let mut d = SetTexture2DData::default();
    d.data.length = 10;
    d.mip_map_sizes = vec![IVec2::new(4, 4)];
    d.mip_starts = vec![0];
    assert!(validate_mip_upload_layout(TextureFormat::RGBA32, &d).is_err());
}

#[test]
fn validate_mip_layout_rejects_shape_mismatches_and_bad_offsets() {
    let mut mismatch = SetTexture2DData::default();
    mismatch.data.length = 64;
    mismatch.mip_map_sizes = vec![IVec2::new(4, 4)];
    mismatch.mip_starts = vec![0, 16];
    assert_eq!(
        validate_mip_upload_layout(TextureFormat::RGBA32, &mismatch),
        Err("mip_map_sizes and mip_starts length mismatch")
    );

    let mut non_positive = SetTexture2DData::default();
    non_positive.data.length = 64;
    non_positive.mip_map_sizes = vec![IVec2::new(0, 4)];
    non_positive.mip_starts = vec![0];
    assert_eq!(
        validate_mip_upload_layout(TextureFormat::RGBA32, &non_positive),
        Err("non-positive mip dimensions")
    );

    let mut negative = SetTexture2DData::default();
    negative.data.length = 64;
    negative.mip_map_sizes = vec![IVec2::new(4, 4)];
    negative.mip_starts = vec![-1];
    assert_eq!(
        validate_mip_upload_layout(TextureFormat::RGBA32, &negative),
        Err("negative mip_starts")
    );
}

#[test]
fn bc1_mip0_128_bytes_for_32x32() {
    let b = mip_byte_len(TextureFormat::BC1, 32, 32).expect("bc1");
    assert_eq!(b, (32 / 4) * (32 / 4) * 8);
}

#[test]
fn compressed_formats_report_block_dimensions_and_sizes() {
    assert!(host_format_is_compressed(TextureFormat::BC7));
    assert!(!host_format_is_compressed(TextureFormat::RGBA32));
    assert_eq!(block_extent(TextureFormat::ASTC5x5), (5, 5));
    assert_eq!(block_extent(TextureFormat::BC1), (4, 4));
    assert_eq!(bytes_per_compressed_block(TextureFormat::BC1), Some(8));
    assert_eq!(bytes_per_compressed_block(TextureFormat::BC7), Some(16));
    assert_eq!(bytes_per_compressed_block(TextureFormat::RGBA32), None);
}

#[test]
fn compressed_mip_size_rounds_up_to_block_grid() {
    assert_eq!(mip_compressed_byte_len(TextureFormat::BC1, 1, 1), Some(8));
    assert_eq!(mip_compressed_byte_len(TextureFormat::BC1, 5, 5), Some(32));
    assert_eq!(
        mip_compressed_byte_len(TextureFormat::ASTC5x5, 6, 5),
        Some(32)
    );
}

#[test]
fn rgba32_mip0_byte_len() {
    assert_eq!(
        mip_byte_len(TextureFormat::RGBA32, 16, 16).unwrap(),
        16 * 16 * 4
    );
}

#[test]
fn uncompressed_mip_size_covers_supported_texel_widths() {
    assert_eq!(
        mip_uncompressed_byte_len(TextureFormat::Alpha8, 3, 2),
        Some(6)
    );
    assert_eq!(
        mip_uncompressed_byte_len(TextureFormat::RGB24, 3, 2),
        Some(18)
    );
    assert_eq!(
        mip_uncompressed_byte_len(TextureFormat::RGBAHalf, 3, 2),
        Some(48)
    );
    assert_eq!(
        mip_uncompressed_byte_len(TextureFormat::Unknown, 3, 2),
        None
    );
    assert_eq!(mip_uncompressed_byte_len(TextureFormat::BC1, 3, 2), None);
}

#[test]
fn rgba_float_matches_rgba32_float_texel_size() {
    assert_eq!(mip_byte_len(TextureFormat::RGBAFloat, 1, 1).unwrap(), 16);
    assert_eq!(mip_tight_bytes_per_texel(16 * 4 * 4, 4, 4), Some(16));
}

#[test]
fn tight_bytes_per_texel_rejects_zero_or_uneven_payloads() {
    assert_eq!(mip_tight_bytes_per_texel(0, 0, 4), None);
    assert_eq!(mip_tight_bytes_per_texel(7, 2, 2), None);
    assert_eq!(mip_tight_bytes_per_texel(16, 2, 2), Some(4));
}

#[test]
fn host_mip_payload_byte_offset_converts_texels_to_bytes() {
    assert_eq!(
        host_mip_payload_byte_offset(TextureFormat::RGBA32, 16),
        Some(64)
    );
    assert_eq!(host_mip_payload_byte_offset(TextureFormat::BC1, 0), Some(0));
    assert_eq!(
        host_mip_payload_byte_offset(TextureFormat::BC1, 16),
        Some(8)
    );
}

#[test]
fn mip_dimensions_at_level_halves_each_step() {
    assert_eq!(mip_dimensions_at_level(114, 200, 0), (114, 200));
    assert_eq!(mip_dimensions_at_level(114, 200, 1), (57, 100));
    assert_eq!(mip_dimensions_at_level(114, 200, 2), (28, 50));
    assert_eq!(mip_dimensions_at_level(1, 1, 5), (1, 1));
}

#[test]
fn mip_dimensions_at_level_3d_halves_all_axes() {
    assert_eq!(mip_dimensions_at_level_3d(9, 5, 3, 0), (9, 5, 3));
    assert_eq!(mip_dimensions_at_level_3d(9, 5, 3, 1), (4, 2, 1));
    assert_eq!(mip_dimensions_at_level_3d(9, 5, 3, 4), (1, 1, 1));
}

#[test]
fn legal_texture2d_mips_follow_largest_axis() {
    assert_eq!(legal_texture2d_mip_level_count(1, 1), 1);
    assert_eq!(legal_texture2d_mip_level_count(256, 128), 9);
    assert_eq!(legal_texture2d_mip_level_count(257, 16), 9);
}

#[test]
fn legal_texture3d_mips_follow_largest_axis() {
    assert_eq!(legal_texture3d_mip_level_count(1, 1, 1), 1);
    assert_eq!(legal_texture3d_mip_level_count(64, 32, 16), 7);
    assert_eq!(legal_texture3d_mip_level_count(7, 9, 33), 6);
}

#[test]
fn host_mip_count_clamps_to_allocated_texture_count() {
    assert_eq!(clamp_host_texture_mip_count(-4, 8), 1);
    assert_eq!(clamp_host_texture_mip_count(0, 8), 1);
    assert_eq!(clamp_host_texture_mip_count(12, 9), 9);
    assert_eq!(clamp_host_texture_mip_count(5, 9), 5);
}

#[test]
fn gpu_texture_byte_estimates_follow_wgpu_block_layouts() {
    assert_eq!(
        estimate_gpu_texture_bytes(wgpu::TextureFormat::Rgba8Unorm, 4, 2, 3),
        44
    );
    assert_eq!(
        estimate_gpu_texture3d_bytes(wgpu::TextureFormat::Rgba8Unorm, 4, 2, 2, 2),
        72
    );
    assert_eq!(
        estimate_gpu_cubemap_bytes(wgpu::TextureFormat::Rgba8Unorm, 1, 1),
        24
    );
}
