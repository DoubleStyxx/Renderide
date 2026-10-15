//! Fullscreen blit from a host texture to a display surface.
//!
//! Pairs with `crate::gpu::display_blit::surface_blit::DisplayBlitResources`. The host clears the
//! whole surface to `BlitToDisplayState.background_color` via `LoadOp::Clear`, then constrains
//! rasterization to the fitted (letterboxed) rect via `set_viewport`, leaving the bars in the
//! cleared color. This shader samples the texture at the standard fullscreen UV, with optional
//! axis flips encoded by the host via `uv_flip` `(scale, offset)` per axis.
//!
//! Implements the host display-blit contract with a fitted rectangle and flipped pixel transform.
//!
//! ### Y orientation
//!
//! Uses [`fs::vertex_flipped_y_main`] (matches the VR-mirror surface blit at
//! `shaders/passes/present/vr_mirror_surface.wgsl`). renderide-rendered render textures (output of
//! the dash camera, interactive-camera mirrors, etc.) write the camera's "top of view" to the
//! upper rows of the texture, but the surface swapchain expects screen-Y-down on output, so we
//! invert UV.y before sampling. Without this the dash appears upside-down on the desktop window.

#import renderide::core::fullscreen as fs

@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> uv_flip: vec4<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> fs::FullscreenVertexOutput {
    return fs::vertex_flipped_y_main(vi);
}

@fragment
fn fs_main(in: fs::FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // uv_flip.xy = scale (1 or -1 per axis); uv_flip.zw = offset (0 or 1 per axis).
    let uv = in.uv * uv_flip.xy + uv_flip.zw;
    return textureSample(t, samp, uv);
}
