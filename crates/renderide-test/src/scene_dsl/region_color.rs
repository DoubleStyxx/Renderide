//! Per-band color gate for multi-object goldens.
//!
//! Each rectangle compares the mean of pixels that are not that image's dominant
//! color. The dominant color is read only when a case sets at least one rectangle.

use image::RgbaImage;

use super::tolerance::CriterionResult;

/// Largest per-channel mean error allowed in a `pbs_lit` sphere band.
///
/// A correct frame is at 0.011 or below. Gray checker bands land near 120, 17.6,
/// 183, and 179. A limit of 24 does not fail the top-right band, so this limit
/// sits under that 17.6 and over the correct-frame error.
pub const PBS_BAND_COLOR_DELTA: f64 = 8.0;

/// One rectangle whose non-dominant pixels must stay close to the golden's mean color.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct RegionColor {
    /// Left edge, in pixels.
    pub x: u32,
    /// Top edge, in pixels.
    pub y: u32,
    /// Width, in pixels.
    pub width: u32,
    /// Height, in pixels.
    pub height: u32,
    /// Maximum allowed absolute per-channel difference of the two means.
    pub max_mean_channel_delta: f64,
}

/// Sphere bands for `pbs_lit_material_matrix` on its 320×320 capture.
///
/// `y` ends are exclusive: 77..150 and 162..234.
pub fn pbs_lit_region_colors() -> Vec<RegionColor> {
    vec![
        band(0, 77, 160, 73),
        band(160, 77, 160, 73),
        band(0, 162, 160, 72),
        band(160, 162, 160, 72),
    ]
}

fn band(x: u32, y: u32, width: u32, height: u32) -> RegionColor {
    RegionColor {
        x,
        y,
        width,
        height,
        max_mean_channel_delta: PBS_BAND_COLOR_DELTA,
    }
}

/// Required per-rectangle color gates.
///
/// The caller skips this function when `regions` is empty, so the dominant color
/// is not scanned for cases that do not set a color gate.
pub(crate) fn region_color_criteria(
    actual: &RgbaImage,
    golden: &RgbaImage,
    regions: &[RegionColor],
) -> Result<Vec<CriterionResult>, String> {
    let actual_dominant = super::band_count::dominant_rgb_key(actual);
    let golden_dominant = super::band_count::dominant_rgb_key(golden);
    let mut criteria = Vec::with_capacity(regions.len());
    for region in regions {
        let actual_mean = super::band_count::non_dominant_mean(
            actual,
            actual_dominant,
            region.x,
            region.y,
            region.width,
            region.height,
        )?;
        let golden_mean = super::band_count::non_dominant_mean(
            golden,
            golden_dominant,
            region.x,
            region.y,
            region.width,
            region.height,
        )?;
        let (mean, golden_mean_value, delta, passed) = match (actual_mean, golden_mean) {
            (Some(mean), Some(golden_mean_value)) => {
                let delta = max_channel_delta(mean, golden_mean_value);
                (
                    mean,
                    golden_mean_value,
                    delta,
                    delta <= region.max_mean_channel_delta,
                )
            }
            (mean, golden_mean_value) => (
                mean.unwrap_or([0.0, 0.0, 0.0]),
                golden_mean_value.unwrap_or([0.0, 0.0, 0.0]),
                region.max_mean_channel_delta + 1.0,
                false,
            ),
        };
        criteria.push(CriterionResult::RegionColor {
            x: region.x,
            y: region.y,
            width: region.width,
            height: region.height,
            mean_r: mean[0],
            mean_g: mean[1],
            mean_b: mean[2],
            golden_mean_r: golden_mean_value[0],
            golden_mean_g: golden_mean_value[1],
            golden_mean_b: golden_mean_value[2],
            max_channel_delta: delta,
            max_mean_channel_delta: region.max_mean_channel_delta,
            passed,
        });
    }
    Ok(criteria)
}

fn max_channel_delta(actual: [f64; 3], golden: [f64; 3]) -> f64 {
    actual
        .iter()
        .zip(golden)
        .map(|(actual, golden)| (actual - golden).abs())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::super::tolerance::{Combine, CriterionResult, Tolerance};
    use super::*;

    fn pbs_color_tolerance() -> Tolerance {
        Tolerance {
            ssim_min: Some(0.78),
            max_abs_diff: Some(56),
            max_failing_pixel_fraction: Some(0.22),
            min_luma_range: Some(18),
            min_unique_colors: Some(24),
            min_non_background_pixel_fraction: Some(0.06),
            region_colors: pbs_lit_region_colors(),
            combine: Combine::Or,
            ..Tolerance::default()
        }
    }

    fn load_pbs_golden() -> RgbaImage {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("goldens/pbs_lit_material_matrix.png");
        crate::image_io::load_rgba(&path).expect("pbs golden")
    }

    /// Same disk mask as the golden, collapsed to `(63,63,63)` plus a short gray fringe.
    fn flat_gray_with_fringe(golden: &RgbaImage) -> RgbaImage {
        let dominant = super::super::band_count::dominant_rgb_key(golden);
        let mut image = golden.clone();
        let mut extras = 0u8;
        for pixel in image.pixels_mut() {
            if super::super::band_count::rgb_key(pixel[0], pixel[1], pixel[2]) == dominant {
                continue;
            }
            if extras < 30 {
                let value = 30 + extras;
                *pixel = Rgba([value, value, value, 255]);
                extras += 1;
            } else {
                *pixel = Rgba([63, 63, 63, 255]);
            }
        }
        image
    }

    fn region_color(eval: &super::super::tolerance::ToleranceEvaluation, x: u32, y: u32) -> f64 {
        eval.criteria
            .iter()
            .find_map(|criterion| match criterion {
                CriterionResult::RegionColor {
                    x: cx,
                    y: cy,
                    max_channel_delta,
                    passed,
                    ..
                } if *cx == x && *cy == y => {
                    assert!(!*passed, "band {x},{y} should fail");
                    Some(*max_channel_delta)
                }
                _ => None,
            })
            .expect("region color criterion")
    }

    #[test]
    fn region_color_passes_the_pbs_golden() {
        let golden = load_pbs_golden();
        let eval = pbs_color_tolerance()
            .evaluate(&golden, &golden)
            .expect("evaluate golden");
        assert!(eval.passed);
        let delta = eval
            .criteria
            .iter()
            .find_map(|criterion| match criterion {
                CriterionResult::RegionColor {
                    x: 0,
                    y: 77,
                    max_channel_delta,
                    passed: true,
                    ..
                } => Some(*max_channel_delta),
                _ => None,
            })
            .expect("top-left color");
        assert!(delta <= 0.011);
    }

    #[test]
    fn region_color_rejects_flat_gray_in_every_band() {
        let golden = load_pbs_golden();
        let gray = flat_gray_with_fringe(&golden);
        let eval = pbs_color_tolerance()
            .evaluate(&gray, &golden)
            .expect("evaluate gray");
        let comparison_passed = eval.criteria.iter().any(|criterion| {
            matches!(
                criterion,
                CriterionResult::Ssim { passed: true, .. }
                    | CriterionResult::PixelDiff { passed: true, .. }
            )
        });
        let coverage_passed = eval.criteria.iter().any(|criterion| {
            matches!(
                criterion,
                CriterionResult::ImageCoverage { passed: true, .. }
            )
        });
        assert!(comparison_passed);
        assert!(coverage_passed);
        assert!(!eval.passed);
        for (x, y) in [(0, 77), (160, 77), (0, 162), (160, 162)] {
            let delta = region_color(&eval, x, y);
            assert!(
                delta > PBS_BAND_COLOR_DELTA,
                "band {x},{y} delta {delta} must miss the gate, including the top-right gray band"
            );
        }
    }
}
