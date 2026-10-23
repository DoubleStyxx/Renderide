//! Per-view color and depth load ops for camera clear modes.
//!
//! The main graph is compiled once and reused for every view, so [`crate::render_graph::pass::RasterPass::setup`]
//! cannot bake a different [`wgpu::LoadOp`] per camera. These helpers are the runtime policy
//! applied when a pass opens its attachments.
//!
//! Color and depth stay a pair. A view loads preserved depth only when it also seeds color.
//! Window, OpenXR, RGBA8, format-mismatch, MSAA, partial-viewport, and double-buffered
//! (`AllowPreviousContents`) views do not seed, so they clear both attachments.

use crate::frame_contract::{FrameViewClear, RenderTextureSelfSampling};
use crate::graph_inputs::GraphPassFrameView;
use crate::shared::CameraClearMode;

/// Cameras folded by one [`camera_clear_load_op_checksum`] sample.
#[cfg(any(test, feature = "bench-api"))]
pub(crate) const CAMERA_CLEAR_LOAD_OP_CAMERAS: usize = 4_096;

/// Inputs that decide whether this view copies its current color into HDR scene color.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScenePreserveQuery {
    /// Raster sample count for the view. Multisample targets are not seeded.
    pub sample_count: u32,
    /// `true` when the view has a full-target offscreen color texture to copy from.
    pub has_color_texture: bool,
    /// Format of the camera color target.
    pub surface_format: wgpu::TextureFormat,
    /// Format of the HDR scene-color transient.
    pub scene_color_format: wgpu::TextureFormat,
    /// Host render-texture self-sampling policy. `None` for window, OpenXR, and untracked targets.
    pub self_sampling: Option<RenderTextureSelfSampling>,
    /// `true` when the depth attachment is the persistent camera depth image.
    pub persistent_depth_target: bool,
}

/// Load ops and the color-seed bit for one view.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ForwardClearLoads {
    /// `true` when the color-preserve pass must copy the current color before opaque shading.
    pub seed_color: bool,
    /// Color load for the opaque forward pass.
    pub color: wgpu::LoadOp<wgpu::Color>,
    /// Depth load for the depth prepass.
    pub depth: wgpu::LoadOp<f32>,
}

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

/// `true` when this view should copy its existing color into HDR scene color before opaque shading.
///
/// Partial-viewport scratches, multisample targets, format mismatches, and double-buffered
/// render textures stay on the clear path. Seeding those would keep the wrong image, copy the
/// write buffer, or be overwritten by an MSAA resolve.
#[inline]
pub(crate) fn seeds_preserved_scene_color(
    mode: CameraClearMode,
    query: ScenePreserveQuery,
) -> bool {
    preserves_scene_color(mode)
        && query.sample_count <= 1
        && query.has_color_texture
        && query.surface_format == query.scene_color_format
        && query.self_sampling != Some(RenderTextureSelfSampling::AllowPreviousContents)
}

/// Color and depth loads for one camera.
///
/// Depth loads only when color is actually seeded and the depth image is the persistent target.
/// Every unseeded view clears both, including Nothing on a window or OpenXR view.
#[inline]
pub(crate) fn forward_clear_loads(
    clear: FrameViewClear,
    query: ScenePreserveQuery,
) -> ForwardClearLoads {
    let seed_color = seeds_preserved_scene_color(clear.mode, query);
    ForwardClearLoads {
        seed_color,
        color: if seed_color {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(crate::present::SWAPCHAIN_CLEAR_COLOR)
        },
        depth: if seed_color && query.persistent_depth_target && preserves_scene_depth(clear.mode) {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(crate::gpu::MAIN_FORWARD_DEPTH_CLEAR)
        },
    }
}

/// Decision used by the color-preserve pass, the opaque color override, and the depth prepass.
#[inline]
pub(crate) fn forward_clear_loads_for_view(view: &GraphPassFrameView<'_>) -> ForwardClearLoads {
    forward_clear_loads(
        view.clear,
        ScenePreserveQuery {
            sample_count: view.sample_count,
            has_color_texture: view.color_texture.is_some(),
            surface_format: view.surface_format,
            scene_color_format: view.scene_color_format,
            self_sampling: view.offscreen_write_target.render_texture_self_sampling(),
            persistent_depth_target: view.persistent_depth_target,
        },
    )
}

/// Folds the load-op policy for `cameras` synthetic views into one scalar.
///
/// The loop is allocation-free. It times the decision the opaque pass and depth prepass make
/// once per view. It does not time the full-target color copy those passes record when the
/// decision seeds.
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
        let self_sampling = if index & 32 == 0 {
            None
        } else if index & 64 == 0 {
            Some(RenderTextureSelfSampling::Suppress)
        } else {
            Some(RenderTextureSelfSampling::AllowPreviousContents)
        };
        let loads = forward_clear_loads(
            clear,
            ScenePreserveQuery {
                sample_count: samples,
                has_color_texture: has_color,
                surface_format: wgpu::TextureFormat::Rgba16Float,
                scene_color_format: scene_format,
                self_sampling,
                persistent_depth_target: persistent_depth,
            },
        );
        acc = acc.wrapping_add(
            color_load_tag(loads.color)
                .wrapping_mul(4)
                .wrapping_add(depth_load_tag(loads.depth)),
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
        CAMERA_CLEAR_LOAD_OP_CAMERAS, ScenePreserveQuery, camera_clear_load_op_checksum,
        forward_clear_loads, preserves_scene_color, preserves_scene_depth,
    };
    use crate::camera::CameraRenderRect;
    use crate::frame_contract::{FrameViewClear, OffscreenWriteTarget, RenderTextureSelfSampling};
    use crate::render_graph::compiled::{
        ClearLoadViewSource, FrameViewTargetKind, clear_load_inputs_from_view,
    };
    use crate::shared::CameraClearMode;

    const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    fn clear(mode: CameraClearMode) -> FrameViewClear {
        FrameViewClear {
            mode,
            color: glam::Vec4::new(0.2, 0.3, 0.4, 1.0),
        }
    }

    fn query(
        sample_count: u32,
        has_color_texture: bool,
        surface_format: wgpu::TextureFormat,
        self_sampling: Option<RenderTextureSelfSampling>,
        persistent_depth_target: bool,
    ) -> ScenePreserveQuery {
        ScenePreserveQuery {
            sample_count,
            has_color_texture,
            surface_format,
            scene_color_format: HDR,
            self_sampling,
            persistent_depth_target,
        }
    }

    fn assert_both_clear(label: &str, mode: CameraClearMode, preserve: ScenePreserveQuery) {
        let loads = forward_clear_loads(clear(mode), preserve);
        assert!(!loads.seed_color, "{label} {mode:?} must not seed color");
        assert!(
            matches!(loads.color, wgpu::LoadOp::Clear(_)),
            "{label} {mode:?} must clear color"
        );
        assert!(
            matches!(loads.depth, wgpu::LoadOp::Clear(depth) if depth == crate::gpu::MAIN_FORWARD_DEPTH_CLEAR),
            "{label} {mode:?} must clear depth"
        );
    }

    fn assert_unseeded_pair(label: &str, preserve: ScenePreserveQuery) {
        assert_both_clear(label, CameraClearMode::Nothing, preserve);
        assert_both_clear(label, CameraClearMode::Depth, preserve);
    }

    #[test]
    fn skybox_and_color_keep_today_clear_ops() {
        let seeded = query(
            1,
            true,
            HDR,
            Some(RenderTextureSelfSampling::Suppress),
            true,
        );
        for mode in [CameraClearMode::Skybox, CameraClearMode::Color] {
            assert!(!preserves_scene_color(mode));
            assert!(!preserves_scene_depth(mode));
            assert_both_clear("skybox or color", mode, seeded);
        }
    }

    #[test]
    fn seeded_nothing_loads_color_and_depth() {
        let loads = forward_clear_loads(
            clear(CameraClearMode::Nothing),
            query(
                1,
                true,
                HDR,
                Some(RenderTextureSelfSampling::Suppress),
                true,
            ),
        );
        assert!(loads.seed_color);
        assert!(matches!(loads.color, wgpu::LoadOp::Load));
        assert!(matches!(loads.depth, wgpu::LoadOp::Load));
    }

    #[test]
    fn seeded_depth_only_loads_color_and_clears_depth() {
        let loads = forward_clear_loads(
            clear(CameraClearMode::Depth),
            query(1, true, HDR, None, true),
        );
        assert!(preserves_scene_color(CameraClearMode::Depth));
        assert!(!preserves_scene_depth(CameraClearMode::Depth));
        assert!(loads.seed_color);
        assert!(matches!(loads.color, wgpu::LoadOp::Load));
        assert!(matches!(loads.depth, wgpu::LoadOp::Clear(_)));
    }

    #[test]
    fn window_nothing_clears_color_and_depth() {
        // Swapchain views have no color texture to copy, even though their depth image persists.
        assert_unseeded_pair(
            "window",
            query(1, false, wgpu::TextureFormat::Bgra8Unorm, None, true),
        );
    }

    #[test]
    fn openxr_nothing_clears_color_and_depth() {
        // External multiview views also leave `color_texture` unset.
        assert_unseeded_pair(
            "openxr",
            query(1, false, wgpu::TextureFormat::Rgba8UnormSrgb, None, true),
        );
    }

    #[test]
    fn rgba8_target_nothing_clears_color_and_depth() {
        assert_unseeded_pair(
            "rgba8",
            query(
                1,
                true,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                Some(RenderTextureSelfSampling::Suppress),
                true,
            ),
        );
    }

    #[test]
    fn format_mismatch_nothing_clears_color_and_depth() {
        assert_unseeded_pair(
            "format mismatch",
            query(
                1,
                true,
                wgpu::TextureFormat::Rgba8Unorm,
                Some(RenderTextureSelfSampling::Suppress),
                true,
            ),
        );
    }

    #[test]
    fn msaa_nothing_clears_color_and_depth() {
        assert_unseeded_pair(
            "msaa",
            query(
                4,
                true,
                HDR,
                Some(RenderTextureSelfSampling::Suppress),
                true,
            ),
        );
    }

    #[test]
    fn partial_viewport_rect_alone_clears_color_and_depth() {
        let extent = (64, 32);
        let full_viewport = CameraRenderRect {
            origin_px: (0, 0),
            extent_px: extent,
        };
        let partial_viewport = CameraRenderRect {
            origin_px: (16, 0),
            extent_px: (32, 32),
        };
        let full = ClearLoadViewSource {
            kind: FrameViewTargetKind::OffscreenRt,
            viewport: full_viewport,
            target_extent_px: extent,
            copies_partial_color: false,
            sample_count: 1,
            surface_format: HDR,
            scene_color_format: HDR,
            write_target: OffscreenWriteTarget::host_render_texture_with_self_sampling(
                7,
                RenderTextureSelfSampling::Suppress,
            ),
        };
        let partial = ClearLoadViewSource {
            viewport: partial_viewport,
            ..full
        };
        assert_eq!(
            ClearLoadViewSource {
                viewport: full.viewport,
                ..partial
            },
            full,
            "partial and full views must differ only by the viewport rect"
        );

        let full_loads = forward_clear_loads(
            clear(CameraClearMode::Nothing),
            preserve_query(clear_load_inputs_from_view(full)),
        );
        assert!(full_loads.seed_color);
        assert!(matches!(full_loads.color, wgpu::LoadOp::Load));
        assert!(matches!(full_loads.depth, wgpu::LoadOp::Load));

        let partial_inputs = clear_load_inputs_from_view(partial);
        assert_unseeded_pair("partial viewport", preserve_query(partial_inputs));
    }

    fn preserve_query(
        inputs: crate::render_graph::compiled::ClearLoadDecisionInputs,
    ) -> ScenePreserveQuery {
        ScenePreserveQuery {
            sample_count: inputs.sample_count,
            has_color_texture: inputs.has_color_texture,
            surface_format: inputs.surface_format,
            scene_color_format: inputs.scene_color_format,
            self_sampling: inputs.self_sampling,
            persistent_depth_target: inputs.persistent_depth_target,
        }
    }

    #[test]
    fn allow_previous_contents_stays_unseeded() {
        // Double-buffered render textures expose the write image on the view. Seeding that image
        // would copy the back buffer. A later change can point the seed at the history image.
        assert_unseeded_pair(
            "allow previous contents",
            query(
                1,
                true,
                HDR,
                Some(RenderTextureSelfSampling::AllowPreviousContents),
                true,
            ),
        );
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
