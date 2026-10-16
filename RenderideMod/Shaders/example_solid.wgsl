//! Example custom material. `CustomShaders.Install` sends this text to the renderer.
//!
//! The fragment color is the host material `_Color` property. `_Color` is a stock
//! FrooxEngine property, so an existing Unlit material lights up with this program
//! when the shader key is `Unlit`.

//#render_queue Geometry
//#mat_default _Color vec4 1.0 0.35 0.1 1.0

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::mesh::vertex as mv

struct Material {
    _Color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> mat: Material;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) _n: vec4<f32>,
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
    return out;
}

//#pass type=forward
@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    return rg::retain_globals_additive(mat._Color);
}
