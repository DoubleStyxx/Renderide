//! Per-view color and depth load ops for camera clear modes.
//!
//! The main graph is compiled once and reused for every view, so [`crate::render_graph::pass::RasterPass::setup`]
//! cannot bake a different [`wgpu::LoadOp`] per camera. These helpers are the runtime policy
//! applied when a pass opens its attachments.

use crate::frame_contract::FrameViewClear;
use crate::graph_inputs::GraphPassFrameView;
use crate::shared::CameraClearMode;

/// Cameras folded by one [`camera_clear_load_op_checksum`] sample.
#[cfg(any(test, feature = "bench-api"))]
pub(crate) const CAMERA_CLEAR_LOAD_OP_CAMERAS: usize = 4_096;

/// `true` when this clear mode must keep the color already in the render target.
#[inline]
pub(crate) fn preserves_scene_color(mode: CameraClearMode) -> bool {
    matches!(mode, CameraClearMode::Depth | CameraClearMode::Nothing)
}

/// `true` when this clear mode must keep the depth already in the render target.
#[inline]
pub(crate) fn preserves_scene_depth(mode: CameraClearMode) -> bool {
    matches!(mode, CameraClearMode::Nothing)
}

/// Color load for the opaque forward pass.
///
/// `seed` is true only when HDR scene color was just copied from the existing target.
/// Skybox and color clears stay [`wgpu::LoadOp::Clear`] so the background draw still replaces
/// black, matching the pass template.
#[inline]
pub(crate) fn forward_color_load_op(
    clear: FrameViewClear,
    seed: bool,
) -> wgpu::LoadOp<wgpu::Color> {
    if seed && preserves_scene_color(clear.mode) {
        wgpu::LoadOp::Load
    } else {
        wgpu::LoadOp::Clear(crate::present::SWAPCHAIN_CLEAR_COLOR)
    }
}

/// Depth load for the depth prepass.
///
/// Multisampled depth is a transient that the resolve pass writes over the imported target, so
/// only a single-sample persistent depth texture can be loaded. Depth-only, skybox, and color
/// clears still clear to far (reverse-Z `0`).
#[inline]
pub(crate) fn forward_depth_load_op(
    clear: FrameViewClear,
    sample_count: u32,
    persistent_depth_target: bool,
) -> wgpu::LoadOp<f32> {
    if persistent_depth_target && sample_count <= 1 && preserves_scene_depth(clear.mode) {
        wgpu::LoadOp::Load
    } else {
        wgpu::LoadOp::Clear(crate::gpu::MAIN_FORWARD_DEPTH_CLEAR)
    }
}

/// `true` when this view should copy its existing color into HDR scene color before opaque shading.
///
/// Partial-viewport scratches, multisample targets, and format mismatches stay on the clear path.
/// Seeding those would keep the wrong image or be overwritten by an MSAA resolve.
#[inline]
pub(crate) fn seeds_preserved_scene_color(
    mode: CameraClearMode,
    sample_count: u32,
    has_color_texture: bool,
    surface_format: wgpu::TextureFormat,
    scene_color_format: wgpu::TextureFormat,
) -> bool {
    preserves_scene_color(mode)
        && sample_count <= 1
        && has_color_texture
        && surface_format == scene_color_format
}

/// View wrapper around [`seeds_preserved_scene_color`].
#[inline]
pub(crate) fn view_seeds_preserved_scene_color(view: &GraphPassFrameView<'_>) -> bool {
    seeds_preserved_scene_color(
        view.clear.mode,
        view.sample_count,
        view.color_texture.is_some(),
        view.surface_format,
        view.scene_color_format,
    )
}

/// Folds the load-op policy for `cameras` synthetic views into one scalar.
///
/// The loop is allocation-free. It exists so `cargo bench -p renderide-bench` can time the
/// decision the opaque pass and depth prepass make once per view.
#[cfg(any(test, feature = "bench-api"))]
#[must_use]
pub(crate) fn camera_clear_load_op_checksum(cameras: usize) -> u32 {
    const MODES: [CameraClearMode; 4] = [
        CameraClearMode::Skybox,
        CameraClearMode::Color,
        CameraClearMode::Depth,
        CameraClearMode::Nothing,
    ];
    let mut acc = 0_u32;
    let mut index = 0_usize;
    while index < cameras {
        let mode = MODES[index & 3];
        let clear = FrameViewClear {
            mode,
            color: glam::Vec4::ZERO,
        };
        let samples = if index & 4 == 0 { 1 } else { 4 };
        let persistent_depth = index & 8 == 0;
        let has_color = index & 2 == 0;
        let scene_format = if index & 16 == 0 {
            wgpu::TextureFormat::Rgba16Float
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let seed = seeds_preserved_scene_color(
            mode,
            samples,
            has_color,
            wgpu::TextureFormat::Rgba16Float,
            scene_format,
        );
        acc = acc.wrapping_add(
            color_load_tag(forward_color_load_op(clear, seed))
                .wrapping_mul(4)
                .wrapping_add(depth_load_tag(forward_depth_load_op(
                    clear,
                    samples,
                    persistent_depth,
                ))),
        );
        index += 1;
    }
    acc
}

#[cfg(any(test, feature = "bench-api"))]
#[inline]
fn color_load_tag(load: wgpu::LoadOp<wgpu::Color>) -> u32 {
    match load {
        wgpu::LoadOp::Load => 1,
        wgpu::LoadOp::Clear(_) => 2,
        wgpu::LoadOp::DontCare(_) => 3,
    }
}

#[cfg(any(test, feature = "bench-api"))]
#[inline]
fn depth_load_tag(load: wgpu::LoadOp<f32>) -> u32 {
    match load {
        wgpu::LoadOp::Load => 1,
        wgpu::LoadOp::Clear(_) => 2,
        wgpu::LoadOp::DontCare(_) => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CAMERA_CLEAR_LOAD_OP_CAMERAS, camera_clear_load_op_checksum, forward_color_load_op,
        forward_depth_load_op, preserves_scene_color, preserves_scene_depth,
        seeds_preserved_scene_color,
    };
    use crate::frame_contract::FrameViewClear;
    use crate::shared::CameraClearMode;

    fn clear(mode: CameraClearMode) -> FrameViewClear {
        FrameViewClear {
            mode,
            color: glam::Vec4::new(0.2, 0.3, 0.4, 1.0),
        }
    }

    #[test]
    fn skybox_and_color_keep_today_clear_ops() {
        for mode in [CameraClearMode::Skybox, CameraClearMode::Color] {
            assert!(!preserves_scene_color(mode));
            assert!(!preserves_scene_depth(mode));
            assert!(matches!(
                forward_color_load_op(clear(mode), false),
                wgpu::LoadOp::Clear(_)
            ));
            assert!(matches!(
                forward_depth_load_op(clear(mode), 1, true),
                wgpu::LoadOp::Clear(depth) if depth == crate::gpu::MAIN_FORWARD_DEPTH_CLEAR
            ));
        }
    }

    #[test]
    fn depth_only_loads_seeded_color_and_clears_depth() {
        let mode = CameraClearMode::Depth;
        assert!(preserves_scene_color(mode));
        assert!(!preserves_scene_depth(mode));
        assert!(matches!(
            forward_color_load_op(clear(mode), true),
            wgpu::LoadOp::Load
        ));
        assert!(matches!(
            forward_depth_load_op(clear(mode), 1, true),
            wgpu::LoadOp::Clear(_)
        ));
    }

    #[test]
    fn nothing_loads_seeded_color_and_persistent_depth() {
        let mode = CameraClearMode::Nothing;
        assert!(preserves_scene_color(mode));
        assert!(preserves_scene_depth(mode));
        assert!(matches!(
            forward_color_load_op(clear(mode), true),
            wgpu::LoadOp::Load
        ));
        assert!(matches!(
            forward_depth_load_op(clear(mode), 1, true),
            wgpu::LoadOp::Load
        ));
    }

    #[test]
    fn unseedable_views_still_clear_color() {
        let mode = CameraClearMode::Nothing;
        assert!(matches!(
            forward_color_load_op(clear(mode), false),
            wgpu::LoadOp::Clear(_)
        ));
        assert!(!seeds_preserved_scene_color(
            mode,
            1,
            false,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba16Float,
        ));
        assert!(!seeds_preserved_scene_color(
            mode,
            4,
            true,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba16Float,
        ));
        assert!(!seeds_preserved_scene_color(
            mode,
            1,
            true,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba16Float,
        ));
        assert!(seeds_preserved_scene_color(
            CameraClearMode::Depth,
            1,
            true,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba16Float,
        ));
        assert!(!seeds_preserved_scene_color(
            CameraClearMode::Skybox,
            1,
            true,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba16Float,
        ));
    }

    #[test]
    fn msaa_and_scratch_depth_stay_cleared() {
        let clear = clear(CameraClearMode::Nothing);
        assert!(matches!(
            forward_depth_load_op(clear, 4, true),
            wgpu::LoadOp::Clear(_)
        ));
        assert!(matches!(
            forward_depth_load_op(clear, 1, false),
            wgpu::LoadOp::Clear(_)
        ));
    }

    #[test]
    fn load_op_checksum_is_stable_and_allocation_free() {
        let first = camera_clear_load_op_checksum(CAMERA_CLEAR_LOAD_OP_CAMERAS);
        let second = camera_clear_load_op_checksum(CAMERA_CLEAR_LOAD_OP_CAMERAS);
        assert_eq!(first, second);
        assert_ne!(first, 0);
        assert_ne!(
            camera_clear_load_op_checksum(1),
            camera_clear_load_op_checksum(2)
        );
    }
}
