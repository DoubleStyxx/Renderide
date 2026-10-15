//! Maps host [`TextureFormat`] + [`ColorProfile`] to [`wgpu::TextureFormat`] when the device reports required compression features.

use crate::shared::{ColorProfile, TextureFormat};

/// Picks a [`wgpu::TextureFormat`] for `host` if this device advertises the needed [`wgpu::Features`].
///
/// Returns [`None`] when the combination is unknown or compression features are missing (caller may decode to `Rgba8UnormSrgb`).
pub fn pick_wgpu_storage_format(
    device: &wgpu::Device,
    host: TextureFormat,
    profile: ColorProfile,
) -> Option<wgpu::TextureFormat> {
    let f = map_host_format(host, profile)?;
    texture_format_supported(device, f).then_some(f)
}

/// Maps host format without feature checks (for estimating sizes or documentation).
pub fn map_host_format(host: TextureFormat, profile: ColorProfile) -> Option<wgpu::TextureFormat> {
    use ColorProfile::{SRGB, SRGBAlpha};
    use TextureFormat as TF;

    let srgb = matches!(profile, SRGB | SRGBAlpha);

    Some(match host {
        TF::Unknown => return None,
        TF::Alpha8 | TF::R8 => wgpu::TextureFormat::R8Unorm,
        TF::RGB24 | TF::RGB565 | TF::BGR565 => return None, // decode path
        TF::RGBA32 | TF::ARGB32 | TF::BGRA32 => {
            if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            }
        }
        TF::RGBAHalf | TF::ARGBHalf => wgpu::TextureFormat::Rgba16Float,
        TF::RHalf => wgpu::TextureFormat::R16Float,
        TF::RGHalf => wgpu::TextureFormat::Rg16Float,
        TF::RGBAFloat | TF::ARGBFloat => wgpu::TextureFormat::Rgba32Float,
        TF::RFloat => wgpu::TextureFormat::R32Float,
        TF::RGFloat => wgpu::TextureFormat::Rg32Float,
        TF::BC1 => {
            if srgb {
                wgpu::TextureFormat::Bc1RgbaUnormSrgb
            } else {
                wgpu::TextureFormat::Bc1RgbaUnorm
            }
        }
        TF::BC2 => {
            if srgb {
                wgpu::TextureFormat::Bc2RgbaUnormSrgb
            } else {
                wgpu::TextureFormat::Bc2RgbaUnorm
            }
        }
        TF::BC3 => {
            if srgb {
                wgpu::TextureFormat::Bc3RgbaUnormSrgb
            } else {
                wgpu::TextureFormat::Bc3RgbaUnorm
            }
        }
        TF::BC4 => wgpu::TextureFormat::Bc4RUnorm,
        TF::BC5 => wgpu::TextureFormat::Bc5RgUnorm,
        TF::BC6H => wgpu::TextureFormat::Bc6hRgbUfloat,
        TF::BC7 => {
            if srgb {
                wgpu::TextureFormat::Bc7RgbaUnormSrgb
            } else {
                wgpu::TextureFormat::Bc7RgbaUnorm
            }
        }
        TF::ETC2RGB => {
            if srgb {
                wgpu::TextureFormat::Etc2Rgb8UnormSrgb
            } else {
                wgpu::TextureFormat::Etc2Rgb8Unorm
            }
        }
        TF::ETC2RGBA1 => {
            if srgb {
                wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb
            } else {
                wgpu::TextureFormat::Etc2Rgb8A1Unorm
            }
        }
        TF::ETC2RGBA8 => {
            if srgb {
                wgpu::TextureFormat::Etc2Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Etc2Rgba8Unorm
            }
        }
        // ASTC always routes through the RGBA8 CPU decode path. Returning [`None`] forces the
        // upload path to allocate `Rgba8Unorm{Srgb}` and decode each mip via
        // [`crate::assets::texture::decode::decode_mip_to_rgba8`].
        TF::ASTC4x4 | TF::ASTC5x5 | TF::ASTC6x6 | TF::ASTC8x8 | TF::ASTC10x10 | TF::ASTC12x12 => {
            return None;
        }
    })
}

fn texture_format_supported(device: &wgpu::Device, format: wgpu::TextureFormat) -> bool {
    if !format.is_compressed() {
        return true;
    }
    let feats = device.features();
    if format_required_bc(format) && !feats.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
        return false;
    }
    if format_required_etc2(format) && !feats.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2) {
        return false;
    }
    if format_required_astc(format) && !feats.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC) {
        return false;
    }
    true
}

fn format_required_bc(f: wgpu::TextureFormat) -> bool {
    matches!(
        f,
        wgpu::TextureFormat::Bc1RgbaUnorm
            | wgpu::TextureFormat::Bc1RgbaUnormSrgb
            | wgpu::TextureFormat::Bc2RgbaUnorm
            | wgpu::TextureFormat::Bc2RgbaUnormSrgb
            | wgpu::TextureFormat::Bc3RgbaUnorm
            | wgpu::TextureFormat::Bc3RgbaUnormSrgb
            | wgpu::TextureFormat::Bc4RUnorm
            | wgpu::TextureFormat::Bc4RSnorm
            | wgpu::TextureFormat::Bc5RgUnorm
            | wgpu::TextureFormat::Bc5RgSnorm
            | wgpu::TextureFormat::Bc6hRgbUfloat
            | wgpu::TextureFormat::Bc6hRgbFloat
            | wgpu::TextureFormat::Bc7RgbaUnorm
            | wgpu::TextureFormat::Bc7RgbaUnormSrgb
    )
}

fn format_required_etc2(f: wgpu::TextureFormat) -> bool {
    matches!(
        f,
        wgpu::TextureFormat::Etc2Rgb8Unorm
            | wgpu::TextureFormat::Etc2Rgb8UnormSrgb
            | wgpu::TextureFormat::Etc2Rgb8A1Unorm
            | wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb
            | wgpu::TextureFormat::Etc2Rgba8Unorm
            | wgpu::TextureFormat::Etc2Rgba8UnormSrgb
    )
}

fn format_required_astc(f: wgpu::TextureFormat) -> bool {
    matches!(
        f,
        wgpu::TextureFormat::Astc {
            channel: wgpu::AstcChannel::Unorm | wgpu::AstcChannel::UnormSrgb,
            ..
        }
    )
}

/// Formats we can accept via GPU-native storage or transient RGBA8 decode (advertised to the host).
pub fn supported_host_formats_for_init() -> Vec<TextureFormat> {
    use TextureFormat as TF;
    vec![
        TF::Alpha8,
        TF::R8,
        TF::RGB24,
        TF::RGBA32,
        TF::ARGB32,
        TF::BGRA32,
        TF::RGB565,
        TF::BGR565,
        TF::RGBAHalf,
        TF::ARGBHalf,
        TF::RHalf,
        TF::RGHalf,
        TF::RGBAFloat,
        TF::ARGBFloat,
        TF::RFloat,
        TF::RGFloat,
        TF::BC1,
        TF::BC2,
        TF::BC3,
        TF::BC4,
        TF::BC5,
        TF::BC6H,
        TF::BC7,
        TF::ETC2RGB,
        TF::ETC2RGBA1,
        TF::ETC2RGBA8,
        TF::ASTC4x4,
        TF::ASTC5x5,
        TF::ASTC6x6,
        TF::ASTC8x8,
        TF::ASTC10x10,
        TF::ASTC12x12,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba32_linear_maps() {
        assert_eq!(
            map_host_format(TextureFormat::RGBA32, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rgba8Unorm)
        );
    }

    #[test]
    fn srgb_profiles_map_color_formats_to_srgb_storage() {
        for profile in [ColorProfile::SRGB, ColorProfile::SRGBAlpha] {
            assert_eq!(
                map_host_format(TextureFormat::RGBA32, profile),
                Some(wgpu::TextureFormat::Rgba8UnormSrgb)
            );
            assert_eq!(
                map_host_format(TextureFormat::ARGB32, profile),
                Some(wgpu::TextureFormat::Rgba8UnormSrgb)
            );
            assert_eq!(
                map_host_format(TextureFormat::BGRA32, profile),
                Some(wgpu::TextureFormat::Rgba8UnormSrgb)
            );
            assert_eq!(
                map_host_format(TextureFormat::BC7, profile),
                Some(wgpu::TextureFormat::Bc7RgbaUnormSrgb)
            );
        }
    }

    #[test]
    fn linear_profiles_map_scalar_float_and_half_formats() {
        assert_eq!(
            map_host_format(TextureFormat::Alpha8, ColorProfile::Linear),
            Some(wgpu::TextureFormat::R8Unorm)
        );
        assert_eq!(
            map_host_format(TextureFormat::R8, ColorProfile::Linear),
            Some(wgpu::TextureFormat::R8Unorm)
        );
        assert_eq!(
            map_host_format(TextureFormat::RGBAHalf, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rgba16Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::ARGBHalf, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rgba16Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::RHalf, ColorProfile::Linear),
            Some(wgpu::TextureFormat::R16Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::RGHalf, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rg16Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::RGBAFloat, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rgba32Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::ARGBFloat, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rgba32Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::RFloat, ColorProfile::Linear),
            Some(wgpu::TextureFormat::R32Float)
        );
        assert_eq!(
            map_host_format(TextureFormat::RGFloat, ColorProfile::Linear),
            Some(wgpu::TextureFormat::Rg32Float)
        );
    }

    #[test]
    fn unsupported_or_decode_only_host_formats_return_none() {
        for format in [
            TextureFormat::Unknown,
            TextureFormat::RGB24,
            TextureFormat::RGB565,
            TextureFormat::BGR565,
            TextureFormat::ASTC4x4,
            TextureFormat::ASTC5x5,
            TextureFormat::ASTC6x6,
            TextureFormat::ASTC8x8,
            TextureFormat::ASTC10x10,
            TextureFormat::ASTC12x12,
        ] {
            assert_eq!(
                map_host_format(format, ColorProfile::Linear),
                None,
                "{format:?}"
            );
        }
    }

    #[test]
    fn compressed_format_feature_classifiers_match_wgpu_formats() {
        assert!(format_required_bc(wgpu::TextureFormat::Bc1RgbaUnorm));
        assert!(format_required_bc(wgpu::TextureFormat::Bc7RgbaUnormSrgb));
        assert!(!format_required_bc(wgpu::TextureFormat::Etc2Rgb8Unorm));

        assert!(format_required_etc2(wgpu::TextureFormat::Etc2Rgb8Unorm));
        assert!(format_required_etc2(
            wgpu::TextureFormat::Etc2Rgba8UnormSrgb
        ));
        assert!(!format_required_etc2(wgpu::TextureFormat::Bc1RgbaUnorm));

        assert!(format_required_astc(wgpu::TextureFormat::Astc {
            block: wgpu::AstcBlock::B4x4,
            channel: wgpu::AstcChannel::Unorm,
        }));
        assert!(format_required_astc(wgpu::TextureFormat::Astc {
            block: wgpu::AstcBlock::B8x8,
            channel: wgpu::AstcChannel::UnormSrgb,
        }));
        assert!(!format_required_astc(wgpu::TextureFormat::Rgba8Unorm));
    }

    #[test]
    fn supported_host_formats_excludes_unknown_and_includes_decode_formats() {
        let formats = supported_host_formats_for_init();
        assert!(!formats.contains(&TextureFormat::Unknown));
        for format in [
            TextureFormat::RGB24,
            TextureFormat::RGBA32,
            TextureFormat::BC1,
            TextureFormat::ETC2RGBA8,
            TextureFormat::ASTC12x12,
        ] {
            assert!(formats.contains(&format), "{format:?}");
        }
    }
}
