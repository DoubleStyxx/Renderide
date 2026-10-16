//! Chromatic dispersion. `CustomShaders.ExampleDispersionWgsl` is this file.
//!
//! `_Color` tints the sample. `_MainTex` is sampled three times with `_MainTex_ST`: red and blue
//! are offset along a refraction axis, green stays on the geometric UV. `_Spread` is the offset
//! distance. `_Speed` (default 0.25) rotates that axis from `rg::frame.frame_time.x`.
//! `//#keyword SPLIT` off writes one `_MainTex * _Color` sample.

//#keyword SPLIT
//#render_queue Geometry
//#mat_default _Color vec4 1.0 1.0 1.0 1.0
//#mat_default _Spread float 0.015
//#mat_default _Speed float 0.25

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Spread: f32,
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
#ifdef SPLIT
    let time = rg::frame.frame_time.x * mat._Speed;
    let view = rg::view_dir_for_world_pos(vtx.world_pos, vtx.view_layer);
    let n = normalize(vtx.world_n + vec3<f32>(0.0, 0.0, 1e-4));
    let fresnel = pow(1.0 - clamp(abs(dot(n, view)), 0.0, 1.0), 2.0);
    let spin = vec2<f32>(cos(time), sin(time));
    let axis = normalize(n.xy + spin + vec2<f32>(1e-3, 0.0));
    let delta = axis * mat._Spread * (0.35 + 0.65 * fresnel);
    let red = textureSample(_MainTex, _MainTex_sampler, uv + delta);
    let green = textureSample(_MainTex, _MainTex_sampler, uv);
    let blue = textureSample(_MainTex, _MainTex_sampler, uv - delta * 1.35);
    let rgb = vec3<f32>(red.r, green.g, blue.b) * mat._Color.rgb;
    let color = vec4<f32>(rgb, mat._Color.a * green.a);
#else
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
    let color = tex * mat._Color;
#endif
    return rg::retain_globals_additive(color);
}
