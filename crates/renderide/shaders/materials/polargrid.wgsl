//! Unity unlit `Shader "Unlit/PolarGrid"`: procedural polar grid visualizing radius bands and density.
//!
//! No `#pragma multi_compile` user keywords on this shader; `_RenderideVariantBits` is
//! reserved for layout consistency with the rest of the embedded materials and is never read.

#import renderide::frame::globals as rg
#import renderide::mesh::vertex as mv

struct PolarGridMaterial {
    _RenderideVariantBits: u32,
    _pad0: vec3<u32>,
}

@group(1) @binding(0) var<uniform> mat: PolarGridMaterial;

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> mv::UvVertexOutput {
#ifdef MULTIVIEW
    return mv::uv_vertex_main(instance_index, view_idx, pos, uv);
#else
    return mv::uv_vertex_main(instance_index, 0u, pos, uv);
#endif
}

//#pass type=forward
@fragment
fn fs_main(
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let centered = uv * 2.0 - 1.0;
    let radius = length(centered);
    let ref_radius = round(radius * 100.0) / 100.0;
    var d = abs(radius - ref_radius) * 100.0;
    let aaf = fwidth(d);
    let raaf = fwidth(radius * 100.0);
    d = 1.0 - smoothstep(0.05 - aaf, 0.05, d);
    let debug = smoothstep(0.15, 0.25, raaf);
    let band = d - debug;
    let col = vec3<f32>(band * debug, band * (1.0 - debug), 0.0);
    // Touch the renderer-reserved uniform so naga-oil keeps the binding live across import pruning.
    let touch = f32(mat._RenderideVariantBits) * 0.0;
    return rg::retain_globals_additive(vec4<f32>(col + vec3<f32>(touch), 1.0));
}
