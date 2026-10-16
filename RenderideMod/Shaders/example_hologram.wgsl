//! Holographic Fresnel and scanlines. `CustomShaders.ExampleHologramWgsl` is this file.
//!
//! `_Color` tints the sample. `_MainTex` is sampled with `_MainTex_ST`. `_Scan.x` is line
//! density and `_Scan.y` is line contrast. `_Fresnel` is the Fresnel power. `_Speed` (default
//! 0.35) scrolls the lines from `rg::frame.frame_time.x`. `//#keyword HOLO` off writes
//! `_MainTex * _Color`.

//#keyword HOLO
//#render_queue Geometry
//#mat_default _Color vec4 0.75 0.9 1.0 1.0
//#mat_default _Scan vec4 42.0 0.55 0.0 0.0
//#mat_default _Fresnel float 3.0
//#mat_default _Speed float 0.35

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Scan: vec4<f32>,
    _Fresnel: f32,
    _Speed: f32,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) view_layer: u32,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) n: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let view = view_idx;
    let vp = mv::select_view_proj(d, view_idx);
#else
    let view = 0u;
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    out.world_pos = world_p.xyz;
    out.world_n = mv::world_normal(d, n);
    out.uv = uv;
    out.view_layer = mv::packed_view_layer(instance_index, view);
    return out;
}

//#pass type=forward
@fragment
fn fs_main(vtx: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vtx.uv * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
#ifdef HOLO
    let time = rg::frame.frame_time.x * mat._Speed;
    let view = rg::view_dir_for_world_pos(vtx.world_pos, vtx.view_layer);
    let ripple = vec3<f32>(sin(uv.x * 18.0 + time), sin(uv.y * 14.0 - time * 0.5), 0.0);
    let n = normalize(vtx.world_n + ripple * 0.45 + vec3<f32>(0.0, 0.0, 1e-4));
    let ndotv = clamp(abs(dot(n, view)), 0.0, 1.0);
    let fresnel = pow(1.0 - ndotv, max(mat._Fresnel, 0.5));
    let lines = 0.5 + 0.5 * sin((uv.y * mat._Scan.x + time * 8.0) * 6.2831853);
    let scan = mix(1.0 - clamp(mat._Scan.y, 0.0, 1.0), 1.0, lines);
    let holo = mix(vec3<f32>(0.12, 0.85, 1.0), vec3<f32>(1.0, 0.25, 0.82), fresnel);
    let rgb = tex.rgb * mat._Color.rgb * holo * scan;
    let color = vec4<f32>(rgb, mat._Color.a * tex.a);
#else
    let color = tex * mat._Color;
#endif
    return rg::retain_globals_additive(color);
}
