//! #216 term-isolation material. Installed only by the opt-in `warp_216_debug` cases.
//!
//! `_TERM_DEBUG` pulls `shade_metallic_term_debug` into this compose. Production
//! `pbsmetallic.wgsl` does not define that keyword, so the function is not in
//! those shaders.
//!
//! `_TermId` selects the term. `_Channel` selects the encoding for isolated terms:
//! 0 writes the signal in R, 1 in G, 2 in B, 3 replicates it into RGB (grey).
//! Composed (`0`) writes the production sum. Constant (`7`) writes `(0.25, 0.5, 0.75)`
//! and does not keep the frame-binding epsilon.

//#keyword _TERM_DEBUG
//#render_queue Geometry
//#mat_default _Color vec4 0.72 0.38 0.22 1.0
//#mat_default _Metallic float 0.0
//#mat_default _Glossiness float 0.62
//#mat_default _TermId float 0.0
//#mat_default _Channel float 0.0

#import renderide::draw::per_draw as pd
#import renderide::frame::globals as rg
#import renderide::mesh::vertex as mv
#import renderide::pbs::lighting as plight
#import renderide::pbs::standard as pstd
#import renderide::pbs::surface as psurf

const DEBUG_TERM_COMPOSED: u32 = 0u;
const DEBUG_TERM_CONSTANT: u32 = 7u;
const DEBUG_CHANNEL_G: u32 = 1u;
const DEBUG_CHANNEL_B: u32 = 2u;
const DEBUG_CHANNEL_GREY: u32 = 3u;

struct TermDebugMaterial {
    _Color: vec4<f32>,
    _Metallic: f32,
    _Glossiness: f32,
    _TermId: f32,
    _Channel: f32,
}

@group(1) @binding(0) var<uniform> mat: TermDebugMaterial;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) @interpolate(flat) view_layer: u32,
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
    let draw = pd::get_draw(instance_index);
    let world_p = mv::world_position(draw, pos);
#ifdef MULTIVIEW
    let view_layer = view_idx;
#else
    let view_layer = 0u;
#endif
    var out: VertexOutput;
    out.clip_pos = mv::select_view_proj(draw, view_layer) * world_p;
    out.world_pos = world_p.xyz;
    out.world_n = mv::world_normal(draw, n);
    out.view_layer = mv::packed_view_layer(instance_index, view_layer);
    return out;
}

/// Signed component with the largest magnitude. Ties keep the earlier channel.
fn debug_term_signal(term: vec3<f32>) -> f32 {
    let ax = abs(term.x);
    let ay = abs(term.y);
    let az = abs(term.z);
    var signal = term.x;
    var peak = ax;
    if (ay > peak) {
        signal = term.y;
        peak = ay;
    }
    if (az > peak) {
        signal = term.z;
    }
    return signal;
}

fn encode_debug_channel(term: vec3<f32>, channel: u32) -> vec3<f32> {
    let signal = debug_term_signal(term);
    if (channel == DEBUG_CHANNEL_G) {
        return vec3<f32>(0.0, signal, 0.0);
    }
    if (channel == DEBUG_CHANNEL_B) {
        return vec3<f32>(0.0, 0.0, signal);
    }
    if (channel == DEBUG_CHANNEL_GREY) {
        return vec3<f32>(signal);
    }
    return vec3<f32>(signal, 0.0, 0.0);
}

//#pass type=forward
@fragment
fn fs_main(
    @builtin(position) frag_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) @interpolate(flat) view_layer: u32,
) -> @location(0) vec4<f32> {
    let term_id = u32(mat._TermId + 0.5);
    let channel = u32(mat._Channel + 0.5);
#ifdef _TERM_DEBUG
    if (term_id == DEBUG_TERM_CONSTANT) {
        // `retain_globals_additive` adds about 1e-10 so the frame bind group stays
        // complete. Treat values under 1e-6 as the authored zero.
        return rg::retain_globals_additive(vec4<f32>(0.25, 0.5, 0.75, 1.0));
    }
    let rough = pstd::roughness_from_smoothness(mat._Glossiness);
    let surface = psurf::metallic_with_geometric_normal(
        mat._Color.rgb,
        mat._Color.a,
        mat._Metallic,
        rough,
        1.0,
        world_n,
        world_n,
        vec3<f32>(0.0),
    );
    let options = plight::ClusterLightingOptions(true, true, true, true);
    let term = plight::shade_metallic_term_debug(
        frag_pos.xy,
        world_pos,
        view_layer,
        surface,
        options,
        term_id,
    );
    var color = term;
    if (term_id != DEBUG_TERM_COMPOSED) {
        color = encode_debug_channel(term, channel);
    }
    return rg::retain_globals_additive(vec4<f32>(color, 1.0));
#else
    return rg::retain_globals_additive(vec4<f32>(1.0, 0.0, 1.0, 1.0));
#endif
}
