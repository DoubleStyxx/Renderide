//! Per-view color and depth load ops for camera clear modes.
//!
//! The main graph is compiled once and reused for every view, so [`crate::render_graph::pass::RasterPass::setup`]
//! cannot bake a different [`wgpu::LoadOp`] per camera. These helpers are the runtime policy
//! applied when a pass opens its attachments.
//!
//! Color and depth stay a pair. A view loads preserved depth only when it also seeds color.
//! Window, OpenXR, RGBA8, format-mismatch, MSAA, and partial-viewport views do not seed, so they
//! clear both attachments. The first writer of a swapped texture seeds its history image. Every
//! later writer of that texture seeds the published image.

use crate::frame_contract::{FrameViewClear, RenderTextureSelfSampling};
use crate::graph_inputs::GraphPassFrameView;
use crate::shared::CameraClearMode;

/// Cameras folded by one [`camera_clear_load_op_checksum`] sample.
#[cfg(any(test, feature = "bench-api"))]
pub(crate) const CAMERA_CLEAR_LOAD_OP_CAMERAS: usize = 4_096;

/// Non-zero xorshift seed for [`camera_clear_load_op_inputs`].
#[cfg(any(test, feature = "bench-api"))]
const CAMERA_CLEAR_LOAD_OP_SEED: u32 = 0xC3A1_5EED;

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
    /// `true` when this view is the first writer of a swapped texture and history is attached.
    ///
    /// That writer copies history even when it is not itself double-buffered. Without the image,
    /// a double-buffered writer stays unseeded so the two-frame-old published image is not copied.
    pub has_history_color: bool,
    /// `true` when a writer after the first one this frame copies the published image.
    ///
    /// The published image already holds the earlier writers. Copying history again would replace
    /// that output and pair last frame's color with this frame's depth.
    pub seed_published_color: bool,
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
/// Partial-viewport scratches, multisample targets, and format mismatches stay on the clear path.
/// The first writer seeds when history is attached. A later writer seeds the published image,
/// which already holds this frame. A double-buffered writer with neither stays unseeded.
#[inline]
pub(crate) fn seeds_preserved_scene_color(
    mode: CameraClearMode,
    query: ScenePreserveQuery,
) -> bool {
    preserves_scene_color(mode)
        && query.sample_count <= 1
        && query.has_color_texture
        && query.surface_format == query.scene_color_format
        && history_image_ready(query)
}

/// `true` when this view's seed source exists.
///
/// The first writer of a swapped texture has history. A later writer seeds the published image.
/// Any other camera seeds its color image. A double-buffered camera with neither image stays
/// unseeded.
#[inline]
fn history_image_ready(query: ScenePreserveQuery) -> bool {
    if query.seed_published_color || query.has_history_color {
        return true;
    }
    query.self_sampling != Some(RenderTextureSelfSampling::AllowPreviousContents)
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
    loads_for_seed(
        clear.mode,
        seeds_preserved_scene_color(clear.mode, query),
        query.persistent_depth_target,
    )
}

/// Color image copied into HDR scene color, or `None` when this view clears.
///
/// This is the only seed decision. [`forward_clear_loads_for_view`] and the color-preserve pass
/// both read it, so the preserve copy and the color load cannot disagree. The first writer of a
/// swapped texture returns history. Every later writer returns the published image. A normal
/// camera with no history returns its color image. A double-buffered camera with no history and
/// no published-seed bit returns `None`.
#[inline]
pub(crate) fn preserved_color_texture<'a>(
    view: &'a GraphPassFrameView<'_>,
) -> Option<&'a wgpu::Texture> {
    if !seeds_preserved_scene_color(view.clear.mode, scene_preserve_query(view)) {
        return None;
    }
    if view.seed_published_color {
        return view.color_texture;
    }
    view.history_color_texture.or(view.color_texture)
}

/// Decision used by the color-preserve pass, the opaque color override, and the depth prepass.
#[inline]
pub(crate) fn forward_clear_loads_for_view(view: &GraphPassFrameView<'_>) -> ForwardClearLoads {
    loads_for_seed(
        view.clear.mode,
        preserved_color_texture(view).is_some(),
        view.persistent_depth_target,
    )
}

#[inline]
fn scene_preserve_query(view: &GraphPassFrameView<'_>) -> ScenePreserveQuery {
    ScenePreserveQuery {
        sample_count: view.sample_count,
        has_color_texture: view.color_texture.is_some(),
        surface_format: view.surface_format,
        scene_color_format: view.scene_color_format,
        self_sampling: view.offscreen_write_target.render_texture_self_sampling(),
        has_history_color: view.history_color_texture.is_some(),
        seed_published_color: view.seed_published_color,
        persistent_depth_target: view.persistent_depth_target,
    }
}

#[inline]
fn loads_for_seed(
    mode: CameraClearMode,
    seed_color: bool,
    persistent_depth_target: bool,
) -> ForwardClearLoads {
    ForwardClearLoads {
        seed_color,
        color: if seed_color {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(crate::present::SWAPCHAIN_CLEAR_COLOR)
        },
        depth: if seed_color && persistent_depth_target && preserves_scene_depth(mode) {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(crate::gpu::MAIN_FORWARD_DEPTH_CLEAR)
        },
    }
}

/// One synthetic camera passed to [`camera_clear_load_op_checksum`].
///
/// Callers build the slice once, outside the timed loop.
#[cfg(any(test, feature = "bench-api"))]
#[derive(Clone, Copy)]
pub struct CameraClearLoadOpInput {
    clear: FrameViewClear,
    query: ScenePreserveQuery,
}

/// Builds `cameras` load-op inputs from a seeded xorshift.
///
/// The previous bench derived every camera from `index & 127`, so the same 128 decisions
/// repeated and the checksum constant-folded. This sequence does not repeat inside 4096
/// cameras. The returned vec is the only allocation; [`camera_clear_load_op_checksum`] does
/// not allocate.
#[cfg(any(test, feature = "bench-api"))]
#[must_use]
pub(crate) fn camera_clear_load_op_inputs(cameras: usize) -> Vec<CameraClearLoadOpInput> {
    let mut state = CAMERA_CLEAR_LOAD_OP_SEED;
    let mut inputs = Vec::with_capacity(cameras);
    for _ in 0..cameras {
        let bits = xorshift32(&mut state);
        inputs.push(camera_clear_load_op_input(bits));
    }
    inputs
}

/// Folds the load-op policy for `inputs` into one scalar.
///
/// Allocation-free. Each camera passes through [`std::hint::black_box`] before the decision,
/// so the optimizer cannot fold or hoist [`forward_clear_loads`]. Does not time the full-target
/// color copy a seeded Nothing or Depth camera records.
#[cfg(any(test, feature = "bench-api"))]
#[must_use]
pub(crate) fn camera_clear_load_op_checksum(inputs: &[CameraClearLoadOpInput]) -> u32 {
    let mut acc = 0_u32;
    for input in inputs {
        let input = std::hint::black_box(*input);
        let loads = forward_clear_loads(input.clear, input.query);
        acc = acc.wrapping_add(
            color_load_tag(loads.color)
                .wrapping_mul(4)
                .wrapping_add(depth_load_tag(loads.depth)),
        );
    }
    acc
}

#[cfg(any(test, feature = "bench-api"))]
fn xorshift32(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

#[cfg(any(test, feature = "bench-api"))]
fn camera_clear_load_op_input(bits: u32) -> CameraClearLoadOpInput {
    let mode = match bits & 3 {
        0 => CameraClearMode::Skybox,
        1 => CameraClearMode::Color,
        2 => CameraClearMode::Depth,
        _ => CameraClearMode::Nothing,
    };
    let sample_count = if bits & 4 == 0 { 1 } else { 4 };
    let has_color_texture = bits & 8 != 0;
    let surface_format = if bits & 16 == 0 {
        wgpu::TextureFormat::Rgba16Float
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let scene_color_format = if bits & 32 == 0 {
        wgpu::TextureFormat::Rgba16Float
    } else {
        wgpu::TextureFormat::Bgra8Unorm
    };
    let sampling_bits = (bits >> 6) & 3;
    let self_sampling = if sampling_bits == 1 {
        Some(RenderTextureSelfSampling::Suppress)
    } else if sampling_bits == 2 {
        Some(RenderTextureSelfSampling::AllowPreviousContents)
    } else {
        None
    };
    let persistent_depth_target = bits & 256 != 0;
    let has_history_color = bits & 512 != 0;
    let seed_published_color = bits & 1024 != 0;
    let channel_byte = u8::try_from((bits >> 16) & 0xff).unwrap_or(0);
    let channel = f32::from(channel_byte) / 255.0;
    CameraClearLoadOpInput {
        clear: FrameViewClear {
            mode,
            color: glam::Vec4::new(channel, 1.0 - channel, 0.5, 1.0),
        },
        query: ScenePreserveQuery {
            sample_count,
            has_color_texture,
            surface_format,
            scene_color_format,
            self_sampling,
            has_history_color,
            seed_published_color,
            persistent_depth_target,
        },
    }
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
        camera_clear_load_op_inputs, forward_clear_loads, preserves_scene_color,
        preserves_scene_depth,
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
            has_history_color: false,
            seed_published_color: false,
            persistent_depth_target,
        }
    }

    impl ScenePreserveQuery {
        fn with_history(mut self) -> Self {
            self.has_history_color = true;
            self
        }

        fn with_published_seed(mut self) -> Self {
            self.seed_published_color = true;
            self
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
            has_history_color: false,
            seed_published_color: false,
            persistent_depth_target: inputs.persistent_depth_target,
        }
    }

    #[test]
    fn allow_previous_contents_without_history_stays_unseeded() {
        assert_unseeded_pair(
            "allow previous contents without history",
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
    fn allow_previous_contents_with_history_seeds_color() {
        let preserve = query(
            1,
            true,
            HDR,
            Some(RenderTextureSelfSampling::AllowPreviousContents),
            true,
        )
        .with_history();
        let nothing = forward_clear_loads(clear(CameraClearMode::Nothing), preserve);
        assert!(nothing.seed_color);
        assert!(matches!(nothing.color, wgpu::LoadOp::Load));
        assert!(matches!(nothing.depth, wgpu::LoadOp::Load));
        let depth = forward_clear_loads(clear(CameraClearMode::Depth), preserve);
        assert!(depth.seed_color);
        assert!(matches!(depth.color, wgpu::LoadOp::Load));
        assert!(matches!(depth.depth, wgpu::LoadOp::Clear(_)));
    }

    #[test]
    fn first_writer_history_seeds_without_allow_previous_contents() {
        let preserve = query(
            1,
            true,
            HDR,
            Some(RenderTextureSelfSampling::Suppress),
            true,
        )
        .with_history();
        let nothing = forward_clear_loads(clear(CameraClearMode::Nothing), preserve);
        assert!(nothing.seed_color);
        assert!(matches!(nothing.color, wgpu::LoadOp::Load));
        assert!(matches!(nothing.depth, wgpu::LoadOp::Load));
    }

    #[test]
    fn later_double_buffered_writer_seeds_published_color_without_history() {
        let preserve = query(
            1,
            true,
            HDR,
            Some(RenderTextureSelfSampling::AllowPreviousContents),
            true,
        )
        .with_published_seed();
        let nothing = forward_clear_loads(clear(CameraClearMode::Nothing), preserve);
        assert!(nothing.seed_color);
        assert!(matches!(nothing.color, wgpu::LoadOp::Load));
        assert!(matches!(nothing.depth, wgpu::LoadOp::Load));
        let depth = forward_clear_loads(clear(CameraClearMode::Depth), preserve);
        assert!(depth.seed_color);
        assert!(matches!(depth.color, wgpu::LoadOp::Load));
        assert!(matches!(depth.depth, wgpu::LoadOp::Clear(_)));
    }

    #[test]
    fn load_op_checksum_is_stable_and_allocation_free() {
        let inputs = camera_clear_load_op_inputs(CAMERA_CLEAR_LOAD_OP_CAMERAS);
        let again = camera_clear_load_op_inputs(CAMERA_CLEAR_LOAD_OP_CAMERAS);
        assert_eq!(inputs.len(), CAMERA_CLEAR_LOAD_OP_CAMERAS);
        let first = camera_clear_load_op_checksum(&inputs);
        let second = camera_clear_load_op_checksum(&inputs);
        assert_eq!(first, second);
        assert_eq!(first, camera_clear_load_op_checksum(&again));
        assert_ne!(first, 0);
        assert_ne!(
            camera_clear_load_op_checksum(&inputs[..1]),
            camera_clear_load_op_checksum(&inputs[..2])
        );
        // The old generator repeated every 128 cameras, which let LLVM fold the checksum.
        assert_ne!(
            decision_mix(&inputs[..128]),
            decision_mix(&inputs[128..256])
        );
    }

    fn decision_mix(inputs: &[super::CameraClearLoadOpInput]) -> u64 {
        let mut acc = 0_u64;
        for input in inputs {
            let mode = match input.clear.mode {
                CameraClearMode::Skybox => 1_u64,
                CameraClearMode::Color => 2,
                CameraClearMode::Depth => 3,
                CameraClearMode::Nothing => 4,
            };
            let sampling = match input.query.self_sampling {
                None => 0_u64,
                Some(RenderTextureSelfSampling::Suppress) => 1,
                Some(RenderTextureSelfSampling::AllowPreviousContents) => 2,
            };
            let surface = format_tag(input.query.surface_format);
            let scene = format_tag(input.query.scene_color_format);
            let piece = mode
                | (u64::from(input.query.sample_count) << 4)
                | (u64::from(input.query.has_color_texture) << 8)
                | (u64::from(input.query.persistent_depth_target) << 9)
                | (u64::from(input.query.has_history_color) << 12)
                | (u64::from(input.query.seed_published_color) << 13)
                | (sampling << 10)
                | (surface << 16)
                | (scene << 32);
            acc = acc.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(piece);
        }
        acc
    }

    fn format_tag(format: wgpu::TextureFormat) -> u64 {
        match format {
            wgpu::TextureFormat::Rgba16Float => 1,
            wgpu::TextureFormat::Rgba8Unorm => 2,
            wgpu::TextureFormat::Bgra8Unorm => 3,
            _ => 4,
        }
    }
}
