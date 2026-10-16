//! Poincare-disk {p,q} tiling. `CustomShaders.ExamplePoincareWgsl` is this file.
//!
//! `_Color` tints one tile parity. `_MainTex` is sampled with `_MainTex_ST` at the folded point.
//! `_Schlafli.x` is p and `_Schlafli.y` is q (default {7,3}). `_Speed` (default 0.15) rotates the
//! disk from `rg::frame.frame_time.x`. `//#keyword TILE` off writes `_MainTex * _Color`.

//#keyword TILE
//#render_queue Geometry
//#mat_default _Color vec4 0.95 0.82 0.45 1.0
//#mat_default _Schlafli vec4 7.0 3.0 0.0 0.0
//#mat_default _Speed float 0.15

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Schlafli: vec4<f32>,
    _Speed: f32,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Fold {
    z: vec2<f32>,
    flips: f32,
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

fn c_mul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn reflect_circle(z: vec2<f32>, center: f32, radius2: f32) -> vec2<f32> {
    let c = vec2<f32>(center, 0.0);
    let d = z - c;
    return c + radius2 * vec2<f32>(d.x, -d.y) / max(dot(d, d), 1e-6);
}

fn poincare_fold(z_in: vec2<f32>, p_in: f32, q_in: f32) -> Fold {
    let pi = 3.14159265;
    let p = clamp(p_in, 3.0, 16.0);
    let q = clamp(q_in, 3.0, 16.0);
    let center = max(cos(pi / q) / max(sin(pi / p), 1e-3), 1.02);
    let radius2 = center * center - 1.0;
    let sector = 6.2831853 / p;
    var z = z_in;
    var flips = 0.0;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let rad = length(z);
        var ang = atan2(z.y, z.x);
        ang = ang - sector * floor(ang / sector);
        if (ang < 0.0) {
            ang = ang + sector;
        }
        if (ang > sector * 0.5) {
            ang = sector - ang;
            flips = flips + 1.0;
        }
        z = rad * vec2<f32>(cos(ang), sin(ang));
        let c = vec2<f32>(center, 0.0);
        if (dot(z - c, z - c) < radius2) {
            z = reflect_circle(z, center, radius2);
            flips = flips + 1.0;
        }
    }
    var out: Fold;
    out.z = z;
    out.flips = flips;
    return out;
}

//#pass type=forward
@fragment
fn fs_main(vtx: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vtx.uv * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
#ifdef TILE
    let time = rg::frame.frame_time.x * mat._Speed;
    let rot = vec2<f32>(cos(time), sin(time));
    let z = c_mul((uv - vec2<f32>(0.5)) * 2.0, rot);
    let fold = poincare_fold(z / max(length(z), 1e-4) * min(length(z), 0.999), mat._Schlafli.x, mat._Schlafli.y);
    let map_uv = (fold.z * 0.5 + vec2<f32>(0.5)) * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let folded = textureSample(_MainTex, _MainTex_sampler, map_uv);
    let parity = step(0.25, fract(fold.flips * 0.5));
    let tile = mix(vec3<f32>(0.07, 0.09, 0.16), mat._Color.rgb, parity);
    let edge = smoothstep(0.08, 0.0, min(abs(atan2(fold.z.y, fold.z.x)), 0.08));
    let rgb = tile * (0.45 + 0.55 * folded.rgb) * (1.0 - 0.35 * edge);
    let inside = select(0.0, 1.0, length(z) < 0.992);
    let color = vec4<f32>(mix(vec3<f32>(0.015, 0.012, 0.02), rgb, inside), mat._Color.a * tex.a);
#else
    let color = tex * mat._Color;
#endif
    return rg::retain_globals_additive(color);
}
