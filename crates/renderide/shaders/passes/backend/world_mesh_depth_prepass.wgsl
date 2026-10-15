//! Generic world-mesh depth prepass.
//!
//! Uses the same per-draw slab packing and position transform convention as material forward
//! vertex stages, but emits only clip-space position and relies on fixed-function depth writes.

#import renderide::draw::types as dt

@group(0) @binding(0) var<storage, read> instances: array<dt::PerDrawUniforms>;

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
    return out;
}

@fragment
fn fs_main() {
}
