//! Per-draw scissor projection and incremental scissor-state management for the world-mesh
//! forward encoder.
//!
//! UI-rect clip draws (overlays) carry an object-local clip rect that must project through the
//! draw's MVP into a pixel-space scissor for the active viewport. [`set_forward_scissor_if_changed`]
//! threads this through the encoder's [`super::ForwardDrawState`] so the rebind only fires when the
//! resolved scissor actually changes from the previous draw, and [`reset_forward_scissor`] restores
//! the full viewport once the subpass is done.

use super::{ForwardDrawResources, ForwardDrawState};

/// Updates the scissor for the draw `representative` when it differs from the previously bound
/// rect, recording the new scissor on [`ForwardDrawState::last_scissor`].
pub(super) fn set_forward_scissor_if_changed(
    rpass: &mut wgpu::RenderPass<'_>,
    resources: &ForwardDrawResources<'_, '_>,
    state: &mut ForwardDrawState,
    representative: usize,
) {
    let item = &resources.draws[representative];
    let scissor = match (item.ui_rect_clip_local, item.rigid_world_matrix) {
        (Some(rect), Some(model)) => project_rect_to_scissor(
            resources.overlay_view_proj * model,
            rect,
            resources.viewport_px,
        )
        .unwrap_or(resources.full_viewport),
        _ => resources.full_viewport,
    };
    if state.last_scissor != Some(scissor) {
        rpass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
        state.last_scissor = Some(scissor);
    }
}

/// Restores the scissor to the full viewport once the subpass finishes, if any prior draw set a
/// narrower rect.
pub(super) fn reset_forward_scissor(
    rpass: &mut wgpu::RenderPass<'_>,
    full_viewport: (u32, u32, u32, u32),
    last_scissor: Option<(u32, u32, u32, u32)>,
) {
    if last_scissor.is_some() && last_scissor != Some(full_viewport) {
        rpass.set_scissor_rect(
            full_viewport.0,
            full_viewport.1,
            full_viewport.2,
            full_viewport.3,
        );
    }
}

/// Projects the four corners of an object-local UI rect through `mvp` into NDC, builds the
/// pixel-space AABB clamped to `viewport_px`, and returns it as a `(x, y, w, h)` scissor.
///
/// Returns `None` when:
/// - all four corners have non-positive `w` (rect entirely behind / on the near plane), or
/// - the rect projects to a degenerate (zero-width or zero-height) screen region.
///
/// `viewport_px` is the active viewport in pixels (`width`, `height`). The scissor is clamped to
/// stay inside the viewport for the partially-off-screen case; fully-off-screen rejection is
/// already handled by the CPU rect-cull in
/// [`crate::world_mesh::culling::overlay_rect_clip_visible`].
fn project_rect_to_scissor(
    mvp: glam::Mat4,
    rect: glam::Vec4,
    viewport_px: (u32, u32),
) -> Option<(u32, u32, u32, u32)> {
    let corners = [
        glam::Vec4::new(rect.x, rect.y, 0.0, 1.0),
        glam::Vec4::new(rect.z, rect.y, 0.0, 1.0),
        glam::Vec4::new(rect.z, rect.w, 0.0, 1.0),
        glam::Vec4::new(rect.x, rect.w, 0.0, 1.0),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut any_in_front = false;
    for c in corners {
        let clip = mvp * c;
        if clip.w <= 0.0 {
            continue;
        }
        any_in_front = true;
        let ndc_x = (clip.x / clip.w).clamp(-1.0, 1.0);
        let ndc_y = (clip.y / clip.w).clamp(-1.0, 1.0);
        if ndc_x < min_x {
            min_x = ndc_x;
        }
        if ndc_x > max_x {
            max_x = ndc_x;
        }
        if ndc_y < min_y {
            min_y = ndc_y;
        }
        if ndc_y > max_y {
            max_y = ndc_y;
        }
    }
    if !any_in_front {
        return None;
    }
    let (vw, vh) = (viewport_px.0 as f32, viewport_px.1 as f32);
    // Clip space y is +up; pixel space y is +down. Flip y when mapping to pixels.
    let px_min_x = ((min_x * 0.5 + 0.5) * vw).floor().clamp(0.0, vw);
    let px_max_x = ((max_x * 0.5 + 0.5) * vw).ceil().clamp(0.0, vw);
    let px_min_y = (((-max_y) * 0.5 + 0.5) * vh).floor().clamp(0.0, vh);
    let px_max_y = (((-min_y) * 0.5 + 0.5) * vh).ceil().clamp(0.0, vh);
    let x = px_min_x as u32;
    let y = px_min_y as u32;
    let w = (px_max_x - px_min_x) as u32;
    let h = (px_max_y - px_min_y) as u32;
    if w == 0 || h == 0 {
        return None;
    }
    Some((x, y, w, h))
}

#[cfg(test)]
mod tests {
    use super::project_rect_to_scissor;
    use glam::{Mat4, Vec4};

    /// Symmetric ortho mapping NDC [-1, 1] in xy to overlay-space [-1, 1].
    fn ortho() -> Mat4 {
        Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0)
    }

    #[test]
    fn project_rect_inside_viewport_clamps_to_pixel_aabb() {
        let mvp = ortho();
        let rect = Vec4::new(-0.5, -0.5, 0.5, 0.5);
        let scissor = project_rect_to_scissor(mvp, rect, (200, 100)).expect("scissor");
        assert_eq!(scissor.0, 50);
        assert_eq!(scissor.2, 100);
        assert_eq!(scissor.1, 25);
        assert_eq!(scissor.3, 50);
    }

    #[test]
    fn project_rect_partially_offscreen_clamps_to_viewport_edges() {
        let mvp = ortho();
        let rect = Vec4::new(-2.0, -0.5, 0.0, 0.5);
        let scissor = project_rect_to_scissor(mvp, rect, (200, 100)).expect("scissor");
        assert_eq!(scissor.0, 0);
        assert_eq!(scissor.0 + scissor.2, 100);
    }

    #[test]
    fn project_rect_fully_behind_camera_returns_none() {
        let mvp = Mat4::from_diagonal(Vec4::new(1.0, 1.0, 1.0, -1.0));
        let rect = Vec4::new(-0.5, -0.5, 0.5, 0.5);
        assert!(project_rect_to_scissor(mvp, rect, (200, 100)).is_none());
    }
}
