//! Opt-in pre-sRGB scene-color readback for the #216 term-isolation scenes.
//!
//! Production frames leave this off. The headless driver sets the flag from
//! `--headless-scene-color-readback` before the first tick. While the flag is
//! off the compose pass does not clone a texture, the scene-color allocation
//! does not gain `COPY_SRC`, and the PNG writer does not map a second buffer.
//!
//! Prism: this module is the production hook for experiment 8d. It is debug-only.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use serde_json::{Value, json};

/// Process-wide switch. Stays false unless the headless CLI asks for the dump.
static ENABLED: AtomicBool = AtomicBool::new(false);
/// Last HDR scene-color texture cloned from compose. Empty while the flag is off.
static STASHED: Mutex<Option<wgpu::Texture>> = Mutex::new(None);

/// Turns the debug readback on or off for this process.
///
/// Call this before the first rendered frame. The scene-color texture is created
/// with `COPY_SRC` only when the flag is already set.
pub(crate) fn set_readback_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether this process should copy HDR scene color beside the headless PNG.
#[inline]
pub(crate) fn readback_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Base usage for the resolved HDR scene-color transient.
///
/// The off state is the production usage. `COPY_SRC` is added only for the debug copy.
pub(crate) fn scene_color_hdr_base_usage() -> wgpu::TextureUsages {
    scene_color_hdr_usage(readback_enabled())
}

/// [`scene_color_hdr_base_usage`] with the debug copy selected explicitly.
pub(crate) fn scene_color_hdr_usage(debug_copy: bool) -> wgpu::TextureUsages {
    let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    if debug_copy {
        usage | wgpu::TextureUsages::COPY_SRC
    } else {
        usage
    }
}

/// Remembers the HDR scene-color texture so the headless dump can copy it.
///
/// Returns immediately when the flag is off, without taking the stash lock.
pub(crate) fn remember_scene_color(texture: &wgpu::Texture) {
    if !readback_enabled() {
        return;
    }
    *STASHED.lock() = Some(texture.clone());
}

/// Clones the stashed scene-color texture, if compose stored one this frame.
pub(crate) fn stashed_scene_color() -> Option<wgpu::Texture> {
    STASHED.lock().clone()
}

/// Raw little-endian `f16` RGBA file written beside the PNG.
pub(crate) fn raw_sidecar_path(png: &Path) -> std::path::PathBuf {
    sidecar_path(png, "scene_color.rgba16f")
}

/// Quadrant stats JSON written beside the PNG.
pub(crate) fn json_sidecar_path(png: &Path) -> std::path::PathBuf {
    sidecar_path(png, "scene_color.json")
}

fn sidecar_path(png: &Path, suffix: &str) -> std::path::PathBuf {
    let stem = png
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".to_string());
    png.with_file_name(format!("{stem}.{suffix}"))
}

/// Header plus tightly packed little-endian `f16` RGBA.
///
/// Bytes 0..4 are `R16F`, then version `1`, width, and height, all `u32` little-endian.
pub(crate) fn encode_rgba16f_file(width: u32, height: u32, tight_rgba16: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16 + tight_rgba16.len());
    bytes.extend_from_slice(b"R16F");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(tight_rgba16);
    bytes
}

/// Quadrant stats over a tightly packed little-endian `f16` RGBA image.
///
/// The grid is 2×2 in texture space, origin top-left, row-major:
/// `top_left`, `top_right`, `bottom_left`, `bottom_right`.
/// The left and top quadrants take the extra pixel when a side is odd.
/// Means skip NaN and infinity. A channel with no finite samples has a null mean.
/// Negative counts are finite values `< 0`. Infinity is not negative.
pub(crate) fn scene_color_stats_json(width: u32, height: u32, tight_rgba16: &[u8]) -> Value {
    let regions = quadrant_stats(width, height, tight_rgba16);
    json!({
        "format": "rgba16float",
        "width": width,
        "height": height,
        "origin": "top-left",
        "grid": "2x2",
        "regions": regions,
    })
}

fn quadrant_stats(width: u32, height: u32, tight_rgba16: &[u8]) -> Vec<Value> {
    let mid_x = width.div_ceil(2);
    let mid_y = height.div_ceil(2);
    let rects = [
        ("top_left", 0, 0, mid_x, mid_y),
        ("top_right", mid_x, 0, width.saturating_sub(mid_x), mid_y),
        ("bottom_left", 0, mid_y, mid_x, height.saturating_sub(mid_y)),
        (
            "bottom_right",
            mid_x,
            mid_y,
            width.saturating_sub(mid_x),
            height.saturating_sub(mid_y),
        ),
    ];
    rects
        .into_iter()
        .map(|(name, x, y, rect_w, rect_h)| {
            region_json(name, x, y, rect_w, rect_h, width, tight_rgba16)
        })
        .collect()
}

fn region_json(
    name: &str,
    x: u32,
    y: u32,
    rect_w: u32,
    rect_h: u32,
    image_w: u32,
    tight_rgba16: &[u8],
) -> Value {
    let mut sum = [0.0f64; 4];
    let mut finite = [0u32; 4];
    let mut negative = [0u32; 4];
    let mut nan = [0u32; 4];
    let mut inf = [0u32; 4];
    for row in y..y.saturating_add(rect_h) {
        for col in x..x.saturating_add(rect_w) {
            let pixel = (row as usize) * (image_w as usize) + (col as usize);
            for channel in 0..4 {
                let value = f16_bits_to_f32(f16_at(tight_rgba16, pixel, channel));
                if value.is_nan() {
                    nan[channel] = nan[channel].saturating_add(1);
                } else if value.is_infinite() {
                    inf[channel] = inf[channel].saturating_add(1);
                } else {
                    finite[channel] = finite[channel].saturating_add(1);
                    sum[channel] += f64::from(value);
                    if value < 0.0 {
                        negative[channel] = negative[channel].saturating_add(1);
                    }
                }
            }
        }
    }
    let mean: Vec<Value> = (0..4)
        .map(|channel| {
            if finite[channel] == 0 {
                Value::Null
            } else {
                json!(sum[channel] / f64::from(finite[channel]))
            }
        })
        .collect();
    json!({
        "name": name,
        "x": x,
        "y": y,
        "width": rect_w,
        "height": rect_h,
        "mean": mean,
        "finite": finite,
        "negative": negative,
        "nan": nan,
        "inf": inf,
    })
}

fn f16_at(bytes: &[u8], pixel: usize, channel: usize) -> u16 {
    let offset = (pixel * 4 + channel) * 2;
    let Some(pair) = bytes.get(offset..offset + 2) else {
        return 0;
    };
    u16::from_le_bytes([pair[0], pair[1]])
}

/// IEEE-754 binary16 to binary32.
pub(crate) fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) & 1;
    let exp = u32::from((bits >> 10) & 0x1f);
    let frac = u32::from(bits & 0x3ff);
    let out = if exp == 0 {
        if frac == 0 {
            sign << 31
        } else {
            let mut magnitude = frac;
            let mut exponent = 127 - 15 + 1;
            while magnitude & 0x400 == 0 {
                magnitude <<= 1;
                exponent -= 1;
            }
            magnitude &= 0x3ff;
            (sign << 31) | (exponent << 23) | (magnitude << 13)
        }
    } else if exp == 31 {
        (sign << 31) | (0xff << 23) | (frac << 13)
    } else {
        (sign << 31) | ((exp + (127 - 15)) << 23) | (frac << 13)
    };
    f32::from_bits(out)
}

#[cfg(test)]
mod tests {
    use super::{
        encode_rgba16f_file, f16_bits_to_f32, json_sidecar_path, raw_sidecar_path,
        readback_enabled, scene_color_hdr_usage, scene_color_stats_json,
    };
    use std::path::Path;

    #[test]
    fn readback_defaults_off_and_usage_matches_production() {
        assert!(!readback_enabled());
        let off = scene_color_hdr_usage(false);
        assert!(off.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
        assert!(off.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(!off.contains(wgpu::TextureUsages::COPY_SRC));
        let on = scene_color_hdr_usage(true);
        assert!(on.contains(wgpu::TextureUsages::COPY_SRC));
        assert!(on.contains(off));
    }

    #[test]
    fn f16_values_match_known_bit_patterns() {
        assert_eq!(f16_bits_to_f32(0), 0.0);
        assert_eq!(f16_bits_to_f32(0x3c00), 1.0);
        assert_eq!(f16_bits_to_f32(0x3800), 0.5);
        assert_eq!(f16_bits_to_f32(0x3400), 0.25);
        assert_eq!(f16_bits_to_f32(0x3a00), 0.75);
        assert_eq!(f16_bits_to_f32(0xbc00), -1.0);
        assert!(f16_bits_to_f32(0x7c00).is_infinite());
        assert!(f16_bits_to_f32(0x7c00).is_sign_positive());
        assert!(f16_bits_to_f32(0xfc00).is_infinite());
        assert!(f16_bits_to_f32(0x7e00).is_nan());
    }

    #[test]
    fn quadrant_stats_count_sign_nan_and_inf() {
        // 2×2, one pixel per quadrant: (1,0,0,1), (0,-1,0,1), (NaN,0,+Inf,1), (0.25,0.5,0.75,1).
        let pixels: [[u16; 4]; 4] = [
            [0x3c00, 0, 0, 0x3c00],
            [0, 0xbc00, 0, 0x3c00],
            [0x7e00, 0, 0x7c00, 0x3c00],
            [0x3400, 0x3800, 0x3a00, 0x3c00],
        ];
        let mut bytes = Vec::new();
        for pixel in pixels {
            for channel in pixel {
                bytes.extend_from_slice(&channel.to_le_bytes());
            }
        }
        let file = encode_rgba16f_file(2, 2, &bytes);
        assert_eq!(&file[..4], b"R16F");
        assert_eq!(u32::from_le_bytes(file[4..8].try_into().expect("ver")), 1);
        assert_eq!(u32::from_le_bytes(file[8..12].try_into().expect("w")), 2);
        assert_eq!(u32::from_le_bytes(file[12..16].try_into().expect("h")), 2);

        let stats = scene_color_stats_json(2, 2, &bytes);
        let regions = stats["regions"].as_array().expect("regions");
        assert_eq!(regions.len(), 4);
        assert_eq!(regions[0]["name"], "top_left");
        assert_eq!(regions[0]["mean"][0], 1.0);
        assert_eq!(regions[0]["negative"][0], 0);
        assert_eq!(regions[1]["name"], "top_right");
        assert_eq!(regions[1]["mean"][1], -1.0);
        assert_eq!(regions[1]["negative"][1], 1);
        assert_eq!(regions[2]["nan"][0], 1);
        assert_eq!(regions[2]["inf"][2], 1);
        assert!(regions[2]["mean"][0].is_null());
        assert_eq!(regions[2]["mean"][1], 0.0);
        assert_eq!(regions[3]["mean"][0], 0.25);
        assert_eq!(regions[3]["mean"][1], 0.5);
        assert_eq!(regions[3]["mean"][2], 0.75);
    }

    #[test]
    fn sidecar_names_sit_beside_the_png() {
        let png = Path::new("out/warp216_constant/actual.png");
        assert_eq!(
            raw_sidecar_path(png),
            Path::new("out/warp216_constant/actual.scene_color.rgba16f")
        );
        assert_eq!(
            json_sidecar_path(png),
            Path::new("out/warp216_constant/actual.scene_color.json")
        );
    }

    #[test]
    fn debug_term_function_is_compiled_out_of_the_production_prefix() {
        let src = include_str!("../shaders/modules/pbs/lighting.wgsl");
        let (prefix, suffix) = src.split_once("#ifdef _TERM_DEBUG").expect("debug block");
        assert!(prefix.contains("fn shade_metallic_clustered("));
        assert!(!prefix.contains("shade_metallic_term_debug"));
        assert!(!prefix.contains("DEBUG_TERM_DIFFUSE"));
        assert!(suffix.contains("fn shade_metallic_term_debug("));
        assert!(suffix.contains("const DEBUG_TERM_COMPOSED: u32 = 0u;"));
        assert!(suffix.contains("const DEBUG_TERM_DIFFUSE: u32 = 1u;"));
        assert!(suffix.contains("const DEBUG_TERM_SPECULAR: u32 = 2u;"));
        assert!(suffix.contains("const DEBUG_TERM_AMBIENT: u32 = 3u;"));
        assert!(suffix.contains("const DEBUG_TERM_INDIRECT: u32 = 4u;"));
        assert!(suffix.contains("const DEBUG_TERM_FRESNEL: u32 = 5u;"));
        assert!(suffix.contains("const DEBUG_TERM_EMISSION: u32 = 6u;"));
        assert!(suffix.contains("#endif"));
        let brdf = include_str!("../shaders/modules/pbs/brdf.wgsl");
        let (brdf_prefix, brdf_suffix) = brdf.split_once("#ifdef _TERM_DEBUG").expect("brdf debug");
        assert!(brdf_prefix.contains("fn direct_radiance_metallic_prepared("));
        assert!(!brdf_prefix.contains("direct_metallic_term_parts"));
        assert!(brdf_suffix.contains("fn direct_metallic_term_parts("));
    }
}
