//! Mean, variance, and non-dominant pixel count on a default 2×2 grid.
//!
//! This is a report diagnostic. It does not change [`super::tolerance`] and it
//! does not use the sphere-band rectangles.

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use super::band_count::rgb_key;

/// Columns in the default region grid.
pub(crate) const DEFAULT_COLUMNS: u32 = 2;
/// Rows in the default region grid.
pub(crate) const DEFAULT_ROWS: u32 = 2;

/// One cell of the grid.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RegionStats {
    /// Column index, starting at 0.
    pub column: u32,
    /// Row index, starting at 0.
    pub row: u32,
    /// Left edge, in pixels.
    pub x: u32,
    /// Top edge, in pixels.
    pub y: u32,
    /// Width, in pixels. The last column absorbs the remainder.
    pub width: u32,
    /// Height, in pixels. The last row absorbs the remainder.
    pub height: u32,
    /// Mean of R, G, B, and A.
    pub mean: [f64; 4],
    /// Population variance of R, G, B, and A.
    pub variance: [f64; 4],
    /// Pixels in the cell whose RGB is not the cell's dominant color.
    pub non_dominant: u64,
}

/// Largest absolute gap between two [`stats`] results.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RegionDelta {
    /// Largest absolute mean gap across cells and channels.
    pub mean: f64,
    /// Largest absolute variance gap across cells and channels.
    pub variance: f64,
    /// Largest absolute non-dominant count gap across cells.
    pub non_dominant: u64,
}

impl RegionDelta {
    /// True when every compared channel and count matches.
    pub(crate) fn is_zero(self) -> bool {
        self.non_dominant == 0 && is_zero_f64(self.mean) && is_zero_f64(self.variance)
    }
}

/// Region stats on the default 2×2 grid.
pub(crate) fn stats(image: &RgbaImage) -> Vec<RegionStats> {
    stats_on_grid(image, DEFAULT_COLUMNS, DEFAULT_ROWS)
}

/// Region stats on a `columns` by `rows` grid. Both axes are at least one.
pub(crate) fn stats_on_grid(image: &RgbaImage, columns: u32, rows: u32) -> Vec<RegionStats> {
    let columns = columns.max(1);
    let rows = rows.max(1);
    let (width, height) = image.dimensions();
    let mut cells = Vec::with_capacity((columns * rows) as usize);
    for row in 0..rows {
        let y = height * row / rows;
        let y1 = height * (row + 1) / rows;
        for column in 0..columns {
            let x = width * column / columns;
            let x1 = width * (column + 1) / columns;
            cells.push(stats_for(image, column, row, x, y, x1 - x, y1 - y));
        }
    }
    cells
}

/// Absolute gaps between two stat lists. A length mismatch is a non-zero delta.
pub(crate) fn delta(actual: &[RegionStats], golden: &[RegionStats]) -> RegionDelta {
    let mut mean = 0.0f64;
    let mut variance = 0.0f64;
    let mut non_dominant = 0u64;
    for (left, right) in actual.iter().zip(golden.iter()) {
        for channel in 0..4 {
            mean = mean.max((left.mean[channel] - right.mean[channel]).abs());
            variance = variance.max((left.variance[channel] - right.variance[channel]).abs());
        }
        non_dominant = non_dominant.max(left.non_dominant.abs_diff(right.non_dominant));
    }
    let extra = u64::try_from(actual.len().abs_diff(golden.len())).unwrap_or(u64::MAX);
    if extra > 0 {
        mean = mean.max(1.0);
        non_dominant = non_dominant.max(extra);
    }
    RegionDelta {
        mean,
        variance,
        non_dominant,
    }
}

fn stats_for(
    image: &RgbaImage,
    column: u32,
    row: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> RegionStats {
    let mut sum = [0u64; 4];
    let mut sumsq = [0u64; 4];
    let mut keys = Vec::new();
    let x1 = x.saturating_add(width).min(image.width());
    let y1 = y.saturating_add(height).min(image.height());
    for py in y..y1 {
        for px in x..x1 {
            let pixel = image.get_pixel(px, py).0;
            for channel in 0..4 {
                let value = u64::from(pixel[channel]);
                sum[channel] += value;
                sumsq[channel] += value * value;
            }
            keys.push(rgb_key(pixel[0], pixel[1], pixel[2]));
        }
    }
    let (mean, variance) = moments(&sum, &sumsq, keys.len());
    RegionStats {
        column,
        row,
        x,
        y,
        width,
        height,
        mean,
        variance,
        non_dominant: non_dominant_count(&keys),
    }
}

fn moments(sum: &[u64; 4], sumsq: &[u64; 4], count: usize) -> ([f64; 4], [f64; 4]) {
    if count == 0 {
        return ([0.0; 4], [0.0; 4]);
    }
    let n = count as f64;
    let mut mean = [0.0; 4];
    let mut variance = [0.0; 4];
    for channel in 0..4 {
        let mean_c = sum[channel] as f64 / n;
        let second = sumsq[channel] as f64 / n;
        mean[channel] = mean_c;
        variance[channel] = (second - mean_c * mean_c).max(0.0);
    }
    (mean, variance)
}

fn non_dominant_count(keys: &[u32]) -> u64 {
    let dominant = dominant_key(keys);
    let count = keys.iter().filter(|key| **key != dominant).count();
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn dominant_key(keys: &[u32]) -> u32 {
    let mut ordered = keys.to_vec();
    ordered.sort_unstable();
    let mut dominant_key = 0u32;
    let mut dominant = 0usize;
    let mut run = 0usize;
    let mut previous = None;
    for color in ordered {
        if Some(color) == previous {
            run += 1;
            continue;
        }
        if run > dominant {
            dominant = run;
            dominant_key = previous.unwrap_or(color);
        }
        previous = Some(color);
        run = 1;
    }
    if run > dominant {
        dominant_key = previous.unwrap_or(0);
    }
    dominant_key
}

fn is_zero_f64(value: f64) -> bool {
    value.abs().to_bits() == 0
}

/// One grid cell plus the gap against the golden cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct RegionStatRow {
    /// Column index, starting at 0.
    pub column: u32,
    /// Row index, starting at 0.
    pub row: u32,
    /// Left edge, in pixels.
    pub x: u32,
    /// Top edge, in pixels.
    pub y: u32,
    /// Width, in pixels.
    pub width: u32,
    /// Height, in pixels.
    pub height: u32,
    /// Mean of R, G, B, and A in the actual image.
    pub mean: [f64; 4],
    /// Population variance of R, G, B, and A in the actual image.
    pub variance: [f64; 4],
    /// Non-dominant pixels in the actual cell.
    pub non_dominant: u64,
    /// Largest absolute mean gap versus the golden cell.
    pub mean_delta: f64,
    /// Largest absolute variance gap versus the golden cell.
    pub variance_delta: f64,
    /// Absolute non-dominant count gap versus the golden cell.
    pub non_dominant_delta: u64,
}

/// Compares the default grid and logs when any cell differs.
pub(crate) fn compare(actual: &RgbaImage, golden: &RgbaImage) -> Vec<RegionStatRow> {
    let actual_stats = stats(actual);
    let golden_stats = stats(golden);
    let gap = delta(&actual_stats, &golden_stats);
    if !gap.is_zero() {
        logger::info!(
            "region stats differ: mean={} variance={} non_dominant={}",
            gap.mean,
            gap.variance,
            gap.non_dominant
        );
    }
    actual_stats
        .iter()
        .zip(golden_stats.iter())
        .map(|(left, right)| row_from_pair(left, right))
        .collect()
}

fn row_from_pair(left: &RegionStats, right: &RegionStats) -> RegionStatRow {
    let mut mean_delta = 0.0f64;
    let mut variance_delta = 0.0f64;
    for channel in 0..4 {
        mean_delta = mean_delta.max((left.mean[channel] - right.mean[channel]).abs());
        variance_delta =
            variance_delta.max((left.variance[channel] - right.variance[channel]).abs());
    }
    RegionStatRow {
        column: left.column,
        row: left.row,
        x: left.x,
        y: left.y,
        width: left.width,
        height: left.height,
        mean: left.mean,
        variance: left.variance,
        non_dominant: left.non_dominant,
        mean_delta,
        variance_delta,
        non_dominant_delta: left.non_dominant.abs_diff(right.non_dominant),
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    #[test]
    fn pbs_lit_golden_against_itself_has_delta_zero() {
        let path =
            crate::scene_dsl::cases::default_goldens_dir().join("pbs_lit_material_matrix.png");
        let image = image::open(&path)
            .unwrap_or_else(|err| panic!("open {}: {err}", path.display()))
            .to_rgba8();
        let cells = stats(&image);
        assert_eq!(cells.len(), 4, "default grid is 2x2");
        assert!(cells.iter().all(|cell| cell.width > 0 && cell.height > 0));
        let gap = delta(&cells, &cells);
        assert!(
            gap.is_zero(),
            "mean {} variance {} non_dominant {}",
            gap.mean,
            gap.variance,
            gap.non_dominant
        );
    }

    #[test]
    fn a_changed_quadrant_is_a_nonzero_delta() {
        let mut image = RgbaImage::new(4, 4);
        for pixel in image.pixels_mut() {
            *pixel = Rgba([10, 20, 30, 255]);
        }
        let same = stats(&image);
        image.put_pixel(0, 0, Rgba([200, 10, 10, 255]));
        let changed = stats(&image);
        let gap = delta(&changed, &same);
        assert!(!gap.is_zero(), "{gap:?}");
        assert!(gap.non_dominant > 0 || gap.mean > 0.0);
    }
}
