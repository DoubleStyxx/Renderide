//! Egyptian gold hieroglyph emissive. `CustomShaders.ExampleHieroglyphWgsl` is this file.
//!
//! A procedural glyph grid is tinted by `_Color` (default gold). `_MainTex` is sampled with
//! `_MainTex_ST` and multiplies the glyph mask. `_Grid.xy` is the cell count. `_Emission` scales
//! the pulse. `_Speed` (default 0.4) drives that pulse from `rg::frame.frame_time.x`.
//! `//#keyword EMIT` off writes `_MainTex * _Color`.

//#keyword EMIT
//#render_queue Geometry
//#mat_default _Color vec4 1.0 0.74 0.28 1.0
//#mat_default _Grid vec4 8.0 8.0 0.0 0.0
//#mat_default _Emission float 1.4
//#mat_default _Speed float 0.4

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Grid: vec4<f32>,
    _Emission: f32,
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

fn stroke_h(cell: vec2<f32>, y: f32, half: f32) -> f32 {
    let on = abs(cell.y - y) < half && cell.x > 0.18 && cell.x < 0.82;
    return select(0.0, 1.0, on);
}

fn stroke_v(cell: vec2<f32>, x: f32, half: f32) -> f32 {
    let on = abs(cell.x - x) < half && cell.y > 0.18 && cell.y < 0.82;
    return select(0.0, 1.0, on);
}

fn glyph_mask(cell: vec2<f32>, seed: f32) -> f32 {
    let kind = floor(seed * 6.0);
    let bars = max(stroke_h(cell, 0.32, 0.045), stroke_h(cell, 0.68, 0.04));
    let column = max(stroke_v(cell, 0.5, 0.05), stroke_h(cell, 0.5, 0.04));
    let frame = select(
        0.0,
        1.0,
        (cell.x > 0.2 && cell.x < 0.8 && cell.y > 0.2 && cell.y < 0.8)
            && (cell.x < 0.3 || cell.x > 0.7 || cell.y < 0.3 || cell.y > 0.7),
    );
    let cross = max(stroke_h(cell, 0.5, 0.04), stroke_v(cell, 0.5, 0.04));
    let diag = select(0.0, 1.0, abs(cell.x - cell.y) < 0.055 && cell.x > 0.2 && cell.x < 0.8);
    var mask = bars;
    mask = select(mask, column, kind >= 1.0 && kind < 2.0);
    mask = select(mask, frame, kind >= 2.0 && kind < 3.0);
    mask = select(mask, cross, kind >= 3.0 && kind < 4.0);
    mask = select(mask, max(diag, stroke_h(cell, 0.72, 0.035)), kind >= 4.0 && kind < 5.0);
    mask = select(mask, max(stroke_v(cell, 0.36, 0.04), stroke_h(cell, 0.62, 0.035)), kind >= 5.0);
    let border = select(0.0, 0.45, cell.x < 0.05 || cell.x > 0.95 || cell.y < 0.05 || cell.y > 0.95);
    return clamp(max(mask, border), 0.0, 1.0);
}

//#pass type=forward
@fragment
fn fs_main(vtx: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vtx.uv * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
#ifdef EMIT
    let grid = max(mat._Grid.xy, vec2<f32>(1.0));
    let cell_id = floor(uv * grid);
    let cell = fract(uv * grid);
    let mask = glyph_mask(cell, hash21(cell_id)) * tex.r;
    let time = rg::frame.frame_time.x * mat._Speed;
    let pulse = 0.62 + 0.38 * sin(time * 6.2831853);
    let n = normalize(vtx.world_n + vec3<f32>(0.0, 0.0, 1e-4));
    let view = rg::view_dir_for_world_pos(vtx.world_pos, vtx.view_layer);
    let ndotv = clamp(dot(n, view), 0.0, 1.0);
    let fresnel = pow(1.0 - ndotv, 4.0);
    let spec = pow(ndotv, 28.0);
    let albedo = vec3<f32>(1.0, 0.74, 0.28) * mat._Color.rgb;
    let base = albedo * (0.16 + 0.42 * ndotv);
    let emissive = albedo * mask * mat._Emission * pulse;
    let sheen = albedo * spec * 0.55 + albedo * fresnel * 0.35;
    let color = vec4<f32>(base + emissive + sheen, mat._Color.a * tex.a);
#else
    let color = tex * mat._Color;
#endif
    return rg::retain_globals_additive(color);
}
