//! Fit/letterbox math for [`super::DisplayBlitResources`].
//!
//! The source texture is uniformly scaled to fit inside the surface with letterbox bars on the
//! longer axis.

/// Fitted rect (in pixels) the texture should be drawn into for a given (texture, surface).
///
/// `(x, y)` is the top-left corner inside the surface; `(w, h)` are pixel dimensions. The rest of
/// the surface is the letterbox region and is left in the cleared `background_color`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FittedRectPx {
    /// Left edge of the fitted rect in surface pixels.
    pub x: u32,
    /// Top edge of the fitted rect in surface pixels.
    pub y: u32,
    /// Width of the fitted rect in surface pixels.
    pub w: u32,
    /// Height of the fitted rect in surface pixels.
    pub h: u32,
}

/// Computes the largest centered rect that fits `(tex_w x tex_h)` inside `(surf_w x surf_h)`
/// preserving the texture aspect ratio (CSS object-fit: contain).
///
/// All inputs are clamped to `>= 1` so a degenerate `0x0` extent does not propagate `NaN` or
/// produce an empty rect.
pub(super) fn fit_rect_px(tex_w: u32, tex_h: u32, surf_w: u32, surf_h: u32) -> FittedRectPx {
    let surf_w = surf_w.max(1);
    let surf_h = surf_h.max(1);
    let tex_w = tex_w.max(1);
    let tex_h = tex_h.max(1);
    // tex_w / tex_h >  surf_w / surf_h  ->  texture is wider relative to surface  ->  letterbox top/bottom
    // tex_w / tex_h <= surf_w / surf_h  ->  texture is taller relative to surface ->  letterbox left/right
    // Compare via cross-product to avoid float division.
    let texture_wider = (tex_w as u64) * (surf_h as u64) > (tex_h as u64) * (surf_w as u64);
    if texture_wider {
        // Width fills surface, height scaled to preserve aspect.
        let w = surf_w;
        let h = ((tex_h as u64 * surf_w as u64) / tex_w as u64).max(1) as u32;
        let y = (surf_h.saturating_sub(h)) / 2;
        FittedRectPx { x: 0, y, w, h }
    } else {
        // Height fills surface, width scaled to preserve aspect.
        let h = surf_h;
        let w = ((tex_w as u64 * surf_h as u64) / tex_h as u64).max(1) as u32;
        let x = (surf_w.saturating_sub(w)) / 2;
        FittedRectPx { x, y: 0, w, h }
    }
}

/// UV scale + offset that pre-flips the fullscreen UV in [0, 1]^2 according to the host's
/// `flipHorizontally` and `flipVertically` flags.
///
/// Returned as `[scale_x, scale_y, offset_x, offset_y]` so it packs into a single `vec4<f32>`
/// uniform expected by `shaders/passes/present/display_blit.wgsl`.
pub(super) fn flip_uv_params(flip_h: bool, flip_v: bool) -> [f32; 4] {
    let (sx, ox) = if flip_h { (-1.0, 1.0) } else { (1.0, 0.0) };
    let (sy, oy) = if flip_v { (-1.0, 1.0) } else { (1.0, 0.0) };
    [sx, sy, ox, oy]
}

#[cfg(test)]
mod tests {
    use super::{FittedRectPx, fit_rect_px, flip_uv_params};

    #[test]
    fn texture_matches_surface_aspect_fills_surface() {
        let r = fit_rect_px(800, 400, 1600, 800);
        assert_eq!(
            r,
            FittedRectPx {
                x: 0,
                y: 0,
                w: 1600,
                h: 800
            }
        );
    }

    #[test]
    fn texture_wider_than_surface_letterboxes_top_bottom() {
        // 2:1 texture in 1:1 surface -> width fills, height halved, top/bottom bars.
        let r = fit_rect_px(200, 100, 100, 100);
        assert_eq!(r.x, 0);
        assert_eq!(r.w, 100);
        assert_eq!(r.h, 50);
        assert_eq!(r.y, 25);
    }

    #[test]
    fn texture_taller_than_surface_letterboxes_left_right() {
        // 1:2 texture in 1:1 surface -> height fills, width halved, left/right bars.
        let r = fit_rect_px(100, 200, 100, 100);
        assert_eq!(r.y, 0);
        assert_eq!(r.h, 100);
        assert_eq!(r.w, 50);
        assert_eq!(r.x, 25);
    }

    #[test]
    fn degenerate_zero_inputs_clamp_to_unit() {
        let r = fit_rect_px(0, 0, 0, 0);
        assert_eq!(r.w, 1);
        assert_eq!(r.h, 1);
    }

    #[test]
    fn flip_uv_params_round_trip_ranges() {
        // No flip: identity.
        assert_eq!(flip_uv_params(false, false), [1.0, 1.0, 0.0, 0.0]);
        // Horizontal flip: x maps 0 -> 1, 1 -> 0.
        assert_eq!(flip_uv_params(true, false), [-1.0, 1.0, 1.0, 0.0]);
        // Vertical flip: y maps 0 -> 1, 1 -> 0.
        assert_eq!(flip_uv_params(false, true), [1.0, -1.0, 0.0, 1.0]);
        // Both flips.
        assert_eq!(flip_uv_params(true, true), [-1.0, -1.0, 1.0, 1.0]);
    }
}
