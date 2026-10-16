//! Domain-warped fbm nebula. `CustomShaders.ExampleNebulaWgsl` is this file.
//!
//! `_Color` tints the nebula. `_MainTex` is sampled with `_MainTex_ST` and multiplies the result.
//! `_Warp.x` is the domain-warp amount and `_Warp.y` is the pattern scale. `_Speed` (default 0.12)
//! scrolls the domain from `rg::frame.frame_time.x`. `//#keyword NEBULA` off writes
//! `_MainTex * _Color`.

//#keyword NEBULA
//#render_queue Geometry
//#mat_default _Color vec4 0.85 0.9 1.0 1.0
//#mat_default _Warp vec4 1.8 3.0 0.0 0.0
//#mat_default _Speed float 0.12

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Warp: vec4<f32>,
    _Speed: f32,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
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
    out.uv = uv;
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    let ix = bitcast<u32>(p.x);
    let iy = bitcast<u32>(p.y);
    var h = ix * 1597334677u ^ iy * 3812015801u;
    h = (h >> 16u) ^ h;
    h = h * 0x7feb352du;
    h = (h >> 15u) ^ h;
    h = h * 0x846ca68bu;
    h = (h >> 16u) ^ h;
    return f32(h) / 4294967296.0;
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var value = 0.0;
    var amp = 0.5;
    var p = p_in;
    for (var octave = 0u; octave < 5u; octave = octave + 1u) {
        value = value + amp * value_noise(p);
        p = p * 2.02 + vec2<f32>(1.7, 9.2);
        amp = amp * 0.5;
    }
    return value;
}

//#pass type=forward
@fragment
fn fs_main(vtx: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vtx.uv * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
#ifdef NEBULA
    let time = rg::frame.frame_time.x * mat._Speed;
    let scale = max(mat._Warp.y, 0.25);
    let warp = mat._Warp.x;
    let p = uv * scale + vec2<f32>(time, time * 0.37);
    let q = vec2<f32>(fbm(p), fbm(p + vec2<f32>(5.2, 1.3)));
    let r = vec2<f32>(
        fbm(p + warp * q + vec2<f32>(1.7, 9.2)),
        fbm(p + warp * q + vec2<f32>(8.3, 2.8)),
    );
    let f = fbm(p + warp * r);
    let deep = vec3<f32>(0.04, 0.02, 0.1);
    let mid = vec3<f32>(0.18, 0.28, 0.75);
    let hot = vec3<f32>(0.95, 0.55, 0.28);
    var rgb = mix(deep, mid, clamp(f * f * 3.2, 0.0, 1.0));
    rgb = mix(rgb, hot, clamp(length(q) * 0.65, 0.0, 1.0));
    rgb = mix(rgb, vec3<f32>(1.0, 0.92, 0.75), clamp(r.x * r.x * 1.4, 0.0, 1.0));
    let color = vec4<f32>(rgb * mat._Color.rgb * (0.35 + 0.65 * tex.rgb), mat._Color.a * tex.a);
#else
    let color = tex * mat._Color;
#endif
    return rg::retain_globals_additive(color);
}
