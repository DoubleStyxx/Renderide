//! `golden | actual | diff`, written for every compared pair.
//!
//! `diff.png` is written in the same call, including when the comparison passes.

use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::error::HarnessError;
use crate::image_io::{load_rgba, save_rgba, write_diff_image};

/// Filename of the three-panel image inside a case directory.
pub(crate) const SIDE_BY_SIDE_FILENAME: &str = "side_by_side.png";

/// Writes `diff.png` and `side_by_side.png`.
///
/// The diff is the image-compare color map when the two images share a size.
/// A size mismatch still leaves a black `diff.png` of the actual size so both
/// files exist. The side-by-side image is `3W×H`, where `W` and `H` are the
/// actual image.
pub(crate) fn write_pair(
    golden: &RgbaImage,
    actual: &RgbaImage,
    diff_path: &Path,
    side_path: &Path,
) -> Result<(), HarnessError> {
    if let Err(err) = write_diff_image(actual, golden, diff_path) {
        logger::warn!("side_by_side: diff write failed ({err}); writing a blank diff");
        let blank = RgbaImage::from_pixel(actual.width(), actual.height(), Rgba([0, 0, 0, 255]));
        save_rgba(&blank, diff_path)?;
    }
    let diff = load_rgba(diff_path)?;
    save_rgba(&compose(golden, actual, &diff), side_path)
}

/// Places `golden`, `actual`, and `diff` in that order.
///
/// Width is three times the actual width. Height is the actual height.
pub(crate) fn compose(golden: &RgbaImage, actual: &RgbaImage, diff: &RgbaImage) -> RgbaImage {
    let width = actual.width();
    let height = actual.height();
    let mut out = RgbaImage::from_pixel(width.saturating_mul(3), height, Rgba([0, 0, 0, 255]));
    blit(&mut out, 0, golden, width, height);
    blit(&mut out, width, actual, width, height);
    blit(&mut out, width.saturating_mul(2), diff, width, height);
    out
}

fn blit(dest: &mut RgbaImage, origin_x: u32, source: &RgbaImage, panel_w: u32, panel_h: u32) {
    let copy_w = source.width().min(panel_w);
    let copy_h = source.height().min(panel_h);
    let max_x = dest.width().saturating_sub(origin_x);
    let copy_w = copy_w.min(max_x);
    for y in 0..copy_h {
        for x in 0..copy_w {
            dest.put_pixel(origin_x + x, y, *source.get_pixel(x, y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba(color))
    }

    #[test]
    fn side_by_side_png_is_three_times_as_wide() {
        let dir = tempfile::tempdir().expect("tempdir");
        let golden = solid(5, 3, [1, 2, 3, 255]);
        let actual = solid(5, 3, [9, 8, 7, 255]);
        let diff_path = dir.path().join("diff.png");
        let side_path = dir.path().join(SIDE_BY_SIDE_FILENAME);
        write_pair(&golden, &actual, &diff_path, &side_path).expect("write pair");
        assert!(diff_path.is_file(), "diff.png is always written");
        let side = load_rgba(&side_path).expect("load side by side");
        assert_eq!(side.dimensions(), (15, 3));
        assert_eq!(side.get_pixel(0, 0).0, [1, 2, 3, 255]);
        assert_eq!(side.get_pixel(5, 0).0, [9, 8, 7, 255]);
    }
}
