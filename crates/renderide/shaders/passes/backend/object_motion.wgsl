//! Rigid object velocity composited into the camera motion-vector target.
//!
//! The vertex stage re-rasterizes the position stream with the current view-projection so the
//! depth-equal test hits the forward depth. It writes full screen-space velocity (object plus
//! camera) for draws whose per-draw row carries a previous model. The blur shader already samples
//! this target and keeps camera reprojection wherever this pass does not overwrite.

#import renderide::draw::types as dt

struct PreviousViewProj {
    left: mat4x4<f32>,
    right: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) velocity: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<dt::PerDrawUniforms>;
@group(1) @binding(0) var<uniform> previous_vp: PreviousViewProj;

fn clip_xy_to_uv(clip_xy: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(clip_xy.x * 0.5 + 0.5, 0.5 - clip_xy.y * 0.5);
}

fn select_previous_view_proj(view_idx: u32) -> mat4x4<f32> {
    if (view_idx == 0u) {
        return previous_vp.left;
    }
    return previous_vp.right;
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
) -> VertexOutput {
#ifdef MULTIVIEW
    let view_layer = view_idx;
#else
    let view_layer = 0u;
#endif
    let draw = instances[instance_index];
    let local = vec4<f32>(pos.xyz, 1.0);
    let current_clip = dt::select_view_proj(draw, view_layer) * (draw.model * local);
    var velocity = vec2<f32>(0.0);
    if (dt::previous_model_valid(draw)) {
        let previous_clip = select_previous_view_proj(view_layer) * (dt::previous_model(draw) * local);
        if (abs(current_clip.w) >= 1e-6 && abs(previous_clip.w) >= 1e-6) {
            let current_uv = clip_xy_to_uv(current_clip.xy / current_clip.w);
            let previous_uv = clip_xy_to_uv(previous_clip.xy / previous_clip.w);
            velocity = previous_uv - current_uv;
        }
    }

    var out: VertexOutput;
    out.clip_pos = current_clip;
    out.velocity = velocity;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec2<f32> {
    return in.velocity;
}
