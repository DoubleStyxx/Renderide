//! Field mapping from a view onto camera-clear load-op inputs.

use super::{ClearLoadDecisionInputs, ClearLoadViewSource, clear_load_inputs_from_view};
use crate::camera::CameraRenderRect;
use crate::frame_contract::{OffscreenWriteTarget, RenderTextureSelfSampling};

use super::super::super::frame_view::FrameViewTargetKind;

const EXTENT: (u32, u32) = (64, 32);
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

fn full_rect() -> CameraRenderRect {
    CameraRenderRect {
        origin_px: (0, 0),
        extent_px: EXTENT,
    }
}

fn partial_rect() -> CameraRenderRect {
    CameraRenderRect {
        origin_px: (16, 0),
        extent_px: (32, 32),
    }
}

fn offscreen(
    viewport: CameraRenderRect,
    sample_count: u32,
    surface_format: wgpu::TextureFormat,
    self_sampling: RenderTextureSelfSampling,
) -> ClearLoadViewSource {
    ClearLoadViewSource {
        kind: FrameViewTargetKind::OffscreenRt,
        viewport,
        target_extent_px: EXTENT,
        copies_partial_color: false,
        sample_count,
        surface_format,
        scene_color_format: HDR,
        write_target: OffscreenWriteTarget::host_render_texture_with_self_sampling(
            7,
            self_sampling,
        ),
    }
}

fn expect_offscreen(
    sample_count: u32,
    has_color_texture: bool,
    surface_format: wgpu::TextureFormat,
    self_sampling: RenderTextureSelfSampling,
    persistent_depth_target: bool,
) -> ClearLoadDecisionInputs {
    ClearLoadDecisionInputs {
        sample_count,
        has_color_texture,
        surface_format,
        scene_color_format: HDR,
        self_sampling: Some(self_sampling),
        persistent_depth_target,
    }
}

#[test]
fn window_maps_no_color_texture_and_persistent_depth() {
    let inputs = clear_load_inputs_from_view(ClearLoadViewSource {
        kind: FrameViewTargetKind::Swapchain,
        viewport: partial_rect(),
        target_extent_px: EXTENT,
        copies_partial_color: true,
        sample_count: 1,
        surface_format: wgpu::TextureFormat::Bgra8Unorm,
        scene_color_format: HDR,
        write_target: OffscreenWriteTarget::host_render_texture_with_self_sampling(
            7,
            RenderTextureSelfSampling::AllowPreviousContents,
        ),
    });
    assert_eq!(
        inputs,
        ClearLoadDecisionInputs {
            sample_count: 1,
            has_color_texture: false,
            surface_format: wgpu::TextureFormat::Bgra8Unorm,
            scene_color_format: HDR,
            self_sampling: None,
            persistent_depth_target: true,
        }
    );
}

#[test]
fn openxr_maps_no_color_texture_and_persistent_depth() {
    let inputs = clear_load_inputs_from_view(ClearLoadViewSource {
        kind: FrameViewTargetKind::ExternalMultiview,
        viewport: partial_rect(),
        target_extent_px: EXTENT,
        copies_partial_color: false,
        sample_count: 1,
        surface_format: wgpu::TextureFormat::Rgba8UnormSrgb,
        scene_color_format: HDR,
        write_target: OffscreenWriteTarget::None,
    });
    assert_eq!(
        inputs,
        ClearLoadDecisionInputs {
            sample_count: 1,
            has_color_texture: false,
            surface_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            scene_color_format: HDR,
            self_sampling: None,
            persistent_depth_target: true,
        }
    );
}

#[test]
fn seeded_rgba16_offscreen_maps_every_field() {
    let source = offscreen(full_rect(), 1, HDR, RenderTextureSelfSampling::Suppress);
    assert_eq!(
        clear_load_inputs_from_view(source),
        expect_offscreen(1, true, HDR, RenderTextureSelfSampling::Suppress, true)
    );
}

#[test]
fn rgba8_offscreen_maps_surface_format() {
    let source = offscreen(
        full_rect(),
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        RenderTextureSelfSampling::Suppress,
    );
    assert_eq!(
        clear_load_inputs_from_view(source),
        expect_offscreen(
            1,
            true,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            RenderTextureSelfSampling::Suppress,
            true,
        )
    );
}

#[test]
fn msaa_offscreen_maps_sample_count() {
    let source = offscreen(full_rect(), 4, HDR, RenderTextureSelfSampling::Suppress);
    assert_eq!(
        clear_load_inputs_from_view(source),
        expect_offscreen(4, true, HDR, RenderTextureSelfSampling::Suppress, true)
    );
}

#[test]
fn partial_viewport_rect_clears_color_and_depth_flags_only() {
    let seeded = offscreen(full_rect(), 1, HDR, RenderTextureSelfSampling::Suppress);
    let partial = ClearLoadViewSource {
        viewport: partial_rect(),
        ..seeded
    };
    assert_eq!(
        ClearLoadViewSource {
            viewport: seeded.viewport,
            ..partial
        },
        seeded
    );
    let partial_inputs = clear_load_inputs_from_view(partial);
    assert_eq!(
        partial_inputs,
        expect_offscreen(1, false, HDR, RenderTextureSelfSampling::Suppress, false)
    );
    assert_eq!(
        ClearLoadDecisionInputs {
            has_color_texture: true,
            persistent_depth_target: true,
            ..partial_inputs
        },
        clear_load_inputs_from_view(seeded)
    );
}

#[test]
fn allow_previous_contents_maps_self_sampling() {
    let source = offscreen(
        full_rect(),
        1,
        HDR,
        RenderTextureSelfSampling::AllowPreviousContents,
    );
    assert_eq!(
        clear_load_inputs_from_view(source),
        expect_offscreen(
            1,
            true,
            HDR,
            RenderTextureSelfSampling::AllowPreviousContents,
            true,
        )
    );
}
