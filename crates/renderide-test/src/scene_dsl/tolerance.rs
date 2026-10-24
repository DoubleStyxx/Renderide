//! Multi-criterion image-comparison tolerance for golden-image validation.
//!
//! A [`Tolerance`] aggregates SSIM-Y and pixel-difference comparison criteria while optional
//! image-coverage gates guard against clear-only or nearly blank captures being accepted just
//! because they compare closely to a stale golden.

use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Combination operator for image-comparison criteria.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combine {
    /// All specified comparison criteria must pass.
    #[default]
    And,
    /// Any specified comparison criterion passing is sufficient.
    Or,
}

/// Multi-criterion comparison tolerance.
///
/// Any criterion left as `None` is omitted from evaluation. SSIM and pixel-diff criteria are
/// combined with [`Self::combine`]. Coverage criteria are always treated as gates: if present,
/// they must pass in addition to the comparison criteria.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tolerance {
    /// Minimum SSIM-Y score (`image_compare::rgba_hybrid_compare`); range `0.0..=1.0`.
    pub ssim_min: Option<f64>,
    /// Maximum allowed per-channel absolute difference (0..=255). Pixels exceeding this
    /// threshold count toward [`Self::max_failing_pixel_fraction`].
    pub max_abs_diff: Option<u8>,
    /// Maximum fraction (`0.0..=1.0`) of pixels allowed to exceed [`Self::max_abs_diff`].
    /// Requires [`Self::max_abs_diff`] to be set.
    pub max_failing_pixel_fraction: Option<f64>,
    /// Minimum actual-image luma range. This catches captures that are too flat to be useful.
    pub min_luma_range: Option<u8>,
    /// Minimum number of unique RGB colors in the actual image.
    pub min_unique_colors: Option<usize>,
    /// Minimum fraction of pixels that must differ from the dominant RGB color in the actual
    /// image. This catches mostly-clear captures with a small amount of noise.
    pub min_non_background_pixel_fraction: Option<f64>,
    /// Per-rectangle color gates. Each one compares the mean of pixels that are not that
    /// image's dominant color. These are required, like coverage, and are not part of
    /// [`Self::combine`].
    #[serde(default)]
    pub region_colors: Vec<RegionColor>,
    /// Operator combining the specified image-comparison criteria.
    pub combine: Combine,
}

/// One rectangle whose non-dominant pixels must stay close to the golden's mean color.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
/// `y` ends are exclusive: 77..150 and 162..234. A channel error of 24 passes the golden
/// against itself and rejects a flat `(63,63,63)` disk.
pub fn pbs_lit_region_colors() -> Vec<RegionColor> {
    const MAX_DELTA: f64 = 24.0;
    vec![
        RegionColor {
            x: 0,
            y: 77,
            width: 160,
            height: 73,
            max_mean_channel_delta: MAX_DELTA,
        },
        RegionColor {
            x: 160,
            y: 77,
            width: 160,
            height: 73,
            max_mean_channel_delta: MAX_DELTA,
        },
        RegionColor {
            x: 0,
            y: 162,
            width: 160,
            height: 72,
            max_mean_channel_delta: MAX_DELTA,
        },
        RegionColor {
            x: 160,
            y: 162,
            width: 160,
            height: 72,
            max_mean_channel_delta: MAX_DELTA,
        },
    ]
}

impl Tolerance {
    /// SSIM-only tolerance: `ssim >= min`.
    pub fn ssim_at_least(min: f64) -> Self {
        Self {
            ssim_min: Some(min),
            ..Self::default()
        }
    }

    /// Absolute-difference tolerance: at most `fraction` of pixels exceed `max_abs_diff` per
    /// channel.
    pub fn pixel_diff(max_abs_diff: u8, fraction: f64) -> Self {
        Self {
            max_abs_diff: Some(max_abs_diff),
            max_failing_pixel_fraction: Some(fraction),
            ..Self::default()
        }
    }

    /// Evaluates this tolerance against the supplied image pair, returning every criterion's
    /// computed value plus an aggregate pass/fail flag.
    pub fn evaluate(
        &self,
        actual: &RgbaImage,
        golden: &RgbaImage,
    ) -> Result<ToleranceEvaluation, String> {
        if actual.dimensions() != golden.dimensions() {
            return Err(format!(
                "dimensions differ: actual {:?} vs golden {:?}",
                actual.dimensions(),
                golden.dimensions()
            ));
        }

        let mut criteria: Vec<CriterionResult> = Vec::new();
        let mut comparison_criteria: Vec<CriterionResult> = Vec::new();
        let mut coverage_criteria: Vec<CriterionResult> = Vec::new();

        let ssim = if let Some(min) = self.ssim_min {
            let score = image_compare::rgba_hybrid_compare(actual, golden)
                .map_err(|e| format!("rgba_hybrid_compare: {e:?}"))?
                .score;
            let criterion = CriterionResult::Ssim {
                score,
                threshold: min,
                passed: score >= min,
            };
            comparison_criteria.push(criterion.clone());
            criteria.push(criterion);
            Some(score)
        } else {
            None
        };

        let (max_abs, failing_fraction) = match (self.max_abs_diff, self.max_failing_pixel_fraction)
        {
            (Some(threshold), fraction_opt) => {
                let stats = compute_pixel_diff_stats(actual, golden, threshold);
                let criterion = if let Some(fraction) = fraction_opt {
                    CriterionResult::PixelDiff {
                        max_abs_diff: threshold,
                        failing_fraction: stats.failing_fraction,
                        max_failing_fraction: fraction,
                        passed: stats.failing_fraction <= fraction,
                    }
                } else {
                    CriterionResult::PixelDiff {
                        max_abs_diff: threshold,
                        failing_fraction: stats.failing_fraction,
                        max_failing_fraction: 0.0,
                        passed: stats.failing_fraction == 0.0,
                    }
                };
                comparison_criteria.push(criterion.clone());
                criteria.push(criterion);
                (Some(stats.max_abs_diff), Some(stats.failing_fraction))
            }
            (None, Some(_)) => {
                return Err(
                    "tolerance configured max_failing_pixel_fraction without max_abs_diff"
                        .to_string(),
                );
            }
            _ => (None, None),
        };

        let actual_coverage = compute_image_coverage_stats(actual);
        let golden_coverage = compute_image_coverage_stats(golden);
        if let Some(criterion) = coverage_criterion(self, &actual_coverage) {
            coverage_criteria.push(criterion.clone());
            criteria.push(criterion);
        }

        let color_criteria = evaluate_region_colors(&self.region_colors, actual, golden)?;
        criteria.extend(color_criteria.iter().cloned());

        let coverage_passed = coverage_criteria.iter().all(CriterionResult::passed);
        let colors_passed = color_criteria.iter().all(CriterionResult::passed);
        let gates_passed = coverage_passed && colors_passed;
        let gates_present = !coverage_criteria.is_empty() || !color_criteria.is_empty();
        let passed = if comparison_criteria.is_empty() {
            gates_present && gates_passed
        } else {
            aggregate(&comparison_criteria, self.combine) && gates_passed
        };

        Ok(ToleranceEvaluation {
            ssim,
            max_abs_diff_observed: max_abs,
            failing_pixel_fraction: failing_fraction,
            actual_coverage,
            golden_coverage,
            criteria,
            passed,
            combine: self.combine,
        })
    }
}

/// Per-criterion result returned by [`Tolerance::evaluate`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CriterionResult {
    /// SSIM-Y criterion outcome.
    Ssim {
        /// Computed SSIM score (`0.0..=1.0`).
        score: f64,
        /// Required minimum.
        threshold: f64,
        /// Whether the criterion passed.
        passed: bool,
    },
    /// Pixel-difference criterion outcome.
    PixelDiff {
        /// Per-channel absolute difference threshold.
        max_abs_diff: u8,
        /// Observed fraction of pixels exceeding the threshold (`0.0..=1.0`).
        failing_fraction: f64,
        /// Allowed fraction of failing pixels.
        max_failing_fraction: f64,
        /// Whether the criterion passed.
        passed: bool,
    },
    /// Actual-image coverage gate outcome.
    ImageCoverage {
        /// Observed actual-image luma range.
        luma_range: u8,
        /// Required minimum luma range, when configured.
        min_luma_range: Option<u8>,
        /// Observed count of unique RGB colors.
        unique_rgb_colors: usize,
        /// Required minimum unique RGB colors, when configured.
        min_unique_colors: Option<usize>,
        /// Fraction of pixels that differ from the dominant RGB color.
        non_background_pixel_fraction: f64,
        /// Required minimum non-background fraction, when configured.
        min_non_background_pixel_fraction: Option<f64>,
        /// Whether the criterion passed.
        passed: bool,
    },
    /// Mean color of non-dominant pixels in one rectangle, compared with the golden.
    RegionColor {
        /// Left edge, in pixels.
        x: u32,
        /// Top edge, in pixels.
        y: u32,
        /// Width, in pixels.
        width: u32,
        /// Height, in pixels.
        height: u32,
        /// Actual mean red channel.
        mean_r: f64,
        /// Actual mean green channel.
        mean_g: f64,
        /// Actual mean blue channel.
        mean_b: f64,
        /// Golden mean red channel.
        golden_mean_r: f64,
        /// Golden mean green channel.
        golden_mean_g: f64,
        /// Golden mean blue channel.
        golden_mean_b: f64,
        /// Largest absolute per-channel mean difference.
        max_channel_delta: f64,
        /// Allowed per-channel mean difference.
        max_mean_channel_delta: f64,
        /// Whether the criterion passed.
        passed: bool,
    },
}

impl CriterionResult {
    /// Returns whether this individual criterion was satisfied.
    pub fn passed(&self) -> bool {
        match self {
            CriterionResult::Ssim { passed, .. }
            | CriterionResult::PixelDiff { passed, .. }
            | CriterionResult::ImageCoverage { passed, .. }
            | CriterionResult::RegionColor { passed, .. } => *passed,
        }
    }
}

/// Basic coverage statistics emitted in per-case JSON reports.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageCoverageStats {
    /// Minimum observed luma value.
    pub luma_min: u8,
    /// Maximum observed luma value.
    pub luma_max: u8,
    /// `luma_max - luma_min`.
    pub luma_range: u8,
    /// Count of unique RGB colors.
    pub unique_rgb_colors: usize,
    /// Fraction of pixels using the most common RGB color.
    pub dominant_rgb_fraction: f64,
    /// Fraction of pixels not using the most common RGB color.
    pub non_background_pixel_fraction: f64,
    /// Fraction of pixels whose alpha channel is zero.
    pub transparent_pixel_fraction: f64,
}

/// Outcome of evaluating a [`Tolerance`] against a pair of images.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToleranceEvaluation {
    /// Computed SSIM score, when the SSIM criterion was specified.
    pub ssim: Option<f64>,
    /// Maximum observed per-channel absolute difference, when the pixel-diff criterion ran.
    pub max_abs_diff_observed: Option<u8>,
    /// Observed failing-pixel fraction, when the pixel-diff criterion ran.
    pub failing_pixel_fraction: Option<f64>,
    /// Coverage statistics for the actual render.
    pub actual_coverage: ImageCoverageStats,
    /// Coverage statistics for the committed golden.
    pub golden_coverage: ImageCoverageStats,
    /// Per-criterion details.
    pub criteria: Vec<CriterionResult>,
    /// Aggregate pass/fail.
    pub passed: bool,
    /// Combine operator used to aggregate comparison criteria.
    pub combine: Combine,
}

struct PixelDiffStats {
    failing_fraction: f64,
    max_abs_diff: u8,
}

fn compute_pixel_diff_stats(
    actual: &RgbaImage,
    golden: &RgbaImage,
    threshold: u8,
) -> PixelDiffStats {
    let total = (actual.width() as u64) * (actual.height() as u64);
    if total == 0 {
        return PixelDiffStats {
            failing_fraction: 0.0,
            max_abs_diff: 0,
        };
    }
    let mut failing: u64 = 0;
    let mut max_abs: u8 = 0;
    let actual_bytes = actual.as_raw();
    let golden_bytes = golden.as_raw();
    for (a_pix, g_pix) in actual_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .zip(golden_bytes.as_chunks::<4>().0)
    {
        let mut pixel_max: u8 = 0;
        for i in 0..3 {
            let diff = a_pix[i].abs_diff(g_pix[i]);
            if diff > pixel_max {
                pixel_max = diff;
            }
        }
        if pixel_max > max_abs {
            max_abs = pixel_max;
        }
        if pixel_max > threshold {
            failing += 1;
        }
    }
    PixelDiffStats {
        failing_fraction: failing as f64 / total as f64,
        max_abs_diff: max_abs,
    }
}

fn compute_image_coverage_stats(image: &RgbaImage) -> ImageCoverageStats {
    let total = image.width() as usize * image.height() as usize;
    if total == 0 {
        return ImageCoverageStats {
            luma_min: 0,
            luma_max: 0,
            luma_range: 0,
            unique_rgb_colors: 0,
            dominant_rgb_fraction: 0.0,
            non_background_pixel_fraction: 0.0,
            transparent_pixel_fraction: 0.0,
        };
    }

    let mut luma_min = u8::MAX;
    let mut luma_max = u8::MIN;
    let mut colors = Vec::with_capacity(total);
    let mut transparent = 0usize;
    for pixel in image.as_raw().as_chunks::<4>().0 {
        let luma = luma_u8(pixel[0], pixel[1], pixel[2]);
        luma_min = luma_min.min(luma);
        luma_max = luma_max.max(luma);
        colors.push(rgb_key(pixel[0], pixel[1], pixel[2]));
        if pixel[3] == 0 {
            transparent += 1;
        }
    }

    colors.sort_unstable();
    let mut unique = 0usize;
    let mut dominant = 0usize;
    let mut run = 0usize;
    let mut previous = None;
    for color in colors {
        if Some(color) == previous {
            run += 1;
        } else {
            if previous.is_some() {
                dominant = dominant.max(run);
            }
            unique += 1;
            previous = Some(color);
            run = 1;
        }
    }
    dominant = dominant.max(run);
    let dominant_fraction = dominant as f64 / total as f64;

    ImageCoverageStats {
        luma_min,
        luma_max,
        luma_range: luma_max.saturating_sub(luma_min),
        unique_rgb_colors: unique,
        dominant_rgb_fraction: dominant_fraction,
        non_background_pixel_fraction: 1.0 - dominant_fraction,
        transparent_pixel_fraction: transparent as f64 / total as f64,
    }
}

fn luma_u8(r: u8, g: u8, b: u8) -> u8 {
    (((r as u16 * 77) + (g as u16 * 150) + (b as u16 * 29)) >> 8) as u8
}

fn rgb_key(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn coverage_criterion(
    tolerance: &Tolerance,
    stats: &ImageCoverageStats,
) -> Option<CriterionResult> {
    if !coverage_requested(tolerance) {
        return None;
    }
    let min_luma_range = tolerance.min_luma_range;
    let min_unique_colors = tolerance.min_unique_colors;
    let min_non_background_pixel_fraction = tolerance.min_non_background_pixel_fraction;
    let passed = min_luma_range.is_none_or(|min| stats.luma_range >= min)
        && min_unique_colors.is_none_or(|min| stats.unique_rgb_colors >= min)
        && min_non_background_pixel_fraction
            .is_none_or(|min| stats.non_background_pixel_fraction >= min);
    Some(CriterionResult::ImageCoverage {
        luma_range: stats.luma_range,
        min_luma_range,
        unique_rgb_colors: stats.unique_rgb_colors,
        min_unique_colors,
        non_background_pixel_fraction: stats.non_background_pixel_fraction,
        min_non_background_pixel_fraction,
        passed,
    })
}

fn evaluate_region_colors(
    regions: &[RegionColor],
    actual: &RgbaImage,
    golden: &RgbaImage,
) -> Result<Vec<CriterionResult>, String> {
    let actual_dominant = dominant_rgb_key(actual);
    let golden_dominant = dominant_rgb_key(golden);
    let mut color_criteria = Vec::with_capacity(regions.len());
    for region in regions {
        let actual_mean = non_dominant_mean(actual, actual_dominant, *region)?;
        let golden_mean = non_dominant_mean(golden, golden_dominant, *region)?;
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
        color_criteria.push(CriterionResult::RegionColor {
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
    Ok(color_criteria)
}

fn dominant_rgb_key(image: &RgbaImage) -> u32 {
    let mut colors = Vec::with_capacity(image.width() as usize * image.height() as usize);
    for pixel in image.as_raw().as_chunks::<4>().0 {
        colors.push(rgb_key(pixel[0], pixel[1], pixel[2]));
    }
    colors.sort_unstable();
    let mut dominant_key = 0u32;
    let mut dominant = 0usize;
    let mut run = 0usize;
    let mut previous = None;
    for color in colors {
        if Some(color) == previous {
            run += 1;
        } else {
            if run > dominant {
                dominant = run;
                dominant_key = previous.unwrap_or(color);
            }
            previous = Some(color);
            run = 1;
        }
    }
    if run > dominant {
        dominant_key = previous.unwrap_or(0);
    }
    dominant_key
}

fn non_dominant_mean(
    image: &RgbaImage,
    dominant: u32,
    region: RegionColor,
) -> Result<Option<[f64; 3]>, String> {
    let (width, height) = image.dimensions();
    let x1 = region
        .x
        .checked_add(region.width)
        .ok_or_else(|| format!("region x {} + width {} overflows", region.x, region.width))?;
    let y1 = region
        .y
        .checked_add(region.height)
        .ok_or_else(|| format!("region y {} + height {} overflows", region.y, region.height))?;
    if x1 > width || y1 > height {
        return Err(format!(
            "region {},{} {}x{} is outside {}x{}",
            region.x, region.y, region.width, region.height, width, height
        ));
    }
    let mut count = 0u64;
    let mut sum = [0u64; 3];
    for y in region.y..y1 {
        for x in region.x..x1 {
            let pixel = image.get_pixel(x, y);
            if rgb_key(pixel[0], pixel[1], pixel[2]) != dominant {
                sum[0] += u64::from(pixel[0]);
                sum[1] += u64::from(pixel[1]);
                sum[2] += u64::from(pixel[2]);
                count += 1;
            }
        }
    }
    if count == 0 {
        return Ok(None);
    }
    let n = count as f64;
    Ok(Some([
        sum[0] as f64 / n,
        sum[1] as f64 / n,
        sum[2] as f64 / n,
    ]))
}

fn max_channel_delta(actual: [f64; 3], golden: [f64; 3]) -> f64 {
    actual
        .iter()
        .zip(golden)
        .map(|(actual, golden)| (actual - golden).abs())
        .fold(0.0, f64::max)
}

fn coverage_requested(tolerance: &Tolerance) -> bool {
    tolerance.min_luma_range.is_some()
        || tolerance.min_unique_colors.is_some()
        || tolerance.min_non_background_pixel_fraction.is_some()
}

fn aggregate(criteria: &[CriterionResult], combine: Combine) -> bool {
    if criteria.is_empty() {
        return false;
    }
    match combine {
        Combine::And => criteria.iter().all(CriterionResult::passed),
        Combine::Or => criteria.iter().any(CriterionResult::passed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(width, height);
        for p in img.pixels_mut() {
            *p = Rgba(color);
        }
        img
    }

    #[test]
    fn ssim_only_passes_for_identical_image() {
        let img = solid(8, 8, [12, 90, 200, 255]);
        let tol = Tolerance::ssim_at_least(0.95);
        let eval = tol.evaluate(&img, &img).expect("evaluate identical");
        assert!(eval.passed);
        assert!(eval.ssim.unwrap() >= 0.99);
    }

    #[test]
    fn pixel_diff_counts_failing_fraction() {
        let mut a = solid(4, 4, [10, 10, 10, 255]);
        let g = solid(4, 4, [10, 10, 10, 255]);
        a.put_pixel(0, 0, Rgba([200, 10, 10, 255]));
        let tol = Tolerance::pixel_diff(5, 0.05);
        let eval = tol.evaluate(&a, &g).expect("evaluate");
        assert_eq!(eval.failing_pixel_fraction.unwrap(), 1.0 / 16.0);
        assert!(!eval.passed, "1/16 == 6.25% > 5%");
    }

    #[test]
    fn or_combine_accepts_when_either_comparison_passes() {
        let img = solid(64, 64, [50, 50, 50, 255]);
        let mut other = img.clone();
        other.put_pixel(0, 0, Rgba([180, 50, 50, 255]));

        let mut tol = Tolerance::pixel_diff(5, 0.0);
        tol.ssim_min = Some(0.5);
        tol.combine = Combine::Or;

        let eval = tol.evaluate(&img, &other).expect("evaluate");
        assert!(
            eval.passed,
            "OR should accept when SSIM passes even if pixel-diff fails"
        );
    }

    #[test]
    fn coverage_gate_rejects_flat_actual_even_when_images_match() {
        let img = solid(16, 16, [50, 50, 50, 255]);
        let tol = Tolerance {
            ssim_min: Some(0.5),
            min_luma_range: Some(4),
            min_unique_colors: Some(2),
            min_non_background_pixel_fraction: Some(0.01),
            combine: Combine::Or,
            ..Tolerance::default()
        };
        let eval = tol.evaluate(&img, &img).expect("evaluate");
        assert!(!eval.passed);
        assert_eq!(eval.actual_coverage.luma_range, 0);
        assert_eq!(eval.actual_coverage.unique_rgb_colors, 1);
    }

    #[test]
    fn coverage_stats_count_dominant_color_fraction() {
        let mut img = solid(4, 4, [0, 0, 0, 255]);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 255, 0, 0]));
        let stats = compute_image_coverage_stats(&img);
        assert_eq!(stats.unique_rgb_colors, 3);
        assert_eq!(stats.dominant_rgb_fraction, 14.0 / 16.0);
        assert_eq!(stats.non_background_pixel_fraction, 2.0 / 16.0);
        assert_eq!(stats.transparent_pixel_fraction, 1.0 / 16.0);
    }

    #[test]
    fn dimension_mismatch_returns_err() {
        let a = solid(4, 4, [0, 0, 0, 255]);
        let b = solid(8, 8, [0, 0, 0, 255]);
        let err = Tolerance::ssim_at_least(0.5).evaluate(&a, &b).unwrap_err();
        assert!(err.contains("dimensions differ"));
    }

    #[test]
    fn empty_tolerance_fails_vacuously() {
        let img = solid(2, 2, [1, 2, 3, 255]);
        let tol = Tolerance::default();
        let eval = tol.evaluate(&img, &img).expect("evaluate");
        assert!(!eval.passed, "empty criteria should not be vacuously true");
    }

    #[test]
    fn missing_max_abs_with_fraction_returns_err() {
        let img = solid(2, 2, [1, 2, 3, 255]);
        let tol = Tolerance {
            max_failing_pixel_fraction: Some(0.1),
            ..Tolerance::default()
        };
        let err = tol.evaluate(&img, &img).unwrap_err();
        assert!(err.contains("max_failing_pixel_fraction"));
    }

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
        }
    }

    fn load_pbs_golden() -> RgbaImage {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("goldens/pbs_lit_material_matrix.png");
        image::open(path).expect("pbs golden").to_rgba8()
    }

    /// Same disk mask as the golden, collapsed to `(63,63,63)` plus a short gray fringe.
    fn flat_gray_with_fringe(golden: &RgbaImage) -> RgbaImage {
        let dominant = dominant_rgb_key(golden);
        let mut image = golden.clone();
        let mut extras = 0u8;
        for pixel in image.pixels_mut() {
            if rgb_key(pixel[0], pixel[1], pixel[2]) == dominant {
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

    fn region_color(eval: &ToleranceEvaluation, x: u32, y: u32) -> (f64, f64, f64, f64, bool) {
        eval.criteria
            .iter()
            .find_map(|criterion| match criterion {
                CriterionResult::RegionColor {
                    x: cx,
                    y: cy,
                    mean_r,
                    mean_g,
                    mean_b,
                    max_channel_delta,
                    passed,
                    ..
                } if *cx == x && *cy == y => {
                    Some((*mean_r, *mean_g, *mean_b, *max_channel_delta, *passed))
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
        let (_, _, _, max_channel_delta, passed) = region_color(&eval, 0, 77);
        assert!(passed);
        assert_eq!(max_channel_delta, 0.0);
    }

    #[test]
    fn region_color_rejects_flat_gray_when_or_and_coverage_pass() {
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
        let (mean_r, mean_g, mean_b, _, passed) = region_color(&eval, 0, 77);
        assert!(!passed);
        assert!((mean_r - 63.0).abs() < 2.0);
        assert!((mean_g - 63.0).abs() < 2.0);
        assert!((mean_b - 63.0).abs() < 2.0);
    }
}
