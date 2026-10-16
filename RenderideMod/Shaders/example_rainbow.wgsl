//! Rainbow custom material. `CustomShaders.Install` sends this text to the renderer.
//!
//! Host `_Color` tints the result, `_MainTex` is sampled and multiplied, and `_Speed`
//! scales the hue cycle. Time comes from `rg::frame.frame_time.x`. Hue comes from world x.
//! Those names are the material properties FrooxEngine already uploads.

//#render_queue Geometry
//#mat_default _Color vec4 1.0 1.0 1.0 1.0
//#mat_default _Speed float 0.2

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
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
    let hue = vtx.world_x + rg::frame.frame_time.x * mat._Speed;
    let tex = textureSample(_MainTex, _MainTex_sampler, vtx.uv);
    let color = vec4<f32>(rainbow(hue), 1.0) * mat._Color * tex;
    return rg::retain_globals_additive(color);
}
