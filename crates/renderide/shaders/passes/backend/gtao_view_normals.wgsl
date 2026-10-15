//! World-mesh normal prepass for GTAO.
//!
//! Writes smooth vertex normals in the view-space convention consumed by `gtao_main`.

#import renderide::core::math as rmath
#import renderide::frame::view_basis as vb
#import renderide::draw::types as dt

@group(0) @binding(0) var<storage, read> instances: array<dt::PerDrawUniforms>;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) view_n: vec3<f32>,
}

fn gtao_view_normal_from_world(world_n: vec3<f32>, vp: mat4x4<f32>) -> vec3<f32> {
    let basis = vb::from_view_projection(vp);
    return rmath::safe_normalize(vec3<f32>(
        dot(world_n, basis.x),
        dot(world_n, basis.y),
        -dot(world_n, basis.z),
    ), vec3<f32>(0.0, 0.0, -1.0));
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) n: vec4<f32>,
) -> VertexOutput {
    let draw = instances[instance_index];
#ifdef MULTIVIEW
    let view_layer = view_idx;
#else
    let view_layer = 0u;
#endif
    let vp = dt::select_view_proj(draw, view_layer);
    let world_p = draw.model * vec4<f32>(pos.xyz, 1.0);
    let world_n = rmath::safe_normalize(draw.normal_matrix * n.xyz, vec3<f32>(0.0, 1.0, 0.0));

    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    out.view_n = gtao_view_normal_from_world(world_n, vp);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(rmath::safe_normalize(in.view_n, vec3<f32>(0.0, 0.0, -1.0)), 1.0);
}
