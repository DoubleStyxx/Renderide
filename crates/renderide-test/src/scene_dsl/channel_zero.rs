//! Channel-zero and per-region colour gates.
//!
//! Per-region colour uses [`super::region_stats`] on the default 2×2 grid: the
//! mean of every pixel in the cell, background included. Channel-zero looks at
//! the same cells, but only at pixels that are not the golden's dominant colour.
//!
//! Thresholds are conservative. A correct lavapipe frame sits far below them.
//! The #232 WARP captures do not.
//!
//! * [`REGION_MEAN_LIMIT`] is 16. A correct non-dominant band error of 8, spread
//!   across a cell that is mostly background, moves the cell mean by about 1.
//!   Sixteen is an order of magnitude above that and still below the warm-tint
//!   cell (about 28) and the G=0 cell (about 41).
//! * [`GOLDEN_CHANNEL_MIN`] is 32. The golden channel has to be meaningfully
//!   above zero before a dark actual channel is a dropped channel. A mean of 32
//!   is dim but not noise.
//! * [`NEAR_ZERO_MAX`] is 2. Exact 0 counts, and so do 1 and 2, so one step of
//!   quantization does not hide a dropped channel. A warm tint whose green mean
//!   is about 45 does not count.
//! * [`NEAR_ZERO_FRACTION`] is 0.75. Most non-background pixels in the cell have
//!   to be near zero. The G=0 frame is about 0.81 overall and 0.89 to 1.00 in
//!   the cells that fail.
//! * [`MIN_NON_BACKGROUND`] is 64. A handful of dark pixels is not a region.

use image::RgbaImage;

use super::tolerance::CriterionResult;

/// Maximum allowed absolute RGB mean gap on one [`super::region_stats`] cell.
pub(crate) const REGION_MEAN_LIMIT: f64 = 16.0;

/// Golden non-background mean at or above this is meaningfully above zero.
pub(crate) const GOLDEN_CHANNEL_MIN: f64 = 32.0;

/// Actual channel values at or below this are near zero. Exact 0 is included.
pub(crate) const NEAR_ZERO_MAX: u8 = 2;

/// Fraction of non-background pixels that must be near zero.
pub(crate) const NEAR_ZERO_FRACTION: f64 = 0.75;

/// Non-background pixels required before a cell can fail channel-zero.
pub(crate) const MIN_NON_BACKGROUND: u64 = 64;

const CHANNEL_NAMES: [&str; 3] = ["R", "G", "B"];

/// Appends the gates when `enabled`. Returns whether they passed.
///
/// An empty tolerance stays empty: these gates do not make it vacuously pass.
pub(crate) fn apply(
    enabled: bool,
    actual: &RgbaImage,
    golden: &RgbaImage,
    criteria: &mut Vec<CriterionResult>,
) -> Result<bool, String> {
    if !enabled {
        return Ok(true);
    }
    let extra = gates(actual, golden)?;
    let passed = extra.iter().all(CriterionResult::passed);
    criteria.extend(extra);
    Ok(passed)
}

/// Region-mean criteria for every cell, plus a channel-zero criterion for each failure.
pub(crate) fn gates(
    actual: &RgbaImage,
    golden: &RgbaImage,
) -> Result<Vec<CriterionResult>, String> {
    if actual.dimensions() != golden.dimensions() {
        return Err(format!(
            "dimensions differ: actual {:?} vs golden {:?}",
            actual.dimensions(),
            golden.dimensions()
        ));
    }
    let actual_cells = super::region_stats::stats(actual);
    let golden_cells = super::region_stats::stats(golden);
    let dominant = super::band_count::dominant_rgb_key(golden);
    let mut criteria = Vec::with_capacity(actual_cells.len() * 2);
    for (actual_cell, golden_cell) in actual_cells.iter().zip(golden_cells.iter()) {
        criteria.push(region_mean(actual_cell, golden_cell));
        criteria.extend(channel_zero_failures(actual, golden, actual_cell, dominant));
    }
    Ok(criteria)
}

/// `case failed tolerance` plus the most specific gate failure, if any.
pub(crate) fn failure_message(criteria: &[CriterionResult]) -> Option<String> {
    let detail = channel_zero_detail(criteria).or_else(|| region_mean_detail(criteria))?;
    Some(format!("case failed tolerance; {detail}"))
}

fn channel_zero_detail(criteria: &[CriterionResult]) -> Option<String> {
    let mut best: Option<(&CriterionResult, f64, u8)> = None;
    for criterion in criteria {
        let CriterionResult::ChannelZero {
            passed: false,
            channel,
            near_zero_fraction,
            ..
        } = criterion
        else {
            continue;
        };
        let prefer = match best {
            None => true,
            Some((_, fraction, previous)) => {
                (*channel == 1 && previous != 1)
                    || (*channel == previous && *near_zero_fraction > fraction)
            }
        };
        if prefer {
            best = Some((criterion, *near_zero_fraction, *channel));
        }
    }
    let (criterion, _, _) = best?;
    let CriterionResult::ChannelZero {
        channel,
        column,
        row,
        golden_mean,
        near_zero_fraction,
        ..
    } = criterion
    else {
        return None;
    };
    Some(format!(
        "channel-zero: {} is near 0 on {:.0}% of non-background pixels in region column {column} row {row} while the golden mean is {golden_mean:.0}",
        CHANNEL_NAMES[usize::from(*channel)],
        near_zero_fraction * 100.0,
    ))
}

fn region_mean_detail(criteria: &[CriterionResult]) -> Option<String> {
    criteria.iter().find_map(|criterion| match criterion {
        CriterionResult::RegionMean {
            passed: false,
            column,
            row,
            max_channel_delta,
            ..
        } => Some(format!(
            "region-mean: region column {column} row {row} differs from the golden by {max_channel_delta:.1}, above the limit of {REGION_MEAN_LIMIT:.0}"
        )),
        _ => None,
    })
}

fn region_mean(
    actual: &super::region_stats::RegionStats,
    golden: &super::region_stats::RegionStats,
) -> CriterionResult {
    let mut max_channel_delta = 0.0f64;
    for channel in 0..3 {
        max_channel_delta =
            max_channel_delta.max((actual.mean[channel] - golden.mean[channel]).abs());
    }
    CriterionResult::RegionMean {
        column: actual.column,
        row: actual.row,
        max_channel_delta,
        max_mean_channel_delta: REGION_MEAN_LIMIT,
        passed: max_channel_delta <= REGION_MEAN_LIMIT,
    }
}

fn channel_zero_failures(
    actual: &RgbaImage,
    golden: &RgbaImage,
    cell: &super::region_stats::RegionStats,
    dominant: u32,
) -> Vec<CriterionResult> {
    let mut sum = [0u64; 3];
    let mut near_zero = [0u64; 3];
    let mut count = 0u64;
    let x1 = cell.x.saturating_add(cell.width).min(golden.width());
    let y1 = cell.y.saturating_add(cell.height).min(golden.height());
    for y in cell.y..y1 {
        for x in cell.x..x1 {
            let golden_px = golden.get_pixel(x, y).0;
            if super::band_count::rgb_key(golden_px[0], golden_px[1], golden_px[2]) == dominant {
                continue;
            }
            let actual_px = actual.get_pixel(x, y).0;
            count = count.saturating_add(1);
            for channel in 0..3 {
                sum[channel] = sum[channel].saturating_add(u64::from(golden_px[channel]));
                if actual_px[channel] <= NEAR_ZERO_MAX {
                    near_zero[channel] = near_zero[channel].saturating_add(1);
                }
            }
        }
    }
    if count < MIN_NON_BACKGROUND {
        return Vec::new();
    }
    let mut failures = Vec::new();
    let count_f = count as f64;
    for channel in 0..3 {
        let golden_mean = sum[channel] as f64 / count_f;
        let fraction = near_zero[channel] as f64 / count_f;
        if golden_mean >= GOLDEN_CHANNEL_MIN && fraction >= NEAR_ZERO_FRACTION {
            failures.push(CriterionResult::ChannelZero {
                channel: u8::try_from(channel).unwrap_or(0),
                column: cell.column,
                row: cell.row,
                golden_mean,
                near_zero_fraction: fraction,
                passed: false,
            });
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn load(path: &std::path::Path) -> RgbaImage {
        image::open(path)
            .unwrap_or_else(|err| panic!("open {}: {err}", path.display()))
            .to_rgba8()
    }

    fn fixture(name: &str) -> RgbaImage {
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/warp_channel")
                .join(name),
        )
    }

    fn golden(name: &str) -> RgbaImage {
        load(&crate::scene_dsl::cases::default_goldens_dir().join(name))
    }

    #[test]
    fn every_lavapipe_golden_passes_against_itself() {
        let dir = crate::scene_dsl::cases::default_goldens_dir();
        let mut seen = 0usize;
        for entry in std::fs::read_dir(&dir).expect("goldens") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("png") {
                continue;
            }
            let image = load(&path);
            let criteria = gates(&image, &image).expect("gates");
            assert!(
                criteria.iter().all(CriterionResult::passed),
                "{}",
                path.display()
            );
            seen += 1;
        }
        assert!(seen >= 15, "expected the committed golden set, saw {seen}");
    }

    #[test]
    fn g0_frame_fails_channel_zero_and_the_old_gates_still_pass() {
        let actual = fixture("negative_light_physically_based_g0.png");
        let golden = golden("negative_light_physically_based.png");
        let case = crate::scene_dsl::cases::negative_light_physically_based();
        let eval = case.tolerance.evaluate(&actual, &golden).expect("evaluate");
        assert!(
            eval.criteria.iter().any(|criterion| {
                matches!(criterion, CriterionResult::Ssim { passed: true, .. })
            }),
            "SSIM still passes; run 41145831835 recorded 0.834 against 0.78"
        );
        assert!(
            eval.criteria.iter().any(|criterion| {
                matches!(criterion, CriterionResult::PixelDiff { passed: true, .. })
            }),
            "pixel diff still passes; run 41145831835 recorded 0.150 against 0.22"
        );
        assert!(
            eval.criteria.iter().any(|criterion| {
                matches!(
                    criterion,
                    CriterionResult::ChannelZero {
                        passed: false,
                        channel: 1,
                        ..
                    }
                )
            }),
            "green is the dropped channel on 4bd5eef0"
        );
        assert!(!eval.passed);
        let message = failure_message(&eval.criteria).expect("reason");
        assert!(message.contains("case failed tolerance"), "{message}");
        assert!(message.contains("channel-zero:"), "{message}");
        assert!(message.contains(" G "), "{message}");
        let mut report = crate::scene_dsl::output::report_from_evaluation(&case, eval);
        crate::scene_dsl::report_v2::assign_outcome(&mut report);
        assert_eq!(report.reason.as_deref(), Some("channel-zero"));
        assert!(!report.reason_text.as_deref().unwrap_or("").is_empty());
    }

    #[test]
    fn warm_tint_fails_region_colour_and_not_channel_zero() {
        let actual = fixture("negative_light_physically_based_warm.png");
        let golden = golden("negative_light_physically_based.png");
        let criteria = gates(&actual, &golden).expect("gates");
        assert!(criteria.iter().any(|criterion| {
            matches!(criterion, CriterionResult::RegionMean { passed: false, .. })
        }));
        assert!(
            criteria.iter().all(|criterion| {
                !matches!(
                    criterion,
                    CriterionResult::ChannelZero { passed: false, .. }
                )
            }),
            "6ae283f1 keeps green above zero; it is a warm tint"
        );
        let case = crate::scene_dsl::cases::negative_light_physically_based();
        let eval = case.tolerance.evaluate(&actual, &golden).expect("evaluate");
        assert!(!eval.passed);
        let message = failure_message(&eval.criteria).expect("reason");
        assert!(message.contains("region-mean:"), "{message}");
        assert!(!message.contains("channel-zero:"), "{message}");
    }
}
