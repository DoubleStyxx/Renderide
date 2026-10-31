//! Unity Standard metallic PBS (`Shader "PBSMetallic"`): forward base + forward additive.
//!
//! This mirrors the built-in Standard metallic forward passes within the renderer's forward path:
//! `FORWARD` writes ambient/emission plus directional lighting, and `FORWARD_DELTA` additively
//! accumulates local lights. Unity's ShadowCaster/Deferred/Meta passes are not declared here because
//! this render path has one forward color target, not shadow-map, G-buffer, or lightmapping targets.
//!
//! Froox variant bits populate `_RenderideVariantBits`; this shader decodes PBSMetallic's
//! shader-specific keyword bits locally.


//#texture_default _MainTex white
//#texture_default _MetallicGlossMap white
//#texture_default _BumpMap bump
//#texture_default _ParallaxMap black
//#texture_default _OcclusionMap white
//#texture_default _EmissionMap white
//#texture_default _DetailMask white
//#texture_default _DetailAlbedoMap gray
//#texture_default _DetailNormalMap bump
//#mat_default _GlossMapScale float 1.0
//#mat_default _SmoothnessTextureChannel float 0.0
//#mat_default _UVSec float 0.0
//#mat_default _OcclusionStrength float 1.0
//#mat_default _BumpScale float 1.0
//#mat_default _Parallax float 0.02
//#mat_default _Color vec4 1.0 1.0 1.0 1.0
//#mat_default _DetailNormalMapScale float 1.0
//#mat_default _EmissionColor vec4 0.0 0.0 0.0 1.0
//#mat_default _Cutoff float 0.5
//#mat_default _Glossiness float 0.5

#import renderide::material::variant_bits as vb
#import renderide::mesh::vertex as mv
#import renderide::pbs::detail as pdet
#import renderide::pbs::lighting as plight
#import renderide::pbs::surface as psurf
#import renderide::pbs::standard as pstd
#import renderide::core::uv as uvu
#import renderide::core::texture_sampling as ts

struct PbsMetallicMaterial {
    _Color: vec4<f32>,
    _EmissionColor: vec4<f32>,
    _MainTex_ST: vec4<f32>,
    _DetailAlbedoMap_ST: vec4<f32>,
    _Cutoff: f32,
    _Glossiness: f32,
    _GlossMapScale: f32,
    _SmoothnessTextureChannel: f32,
    _UVSec: f32,
    _Metallic: f32,
    _BumpScale: f32,
    _Parallax: f32,
    _OcclusionStrength: f32,
    _DetailNormalMapScale: f32,
    _RenderideVariantBits: u32,
    _MainTex_LodBias: f32,
    _MetallicGlossMap_LodBias: f32,
    _BumpMap_LodBias: f32,
    _ParallaxMap_LodBias: f32,
    _OcclusionMap_LodBias: f32,
    _EmissionMap_LodBias: f32,
    _DetailMask_LodBias: f32,
    _DetailAlbedoMap_LodBias: f32,
    _DetailNormalMap_LodBias: f32,
}

const PBSMETALLIC_KW_ALPHABLEND_ON: u32 = 1u << 0u;
const PBSMETALLIC_KW_ALPHAPREMULTIPLY_ON: u32 = 1u << 1u;
const PBSMETALLIC_KW_ALPHATEST_ON: u32 = 1u << 2u;
const PBSMETALLIC_KW_DETAIL_MULX2: u32 = 1u << 3u;
const PBSMETALLIC_KW_EMISSION: u32 = 1u << 4u;
const PBSMETALLIC_KW_GLOSSYREFLECTIONS_OFF: u32 = 1u << 5u;
const PBSMETALLIC_KW_METALLICGLOSSMAP: u32 = 1u << 6u;
const PBSMETALLIC_KW_NORMALMAP: u32 = 1u << 7u;
const PBSMETALLIC_KW_PARALLAXMAP: u32 = 1u << 8u;
const PBSMETALLIC_KW_SMOOTHNESS_TEXTURE_ALBEDO_CHANNEL_A: u32 = 1u << 9u;
const PBSMETALLIC_KW_SPECULARHIGHLIGHTS_OFF: u32 = 1u << 10u;

@group(1) @binding(0)  var<uniform> mat: PbsMetallicMaterial;
@group(1) @binding(1)  var _MainTex: texture_2d<f32>;
@group(1) @binding(2)  var _MainTex_sampler: sampler;
@group(1) @binding(3)  var _MetallicGlossMap: texture_2d<f32>;
@group(1) @binding(4)  var _MetallicGlossMap_sampler: sampler;
@group(1) @binding(5)  var _BumpMap: texture_2d<f32>;
@group(1) @binding(6)  var _BumpMap_sampler: sampler;
@group(1) @binding(7)  var _ParallaxMap: texture_2d<f32>;
@group(1) @binding(8)  var _ParallaxMap_sampler: sampler;
@group(1) @binding(9)  var _OcclusionMap: texture_2d<f32>;
@group(1) @binding(10) var _OcclusionMap_sampler: sampler;
@group(1) @binding(11) var _EmissionMap: texture_2d<f32>;
@group(1) @binding(12) var _EmissionMap_sampler: sampler;
@group(1) @binding(13) var _DetailMask: texture_2d<f32>;
@group(1) @binding(14) var _DetailMask_sampler: sampler;
@group(1) @binding(15) var _DetailAlbedoMap: texture_2d<f32>;
@group(1) @binding(16) var _DetailAlbedoMap_sampler: sampler;
@group(1) @binding(17) var _DetailNormalMap: texture_2d<f32>;
@group(1) @binding(18) var _DetailNormalMap_sampler: sampler;

struct SurfaceData {
    base_color: vec3<f32>,
    alpha: f32,
    metallic: f32,
    roughness: f32,
    occlusion: f32,
    normal: vec3<f32>,
    emission: vec3<f32>,
}

fn pbs_kw(mask: u32) -> bool {
    return vb::enabled(mat._RenderideVariantBits, mask);
}

fn alpha_test_enabled() -> bool {
    return pbs_kw(PBSMETALLIC_KW_ALPHATEST_ON);
}

fn alpha_premultiply_enabled() -> bool {
    return pbs_kw(PBSMETALLIC_KW_ALPHAPREMULTIPLY_ON);
}

fn specular_highlights_enabled() -> bool {
    return !pbs_kw(PBSMETALLIC_KW_SPECULARHIGHLIGHTS_OFF);
}

fn glossy_reflections_enabled() -> bool {
    return !pbs_kw(PBSMETALLIC_KW_GLOSSYREFLECTIONS_OFF);
}

fn metallic_gloss_map_enabled() -> bool {
    return pbs_kw(PBSMETALLIC_KW_METALLICGLOSSMAP);
}

fn smoothness_from_albedo_alpha() -> bool {
    return pbs_kw(PBSMETALLIC_KW_SMOOTHNESS_TEXTURE_ALBEDO_CHANNEL_A);
}

fn sample_surface(_uv0: vec2<f32>, _uv1: vec2<f32>, _world_pos: vec3<f32>, world_n: vec3<f32>, _world_t: vec4<f32>, _view_layer: u32) -> SurfaceData {
    // #216 bisect toward warp_216_term_debug. That composed path never samples a
    // material texture and never discards. These lit cases use variant bits 0 and
    // the white texture defaults, so albedo (1,1,1,1), occlusion 1, a geometric
    // normal, and emission 0 match the sampled surface. The texture globals stay
    // declared so the reflected bind group still lines up with //#texture_default.
    let albedo = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    let base_alpha = pstd::standard_alpha(mat._Color.a, albedo.a, false);
    let base_color = mat._Color.rgb * albedo.rgb;
    let metallic = clamp(mat._Metallic, 0.0, 1.0);
    let roughness = pstd::roughness_from_smoothness(mat._Glossiness);
    let occlusion = pstd::occlusion_from_sample(1.0, mat._OcclusionStrength);
    let n = normalize(world_n);
    let emission = vec3<f32>(0.0);
    return SurfaceData(base_color, base_alpha, metallic, roughness, occlusion, n, emission);
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
    @location(4) t: vec4<f32>,
    @location(5) uv1: vec2<f32>,
) -> mv::WorldUv2VertexOutput {
#ifdef MULTIVIEW
    return mv::world_uv2_vertex_main(instance_index, view_idx, pos, n, t, uv0, uv1);
#else
    return mv::world_uv2_vertex_main(instance_index, 0u, pos, n, t, uv0, uv1);
#endif
}

//#pass type=forward a2c=cutout
@fragment
fn fs_forward_base(
    @builtin(position) frag_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) world_t: vec4<f32>,
    @location(3) uv0: vec2<f32>,
    @location(4) uv1: vec2<f32>,
    @location(5) @interpolate(flat) view_layer: u32,
) -> @location(0) vec4<f32> {
    let s = sample_surface(uv0, uv1, world_pos, world_n, world_t, view_layer);
    let surface = psurf::metallic_with_geometric_normal(
        s.base_color,
        s.alpha,
        s.metallic,
        s.roughness,
        s.occlusion,
        s.normal,
        world_n,
        s.emission,
    );
    let options = plight::ClusterLightingOptions(
        true,
        true,
        specular_highlights_enabled(),
        glossy_reflections_enabled(),
    );
    if (alpha_premultiply_enabled()) {
        return plight::shade_metallic_transparent_clustered(
            frag_pos.xy,
            world_pos,
            view_layer,
            surface,
            options,
        );
    }
    let color = plight::shade_metallic_clustered(
        frag_pos.xy,
        world_pos,
        view_layer,
        surface,
        options,
    );
    return vec4<f32>(color, s.alpha);
}
