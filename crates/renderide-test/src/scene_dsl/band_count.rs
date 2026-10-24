//! Dominant-color counts inside a rectangle.
//!
//! The sphere-band floor and the sphere-band color check both ignore an image's
//! most common color, which is the black background on `pbs_lit`.

use image::RgbaImage;

pub(crate) fn rgb_key(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// RGB key of the color that occupies the most pixels.
pub(crate) fn dominant_rgb_key(image: &RgbaImage) -> u32 {
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

/// Pixels inside the rectangle whose RGB is not `dominant`.
pub(crate) fn count_non_dominant(
    image: &RgbaImage,
    dominant: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<usize, String> {
    let (image_width, image_height) = image.dimensions();
    let (x1, y1) = rect_end(x, y, width, height, image_width, image_height)?;
    let mut count = 0usize;
    for py in y..y1 {
        for px in x..x1 {
            let pixel = image.get_pixel(px, py);
            if rgb_key(pixel[0], pixel[1], pixel[2]) != dominant {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// `floor(count * fraction)` for a fraction in `0.0..=1.0`, at thousandth resolution.
///
/// A fraction of 0.75 keeps about three quarters of the golden band.
pub(crate) fn fraction_floor(count: usize, fraction: f64) -> usize {
    let wide = u64::try_from(count).unwrap_or(u64::MAX);
    let product = u128::from(wide) * u128::from(fraction_thousandths(fraction)) / 1_000;
    usize::try_from(product).unwrap_or(usize::MAX)
}

fn fraction_thousandths(fraction: f64) -> u32 {
    if fraction.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return 0;
    }
    let mut best = 0u32;
    for thousandths in 1..=1_000 {
        if f64::from(thousandths) / 1_000.0 <= fraction {
            best = thousandths;
        } else {
            break;
        }
    }
    best
}

fn rect_end(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image_width: u32,
    image_height: u32,
) -> Result<(u32, u32), String> {
    let x1 = x
        .checked_add(width)
        .ok_or_else(|| format!("region x {x} + width {width} overflows"))?;
    let y1 = y
        .checked_add(height)
        .ok_or_else(|| format!("region y {y} + height {height} overflows"))?;
    if x1 > image_width || y1 > image_height {
        return Err(format!(
            "region {x},{y} {width}x{height} is outside {image_width}x{image_height}"
        ));
    }
    Ok((x1, y1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_floor_is_three_quarters_of_a_golden_band() {
        assert_eq!(fraction_floor(4_200, 0.75), 3_150);
        assert_eq!(fraction_floor(4_030, 0.75), 3_022);
        assert_eq!(fraction_floor(0, 0.75), 0);
    }
}
