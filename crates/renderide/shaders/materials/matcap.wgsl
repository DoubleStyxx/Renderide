//! Matcap (`Shader "Matcap"`): tangent-space normal map, view-space normal matcap lookup.
//!
//! Desktop and in-world cameras match Resonite `Assets/Shaders/Common/Matcap.shader`:
//! `mul((float3x3)UNITY_MATRIX_V, n).xy * 0.5 + 0.5` with that eye's view matrix. The 3x3 drops
//! translation, so parallel HMD eyes sample one UV. That is the reference result, not a defect.
//!
//! The multiview permutation deliberately departs from that reference. The vertex writes this
//! eye's view-space position from the world-to-view rows, and the fragment samples
//! `vec2(-cross(view_dir, n_view).y, cross(view_dir, n_view).x) * 0.5 + 0.5`.
//! `view_dir_for_world_pos` is not used: it is a world-space direction toward
//! `camera_world_pos_for_view` and does not follow render-space scale the way those rows do.
//! On a face-on wall, a fragment 45 degrees off the optical axis stays at UV (0.5, 0.5) in the
//! reference lookup and moves by `0.5 / sqrt(2)` (about 0.354) in U under this departure.


//#texture_default _MainTex white
//#texture_default _NormalMap bump

#import renderide::frame::globals as rg
#import renderide::draw::per_draw as pd
#import renderide::core::math as rmath
#import renderide::material::variant_bits as varb
#import renderide::mesh::vertex as mv
#import renderide::core::normal_decode as nd
#import renderide::pbs::normal as pnorm
#import renderide::core::uv as uvu
#import renderide::core::texture_sampling as ts

struct MatcapMaterial {
    _RenderideVariantBits: u32,
    _MainTex_LodBias: f32,
    _NormalMap_LodBias: f32,
    _NormalMap_ST: vec4<f32>,
}

const MATCAP_KW_NORMALMAP: u32 = 1u << 0u;

@group(1) @binding(0) var<uniform> mat: MatcapMaterial;
@group(1) @binding(1) var _MainTex: texture_2d<f32>;
@group(1) @binding(2) var _MainTex_sampler: sampler;
@group(1) @binding(3) var _NormalMap: texture_2d<f32>;
@group(1) @binding(4) var _NormalMap_sampler: sampler;

fn matcap_kw(mask: u32) -> bool {
    return varb::enabled(mat._RenderideVariantBits, mask);
}

fn kw_NORMALMAP() -> bool {
    return matcap_kw(MATCAP_KW_NORMALMAP);
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv_normal: vec2<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) world_t: vec3<f32>,
    @location(3) world_b: vec3<f32>,
    @location(4) @interpolate(flat) view_layer: u32,
#ifdef MULTIVIEW
    @location(5) view_pos: vec3<f32>,
#endif
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec4<f32>,
    @location(1) n: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(4) tangent: vec4<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
#ifdef MULTIVIEW
    let view_layer = view_idx;
#else
    let view_layer = 0u;
#endif
    let world_p = mv::world_position(d, pos);
    let world_n = rmath::safe_normalize(d.normal_matrix * n.xyz, vec3<f32>(0.0, 1.0, 0.0));
    let tbn = pnorm::orthonormal_tbn(world_n, mv::world_tangent(d, tangent));
    let vp = mv::select_view_proj(d, view_layer);

    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    out.uv_normal = uvu::apply_st(uv0, mat._NormalMap_ST);
    out.world_n = world_n;
    out.world_t = tbn[0];
    out.world_b = tbn[1];
    out.view_layer = view_layer;
#ifdef MULTIVIEW
    // Same rows as the mesh view matrix. Not `view_dir_for_world_pos` (world-space, no scale).
    out.view_pos = rg::world_to_view_position_for_view(world_p.xyz, view_layer);
#endif
    return out;
}

//#pass type=forward blend=material_filter
@fragment
fn fs_main(
    @location(0) uv_normal: vec2<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) world_t: vec3<f32>,
    @location(3) world_b: vec3<f32>,
    @location(4) @interpolate(flat) view_layer: u32,
#ifdef MULTIVIEW
    @location(5) view_pos: vec3<f32>,
#endif
) -> @location(0) vec4<f32> {
    var normal_ts = vec3<f32>(0.0, 0.0, 1.0);
    if (kw_NORMALMAP()) {
        normal_ts = nd::decode_ts_normal_with_placeholder_sample(
            ts::sample_tex_2d(_NormalMap, _NormalMap_sampler, uv_normal, mat._NormalMap_LodBias),
            1.0,
        );
    }
    let tbn = mat3x3<f32>(
        rmath::safe_normalize(world_t, vec3<f32>(1.0, 0.0, 0.0)),
        rmath::safe_normalize(world_b, vec3<f32>(0.0, 0.0, 1.0)),
        rmath::safe_normalize(world_n, vec3<f32>(0.0, 1.0, 0.0)),
    );
    let n_world = rmath::safe_normalize(tbn * normal_ts, world_n);
    let n_view = rg::world_to_view_normal_for_view(n_world, view_layer);
#ifdef MULTIVIEW
    let uv = rg::matcap_uv_for_view(n_view, view_pos, view_layer);
#else
    let uv = rg::matcap_uv_for_view(n_view, vec3<f32>(0.0), view_layer);
#endif
    let col = ts::sample_tex_2d(_MainTex, _MainTex_sampler, uv, mat._MainTex_LodBias);
    return rg::retain_globals_additive(col);
}
