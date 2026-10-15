//! Fullscreen pass: applies analytic AgX tonemap (HDR linear -> display-referred linear in
//! [0, 1]) to the HDR scene color and writes the post-processing chain output.
//!
//! Build script composes this source into `agx_tonemap_default` (mono) and
//! `agx_tonemap_multiview` (stereo, `view_index` selects array layer) targets.

#import renderide::core::fullscreen as fs

const AGX_MIN_EV: f32 = -12.47393;
const AGX_MAX_EV: f32 = 4.026069;

@group(0) @binding(0) var scene_color_hdr: texture_2d_array<f32>;
@group(0) @binding(1) var scene_color_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> fs::FullscreenVertexOutput {
    return fs::vertex_main(vid);
}

// Blender AgX Rec.2020 inset matrix. WGSL mat3x3 constructors take column vectors, so these
// coefficients intentionally keep the source column grouping.
fn agx_inset_matrix() -> mat3x3<f32> {
    return mat3x3<f32>(
        vec3<f32>(0.8566271533, 0.1373189729, 0.1118982130),
        vec3<f32>(0.0951212405, 0.7612419906, 0.0767994186),
        vec3<f32>(0.0482516061, 0.1014390365, 0.8113023684),
    );
}

// Inverse Blender AgX Rec.2020 outset matrix.
fn agx_outset_matrix() -> mat3x3<f32> {
    return mat3x3<f32>(
        vec3<f32>( 1.12710058, -0.14132976, -0.14132976),
        vec3<f32>(-0.11060664,  1.15782370, -0.11060664),
        vec3<f32>(-0.01649394, -0.01649394,  1.25193641),
    );
}

fn agx_default_contrast_approx(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x6 = x4 * x2;
    return -17.86 * x6 * x
        + 78.01 * x6
        - 126.7 * x4 * x
        + 92.06 * x4
        - 28.72 * x2 * x
        + 4.361 * x2
        - 0.1718 * x
        + vec3<f32>(0.002857);
}

fn agx_tonemap(color_linear: vec3<f32>) -> vec3<f32> {
    var v = max(color_linear, vec3<f32>(0.0));
    v = agx_inset_matrix() * v;
    v = max(v, vec3<f32>(1e-10));
    v = log2(v);
    v = (v - vec3<f32>(AGX_MIN_EV)) / (AGX_MAX_EV - AGX_MIN_EV);
    v = clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
    v = agx_default_contrast_approx(v);
    v = agx_outset_matrix() * v;
    v = pow(max(v, vec3<f32>(0.0)), vec3<f32>(2.2));
    return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
}

#ifdef MULTIVIEW
@fragment
fn fs_main(in: fs::FullscreenVertexOutput, @builtin(view_index) view: u32) -> @location(0) vec4<f32> {
    let hdr = textureSample(scene_color_hdr, scene_color_sampler, in.uv, view);
    let ldr = agx_tonemap(hdr.rgb);
    return vec4<f32>(ldr, hdr.a);
}
#else
@fragment
fn fs_main(in: fs::FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let hdr = textureSample(scene_color_hdr, scene_color_sampler, in.uv, 0u);
    let ldr = agx_tonemap(hdr.rgb);
    return vec4<f32>(ldr, hdr.a);
}
#endif
