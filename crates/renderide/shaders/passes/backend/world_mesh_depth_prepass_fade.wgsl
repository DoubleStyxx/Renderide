//! Depth prepass for draws inside an LOD cross-fade.
//!
//! The default `world_mesh_depth_prepass.wgsl` stays free of the fade varying and the fragment
//! discard. This variant is selected only for draws whose stipple weight is non-zero, so a view
//! with cross-fade off keeps the historical depth pipeline.

#import renderide::draw::types as dt
#import renderide::draw::lod_fade as lf

@group(0) @binding(0) var<storage, read> instances: array<dt::PerDrawUniforms>;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) @interpolate(flat) lod_fade: f32,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
) -> VertexOutput {
    let draw = instances[instance_index];
#ifdef MULTIVIEW
    let view_layer = view_idx;
#else
    let view_layer = 0u;
#endif
    let vp = dt::select_view_proj(draw, view_layer);
    let world_p = draw.model * vec4<f32>(pos.xyz, 1.0);

    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    out.lod_fade = draw.particle_padding[5].x;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) {
    if (lf::lod_fade_discards(in.clip_pos.xy, in.lod_fade)) {
        discard;
    }
}
