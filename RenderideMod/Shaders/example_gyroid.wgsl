//! Raymarched gyroid SDF in object space. `CustomShaders.ExampleGyroidWgsl` is this file.
//!
//! `_Color` tints the isosurface. `_MainTex` is sampled with `_MainTex_ST`. `_Frequency` scales
//! the gyroid. `_Speed` (default 0.2) scrolls the phase from `rg::frame.frame_time.x`.
//! `//#keyword GYROID` off writes `_MainTex * _Color`.

//#keyword GYROID
//#render_queue Geometry
//#mat_default _Color vec4 0.25 0.75 0.95 1.0
//#mat_default _Frequency float 6.0
//#mat_default _Speed float 0.2

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _Frequency: f32,
    _Speed: f32,
}

@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) object_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) view_layer: u32,
    @location(3) @interpolate(flat) draw_index: u32,
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
    let view = view_idx;
    let vp = mv::select_view_proj(d, view_idx);
#else
    let view = 0u;
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    out.object_pos = pos.xyz;
    out.uv = uv;
    out.view_layer = mv::packed_view_layer(instance_index, view);
    out.draw_index = instance_index;
    return out;
}

fn mat3_inverse(m: mat3x3<f32>) -> mat3x3<f32> {
    let c0 = m[0];
    let c1 = m[1];
    let c2 = m[2];
    let r0 = cross(c1, c2);
    let r1 = cross(c2, c0);
    let r2 = cross(c0, c1);
    let det = dot(c0, r0);
    let inv_det = 1.0 / select(det, 1.0, abs(det) < 1e-8);
    return mat3x3<f32>(
        vec3<f32>(r0.x, r1.x, r2.x) * inv_det,
        vec3<f32>(r0.y, r1.y, r2.y) * inv_det,
        vec3<f32>(r0.z, r1.z, r2.z) * inv_det,
    );
}

fn world_to_object(model: mat4x4<f32>, world: vec3<f32>) -> vec3<f32> {
    let linear = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz);
    return mat3_inverse(linear) * (world - model[3].xyz);
}

fn safe_component(v: f32) -> f32 {
    if (abs(v) < 1e-5) {
        return select(-1e-5, 1e-5, v >= 0.0);
    }
    return v;
}

fn gyroid_field(p: vec3<f32>, freq: f32, phase: f32) -> f32 {
    let q = p * freq + vec3<f32>(phase, phase * 0.37, phase * 0.13);
    return dot(sin(q), cos(q.zxy));
}

fn gyroid_normal(p: vec3<f32>, freq: f32, phase: f32) -> vec3<f32> {
    let e = 0.004;
    let dx = gyroid_field(p + vec3<f32>(e, 0.0, 0.0), freq, phase) - gyroid_field(p - vec3<f32>(e, 0.0, 0.0), freq, phase);
    let dy = gyroid_field(p + vec3<f32>(0.0, e, 0.0), freq, phase) - gyroid_field(p - vec3<f32>(0.0, e, 0.0), freq, phase);
    let dz = gyroid_field(p + vec3<f32>(0.0, 0.0, e), freq, phase) - gyroid_field(p - vec3<f32>(0.0, 0.0, e), freq, phase);
    return normalize(vec3<f32>(dx, dy, dz) + vec3<f32>(1e-5, 0.0, 0.0));
}

fn ray_box(ro: vec3<f32>, rd: vec3<f32>, extent: f32) -> vec2<f32> {
    let safe = vec3<f32>(safe_component(rd.x), safe_component(rd.y), safe_component(rd.z));
    let inv = 1.0 / safe;
    let box = vec3<f32>(extent);
    let t0 = (-box - ro) * inv;
    let t1 = (box - ro) * inv;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    return vec2<f32>(max(max(tmin.x, tmin.y), tmin.z), min(min(tmax.x, tmax.y), tmax.z));
}

//#pass type=forward
@fragment
fn fs_main(vtx: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vtx.uv * mat._MainTex_ST.xy + mat._MainTex_ST.zw;
    let tex = textureSample(_MainTex, _MainTex_sampler, uv);
#ifdef GYROID
    let phase = rg::frame.frame_time.x * mat._Speed;
    let freq = max(mat._Frequency, 0.5);
    let draw = pd::get_draw(vtx.draw_index);
    let cam = world_to_object(draw.model, rg::camera_world_pos_for_view(vtx.view_layer));
    let delta = vtx.object_pos - cam;
    let rd = delta / max(length(delta), 1e-4);
    let bounds = ray_box(cam, rd, 0.5);
    let hit_box = bounds.y > max(bounds.x, 0.0);
    let enter = max(bounds.x, 0.0);
    let span = max(bounds.y - enter, 1e-3);
    var closest = 1e3;
    var closest_p = vtx.object_pos;
    for (var i = 0u; i < 40u; i = i + 1u) {
        let t = enter + (f32(i) + 0.5) * span / 40.0;
        let p = cam + rd * t;
        let field = abs(gyroid_field(p, freq, phase));
        if (hit_box && field < closest) {
            closest = field;
            closest_p = p;
        }
    }
    if (!hit_box) {
        closest_p = vtx.object_pos;
        closest = abs(gyroid_field(closest_p, freq, phase));
    }
    let n = gyroid_normal(closest_p, freq, phase);
    let shade = exp(-closest * freq * 0.85);
    let ndot = clamp(dot(n, normalize(vec3<f32>(0.35, 0.72, 0.6))), 0.0, 1.0);
    let facing = clamp(dot(n, -rd), 0.0, 1.0);
    let lit = mat._Color.rgb * tex.rgb * (0.15 + 0.85 * shade) * (0.22 + 0.78 * ndot);
    let spec = vec3<f32>(0.85, 0.95, 1.0) * pow(facing, 16.0) * shade;
    let color = vec4<f32>(lit + spec, mat._Color.a * tex.a);
#else
    let color = tex * mat._Color;
#endif
    return rg::retain_globals_additive(color);
}
