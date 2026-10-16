//! Rainbow custom material. `CustomShaders.ExampleRainbowWgsl` is this file.
//!
//! `_Color` tints the sample. `_MainTex` is sampled with `_MainTex_ST` tiling and offset.
//! `_Speed` (default 0.2) scales the hue cycle. Time is `rg::frame.frame_time.x`.
//! Hue is world x plus that scaled time.
//! `//#keyword RAINBOW` off writes `_MainTex * _Color`. On multiplies that by the hue cycle.

//#keyword RAINBOW
//#render_queue Geometry
//#mat_default _Color vec4 1.0 1.0 1.0 1.0
//#mat_default _Speed float 0.2

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Speed: f32,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_x: f32,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, pos);
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    out.world_x = world_p.x;
    out.uv = uv;
    return out;
}

fn rainbow(hue: f32) -> vec3<f32> {
    let h = fract(hue);
    let r = abs(h * 6.0 - 3.0) - 1.0;
    let g = 2.0 - abs(h * 6.0 - 2.0);
    let b = 2.0 - abs(h * 6.0 - 4.0);
    return clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
}

//#pass type=forward
@fragment
fn fs_main(vtx: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vtx.uv * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
    let tinted = tex * mat._Color;
#ifdef RAINBOW
    let hue = vtx.world_x + rg::frame.frame_time.x * mat._Speed;
    let color = vec4<f32>(rainbow(hue), tinted.a) * tinted;
#else
    let color = tinted;
#endif
    return rg::retain_globals_additive(color);
}
