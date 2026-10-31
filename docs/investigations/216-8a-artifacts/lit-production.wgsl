struct GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    position: vec3<f32>,
    align_pad_vec3_pos: f32,
    direction: vec3<f32>,
    align_pad_vec3_dir: f32,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    spot_cos_half_angle: f32,
    light_type: u32,
    spot_angle_scale: f32,
    shadow_strength: f32,
    shadow_near_plane: f32,
    shadow_bias: f32,
    shadow_normal_bias: f32,
    shadow_type: u32,
    shadow_view_start: u32,
    shadow_view_count: u32,
    shadow_flags: u32,
    shadow_map_resolution: u32,
    cookie_kind: u32,
    cookie_layer: u32,
    cookie_reserved: u32,
    cookie_right_tan_half_angle: vec4<f32>,
    cookie_up: vec4<f32>,
}

struct GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    world_to_shadow: mat4x4<f32>,
    atlas_rect: vec4<f32>,
    params: vec4<f32>,
    light_params: vec4<f32>,
}

struct GpuLightCookieRectX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    origin_scale: vec4<f32>,
}

struct GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    box_min: vec4<f32>,
    box_max: vec4<f32>,
    position: vec4<f32>,
    params: vec4<f32>,
    sh2_a: vec4<f32>,
    sh2_b: vec4<f32>,
    sh2_c: vec4<f32>,
    sh2_d: vec4<f32>,
    sh2_e: vec4<f32>,
    sh2_f: vec4<f32>,
    sh2_g: vec4<f32>,
    sh2_h: vec4<f32>,
    sh2_i: vec4<f32>,
}

struct FrameGlobalsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    camera_world_pos: vec4<f32>,
    camera_world_pos_right: vec4<f32>,
    view_space_z_coeffs: vec4<f32>,
    view_space_z_coeffs_right: vec4<f32>,
    view_to_world_y_coeffs: vec4<f32>,
    view_to_world_y_coeffs_right: vec4<f32>,
    cluster_count_x: u32,
    cluster_count_y: u32,
    cluster_count_z: u32,
    near_clip: f32,
    far_clip: f32,
    light_count: u32,
    viewport_width: u32,
    viewport_height: u32,
    proj_params_left: vec4<f32>,
    proj_params_right: vec4<f32>,
    proj_left: mat4x4<f32>,
    proj_right: mat4x4<f32>,
    frame_tail: vec4<u32>,
    skybox_specular: vec4<f32>,
    frame_time: vec4<f32>,
    ambient_sh_a: vec4<f32>,
    ambient_sh_b: vec4<f32>,
    ambient_sh_c: vec4<f32>,
    ambient_sh_d: vec4<f32>,
    ambient_sh_e: vec4<f32>,
    ambient_sh_f: vec4<f32>,
    ambient_sh_g: vec4<f32>,
    ambient_sh_h: vec4<f32>,
    ambient_sh_i: vec4<f32>,
    fog_color_mode: vec4<f32>,
    fog_params: vec4<f32>,
    view_space_x_coeffs: vec4<f32>,
    view_space_x_coeffs_right: vec4<f32>,
    view_space_y_coeffs: vec4<f32>,
    view_space_y_coeffs_right: vec4<f32>,
}

struct PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX {
    view_proj_left: mat4x4<f32>,
    view_proj_right: mat4x4<f32>,
    model: mat4x4<f32>,
    normal_matrix: mat3x3<f32>,
    _pad: vec4<f32>,
    particle_header: vec4<f32>,
    particle_tint: vec4<f32>,
    particle_extra: vec4<f32>,
    particle_frame_grid: vec4<f32>,
    particle_padding: array<vec4<f32>, 12>,
}

struct BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    right: vec3<f32>,
    up: vec3<f32>,
}

struct MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX {
    right: vec3<f32>,
    up: vec3<f32>,
    forward: vec3<f32>,
}

struct WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) world_t: vec4<f32>,
    @location(3) primary_uv: vec2<f32>,
    @location(4) secondary_uv: vec2<f32>,
    @location(5) @interpolate(flat) view_layer: u32,
}

struct MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX {
    base_color: vec3<f32>,
    alpha: f32,
    metallic: f32,
    roughness: f32,
    occlusion: f32,
    normal: vec3<f32>,
    geometric_normal: vec3<f32>,
    emission: vec3<f32>,
}

struct ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX {
    near_clip: f32,
    far_clip: f32,
}

struct SpecularSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX {
    base_color: vec3<f32>,
    alpha: f32,
    specular_color: vec3<f32>,
    roughness: f32,
    one_minus_reflectivity: f32,
    occlusion: f32,
    normal: vec3<f32>,
    geometric_normal: vec3<f32>,
    emission: vec3<f32>,
}

struct PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX {
    face: u32,
    local_uv: vec2<f32>,
}

struct PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX {
    direction: vec3<f32>,
    up: vec3<f32>,
    right: vec3<f32>,
}

struct LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX {
    l: vec3<f32>,
    attenuation: f32,
}

struct DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX {
    view: vec3<f32>,
    n: vec3<f32>,
}

struct DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX {
    f: vec3<f32>,
    specular_brdf: vec3<f32>,
    n_dot_l: f32,
    n_dot_v: f32,
    l_dot_h: f32,
}

struct CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX {
    face: u32,
    uv: vec2<f32>,
}

struct ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX {
    weighted_result: vec3<f32>,
    weight_sum: f32,
    coverage: f32,
}

struct ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX {
    include_directional: bool,
    include_local: bool,
    specular_highlights_enabled: bool,
    glossy_reflections_enabled: bool,
}

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
    _BumpMap_WrapModeBits: u32,
    _DetailAlbedoMap_WrapModeBits: u32,
    _DetailMask_WrapModeBits: u32,
    _DetailNormalMap_WrapModeBits: u32,
    _EmissionMap_WrapModeBits: u32,
    _MainTex_WrapModeBits: u32,
    _MetallicGlossMap_WrapModeBits: u32,
    _OcclusionMap_WrapModeBits: u32,
    _ParallaxMap_WrapModeBits: u32,
}

struct SurfaceData {
    base_color: vec3<f32>,
    alpha: f32,
    metallic: f32,
    roughness: f32,
    occlusion: f32,
    normal: vec3<f32>,
    emission: vec3<f32>,
}

const FRAME_PROJECTION_FLAG_ORTHOGRAPHICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 1u;
const FRAME_TAIL_SAMPLE_COUNT_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 1u;
const FRAME_TAIL_SAMPLE_COUNT_MASKX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 30u;
const UNITY_PARALLAX_VIEW_Z_BIASX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX: f32 = 0.42f;
const COLOR_SPACE_DOUBLEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX: f32 = 4.594794f;
const PARTICLE_KIND_BILLBOARDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 1u;
const PARTICLE_KIND_MESHX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 2u;
const BILLBOARD_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 1u;
const BILLBOARD_ALIGNMENT_LOCALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 2u;
const BILLBOARD_ALIGNMENT_GLOBALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 3u;
const BILLBOARD_ALIGNMENT_DIRECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 4u;
const MESH_ALIGNMENT_VIEWX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 0u;
const MESH_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX: u32 = 1u;
const TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX: u32 = 32u;
const CLUSTER_NEAR_CLIP_MINX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX: f32 = 0.0001f;
const CLUSTER_FAR_CLIP_MIN_SPANX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX: f32 = 0.0001f;
const TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX: u32 = 32u;
const SRGB_LINEAR_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 0.04045f;
const SRGB_LINEAR_INV_SLOPEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 0.07739938f;
const SRGB_NONLINEAR_OFFSETX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 0.055f;
const SRGB_NONLINEAR_INV_SCALEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 0.9478673f;
const SRGB_NONLINEAR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 2.4f;
const SRGB_HDR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 2.2f;
const INTENSITY_BOOSTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 3.1415927f;
const BIRP_ATTENUATION_QUADRATICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX: f32 = 25f;
const LIGHT_COOKIE_KIND_SPOT_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 1u;
const LIGHT_COOKIE_KIND_POINT_CUBEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 2u;
const LIGHT_COOKIE_KIND_DIRECTIONAL_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 3u;
const LIGHT_COOKIE_WRAP_U_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 0u;
const LIGHT_COOKIE_WRAP_V_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 2u;
const LIGHT_COOKIE_WRAP_MODE_MASKX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 3u;
const LIGHT_COOKIE_WRAP_MODE_CLAMPX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 1u;
const LIGHT_COOKIE_WRAP_MODE_MIRRORX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 2u;
const LIGHT_COOKIE_WRAP_MODE_MIRROR_ONCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 3u;
const COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX: f32 = 0.00001f;
const SHADOW_VIEW_KIND_SPOTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 2u;
const SHADOW_VIEW_KIND_POINTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX: u32 = 3u;
const SHADOW_TYPE_SOFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX: u32 = 2u;
const SHADOW_UV_BORDERX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX: f32 = 0.0005f;
const SPECULAR_AA_VARIANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX: f32 = 0.25f;
const GEOMETRIC_SPECULAR_AA_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX: f32 = 0.18f;
const MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX: f32 = 0.0001f;
const MIN_ALPHAX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX: f32 = 0.002f;
const MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX: f32 = 0.00001f;
const PROBE_SH2_SOURCE_NONEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX: f32 = 0f;
const MIN_PROBE_BLEND_DISTANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX: f32 = 0.000001f;
const PROBE_FLAG_BOX_PROJECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX: f32 = 1f;
const BOX_PROJECTION_FULL_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX: f32 = 0.55f;
const BOX_PROJECTION_OFF_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX: f32 = 0.9f;
const PBSMETALLIC_KW_ALPHABLEND_ON: u32 = 1u;
const PBSMETALLIC_KW_ALPHAPREMULTIPLY_ON: u32 = 2u;
const PBSMETALLIC_KW_ALPHATEST_ON: u32 = 4u;
const PBSMETALLIC_KW_DETAIL_MULX2_: u32 = 8u;
const PBSMETALLIC_KW_EMISSION: u32 = 16u;
const PBSMETALLIC_KW_GLOSSYREFLECTIONS_OFF: u32 = 32u;
const PBSMETALLIC_KW_METALLICGLOSSMAP: u32 = 64u;
const PBSMETALLIC_KW_NORMALMAP: u32 = 128u;
const PBSMETALLIC_KW_PARALLAXMAP: u32 = 256u;
const PBSMETALLIC_KW_SMOOTHNESS_TEXTURE_ALBEDO_CHANNEL_A: u32 = 512u;
const PBSMETALLIC_KW_SPECULARHIGHLIGHTS_OFF: u32 = 1024u;

override renderide_static_variant_bits_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX: u32 = 0u;
override renderide_static_variant_bitsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX: u32 = 0u;

@group(0) @binding(0) 
var<uniform> frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: FrameGlobalsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX;
@group(2) @binding(0) 
var<storage> instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX: array<PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX>;
@group(0) @binding(2) 
var<storage> cluster_light_rangesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: array<vec2<u32>>;
@group(0) @binding(3) 
var<storage> cluster_light_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: array<u32>;
@group(0) @binding(1) 
var<storage> lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: array<GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX>;
@group(0) @binding(11) 
var ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: texture_2d<f32>;
@group(0) @binding(13) 
var light_cookie_2d_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: texture_2d<f32>;
@group(0) @binding(14) 
var light_cookie_point_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: texture_2d<f32>;
@group(0) @binding(15) 
var light_cookie_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: sampler;
@group(0) @binding(19) 
var<storage> light_cookie_rectsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: array<GpuLightCookieRectX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX>;
@group(0) @binding(16) 
var<storage> shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: array<GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX>;
@group(0) @binding(17) 
var shadow_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: texture_depth_2d_array;
@group(0) @binding(18) 
var shadow_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: sampler_comparison;
@group(0) @binding(9) 
var reflection_probe_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: texture_2d_array<f32>;
@group(0) @binding(12) 
var<storage> reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX: array<GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX>;
@group(1) @binding(0) 
var<uniform> mat: PbsMetallicMaterial;
@group(1) @binding(1) 
var _MainTex: texture_2d<f32>;
@group(1) @binding(2) 
var _MainTex_sampler: sampler;
@group(1) @binding(3) 
var _MetallicGlossMap: texture_2d<f32>;
@group(1) @binding(4) 
var _MetallicGlossMap_sampler: sampler;
@group(1) @binding(5) 
var _BumpMap: texture_2d<f32>;
@group(1) @binding(6) 
var _BumpMap_sampler: sampler;
@group(1) @binding(7) 
var _ParallaxMap: texture_2d<f32>;
@group(1) @binding(8) 
var _ParallaxMap_sampler: sampler;
@group(1) @binding(9) 
var _OcclusionMap: texture_2d<f32>;
@group(1) @binding(10) 
var _OcclusionMap_sampler: sampler;
@group(1) @binding(11) 
var _EmissionMap: texture_2d<f32>;
@group(1) @binding(12) 
var _EmissionMap_sampler: sampler;
@group(1) @binding(13) 
var _DetailMask: texture_2d<f32>;
@group(1) @binding(14) 
var _DetailMask_sampler: sampler;
@group(1) @binding(15) 
var _DetailAlbedoMap: texture_2d<f32>;
@group(1) @binding(16) 
var _DetailAlbedoMap_sampler: sampler;
@group(1) @binding(17) 
var _DetailNormalMap: texture_2d<f32>;
@group(1) @binding(18) 
var _DetailNormalMap_sampler: sampler;

fn effectiveX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(bits: u32) -> u32 {
    if (renderide_static_variant_bits_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX != 0u) {
        return renderide_static_variant_bitsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX;
    }
    return bits;
}

fn enabledX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(bits_1: u32, mask: u32) -> bool {
    let _e1: u32 = effectiveX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(bits_1);
    return ((_e1 & mask) != 0u);
}

fn apply_stX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HK5QX(uv_in: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return ((uv_in * st.xy) + st.zw);
}

fn sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(tex: texture_2d<f32>, tex_wrap_mode_bits: u32, samp: sampler, uv: vec2<f32>, lod_bias: f32) -> vec4<f32> {
    let _e4: vec4<f32> = renderide_mirroronce_sample_bias_2d(tex, samp, uv, lod_bias, tex_wrap_mode_bits);
    return _e4;
}

fn view_space_z_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_1: u32) -> vec4<f32> {
    let _e2: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_space_z_coeffs;
    return _e2;
}

fn safe_normalize_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len: f32 = length(v);
    if (len <= 0.000001f) {
        return fallback;
    }
    return (v / vec3(len));
}

fn orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_2: u32) -> vec3<f32> {
    let _e1: vec4<f32> = view_space_z_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_2);
    let _e7: vec3<f32> = safe_normalize_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(_e1.xyz, vec3<f32>(0f, 0f, -1f));
    return _e7;
}

fn projection_flags_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_3: u32) -> u32 {
    let _e3: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.frame_tail.y;
    return _e3;
}

fn view_is_orthographicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_4: u32) -> bool {
    let _e1: u32 = projection_flags_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_4);
    return ((_e1 & FRAME_PROJECTION_FLAG_ORTHOGRAPHICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) != 0u);
}

fn camera_world_pos_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_5: u32) -> vec3<f32> {
    let _e2: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.camera_world_pos;
    return _e2.xyz;
}

fn view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_1: vec3<f32>, view_layer_6: u32) -> vec3<f32> {
    let _e1: vec3<f32> = orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_6);
    let _e2: bool = view_is_orthographicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_6);
    if _e2 {
        return _e1;
    }
    let _e3: vec3<f32> = camera_world_pos_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_6);
    let _e6: vec3<f32> = safe_normalize_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX((_e3 - world_pos_1), _e1);
    return _e6;
}

fn safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(v_1: vec3<f32>, fallback_1: vec3<f32>) -> vec3<f32> {
    let len_sq: f32 = dot(v_1, v_1);
    if (len_sq <= 0.000000000001f) {
        return fallback_1;
    }
    return (v_1 * inverseSqrt(len_sq));
}

fn orthonormal_tbn_fallbackX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(n_1: vec3<f32>) -> mat3x3<f32> {
    let sign_: f32 = select(-1f, 1f, (n_1.z >= 0f));
    let a: f32 = (-1f / (sign_ + n_1.z));
    let b: f32 = ((n_1.x * n_1.y) * a);
    let t_11: vec3<f32> = vec3<f32>((1f + (((sign_ * n_1.x) * n_1.x) * a)), (sign_ * b), (-(sign_) * n_1.x));
    let bitan: vec3<f32> = vec3<f32>(b, (sign_ + ((n_1.y * n_1.y) * a)), -(n_1.y));
    return mat3x3<f32>(normalize(t_11), normalize(bitan), n_1);
}

fn orthonormal_tbnX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(world_n_1: vec3<f32>, world_t_1: vec4<f32>) -> mat3x3<f32> {
    var local: bool;
    var local_1: bool;

    let n_len_sq: f32 = dot(world_n_1, world_n_1);
    if !(!((n_len_sq > 0.0000000001f))) {
        local = (n_len_sq > 340282300000000000000000000000000000000f);
    } else {
        local = true;
    }
    let _e11: bool = local;
    if _e11 {
        let _e16: mat3x3<f32> = orthonormal_tbn_fallbackX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(vec3<f32>(0f, 0f, 1f));
        return _e16;
    }
    let n_21: vec3<f32> = (world_n_1 * inverseSqrt(n_len_sq));
    let t_ortho: vec3<f32> = (world_t_1.xyz - (n_21 * dot(world_t_1.xyz, n_21)));
    let t_len_sq: f32 = dot(t_ortho, t_ortho);
    if !(!((t_len_sq > 0.0000000001f))) {
        local_1 = (t_len_sq > 340282300000000000000000000000000000000f);
    } else {
        local_1 = true;
    }
    let _e35: bool = local_1;
    if _e35 {
        let _e36: mat3x3<f32> = orthonormal_tbn_fallbackX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(n_21);
        return _e36;
    }
    let t_12: vec3<f32> = (t_ortho * inverseSqrt(t_len_sq));
    let sign_1: f32 = select(1f, -1f, (world_t_1.w < 0f));
    let _e50: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(n_21, t_12), vec3<f32>(0f, 1f, 0f));
    let b_1: vec3<f32> = (_e50 * sign_1);
    return mat3x3<f32>(t_12, b_1, n_21);
}

fn tangent_space_view_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(world_pos_2: vec3<f32>, world_n_2: vec3<f32>, world_t_2: vec4<f32>, view_layer_7: u32) -> vec3<f32> {
    let _e2: vec3<f32> = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_2, view_layer_7);
    let _e5: mat3x3<f32> = orthonormal_tbnX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(world_n_2, world_t_2);
    return normalize(vec3<f32>(dot(_e2, _e5[0]), dot(_e2, _e5[1]), dot(_e2, _e5[2])));
}

fn unity_parallax_offsetX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(height_sample: f32, height_scale: f32, world_pos_3: vec3<f32>, world_n_3: vec3<f32>, world_t_3: vec4<f32>, view_layer_8: u32) -> vec2<f32> {
    let _e4: vec3<f32> = tangent_space_view_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(world_pos_3, world_n_3, world_t_3, view_layer_8);
    let centered_height: f32 = ((height_sample * height_scale) - (height_scale * 0.5f));
    return (centered_height * (_e4.xy / vec2((_e4.z + UNITY_PARALLAX_VIEW_Z_BIASX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX))));
}

fn decode_ts_normal_sample_rawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(s: vec4<f32>) -> vec3<f32> {
    var local_2: bool;

    let uniform_white_rgb: bool = all((s.xyz > vec3<f32>(0.99f, 0.99f, 0.99f)));
    if uniform_white_rgb {
        return s.xyz;
    }
    let all_r_high: bool = (s.x >= 0.98039216f);
    let gb_close: bool = (abs((s.y - s.z)) <= 0.03137255f);
    if all_r_high {
        local_2 = gb_close;
    } else {
        local_2 = false;
    }
    let _e21: bool = local_2;
    if _e21 {
        return vec3<f32>(s.w, s.y, s.z);
    }
    return s.xyz;
}

fn reconstruct_ts_normal_from_scaled_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(xy: vec2<f32>) -> vec3<f32> {
    let z: f32 = sqrt(max((1f - dot(xy, xy)), 0f));
    return vec3<f32>(xy, z);
}

fn decode_ts_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(raw: vec3<f32>, scale: f32) -> vec3<f32> {
    let xy_1: vec2<f32> = (((raw.xy * 2f) - vec2(1f)) * scale);
    let _e9: vec3<f32> = reconstruct_ts_normal_from_scaled_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(xy_1);
    return _e9;
}

fn decode_ts_normal_with_placeholderX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(raw_1: vec3<f32>, scale_1: f32) -> vec3<f32> {
    if all((raw_1 > vec3<f32>(0.99f, 0.99f, 0.99f))) {
        return vec3<f32>(0f, 0f, 1f);
    }
    let _e12: vec3<f32> = decode_ts_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(raw_1, scale_1);
    return _e12;
}

fn decode_ts_normal_with_placeholder_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(s_1: vec4<f32>, scale_2: f32) -> vec3<f32> {
    let _e1: vec3<f32> = decode_ts_normal_sample_rawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(s_1);
    let _e3: vec3<f32> = decode_ts_normal_with_placeholderX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(_e1, scale_2);
    return _e3;
}

fn roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZWC3LQNRUW4ZYX(smoothness: f32) -> f32 {
    return clamp((1f - smoothness), 0f, 1f);
}

fn standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(color_alpha: f32, texture_alpha: f32, smoothness_from_albedo_alpha: bool) -> f32 {
    if smoothness_from_albedo_alpha {
        return color_alpha;
    }
    return (color_alpha * texture_alpha);
}

fn clip_standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(alpha: f32, cutoff: f32, enabled: bool) {
    var local_3: bool;

    if enabled {
        local_3 = (alpha < cutoff);
    } else {
        local_3 = false;
    }
    let _e7: bool = local_3;
    if _e7 {
        discard;
    } else {
        return;
    }
}

fn apply_parallaxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(uv_1: vec2<f32>, enabled_1: bool, parallax: f32, world_pos_4: vec3<f32>, world_n_4: vec3<f32>, world_t_4: vec4<f32>, view_layer_9: u32, parallax_map: texture_2d<f32>, parallax_map_wrap_mode_bits: u32, parallax_sampler: sampler, parallax_lod_bias: f32) -> vec2<f32> {
    if !(enabled_1) {
        return uv_1;
    }
    let _e6: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(parallax_map, parallax_map_wrap_mode_bits, parallax_sampler, uv_1, parallax_lod_bias);
    let h_1: f32 = _e6.y;
    let _e13: vec2<f32> = unity_parallax_offsetX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(h_1, parallax, world_pos_4, world_n_4, world_t_4, view_layer_9);
    return (uv_1 + _e13);
}

fn sample_world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(normal_enabled: bool, detail_enabled: bool, detail_mask: f32, normal_tex: texture_2d<f32>, normal_tex_wrap_mode_bits: u32, normal_sampler: sampler, uv_main: vec2<f32>, normal_lod_bias: f32, normal_scale: f32, detail_normal_tex: texture_2d<f32>, detail_normal_tex_wrap_mode_bits: u32, detail_normal_sampler: sampler, uv_detail: vec2<f32>, detail_normal_lod_bias: f32, detail_normal_scale: f32, world_n_5: vec3<f32>, world_t_5: vec4<f32>, fallback_normal: vec3<f32>) -> vec3<f32> {
    var ts_n: vec3<f32>;
    var local_4: bool;

    if !(normal_enabled) {
        return fallback_normal;
    }
    let _e5: mat3x3<f32> = orthonormal_tbnX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(world_n_5, world_t_5);
    let _e10: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(normal_tex, normal_tex_wrap_mode_bits, normal_sampler, uv_main, normal_lod_bias);
    let _e12: vec3<f32> = decode_ts_normal_with_placeholder_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(_e10, normal_scale);
    ts_n = _e12;
    if detail_enabled {
        local_4 = (detail_mask > 0.001f);
    } else {
        local_4 = false;
    }
    let _e21: bool = local_4;
    if _e21 {
        let _e26: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(detail_normal_tex, detail_normal_tex_wrap_mode_bits, detail_normal_sampler, uv_detail, detail_normal_lod_bias);
        let _e28: vec3<f32> = decode_ts_normal_with_placeholder_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(_e26, detail_normal_scale);
        let _e29: vec3<f32> = ts_n;
        let _e34: f32 = ts_n.z;
        let detail_blend: vec3<f32> = normalize(vec3<f32>((_e29.xy + _e28.xy), (_e34 * _e28.z)));
        let _e39: vec3<f32> = ts_n;
        ts_n = mix(_e39, detail_blend, clamp(detail_mask, 0f, 1f));
    }
    let _e44: vec3<f32> = ts_n;
    return normalize((_e5 * _e44));
}

fn roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(smoothness_1: f32) -> f32 {
    let _e1: f32 = roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZWC3LQNRUW4ZYX(smoothness_1);
    return _e1;
}

fn occlusion_from_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(sample: f32, strength: f32) -> f32 {
    return mix(1f, sample, clamp(strength, 0f, 1f));
}

fn sample_emissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(enabled_2: bool, emission_color: vec3<f32>, emission_tex: texture_2d<f32>, emission_tex_wrap_mode_bits: u32, emission_sampler: sampler, uv_main_1: vec2<f32>, lod_bias_1: f32) -> vec3<f32> {
    if !(enabled_2) {
        return vec3(0f);
    }
    let _e8: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(emission_tex, emission_tex_wrap_mode_bits, emission_sampler, uv_main_1, lod_bias_1);
    return (_e8.xyz * emission_color);
}

fn detail_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(uv0_2: vec2<f32>, uv1_2: vec2<f32>, uv_sec: f32, detail_st: vec4<f32>) -> vec2<f32> {
    let _e7: vec2<f32> = apply_stX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HK5QX(select(uv0_2, uv1_2, (uv_sec >= 0.5f)), detail_st);
    return _e7;
}

fn sample_detail_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(enabled_3: bool, detail_mask_tex: texture_2d<f32>, detail_mask_tex_wrap_mode_bits: u32, detail_mask_sampler: sampler, uv_main_2: vec2<f32>, lod_bias_2: f32) -> f32 {
    if !(enabled_3) {
        return 0f;
    }
    let _e7: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(detail_mask_tex, detail_mask_tex_wrap_mode_bits, detail_mask_sampler, uv_main_2, lod_bias_2);
    return _e7.w;
}

fn apply_detail_albedoX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(base_color: vec3<f32>, enabled_4: bool, detail_mask_1: f32, detail_tex: texture_2d<f32>, detail_tex_wrap_mode_bits: u32, detail_sampler: sampler, uv_detail_1: vec2<f32>, lod_bias_3: f32) -> vec3<f32> {
    if !(enabled_4) {
        return base_color;
    }
    let _e7: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(detail_tex, detail_tex_wrap_mode_bits, detail_sampler, uv_detail_1, lod_bias_3);
    let detail: vec3<f32> = _e7.xyz;
    return (base_color * mix(vec3(1f), (detail * COLOR_SPACE_DOUBLEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX), detail_mask_1));
}

fn particle_colorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> vec4<f32> {
    return draw.particle_tint;
}

fn select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_1: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, view_idx: u32) -> mat4x4<f32> {
    if (view_idx == 0u) {
        return draw_1.view_proj_left;
    }
    return draw_1.view_proj_right;
}

fn position_stream_is_world_spaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_2: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> bool {
    return (draw_2._pad.x > 0.5f);
}

fn select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_3: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, view_idx_1: u32) -> mat4x4<f32> {
    let _e2: mat4x4<f32> = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_3, view_idx_1);
    return _e2;
}

fn world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_4: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_1: vec4<f32>) -> vec4<f32> {
    return (draw_4.model * vec4<f32>(pos_1.xyz, 1f));
}

fn world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_5: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, n_2: vec4<f32>) -> vec3<f32> {
    let _e9: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX((draw_5.normal_matrix * n_2.xyz), vec3<f32>(0f, 1f, 0f));
    return _e9;
}

fn model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_6: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, v_2: vec3<f32>) -> vec3<f32> {
    return (draw_6.model * vec4<f32>(v_2, 0f)).xyz;
}

fn model_handednessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_7: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> f32 {
    let _e1: bool = position_stream_is_world_spaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_7);
    if _e1 {
        return 1f;
    }
    let det: f32 = dot(draw_7.model[0].xyz, cross(draw_7.model[1].xyz, draw_7.model[2].xyz));
    return select(1f, -1f, (det < 0f));
}

fn tangent_w_signX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(tangent_w: f32) -> f32 {
    return select(1f, -1f, (tangent_w < 0f));
}

fn world_tangentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_8: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, t_1: vec4<f32>) -> vec4<f32> {
    let _e3: vec3<f32> = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_8, t_1.xyz);
    let _e6: f32 = tangent_w_signX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(t_1.w);
    let _e7: f32 = model_handednessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_8);
    return vec4<f32>(normalize(_e3), (_e6 * _e7));
}

fn packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(instance_index_1: u32, view_idx_2: u32) -> u32 {
    return ((instance_index_1 << 1u) | (view_idx_2 & 1u));
}

fn particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_9: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    return u32((max(draw_9.particle_header.x, 0f) + 0.5f));
}

fn particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_10: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    return u32((max(draw_10.particle_header.y, 0f) + 0.5f));
}

fn particle_min_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_11: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> f32 {
    return max(draw_11.particle_header.z, 0f);
}

fn particle_max_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_12: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> f32 {
    return max(draw_12.particle_header.w, 0f);
}

fn particle_frame_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_13: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    if (draw_13.particle_extra.y >= 4294967000f) {
        return 4294967295u;
    }
    return u32((max(draw_13.particle_extra.y, 0f) + 0.5f));
}

fn particle_frame_grid_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_14: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> vec2<u32> {
    return vec2<u32>(u32((max(draw_14.particle_frame_grid.x, 0f) + 0.5f)), u32((max(draw_14.particle_frame_grid.y, 0f) + 0.5f)));
}

fn view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_10: u32) -> vec4<f32> {
    let _e2: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_to_world_y_coeffs;
    return _e2;
}

fn viewport_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX() -> vec2<f32> {
    let _e2: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_width;
    let _e6: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_height;
    return vec2<f32>(f32(_e2), f32(_e6));
}

fn rotate_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(angle: f32, right: vec3<f32>, up: vec3<f32>) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    let c: f32 = cos(angle);
    let s_6: f32 = sin(angle);
    return BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(((right * c) - (up * s_6)), ((right * s_6) + (up * c)));
}

fn ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(clip: vec4<f32>) -> vec2<f32> {
    return (clip.xy / vec2(max(abs(clip.w), 0.000001f)));
}

fn particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_15: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, uv_2: vec2<f32>) -> vec2<f32> {
    var local_5: bool;

    let _e1: u32 = particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_15);
    if (_e1 != PARTICLE_KIND_MESHX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) {
        return uv_2;
    }
    let _e5: u32 = particle_frame_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_15);
    if (_e5 == 4294967295u) {
        return uv_2;
    }
    let _e8: vec2<u32> = particle_frame_grid_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_15);
    if !((_e8.x == 0u)) {
        local_5 = (_e8.y == 0u);
    } else {
        local_5 = true;
    }
    let _e19: bool = local_5;
    if _e19 {
        return uv_2;
    }
    let frame_count: u32 = max((_e8.x * _e8.y), 1u);
    let clamped_frame: u32 = min(_e5, (frame_count - 1u));
    let column: u32 = (clamped_frame % _e8.x);
    let row: u32 = ((_e8.y - 1u) - (clamped_frame / _e8.x));
    return vec2<f32>(((f32(column) + uv_2.x) / f32(_e8.x)), ((f32(row) + uv_2.y) / f32(_e8.y)));
}

fn mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_16: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> bool {
    var local_6: bool;
    var local_7: bool;

    let _e1: u32 = particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_16);
    let _e2: u32 = particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_16);
    if (_e2 == PARTICLE_KIND_MESHX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) {
        if !((_e1 == MESH_ALIGNMENT_VIEWX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
            local_7 = (_e1 == MESH_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX);
        } else {
            local_7 = true;
        }
        let _e13: bool = local_7;
        local_6 = _e13;
    } else {
        local_6 = false;
    }
    let _e17: bool = local_6;
    return _e17;
}

fn mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_17: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, view_idx_3: u32) -> MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX {
    var to_camera: vec3<f32>;

    let center_world_5: vec3<f32> = draw_17.model[3].xyz;
    let _e5: vec4<f32> = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_3);
    let _e11: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e5.xyz, vec3<f32>(0f, 1f, 0f));
    let _e12: vec3<f32> = orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_3);
    to_camera = _e12;
    let _e14: u32 = particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_17);
    if (_e14 == MESH_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) {
        let _e17: vec3<f32> = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(center_world_5, view_idx_3);
        to_camera = _e17;
    }
    let _e18: vec3<f32> = to_camera;
    let _e24: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e11, _e18), vec3<f32>(1f, 0f, 0f));
    let _e25: vec3<f32> = to_camera;
    let _e27: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e25, _e24), _e11);
    let _e28: vec3<f32> = to_camera;
    return MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(_e24, _e27, _e28);
}

fn mesh_particle_model_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_18: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> vec3<f32> {
    return vec3<f32>(max(length(draw_18.model[0].xyz), 0.000001f), max(length(draw_18.model[1].xyz), 0.000001f), max(length(draw_18.model[2].xyz), 0.000001f));
}

fn mesh_particle_world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_2: vec4<f32>, view_idx_4: u32) -> vec4<f32> {
    let _e1: bool = mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19);
    if !(_e1) {
        let _e4: vec4<f32> = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_19, pos_2);
        return _e4;
    }
    let _e6: MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX = mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19, view_idx_4);
    let _e8: vec3<f32> = mesh_particle_model_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19);
    let local_32: vec3<f32> = (pos_2.xyz * _e8);
    let center_world_6: vec3<f32> = draw_19.model[3].xyz;
    return vec4<f32>((((center_world_6 + (_e6.right * local_32.x)) + (_e6.up * local_32.y)) + (_e6.forward * local_32.z)), 1f);
}

fn render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_20: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> bool {
    let _e1: u32 = particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_20);
    return (_e1 == PARTICLE_KIND_BILLBOARDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX);
}

fn rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(angle_1: f32, right_1: vec3<f32>, up_1: vec3<f32>) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    let c_1: f32 = cos(angle_1);
    let s_7: f32 = sin(angle_1);
    let _e8: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(((right_1 * c_1) + (up_1 * s_7)), right_1);
    let _e13: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(((-(right_1) * s_7) + (up_1 * c_1)), up_1);
    return BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e8, _e13);
}

fn source_material_facing_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(center_world: vec3<f32>, view_idx_5: u32, roll: f32) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    let _e1: vec4<f32> = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_5);
    let _e7: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e1.xyz, vec3<f32>(0f, 1f, 0f));
    let _e9: vec3<f32> = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(center_world, view_idx_5);
    let _e15: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e7, _e9), vec3<f32>(1f, 0f, 0f));
    let _e17: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e9, _e15), _e7);
    if (abs(roll) > 0.0001f) {
        let _e22: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(roll, _e15, _e17);
        return _e22;
    }
    return BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e15, _e17);
}

fn source_material_local_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_21: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pointdata: vec3<f32>, tangent: vec4<f32>) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    var fallback_seed: vec3<f32> = vec3<f32>(0f, 1f, 0f);
    var right_2: vec3<f32>;
    var up_2: vec3<f32>;

    let _e9: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(tangent.xyz, vec3<f32>(0f, 0f, 1f));
    if (abs(_e9.y) > 0.95f) {
        fallback_seed = vec3<f32>(1f, 0f, 0f);
    }
    let _e19: vec3<f32> = fallback_seed;
    let _e25: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e9, _e19), vec3<f32>(1f, 0f, 0f));
    let _e27: vec3<f32> = fallback_seed;
    let _e28: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e25, _e9), _e27);
    let _e30: vec3<f32> = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_21, _e9);
    let _e35: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e30, vec3<f32>(0f, 0f, 1f));
    let _e36: vec3<f32> = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_21, _e28);
    let _e41: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e36, vec3<f32>(0f, 1f, 0f));
    let _e47: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e35, _e41), vec3<f32>(1f, 0f, 0f));
    right_2 = _e47;
    let _e49: vec3<f32> = right_2;
    let _e51: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e49, _e35), _e41);
    up_2 = _e51;
    if (abs(pointdata.z) > 0.0001f) {
        let _e59: vec3<f32> = right_2;
        let _e60: vec3<f32> = up_2;
        let _e61: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(pointdata.z, _e59, _e60);
        right_2 = _e61.right;
        up_2 = _e61.up;
    }
    let _e64: vec3<f32> = right_2;
    let _e65: vec3<f32> = up_2;
    return BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e64, _e65);
}

fn direction_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_22: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, center_world_1: vec3<f32>, direction: vec3<f32>, view_idx_6: u32) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    var up_3: vec3<f32>;

    let _e2: vec3<f32> = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(center_world_1, view_idx_6);
    let _e5: vec3<f32> = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_22, direction);
    let velocity_in_plane: vec3<f32> = (_e5 - (_e2 * dot(_e5, _e2)));
    let _e9: vec4<f32> = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_6);
    let view_up: vec3<f32> = _e9.xyz;
    let view_up_in_plane: vec3<f32> = (view_up - (_e2 * dot(view_up, _e2)));
    let _e18: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(view_up_in_plane, vec3<f32>(0f, 1f, 0f));
    let _e19: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(velocity_in_plane, _e18);
    up_3 = _e19;
    let _e21: vec3<f32> = up_3;
    let _e27: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e21, _e2), vec3<f32>(1f, 0f, 0f));
    let _e29: vec3<f32> = up_3;
    let _e30: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e2, _e27), _e29);
    up_3 = _e30;
    let _e31: vec3<f32> = up_3;
    return BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e27, _e31);
}

fn source_material_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(view_idx_7: u32, roll_1: f32) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    let _e1: vec4<f32> = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_7);
    let _e7: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e1.xyz, vec3<f32>(0f, 1f, 0f));
    let _e8: vec3<f32> = orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_7);
    let _e14: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e7, _e8), vec3<f32>(1f, 0f, 0f));
    let _e16: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e8, _e14), _e7);
    if (abs(roll_1) > 0.0001f) {
        let _e21: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(roll_1, _e14, _e16);
        return _e21;
    }
    return BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e14, _e16);
}

fn source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_23: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, center_world_2: vec3<f32>, pointdata_1: vec3<f32>, tangent_1: vec4<f32>, view_idx_8: u32) -> BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    var local_8: bool;

    let _e1: u32 = particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_23);
    if (_e1 == BILLBOARD_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) {
        let _e8: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = source_material_facing_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(center_world_2, view_idx_8, pointdata_1.z);
        return _e8;
    }
    if !((_e1 == BILLBOARD_ALIGNMENT_LOCALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
        local_8 = (_e1 == BILLBOARD_ALIGNMENT_GLOBALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX);
    } else {
        local_8 = true;
    }
    let _e17: bool = local_8;
    if _e17 {
        let _e19: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = source_material_local_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_23, pointdata_1, tangent_1);
        return _e19;
    }
    if (_e1 == BILLBOARD_ALIGNMENT_DIRECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) {
        let _e23: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = direction_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_23, center_world_2, tangent_1.xyz, view_idx_8);
        return _e23;
    }
    let _e25: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = source_material_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(view_idx_8, pointdata_1.z);
    return _e25;
}

fn signed_corner_from_pointdataX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(pointdata_2: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(select(-1f, 1f, (pointdata_2.x >= 0f)), select(-1f, 1f, (pointdata_2.y >= 0f)));
}

fn screen_clamped_billboard_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_24: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, center_world_3: vec3<f32>, axes: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX, size: vec2<f32>, vp: mat4x4<f32>) -> vec2<f32> {
    var local_9: bool;
    var scale_3: f32 = 1f;
    var local_10: bool;
    var local_11: bool;

    let _e2: f32 = particle_min_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_24);
    let _e3: f32 = particle_max_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_24);
    if (_e2 <= 0f) {
        local_9 = (_e3 <= 0f);
    } else {
        local_9 = false;
    }
    let _e11: bool = local_9;
    if _e11 {
        return size;
    }
    let _e13: vec2<f32> = viewport_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX();
    let viewport: vec2<f32> = max(_e13, vec2<f32>(1f, 1f));
    let _e23: vec2<f32> = ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX((vp * vec4<f32>(center_world_3, 1f)));
    let _e32: vec2<f32> = ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX((vp * vec4<f32>((center_world_3 + (axes.right * size.x)), 1f)));
    let _e40: vec2<f32> = ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX((vp * vec4<f32>((center_world_3 + (axes.up * size.y)), 1f)));
    let right_pixels: f32 = length((((_e32 - _e23) * viewport) * 0.5f));
    let up_pixels: f32 = length((((_e40 - _e23) * viewport) * 0.5f));
    let screen_fraction: f32 = (max(right_pixels, up_pixels) / max(min(viewport.x, viewport.y), 1f));
    if (screen_fraction <= 0.000001f) {
        return size;
    }
    if (_e2 > 0f) {
        local_10 = (screen_fraction < _e2);
    } else {
        local_10 = false;
    }
    let _e66: bool = local_10;
    if _e66 {
        let _e68: f32 = scale_3;
        scale_3 = max(_e68, (_e2 / screen_fraction));
    }
    if (_e3 > 0f) {
        let _e73: f32 = scale_3;
        local_11 = ((screen_fraction * _e73) > _e3);
    } else {
        local_11 = false;
    }
    let _e79: bool = local_11;
    if _e79 {
        scale_3 = (_e3 / screen_fraction);
    }
    let _e81: f32 = scale_3;
    return max((size * _e81), vec2<f32>(0.000001f, 0.000001f));
}

fn render_buffer_billboard_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(center_world_4: vec3<f32>, axes_1: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX, corner: vec2<f32>, size_1: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(((center_world_4 + ((axes_1.right * corner.x) * size_1.x)) + ((axes_1.up * corner.y) * size_1.y)), 1f);
}

fn source_material_render_buffer_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_3: vec4<f32>, pointdata_3: vec4<f32>, tangent_2: vec4<f32>, view_idx_9: u32) -> vec4<f32> {
    let _e2: vec4<f32> = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_25, pos_3);
    let center_world_7: vec3<f32> = _e2.xyz;
    let _e8: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25, center_world_7, pointdata_3.xyz, tangent_2, view_idx_9);
    let _e10: vec2<f32> = signed_corner_from_pointdataX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(pointdata_3.xyz);
    let _e11: vec3<f32> = mesh_particle_model_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25);
    let model_scale: vec2<f32> = _e11.xy;
    let unclamped_size: vec2<f32> = max((abs(pointdata_3.xy) * model_scale), vec2<f32>(0.000001f, 0.000001f));
    let _e20: mat4x4<f32> = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_25, view_idx_9);
    let _e21: vec2<f32> = screen_clamped_billboard_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25, center_world_7, _e8, unclamped_size, _e20);
    let _e22: vec4<f32> = render_buffer_billboard_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(center_world_7, _e8, _e10, _e21);
    return _e22;
}

fn world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_4: vec4<f32>, n_3: vec4<f32>, t_2: vec4<f32>, view_idx_10: u32) -> vec4<f32> {
    let _e1: bool = render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26);
    if _e1 {
        let _e6: vec4<f32> = source_material_render_buffer_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26, pos_4, n_3, t_2, view_idx_10);
        return _e6;
    }
    let _e7: vec4<f32> = mesh_particle_world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26, pos_4, view_idx_10);
    return _e7;
}

fn render_buffer_billboard_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(axes_2: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX) -> vec3<f32> {
    let _e8: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(axes_2.right, axes_2.up), vec3<f32>(0f, 0f, 1f));
    return _e8;
}

fn source_material_render_buffer_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_27: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_5: vec4<f32>, pointdata_4: vec4<f32>, tangent_3: vec4<f32>, view_idx_11: u32) -> vec3<f32> {
    let _e2: vec4<f32> = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_27, pos_5);
    let center_world_8: vec3<f32> = _e2.xyz;
    let _e8: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_27, center_world_8, pointdata_4.xyz, tangent_3, view_idx_11);
    let _e9: vec3<f32> = render_buffer_billboard_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(_e8);
    return _e9;
}

fn world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_6: vec4<f32>, n_4: vec4<f32>, t_3: vec4<f32>, view_idx_12: u32) -> vec3<f32> {
    let _e1: bool = render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28);
    if _e1 {
        let _e6: vec3<f32> = source_material_render_buffer_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28, pos_6, n_4, t_3, view_idx_12);
        return _e6;
    }
    let _e7: bool = mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28);
    if !(_e7) {
        let _e9: vec3<f32> = world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_28, n_4);
        return _e9;
    }
    let _e10: MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX = mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28, view_idx_12);
    let _e23: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX((((_e10.right * n_4.x) + (_e10.up * n_4.y)) + (_e10.forward * n_4.z)), _e10.forward);
    return _e23;
}

fn source_material_render_buffer_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_29: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_7: vec4<f32>, pointdata_5: vec4<f32>, tangent_4: vec4<f32>, view_idx_13: u32) -> vec4<f32> {
    let _e2: vec4<f32> = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_29, pos_7);
    let center_world_9: vec3<f32> = _e2.xyz;
    let _e8: BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX = source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_29, center_world_9, pointdata_5.xyz, tangent_4, view_idx_13);
    return vec4<f32>(_e8.right, 1f);
}

fn world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_8: vec4<f32>, n_5: vec4<f32>, t_4: vec4<f32>, view_idx_14: u32) -> vec4<f32> {
    let _e1: bool = render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30);
    if _e1 {
        let _e6: vec4<f32> = source_material_render_buffer_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30, pos_8, n_5, t_4, view_idx_14);
        return _e6;
    }
    let _e7: bool = mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30);
    if !(_e7) {
        let _e9: vec4<f32> = world_tangentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_30, t_4);
        return _e9;
    }
    let _e10: MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX = mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30, view_idx_14);
    let _e23: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX((((_e10.right * t_4.x) + (_e10.up * t_4.y)) + (_e10.forward * t_4.z)), _e10.right);
    let _e25: f32 = tangent_w_signX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(t_4.w);
    return vec4<f32>(_e23, _e25);
}

fn reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_31: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    let packed: u32 = bitcast<u32>(draw_31._pad.y);
    return (packed & 65535u);
}

fn fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_32: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    let packed_1: u32 = bitcast<u32>(draw_32._pad.y);
    return (packed_1 >> 16u);
}

fn local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_33: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> vec4<u32> {
    let packed_z: u32 = bitcast<u32>(draw_33._pad.z);
    let packed_w: u32 = bitcast<u32>(draw_33._pad.w);
    return vec4<u32>((packed_z & 65535u), (packed_z >> 16u), (packed_w & 65535u), (packed_w >> 16u));
}

fn has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_34: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> bool {
    var local_12: bool;

    let _e1: vec4<u32> = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_34);
    let _e2: u32 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_34);
    if !((_e2 != 0u)) {
        local_12 = any((_e1 != vec4(0u)));
    } else {
        local_12 = true;
    }
    let _e13: bool = local_12;
    return _e13;
}

fn get_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(instance_idx: u32) -> PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX {
    let _e3: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX[instance_idx];
    return _e3;
}

fn world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(draw_35: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_9: vec4<f32>, n_6: vec4<f32>, t_5: vec4<f32>, view_idx_15: u32) -> vec4<f32> {
    let _e5: vec4<f32> = world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_35, pos_9, n_6, t_5, view_idx_15);
    return _e5;
}

fn select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(draw_36: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, view_idx_16: u32) -> mat4x4<f32> {
    let _e2: mat4x4<f32> = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_36, view_idx_16);
    return _e2;
}

fn world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(draw_37: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_10: vec4<f32>, n_7: vec4<f32>, t_6: vec4<f32>, view_idx_17: u32) -> vec3<f32> {
    let _e5: vec3<f32> = world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_37, pos_10, n_7, t_6, view_idx_17);
    return _e5;
}

fn world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(draw_38: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, pos_11: vec4<f32>, n_8: vec4<f32>, t_7: vec4<f32>, view_idx_18: u32) -> vec4<f32> {
    let _e5: vec4<f32> = world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_38, pos_11, n_8, t_7, view_idx_18);
    return _e5;
}

fn particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(draw_39: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX, uv_3: vec2<f32>) -> vec2<f32> {
    let _e2: vec2<f32> = particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_39, uv_3);
    return _e2;
}

fn packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(instance_index_2: u32, view_idx_19: u32) -> u32 {
    let _e2: u32 = packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(instance_index_2, view_idx_19);
    return _e2;
}

fn world_uv2_vertex_mainX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(instance_index_3: u32, view_idx_20: u32, pos_12: vec4<f32>, n_9: vec4<f32>, t_8: vec4<f32>, primary_uv: vec2<f32>, secondary_uv: vec2<f32>) -> WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX {
    var out: WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX;

    let _e1: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = get_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(instance_index_3);
    let _e6: vec4<f32> = world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e1, pos_12, n_9, t_8, view_idx_20);
    let _e7: mat4x4<f32> = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e1, view_idx_20);
    out.clip_pos = (_e7 * _e6);
    out.world_pos = _e6.xyz;
    let _e14: vec3<f32> = world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e1, pos_12, n_9, t_8, view_idx_20);
    out.world_n = _e14;
    let _e16: vec4<f32> = world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e1, pos_12, n_9, t_8, view_idx_20);
    out.world_t = _e16;
    let _e19: vec2<f32> = particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e1, primary_uv);
    out.primary_uv = _e19;
    out.secondary_uv = secondary_uv;
    let _e23: u32 = packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(instance_index_3, view_idx_20);
    out.view_layer = _e23;
    let _e24: WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX = out;
    return _e24;
}

fn metallic_with_geometric_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(base_color_1: vec3<f32>, alpha_1: f32, metallic_value: f32, roughness: f32, occlusion: f32, normal: vec3<f32>, geometric_normal: vec3<f32>, emission: vec3<f32>) -> MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX {
    return MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(base_color_1, alpha_1, clamp(metallic_value, 0f, 1f), roughness, occlusion, normalize(normal), normalize(geometric_normal), emission);
}

fn finite_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(value: f32, fallback_2: f32) -> f32 {
    return select(fallback_2, value, (abs(value) <= 340282300000000000000000000000000000000f));
}

fn sanitize_cluster_clip_planesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_clip: f32, far_clip: f32) -> ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX {
    let _e2: f32 = finite_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_clip, CLUSTER_NEAR_CLIP_MINX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX);
    let near_safe: f32 = max(_e2, CLUSTER_NEAR_CLIP_MINX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX);
    let _e8: f32 = finite_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(far_clip, (near_safe + CLUSTER_FAR_CLIP_MIN_SPANX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX));
    let far_safe: f32 = max(_e8, (near_safe + CLUSTER_FAR_CLIP_MIN_SPANX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX));
    return ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_safe, far_safe);
}

fn cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(view_z: f32, near_clip_1: f32, far_clip_1: f32, cluster_count_z: u32) -> u32 {
    let z_count: u32 = max(cluster_count_z, 1u);
    let _e5: ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX = sanitize_cluster_clip_planesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_clip_1, far_clip_1);
    let d: f32 = clamp(-(view_z), _e5.near_clip, _e5.far_clip);
    let z_1: f32 = ((log((d / _e5.near_clip)) / log((_e5.far_clip / _e5.near_clip))) * f32(z_count));
    return u32(clamp(z_1, 0f, f32((z_count - 1u))));
}

fn view_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_11: u32) -> u32 {
    return (view_layer_11 & 1u);
}

fn cluster_light_range_atX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(cluster_id: u32) -> vec2<u32> {
    let _e3: vec2<u32> = cluster_light_rangesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[cluster_id];
    return _e3;
}

fn cluster_light_index_from_rangeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(range: vec2<u32>, slot: u32) -> u32 {
    let _e6: u32 = cluster_light_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[(range.x + slot)];
    return _e6;
}

fn cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(view_z_1: f32, near_clip_2: f32, far_clip_2: f32, cluster_count_z_1: u32) -> u32 {
    let _e4: u32 = cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(view_z_1, near_clip_2, far_clip_2, cluster_count_z_1);
    return _e4;
}

fn cluster_xy_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(frag_xy: vec2<f32>, viewport_w: u32, viewport_h: u32) -> vec2<u32> {
    let vw: u32 = max(viewport_w, 1u);
    let vh: u32 = max(viewport_h, 1u);
    let px: u32 = min(u32(max(frag_xy.x, 0f)), (vw - 1u));
    let py: u32 = min(u32(max(frag_xy.y, 0f)), (vh - 1u));
    return vec2<u32>((px / TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX), (py / TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX));
}

fn cluster_id_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(clip_xy: vec2<f32>, world_pos_5: vec3<f32>, view_space_z_coeffs: vec4<f32>, view_space_z_coeffs_right: vec4<f32>, view_index: u32, viewport_w_1: u32, viewport_h_1: u32, cluster_count_x: u32, cluster_count_y: u32, cluster_count_z_2: u32, near_clip_3: f32, far_clip_3: f32) -> u32 {
    let count_x: u32 = max(cluster_count_x, 1u);
    let count_y: u32 = max(cluster_count_y, 1u);
    let count_z: u32 = max(cluster_count_z_2, 1u);
    let _e10: u32 = view_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_index);
    let z_coeffs: vec4<f32> = select(view_space_z_coeffs, view_space_z_coeffs_right, (_e10 != 0u));
    let view_z_2: f32 = (dot(z_coeffs.xyz, world_pos_5) + z_coeffs.w);
    let _e23: u32 = cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(view_z_2, near_clip_3, far_clip_3, count_z);
    let _e27: vec2<u32> = cluster_xy_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(clip_xy, viewport_w_1, viewport_h_1);
    let cx: u32 = min(_e27.x, (count_x - 1u));
    let cy: u32 = min(_e27.y, (count_y - 1u));
    let local_id: u32 = (cx + (count_x * (cy + (count_y * _e23))));
    let cluster_offset: u32 = (((_e10 * count_x) * count_y) * count_z);
    return (cluster_offset + local_id);
}

fn srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(value_1: f32) -> f32 {
    if (value_1 <= SRGB_LINEAR_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX) {
        return (value_1 * SRGB_LINEAR_INV_SLOPEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
    }
    if (value_1 < 1f) {
        let normalized: f32 = ((value_1 + SRGB_NONLINEAR_OFFSETX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX) * SRGB_NONLINEAR_INV_SCALEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
        return pow(normalized, SRGB_NONLINEAR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
    }
    return pow(value_1, SRGB_HDR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
}

fn srgb_light_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color: vec3<f32>) -> vec3<f32> {
    let _e2: f32 = srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color.x);
    let _e4: f32 = srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color.y);
    let _e6: f32 = srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color.z);
    return vec3<f32>(_e2, _e4, _e6);
}

fn light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) -> vec3<f32> {
    let _e4: vec3<f32> = srgb_light_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX((light.color * light.intensity));
    return _e4;
}

fn direct_light_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX() -> f32 {
    return INTENSITY_BOOSTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX;
}

fn squared_edge_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(t_9: f32) -> f32 {
    let fade: f32 = clamp((1f - t_9), 0f, 1f);
    return (fade * fade);
}

fn range_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(t_10: f32) -> f32 {
    let t2_: f32 = (t_10 * t_10);
    let _e4: f32 = squared_edge_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(((t2_ * t2_) * t2_));
    return _e4;
}

fn distance_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(dist: f32, range_1: f32) -> f32 {
    if (range_1 <= 0f) {
        return 0f;
    }
    let t_13: f32 = (dist / range_1);
    let t2_1: f32 = (t_13 * t_13);
    let lut: f32 = (1f / (1f + (BIRP_ATTENUATION_QUADRATICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX * t2_1)));
    let _e13: f32 = range_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(t_13);
    return (lut * _e13);
}

fn distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(dist_1: f32, range_2: f32) -> f32 {
    let _e2: f32 = distance_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(dist_1, range_2);
    return (_e2 * INTENSITY_BOOSTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
}

fn normalized_spot_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(direction_1: vec3<f32>) -> vec3<f32> {
    let len_sq_1: f32 = dot(direction_1, direction_1);
    if (len_sq_1 <= 0.0000000000000001f) {
        return vec3<f32>(0f, 0f, 1f);
    }
    return (direction_1 * inverseSqrt(len_sq_1));
}

fn spot_angle_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_1: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, l: vec3<f32>) -> f32 {
    let _e4: vec3<f32> = normalized_spot_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_1.direction);
    let rho: f32 = dot(-(l), _e4);
    if (rho <= light_1.spot_cos_half_angle) {
        return 0f;
    }
    if (light_1.spot_angle_scale <= 0f) {
        return select(0f, 1f, (light_1.spot_cos_half_angle <= 0f));
    }
    let rho2_: f32 = max((rho * rho), 0.000001f);
    let tan2_theta: f32 = (max((1f - rho2_), 0f) / rho2_);
    let r2_: f32 = clamp((tan2_theta * light_1.spot_angle_scale), 0f, 1f);
    let _e33: f32 = squared_edge_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(((r2_ * r2_) * r2_));
    return _e33;
}

fn wrap_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(bits_2: u32, shift: u32) -> u32 {
    return ((bits_2 >> shift) & LIGHT_COOKIE_WRAP_MODE_MASKX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
}

fn mirror_repeat_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord: f32) -> f32 {
    return (1f - abs(((fract((coord * 0.5f)) * 2f) - 1f)));
}

fn mirror_once_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord_1: f32) -> f32 {
    return clamp(abs(coord_1), 0f, 1f);
}

fn wrap_cookie_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord_2: f32, mode: u32) -> f32 {
    if (mode == LIGHT_COOKIE_WRAP_MODE_CLAMPX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) {
        return clamp(coord_2, 0f, 1f);
    }
    if (mode == LIGHT_COOKIE_WRAP_MODE_MIRRORX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) {
        let _e9: f32 = mirror_repeat_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord_2);
        return _e9;
    }
    if (mode == LIGHT_COOKIE_WRAP_MODE_MIRROR_ONCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) {
        let _e12: f32 = mirror_once_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord_2);
        return _e12;
    }
    return fract(coord_2);
}

fn wrap_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_4: vec2<f32>, bits_3: u32) -> vec2<f32> {
    let _e4: u32 = wrap_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(bits_3, LIGHT_COOKIE_WRAP_U_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
    let _e5: f32 = wrap_cookie_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_4.x, _e4);
    let _e8: u32 = wrap_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(bits_3, LIGHT_COOKIE_WRAP_V_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
    let _e9: f32 = wrap_cookie_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_4.y, _e8);
    return vec2<f32>(_e5, _e9);
}

fn atlas_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(rect_index: u32, uv_5: vec2<f32>) -> vec2<f32> {
    let rect: vec4<f32> = light_cookie_rectsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[rect_index].origin_scale;
    return (rect.xy + (uv_5 * rect.zw));
}

fn sample_2d_cookieX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_2: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, uv_6: vec2<f32>) -> f32 {
    let _e4: vec2<f32> = wrap_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_6, light_2.cookie_reserved);
    let _e5: vec2<f32> = atlas_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_2.cookie_layer, _e4);
    let _e8: vec4<f32> = textureSample(light_cookie_2d_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, light_cookie_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e5);
    return _e8.x;
}

fn spot_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_3: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_6: vec3<f32>) -> f32 {
    var local_13: bool;

    let tan_half: f32 = light_3.cookie_right_tan_half_angle.w;
    if (tan_half <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX) {
        return 1f;
    }
    let from_light: vec3<f32> = (world_pos_6 - light_3.position);
    let local_33: vec3<f32> = vec3<f32>(dot(from_light, light_3.cookie_right_tan_half_angle.xyz), dot(from_light, light_3.cookie_up.xyz), dot(from_light, light_3.direction));
    if (local_33.z <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX) {
        return 0f;
    }
    let uv_9: vec2<f32> = (((local_33.xy / vec2((local_33.z * tan_half))) * 0.5f) + vec2(0.5f));
    if !(any((uv_9 < vec2(0f)))) {
        local_13 = any((uv_9 > vec2(1f)));
    } else {
        local_13 = true;
    }
    let _e44: bool = local_13;
    if _e44 {
        return 0f;
    }
    let _e46: f32 = sample_2d_cookieX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_3, uv_9);
    return _e46;
}

fn cube_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(dir: vec3<f32>) -> vec3<f32> {
    var local_14: bool;

    let ad: vec3<f32> = abs(dir);
    if (ad.x >= ad.y) {
        local_14 = (ad.x >= ad.z);
    } else {
        local_14 = false;
    }
    let _e11: bool = local_14;
    if _e11 {
        if (dir.x >= 0f) {
            return vec3<f32>((((vec2<f32>(-(dir.z), -(dir.y)) / vec2(ad.x)) * 0.5f) + vec2(0.5f)), 0f);
        }
        return vec3<f32>((((vec2<f32>(dir.z, -(dir.y)) / vec2(ad.x)) * 0.5f) + vec2(0.5f)), 1f);
    }
    if (ad.y >= ad.z) {
        if (dir.y >= 0f) {
            return vec3<f32>((((vec2<f32>(dir.x, dir.z) / vec2(ad.y)) * 0.5f) + vec2(0.5f)), 2f);
        }
        return vec3<f32>((((vec2<f32>(dir.x, -(dir.z)) / vec2(ad.y)) * 0.5f) + vec2(0.5f)), 3f);
    }
    if (dir.z >= 0f) {
        return vec3<f32>((((vec2<f32>(dir.x, -(dir.y)) / vec2(ad.z)) * 0.5f) + vec2(0.5f)), 4f);
    }
    return vec3<f32>((((vec2<f32>(-(dir.x), -(dir.y)) / vec2(ad.z)) * 0.5f) + vec2(0.5f)), 5f);
}

fn point_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_4: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_7: vec3<f32>) -> f32 {
    let from_light_1: vec3<f32> = (world_pos_7 - light_4.position);
    let len_sq_2: f32 = dot(from_light_1, from_light_1);
    if (len_sq_2 <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX) {
        return 1f;
    }
    let local_34: vec3<f32> = (vec3<f32>(dot(from_light_1, light_4.cookie_right_tan_half_angle.xyz), dot(from_light_1, light_4.cookie_up.xyz), dot(from_light_1, light_4.direction)) * inverseSqrt(len_sq_2));
    let _e19: vec3<f32> = cube_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(local_34);
    let rect_index_1: u32 = (light_4.cookie_layer + u32(_e19.z));
    let _e25: vec2<f32> = atlas_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(rect_index_1, _e19.xy);
    let _e28: vec4<f32> = textureSample(light_cookie_point_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, light_cookie_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e25);
    return _e28.x;
}

fn directional_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_5: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_8: vec3<f32>) -> f32 {
    let cookie_size: f32 = light_5.cookie_right_tan_half_angle.w;
    if (cookie_size <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX) {
        return 1f;
    }
    let from_light_2: vec3<f32> = (world_pos_8 - light_5.position);
    let uv_10: vec2<f32> = ((vec2<f32>(dot(from_light_2, light_5.cookie_right_tan_half_angle.xyz), dot(from_light_2, light_5.cookie_up.xyz)) / vec2(cookie_size)) + vec2(0.5f));
    let _e21: f32 = sample_2d_cookieX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_5, uv_10);
    return _e21;
}

fn multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_9: vec3<f32>) -> f32 {
    if (light_6.cookie_kind == LIGHT_COOKIE_KIND_SPOT_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) {
        let _e5: f32 = spot_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6, world_pos_9);
        return _e5;
    }
    if (light_6.cookie_kind == LIGHT_COOKIE_KIND_POINT_CUBEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) {
        let _e9: f32 = point_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6, world_pos_9);
        return _e9;
    }
    if (light_6.cookie_kind == LIGHT_COOKIE_KIND_DIRECTIONAL_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) {
        let _e13: f32 = directional_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6, world_pos_9);
        return _e13;
    }
    return 1f;
}

fn has_shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_7: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) -> bool {
    var local_15: bool;
    var local_16: bool;

    if (light_7.shadow_view_count > 0u) {
        local_15 = (light_7.shadow_strength > 0f);
    } else {
        local_15 = false;
    }
    let _e10: bool = local_15;
    if _e10 {
        local_16 = (light_7.shadow_type != 0u);
    } else {
        local_16 = false;
    }
    let _e17: bool = local_16;
    return _e17;
}

fn receiver_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_10: vec3<f32>, world_normal: vec3<f32>) -> vec3<f32> {
    var local_17: bool;

    let bias: f32 = max(shadow_view.light_params.z, 0f);
    let normal_len_sq: f32 = dot(world_normal, world_normal);
    if !((bias <= 0f)) {
        local_17 = (normal_len_sq <= 0.000000000001f);
    } else {
        local_17 = true;
    }
    let _e15: bool = local_17;
    if _e15 {
        return world_pos_10;
    }
    return (world_pos_10 + (normalize(world_normal) * bias));
}

fn radial_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_8: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_11: vec3<f32>) -> f32 {
    let range_4: f32 = max(light_8.range, 0.001f);
    return clamp((length((world_pos_11 - light_8.position.xyz)) / range_4), 0f, 1f);
}

fn point_face_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_2: vec3<f32>) -> u32 {
    var local_18: bool;

    let a_1: vec3<f32> = abs(direction_2);
    if (a_1.x >= a_1.y) {
        local_18 = (a_1.x >= a_1.z);
    } else {
        local_18 = false;
    }
    let _e11: bool = local_18;
    if _e11 {
        return select(1u, 0u, (direction_2.x >= 0f));
    }
    if (a_1.y >= a_1.z) {
        return select(3u, 2u, (direction_2.y >= 0f));
    }
    return select(5u, 4u, (direction_2.z >= 0f));
}

fn point_face_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face: u32) -> vec3<f32> {
    switch (face % 6u) {
        case 0u: {
            return vec3<f32>(1f, 0f, 0f);
        }
        case 1u: {
            return vec3<f32>(-1f, 0f, 0f);
        }
        case 2u: {
            return vec3<f32>(0f, 1f, 0f);
        }
        case 3u: {
            return vec3<f32>(0f, -1f, 0f);
        }
        case 4u: {
            return vec3<f32>(0f, 0f, 1f);
        }
        default: {
            return vec3<f32>(0f, 0f, -1f);
        }
    }
}

fn point_face_upX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face_1: u32) -> vec3<f32> {
    switch (face_1 % 6u) {
        case 2u: {
            return vec3<f32>(0f, 0f, -1f);
        }
        case 3u: {
            return vec3<f32>(0f, 0f, 1f);
        }
        default: {
            return vec3<f32>(0f, 1f, 0f);
        }
    }
}

fn point_face_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face_2: u32) -> PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX {
    let _e1: vec3<f32> = point_face_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face_2);
    let _e2: vec3<f32> = point_face_upX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face_2);
    return PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e1, _e2, cross(_e1, _e2));
}

fn point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_3: vec3<f32>) -> PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX {
    let _e1: u32 = point_face_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_3);
    let _e2: PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = point_face_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e1);
    let denom: f32 = max(dot(direction_3, _e2.direction), 0.000001f);
    let ndc_1: vec2<f32> = (vec2<f32>(dot(direction_3, _e2.right), dot(direction_3, _e2.up)) / vec2(denom));
    let local_uv_6: vec2<f32> = clamp(vec2<f32>(((ndc_1.x * 0.5f) + 0.5f), (0.5f - (ndc_1.y * 0.5f))), vec2(0f), vec2(1f));
    return PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e1, local_uv_6);
}

fn point_shadow_direction_for_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(basis: PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX, local_uv: vec2<f32>) -> vec3<f32> {
    let ndc_2: vec2<f32> = vec2<f32>(((local_uv.x * 2f) - 1f), (1f - (local_uv.y * 2f)));
    return ((basis.direction + (basis.right * ndc_2.x)) + (basis.up * ndc_2.y));
}

fn atlas_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_1: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, local_uv_1: vec2<f32>) -> vec2<f32> {
    return (shadow_view_1.atlas_rect.xy + (local_uv_1 * shadow_view_1.atlas_rect.zw));
}

fn sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_2: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, local_uv_2: vec2<f32>, layer: i32, compare_depth: f32) -> f32 {
    let _e2: vec2<f32> = atlas_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_2, local_uv_2);
    let _e7: f32 = textureSampleCompare(shadow_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, shadow_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e2, layer, compare_depth);
    return _e7;
}

fn sample_point_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_9: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, start: u32, face_uv: PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX, compare_depth_1: f32) -> f32 {
    let face_8: u32 = min(face_uv.face, (light_9.shadow_view_count - 1u));
    let shadow_view_8: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[(start + face_8)];
    let layer_4: i32 = i32((shadow_view_8.params.x + 0.5f));
    let _e19: f32 = sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_8, face_uv.local_uv, layer_4, compare_depth_1);
    return _e19;
}

fn sample_point_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_10: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, start_1: u32, direction_4: vec3<f32>, compare_depth_2: f32) -> f32 {
    var sum: f32 = 0f;
    var count: f32 = 0f;
    var y: i32 = -1i;
    var x: i32;

    let _e3: PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_4);
    let _e5: PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = point_face_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e3.face);
    let base_shadow_view: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[(start_1 + min(_e3.face, (light_10.shadow_view_count - 1u)))];
    let texel: f32 = max(base_shadow_view.params.y, 0.000001f);
    loop {
        let _e22: i32 = y;
        if (_e22 <= 1i) {
        } else {
            break;
        }
        {
            x = -1i;
            loop {
                let _e27: i32 = x;
                if (_e27 <= 1i) {
                } else {
                    break;
                }
                {
                    let _e31: i32 = x;
                    let _e33: i32 = y;
                    let tap_uv: vec2<f32> = (_e3.local_uv + (vec2<f32>(f32(_e31), f32(_e33)) * texel));
                    let _e38: vec3<f32> = point_shadow_direction_for_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e5, tap_uv);
                    let _e40: f32 = sum;
                    let _e41: PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e38);
                    let _e43: f32 = sample_point_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_10, start_1, _e41, compare_depth_2);
                    sum = (_e40 + _e43);
                    let _e46: f32 = count;
                    count = (_e46 + 1f);
                }
                continuing {
                    let _e49: i32 = x;
                    x = (_e49 + 1i);
                }
            }
        }
        continuing {
            let _e52: i32 = y;
            y = (_e52 + 1i);
        }
    }
    let _e55: f32 = count;
    if (_e55 <= 0f) {
        return 1f;
    }
    let _e59: f32 = sum;
    let _e60: f32 = count;
    return (_e59 / _e60);
}

fn sample_point_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_11: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, start_2: u32, direction_5: vec3<f32>, compare_depth_3: f32) -> f32 {
    let _e1: PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_5);
    let _e5: f32 = sample_point_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_11, start_2, _e1, compare_depth_3);
    return _e5;
}

fn sample_point_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_12: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, start_3: u32, direction_6: vec3<f32>, compare_depth_4: f32) -> f32 {
    if (light_12.shadow_type == SHADOW_TYPE_SOFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX) {
        let _e7: f32 = sample_point_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_12, start_3, direction_6, compare_depth_4);
        return _e7;
    }
    let _e8: f32 = sample_point_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_12, start_3, direction_6, compare_depth_4);
    return _e8;
}

fn point_shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_13: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, start_4: u32, world_pos_12: vec3<f32>, world_normal_1: vec3<f32>) -> f32 {
    let base_shadow_view_1: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[start_4];
    let _e6: vec3<f32> = receiver_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(base_shadow_view_1, world_pos_12, world_normal_1);
    let direction_7: vec3<f32> = (_e6 - light_13.position.xyz);
    if (dot(direction_7, direction_7) <= 0.000000000001f) {
        return 1f;
    }
    let _e15: f32 = radial_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_13, _e6);
    let _e16: f32 = sample_point_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_13, start_4, direction_7, _e15);
    return _e16;
}

fn shadow_view_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_3: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) -> u32 {
    return u32((max(shadow_view_3.light_params.x, 0f) + 0.5f));
}

fn radial_shadow_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(kind: u32) -> bool {
    var local_19: bool;

    if !((kind == SHADOW_VIEW_KIND_POINTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        local_19 = (kind == SHADOW_VIEW_KIND_SPOTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
    } else {
        local_19 = true;
    }
    let _e9: bool = local_19;
    return _e9;
}

fn shadow_uv_in_boundsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uv_7: vec2<f32>) -> bool {
    var local_20: bool;
    var local_21: bool;
    var local_22: bool;

    if (uv_7.x >= SHADOW_UV_BORDERX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX) {
        local_20 = (uv_7.x <= 0.9995f);
    } else {
        local_20 = false;
    }
    let _e10: bool = local_20;
    if _e10 {
        local_21 = (uv_7.y >= SHADOW_UV_BORDERX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX);
    } else {
        local_21 = false;
    }
    let _e17: bool = local_21;
    if _e17 {
        local_22 = (uv_7.y <= 0.9995f);
    } else {
        local_22 = false;
    }
    let _e24: bool = local_22;
    return _e24;
}

fn projected_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_14: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, shadow_view_4: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, ndc: vec3<f32>) -> f32 {
    let bias_1: f32 = max(light_14.shadow_bias, shadow_view_4.light_params.w);
    return clamp((ndc.z - bias_1), 0f, 1f);
}

fn sample_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_5: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, local_uv_3: vec2<f32>, layer_1: i32, compare_depth_5: f32) -> f32 {
    var sum_1: f32 = 0f;
    var count_1: f32 = 0f;
    var y_1: i32 = -1i;
    var x_1: i32;

    let texel_1: f32 = max(shadow_view_5.params.y, 0.000001f);
    loop {
        let _e8: i32 = y_1;
        if (_e8 <= 1i) {
        } else {
            break;
        }
        {
            x_1 = -1i;
            loop {
                let _e13: i32 = x_1;
                if (_e13 <= 1i) {
                } else {
                    break;
                }
                {
                    let _e17: i32 = x_1;
                    let _e19: i32 = y_1;
                    let tap_uv_1: vec2<f32> = (local_uv_3 + (vec2<f32>(f32(_e17), f32(_e19)) * texel_1));
                    let _e24: bool = shadow_uv_in_boundsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(tap_uv_1);
                    if _e24 {
                        let _e26: f32 = sum_1;
                        let _e29: f32 = sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_5, tap_uv_1, layer_1, compare_depth_5);
                        sum_1 = (_e26 + _e29);
                        let _e32: f32 = count_1;
                        count_1 = (_e32 + 1f);
                    }
                }
                continuing {
                    let _e35: i32 = x_1;
                    x_1 = (_e35 + 1i);
                }
            }
        }
        continuing {
            let _e38: i32 = y_1;
            y_1 = (_e38 + 1i);
        }
    }
    let _e41: f32 = count_1;
    if (_e41 <= 0f) {
        return 1f;
    }
    let _e45: f32 = sum_1;
    let _e46: f32 = count_1;
    return (_e45 / _e46);
}

fn sample_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_6: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, local_uv_4: vec2<f32>, layer_2: i32, compare_depth_6: f32) -> f32 {
    let _e4: f32 = sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_6, local_uv_4, layer_2, compare_depth_6);
    return _e4;
}

fn sample_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_15: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, shadow_view_7: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, local_uv_5: vec2<f32>, layer_3: i32, compare_depth_7: f32) -> f32 {
    if (light_15.shadow_type == SHADOW_TYPE_SOFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX) {
        let _e8: f32 = sample_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_7, local_uv_5, layer_3, compare_depth_7);
        return _e8;
    }
    let _e9: f32 = sample_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_7, local_uv_5, layer_3, compare_depth_7);
    return _e9;
}

fn shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, view_index_1: u32, world_pos_13: vec3<f32>, world_normal_2: vec3<f32>) -> f32 {
    var local_23: bool;
    var local_24: bool;
    var compare_depth_8: f32;

    let shadow_view_9: GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[view_index_1];
    let _e6: vec3<f32> = receiver_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_9, world_pos_13, world_normal_2);
    let clip_1: vec4<f32> = (shadow_view_9.world_to_shadow * vec4<f32>(_e6, 1f));
    if (clip_1.w <= 0f) {
        return -1f;
    }
    let ndc_3: vec3<f32> = (clip_1.xyz / vec3(clip_1.w));
    let _e19: u32 = shadow_view_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_9);
    let _e20: bool = radial_shadow_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e19);
    if !(_e20) {
        if !((ndc_3.z < 0f)) {
            local_24 = (ndc_3.z > 1f);
        } else {
            local_24 = true;
        }
        let _e32: bool = local_24;
        local_23 = _e32;
    } else {
        local_23 = false;
    }
    let _e36: bool = local_23;
    if _e36 {
        return -1f;
    }
    let uv_11: vec2<f32> = vec2<f32>(((ndc_3.x * 0.5f) + 0.5f), (0.5f - (ndc_3.y * 0.5f)));
    let _e49: bool = shadow_uv_in_boundsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uv_11);
    if !(_e49) {
        return -1f;
    }
    let layer_5: i32 = i32((shadow_view_9.params.x + 0.5f));
    if _e20 {
        let _e58: f32 = radial_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16, _e6);
        compare_depth_8 = _e58;
    } else {
        let _e60: f32 = projected_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16, shadow_view_9, ndc_3);
        compare_depth_8 = _e60;
    }
    let _e61: f32 = compare_depth_8;
    let _e62: f32 = sample_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16, shadow_view_9, uv_11, layer_5, _e61);
    return _e62;
}

fn visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_14: vec3<f32>, world_normal_3: vec3<f32>) -> f32 {
    var local_25: bool;
    var i: u32 = 0u;

    let _e2: bool = has_shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17);
    if !(_e2) {
        return 1f;
    }
    let strength_1: f32 = clamp(light_17.shadow_strength, 0f, 1f);
    let start_5: u32 = light_17.shadow_view_start;
    let count_2: u32 = light_17.shadow_view_count;
    if (light_17.light_type == 0u) {
        local_25 = (count_2 >= 6u);
    } else {
        local_25 = false;
    }
    let _e19: bool = local_25;
    if _e19 {
        let _e22: f32 = point_shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17, start_5, world_pos_14, world_normal_3);
        return mix(1f, select(1f, _e22, (_e22 >= 0f)), strength_1);
    }
    loop {
        let _e30: u32 = i;
        if (_e30 < count_2) {
        } else {
            break;
        }
        {
            let _e32: u32 = i;
            let _e34: f32 = shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17, (start_5 + _e32), world_pos_14, world_normal_3);
            if (_e34 >= 0f) {
                return mix(1f, _e34, strength_1);
            }
        }
        continuing {
            let _e40: u32 = i;
            i = (_e40 + 1u);
        }
    }
    return 1f;
}

fn distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(dist_2: f32, range_3: f32) -> f32 {
    let _e2: f32 = distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(dist_2, range_3);
    return _e2;
}

fn eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(light_18: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_15: vec3<f32>, world_normal_4: vec3<f32>) -> LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX {
    var out_1: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX;

    let light_pos: vec3<f32> = light_18.position.xyz;
    let light_dir: vec3<f32> = light_18.direction.xyz;
    if (light_18.light_type == 0u) {
        let to_light: vec3<f32> = (light_pos - world_pos_15);
        let dist_sq: f32 = dot(to_light, to_light);
        let dist_3: f32 = sqrt(dist_sq);
        out_1.l = (to_light / vec3(dist_3));
        let _e18: f32 = distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(dist_3, light_18.range);
        out_1.attenuation = _e18;
        let _e21: f32 = out_1.attenuation;
        let _e22: f32 = multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_18, world_pos_15);
        out_1.attenuation = (_e21 * _e22);
    } else {
        if (light_18.light_type == 1u) {
            let dir_len_sq: f32 = dot(light_dir, light_dir);
            out_1.l = select(vec3<f32>(0f, 0f, 1f), (-(light_dir) / vec3(sqrt(dir_len_sq))), (dir_len_sq > 0.0000000000000001f));
            let _e41: f32 = direct_light_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX();
            out_1.attenuation = _e41;
            let _e44: f32 = out_1.attenuation;
            let _e45: f32 = multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_18, world_pos_15);
            out_1.attenuation = (_e44 * _e45);
        } else {
            let to_light_1: vec3<f32> = (light_pos - world_pos_15);
            let dist_sq_1: f32 = dot(to_light_1, to_light_1);
            let dist_4: f32 = sqrt(dist_sq_1);
            out_1.l = (to_light_1 / vec3(dist_4));
            let _e54: vec3<f32> = out_1.l;
            let _e55: f32 = spot_angle_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_18, _e54);
            let _e58: f32 = distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(dist_4, light_18.range);
            out_1.attenuation = (_e55 * _e58);
            let _e62: f32 = out_1.attenuation;
            let _e63: f32 = multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_18, world_pos_15);
            out_1.attenuation = (_e62 * _e63);
        }
    }
    let _e67: f32 = out_1.attenuation;
    let _e69: f32 = visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_18, world_pos_15, world_normal_4);
    out_1.attenuation = (_e67 * _e69);
    let _e71: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX = out_1;
    return _e71;
}

fn signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(light_19: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, attenuation: f32, n_dot_l: f32) -> vec3<f32> {
    let _e1: vec3<f32> = light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_19);
    return ((_e1 * attenuation) * n_dot_l);
}

fn safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_10: vec3<f32>, fallback_3: vec3<f32>) -> vec3<f32> {
    if (dot(n_10, n_10) <= 0.000000000001f) {
        return fallback_3;
    }
    return normalize(n_10);
}

fn normal_derivative_varianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_11: vec3<f32>) -> f32 {
    let _e5: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_11, vec3<f32>(0f, 0f, 1f));
    let du: vec3<f32> = dpdx(_e5);
    let dv: vec3<f32> = dpdy(_e5);
    return (SPECULAR_AA_VARIANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX * (dot(du, du) + dot(dv, dv)));
}

fn filter_normal_map_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness: f32, variance: f32) -> f32 {
    let clamped: f32 = clamp(perceptual_roughness, 0f, 1f);
    let kernel: f32 = min((2f * variance), 0.040000003f);
    return sqrt(clamp(((clamped * clamped) + kernel), 0f, 1f));
}

fn filter_geometric_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_1: f32, variance_1: f32) -> f32 {
    let clamped_1: f32 = clamp(perceptual_roughness_1, 0f, 1f);
    let alpha_7: f32 = (clamped_1 * clamped_1);
    let kernel_1: f32 = min((2f * variance_1), GEOMETRIC_SPECULAR_AA_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    let alpha2_: f32 = clamp(((alpha_7 * alpha_7) + kernel_1), 0f, 1f);
    return sqrt(sqrt(alpha2_));
}

fn filter_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_2: f32, shading_normal: vec3<f32>, geometric_normal_1: vec3<f32>) -> f32 {
    let _e1: f32 = normal_derivative_varianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(shading_normal);
    let _e3: f32 = normal_derivative_varianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(geometric_normal_1);
    let normal_map_variance: f32 = max((_e1 - _e3), 0f);
    let _e8: f32 = filter_normal_map_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_2, normal_map_variance);
    let _e9: f32 = filter_geometric_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e8, _e3);
    return _e9;
}

fn view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_12: vec3<f32>, v_3: vec3<f32>) -> vec3<f32> {
    let _e5: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_12, vec3<f32>(0f, 0f, 1f));
    let _e11: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_3, vec3<f32>(0f, 0f, 1f));
    let n_dot_v_8: f32 = dot(_e5, _e11);
    if (n_dot_v_8 < MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX) {
        let _e19: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((_e5 + (_e11 * (MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX - n_dot_v_8))), _e11);
        return _e19;
    }
    return _e5;
}

fn direct_alpha_from_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_3: f32) -> f32 {
    let clamped_2: f32 = clamp(perceptual_roughness_3, 0f, 1f);
    return max((clamped_2 * clamped_2), MIN_ALPHAX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
}

fn direct_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_4: f32) -> f32 {
    let _e1: f32 = direct_alpha_from_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_4);
    return sqrt(_e1);
}

fn f90_from_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(f0_: vec3<f32>) -> f32 {
    return clamp(dot(f0_, vec3(16.666666f)), 0f, 1f);
}

fn sample_ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_5: f32, n_dot_v: f32) -> vec2<f32> {
    let dims_u: vec2<u32> = textureDimensions(ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX);
    let dims: vec2<f32> = vec2<f32>(f32(dims_u.x), f32(dims_u.y));
    let max_xy: vec2<i32> = vec2<i32>((i32(dims_u.x) - 1i), (i32(dims_u.y) - 1i));
    let uv_12: vec2<f32> = vec2<f32>(clamp(n_dot_v, 0f, 1f), clamp(perceptual_roughness_5, 0f, 1f));
    let xy_2: vec2<f32> = ((uv_12 * dims) - vec2(0.5f));
    let base: vec2<f32> = floor(xy_2);
    let base_i: vec2<i32> = vec2<i32>(base);
    let f_1: vec2<f32> = (xy_2 - base);
    let p00_: vec2<i32> = clamp(base_i, vec2(0i), max_xy);
    let p10_: vec2<i32> = clamp((base_i + vec2<i32>(1i, 0i)), vec2(0i), max_xy);
    let p01_: vec2<i32> = clamp((base_i + vec2<i32>(0i, 1i)), vec2(0i), max_xy);
    let p11_: vec2<i32> = clamp((base_i + vec2<i32>(1i, 1i)), vec2(0i), max_xy);
    let _e58: vec4<f32> = textureLoad(ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, p00_, 0i);
    let a_2: vec2<f32> = _e58.xy;
    let _e62: vec4<f32> = textureLoad(ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, p10_, 0i);
    let b_2: vec2<f32> = _e62.xy;
    let _e66: vec4<f32> = textureLoad(ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, p01_, 0i);
    let c_2: vec2<f32> = _e66.xy;
    let _e70: vec4<f32> = textureLoad(ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, p11_, 0i);
    let d_1: vec2<f32> = _e70.xy;
    return mix(mix(a_2, b_2, f_1.x), mix(c_2, d_1, f_1.x), f_1.y);
}

fn energy_compensation_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(dfg: vec2<f32>, f0_1: vec3<f32>) -> vec3<f32> {
    let clamped_f0_: vec3<f32> = clamp(f0_1, vec3(0f), vec3(1f));
    return (vec3(1f) + (clamped_f0_ * (vec3((1f / max(dfg.y, 0.0001f))) - vec3(1f))));
}

fn specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(dfg_1: vec2<f32>, f0_2: vec3<f32>) -> vec3<f32> {
    let clamped_f0_1: vec3<f32> = clamp(f0_2, vec3(0f), vec3(1f));
    let _e6: f32 = f90_from_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(clamped_f0_1);
    let f90_6: vec3<f32> = vec3(_e6);
    return ((clamped_f0_1 * (dfg_1.y - dfg_1.x)) + (f90_6 * dfg_1.x));
}

fn indirect_specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(dfg_2: vec2<f32>, f0_3: vec3<f32>, enabled_5: bool) -> vec3<f32> {
    if !(enabled_5) {
        return vec3(0f);
    }
    let _e6: vec3<f32> = specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(dfg_2, f0_3);
    return _e6;
}

fn specular_ao_lagardeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_1: f32, visibility: f32, perceptual_roughness_6: f32) -> f32 {
    let no_v: f32 = clamp(n_dot_v_1, 0f, 1f);
    let ao: f32 = clamp(visibility, 0f, 1f);
    let linear_roughness: f32 = (clamp(perceptual_roughness_6, 0f, 1f) * clamp(perceptual_roughness_6, 0f, 1f));
    let exponent: f32 = exp2(((-16f * linear_roughness) - 1f));
    return clamp(((pow((no_v + ao), exponent) - 1f) + ao), 0f, 1f);
}

fn multi_bounce_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(visibility_1: f32, albedo: vec3<f32>) -> vec3<f32> {
    let ao_1: f32 = clamp(visibility_1, 0f, 1f);
    let clamped_albedo: vec3<f32> = clamp(albedo, vec3(0f), vec3(1f));
    let a_3: vec3<f32> = ((2.0404f * clamped_albedo) - vec3(0.3324f));
    let b_3: vec3<f32> = ((-4.7951f * clamped_albedo) + vec3(0.6417f));
    let c_3: vec3<f32> = ((2.7552f * clamped_albedo) + vec3(0.6903f));
    return max(vec3(ao_1), (((((ao_1 * a_3) + b_3) * ao_1) + c_3) * ao_1));
}

fn indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_2: f32, visibility_2: f32, perceptual_roughness_7: f32, f0_4: vec3<f32>) -> vec3<f32> {
    let _e3: f32 = specular_ao_lagardeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_2, visibility_2, perceptual_roughness_7);
    let _e5: vec3<f32> = multi_bounce_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e3, f0_4);
    return _e5;
}

fn indirect_diffuse_energy_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(specular_energy: vec3<f32>, enabled_6: bool) -> vec3<f32> {
    if !(enabled_6) {
        return vec3(1f);
    }
    return max(vec3(0f), (vec3(1f) - specular_energy));
}

fn indirect_diffuse_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(visibility_3: f32, diffuse_color: vec3<f32>) -> vec3<f32> {
    let _e2: vec3<f32> = multi_bounce_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(visibility_3, diffuse_color);
    return _e2;
}

fn indirect_diffuse_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(ambient: vec3<f32>, base_color_2: vec3<f32>, metallic: f32, specular_energy_1: vec3<f32>, occlusion_1: f32, glossy_reflections_enabled: bool) -> vec3<f32> {
    let _e2: vec3<f32> = indirect_diffuse_energy_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(specular_energy_1, glossy_reflections_enabled);
    let diffuse_color_1: vec3<f32> = (base_color_2 * (1f - clamp(metallic, 0f, 1f)));
    let _e12: vec3<f32> = indirect_diffuse_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(occlusion_1, diffuse_color_1);
    return (((ambient * diffuse_color_1) * _e2) * _e12);
}

fn indirect_diffuse_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(ambient_1: vec3<f32>, base_color_3: vec3<f32>, one_minus_reflectivity: f32, specular_energy_2: vec3<f32>, occlusion_2: f32, glossy_reflections_enabled_1: bool) -> vec3<f32> {
    let _e2: vec3<f32> = indirect_diffuse_energy_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(specular_energy_2, glossy_reflections_enabled_1);
    let diffuse_color_2: vec3<f32> = (base_color_3 * clamp(one_minus_reflectivity, 0f, 1f));
    let _e10: vec3<f32> = indirect_diffuse_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(occlusion_2, diffuse_color_2);
    return (((ambient_1 * diffuse_color_2) * _e2) * _e10);
}

fn metallic_one_minus_reflectivityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(metallic_1: f32) -> f32 {
    return (0.96f * (1f - clamp(metallic_1, 0f, 1f)));
}

fn unity_premultiplied_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(alpha_2: f32, one_minus_reflectivity_1: f32) -> f32 {
    let diffuse_alpha: f32 = clamp(alpha_2, 0f, 1f);
    let diffuse_visibility: f32 = clamp(one_minus_reflectivity_1, 0f, 1f);
    return ((1f - diffuse_visibility) + (diffuse_alpha * diffuse_visibility));
}

fn metallic_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(base_color_4: vec3<f32>, metallic_2: f32) -> vec3<f32> {
    return mix(vec3(0.04f), base_color_4, metallic_2);
}

fn direct_brdf_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_13: vec3<f32>, v_4: vec3<f32>) -> DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX {
    let _e5: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_4, vec3<f32>(0f, 0f, 1f));
    let _e7: vec3<f32> = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_13, _e5);
    return DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e5, _e7);
}

fn eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_20: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_16: vec3<f32>, world_normal_5: vec3<f32>) -> LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX {
    let _e3: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(light_20, world_pos_16, world_normal_5);
    return _e3;
}

fn pow5X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(x_2: f32) -> f32 {
    let x2_: f32 = (x_2 * x_2);
    return ((x2_ * x2_) * x_2);
}

fn f_schlickX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(f0_5: vec3<f32>, f90_: f32, v_dot_h: f32) -> vec3<f32> {
    let _e7: f32 = pow5X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((1f - v_dot_h));
    return (f0_5 + ((vec3(f90_) - f0_5) * _e7));
}

fn d_ggxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_h: f32, roughness_1: f32) -> f32 {
    let a_4: f32 = (n_dot_h * roughness_1);
    let k: f32 = (roughness_1 / max(((1f - (n_dot_h * n_dot_h)) + (a_4 * a_4)), 0.0000001f));
    return min(((k * k) * 0.31830987f), 65504f);
}

fn v_smith_ggx_correlatedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_3: f32, n_dot_l_1: f32, roughness_2: f32) -> f32 {
    let a2_: f32 = (roughness_2 * roughness_2);
    let lv: f32 = (n_dot_l_1 * sqrt((((n_dot_v_3 - (a2_ * n_dot_v_3)) * n_dot_v_3) + a2_)));
    let ll: f32 = (n_dot_v_3 * sqrt((((n_dot_l_1 - (a2_ * n_dot_l_1)) * n_dot_l_1) + a2_)));
    return (0.5f / max((lv + ll), 0.0000001f));
}

fn eval_direct_specular_lobe_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(basis_1: DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX, l_1: vec3<f32>, alpha_3: f32, f0_6: vec3<f32>, f90_1: f32, energy_compensation: vec3<f32>) -> DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX {
    var h: vec3<f32>;

    let view: vec3<f32> = basis_1.view;
    let brdf_n: vec3<f32> = basis_1.n;
    let _e4: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(l_1, brdf_n);
    let n_dot_l_4: f32 = max(dot(brdf_n, _e4), 0f);
    let n_dot_v_9: f32 = max(dot(brdf_n, view), MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    if (n_dot_l_4 <= 0f) {
        return DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(vec3(0f), vec3(0f), n_dot_l_4, n_dot_v_9, 0f);
    }
    let h_unorm: vec3<f32> = (view + _e4);
    h = brdf_n;
    if (dot(h_unorm, h_unorm) > 0.000000000001f) {
        h = normalize(h_unorm);
    }
    let _e25: vec3<f32> = h;
    let n_dot_h_1: f32 = clamp(dot(brdf_n, _e25), 0f, 1f);
    let _e30: vec3<f32> = h;
    let v_dot_h_1: f32 = clamp(dot(view, _e30), 0f, 1f);
    let _e35: vec3<f32> = h;
    let l_dot_h_1: f32 = clamp(dot(_e4, _e35), 0f, 1f);
    let _e42: vec3<f32> = f_schlickX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(f0_6, f90_1, v_dot_h_1);
    let _e44: f32 = d_ggxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_h_1, alpha_3);
    let _e45: f32 = v_smith_ggx_correlatedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_9, n_dot_l_4, alpha_3);
    let fr: vec3<f32> = max(vec3(0f), (((_e44 * _e45) * _e42) * energy_compensation));
    return DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e42, fr, n_dot_l_4, n_dot_v_9, l_dot_h_1);
}

fn max_componentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_5: vec3<f32>) -> f32 {
    return max(max(v_5.x, v_5.y), v_5.z);
}

fn direct_diffuse_fresnel_transmissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(f: vec3<f32>, f0_7: vec3<f32>) -> f32 {
    let _e6: f32 = max_componentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(clamp(f, vec3(0f), vec3(1f)));
    let _e13: f32 = max_componentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(clamp(f0_7, vec3(0f), vec3(1f)));
    return clamp(((1f - _e6) / max((1f - _e13), 0.0001f)), 0f, 1f);
}

fn f_schlick_scalarX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(f0_8: f32, f90_2: f32, cos_theta: f32) -> f32 {
    let _e9: f32 = pow5X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((1f - clamp(cos_theta, 0f, 1f)));
    return (f0_8 + ((f90_2 - f0_8) * _e9));
}

fn fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_4: f32, n_dot_l_2: f32, l_dot_h: f32, perceptual_roughness_8: f32) -> f32 {
    let roughness_5: f32 = clamp(perceptual_roughness_8, 0f, 1f);
    let loh: f32 = clamp(l_dot_h, 0f, 1f);
    let fd90_: f32 = (0.5f + (((2f * roughness_5) * loh) * loh));
    let _e16: f32 = f_schlick_scalarX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(1f, fd90_, n_dot_l_2);
    let _e19: f32 = f_schlick_scalarX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(1f, fd90_, n_dot_v_4);
    return ((_e16 * _e19) * 0.31830987f);
}

fn signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_21: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, attenuation_1: f32, n_dot_l_3: f32) -> vec3<f32> {
    let _e3: vec3<f32> = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(light_21, attenuation_1, n_dot_l_3);
    return _e3;
}

fn direct_radiance_metallic_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_22: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_17: vec3<f32>, shading_normal_1: vec3<f32>, basis_2: DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX, alpha_4: f32, f90_3: f32, diffuse_roughness: f32, metallic_3: f32, base_color_5: vec3<f32>, f0_9: vec3<f32>, energy_compensation_1: vec3<f32>) -> vec3<f32> {
    let _e3: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_22, world_pos_17, shading_normal_1);
    let _e10: DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = eval_direct_specular_lobe_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(basis_2, _e3.l, alpha_4, f0_9, f90_3, energy_compensation_1);
    if (_e10.n_dot_l <= 0f) {
        return vec3(0f);
    }
    let diffuse_color_3: vec3<f32> = (base_color_5 * (1f - metallic_3));
    let _e22: f32 = direct_diffuse_fresnel_transmissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e10.f, f0_9);
    let _e28: f32 = fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e10.n_dot_v, _e10.n_dot_l, _e10.l_dot_h, diffuse_roughness);
    let fd: vec3<f32> = ((diffuse_color_3 * _e22) * _e28);
    let _e32: vec3<f32> = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_22, _e3.attenuation, _e10.n_dot_l);
    return ((fd + _e10.specular_brdf) * _e32);
}

fn direct_radiance_specular_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_23: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_18: vec3<f32>, shading_normal_2: vec3<f32>, basis_3: DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX, alpha_5: f32, f90_4: f32, diffuse_roughness_1: f32, base_color_6: vec3<f32>, f0_10: vec3<f32>, one_minus_reflectivity_2: f32, energy_compensation_2: vec3<f32>) -> vec3<f32> {
    let _e3: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_23, world_pos_18, shading_normal_2);
    let _e10: DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = eval_direct_specular_lobe_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(basis_3, _e3.l, alpha_5, f0_10, f90_4, energy_compensation_2);
    if (_e10.n_dot_l <= 0f) {
        return vec3(0f);
    }
    let diffuse_color_4: vec3<f32> = (base_color_6 * one_minus_reflectivity_2);
    let _e20: f32 = direct_diffuse_fresnel_transmissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e10.f, f0_10);
    let _e26: f32 = fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e10.n_dot_v, _e10.n_dot_l, _e10.l_dot_h, diffuse_roughness_1);
    let fd_1: vec3<f32> = ((diffuse_color_4 * _e20) * _e26);
    let _e30: vec3<f32> = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_23, _e3.attenuation, _e10.n_dot_l);
    return ((fd_1 + _e10.specular_brdf) * _e30);
}

fn diffuse_only_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_24: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_19: vec3<f32>, n_14: vec3<f32>, v_6: vec3<f32>, roughness_3: f32, base_color_7: vec3<f32>, metallic_4: f32) -> vec3<f32> {
    let _e3: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_24, world_pos_19, n_14);
    let _e9: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_6, vec3<f32>(0f, 0f, 1f));
    let _e10: vec3<f32> = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_14, _e9);
    let n_dot_l_5: f32 = max(dot(_e10, _e3.l), 0f);
    if (n_dot_l_5 <= 0f) {
        return vec3(0f);
    }
    let _e21: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((_e9 + _e3.l), _e10);
    let n_dot_v_10: f32 = max(dot(_e10, _e9), MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    let l_dot_h_2: f32 = clamp(dot(_e3.l, _e21), 0f, 1f);
    let diffuse_color_5: vec3<f32> = (base_color_7 * (1f - metallic_4));
    let _e36: f32 = fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_10, n_dot_l_5, l_dot_h_2, roughness_3);
    let _e39: vec3<f32> = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_24, _e3.attenuation, n_dot_l_5);
    return ((diffuse_color_5 * _e36) * _e39);
}

fn diffuse_only_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_25: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_20: vec3<f32>, n_15: vec3<f32>, v_7: vec3<f32>, roughness_4: f32, base_color_8: vec3<f32>, one_minus_reflectivity_3: f32) -> vec3<f32> {
    let _e3: LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_25, world_pos_20, n_15);
    let _e9: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_7, vec3<f32>(0f, 0f, 1f));
    let _e10: vec3<f32> = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_15, _e9);
    let n_dot_l_6: f32 = max(dot(_e10, _e3.l), 0f);
    if (n_dot_l_6 <= 0f) {
        return vec3(0f);
    }
    let _e21: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((_e9 + _e3.l), _e10);
    let n_dot_v_11: f32 = max(dot(_e10, _e9), MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    let l_dot_h_3: f32 = clamp(dot(_e3.l, _e21), 0f, 1f);
    let _e34: f32 = fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_11, n_dot_l_6, l_dot_h_3, roughness_4);
    let _e37: vec3<f32> = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_25, _e3.attenuation, n_dot_l_6);
    return (((base_color_8 * one_minus_reflectivity_3) * _e34) * _e37);
}

fn local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(draw_40: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> vec4<u32> {
    let _e1: vec4<u32> = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_40);
    return _e1;
}

fn fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(draw_41: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    let _e1: u32 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_41);
    return _e1;
}

fn reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(draw_42: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> u32 {
    let _e1: u32 = reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_42);
    return _e1;
}

fn has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(draw_43: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX) -> bool {
    let _e1: bool = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_43);
    return _e1;
}

fn draw_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_12: u32) -> u32 {
    return (view_layer_12 >> 1u);
}

fn safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(v_8: vec3<f32>) -> vec3<f32> {
    let len_sq_3: f32 = dot(v_8, v_8);
    if (len_sq_3 <= 0.00000000000000000001f) {
        return vec3<f32>(0f, 0f, 1f);
    }
    return (v_8 * inverseSqrt(len_sq_3));
}

fn dir_to_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_in: vec3<f32>) -> CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX {
    var local_26: bool;

    let _e1: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_in);
    let a_5: vec3<f32> = abs(_e1);
    if (a_5.x >= a_5.y) {
        local_26 = (a_5.x >= a_5.z);
    } else {
        local_26 = false;
    }
    let _e12: bool = local_26;
    if _e12 {
        if (_e1.x >= 0f) {
            return CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(0u, ((vec2<f32>((-(_e1.z) / a_5.x), (-(_e1.y) / a_5.x)) * 0.5f) + vec2(0.5f)));
        }
        return CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(1u, ((vec2<f32>((_e1.z / a_5.x), (-(_e1.y) / a_5.x)) * 0.5f) + vec2(0.5f)));
    }
    if (a_5.y >= a_5.z) {
        if (_e1.y >= 0f) {
            return CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(2u, ((vec2<f32>((_e1.x / a_5.y), (_e1.z / a_5.y)) * 0.5f) + vec2(0.5f)));
        }
        return CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(3u, ((vec2<f32>((_e1.x / a_5.y), (-(_e1.z) / a_5.y)) * 0.5f) + vec2(0.5f)));
    }
    if (_e1.z >= 0f) {
        return CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(4u, ((vec2<f32>((_e1.x / a_5.z), (-(_e1.y) / a_5.z)) * 0.5f) + vec2(0.5f)));
    }
    return CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(5u, ((vec2<f32>((-(_e1.x) / a_5.z), (-(_e1.y) / a_5.z)) * 0.5f) + vec2(0.5f)));
}

fn face_st_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_3: u32, st_1: vec2<f32>) -> vec3<f32> {
    let s_8: f32 = st_1.x;
    let t_14: f32 = st_1.y;
    if (face_3 == 0u) {
        let _e10: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(vec3<f32>(1f, -(t_14), -(s_8)));
        return _e10;
    }
    if (face_3 == 1u) {
        let _e16: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(vec3<f32>(-1f, -(t_14), s_8));
        return _e16;
    }
    if (face_3 == 2u) {
        let _e21: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(vec3<f32>(s_8, 1f, t_14));
        return _e21;
    }
    if (face_3 == 3u) {
        let _e27: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(vec3<f32>(s_8, -1f, -(t_14)));
        return _e27;
    }
    if (face_3 == 4u) {
        let _e33: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(vec3<f32>(s_8, -(t_14), 1f));
        return _e33;
    }
    let _e38: vec3<f32> = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(vec3<f32>(-(s_8), -(t_14), -1f));
    return _e38;
}

fn face_uv_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_4: u32, uv_8: vec2<f32>) -> vec3<f32> {
    let _e7: vec3<f32> = face_st_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_4, ((uv_8 * 2f) - vec2(1f)));
    return _e7;
}

fn face_texel_coord_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_5: u32, coord_3: vec2<f32>, face_size: u32) -> vec3<f32> {
    let size_2: u32 = max(face_size, 1u);
    let uv_13: vec2<f32> = ((coord_3 + vec2(0.5f)) / vec2(f32(size_2)));
    let _e11: vec3<f32> = face_uv_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_5, uv_13);
    return _e11;
}

fn canonical_texel_from_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_1: vec3<f32>, face_size_1: u32) -> vec3<i32> {
    let size_3: u32 = max(face_size_1, 1u);
    let _e4: CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX = dir_to_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_1);
    let xy_3: vec2<i32> = clamp(vec2<i32>(floor((_e4.uv * f32(size_3)))), vec2(0i), vec2((i32(size_3) - 1i)));
    return vec3<i32>(xy_3, i32(_e4.face));
}

fn canonical_texel_from_face_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_6: u32, coord_4: vec2<f32>, face_size_2: u32) -> vec3<i32> {
    let _e3: vec3<f32> = face_texel_coord_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_6, coord_4, face_size_2);
    let _e4: vec3<i32> = canonical_texel_from_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(_e3, face_size_2);
    return _e4;
}

fn load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_1: texture_2d_array<f32>, face_7: u32, coord_5: vec2<f32>, face_size_3: u32, mip_level: u32, base_layer: u32) -> vec3<f32> {
    let _e3: vec3<i32> = canonical_texel_from_face_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_7, coord_5, face_size_3);
    let _e12: vec4<f32> = textureLoad(tex_1, _e3.xy, (_e3.z + i32(base_layer)), i32(mip_level));
    return _e12.xyz;
}

fn sample_bilinear_lod_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2: texture_2d_array<f32>, dir_2: vec3<f32>, face_size_4: u32, mip_level_1: u32, base_layer_1: u32) -> vec3<f32> {
    let size_4: u32 = max((face_size_4 >> mip_level_1), 1u);
    let _e6: CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX = dir_to_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_2);
    let coord_6: vec2<f32> = ((_e6.uv * f32(size_4)) - vec2(0.5f));
    let base_1: vec2<f32> = floor(coord_6);
    let f_2: vec2<f32> = (coord_6 - base_1);
    let _e18: vec3<f32> = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2, _e6.face, base_1, size_4, mip_level_1, base_layer_1);
    let _e24: vec3<f32> = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2, _e6.face, (base_1 + vec2<f32>(1f, 0f)), size_4, mip_level_1, base_layer_1);
    let _e30: vec3<f32> = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2, _e6.face, (base_1 + vec2<f32>(0f, 1f)), size_4, mip_level_1, base_layer_1);
    let _e36: vec3<f32> = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2, _e6.face, (base_1 + vec2<f32>(1f, 1f)), size_4, mip_level_1, base_layer_1);
    return mix(mix(_e18, _e24, f_2.x), mix(_e30, _e36, f_2.x), f_2.y);
}

fn sample_trilinear_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_3: texture_2d_array<f32>, dir_3: vec3<f32>, lod: f32, base_face_size: u32, max_lod: f32, base_layer_2: u32) -> vec3<f32> {
    let clamped_lod: f32 = clamp(lod, 0f, max_lod);
    let lod0f: f32 = floor(clamped_lod);
    let lod0_: u32 = u32(lod0f);
    let lod1_: u32 = min((lod0_ + 1u), u32(max_lod));
    let _e14: vec3<f32> = sample_bilinear_lod_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_3, dir_3, base_face_size, lod0_, base_layer_2);
    let _e15: vec3<f32> = sample_bilinear_lod_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_3, dir_3, base_face_size, lod1_, base_layer_2);
    return mix(_e14, _e15, (clamped_lod - lod0f));
}

fn diffuse_from_raw_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(sh_a: vec3<f32>, sh_b: vec3<f32>, sh_c: vec3<f32>, sh_d: vec3<f32>, sh_e: vec3<f32>, sh_f: vec3<f32>, sh_g: vec3<f32>, sh_h: vec3<f32>, sh_i: vec3<f32>, normal_ws: vec3<f32>) -> vec3<f32> {
    let n_22: vec3<f32> = normalize(normal_ws);
    let sh: vec3<f32> = (((((((((sh_a * 0.2820948f) + (sh_b * (0.32573503f * n_22.y))) + (sh_c * (0.32573503f * n_22.z))) + (sh_d * (0.32573503f * n_22.x))) + (sh_e * ((0.27313712f * n_22.x) * n_22.y))) + (sh_f * ((0.27313712f * n_22.y) * n_22.z))) + (sh_g * (0.07884789f * (((3f * n_22.z) * n_22.z) - 1f)))) + (sh_h * ((0.27313712f * n_22.x) * n_22.z))) + (sh_i * (0.13656856f * ((n_22.x * n_22.x) - (n_22.y * n_22.y)))));
    return max(sh, vec3(0f));
}

fn ambient_probeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(normal_ws_1: vec3<f32>) -> vec3<f32> {
    let _e2: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_a;
    let _e6: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_b;
    let _e10: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_c;
    let _e14: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_d;
    let _e18: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_e;
    let _e22: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_f;
    let _e26: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_g;
    let _e30: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_h;
    let _e34: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_i;
    let _e37: vec3<f32> = diffuse_from_raw_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(_e2.xyz, _e6.xyz, _e10.xyz, _e14.xyz, _e18.xyz, _e22.xyz, _e26.xyz, _e30.xyz, _e34.xyz, normal_ws_1);
    return _e37;
}

fn ambient_probe_is_validX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX() -> bool {
    let _e3: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.frame_tail.w;
    return (_e3 != 0u);
}

fn selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_13: u32) -> PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX {
    let _e1: u32 = draw_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_13);
    let _e2: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = get_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e1);
    return _e2;
}

fn has_indirect_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_14: u32, enabled_7: bool) -> bool {
    var local_27: bool;

    let _e1: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_14);
    if enabled_7 {
        let _e3: bool = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e1);
        local_27 = _e3;
    } else {
        local_27 = false;
    }
    let _e7: bool = local_27;
    return _e7;
}

fn ambient_probe_or_zeroX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(normal_ws_2: vec3<f32>) -> vec3<f32> {
    let _e0: bool = ambient_probe_is_validX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX();
    if _e0 {
        let _e2: vec3<f32> = ambient_probeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(normal_ws_2);
        return _e2;
    }
    return vec3(0f);
}

fn empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX() -> ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX {
    return ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(vec3(0f), 0f, 0f);
}

fn add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total: vec4<f32>, importance: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) -> vec4<f32> {
    var local_28: bool;

    if !((importance.weight_sum < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        local_28 = (importance.coverage < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX);
    } else {
        local_28 = true;
    }
    let _e11: bool = local_28;
    if _e11 {
        return total;
    }
    let remaining: f32 = min(importance.coverage, (1f - total.w));
    let result: vec3<f32> = (importance.weighted_result / vec3(importance.weight_sum));
    return (total + vec4<f32>((result * remaining), remaining));
}

fn probe_sh2_sourceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index: u32) -> f32 {
    if (atlas_index == 0u) {
        return PROBE_SH2_SOURCE_NONEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX;
    }
    let _e8: f32 = reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[atlas_index].params.w;
    return _e8;
}

fn probe_has_any_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_1: u32) -> bool {
    let _e1: f32 = probe_sh2_sourceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_1);
    return (_e1 != PROBE_SH2_SOURCE_NONEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX);
}

fn sample_probe_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_2: u32, normal_ws_3: vec3<f32>) -> vec3<f32> {
    let _e1: bool = probe_has_any_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_2);
    if !(_e1) {
        return vec3(0f);
    }
    let probe_3: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[atlas_index_2];
    let _e27: vec3<f32> = diffuse_from_raw_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(probe_3.sh2_a.xyz, probe_3.sh2_b.xyz, probe_3.sh2_c.xyz, probe_3.sh2_d.xyz, probe_3.sh2_e.xyz, probe_3.sh2_f.xyz, probe_3.sh2_g.xyz, probe_3.sh2_h.xyz, probe_3.sh2_i.xyz, normal_ws_3);
    return (_e27 * max(probe_3.params.x, 0f));
}

fn sample_probe_sh2_or_ambientX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_3: u32, normal_ws_4: vec3<f32>) -> vec3<f32> {
    let _e1: bool = probe_has_any_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_3);
    if _e1 {
        let _e3: vec3<f32> = sample_probe_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_3, normal_ws_4);
        return _e3;
    }
    let _e4: vec3<f32> = ambient_probe_or_zeroX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(normal_ws_4);
    return _e4;
}

fn finalize_diffuse_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total_1: vec4<f32>, importance_1: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, fallback_index: u32, normal_ws_5: vec3<f32>) -> vec3<f32> {
    let _e2: vec4<f32> = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total_1, importance_1);
    let remaining_weight: f32 = (1f - _e2.w);
    if (remaining_weight < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
        return _e2.xyz;
    }
    let _e12: vec3<f32> = sample_probe_sh2_or_ambientX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(fallback_index, normal_ws_5);
    return (_e2.xyz + (remaining_weight * _e12));
}

fn distance_from_aabbX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_21: vec3<f32>, aabb_min: vec3<f32>, aabb_max: vec3<f32>) -> vec3<f32> {
    return max(max((world_pos_21 - aabb_max), (aabb_min - world_pos_21)), vec3(0f));
}

fn probe_blend_distanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) -> f32 {
    return max(probe.box_min.w, 0f);
}

fn probe_edge_weightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_1: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_22: vec3<f32>) -> f32 {
    let _e6: vec3<f32> = distance_from_aabbX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_22, probe_1.box_min.xyz, probe_1.box_max.xyz);
    let outside_distance: f32 = length(_e6);
    let _e8: f32 = probe_blend_distanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_1);
    if (_e8 <= MIN_PROBE_BLEND_DISTANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
        return select(1f, 0f, (outside_distance > 0f));
    }
    return clamp((1f - (outside_distance / _e8)), 0f, 1f);
}

fn add_diffuse_sh2_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(importance_2: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, index: u32, world_pos_23: vec3<f32>, normal_ws_6: vec3<f32>) -> ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX {
    let probe_4: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[index];
    let _e5: f32 = probe_edge_weightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_4, world_pos_23);
    if (_e5 < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
        return importance_2;
    }
    let _e10: vec3<f32> = sample_probe_sh2_or_ambientX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(index, normal_ws_6);
    return ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX((importance_2.weighted_result + (_e5 * _e10)), (importance_2.weight_sum + _e5), max(importance_2.coverage, _e5));
}

fn indirect_diffuseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_24: vec3<f32>, normal_ws_7: vec3<f32>, view_layer_15: u32, enabled_8: bool) -> vec3<f32> {
    var total_2: vec4<f32> = vec4(0f);
    var importance_3: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX;
    var i_1: u32 = 0u;

    if !(enabled_8) {
        return vec3(0f);
    }
    let _e8: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_15);
    let _e9: bool = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    if !(_e9) {
        let _e12: vec3<f32> = ambient_probe_or_zeroX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(normal_ws_7);
        return _e12;
    }
    let _e13: u32 = reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    let _e14: vec4<u32> = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    let _e15: u32 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    let _e16: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
    importance_3 = _e16;
    loop {
        let _e19: u32 = i_1;
        if (_e19 < 4u) {
        } else {
            break;
        }
        {
            let _e22: u32 = i_1;
            let index_2: u32 = _e14[_e22];
            if (index_2 == 0u) {
                let _e27: vec4<f32> = total_2;
                let _e28: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_3;
                let _e29: vec3<f32> = finalize_diffuse_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e27, _e28, _e15, normal_ws_7);
                return _e29;
            }
            let _e31: u32 = i_1;
            if ((_e13 & (1u << _e31)) > 0u) {
                let _e36: vec4<f32> = total_2;
                let _e37: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_3;
                let _e38: vec4<f32> = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e36, _e37);
                total_2 = _e38;
                let _e40: f32 = total_2.w;
                if ((1f - _e40) <= MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
                    let _e45: vec4<f32> = total_2;
                    return _e45.xyz;
                }
                let _e47: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
                importance_3 = _e47;
            }
            let _e48: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_3;
            let _e50: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = add_diffuse_sh2_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e48, index_2, world_pos_24, normal_ws_7);
            importance_3 = _e50;
        }
        continuing {
            let _e52: u32 = i_1;
            i_1 = (_e52 + 1u);
        }
    }
    let _e54: vec4<f32> = total_2;
    let _e55: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_3;
    let _e56: vec3<f32> = finalize_diffuse_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e54, _e55, _e15, normal_ws_7);
    return _e56;
}

fn dominant_reflection_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_16: vec3<f32>, v_9: vec3<f32>, perceptual_roughness_9: f32) -> vec3<f32> {
    let r: vec3<f32> = reflect(-(v_9), n_16);
    let roughness_6: f32 = clamp(perceptual_roughness_9, 0f, 1f);
    let blend: f32 = (roughness_6 * roughness_6);
    return normalize(mix(r, n_16, blend));
}

fn box_project_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_2: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX, world_pos_25: vec3<f32>, dir_4: vec3<f32>, perceptual_roughness_10: f32) -> vec3<f32> {
    if (probe_2.params.z < PROBE_FLAG_BOX_PROJECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
        return dir_4;
    }
    let projection_weight: f32 = (1f - smoothstep(BOX_PROJECTION_FULL_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, BOX_PROJECTION_OFF_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, clamp(perceptual_roughness_10, 0f, 1f)));
    if (projection_weight <= 0f) {
        return dir_4;
    }
    let _e17: f32 = probe_blend_distanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_2);
    let box_min: vec3<f32> = (probe_2.box_min.xyz - vec3(_e17));
    let box_max: vec3<f32> = (probe_2.box_max.xyz + vec3(_e17));
    let safe_dir: vec3<f32> = select(vec3(0.000001f), dir_4, (abs(dir_4) > vec3(0.000001f)));
    let plane: vec3<f32> = select(box_min, box_max, (safe_dir > vec3(0f)));
    let t_15: vec3<f32> = ((plane - world_pos_25) / safe_dir);
    let distance_: f32 = min(t_15.x, min(t_15.y, t_15.z));
    if (distance_ <= 0f) {
        return dir_4;
    }
    let projected: vec3<f32> = normalize(((world_pos_25 + (safe_dir * distance_)) - probe_2.position.xyz));
    return normalize(mix(dir_4, projected, projection_weight));
}

fn roughness_lodX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(perceptual_roughness_11: f32, max_lod_1: f32) -> f32 {
    let r_1: f32 = clamp(perceptual_roughness_11, 0f, 1f);
    return clamp(((max_lod_1 * r_1) * (2f - r_1)), 0f, max_lod_1);
}

fn sample_probe_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_4: u32, world_pos_26: vec3<f32>, dir_5: vec3<f32>, perceptual_roughness_12: f32) -> vec3<f32> {
    if (atlas_index_4 == 0u) {
        return vec3(0f);
    }
    let probe_5: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[atlas_index_4];
    let intensity: f32 = max(probe_5.params.x, 0f);
    if (intensity <= 0f) {
        return vec3(0f);
    }
    let _e19: vec3<f32> = box_project_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_5, world_pos_26, dir_5, perceptual_roughness_12);
    let _e24: f32 = roughness_lodX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(perceptual_roughness_12, max(probe_5.params.y, 0f));
    let _e27: vec2<u32> = textureDimensions(reflection_probe_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, 0i);
    let _e36: vec3<f32> = sample_trilinear_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(reflection_probe_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e19, _e24, _e27.x, max(probe_5.params.y, 0f), (atlas_index_4 * 6u));
    return (_e36 * intensity);
}

fn finalize_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total_3: vec4<f32>, importance_4: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, fallback_index_1: u32, world_pos_27: vec3<f32>, dir_6: vec3<f32>, perceptual_roughness_13: f32) -> vec3<f32> {
    let _e2: vec4<f32> = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total_3, importance_4);
    let remaining_weight_1: f32 = (1f - _e2.w);
    if (remaining_weight_1 < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
        return _e2.xyz;
    }
    let _e14: vec3<f32> = sample_probe_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(fallback_index_1, world_pos_27, dir_6, perceptual_roughness_13);
    return (_e2.xyz + (remaining_weight_1 * _e14));
}

fn add_radiance_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(importance_5: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, index_1: u32, world_pos_28: vec3<f32>, dir_7: vec3<f32>, perceptual_roughness_14: f32) -> ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX {
    let probe_6: GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[index_1];
    let _e5: f32 = probe_edge_weightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_6, world_pos_28);
    if (_e5 < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
        return importance_5;
    }
    let _e11: vec3<f32> = sample_probe_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(index_1, world_pos_28, dir_7, perceptual_roughness_14);
    return ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX((importance_5.weighted_result + (_e5 * _e11)), (importance_5.weight_sum + _e5), max(importance_5.coverage, _e5));
}

fn indirect_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_29: vec3<f32>, n_17: vec3<f32>, v_10: vec3<f32>, perceptual_roughness_15: f32, view_layer_16: u32, enabled_9: bool) -> vec3<f32> {
    var total_4: vec4<f32> = vec4(0f);
    var importance_6: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX;
    var i_2: u32 = 0u;

    if !(enabled_9) {
        return vec3(0f);
    }
    let _e8: PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_16);
    let _e9: bool = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    if !(_e9) {
        return vec3(0f);
    }
    let _e13: u32 = reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    let _e17: vec3<f32> = dominant_reflection_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_17, v_10, perceptual_roughness_15);
    let _e18: vec4<u32> = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    let _e19: u32 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e8);
    let _e20: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
    importance_6 = _e20;
    loop {
        let _e23: u32 = i_2;
        if (_e23 < 4u) {
        } else {
            break;
        }
        {
            let _e26: u32 = i_2;
            let index_3: u32 = _e18[_e26];
            if (index_3 == 0u) {
                let _e31: vec4<f32> = total_4;
                let _e32: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_6;
                let _e34: vec3<f32> = finalize_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e31, _e32, _e19, world_pos_29, _e17, perceptual_roughness_15);
                return _e34;
            }
            let _e36: u32 = i_2;
            if ((_e13 & (1u << _e36)) > 0u) {
                let _e41: vec4<f32> = total_4;
                let _e42: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_6;
                let _e43: vec4<f32> = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e41, _e42);
                total_4 = _e43;
                let _e45: f32 = total_4.w;
                if ((1f - _e45) <= MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX) {
                    let _e50: vec4<f32> = total_4;
                    return _e50.xyz;
                }
                let _e52: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
                importance_6 = _e52;
            }
            let _e53: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_6;
            let _e54: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = add_radiance_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e53, index_3, world_pos_29, _e17, perceptual_roughness_15);
            importance_6 = _e54;
        }
        continuing {
            let _e56: u32 = i_2;
            i_2 = (_e56 + 1u);
        }
    }
    let _e58: vec4<f32> = total_4;
    let _e59: ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = importance_6;
    let _e60: vec3<f32> = finalize_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e58, _e59, _e19, world_pos_29, _e17, perceptual_roughness_15);
    return _e60;
}

fn horizon_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_18: vec3<f32>, geometric_n: vec3<f32>) -> vec3<f32> {
    if (dot(geometric_n, geometric_n) <= 0.000000000001f) {
        return normalize(n_18);
    }
    return normalize(geometric_n);
}

fn horizon_specular_occlusionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_19: vec3<f32>, geometric_n_1: vec3<f32>, v_11: vec3<f32>, perceptual_roughness_16: f32) -> f32 {
    let _e3: vec3<f32> = dominant_reflection_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_19, v_11, perceptual_roughness_16);
    let _e5: vec3<f32> = horizon_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_19, geometric_n_1);
    let horizon: f32 = clamp((1f + dot(_e3, _e5)), 0f, 1f);
    return (horizon * horizon);
}

fn indirect_specular_with_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_30: vec3<f32>, n_20: vec3<f32>, geometric_n_2: vec3<f32>, v_12: vec3<f32>, perceptual_roughness_17: f32, specular_energy_3: vec3<f32>, specular_occlusion: f32, enabled_10: bool, view_layer_17: u32) -> vec3<f32> {
    let _e6: vec3<f32> = indirect_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_30, n_20, v_12, perceptual_roughness_17, view_layer_17, enabled_10);
    let _e8: f32 = horizon_specular_occlusionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_20, geometric_n_2, v_12, perceptual_roughness_17);
    return (((_e6 * specular_energy_3) * clamp(specular_occlusion, 0f, 1f)) * _e8);
}

fn direct_energy_compensationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(perceptual_roughness_18: f32, n_dot_v_5: f32, f0_11: vec3<f32>, enabled_11: bool) -> vec3<f32> {
    if !(enabled_11) {
        return vec3(1f);
    }
    let _e5: f32 = direct_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_18);
    let _e7: vec2<f32> = sample_ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e5, n_dot_v_5);
    let _e9: vec3<f32> = energy_compensation_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e7, f0_11);
    return _e9;
}

fn cluster_id_for_fragmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_1: vec2<f32>, world_pos_31: vec3<f32>, view_layer_18: u32) -> u32 {
    let _e2: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_space_z_coeffs;
    let _e5: vec4<f32> = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_space_z_coeffs_right;
    let _e8: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_width;
    let _e11: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_height;
    let _e14: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.cluster_count_x;
    let _e17: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.cluster_count_y;
    let _e20: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.cluster_count_z;
    let _e23: f32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.near_clip;
    let _e26: f32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.far_clip;
    let _e30: u32 = cluster_id_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(frag_xy_1, world_pos_31, _e2, _e5, view_layer_18, _e8, _e11, _e14, _e17, _e20, _e23, _e26);
    return _e30;
}

fn light_enabled_for_optionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(light_type: u32, options: ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX) -> bool {
    var local_29: bool;
    var local_30: bool;
    var local_31: bool;

    let is_directional: bool = (light_type == 1u);
    if is_directional {
        local_29 = !(options.include_directional);
    } else {
        local_29 = false;
    }
    let _e9: bool = local_29;
    if !(_e9) {
        if !(is_directional) {
            local_31 = !(options.include_local);
        } else {
            local_31 = false;
        }
        let _e17: bool = local_31;
        local_30 = _e17;
    } else {
        local_30 = true;
    }
    let _e21: bool = local_30;
    return !(_e21);
}

fn direct_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_2: vec2<f32>, world_pos_32: vec3<f32>, view_layer_19: u32, s_2: MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX, view_dir: vec3<f32>, filtered_roughness: f32, specular_color: vec3<f32>, energy_compensation_3: vec3<f32>, options_1: ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX) -> vec3<f32> {
    var basis_4: DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 0f, 1f));
    var alpha_6: f32 = 0f;
    var f90_5: f32 = 0f;
    var lo: vec3<f32> = vec3(0f);
    var i_3: u32 = 0u;

    let _e9: u32 = cluster_id_for_fragmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_2, world_pos_32, view_layer_19);
    let _e10: vec2<u32> = cluster_light_range_atX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(_e9);
    let count_3: u32 = _e10.y;
    if (count_3 == 0u) {
        return vec3(0f);
    }
    if options_1.specular_highlights_enabled {
        let _e21: DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = direct_brdf_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_2.normal, view_dir);
        basis_4 = _e21;
        let _e24: f32 = direct_alpha_from_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(filtered_roughness);
        alpha_6 = _e24;
        let _e27: f32 = f90_from_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(specular_color);
        f90_5 = _e27;
    }
    loop {
        let _e30: u32 = i_3;
        if (_e30 < count_3) {
        } else {
            break;
        }
        {
            let _e32: u32 = i_3;
            let _e33: u32 = cluster_light_index_from_rangeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(_e10, _e32);
            let _e36: u32 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.light_count;
            if (_e33 >= _e36) {
                continue;
            }
            let light_26: GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX[_e33];
            let _e42: bool = light_enabled_for_optionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(light_26.light_type, options_1);
            if !(_e42) {
                continue;
            }
            if options_1.specular_highlights_enabled {
                let _e46: vec3<f32> = lo;
                let _e48: DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = basis_4;
                let _e49: f32 = alpha_6;
                let _e50: f32 = f90_5;
                let _e55: vec3<f32> = direct_radiance_metallic_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_26, world_pos_32, s_2.normal, _e48, _e49, _e50, s_2.roughness, s_2.metallic, s_2.base_color, specular_color, energy_compensation_3);
                lo = (_e46 + _e55);
            } else {
                let _e57: vec3<f32> = lo;
                let _e62: vec3<f32> = diffuse_only_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_26, world_pos_32, s_2.normal, view_dir, s_2.roughness, s_2.base_color, s_2.metallic);
                lo = (_e57 + _e62);
            }
        }
        continuing {
            let _e65: u32 = i_3;
            i_3 = (_e65 + 1u);
        }
    }
    let _e67: vec3<f32> = lo;
    return _e67;
}

fn indirect_specular_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(perceptual_roughness_19: f32, n_dot_v_6: f32, f0_12: vec3<f32>, enabled_12: bool) -> vec3<f32> {
    if !(enabled_12) {
        return vec3(0f);
    }
    let _e6: vec2<f32> = sample_ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_19, n_dot_v_6);
    let _e9: vec3<f32> = indirect_specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e6, f0_12, true);
    return _e9;
}

fn indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(n_dot_v_7: f32, occlusion_3: f32, perceptual_roughness_20: f32, f0_13: vec3<f32>, enabled_13: bool) -> vec3<f32> {
    if !(enabled_13) {
        return vec3(0f);
    }
    let _e8: vec3<f32> = indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_7, occlusion_3, perceptual_roughness_20, f0_13);
    return _e8;
}

fn indirect_specular_contributionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(world_pos_33: vec3<f32>, normal_1: vec3<f32>, geometric_normal_2: vec3<f32>, view_dir_1: vec3<f32>, perceptual_roughness_21: f32, specular_energy_4: vec3<f32>, specular_visibility: vec3<f32>, enabled_14: bool, view_layer_20: u32) -> vec3<f32> {
    if !(enabled_14) {
        return vec3(0f);
    }
    let _e15: vec3<f32> = indirect_specular_with_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_33, normal_1, geometric_normal_2, view_dir_1, perceptual_roughness_21, (specular_energy_4 * specular_visibility), 1f, true, view_layer_20);
    return _e15;
}

fn shade_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_3: vec2<f32>, world_pos_34: vec3<f32>, view_layer_21: u32, s_3: MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX, options_2: ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX) -> vec3<f32> {
    let _e2: vec3<f32> = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_34, view_layer_21);
    let _e5: vec3<f32> = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_3.normal, _e2);
    let _e8: vec3<f32> = metallic_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_3.base_color, s_3.metallic);
    let n_dot_v_12: f32 = clamp(dot(_e5, _e2), 0f, 1f);
    let _e16: f32 = filter_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_3.roughness, s_3.normal, s_3.geometric_normal);
    let _e20: vec3<f32> = direct_energy_compensationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_3.roughness, n_dot_v_12, _e8, options_2.specular_highlights_enabled);
    let _e22: vec3<f32> = direct_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_3, world_pos_34, view_layer_21, s_3, _e2, _e16, _e8, _e20, options_2);
    let _e24: bool = has_indirect_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_21, options_2.glossy_reflections_enabled);
    let _e25: vec3<f32> = indirect_specular_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(_e16, n_dot_v_12, _e8, _e24);
    let _e27: vec3<f32> = indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(n_dot_v_12, s_3.occlusion, _e16, _e8, _e24);
    let _e30: vec3<f32> = indirect_diffuseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_34, s_3.normal, view_layer_21, options_2.include_directional);
    let _e34: vec3<f32> = indirect_diffuse_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e30, s_3.base_color, s_3.metallic, _e25, s_3.occlusion, _e24);
    let _e36: vec3<f32> = indirect_specular_contributionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(world_pos_34, _e5, s_3.geometric_normal, _e2, _e16, _e25, _e27, _e24, view_layer_21);
    let extra: vec3<f32> = select(vec3(0f), s_3.emission, options_2.include_directional);
    return (((_e34 + _e36) + _e22) + extra);
}

fn premultiplied_metallic_surfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_4: MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX) -> MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX {
    let diffuse_alpha_1: f32 = clamp(s_4.alpha, 0f, 1f);
    return MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX((s_4.base_color * diffuse_alpha_1), s_4.alpha, s_4.metallic, s_4.roughness, s_4.occlusion, s_4.normal, s_4.geometric_normal, s_4.emission);
}

fn shade_metallic_transparent_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_4: vec2<f32>, world_pos_35: vec3<f32>, view_layer_22: u32, s_5: MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX, options_3: ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX) -> vec4<f32> {
    let _e1: MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX = premultiplied_metallic_surfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_5);
    let _e4: vec3<f32> = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_35, view_layer_22);
    let _e6: vec3<f32> = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5.normal, _e4);
    let _e9: vec3<f32> = metallic_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5.base_color, s_5.metallic);
    let n_dot_v_13: f32 = clamp(dot(_e6, _e4), 0f, 1f);
    let _e17: f32 = filter_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5.roughness, s_5.normal, s_5.geometric_normal);
    let _e21: vec3<f32> = direct_energy_compensationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_5.roughness, n_dot_v_13, _e9, options_3.specular_highlights_enabled);
    let _e23: vec3<f32> = direct_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_4, world_pos_35, view_layer_22, _e1, _e4, _e17, _e9, _e21, options_3);
    let _e25: bool = has_indirect_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_22, options_3.glossy_reflections_enabled);
    let _e26: vec3<f32> = indirect_specular_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(_e17, n_dot_v_13, _e9, _e25);
    let _e28: vec3<f32> = indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(n_dot_v_13, s_5.occlusion, _e17, _e9, _e25);
    let _e31: vec3<f32> = indirect_diffuseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_35, s_5.normal, view_layer_22, options_3.include_directional);
    let _e35: vec3<f32> = indirect_diffuse_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e31, _e1.base_color, s_5.metallic, _e26, s_5.occlusion, _e25);
    let _e37: vec3<f32> = indirect_specular_contributionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(world_pos_35, _e6, s_5.geometric_normal, _e4, _e17, _e26, _e28, _e25, view_layer_22);
    let extra_1: vec3<f32> = select(vec3(0f), s_5.emission, options_3.include_directional);
    let _e44: f32 = metallic_one_minus_reflectivityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5.metallic);
    let _e49: f32 = unity_premultiplied_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5.alpha, _e44);
    return vec4<f32>((((_e35 + _e37) + _e23) + extra_1), _e49);
}

fn pbs_kw(mask_1: u32) -> bool {
    let _e2: u32 = mat._RenderideVariantBits;
    let _e4: bool = enabledX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(_e2, mask_1);
    return _e4;
}

fn alpha_test_enabled() -> bool {
    let _e1: bool = pbs_kw(PBSMETALLIC_KW_ALPHATEST_ON);
    return _e1;
}

fn alpha_premultiply_enabled() -> bool {
    let _e1: bool = pbs_kw(PBSMETALLIC_KW_ALPHAPREMULTIPLY_ON);
    return _e1;
}

fn specular_highlights_enabled() -> bool {
    let _e1: bool = pbs_kw(PBSMETALLIC_KW_SPECULARHIGHLIGHTS_OFF);
    return !(_e1);
}

fn glossy_reflections_enabled_2() -> bool {
    let _e1: bool = pbs_kw(PBSMETALLIC_KW_GLOSSYREFLECTIONS_OFF);
    return !(_e1);
}

fn metallic_gloss_map_enabled() -> bool {
    let _e1: bool = pbs_kw(PBSMETALLIC_KW_METALLICGLOSSMAP);
    return _e1;
}

fn smoothness_from_albedo_alpha_1() -> bool {
    let _e1: bool = pbs_kw(PBSMETALLIC_KW_SMOOTHNESS_TEXTURE_ALBEDO_CHANNEL_A);
    return _e1;
}

fn sample_surface(uv0_3: vec2<f32>, uv1_3: vec2<f32>, world_pos_36: vec3<f32>, world_n_6: vec3<f32>, world_t_6: vec4<f32>, view_layer_23: u32) -> SurfaceData {
    var base_color_9: vec3<f32>;
    var metallic_5: f32;
    var smoothness_2: f32;

    let _e2: vec4<f32> = mat._MainTex_ST;
    let _e4: vec2<f32> = apply_stX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HK5QX(uv0_3, _e2);
    let _e6: bool = pbs_kw(PBSMETALLIC_KW_PARALLAXMAP);
    let _e9: f32 = mat._Parallax;
    let _e12: f32 = mat._ParallaxMap_LodBias;
    let _e19: vec2<f32> = apply_parallaxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e4, _e6, _e9, world_pos_36, world_n_6, world_t_6, view_layer_23, _ParallaxMap, mat._ParallaxMap_WrapModeBits, _ParallaxMap_sampler, _e12);
    let _e22: f32 = mat._UVSec;
    let _e25: vec4<f32> = mat._DetailAlbedoMap_ST;
    let _e27: vec2<f32> = detail_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(uv0_3, uv1_3, _e22, _e25);
    let _e30: f32 = mat._MainTex_LodBias;
    let _e33: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(_MainTex, mat._MainTex_WrapModeBits, _MainTex_sampler, _e19, _e30);
    let _e37: f32 = mat._Color.w;
    let _e39: bool = smoothness_from_albedo_alpha_1();
    let _e40: f32 = standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e37, _e33.w, _e39);
    let _e43: f32 = mat._Cutoff;
    let _e44: bool = alpha_test_enabled();
    clip_standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e40, _e43, _e44);
    let _e47: vec4<f32> = mat._Color;
    base_color_9 = (_e47.xyz * _e33.xyz);
    let _e54: f32 = mat._Metallic;
    metallic_5 = _e54;
    let _e58: f32 = mat._Glossiness;
    smoothness_2 = _e58;
    let smoothness_scale: f32 = mat._GlossMapScale;
    let _e63: bool = metallic_gloss_map_enabled();
    if _e63 {
        let _e66: f32 = mat._MetallicGlossMap_LodBias;
        let _e69: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(_MetallicGlossMap, mat._MetallicGlossMap_WrapModeBits, _MetallicGlossMap_sampler, _e19, _e66);
        metallic_5 = _e69.x;
        smoothness_2 = (_e69.w * smoothness_scale);
    }
    let _e73: bool = smoothness_from_albedo_alpha_1();
    if _e73 {
        smoothness_2 = (_e33.w * smoothness_scale);
    }
    let _e76: f32 = metallic_5;
    metallic_5 = clamp(_e76, 0f, 1f);
    let _e80: f32 = smoothness_2;
    let _e81: f32 = roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e80);
    let _e84: f32 = mat._OcclusionMap_LodBias;
    let _e87: vec4<f32> = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(_OcclusionMap, mat._OcclusionMap_WrapModeBits, _OcclusionMap_sampler, _e19, _e84);
    let occlusion_sample: f32 = _e87.y;
    let _e91: f32 = mat._OcclusionStrength;
    let _e92: f32 = occlusion_from_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(occlusion_sample, _e91);
    let _e94: bool = pbs_kw(PBSMETALLIC_KW_DETAIL_MULX2_);
    let _e97: f32 = mat._DetailMask_LodBias;
    let _e100: f32 = sample_detail_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(_e94, _DetailMask, mat._DetailMask_WrapModeBits, _DetailMask_sampler, _e19, _e97);
    let _e101: vec3<f32> = base_color_9;
    let _e104: f32 = mat._DetailAlbedoMap_LodBias;
    let _e107: vec3<f32> = apply_detail_albedoX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(_e101, _e94, _e100, _DetailAlbedoMap, mat._DetailAlbedoMap_WrapModeBits, _DetailAlbedoMap_sampler, _e27, _e104);
    base_color_9 = _e107;
    let _e109: bool = pbs_kw(PBSMETALLIC_KW_NORMALMAP);
    let _e112: f32 = mat._BumpMap_LodBias;
    let _e115: f32 = mat._BumpScale;
    let _e118: f32 = mat._DetailNormalMap_LodBias;
    let _e121: f32 = mat._DetailNormalMapScale;
    let _e127: vec3<f32> = sample_world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e109, _e94, _e100, _BumpMap, mat._BumpMap_WrapModeBits, _BumpMap_sampler, _e19, _e112, _e115, _DetailNormalMap, mat._DetailNormalMap_WrapModeBits, _DetailNormalMap_sampler, _e27, _e118, _e121, world_n_6, world_t_6, normalize(world_n_6));
    let _e129: bool = pbs_kw(PBSMETALLIC_KW_EMISSION);
    let _e132: vec4<f32> = mat._EmissionColor;
    let _e136: f32 = mat._EmissionMap_LodBias;
    let _e139: vec3<f32> = sample_emissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e129, _e132.xyz, _EmissionMap, mat._EmissionMap_WrapModeBits, _EmissionMap_sampler, _e19, _e136);
    let _e140: vec3<f32> = base_color_9;
    let _e141: f32 = metallic_5;
    return SurfaceData(_e140, _e40, _e141, _e81, _e92, _e127, _e139);
}

@vertex 
fn vs_main(@builtin(instance_index) instance_index: u32, @location(0) pos: vec4<f32>, @location(1) n: vec4<f32>, @location(2) uv0_: vec2<f32>, @location(4) t: vec4<f32>, @location(5) uv1_: vec2<f32>) -> WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX {
    let _e7: WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX = world_uv2_vertex_mainX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(instance_index, 0u, pos, n, t, uv0_, uv1_);
    return _e7;
}

@fragment 
fn fs_forward_base(@builtin(position) frag_pos: vec4<f32>, @location(0) world_pos: vec3<f32>, @location(1) world_n: vec3<f32>, @location(2) world_t: vec4<f32>, @location(3) uv0_1: vec2<f32>, @location(4) uv1_1: vec2<f32>, @location(5) @interpolate(flat) view_layer: u32) -> @location(0) vec4<f32> {
    let _e6: SurfaceData = sample_surface(uv0_1, uv1_1, world_pos, world_n, world_t, view_layer);
    let _e14: MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX = metallic_with_geometric_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(_e6.base_color, _e6.alpha, _e6.metallic, _e6.roughness, _e6.occlusion, _e6.normal, world_n, _e6.emission);
    let _e15: bool = specular_highlights_enabled();
    let _e16: bool = glossy_reflections_enabled_2();
    let options_4: ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX = ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(true, true, _e15, _e16);
    let _e20: bool = alpha_premultiply_enabled();
    if _e20 {
        let _e23: vec4<f32> = shade_metallic_transparent_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_pos.xy, world_pos, view_layer, _e14, options_4);
        return _e23;
    }
    let _e25: vec3<f32> = shade_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_pos.xy, world_pos, view_layer, _e14, options_4);
    return vec4<f32>(_e25, _e6.alpha);
}

const RENDERIDE_WRAP_MODE_MIRROR_ONCE_U: u32 = 1u;
const RENDERIDE_WRAP_MODE_MIRROR_ONCE_V: u32 = 2u;
const RENDERIDE_WRAP_MODE_MIRROR_ONCE_W: u32 = 4u;

struct RenderideWrappedGrad2D {
    uv: vec2<f32>,
    ddx_uv: vec2<f32>,
    ddy_uv: vec2<f32>,
}

fn renderide_mirror_once_coord(coord: f32) -> f32 {
    return clamp(abs(coord), 0.0, 1.0);
}

fn renderide_mirror_once_grad_scale(coord: f32) -> f32 {
    if (coord < -1.0 || coord > 1.0) {
        return 0.0;
    }
    if (coord < 0.0) {
        return -1.0;
    }
    return 1.0;
}

fn renderide_mirror_once_2d(uv: vec2<f32>, wrap_mode_bits: u32) -> vec2<f32> {
    var wrapped = uv;
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_U) != 0u) {
        wrapped.x = renderide_mirror_once_coord(wrapped.x);
    }
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_V) != 0u) {
        wrapped.y = renderide_mirror_once_coord(wrapped.y);
    }
    return wrapped;
}

fn renderide_mirror_once_3d(uvw: vec3<f32>, wrap_mode_bits: u32) -> vec3<f32> {
    var wrapped = uvw;
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_U) != 0u) {
        wrapped.x = renderide_mirror_once_coord(wrapped.x);
    }
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_V) != 0u) {
        wrapped.y = renderide_mirror_once_coord(wrapped.y);
    }
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_W) != 0u) {
        wrapped.z = renderide_mirror_once_coord(wrapped.z);
    }
    return wrapped;
}

fn renderide_mirror_once_grad_2d(
    uv: vec2<f32>,
    ddx_uv: vec2<f32>,
    ddy_uv: vec2<f32>,
    wrap_mode_bits: u32,
) -> RenderideWrappedGrad2D {
    var wrapped = RenderideWrappedGrad2D(uv, ddx_uv, ddy_uv);
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_U) != 0u) {
        let scale_x = renderide_mirror_once_grad_scale(uv.x);
        wrapped.uv.x = renderide_mirror_once_coord(uv.x);
        wrapped.ddx_uv.x = ddx_uv.x * scale_x;
        wrapped.ddy_uv.x = ddy_uv.x * scale_x;
    }
    if ((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_V) != 0u) {
        let scale_y = renderide_mirror_once_grad_scale(uv.y);
        wrapped.uv.y = renderide_mirror_once_coord(uv.y);
        wrapped.ddx_uv.y = ddx_uv.y * scale_y;
        wrapped.ddy_uv.y = ddy_uv.y * scale_y;
    }
    return wrapped;
}

fn renderide_mirroronce_sample_2d(
    tex: texture_2d<f32>,
    samp: sampler,
    uv: vec2<f32>,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    return textureSample(tex, samp, renderide_mirror_once_2d(uv, wrap_mode_bits));
}

fn renderide_mirroronce_sample_bias_2d(
    tex: texture_2d<f32>,
    samp: sampler,
    uv: vec2<f32>,
    bias: f32,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    return textureSampleBias(tex, samp, renderide_mirror_once_2d(uv, wrap_mode_bits), bias);
}

fn renderide_mirroronce_sample_level_2d(
    tex: texture_2d<f32>,
    samp: sampler,
    uv: vec2<f32>,
    level: f32,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    return textureSampleLevel(tex, samp, renderide_mirror_once_2d(uv, wrap_mode_bits), level);
}

fn renderide_mirroronce_sample_grad_2d(
    tex: texture_2d<f32>,
    samp: sampler,
    uv: vec2<f32>,
    ddx_uv: vec2<f32>,
    ddy_uv: vec2<f32>,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    let wrapped = renderide_mirror_once_grad_2d(uv, ddx_uv, ddy_uv, wrap_mode_bits);
    return textureSampleGrad(tex, samp, wrapped.uv, wrapped.ddx_uv, wrapped.ddy_uv);
}

fn renderide_mirroronce_sample_3d(
    tex: texture_3d<f32>,
    samp: sampler,
    uvw: vec3<f32>,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    return textureSample(tex, samp, renderide_mirror_once_3d(uvw, wrap_mode_bits));
}

fn renderide_mirroronce_sample_bias_3d(
    tex: texture_3d<f32>,
    samp: sampler,
    uvw: vec3<f32>,
    bias: f32,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    return textureSampleBias(tex, samp, renderide_mirror_once_3d(uvw, wrap_mode_bits), bias);
}

fn renderide_mirroronce_sample_level_3d(
    tex: texture_3d<f32>,
    samp: sampler,
    uvw: vec3<f32>,
    level: f32,
    wrap_mode_bits: u32,
) -> vec4<f32> {
    return textureSampleLevel(tex, samp, renderide_mirror_once_3d(uvw, wrap_mode_bits), level);
}
