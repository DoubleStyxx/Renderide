//! Fullscreen pass: GTAO depth-aware denoise (intermediate iteration).
//!
//! Reads the AO term + packed edges produced by `gtao_main`, runs the edge-preserving
//! 3x3 bilateral kernel with `finalApply = false`, and writes a denoised
//! AO term to a ping-pong target. The kernel uses the symmetricity correction and edge-leak
//! step, and the diagonal weights derive from the two cardinal edges
//! that straddle each diagonal (factored into the shared
//! `renderide::post::gtao_filter` module so this shader and `gtao_apply` cannot drift).
//!
//! Intermediate iterations use `blur_amount = denoise_blur_beta / 5.0`; the apply stage uses the
//! full beta. The output stays in the production-scaled `[0, 1] / OCCLUSION_TERM_SCALE`
//! representation; the headroom factor is only removed in the final-apply stage.
//!
//! Build script composes this into `gtao_denoise_default` (mono) and `gtao_denoise_multiview`
//! (stereo).
//!
//! Bind group (`@group(0)`):
//! - `@binding(0)` AO term (`texture_2d_array<f32>`).
//! - `@binding(1)` packed edges (`texture_2d_array<f32>`).
//! - `@binding(2)` `GtaoParams` uniform (only `denoise_blur_beta` is consumed here).

#import renderide::core::fullscreen as fs
#import renderide::post::gtao_filter as gf
#import renderide::post::gtao_params as gparams
#import renderide::post::gtao_textures as gt

@group(0) @binding(0) var ao_term: texture_2d_array<f32>;
@group(0) @binding(1) var ao_edges: texture_2d_array<f32>;

@group(0) @binding(2) var<uniform> gtao: gparams::GtaoParams;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> fs::FullscreenVertexOutput {
    return fs::vertex_main(vid);
}

/// Runs the bilateral kernel at `pix`. Returns the denoised AO term in the same
/// production-scaled `[0, 1]` representation as the input; intermediate iterations leave the
/// `OCCLUSION_TERM_SCALE` factor in place.
fn denoise_at(pix: vec2<i32>, view_layer: u32, viewport_max: vec2<i32>) -> f32 {
    let edges_c = gt::load_edges_lrtb(ao_edges, pix, view_layer, viewport_max);
    let edges_l = gt::load_edges_lrtb(ao_edges, pix + vec2<i32>(-1, 0), view_layer, viewport_max);
    let edges_r = gt::load_edges_lrtb(ao_edges, pix + vec2<i32>( 1, 0), view_layer, viewport_max);
    let edges_t = gt::load_edges_lrtb(ao_edges, pix + vec2<i32>(0, -1), view_layer, viewport_max);
    let edges_b = gt::load_edges_lrtb(ao_edges, pix + vec2<i32>(0,  1), view_layer, viewport_max);

    var edges_c_sym = gf::gtao_symmetricise_edges(edges_c, edges_l, edges_r, edges_t, edges_b);
    edges_c_sym = gf::gtao_apply_edge_leak(edges_c_sym);
    let diag = gf::gtao_diagonal_weights(edges_c_sym, edges_l, edges_r, edges_t, edges_b);

    let ao = gf::GtaoKernelAo(
        gt::load_ao(ao_term, pix, view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>(-1, 0), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>( 1, 0), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>(0, -1), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>(0,  1), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>(-1, -1), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>( 1, -1), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>(-1,  1), view_layer, viewport_max),
        gt::load_ao(ao_term, pix + vec2<i32>( 1,  1), view_layer, viewport_max),
    );

    let blur_amount = max(gtao.denoise_blur_beta, 1e-4);
    let denoised = gf::gtao_denoise_kernel(edges_c_sym, diag, ao, blur_amount);
    return clamp(denoised, 0.0, 1.0);
}

#ifdef MULTIVIEW
@fragment
fn fs_main(in: fs::FullscreenVertexOutput, @builtin(view_index) view: u32) -> @location(0) vec4<f32> {
    let dim = textureDimensions(ao_term);
    let viewport_max = vec2<i32>(i32(dim.x) - 1, i32(dim.y) - 1);
    let pix = vec2<i32>(in.clip_pos.xy);
    let ao = denoise_at(pix, view, viewport_max);
    return vec4<f32>(ao, 0.0, 0.0, 1.0);
}
#else
@fragment
fn fs_main(in: fs::FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dim = textureDimensions(ao_term);
    let viewport_max = vec2<i32>(i32(dim.x) - 1, i32(dim.y) - 1);
    let pix = vec2<i32>(in.clip_pos.xy);
    let ao = denoise_at(pix, 0u, viewport_max);
    return vec4<f32>(ao, 0.0, 0.0, 1.0);
}
#endif
