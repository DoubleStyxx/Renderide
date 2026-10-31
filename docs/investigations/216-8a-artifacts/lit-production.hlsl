struct GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    float3 position;
    float align_pad_vec3_pos;
    float3 direction;
    float align_pad_vec3_dir;
    float3 color;
    float intensity;
    float range;
    float spot_cos_half_angle;
    uint light_type;
    float spot_angle_scale;
    float shadow_strength;
    float shadow_near_plane;
    float shadow_bias;
    float shadow_normal_bias;
    uint shadow_type;
    uint shadow_view_start;
    uint shadow_view_count;
    uint shadow_flags;
    uint shadow_map_resolution;
    uint cookie_kind;
    uint cookie_layer;
    uint cookie_reserved;
    float4 cookie_right_tan_half_angle;
    float4 cookie_up;
};

struct GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    row_major float4x4 world_to_shadow;
    float4 atlas_rect;
    float4 params;
    float4 light_params;
};

struct GpuLightCookieRectX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    float4 origin_scale;
};

struct GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    float4 box_min;
    float4 box_max;
    float4 position;
    float4 params;
    float4 sh2_a;
    float4 sh2_b;
    float4 sh2_c;
    float4 sh2_d;
    float4 sh2_e;
    float4 sh2_f;
    float4 sh2_g;
    float4 sh2_h;
    float4 sh2_i;
};

struct FrameGlobalsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX {
    float4 camera_world_pos;
    float4 camera_world_pos_right;
    float4 view_space_z_coeffs;
    float4 view_space_z_coeffs_right;
    float4 view_to_world_y_coeffs;
    float4 view_to_world_y_coeffs_right;
    uint cluster_count_x;
    uint cluster_count_y;
    uint cluster_count_z;
    float near_clip;
    float far_clip;
    uint light_count;
    uint viewport_width;
    uint viewport_height;
    float4 proj_params_left;
    float4 proj_params_right;
    row_major float4x4 proj_left;
    row_major float4x4 proj_right;
    uint4 frame_tail;
    float4 skybox_specular;
    float4 frame_time;
    float4 ambient_sh_a;
    float4 ambient_sh_b;
    float4 ambient_sh_c;
    float4 ambient_sh_d;
    float4 ambient_sh_e;
    float4 ambient_sh_f;
    float4 ambient_sh_g;
    float4 ambient_sh_h;
    float4 ambient_sh_i;
    float4 fog_color_mode;
    float4 fog_params;
    float4 view_space_x_coeffs;
    float4 view_space_x_coeffs_right;
    float4 view_space_y_coeffs;
    float4 view_space_y_coeffs_right;
};

struct PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX {
    row_major float4x4 view_proj_left;
    row_major float4x4 view_proj_right;
    row_major float4x4 model;
    row_major float3x3 normal_matrix;
    int _pad4_0;
    float4 _pad;
    float4 particle_header;
    float4 particle_tint;
    float4 particle_extra;
    float4 particle_frame_grid;
    float4 particle_padding[12];
};

struct BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX {
    float3 right;
    int _pad1_0;
    float3 up;
    int _end_pad_0;
};

struct MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX {
    float3 right;
    int _pad1_0;
    float3 up;
    int _pad2_0;
    float3 forward;
    int _end_pad_0;
};

struct WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX {
    float4 clip_pos : SV_Position;
    float3 world_pos : LOC0;
    float3 world_n : LOC1;
    float4 world_t : LOC2;
    float2 primary_uv : LOC3;
    float2 secondary_uv : LOC4;
    nointerpolation uint view_layer : LOC5;
};

struct MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX {
    float3 base_color;
    float alpha;
    float metallic;
    float roughness;
    float occlusion;
    int _pad5_0;
    float3 normal;
    int _pad6_0;
    float3 geometric_normal;
    int _pad7_0;
    float3 emission;
    int _end_pad_0;
};

struct ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX {
    float near_clip;
    float far_clip;
};

struct PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX {
    uint face;
    int _pad1_0;
    float2 local_uv;
};

struct PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX {
    float3 direction;
    int _pad1_0;
    float3 up;
    int _pad2_0;
    float3 right;
    int _end_pad_0;
};

struct LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX {
    float3 l;
    float attenuation;
};

struct DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX {
    float3 view;
    int _pad1_0;
    float3 n;
    int _end_pad_0;
};

struct DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX {
    float3 f;
    int _pad1_0;
    float3 specular_brdf;
    float n_dot_l;
    float n_dot_v;
    float l_dot_h;
    int _end_pad_0;
    int _end_pad_1;
};

struct CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX {
    uint face;
    int _pad1_0;
    float2 uv;
};

struct ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX {
    float3 weighted_result;
    float weight_sum;
    float coverage;
    int _end_pad_0;
    int _end_pad_1;
    int _end_pad_2;
};

struct ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX {
    bool include_directional;
    bool include_local;
    bool specular_highlights_enabled;
    bool glossy_reflections_enabled;
};

struct PbsMetallicMaterial {
    float4 _Color;
    float4 _EmissionColor;
    float4 _MainTex_ST;
    float4 _DetailAlbedoMap_ST;
    float _Cutoff;
    float _Glossiness;
    float _GlossMapScale;
    float _SmoothnessTextureChannel;
    float _UVSec;
    float _Metallic;
    float _BumpScale;
    float _Parallax;
    float _OcclusionStrength;
    float _DetailNormalMapScale;
    uint _RenderideVariantBits;
    float _MainTex_LodBias;
    float _MetallicGlossMap_LodBias;
    float _BumpMap_LodBias;
    float _ParallaxMap_LodBias;
    float _OcclusionMap_LodBias;
    float _EmissionMap_LodBias;
    float _DetailMask_LodBias;
    float _DetailAlbedoMap_LodBias;
    float _DetailNormalMap_LodBias;
    uint _BumpMap_WrapModeBits;
    uint _DetailAlbedoMap_WrapModeBits;
    uint _DetailMask_WrapModeBits;
    uint _DetailNormalMap_WrapModeBits;
    uint _EmissionMap_WrapModeBits;
    uint _MainTex_WrapModeBits;
    uint _MetallicGlossMap_WrapModeBits;
    uint _OcclusionMap_WrapModeBits;
    uint _ParallaxMap_WrapModeBits;
    int _end_pad_0;
    int _end_pad_1;
    int _end_pad_2;
};

struct SurfaceData {
    float3 base_color;
    float alpha;
    float metallic;
    float roughness;
    float occlusion;
    int _pad5_0;
    float3 normal;
    int _pad6_0;
    float3 emission;
    int _end_pad_0;
};

static const uint FRAME_PROJECTION_FLAG_ORTHOGRAPHICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 1u;
static const uint FRAME_TAIL_SAMPLE_COUNT_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 1u;
static const uint FRAME_TAIL_SAMPLE_COUNT_MASKX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 30u;
static const float UNITY_PARALLAX_VIEW_Z_BIASX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX = 0.42;
static const float COLOR_SPACE_DOUBLEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX = 4.594794;
static const uint PARTICLE_KIND_BILLBOARDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 1u;
static const uint PARTICLE_KIND_MESHX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 2u;
static const uint BILLBOARD_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 1u;
static const uint BILLBOARD_ALIGNMENT_LOCALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 2u;
static const uint BILLBOARD_ALIGNMENT_GLOBALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 3u;
static const uint BILLBOARD_ALIGNMENT_DIRECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 4u;
static const uint MESH_ALIGNMENT_VIEWX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 0u;
static const uint MESH_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX = 1u;
static const uint TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX = 32u;
static const float CLUSTER_NEAR_CLIP_MINX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX = 0.0001;
static const float CLUSTER_FAR_CLIP_MIN_SPANX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX = 0.0001;
static const uint TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX = 32u;
static const float SRGB_LINEAR_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 0.04045;
static const float SRGB_LINEAR_INV_SLOPEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 0.07739938;
static const float SRGB_NONLINEAR_OFFSETX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 0.055;
static const float SRGB_NONLINEAR_INV_SCALEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 0.9478673;
static const float SRGB_NONLINEAR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 2.4;
static const float SRGB_HDR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 2.2;
static const float INTENSITY_BOOSTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 3.1415927;
static const float BIRP_ATTENUATION_QUADRATICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX = 25.0;
static const uint LIGHT_COOKIE_KIND_SPOT_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 1u;
static const uint LIGHT_COOKIE_KIND_POINT_CUBEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 2u;
static const uint LIGHT_COOKIE_KIND_DIRECTIONAL_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 3u;
static const uint LIGHT_COOKIE_WRAP_U_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 0u;
static const uint LIGHT_COOKIE_WRAP_V_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 2u;
static const uint LIGHT_COOKIE_WRAP_MODE_MASKX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 3u;
static const uint LIGHT_COOKIE_WRAP_MODE_CLAMPX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 1u;
static const uint LIGHT_COOKIE_WRAP_MODE_MIRRORX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 2u;
static const uint LIGHT_COOKIE_WRAP_MODE_MIRROR_ONCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 3u;
static const float COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX = 1e-5;
static const uint SHADOW_VIEW_KIND_SPOTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 2u;
static const uint SHADOW_VIEW_KIND_POINTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX = 3u;
static const uint SHADOW_TYPE_SOFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = 2u;
static const float SHADOW_UV_BORDERX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX = 0.0005;
static const float SPECULAR_AA_VARIANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = 0.25;
static const float GEOMETRIC_SPECULAR_AA_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = 0.18;
static const float MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = 0.0001;
static const float MIN_ALPHAX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX = 0.002;
static const float MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = 1e-5;
static const float PROBE_SH2_SOURCE_NONEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = 0.0;
static const float MIN_PROBE_BLEND_DISTANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = 1e-6;
static const float PROBE_FLAG_BOX_PROJECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = 1.0;
static const float BOX_PROJECTION_FULL_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = 0.55;
static const float BOX_PROJECTION_OFF_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX = 0.9;
static const uint PBSMETALLIC_KW_ALPHABLEND_ON = 1u;
static const uint PBSMETALLIC_KW_ALPHAPREMULTIPLY_ON = 2u;
static const uint PBSMETALLIC_KW_ALPHATEST_ON = 4u;
static const uint PBSMETALLIC_KW_DETAIL_MULX2_ = 8u;
static const uint PBSMETALLIC_KW_EMISSION = 16u;
static const uint PBSMETALLIC_KW_GLOSSYREFLECTIONS_OFF = 32u;
static const uint PBSMETALLIC_KW_METALLICGLOSSMAP = 64u;
static const uint PBSMETALLIC_KW_NORMALMAP = 128u;
static const uint PBSMETALLIC_KW_PARALLAXMAP = 256u;
static const uint PBSMETALLIC_KW_SMOOTHNESS_TEXTURE_ALBEDO_CHANNEL_A = 512u;
static const uint PBSMETALLIC_KW_SPECULARHIGHLIGHTS_OFF = 1024u;
static const uint RENDERIDE_WRAP_MODE_MIRROR_ONCE_U = 1u;
static const uint RENDERIDE_WRAP_MODE_MIRROR_ONCE_V = 2u;
static const uint RENDERIDE_WRAP_MODE_MIRROR_ONCE_W = 4u;
static const uint renderide_static_variant_bits_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX = 0u;
static const uint renderide_static_variant_bitsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX = 0u;

cbuffer frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(b0) { FrameGlobalsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX; }
ByteAddressBuffer instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX : register(t0, space2);
ByteAddressBuffer cluster_light_rangesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t2);
ByteAddressBuffer cluster_light_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t3);
ByteAddressBuffer lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t1);
Texture2D<float4> ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t11);
Texture2D<float4> light_cookie_2d_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t13);
Texture2D<float4> light_cookie_point_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t14);
SamplerState nagaSamplerHeap[2048]: register(s0, space0);
SamplerComparisonState nagaComparisonSamplerHeap[2048]: register(s0, space1);
StructuredBuffer<uint> nagaGroup0SamplerIndexArray : register(t0, space255);
static const SamplerState light_cookie_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX = nagaSamplerHeap[nagaGroup0SamplerIndexArray[15]];
ByteAddressBuffer light_cookie_rectsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t19);
ByteAddressBuffer shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t16);
Texture2DArray<float> shadow_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t17);
static const SamplerComparisonState shadow_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX = nagaComparisonSamplerHeap[nagaGroup0SamplerIndexArray[18]];
Texture2DArray<float4> reflection_probe_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t9);
ByteAddressBuffer reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX : register(t12);
cbuffer mat : register(b0, space1) { PbsMetallicMaterial mat; }
Texture2D<float4> _MainTex : register(t1, space1);
StructuredBuffer<uint> nagaGroup1SamplerIndexArray : register(t1, space255);
static const SamplerState _MainTex_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[2]];
Texture2D<float4> _MetallicGlossMap : register(t3, space1);
static const SamplerState _MetallicGlossMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[4]];
Texture2D<float4> _BumpMap : register(t5, space1);
static const SamplerState _BumpMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[6]];
Texture2D<float4> _ParallaxMap : register(t7, space1);
static const SamplerState _ParallaxMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[8]];
Texture2D<float4> _OcclusionMap : register(t9, space1);
static const SamplerState _OcclusionMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[10]];
Texture2D<float4> _EmissionMap : register(t11, space1);
static const SamplerState _EmissionMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[12]];
Texture2D<float4> _DetailMask : register(t13, space1);
static const SamplerState _DetailMask_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[14]];
Texture2D<float4> _DetailAlbedoMap : register(t15, space1);
static const SamplerState _DetailAlbedoMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[16]];
Texture2D<float4> _DetailNormalMap : register(t17, space1);
static const SamplerState _DetailNormalMap_sampler = nagaSamplerHeap[nagaGroup1SamplerIndexArray[18]];

struct VertexOutput_vs_main {
    float3 world_pos_1 : LOC0;
    float3 world_n_1 : LOC1;
    float4 world_t_1 : LOC2;
    float2 primary_uv_1 : LOC3;
    float2 secondary_uv_1 : LOC4;
    nointerpolation uint view_layer_1 : LOC5;
    float4 clip_pos : SV_Position;
};

struct FragmentInput_fs_forward_base {
    float3 world_pos_2 : LOC0;
    float3 world_n_2 : LOC1;
    float4 world_t_2 : LOC2;
    float2 uv0_1_1 : LOC3;
    float2 uv1_1_1 : LOC4;
    nointerpolation uint view_layer_2 : LOC5;
    float4 frag_pos_1 : SV_Position;
};

uint effectiveX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(uint bits)
{
    if (false) {
        return renderide_static_variant_bitsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX;
    }
    return bits;
}

bool enabledX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(uint bits_1_, uint mask)
{
    const uint _e2 = effectiveX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(bits_1_);
    return ((_e2 & mask) != 0u);
}

float2 apply_stX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HK5QX(float2 uv_in, float4 st)
{
    return ((uv_in * st.xy) + st.zw);
}

float renderide_mirror_once_coord(float coord)
{
    return clamp(abs(coord), 0.0, 1.0);
}

float2 renderide_mirror_once_2d(float2 uv, uint wrap_mode_bits)
{
    float2 wrapped = (float2)0;

    wrapped = uv;
    if (((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_U) != 0u)) {
        float _e9 = wrapped.x;
        const float _e10 = renderide_mirror_once_coord(_e9);
        wrapped.x = _e10;
    }
    if (((wrap_mode_bits & RENDERIDE_WRAP_MODE_MIRROR_ONCE_V) != 0u)) {
        float _e17 = wrapped.y;
        const float _e18 = renderide_mirror_once_coord(_e17);
        wrapped.y = _e18;
    }
    float2 _e19 = wrapped;
    return _e19;
}

float4 renderide_mirroronce_sample_bias_2d(Texture2D<float4> tex, SamplerState samp, float2 uv_1, float bias, uint wrap_mode_bits_1)
{
    const float2 _e5 = renderide_mirror_once_2d(uv_1, wrap_mode_bits_1);
    float4 _e6 = tex.SampleBias(samp, _e5, bias);
    return _e6;
}

float4 sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(Texture2D<float4> tex_1, uint tex_wrap_mode_bits, SamplerState samp_1, float2 uv_2, float lod_bias)
{
    const float4 _e5 = renderide_mirroronce_sample_bias_2d(tex_1, samp_1, uv_2, lod_bias, tex_wrap_mode_bits);
    return _e5;
}

float4 view_space_z_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_1_)
{
    float4 _e2_ = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_space_z_coeffs;
    return _e2_;
}

float3 safe_normalize_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(float3 v, float3 fallback)
{
    float len = length(v);
    if ((len <= 1e-6)) {
        return fallback;
    }
    return (v / (len).xxx);
}

float3 orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_2_)
{
    const float4 _e1 = view_space_z_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_2_);
    const float3 _e7 = safe_normalize_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(_e1.xyz, float3(0.0, 0.0, -1.0));
    return _e7;
}

uint projection_flags_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_3_)
{
    uint _e3_ = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.frame_tail.y;
    return _e3_;
}

bool view_is_orthographicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_4_)
{
    const uint _e1 = projection_flags_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_4_);
    return ((_e1 & FRAME_PROJECTION_FLAG_ORTHOGRAPHICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX) != 0u);
}

float3 camera_world_pos_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_5_)
{
    float4 _e2_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.camera_world_pos;
    return _e2_1.xyz;
}

float3 view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(float3 world_pos_1_, uint view_layer_6_)
{
    const float3 _e2 = orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_6_);
    const bool _e3 = view_is_orthographicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_6_);
    if (_e3) {
        return _e2;
    }
    const float3 _e4 = camera_world_pos_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_6_);
    const float3 _e6 = safe_normalize_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX((_e4 - world_pos_1_), _e2);
    return _e6;
}

float3 safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(float3 v_1_, float3 fallback_1_)
{
    float len_sq = dot(v_1_, v_1_);
    if ((len_sq <= 1e-12)) {
        return fallback_1_;
    }
    return (v_1_ * rsqrt(len_sq));
}

float3x3 orthonormal_tbn_fallbackX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(float3 n_1_)
{
    float sign_ = ((n_1_.z >= 0.0) ? 1.0 : -1.0);
    float a = (-1.0 / (sign_ + n_1_.z));
    float b = ((n_1_.x * n_1_.y) * a);
    float3 t_11_ = float3((1.0 + (((sign_ * n_1_.x) * n_1_.x) * a)), (sign_ * b), (-(sign_) * n_1_.x));
    float3 bitan = float3(b, (sign_ + ((n_1_.y * n_1_.y) * a)), -(n_1_.y));
    return float3x3(normalize(t_11_), normalize(bitan), n_1_);
}

float3x3 orthonormal_tbnX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(float3 world_n_1_, float4 world_t_1_)
{
    bool local = (bool)0;
    bool local_1_ = (bool)0;

    float n_len_sq = dot(world_n_1_, world_n_1_);
    if (!(!((n_len_sq > 1e-10)))) {
        local = (n_len_sq > 3.402823e38);
    } else {
        local = true;
    }
    bool _e11_ = local;
    if (_e11_) {
        const float3x3 _e17 = orthonormal_tbn_fallbackX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(float3(0.0, 0.0, 1.0));
        return _e17;
    }
    float3 n_21_ = (world_n_1_ * rsqrt(n_len_sq));
    float3 t_ortho = (world_t_1_.xyz - (n_21_ * dot(world_t_1_.xyz, n_21_)));
    float t_len_sq = dot(t_ortho, t_ortho);
    if (!(!((t_len_sq > 1e-10)))) {
        local_1_ = (t_len_sq > 3.402823e38);
    } else {
        local_1_ = true;
    }
    bool _e35_ = local_1_;
    if (_e35_) {
        const float3x3 _e34 = orthonormal_tbn_fallbackX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(n_21_);
        return _e34;
    }
    float3 t_12_ = (t_ortho * rsqrt(t_len_sq));
    float sign_1_ = ((world_t_1_.w < 0.0) ? -1.0 : 1.0);
    const float3 _e48 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(n_21_, t_12_), float3(0.0, 1.0, 0.0));
    float3 b_1_ = (_e48 * sign_1_);
    return float3x3(t_12_, b_1_, n_21_);
}

float3 tangent_space_view_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(float3 world_pos_2_, float3 world_n_2_, float4 world_t_2_, uint view_layer_7_)
{
    const float3 _e4 = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_2_, view_layer_7_);
    const float3x3 _e5 = orthonormal_tbnX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(world_n_2_, world_t_2_);
    return normalize(float3(dot(_e4, _e5[0]), dot(_e4, _e5[1]), dot(_e4, _e5[2])));
}

float2 unity_parallax_offsetX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(float height_sample, float height_scale, float3 world_pos_3_, float3 world_n_3_, float4 world_t_3_, uint view_layer_8_)
{
    const float3 _e6 = tangent_space_view_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(world_pos_3_, world_n_3_, world_t_3_, view_layer_8_);
    float centered_height = ((height_sample * height_scale) - (height_scale * 0.5));
    return (centered_height * (_e6.xy / ((_e6.z + UNITY_PARALLAX_VIEW_Z_BIASX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX)).xx));
}

float3 decode_ts_normal_sample_rawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(float4 s)
{
    bool local_2_ = (bool)0;

    bool uniform_white_rgb = all((s.xyz > float3(0.99, 0.99, 0.99)));
    if (uniform_white_rgb) {
        return s.xyz;
    }
    bool all_r_high = (s.x >= 0.98039216);
    bool gb_close = (abs((s.y - s.z)) <= 0.03137255);
    if (all_r_high) {
        local_2_ = gb_close;
    } else {
        local_2_ = false;
    }
    bool _e21_ = local_2_;
    if (_e21_) {
        return float3(s.w, s.y, s.z);
    }
    return s.xyz;
}

float3 reconstruct_ts_normal_from_scaled_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(float2 xy)
{
    float z = sqrt(max((1.0 - dot(xy, xy)), 0.0));
    return float3(xy, z);
}

float3 decode_ts_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(float3 raw, float scale)
{
    float2 xy_1_ = (((raw.xy * 2.0) - (1.0).xx) * scale);
    const float3 _e9 = reconstruct_ts_normal_from_scaled_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(xy_1_);
    return _e9;
}

float3 decode_ts_normal_with_placeholderX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(float3 raw_1_, float scale_1_)
{
    if (all((raw_1_ > float3(0.99, 0.99, 0.99)))) {
        return float3(0.0, 0.0, 1.0);
    }
    const float3 _e12 = decode_ts_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(raw_1_, scale_1_);
    return _e12;
}

float3 decode_ts_normal_with_placeholder_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(float4 s_1_, float scale_2_)
{
    const float3 _e2 = decode_ts_normal_sample_rawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(s_1_);
    const float3 _e3 = decode_ts_normal_with_placeholderX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(_e2, scale_2_);
    return _e3;
}

float roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZWC3LQNRUW4ZYX(float smoothness)
{
    return clamp((1.0 - smoothness), 0.0, 1.0);
}

float standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(float color_alpha, float texture_alpha, bool smoothness_from_albedo_alpha)
{
    if (smoothness_from_albedo_alpha) {
        return color_alpha;
    }
    return (color_alpha * texture_alpha);
}

void clip_standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(float alpha, float cutoff, bool enabled)
{
    bool local_3_ = (bool)0;

    if (enabled) {
        local_3_ = (alpha < cutoff);
    } else {
        local_3_ = false;
    }
    bool _e7_ = local_3_;
    if (_e7_) {
        discard;
    } else {
        return;
    }
}

float2 apply_parallaxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(float2 uv_1_, bool enabled_1_, float parallax, float3 world_pos_4_, float3 world_n_4_, float4 world_t_4_, uint view_layer_9_, Texture2D<float4> parallax_map, uint parallax_map_wrap_mode_bits, SamplerState parallax_sampler, float parallax_lod_bias)
{
    if (!(enabled_1_)) {
        return uv_1_;
    }
    const float4 _e12 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(parallax_map, parallax_map_wrap_mode_bits, parallax_sampler, uv_1_, parallax_lod_bias);
    float h_1_ = _e12.y;
    const float2 _e14 = unity_parallax_offsetX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJYGC4TBNRWGC6AX(h_1_, parallax, world_pos_4_, world_n_4_, world_t_4_, view_layer_9_);
    return (uv_1_ + _e14);
}

float3 sample_world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(bool normal_enabled, bool detail_enabled, float detail_mask, Texture2D<float4> normal_tex, uint normal_tex_wrap_mode_bits, SamplerState normal_sampler, float2 uv_main, float normal_lod_bias, float normal_scale, Texture2D<float4> detail_normal_tex, uint detail_normal_tex_wrap_mode_bits, SamplerState detail_normal_sampler, float2 uv_detail, float detail_normal_lod_bias, float detail_normal_scale, float3 world_n_5_, float4 world_t_5_, float3 fallback_normal)
{
    float3 ts_n = (float3)0;
    bool local_4_ = (bool)0;

    if (!(normal_enabled)) {
        return fallback_normal;
    }
    const float3x3 _e21 = orthonormal_tbnX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJXG64TNMFWAX(world_n_5_, world_t_5_);
    const float4 _e22 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(normal_tex, normal_tex_wrap_mode_bits, normal_sampler, uv_main, normal_lod_bias);
    const float3 _e23 = decode_ts_normal_with_placeholder_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(_e22, normal_scale);
    ts_n = _e23;
    if (detail_enabled) {
        local_4_ = (detail_mask > 0.001);
    } else {
        local_4_ = false;
    }
    bool _e21_1 = local_4_;
    if (_e21_1) {
        const float4 _e28 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(detail_normal_tex, detail_normal_tex_wrap_mode_bits, detail_normal_sampler, uv_detail, detail_normal_lod_bias);
        const float3 _e29 = decode_ts_normal_with_placeholder_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G433SNVQWYX3EMVRW6ZDFX(_e28, detail_normal_scale);
        float3 _e29_ = ts_n;
        float _e34_ = ts_n.z;
        float3 detail_blend = normalize(float3((_e29_.xy + _e29.xy), (_e34_ * _e29.z)));
        float3 _e39_ = ts_n;
        ts_n = lerp(_e39_, detail_blend, clamp(detail_mask, 0.0, 1.0));
    }
    float3 _e44_ = ts_n;
    return normalize(mul(_e44_, _e21));
}

float roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(float smoothness_1_)
{
    const float _e1 = roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZWC3LQNRUW4ZYX(smoothness_1_);
    return _e1;
}

float occlusion_from_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(float sample_, float strength)
{
    return lerp(1.0, sample_, clamp(strength, 0.0, 1.0));
}

float3 sample_emissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(bool enabled_2_, float3 emission_color, Texture2D<float4> emission_tex, uint emission_tex_wrap_mode_bits, SamplerState emission_sampler, float2 uv_main_1_, float lod_bias_1_)
{
    if (!(enabled_2_)) {
        return (0.0).xxx;
    }
    const float4 _e10 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(emission_tex, emission_tex_wrap_mode_bits, emission_sampler, uv_main_1_, lod_bias_1_);
    return (_e10.xyz * emission_color);
}

float2 detail_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(float2 uv0_2_, float2 uv1_2_, float uv_sec, float4 detail_st)
{
    const float2 _e7 = apply_stX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HK5QX(((uv_sec >= 0.5) ? uv1_2_ : uv0_2_), detail_st);
    return _e7;
}

float sample_detail_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(bool enabled_3_, Texture2D<float4> detail_mask_tex, uint detail_mask_tex_wrap_mode_bits, SamplerState detail_mask_sampler, float2 uv_main_2_, float lod_bias_2_)
{
    if (!(enabled_3_)) {
        return 0.0;
    }
    const float4 _e8 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(detail_mask_tex, detail_mask_tex_wrap_mode_bits, detail_mask_sampler, uv_main_2_, lod_bias_2_);
    return _e8.w;
}

float3 apply_detail_albedoX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(float3 base_color, bool enabled_4_, float detail_mask_1_, Texture2D<float4> detail_tex, uint detail_tex_wrap_mode_bits, SamplerState detail_sampler, float2 uv_detail_1_, float lod_bias_3_)
{
    if (!(enabled_4_)) {
        return base_color;
    }
    const float4 _e9 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(detail_tex, detail_tex_wrap_mode_bits, detail_sampler, uv_detail_1_, lod_bias_3_);
    float3 detail = _e9.xyz;
    return (base_color * lerp((1.0).xxx, (detail * COLOR_SPACE_DOUBLEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX), detail_mask_1_));
}

float4x4 select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_1_, uint view_idx)
{
    if ((view_idx == 0u)) {
        return draw_1_.view_proj_left;
    }
    return draw_1_.view_proj_right;
}

bool position_stream_is_world_spaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_2_)
{
    return (draw_2_._pad.x > 0.5);
}

float4x4 select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_3_, uint view_idx_1_)
{
    const float4x4 _e2 = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_3_, view_idx_1_);
    return _e2;
}

float4 world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_4_, float4 pos_1_)
{
    return mul(float4(pos_1_.xyz, 1.0), draw_4_.model);
}

float3 world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_5_, float4 n_2_)
{
    const float3 _e9 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(mul(n_2_.xyz, draw_5_.normal_matrix), float3(0.0, 1.0, 0.0));
    return _e9;
}

float3 model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_6_, float3 v_2_)
{
    return mul(float4(v_2_, 0.0), draw_6_.model).xyz;
}

float model_handednessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_7_)
{
    const bool _e1 = position_stream_is_world_spaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_7_);
    if (_e1) {
        return 1.0;
    }
    float det = dot(draw_7_.model[0].xyz, cross(draw_7_.model[1].xyz, draw_7_.model[2].xyz));
    return ((det < 0.0) ? -1.0 : 1.0);
}

float tangent_w_signX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(float tangent_w)
{
    return ((tangent_w < 0.0) ? -1.0 : 1.0);
}

float4 world_tangentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_8_, float4 t_1_)
{
    const float3 _e3 = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_8_, t_1_.xyz);
    const float _e5 = tangent_w_signX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(t_1_.w);
    const float _e6 = model_handednessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_8_);
    return float4(normalize(_e3), (_e5 * _e6));
}

uint packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(uint instance_index_1_, uint view_idx_2_)
{
    return ((instance_index_1_ << 1u) | (view_idx_2_ & 1u));
}

uint naga_f2u32(float value) {
    return uint(clamp(value, 0.0, 4294967000.0));
}

uint particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_9_)
{
    return naga_f2u32((max(draw_9_.particle_header.x, 0.0) + 0.5));
}

uint particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_10_)
{
    return naga_f2u32((max(draw_10_.particle_header.y, 0.0) + 0.5));
}

float particle_min_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_11_)
{
    return max(draw_11_.particle_header.z, 0.0);
}

float particle_max_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_12_)
{
    return max(draw_12_.particle_header.w, 0.0);
}

uint particle_frame_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_13_)
{
    if ((draw_13_.particle_extra.y >= 4294967000.0)) {
        return 4294967295u;
    }
    return naga_f2u32((max(draw_13_.particle_extra.y, 0.0) + 0.5));
}

uint2 particle_frame_grid_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_14_)
{
    return uint2(naga_f2u32((max(draw_14_.particle_frame_grid.x, 0.0) + 0.5)), naga_f2u32((max(draw_14_.particle_frame_grid.y, 0.0) + 0.5)));
}

float4 view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_10_)
{
    float4 _e2_2 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_to_world_y_coeffs;
    return _e2_2;
}

float2 viewport_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX()
{
    uint _e2_3 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_width;
    uint _e6_ = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_height;
    return float2(float(_e2_3), float(_e6_));
}

float2 ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(float4 clip_)
{
    return (clip_.xy / (max(abs(clip_.w), 1e-6)).xx);
}

uint naga_mod(uint lhs, uint rhs) {
    return lhs % (rhs == 0u ? 1u : rhs);
}

uint naga_div(uint lhs, uint rhs) {
    return lhs / (rhs == 0u ? 1u : rhs);
}

float2 particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_15_, float2 uv_2_)
{
    bool local_5_ = (bool)0;

    const uint _e3 = particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_15_);
    if ((_e3 != PARTICLE_KIND_MESHX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
        return uv_2_;
    }
    const uint _e6 = particle_frame_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_15_);
    if ((_e6 == 4294967295u)) {
        return uv_2_;
    }
    const uint2 _e9 = particle_frame_grid_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_15_);
    if (!((_e9.x == 0u))) {
        local_5_ = (_e9.y == 0u);
    } else {
        local_5_ = true;
    }
    bool _e19_ = local_5_;
    if (_e19_) {
        return uv_2_;
    }
    uint frame_count = max((_e9.x * _e9.y), 1u);
    uint clamped_frame = min(_e6, (frame_count - 1u));
    uint column = naga_mod(clamped_frame, _e9.x);
    uint row = ((_e9.y - 1u) - naga_div(clamped_frame, _e9.x));
    return float2(((float(column) + uv_2_.x) / float(_e9.x)), ((float(row) + uv_2_.y) / float(_e9.y)));
}

bool mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_16_)
{
    bool local_6_ = (bool)0;
    bool local_7_ = (bool)0;

    const uint _e3 = particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_16_);
    const uint _e4 = particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_16_);
    if ((_e4 == PARTICLE_KIND_MESHX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
        if (!((_e3 == MESH_ALIGNMENT_VIEWX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX))) {
            local_7_ = (_e3 == MESH_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX);
        } else {
            local_7_ = true;
        }
        bool _e13_ = local_7_;
        local_6_ = _e13_;
    } else {
        local_6_ = false;
    }
    bool _e17_ = local_6_;
    return _e17_;
}

MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX ConstructMeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(float3 arg0, float3 arg1, float3 arg2) {
    MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX ret = (MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX)0;
    ret.right = arg0;
    ret.up = arg1;
    ret.forward = arg2;
    return ret;
}

MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_17_, uint view_idx_3_)
{
    float3 to_camera = (float3)0;

    float3 center_world_5_ = draw_17_.model[3].xyz;
    const float4 _e6 = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_3_);
    const float3 _e12 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e6.xyz, float3(0.0, 1.0, 0.0));
    const float3 _e13 = orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_3_);
    to_camera = _e13;
    const uint _e14 = particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_17_);
    if ((_e14 == MESH_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
        const float3 _e17 = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(center_world_5_, view_idx_3_);
        to_camera = _e17;
    }
    float3 _e18_ = to_camera;
    const float3 _e24 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e12, _e18_), float3(1.0, 0.0, 0.0));
    float3 _e25_ = to_camera;
    const float3 _e27 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e25_, _e24), _e12);
    float3 _e28_ = to_camera;
    const MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX meshparticlebasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5haylsoruwg3dfx = ConstructMeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(_e24, _e27, _e28_);
    return meshparticlebasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5haylsoruwg3dfx;
}

float3 mesh_particle_model_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_18_)
{
    return float3(max(length(draw_18_.model[0].xyz), 1e-6), max(length(draw_18_.model[1].xyz), 1e-6), max(length(draw_18_.model[2].xyz), 1e-6));
}

float4 mesh_particle_world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_19_, float4 pos_2_, uint view_idx_4_)
{
    const bool _e3 = mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19_);
    if (!(_e3)) {
        const float4 _e5 = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_19_, pos_2_);
        return _e5;
    }
    const MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX _e6 = mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19_, view_idx_4_);
    const float3 _e7 = mesh_particle_model_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_19_);
    float3 local_32_ = (pos_2_.xyz * _e7);
    float3 center_world_6_ = draw_19_.model[3].xyz;
    return float4((((center_world_6_ + (_e6.right * local_32_.x)) + (_e6.up * local_32_.y)) + (_e6.forward * local_32_.z)), 1.0);
}

bool render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_20_)
{
    const uint _e1 = particle_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_20_);
    return (_e1 == PARTICLE_KIND_BILLBOARDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX);
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX ConstructBillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(float3 arg0, float3 arg1) {
    BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX ret = (BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX)0;
    ret.right = arg0;
    ret.up = arg1;
    return ret;
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(float angle_1_, float3 right_1_, float3 up_1_)
{
    float c_1_ = cos(angle_1_);
    float s_7_ = sin(angle_1_);
    const float3 _e8 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(((right_1_ * c_1_) + (up_1_ * s_7_)), right_1_);
    const float3 _e13 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(((-(right_1_) * s_7_) + (up_1_ * c_1_)), up_1_);
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx = ConstructBillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e8, _e13);
    return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx;
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX source_material_facing_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(float3 center_world, uint view_idx_5_, float roll)
{
    const float4 _e3 = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_5_);
    const float3 _e9 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e3.xyz, float3(0.0, 1.0, 0.0));
    const float3 _e10 = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(center_world, view_idx_5_);
    const float3 _e16 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e9, _e10), float3(1.0, 0.0, 0.0));
    const float3 _e18 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e10, _e16), _e9);
    if ((abs(roll) > 0.0001)) {
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e22 = rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(roll, _e16, _e18);
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_1 = _e22;
        return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_1;
    }
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_2 = ConstructBillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e16, _e18);
    return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_2;
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX source_material_local_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_21_, float3 pointdata, float4 tangent)
{
    float3 fallback_seed = float3(0.0, 1.0, 0.0);
    float3 right_2_ = (float3)0;
    float3 up_2_ = (float3)0;

    const float3 _e15 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(tangent.xyz, float3(0.0, 0.0, 1.0));
    if ((abs(_e15.y) > 0.95)) {
        fallback_seed = float3(1.0, 0.0, 0.0);
    }
    float3 _e19_1 = fallback_seed;
    const float3 _e30 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e15, _e19_1), float3(1.0, 0.0, 0.0));
    float3 _e27_ = fallback_seed;
    const float3 _e33 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e30, _e15), _e27_);
    const float3 _e34 = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_21_, _e15);
    const float3 _e39 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e34, float3(0.0, 0.0, 1.0));
    const float3 _e40 = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_21_, _e33);
    const float3 _e45 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e40, float3(0.0, 1.0, 0.0));
    const float3 _e51 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e39, _e45), float3(1.0, 0.0, 0.0));
    right_2_ = _e51;
    float3 _e49_ = right_2_;
    const float3 _e54 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e49_, _e39), _e45);
    up_2_ = _e54;
    if ((abs(pointdata.z) > 0.0001)) {
        float3 _e59_ = right_2_;
        float3 _e60_ = up_2_;
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e62 = rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(pointdata.z, _e59_, _e60_);
        right_2_ = _e62.right;
        up_2_ = _e62.up;
    }
    float3 _e64_ = right_2_;
    float3 _e65_ = up_2_;
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_3 = ConstructBillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e64_, _e65_);
    return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_3;
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX direction_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_22_, float3 center_world_1_, float3 direction, uint view_idx_6_)
{
    float3 up_3_ = (float3)0;

    const float3 _e5 = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(center_world_1_, view_idx_6_);
    const float3 _e6 = model_vectorX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_22_, direction);
    float3 velocity_in_plane = (_e6 - (_e5 * dot(_e6, _e5)));
    const float4 _e10 = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_6_);
    float3 view_up = _e10.xyz;
    float3 view_up_in_plane = (view_up - (_e5 * dot(view_up, _e5)));
    const float3 _e19 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(view_up_in_plane, float3(0.0, 1.0, 0.0));
    const float3 _e20 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(velocity_in_plane, _e19);
    up_3_ = _e20;
    float3 _e21_2 = up_3_;
    const float3 _e27 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e21_2, _e5), float3(1.0, 0.0, 0.0));
    float3 _e29_1 = up_3_;
    const float3 _e30 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e5, _e27), _e29_1);
    up_3_ = _e30;
    float3 _e31_ = up_3_;
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_4 = ConstructBillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e27, _e31_);
    return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_4;
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX source_material_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(uint view_idx_7_, float roll_1_)
{
    const float4 _e2 = view_to_world_y_coeffs_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_7_);
    const float3 _e8 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(_e2.xyz, float3(0.0, 1.0, 0.0));
    const float3 _e9 = orthographic_view_dir_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_idx_7_);
    const float3 _e15 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e8, _e9), float3(1.0, 0.0, 0.0));
    const float3 _e17 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(_e9, _e15), _e8);
    if ((abs(roll_1_) > 0.0001)) {
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e21 = rotate_source_material_billboard_axesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(roll_1_, _e15, _e17);
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_5 = _e21;
        return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_5;
    }
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_6 = ConstructBillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX(_e15, _e17);
    return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_6;
}

BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_23_, float3 center_world_2_, float3 pointdata_1_, float4 tangent_1_, uint view_idx_8_)
{
    bool local_8_ = (bool)0;

    const uint _e6 = particle_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_23_);
    if ((_e6 == BILLBOARD_ALIGNMENT_FACINGX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e10 = source_material_facing_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(center_world_2_, view_idx_8_, pointdata_1_.z);
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_7 = _e10;
        return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_7;
    }
    if (!((_e6 == BILLBOARD_ALIGNMENT_LOCALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX))) {
        local_8_ = (_e6 == BILLBOARD_ALIGNMENT_GLOBALX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX);
    } else {
        local_8_ = true;
    }
    bool _e17_1 = local_8_;
    if (_e17_1) {
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e18 = source_material_local_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_23_, pointdata_1_, tangent_1_);
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_8 = _e18;
        return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_8;
    }
    if ((_e6 == BILLBOARD_ALIGNMENT_DIRECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)) {
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e22 = direction_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_23_, center_world_2_, tangent_1_.xyz, view_idx_8_);
        const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_9 = _e22;
        return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_9;
    }
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e24 = source_material_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(view_idx_8_, pointdata_1_.z);
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_10 = _e24;
    return billboardbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5ge2lmnrrg6ylsmqx_10;
}

float2 signed_corner_from_pointdataX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(float3 pointdata_2_)
{
    return float2(((pointdata_2_.x >= 0.0) ? 1.0 : -1.0), ((pointdata_2_.y >= 0.0) ? 1.0 : -1.0));
}

float2 screen_clamped_billboard_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_24_, float3 center_world_3_, BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX axes, float2 size, float4x4 vp)
{
    bool local_9_ = (bool)0;
    float scale_3_ = 1.0;
    bool local_10_ = (bool)0;
    bool local_11_ = (bool)0;

    const float _e10 = particle_min_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_24_);
    const float _e11 = particle_max_screen_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_24_);
    if ((_e10 <= 0.0)) {
        local_9_ = (_e11 <= 0.0);
    } else {
        local_9_ = false;
    }
    bool _e11_1 = local_9_;
    if (_e11_1) {
        return size;
    }
    const float2 _e18 = viewport_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX();
    float2 viewport = max(_e18, float2(1.0, 1.0));
    const float2 _e26 = ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(mul(float4(center_world_3_, 1.0), vp));
    const float2 _e34 = ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(mul(float4((center_world_3_ + (axes.right * size.x)), 1.0), vp));
    const float2 _e42 = ndc_xyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(mul(float4((center_world_3_ + (axes.up * size.y)), 1.0), vp));
    float right_pixels = length((((_e34 - _e26) * viewport) * 0.5));
    float up_pixels = length((((_e42 - _e26) * viewport) * 0.5));
    float screen_fraction = (max(right_pixels, up_pixels) / max(min(viewport.x, viewport.y), 1.0));
    if ((screen_fraction <= 1e-6)) {
        return size;
    }
    if ((_e10 > 0.0)) {
        local_10_ = (screen_fraction < _e10);
    } else {
        local_10_ = false;
    }
    bool _e66_ = local_10_;
    if (_e66_) {
        float _e68_ = scale_3_;
        scale_3_ = max(_e68_, (_e10 / screen_fraction));
    }
    if ((_e11 > 0.0)) {
        float _e73_ = scale_3_;
        local_11_ = ((screen_fraction * _e73_) > _e11);
    } else {
        local_11_ = false;
    }
    bool _e79_ = local_11_;
    if (_e79_) {
        scale_3_ = (_e11 / screen_fraction);
    }
    float _e81_ = scale_3_;
    return max((size * _e81_), float2(1e-6, 1e-6));
}

float4 render_buffer_billboard_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(float3 center_world_4_, BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX axes_1_, float2 corner, float2 size_1_)
{
    return float4(((center_world_4_ + ((axes_1_.right * corner.x) * size_1_.x)) + ((axes_1_.up * corner.y) * size_1_.y)), 1.0);
}

float4 source_material_render_buffer_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_25_, float4 pos_3_, float4 pointdata_3_, float4 tangent_2_, uint view_idx_9_)
{
    const float4 _e5 = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_25_, pos_3_);
    float3 center_world_7_ = _e5.xyz;
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e8 = source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25_, center_world_7_, pointdata_3_.xyz, tangent_2_, view_idx_9_);
    const float2 _e10 = signed_corner_from_pointdataX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(pointdata_3_.xyz);
    const float3 _e11 = mesh_particle_model_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25_);
    float2 model_scale = _e11.xy;
    float2 unclamped_size = max((abs(pointdata_3_.xy) * model_scale), float2(1e-6, 1e-6));
    const float4x4 _e20 = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_25_, view_idx_9_);
    const float2 _e21 = screen_clamped_billboard_sizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_25_, center_world_7_, _e8, unclamped_size, _e20);
    const float4 _e22 = render_buffer_billboard_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(center_world_7_, _e8, _e10, _e21);
    return _e22;
}

float4 world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_26_, float4 pos_4_, float4 n_3_, float4 t_2_, uint view_idx_10_)
{
    const bool _e5 = render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26_);
    if (_e5) {
        const float4 _e6 = source_material_render_buffer_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26_, pos_4_, n_3_, t_2_, view_idx_10_);
        return _e6;
    }
    const float4 _e7 = mesh_particle_world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_26_, pos_4_, view_idx_10_);
    return _e7;
}

float3 render_buffer_billboard_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX axes_2_)
{
    const float3 _e8 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX(cross(axes_2_.right, axes_2_.up), float3(0.0, 0.0, 1.0));
    return _e8;
}

float3 source_material_render_buffer_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_27_, float4 pos_5_, float4 pointdata_4_, float4 tangent_3_, uint view_idx_11_)
{
    const float4 _e5 = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_27_, pos_5_);
    float3 center_world_8_ = _e5.xyz;
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e8 = source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_27_, center_world_8_, pointdata_4_.xyz, tangent_3_, view_idx_11_);
    const float3 _e9 = render_buffer_billboard_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(_e8);
    return _e9;
}

float3 world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_28_, float4 pos_6_, float4 n_4_, float4 t_3_, uint view_idx_12_)
{
    const bool _e5 = render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28_);
    if (_e5) {
        const float3 _e6 = source_material_render_buffer_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28_, pos_6_, n_4_, t_3_, view_idx_12_);
        return _e6;
    }
    const bool _e7 = mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28_);
    if (!(_e7)) {
        const float3 _e9 = world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_28_, n_4_);
        return _e9;
    }
    const MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX _e10 = mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_28_, view_idx_12_);
    const float3 _e23 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX((((_e10.right * n_4_.x) + (_e10.up * n_4_.y)) + (_e10.forward * n_4_.z)), _e10.forward);
    return _e23;
}

float4 source_material_render_buffer_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_29_, float4 pos_7_, float4 pointdata_5_, float4 tangent_4_, uint view_idx_13_)
{
    const float4 _e5 = world_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_29_, pos_7_);
    float3 center_world_9_ = _e5.xyz;
    const BillboardBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5GE2LMNRRG6YLSMQX _e8 = source_material_render_buffer_billboard_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_29_, center_world_9_, pointdata_5_.xyz, tangent_4_, view_idx_13_);
    return float4(_e8.right, 1.0);
}

float4 world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_30_, float4 pos_8_, float4 n_5_, float4 t_4_, uint view_idx_14_)
{
    const bool _e5 = render_buffer_billboard_uses_source_materialX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30_);
    if (_e5) {
        const float4 _e6 = source_material_render_buffer_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30_, pos_8_, n_5_, t_4_, view_idx_14_);
        return _e6;
    }
    const bool _e7 = mesh_particle_uses_view_alignmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30_);
    if (!(_e7)) {
        const float4 _e9 = world_tangentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_30_, t_4_);
        return _e9;
    }
    const MeshParticleBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX _e10 = mesh_particle_view_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_30_, view_idx_14_);
    const float3 _e23 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5G2YLUNAX((((_e10.right * t_4_.x) + (_e10.up * t_4_.y)) + (_e10.forward * t_4_.z)), _e10.right);
    const float _e25 = tangent_w_signX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(t_4_.w);
    return float4(_e23, _e25);
}

uint reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_31_)
{
    uint packed = asuint(draw_31_._pad.y);
    return (packed & 65535u);
}

uint fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_32_)
{
    uint packed_1_ = asuint(draw_32_._pad.y);
    return (packed_1_ >> 16u);
}

uint4 local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_33_)
{
    uint packed_z = asuint(draw_33_._pad.z);
    uint packed_w = asuint(draw_33_._pad.w);
    return uint4((packed_z & 65535u), (packed_z >> 16u), (packed_w & 65535u), (packed_w >> 16u));
}

bool has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_34_)
{
    bool local_12_ = (bool)0;

    const uint4 _e2 = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_34_);
    const uint _e3 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_34_);
    if (!((_e3 != 0u))) {
        local_12_ = any((_e2 != (0u).xxxx));
    } else {
        local_12_ = true;
    }
    bool _e13_1 = local_12_;
    return _e13_1;
}

typedef float4 ret_Constructarray12_float4_[12];
ret_Constructarray12_float4_ Constructarray12_float4_(float4 arg0, float4 arg1, float4 arg2, float4 arg3, float4 arg4, float4 arg5, float4 arg6, float4 arg7, float4 arg8, float4 arg9, float4 arg10, float4 arg11) {
    float4 ret[12] = { arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11 };
    return ret;
}

PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX ConstructPerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(float4x4 arg0, float4x4 arg1, float4x4 arg2, float3x3 arg3, float4 arg4, float4 arg5, float4 arg6, float4 arg7, float4 arg8, float4 arg9[12]) {
    PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX ret = (PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX)0;
    ret.view_proj_left = arg0;
    ret.view_proj_right = arg1;
    ret.model = arg2;
    ret.normal_matrix = arg3;
    ret._pad = arg4;
    ret.particle_header = arg5;
    ret.particle_tint = arg6;
    ret.particle_extra = arg7;
    ret.particle_frame_grid = arg8;
    ret.particle_padding = arg9;
    return ret;
}

PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX get_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(uint instance_idx)
{
    PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX _e3_1 = ConstructPerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(float4x4(asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+0+0)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+0+16)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+0+32)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+0+48))), float4x4(asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+64+0)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+64+16)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+64+32)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+64+48))), float4x4(asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+128+0)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+128+16)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+128+32)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+128+48))), float3x3(asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load3(instance_idx*512+192+0)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load3(instance_idx*512+192+16)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load3(instance_idx*512+192+32))), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+240)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+256)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+272)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+288)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+304)), Constructarray12_float4_(asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+0)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+16)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+32)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+48)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+64)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+80)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+96)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+112)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+128)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+144)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+160)), asfloat(instancesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX.Load4(instance_idx*512+320+176))));
    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX perdrawuniformsx_naga_oil_mod_xojsw4zdfojuwizj2hjsheylxhi5hi6lqmvzqx = _e3_1;
    return perdrawuniformsx_naga_oil_mod_xojsw4zdfojuwizj2hjsheylxhi5hi6lqmvzqx;
}

float4 world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_35_, float4 pos_9_, float4 n_6_, float4 t_5_, uint view_idx_15_)
{
    const float4 _e5 = world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_35_, pos_9_, n_6_, t_5_, view_idx_15_);
    return _e5;
}

float4x4 select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_36_, uint view_idx_16_)
{
    const float4x4 _e2 = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(draw_36_, view_idx_16_);
    return _e2;
}

float3 world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_37_, float4 pos_10_, float4 n_7_, float4 t_6_, uint view_idx_17_)
{
    const float3 _e5 = world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_37_, pos_10_, n_7_, t_6_, view_idx_17_);
    return _e5;
}

float4 world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_38_, float4 pos_11_, float4 n_8_, float4 t_7_, uint view_idx_18_)
{
    const float4 _e5 = world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_38_, pos_11_, n_8_, t_7_, view_idx_18_);
    return _e5;
}

float2 particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_39_, float2 uv_3_)
{
    const float2 _e2 = particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HAYLSORUWG3DFX(draw_39_, uv_3_);
    return _e2;
}

uint packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(uint instance_index_2_, uint view_idx_19_)
{
    const uint _e2 = packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HI4TBNZZWM33SNUX(instance_index_2_, view_idx_19_);
    return _e2;
}

WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX world_uv2_vertex_mainX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(uint instance_index_3_, uint view_idx_20_, float4 pos_12_, float4 n_9_, float4 t_8_, float2 primary_uv, float2 secondary_uv)
{
    WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX out_ = (WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX)0;

    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX _e8 = get_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(instance_index_3_);
    const float4 _e9 = world_position_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e8, pos_12_, n_9_, t_8_, view_idx_20_);
    const float4x4 _e10 = select_view_projX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e8, view_idx_20_);
    out_.clip_pos = mul(_e9, _e10);
    out_.world_pos = _e9.xyz;
    const float3 _e15 = world_normal_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e8, pos_12_, n_9_, t_8_, view_idx_20_);
    out_.world_n = _e15;
    const float4 _e17 = world_tangent_for_viewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e8, pos_12_, n_9_, t_8_, view_idx_20_);
    out_.world_t = _e17;
    const float2 _e19 = particle_primary_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(_e8, primary_uv);
    out_.primary_uv = _e19;
    out_.secondary_uv = secondary_uv;
    const uint _e22 = packed_view_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(instance_index_3_, view_idx_20_);
    out_.view_layer = _e22;
    WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX _e24_ = out_;
    const WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx = _e24_;
    return worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx;
}

MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX ConstructMetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(float3 arg0, float arg1, float arg2, float arg3, float arg4, float3 arg5, float3 arg6, float3 arg7) {
    MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX ret = (MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX)0;
    ret.base_color = arg0;
    ret.alpha = arg1;
    ret.metallic = arg2;
    ret.roughness = arg3;
    ret.occlusion = arg4;
    ret.normal = arg5;
    ret.geometric_normal = arg6;
    ret.emission = arg7;
    return ret;
}

MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX metallic_with_geometric_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(float3 base_color_1_, float alpha_1_, float metallic_value, float roughness, float occlusion, float3 normal, float3 geometric_normal, float3 emission)
{
    const MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX metallicsurfacex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjzxk4tgmfrwkx = ConstructMetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(base_color_1_, alpha_1_, clamp(metallic_value, 0.0, 1.0), roughness, occlusion, normalize(normal), normalize(geometric_normal), emission);
    return metallicsurfacex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjzxk4tgmfrwkx;
}

float finite_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(float value, float fallback_2_)
{
    return ((abs(value) <= 3.402823e38) ? value : fallback_2_);
}

ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX ConstructClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(float arg0, float arg1) {
    ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX ret = (ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX)0;
    ret.near_clip = arg0;
    ret.far_clip = arg1;
    return ret;
}

ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX sanitize_cluster_clip_planesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(float near_clip, float far_clip)
{
    const float _e3 = finite_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_clip, CLUSTER_NEAR_CLIP_MINX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX);
    float near_safe = max(_e3, CLUSTER_NEAR_CLIP_MINX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX);
    const float _e8 = finite_orX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(far_clip, (near_safe + CLUSTER_FAR_CLIP_MIN_SPANX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX));
    float far_safe = max(_e8, (near_safe + CLUSTER_FAR_CLIP_MIN_SPANX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX));
    const ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX clusterclipplanesx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjrwy5ltorsxex3nmf2gqx = ConstructClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_safe, far_safe);
    return clusterclipplanesx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjrwy5ltorsxex3nmf2gqx;
}

uint cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(float view_z, float near_clip_1_, float far_clip_1_, uint cluster_count_z)
{
    uint z_count = max(cluster_count_z, 1u);
    const ClusterClipPlanesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX _e6 = sanitize_cluster_clip_planesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(near_clip_1_, far_clip_1_);
    float d = clamp(-(view_z), _e6.near_clip, _e6.far_clip);
    float z_1_ = ((log((d / _e6.near_clip)) / log((_e6.far_clip / _e6.near_clip))) * float(z_count));
    return naga_f2u32(clamp(z_1_, 0.0, float((z_count - 1u))));
}

uint view_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_11_)
{
    return (view_layer_11_ & 1u);
}

uint2 cluster_light_range_atX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(uint cluster_id)
{
    uint2 _e3_2 = asuint(cluster_light_rangesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load2(cluster_id*8));
    return _e3_2;
}

uint cluster_light_index_from_rangeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(uint2 range, uint slot)
{
    uint _e6_1 = asuint(cluster_light_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load((range.x + slot)*4));
    return _e6_1;
}

uint cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(float view_z_1_, float near_clip_2_, float far_clip_2_, uint cluster_count_z_1_)
{
    const uint _e4 = cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRWY5LTORSXEX3NMF2GQX(view_z_1_, near_clip_2_, far_clip_2_, cluster_count_z_1_);
    return _e4;
}

uint2 cluster_xy_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(float2 frag_xy, uint viewport_w, uint viewport_h)
{
    uint vw = max(viewport_w, 1u);
    uint vh = max(viewport_h, 1u);
    uint px = min(naga_f2u32(max(frag_xy.x, 0.0)), (vw - 1u));
    uint py = min(naga_f2u32(max(frag_xy.y, 0.0)), (vh - 1u));
    return uint2(naga_div(px, TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX), naga_div(py, TILE_SIZEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX));
}

uint cluster_id_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(float2 clip_xy, float3 world_pos_5_, float4 view_space_z_coeffs, float4 view_space_z_coeffs_right, uint view_index, uint viewport_w_1_, uint viewport_h_1_, uint cluster_count_x, uint cluster_count_y, uint cluster_count_z_2_, float near_clip_3_, float far_clip_3_)
{
    uint count_x = max(cluster_count_x, 1u);
    uint count_y = max(cluster_count_y, 1u);
    uint count_z = max(cluster_count_z_2_, 1u);
    const uint _e18 = view_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_index);
    float4 z_coeffs = ((_e18 != 0u) ? view_space_z_coeffs_right : view_space_z_coeffs);
    float view_z_2_ = (dot(z_coeffs.xyz, world_pos_5_) + z_coeffs.w);
    const uint _e26 = cluster_z_from_view_zX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(view_z_2_, near_clip_3_, far_clip_3_, count_z);
    const uint2 _e27 = cluster_xy_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(clip_xy, viewport_w_1_, viewport_h_1_);
    uint cx = min(_e27.x, (count_x - 1u));
    uint cy = min(_e27.y, (count_y - 1u));
    uint local_id = (cx + (count_x * (cy + (count_y * _e26))));
    uint cluster_offset = (((_e18 * count_x) * count_y) * count_z);
    return (cluster_offset + local_id);
}

float srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float value_1_)
{
    if ((value_1_ <= SRGB_LINEAR_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX)) {
        return (value_1_ * SRGB_LINEAR_INV_SLOPEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
    }
    if ((value_1_ < 1.0)) {
        float normalized = ((value_1_ + SRGB_NONLINEAR_OFFSETX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX) * SRGB_NONLINEAR_INV_SCALEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
        return pow(normalized, SRGB_NONLINEAR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
    }
    return pow(value_1_, SRGB_HDR_EXPONENTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
}

float3 srgb_light_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float3 color)
{
    const float _e2 = srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color.x);
    const float _e4 = srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color.y);
    const float _e6 = srgb_light_channel_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(color.z);
    return float3(_e2, _e4, _e6);
}

float3 light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light)
{
    const float3 _e4 = srgb_light_to_linearX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX((light.color * light.intensity));
    return _e4;
}

float direct_light_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX()
{
    return INTENSITY_BOOSTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX;
}

float squared_edge_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float t_9_)
{
    float fade = clamp((1.0 - t_9_), 0.0, 1.0);
    return (fade * fade);
}

float range_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float t_10_)
{
    float t2_ = (t_10_ * t_10_);
    const float _e4 = squared_edge_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(((t2_ * t2_) * t2_));
    return _e4;
}

float distance_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float dist, float range_1_)
{
    if ((range_1_ <= 0.0)) {
        return 0.0;
    }
    float t_13_ = (dist / range_1_);
    float t2_1_ = (t_13_ * t_13_);
    float lut = (1.0 / (1.0 + (BIRP_ATTENUATION_QUADRATICX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX * t2_1_)));
    const float _e13 = range_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(t_13_);
    return (lut * _e13);
}

float distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float dist_1_, float range_2_)
{
    const float _e2 = distance_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(dist_1_, range_2_);
    return (_e2 * INTENSITY_BOOSTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX);
}

float3 normalized_spot_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(float3 direction_1_)
{
    float len_sq_1_ = dot(direction_1_, direction_1_);
    if ((len_sq_1_ <= 1e-16)) {
        return float3(0.0, 0.0, 1.0);
    }
    return (direction_1_ * rsqrt(len_sq_1_));
}

float spot_angle_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_1_, float3 l)
{
    const float3 _e3 = normalized_spot_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_1_.direction);
    float rho = dot(-(l), _e3);
    if ((rho <= light_1_.spot_cos_half_angle)) {
        return 0.0;
    }
    if ((light_1_.spot_angle_scale <= 0.0)) {
        return ((light_1_.spot_cos_half_angle <= 0.0) ? 1.0 : 0.0);
    }
    float rho2_ = max((rho * rho), 1e-6);
    float tan2_theta = (max((1.0 - rho2_), 0.0) / rho2_);
    float r2_ = clamp((tan2_theta * light_1_.spot_angle_scale), 0.0, 1.0);
    const float _e33 = squared_edge_fadeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(((r2_ * r2_) * r2_));
    return _e33;
}

uint wrap_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uint bits_2_, uint shift)
{
    return ((bits_2_ >> shift) & LIGHT_COOKIE_WRAP_MODE_MASKX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
}

float mirror_repeat_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(float coord_1)
{
    return (1.0 - abs(((frac((coord_1 * 0.5)) * 2.0) - 1.0)));
}

float mirror_once_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(float coord_1_)
{
    return clamp(abs(coord_1_), 0.0, 1.0);
}

float wrap_cookie_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(float coord_2_, uint mode)
{
    if ((mode == LIGHT_COOKIE_WRAP_MODE_CLAMPX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        return clamp(coord_2_, 0.0, 1.0);
    }
    if ((mode == LIGHT_COOKIE_WRAP_MODE_MIRRORX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        const float _e9 = mirror_repeat_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord_2_);
        return _e9;
    }
    if ((mode == LIGHT_COOKIE_WRAP_MODE_MIRROR_ONCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        const float _e12 = mirror_once_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(coord_2_);
        return _e12;
    }
    return frac(coord_2_);
}

float2 wrap_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(float2 uv_4_, uint bits_3_)
{
    const uint _e3 = wrap_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(bits_3_, LIGHT_COOKIE_WRAP_U_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
    const float _e5 = wrap_cookie_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_4_.x, _e3);
    const uint _e7 = wrap_modeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(bits_3_, LIGHT_COOKIE_WRAP_V_SHIFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
    const float _e9 = wrap_cookie_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_4_.y, _e7);
    return float2(_e5, _e9);
}

float2 atlas_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uint rect_index, float2 uv_5_)
{
    float4 rect = asfloat(light_cookie_rectsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(0+rect_index*16));
    return (rect.xy + (uv_5_ * rect.zw));
}

float sample_2d_cookieX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_2_, float2 uv_6_)
{
    const float2 _e3 = wrap_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(uv_6_, light_2_.cookie_reserved);
    const float2 _e5 = atlas_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_2_.cookie_layer, _e3);
    float4 _e8_ = light_cookie_2d_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Sample(light_cookie_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e5);
    return _e8_.x;
}

float spot_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_3_, float3 world_pos_6_)
{
    bool local_13_ = (bool)0;

    float tan_half = light_3_.cookie_right_tan_half_angle.w;
    if ((tan_half <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX)) {
        return 1.0;
    }
    float3 from_light = (world_pos_6_ - light_3_.position);
    float3 local_33_ = float3(dot(from_light, light_3_.cookie_right_tan_half_angle.xyz), dot(from_light, light_3_.cookie_up.xyz), dot(from_light, light_3_.direction));
    if ((local_33_.z <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX)) {
        return 0.0;
    }
    float2 uv_9_ = (((local_33_.xy / ((local_33_.z * tan_half)).xx) * 0.5) + (0.5).xx);
    if (!(any((uv_9_ < (0.0).xx)))) {
        local_13_ = any((uv_9_ > (1.0).xx));
    } else {
        local_13_ = true;
    }
    bool _e44_1 = local_13_;
    if (_e44_1) {
        return 0.0;
    }
    const float _e45 = sample_2d_cookieX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_3_, uv_9_);
    return _e45;
}

float3 cube_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(float3 dir)
{
    bool local_14_ = (bool)0;

    float3 ad = abs(dir);
    if ((ad.x >= ad.y)) {
        local_14_ = (ad.x >= ad.z);
    } else {
        local_14_ = false;
    }
    bool _e11_2 = local_14_;
    if (_e11_2) {
        if ((dir.x >= 0.0)) {
            return float3((((float2(-(dir.z), -(dir.y)) / (ad.x).xx) * 0.5) + (0.5).xx), 0.0);
        }
        return float3((((float2(dir.z, -(dir.y)) / (ad.x).xx) * 0.5) + (0.5).xx), 1.0);
    }
    if ((ad.y >= ad.z)) {
        if ((dir.y >= 0.0)) {
            return float3((((float2(dir.x, dir.z) / (ad.y).xx) * 0.5) + (0.5).xx), 2.0);
        }
        return float3((((float2(dir.x, -(dir.z)) / (ad.y).xx) * 0.5) + (0.5).xx), 3.0);
    }
    if ((dir.z >= 0.0)) {
        return float3((((float2(dir.x, -(dir.y)) / (ad.z).xx) * 0.5) + (0.5).xx), 4.0);
    }
    return float3((((float2(-(dir.x), -(dir.y)) / (ad.z).xx) * 0.5) + (0.5).xx), 5.0);
}

float point_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_4_, float3 world_pos_7_)
{
    float3 from_light_1_ = (world_pos_7_ - light_4_.position);
    float len_sq_2_ = dot(from_light_1_, from_light_1_);
    if ((len_sq_2_ <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX)) {
        return 1.0;
    }
    float3 local_34_ = (float3(dot(from_light_1_, light_4_.cookie_right_tan_half_angle.xyz), dot(from_light_1_, light_4_.cookie_up.xyz), dot(from_light_1_, light_4_.direction)) * rsqrt(len_sq_2_));
    const float3 _e19 = cube_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(local_34_);
    uint rect_index_1_ = (light_4_.cookie_layer + naga_f2u32(_e19.z));
    const float2 _e25 = atlas_cookie_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(rect_index_1_, _e19.xy);
    float4 _e28_1 = light_cookie_point_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Sample(light_cookie_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e25);
    return _e28_1.x;
}

float directional_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_5_, float3 world_pos_8_)
{
    float cookie_size = light_5_.cookie_right_tan_half_angle.w;
    if ((cookie_size <= COOKIE_PROJECTION_EPSILONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX)) {
        return 1.0;
    }
    float3 from_light_2_ = (world_pos_8_ - light_5_.position);
    float2 uv_10_ = ((float2(dot(from_light_2_, light_5_.cookie_right_tan_half_angle.xyz), dot(from_light_2_, light_5_.cookie_up.xyz)) / (cookie_size).xx) + (0.5).xx);
    const float _e21 = sample_2d_cookieX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_5_, uv_10_);
    return _e21;
}

float multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_6_, float3 world_pos_9_)
{
    if ((light_6_.cookie_kind == LIGHT_COOKIE_KIND_SPOT_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        const float _e5 = spot_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6_, world_pos_9_);
        return _e5;
    }
    if ((light_6_.cookie_kind == LIGHT_COOKIE_KIND_POINT_CUBEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        const float _e9 = point_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6_, world_pos_9_);
        return _e9;
    }
    if ((light_6_.cookie_kind == LIGHT_COOKIE_KIND_DIRECTIONAL_2DX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)) {
        const float _e13 = directional_cookie_multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_6_, world_pos_9_);
        return _e13;
    }
    return 1.0;
}

bool has_shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_7_)
{
    bool local_15_ = (bool)0;
    bool local_16_ = (bool)0;

    if ((light_7_.shadow_view_count > 0u)) {
        local_15_ = (light_7_.shadow_strength > 0.0);
    } else {
        local_15_ = false;
    }
    bool _e10_ = local_15_;
    if (_e10_) {
        local_16_ = (light_7_.shadow_type != 0u);
    } else {
        local_16_ = false;
    }
    bool _e17_2 = local_16_;
    return _e17_2;
}

float3 receiver_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view, float3 world_pos_10_, float3 world_normal)
{
    bool local_17_ = (bool)0;

    float bias_1 = max(shadow_view.light_params.z, 0.0);
    float normal_len_sq = dot(world_normal, world_normal);
    if (!((bias_1 <= 0.0))) {
        local_17_ = (normal_len_sq <= 1e-12);
    } else {
        local_17_ = true;
    }
    bool _e15_ = local_17_;
    if (_e15_) {
        return world_pos_10_;
    }
    return (world_pos_10_ + (normalize(world_normal) * bias_1));
}

float radial_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_8_, float3 world_pos_11_)
{
    float range_4_ = max(light_8_.range, 0.001);
    return clamp((length((world_pos_11_ - light_8_.position.xyz)) / range_4_), 0.0, 1.0);
}

uint point_face_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(float3 direction_2_)
{
    bool local_18_ = (bool)0;

    float3 a_1_ = abs(direction_2_);
    if ((a_1_.x >= a_1_.y)) {
        local_18_ = (a_1_.x >= a_1_.z);
    } else {
        local_18_ = false;
    }
    bool _e11_3 = local_18_;
    if (_e11_3) {
        return ((direction_2_.x >= 0.0) ? 0u : 1u);
    }
    if ((a_1_.y >= a_1_.z)) {
        return ((direction_2_.y >= 0.0) ? 2u : 3u);
    }
    return ((direction_2_.z >= 0.0) ? 4u : 5u);
}

float3 point_face_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uint face)
{
    switch(naga_mod(face, 6u)) {
        case 0u: {
            return float3(1.0, 0.0, 0.0);
        }
        case 1u: {
            return float3(-1.0, 0.0, 0.0);
        }
        case 2u: {
            return float3(0.0, 1.0, 0.0);
        }
        case 3u: {
            return float3(0.0, -1.0, 0.0);
        }
        case 4u: {
            return float3(0.0, 0.0, 1.0);
        }
        default: {
            return float3(0.0, 0.0, -1.0);
        }
    }
}

float3 point_face_upX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uint face_1_)
{
    switch(naga_mod(face_1_, 6u)) {
        case 2u: {
            return float3(0.0, 0.0, -1.0);
        }
        case 3u: {
            return float3(0.0, 0.0, 1.0);
        }
        default: {
            return float3(0.0, 1.0, 0.0);
        }
    }
}

PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX ConstructPointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(float3 arg0, float3 arg1, float3 arg2) {
    PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX ret = (PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX)0;
    ret.direction = arg0;
    ret.up = arg1;
    ret.right = arg2;
    return ret;
}

PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX point_face_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uint face_2_)
{
    const float3 _e1 = point_face_directionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face_2_);
    const float3 _e2 = point_face_upX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(face_2_);
    const PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX pointfacebasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzwqylen53xgx = ConstructPointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e1, _e2, cross(_e1, _e2));
    return pointfacebasisx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzwqylen53xgx;
}

PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX ConstructPointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uint arg0, float2 arg1) {
    PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX ret = (PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX)0;
    ret.face = arg0;
    ret.local_uv = arg1;
    return ret;
}

PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(float3 direction_3_)
{
    const uint _e1 = point_face_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_3_);
    const PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX _e2 = point_face_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e1);
    float denom = max(dot(direction_3_, _e2.direction), 1e-6);
    float2 ndc_1_ = (float2(dot(direction_3_, _e2.right), dot(direction_3_, _e2.up)) / (denom).xx);
    float2 local_uv_6_ = clamp(float2(((ndc_1_.x * 0.5) + 0.5), (0.5 - (ndc_1_.y * 0.5))), (0.0).xx, (1.0).xx);
    const PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX pointshadowfaceuvx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzwqylen53xgx = ConstructPointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e1, local_uv_6_);
    return pointshadowfaceuvx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzwqylen53xgx;
}

float3 point_shadow_direction_for_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX basis, float2 local_uv)
{
    float2 ndc_2_ = float2(((local_uv.x * 2.0) - 1.0), (1.0 - (local_uv.y * 2.0)));
    return ((basis.direction + (basis.right * ndc_2_.x)) + (basis.up * ndc_2_.y));
}

float2 atlas_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_1_, float2 local_uv_1_)
{
    return (shadow_view_1_.atlas_rect.xy + (local_uv_1_ * shadow_view_1_.atlas_rect.zw));
}

float sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_2_, float2 local_uv_2_, int layer, float compare_depth)
{
    const float2 _e4 = atlas_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_2_, local_uv_2_);
    float _e7_1 = shadow_atlasX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.SampleCmp(shadow_samplerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, float3(_e4, layer), compare_depth);
    return _e7_1;
}

int naga_f2i32(float value) {
    return int(clamp(value, -2147483600.0, 2147483500.0));
}

GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX ConstructGpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float4x4 arg0, float4 arg1, float4 arg2, float4 arg3) {
    GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX ret = (GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)0;
    ret.world_to_shadow = arg0;
    ret.atlas_rect = arg1;
    ret.params = arg2;
    ret.light_params = arg3;
    return ret;
}

float sample_point_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_9_, uint start, PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX face_uv, float compare_depth_1_)
{
    uint face_8_ = min(face_uv.face, (light_9_.shadow_view_count - 1u));
    GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_8_ = ConstructGpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float4x4(asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+0+0)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+0+16)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+0+32)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+0+48))), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+64)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+80)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start + face_8_)*112+96)));
    int layer_4_ = naga_f2i32((shadow_view_8_.params.x + 0.5));
    const float _e19 = sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_8_, face_uv.local_uv, layer_4_, compare_depth_1_);
    return _e19;
}

float sample_point_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_10_, uint start_1_, float3 direction_4_, float compare_depth_2_)
{
    float sum = 0.0;
    float count = 0.0;
    int y = int(-1);
    int x = (int)0;

    const PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX _e11 = point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_4_);
    const PointFaceBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX _e13 = point_face_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e11.face);
    GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX base_shadow_view = ConstructGpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float4x4(asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+0+0)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+0+16)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+0+32)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+0+48))), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+64)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+80)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4((start_1_ + min(_e11.face, (light_10_.shadow_view_count - 1u)))*112+96)));
    float texel = max(base_shadow_view.params.y, 1e-6);
    uint2 loop_bound = uint2(4294967295u, 4294967295u);
    bool loop_init = true;
    while(true) {
        if (all(loop_bound == uint2(0u, 0u))) { break; }
        loop_bound -= uint2(loop_bound.y == 0u, 1u);
        if (!loop_init) {
            int _e52_ = y;
            y = asint(asuint(_e52_) + asuint(int(1)));
        }
        loop_init = false;
        int _e22_ = y;
        if ((_e22_ <= int(1))) {
        } else {
            break;
        }
        {
            x = int(-1);
            uint2 loop_bound_1 = uint2(4294967295u, 4294967295u);
            bool loop_init_1 = true;
            while(true) {
                if (all(loop_bound_1 == uint2(0u, 0u))) { break; }
                loop_bound_1 -= uint2(loop_bound_1.y == 0u, 1u);
                if (!loop_init_1) {
                    int _e49_1 = x;
                    x = asint(asuint(_e49_1) + asuint(int(1)));
                }
                loop_init_1 = false;
                int _e27_1 = x;
                if ((_e27_1 <= int(1))) {
                } else {
                    break;
                }
                {
                    int _e31_1 = x;
                    int _e33_ = y;
                    float2 tap_uv = (_e11.local_uv + (float2(float(_e31_1), float(_e33_)) * texel));
                    const float3 _e42 = point_shadow_direction_for_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e13, tap_uv);
                    float _e40_ = sum;
                    const PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX _e44 = point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e42);
                    const float _e45 = sample_point_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_10_, start_1_, _e44, compare_depth_2_);
                    sum = (_e40_ + _e45);
                    float _e46_ = count;
                    count = (_e46_ + 1.0);
                }
            }
        }
    }
    float _e55_ = count;
    if ((_e55_ <= 0.0)) {
        return 1.0;
    }
    float _e59_1 = sum;
    float _e60_1 = count;
    return (_e59_1 / _e60_1);
}

float sample_point_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_11_, uint start_2_, float3 direction_5_, float compare_depth_3_)
{
    const PointShadowFaceUvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX _e4 = point_shadow_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(direction_5_);
    const float _e5 = sample_point_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_11_, start_2_, _e4, compare_depth_3_);
    return _e5;
}

float sample_point_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_12_, uint start_3_, float3 direction_6_, float compare_depth_4_)
{
    if ((light_12_.shadow_type == SHADOW_TYPE_SOFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX)) {
        const float _e7 = sample_point_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_12_, start_3_, direction_6_, compare_depth_4_);
        return _e7;
    }
    const float _e8 = sample_point_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_12_, start_3_, direction_6_, compare_depth_4_);
    return _e8;
}

float point_shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_13_, uint start_4_, float3 world_pos_12_, float3 world_normal_1_)
{
    GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX base_shadow_view_1_ = ConstructGpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float4x4(asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+0+0)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+0+16)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+0+32)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+0+48))), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+64)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+80)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(start_4_*112+96)));
    const float3 _e7 = receiver_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(base_shadow_view_1_, world_pos_12_, world_normal_1_);
    float3 direction_7_ = (_e7 - light_13_.position.xyz);
    if ((dot(direction_7_, direction_7_) <= 1e-12)) {
        return 1.0;
    }
    const float _e15 = radial_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_13_, _e7);
    const float _e16 = sample_point_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_13_, start_4_, direction_7_, _e15);
    return _e16;
}

uint shadow_view_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_3_)
{
    return naga_f2u32((max(shadow_view_3_.light_params.x, 0.0) + 0.5));
}

bool radial_shadow_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uint kind)
{
    bool local_19_ = (bool)0;

    if (!((kind == SHADOW_VIEW_KIND_POINTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX))) {
        local_19_ = (kind == SHADOW_VIEW_KIND_SPOTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX);
    } else {
        local_19_ = true;
    }
    bool _e9_ = local_19_;
    return _e9_;
}

bool shadow_uv_in_boundsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(float2 uv_7_)
{
    bool local_20_ = (bool)0;
    bool local_21_ = (bool)0;
    bool local_22_ = (bool)0;

    if ((uv_7_.x >= SHADOW_UV_BORDERX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX)) {
        local_20_ = (uv_7_.x <= 0.9995);
    } else {
        local_20_ = false;
    }
    bool _e10_1 = local_20_;
    if (_e10_1) {
        local_21_ = (uv_7_.y >= SHADOW_UV_BORDERX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX);
    } else {
        local_21_ = false;
    }
    bool _e17_3 = local_21_;
    if (_e17_3) {
        local_22_ = (uv_7_.y <= 0.9995);
    } else {
        local_22_ = false;
    }
    bool _e24_1 = local_22_;
    return _e24_1;
}

float projected_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_14_, GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_4_, float3 ndc)
{
    float bias_1_ = max(light_14_.shadow_bias, shadow_view_4_.light_params.w);
    return clamp((ndc.z - bias_1_), 0.0, 1.0);
}

float sample_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_5_, float2 local_uv_3_, int layer_1_, float compare_depth_5_)
{
    float sum_1_ = 0.0;
    float count_1_ = 0.0;
    int y_1_ = int(-1);
    int x_1_ = (int)0;

    float texel_1_ = max(shadow_view_5_.params.y, 1e-6);
    uint2 loop_bound_2 = uint2(4294967295u, 4294967295u);
    bool loop_init_2 = true;
    while(true) {
        if (all(loop_bound_2 == uint2(0u, 0u))) { break; }
        loop_bound_2 -= uint2(loop_bound_2.y == 0u, 1u);
        if (!loop_init_2) {
            int _e38_ = y_1_;
            y_1_ = asint(asuint(_e38_) + asuint(int(1)));
        }
        loop_init_2 = false;
        int _e8_1 = y_1_;
        if ((_e8_1 <= int(1))) {
        } else {
            break;
        }
        {
            x_1_ = int(-1);
            uint2 loop_bound_3 = uint2(4294967295u, 4294967295u);
            bool loop_init_3 = true;
            while(true) {
                if (all(loop_bound_3 == uint2(0u, 0u))) { break; }
                loop_bound_3 -= uint2(loop_bound_3.y == 0u, 1u);
                if (!loop_init_3) {
                    int _e35_1 = x_1_;
                    x_1_ = asint(asuint(_e35_1) + asuint(int(1)));
                }
                loop_init_3 = false;
                int _e13_2 = x_1_;
                if ((_e13_2 <= int(1))) {
                } else {
                    break;
                }
                {
                    int _e17_4 = x_1_;
                    int _e19_2 = y_1_;
                    float2 tap_uv_1_ = (local_uv_3_ + (float2(float(_e17_4), float(_e19_2)) * texel_1_));
                    const bool _e29 = shadow_uv_in_boundsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(tap_uv_1_);
                    if (_e29) {
                        float _e26_ = sum_1_;
                        const float _e31 = sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_5_, tap_uv_1_, layer_1_, compare_depth_5_);
                        sum_1_ = (_e26_ + _e31);
                        float _e32_ = count_1_;
                        count_1_ = (_e32_ + 1.0);
                    }
                }
            }
        }
    }
    float _e41_ = count_1_;
    if ((_e41_ <= 0.0)) {
        return 1.0;
    }
    float _e45_ = sum_1_;
    float _e46_1 = count_1_;
    return (_e45_ / _e46_1);
}

float sample_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_6_, float2 local_uv_4_, int layer_2_, float compare_depth_6_)
{
    const float _e4 = sample_shadow_compareX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_6_, local_uv_4_, layer_2_, compare_depth_6_);
    return _e4;
}

float sample_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_15_, GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_7_, float2 local_uv_5_, int layer_3_, float compare_depth_7_)
{
    if ((light_15_.shadow_type == SHADOW_TYPE_SOFTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX)) {
        const float _e8 = sample_soft_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_7_, local_uv_5_, layer_3_, compare_depth_7_);
        return _e8;
    }
    const float _e9 = sample_hard_shadowX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_7_, local_uv_5_, layer_3_, compare_depth_7_);
    return _e9;
}

float shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_16_, uint view_index_1_, float3 world_pos_13_, float3 world_normal_2_)
{
    bool local_23_ = (bool)0;
    bool local_24_ = (bool)0;
    float compare_depth_8_ = (float)0;

    GpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX shadow_view_9_ = ConstructGpuShadowViewX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float4x4(asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+0+0)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+0+16)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+0+32)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+0+48))), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+64)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+80)), asfloat(shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(view_index_1_*112+96)));
    const float3 _e10 = receiver_positionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_9_, world_pos_13_, world_normal_2_);
    float4 clip_1_ = mul(float4(_e10, 1.0), shadow_view_9_.world_to_shadow);
    if ((clip_1_.w <= 0.0)) {
        return -1.0;
    }
    float3 ndc_3_ = (clip_1_.xyz / (clip_1_.w).xxx);
    const uint _e23 = shadow_view_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(shadow_view_9_);
    const bool _e24 = radial_shadow_kindX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(_e23);
    if (!(_e24)) {
        if (!((ndc_3_.z < 0.0))) {
            local_24_ = (ndc_3_.z > 1.0);
        } else {
            local_24_ = true;
        }
        bool _e32_1 = local_24_;
        local_23_ = _e32_1;
    } else {
        local_23_ = false;
    }
    bool _e36_ = local_23_;
    if (_e36_) {
        return -1.0;
    }
    float2 uv_11_ = float2(((ndc_3_.x * 0.5) + 0.5), (0.5 - (ndc_3_.y * 0.5)));
    const bool _e49 = shadow_uv_in_boundsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(uv_11_);
    if (!(_e49)) {
        return -1.0;
    }
    int layer_5_ = naga_f2i32((shadow_view_9_.params.x + 0.5));
    if (_e24) {
        const float _e57 = radial_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16_, _e10);
        compare_depth_8_ = _e57;
    } else {
        const float _e58 = projected_shadow_compare_depthX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16_, shadow_view_9_, ndc_3_);
        compare_depth_8_ = _e58;
    }
    float _e61_ = compare_depth_8_;
    const float _e60 = sample_shadow_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_16_, shadow_view_9_, uv_11_, layer_5_, _e61_);
    return _e60;
}

float visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_17_, float3 world_pos_14_, float3 world_normal_3_)
{
    bool local_25_ = (bool)0;
    uint i = 0u;

    const bool _e6 = has_shadow_viewsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17_);
    if (!(_e6)) {
        return 1.0;
    }
    float strength_1_ = clamp(light_17_.shadow_strength, 0.0, 1.0);
    uint start_5_ = light_17_.shadow_view_start;
    uint count_2_ = light_17_.shadow_view_count;
    if ((light_17_.light_type == 0u)) {
        local_25_ = (count_2_ >= 6u);
    } else {
        local_25_ = false;
    }
    bool _e19_3 = local_25_;
    if (_e19_3) {
        const float _e22 = point_shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17_, start_5_, world_pos_14_, world_normal_3_);
        return lerp(1.0, ((_e22 >= 0.0) ? _e22 : 1.0), strength_1_);
    }
    uint2 loop_bound_4 = uint2(4294967295u, 4294967295u);
    bool loop_init_4 = true;
    while(true) {
        if (all(loop_bound_4 == uint2(0u, 0u))) { break; }
        loop_bound_4 -= uint2(loop_bound_4.y == 0u, 1u);
        if (!loop_init_4) {
            uint _e40_1 = i;
            i = (_e40_1 + 1u);
        }
        loop_init_4 = false;
        uint _e30_ = i;
        if ((_e30_ < count_2_)) {
        } else {
            break;
        }
        {
            uint _e32_2 = i;
            const float _e33 = shadow_layer_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_17_, (start_5_ + _e32_2), world_pos_14_, world_normal_3_);
            if ((_e33 >= 0.0)) {
                return lerp(1.0, _e33, strength_1_);
            }
        }
    }
    return 1.0;
}

float distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(float dist_2_, float range_3_)
{
    const float _e2 = distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(dist_2_, range_3_);
    return _e2;
}

LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_18_, float3 world_pos_15_, float3 world_normal_4_)
{
    LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX out_1_ = (LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX)0;

    float3 light_pos = light_18_.position.xyz;
    float3 light_dir = light_18_.direction.xyz;
    if ((light_18_.light_type == 0u)) {
        float3 to_light = (light_pos - world_pos_15_);
        float dist_sq = dot(to_light, to_light);
        float dist_3_ = sqrt(dist_sq);
        out_1_.l = (to_light / (dist_3_).xxx);
        const float _e18 = distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(dist_3_, light_18_.range);
        out_1_.attenuation = _e18;
        float _e21_3 = out_1_.attenuation;
        const float _e22 = multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_18_, world_pos_15_);
        out_1_.attenuation = (_e21_3 * _e22);
    } else {
        if ((light_18_.light_type == 1u)) {
            float dir_len_sq = dot(light_dir, light_dir);
            out_1_.l = ((dir_len_sq > 1e-16) ? (-(light_dir) / (sqrt(dir_len_sq)).xxx) : float3(0.0, 0.0, 1.0));
            const float _e41 = direct_light_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX();
            out_1_.attenuation = _e41;
            float _e44_2 = out_1_.attenuation;
            const float _e45 = multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_18_, world_pos_15_);
            out_1_.attenuation = (_e44_2 * _e45);
        } else {
            float3 to_light_1_ = (light_pos - world_pos_15_);
            float dist_sq_1_ = dot(to_light_1_, to_light_1_);
            float dist_4_ = sqrt(dist_sq_1_);
            out_1_.l = (to_light_1_ / (dist_4_).xxx);
            float3 _e54_ = out_1_.l;
            const float _e56 = spot_angle_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_18_, _e54_);
            const float _e58 = distance_attenuationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(dist_4_, light_18_.range);
            out_1_.attenuation = (_e56 * _e58);
            float _e62_ = out_1_.attenuation;
            const float _e63 = multiplierX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJWGSZ3IORPWG33PNNUWK4YX(light_18_, world_pos_15_);
            out_1_.attenuation = (_e62_ * _e63);
        }
    }
    float _e67_ = out_1_.attenuation;
    const float _e68 = visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZWQYLEN53XGX(light_18_, world_pos_15_, world_normal_4_);
    out_1_.attenuation = (_e67_ * _e68);
    LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX _e71_ = out_1_;
    const LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX lightsamplex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjwgsz3iorpwk5tbnqx = _e71_;
    return lightsamplex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjwgsz3iorpwk5tbnqx;
}

float3 signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_19_, float attenuation, float n_dot_l)
{
    const float3 _e3 = light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJRGS4TQX(light_19_);
    return ((_e3 * attenuation) * n_dot_l);
}

float3 safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 n_10_, float3 fallback_3_)
{
    if ((dot(n_10_, n_10_) <= 1e-12)) {
        return fallback_3_;
    }
    return normalize(n_10_);
}

float normal_derivative_varianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 n_11_)
{
    const float3 _e5 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_11_, float3(0.0, 0.0, 1.0));
    float3 du = ddx(_e5);
    float3 dv = ddy(_e5);
    return (SPECULAR_AA_VARIANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX * (dot(du, du) + dot(dv, dv)));
}

float filter_normal_map_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float perceptual_roughness, float variance)
{
    float clamped = clamp(perceptual_roughness, 0.0, 1.0);
    float kernel = min((2.0 * variance), 0.040000003);
    return sqrt(clamp(((clamped * clamped) + kernel), 0.0, 1.0));
}

float filter_geometric_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float perceptual_roughness_1_, float variance_1_)
{
    float clamped_1_ = clamp(perceptual_roughness_1_, 0.0, 1.0);
    float alpha_7_ = (clamped_1_ * clamped_1_);
    float kernel_1_ = min((2.0 * variance_1_), GEOMETRIC_SPECULAR_AA_THRESHOLDX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    float alpha2_ = clamp(((alpha_7_ * alpha_7_) + kernel_1_), 0.0, 1.0);
    return sqrt(sqrt(alpha2_));
}

float filter_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float perceptual_roughness_2_, float3 shading_normal, float3 geometric_normal_1_)
{
    const float _e3 = normal_derivative_varianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(shading_normal);
    const float _e4 = normal_derivative_varianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(geometric_normal_1_);
    float normal_map_variance = max((_e3 - _e4), 0.0);
    const float _e8 = filter_normal_map_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_2_, normal_map_variance);
    const float _e9 = filter_geometric_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e8, _e4);
    return _e9;
}

float3 view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 n_12_, float3 v_3_)
{
    const float3 _e6 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_12_, float3(0.0, 0.0, 1.0));
    const float3 _e11 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_3_, float3(0.0, 0.0, 1.0));
    float n_dot_v_8_ = dot(_e6, _e11);
    if ((n_dot_v_8_ < MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX)) {
        const float3 _e19 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((_e6 + (_e11 * (MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX - n_dot_v_8_))), _e11);
        return _e19;
    }
    return _e6;
}

float direct_alpha_from_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float perceptual_roughness_3_)
{
    float clamped_2_ = clamp(perceptual_roughness_3_, 0.0, 1.0);
    return max((clamped_2_ * clamped_2_), MIN_ALPHAX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
}

float direct_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float perceptual_roughness_4_)
{
    const float _e1 = direct_alpha_from_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_4_);
    return sqrt(_e1);
}

float f90_from_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 f0_)
{
    return clamp(dot(f0_, (16.666666).xxx), 0.0, 1.0);
}

int2 naga_f2i32(float2 value) {
    return int2(clamp(value, -2147483600.0, 2147483500.0));
}

uint2 NagaDimensions2D(Texture2D<float4> tex)
{
    uint4 ret;
    tex.GetDimensions(0, ret.x, ret.y, ret.z);
    return ret.xy;
}

float2 sample_ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float perceptual_roughness_5_, float n_dot_v)
{
    uint2 dims_u = NagaDimensions2D(ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX);
    float2 dims = float2(float(dims_u.x), float(dims_u.y));
    int2 max_xy = int2(asint(asuint(int(dims_u.x)) - asuint(int(1))), asint(asuint(int(dims_u.y)) - asuint(int(1))));
    float2 uv_12_ = float2(clamp(n_dot_v, 0.0, 1.0), clamp(perceptual_roughness_5_, 0.0, 1.0));
    float2 xy_2_ = ((uv_12_ * dims) - (0.5).xx);
    float2 base = floor(xy_2_);
    int2 base_i = naga_f2i32(base);
    float2 f_1_ = (xy_2_ - base);
    int2 p00_ = clamp(base_i, (int(0)).xx, max_xy);
    int2 p10_ = clamp(asint(asuint(base_i) + asuint(int2(int(1), int(0)))), (int(0)).xx, max_xy);
    int2 p01_ = clamp(asint(asuint(base_i) + asuint(int2(int(0), int(1)))), (int(0)).xx, max_xy);
    int2 p11_ = clamp(asint(asuint(base_i) + asuint(int2(int(1), int(1)))), (int(0)).xx, max_xy);
    float4 _e58_ = ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(int3(p00_, int(0)));
    float2 a_2_ = _e58_.xy;
    float4 _e62_1 = ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(int3(p10_, int(0)));
    float2 b_2_ = _e62_1.xy;
    float4 _e66_1 = ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(int3(p01_, int(0)));
    float2 c_2_ = _e66_1.xy;
    float4 _e70_ = ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(int3(p11_, int(0)));
    float2 d_1_ = _e70_.xy;
    return lerp(lerp(a_2_, b_2_, f_1_.x), lerp(c_2_, d_1_, f_1_.x), f_1_.y);
}

float3 energy_compensation_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float2 dfg, float3 f0_1_)
{
    float3 clamped_f0_ = clamp(f0_1_, (0.0).xxx, (1.0).xxx);
    return ((1.0).xxx + (clamped_f0_ * (((1.0 / max(dfg.y, 0.0001))).xxx - (1.0).xxx)));
}

float3 specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float2 dfg_1_, float3 f0_2_)
{
    float3 clamped_f0_1_ = clamp(f0_2_, (0.0).xxx, (1.0).xxx);
    const float _e7 = f90_from_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(clamped_f0_1_);
    float3 f90_6_ = (_e7).xxx;
    return ((clamped_f0_1_ * (dfg_1_.y - dfg_1_.x)) + (f90_6_ * dfg_1_.x));
}

float3 indirect_specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float2 dfg_2_, float3 f0_3_, bool enabled_5_)
{
    if (!(enabled_5_)) {
        return (0.0).xxx;
    }
    const float3 _e6 = specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(dfg_2_, f0_3_);
    return _e6;
}

float specular_ao_lagardeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float n_dot_v_1_, float visibility, float perceptual_roughness_6_)
{
    float no_v = clamp(n_dot_v_1_, 0.0, 1.0);
    float ao = clamp(visibility, 0.0, 1.0);
    float linear_roughness = (clamp(perceptual_roughness_6_, 0.0, 1.0) * clamp(perceptual_roughness_6_, 0.0, 1.0));
    float exponent = exp2(((-16.0 * linear_roughness) - 1.0));
    return clamp(((pow((no_v + ao), exponent) - 1.0) + ao), 0.0, 1.0);
}

float3 multi_bounce_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float visibility_1_, float3 albedo)
{
    float ao_1_ = clamp(visibility_1_, 0.0, 1.0);
    float3 clamped_albedo = clamp(albedo, (0.0).xxx, (1.0).xxx);
    float3 a_3_ = ((2.0404 * clamped_albedo) - (0.3324).xxx);
    float3 b_3_ = ((-4.7951 * clamped_albedo) + (0.6417).xxx);
    float3 c_3_ = ((2.7552 * clamped_albedo) + (0.6903).xxx);
    return max((ao_1_).xxx, (((((ao_1_ * a_3_) + b_3_) * ao_1_) + c_3_) * ao_1_));
}

float3 indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float n_dot_v_2_, float visibility_2_, float perceptual_roughness_7_, float3 f0_4_)
{
    const float _e4 = specular_ao_lagardeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_2_, visibility_2_, perceptual_roughness_7_);
    const float3 _e5 = multi_bounce_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e4, f0_4_);
    return _e5;
}

float3 indirect_diffuse_energy_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 specular_energy, bool enabled_6_)
{
    if (!(enabled_6_)) {
        return (1.0).xxx;
    }
    return max((0.0).xxx, ((1.0).xxx - specular_energy));
}

float3 indirect_diffuse_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float visibility_3_, float3 diffuse_color)
{
    const float3 _e2 = multi_bounce_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(visibility_3_, diffuse_color);
    return _e2;
}

float3 indirect_diffuse_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 ambient, float3 base_color_2_, float metallic, float3 specular_energy_1_, float occlusion_1_, bool glossy_reflections_enabled)
{
    const float3 _e6 = indirect_diffuse_energy_scaleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(specular_energy_1_, glossy_reflections_enabled);
    float3 diffuse_color_1_ = (base_color_2_ * (1.0 - clamp(metallic, 0.0, 1.0)));
    const float3 _e13 = indirect_diffuse_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(occlusion_1_, diffuse_color_1_);
    return (((ambient * diffuse_color_1_) * _e6) * _e13);
}

float metallic_one_minus_reflectivityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float metallic_1_)
{
    return (0.96 * (1.0 - clamp(metallic_1_, 0.0, 1.0)));
}

float unity_premultiplied_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float alpha_2_, float one_minus_reflectivity_1_)
{
    float diffuse_alpha = clamp(alpha_2_, 0.0, 1.0);
    float diffuse_visibility = clamp(one_minus_reflectivity_1_, 0.0, 1.0);
    return ((1.0 - diffuse_visibility) + (diffuse_alpha * diffuse_visibility));
}

float3 metallic_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 base_color_4_, float metallic_2_)
{
    return lerp((0.04).xxx, base_color_4_, metallic_2_);
}

DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX ConstructDirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 arg0, float3 arg1) {
    DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX ret = (DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX)0;
    ret.view = arg0;
    ret.n = arg1;
    return ret;
}

DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX direct_brdf_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 n_13_, float3 v_4_)
{
    const float3 _e6 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_4_, float3(0.0, 0.0, 1.0));
    const float3 _e7 = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_13_, _e6);
    const DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX directbrdfbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjrhezdgx = ConstructDirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e6, _e7);
    return directbrdfbasisx_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjrhezdgx;
}

LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_20_, float3 world_pos_16_, float3 world_normal_5_)
{
    const LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX _e3 = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(light_20_, world_pos_16_, world_normal_5_);
    const LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX lightsamplex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjwgsz3iorpwk5tbnqx_1 = _e3;
    return lightsamplex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjwgsz3iorpwk5tbnqx_1;
}

float pow5X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float x_2_)
{
    float x2_ = (x_2_ * x_2_);
    return ((x2_ * x2_) * x_2_);
}

float3 f_schlickX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 f0_5_, float f90_, float v_dot_h)
{
    const float _e5 = pow5X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((1.0 - v_dot_h));
    return (f0_5_ + (((f90_).xxx - f0_5_) * _e5));
}

float d_ggxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float n_dot_h, float roughness_1_)
{
    float a_4_ = (n_dot_h * roughness_1_);
    float k = (roughness_1_ / max(((1.0 - (n_dot_h * n_dot_h)) + (a_4_ * a_4_)), 1e-7));
    return min(((k * k) * 0.31830987), 65504.0);
}

float v_smith_ggx_correlatedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float n_dot_v_3_, float n_dot_l_1_, float roughness_2_)
{
    float a2_ = (roughness_2_ * roughness_2_);
    float lv = (n_dot_l_1_ * sqrt((((n_dot_v_3_ - (a2_ * n_dot_v_3_)) * n_dot_v_3_) + a2_)));
    float ll = (n_dot_v_3_ * sqrt((((n_dot_l_1_ - (a2_ * n_dot_l_1_)) * n_dot_l_1_) + a2_)));
    return (0.5 / max((lv + ll), 1e-7));
}

DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX ConstructDirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 arg0, float3 arg1, float arg2, float arg3, float arg4) {
    DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX ret = (DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX)0;
    ret.f = arg0;
    ret.specular_brdf = arg1;
    ret.n_dot_l = arg2;
    ret.n_dot_v = arg3;
    ret.l_dot_h = arg4;
    return ret;
}

DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX eval_direct_specular_lobe_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX basis_1_, float3 l_1_, float alpha_3_, float3 f0_6_, float f90_1_, float3 energy_compensation)
{
    float3 h = (float3)0;

    float3 view = basis_1_.view;
    float3 brdf_n = basis_1_.n;
    const float3 _e9 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(l_1_, brdf_n);
    float n_dot_l_4_ = max(dot(brdf_n, _e9), 0.0);
    float n_dot_v_9_ = max(dot(brdf_n, view), MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    if ((n_dot_l_4_ <= 0.0)) {
        const DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX directspecularevalx_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjrhezdgx = ConstructDirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((0.0).xxx, (0.0).xxx, n_dot_l_4_, n_dot_v_9_, 0.0);
        return directspecularevalx_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjrhezdgx;
    }
    float3 h_unorm = (view + _e9);
    h = brdf_n;
    if ((dot(h_unorm, h_unorm) > 1e-12)) {
        h = normalize(h_unorm);
    }
    float3 _e25_1 = h;
    float n_dot_h_1_ = clamp(dot(brdf_n, _e25_1), 0.0, 1.0);
    float3 _e30_1 = h;
    float v_dot_h_1_ = clamp(dot(view, _e30_1), 0.0, 1.0);
    float3 _e35_2 = h;
    float l_dot_h_1_ = clamp(dot(_e9, _e35_2), 0.0, 1.0);
    const float3 _e44 = f_schlickX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(f0_6_, f90_1_, v_dot_h_1_);
    const float _e45 = d_ggxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_h_1_, alpha_3_);
    const float _e46 = v_smith_ggx_correlatedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_9_, n_dot_l_4_, alpha_3_);
    float3 fr = max((0.0).xxx, (((_e45 * _e46) * _e44) * energy_compensation));
    const DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX directspecularevalx_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjrhezdgx_1 = ConstructDirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e44, fr, n_dot_l_4_, n_dot_v_9_, l_dot_h_1_);
    return directspecularevalx_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjrhezdgx_1;
}

float max_componentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 v_5_)
{
    return max(max(v_5_.x, v_5_.y), v_5_.z);
}

float direct_diffuse_fresnel_transmissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3 f, float3 f0_7_)
{
    const float _e7 = max_componentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(clamp(f, (0.0).xxx, (1.0).xxx));
    const float _e13 = max_componentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(clamp(f0_7_, (0.0).xxx, (1.0).xxx));
    return clamp(((1.0 - _e7) / max((1.0 - _e13), 0.0001)), 0.0, 1.0);
}

float f_schlick_scalarX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float f0_8_, float f90_2_, float cos_theta)
{
    const float _e8 = pow5X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((1.0 - clamp(cos_theta, 0.0, 1.0)));
    return (f0_8_ + ((f90_2_ - f0_8_) * _e8));
}

float fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float n_dot_v_4_, float n_dot_l_2_, float l_dot_h, float perceptual_roughness_8_)
{
    float roughness_5_ = clamp(perceptual_roughness_8_, 0.0, 1.0);
    float loh = clamp(l_dot_h, 0.0, 1.0);
    float fd90_ = (0.5 + (((2.0 * roughness_5_) * loh) * loh));
    const float _e17 = f_schlick_scalarX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(1.0, fd90_, n_dot_l_2_);
    const float _e19 = f_schlick_scalarX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(1.0, fd90_, n_dot_v_4_);
    return ((_e17 * _e19) * 0.31830987);
}

float3 signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_21_, float attenuation_1_, float n_dot_l_3_)
{
    const float3 _e3 = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX(light_21_, attenuation_1_, n_dot_l_3_);
    return _e3;
}

float3 direct_radiance_metallic_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_22_, float3 world_pos_17_, float3 shading_normal_1_, DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX basis_2_, float alpha_4_, float f90_3_, float diffuse_roughness, float metallic_3_, float3 base_color_5_, float3 f0_9_, float3 energy_compensation_1_)
{
    const LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX _e11 = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_22_, world_pos_17_, shading_normal_1_);
    const DirectSpecularEvalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX _e13 = eval_direct_specular_lobe_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(basis_2_, _e11.l, alpha_4_, f0_9_, f90_3_, energy_compensation_1_);
    if ((_e13.n_dot_l <= 0.0)) {
        return (0.0).xxx;
    }
    float3 diffuse_color_3_ = (base_color_5_ * (1.0 - metallic_3_));
    const float _e23 = direct_diffuse_fresnel_transmissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e13.f, f0_9_);
    const float _e27 = fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e13.n_dot_v, _e13.n_dot_l, _e13.l_dot_h, diffuse_roughness);
    float3 fd = ((diffuse_color_3_ * _e23) * _e27);
    const float3 _e32 = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_22_, _e11.attenuation, _e13.n_dot_l);
    return ((fd + _e13.specular_brdf) * _e32);
}

float3 diffuse_only_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_24_, float3 world_pos_19_, float3 n_14_, float3 v_6_, float roughness_3_, float3 base_color_7_, float metallic_4_)
{
    const LightSampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORPWK5TBNQX _e7 = eval_lightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_24_, world_pos_19_, n_14_);
    const float3 _e12 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(v_6_, float3(0.0, 0.0, 1.0));
    const float3 _e13 = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_14_, _e12);
    float n_dot_l_5_ = max(dot(_e13, _e7.l), 0.0);
    if ((n_dot_l_5_ <= 0.0)) {
        return (0.0).xxx;
    }
    const float3 _e24 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX((_e12 + _e7.l), _e13);
    float n_dot_v_10_ = max(dot(_e13, _e12), MIN_VIEW_FACING_N_DOT_VX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX);
    float l_dot_h_2_ = clamp(dot(_e7.l, _e24), 0.0, 1.0);
    float3 diffuse_color_5_ = (base_color_7_ * (1.0 - metallic_4_));
    const float _e36 = fd_burleyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_10_, n_dot_l_5_, l_dot_h_2_, roughness_3_);
    const float3 _e38 = signed_light_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_24_, _e7.attenuation, n_dot_l_5_);
    return ((diffuse_color_5_ * _e36) * _e38);
}

uint4 local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_40_)
{
    const uint4 _e1 = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_40_);
    return _e1;
}

uint fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_41_)
{
    const uint _e1 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_41_);
    return _e1;
}

uint reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_42_)
{
    const uint _e1 = reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_42_);
    return _e1;
}

bool has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX draw_43_)
{
    const bool _e1 = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX(draw_43_);
    return _e1;
}

uint draw_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(uint view_layer_12_)
{
    return (view_layer_12_ >> 1u);
}

float3 safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3 v_8_)
{
    float len_sq_3_ = dot(v_8_, v_8_);
    if ((len_sq_3_ <= 1e-20)) {
        return float3(0.0, 0.0, 1.0);
    }
    return (v_8_ * rsqrt(len_sq_3_));
}

CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(uint arg0, float2 arg1) {
    CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX ret = (CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX)0;
    ret.face = arg0;
    ret.uv = arg1;
    return ret;
}

CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX dir_to_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3 dir_in)
{
    bool local_26_ = (bool)0;

    const float3 _e2 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_in);
    float3 a_5_ = abs(_e2);
    if ((a_5_.x >= a_5_.y)) {
        local_26_ = (a_5_.x >= a_5_.z);
    } else {
        local_26_ = false;
    }
    bool _e12_ = local_26_;
    if (_e12_) {
        if ((_e2.x >= 0.0)) {
            const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx = ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(0u, ((float2((-(_e2.z) / a_5_.x), (-(_e2.y) / a_5_.x)) * 0.5) + (0.5).xx));
            return cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx;
        }
        const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_1 = ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(1u, ((float2((_e2.z / a_5_.x), (-(_e2.y) / a_5_.x)) * 0.5) + (0.5).xx));
        return cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_1;
    }
    if ((a_5_.y >= a_5_.z)) {
        if ((_e2.y >= 0.0)) {
            const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_2 = ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(2u, ((float2((_e2.x / a_5_.y), (_e2.z / a_5_.y)) * 0.5) + (0.5).xx));
            return cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_2;
        }
        const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_3 = ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(3u, ((float2((_e2.x / a_5_.y), (-(_e2.z) / a_5_.y)) * 0.5) + (0.5).xx));
        return cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_3;
    }
    if ((_e2.z >= 0.0)) {
        const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_4 = ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(4u, ((float2((_e2.x / a_5_.z), (-(_e2.y) / a_5_.z)) * 0.5) + (0.5).xx));
        return cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_4;
    }
    const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_5 = ConstructCubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(5u, ((float2((-(_e2.x) / a_5_.z), (-(_e2.y) / a_5_.z)) * 0.5) + (0.5).xx));
    return cubeaddressx_naga_oil_mod_xojsw4zdfojuwizj2hjuwe3b2hjrxkytfnvqxax3gnfwhizlsx_5;
}

float3 face_st_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(uint face_3_, float2 st_1_)
{
    float s_8_ = st_1_.x;
    float t_14_ = st_1_.y;
    if ((face_3_ == 0u)) {
        const float3 _e10 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3(1.0, -(t_14_), -(s_8_)));
        return _e10;
    }
    if ((face_3_ == 1u)) {
        const float3 _e16 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3(-1.0, -(t_14_), s_8_));
        return _e16;
    }
    if ((face_3_ == 2u)) {
        const float3 _e21 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3(s_8_, 1.0, t_14_));
        return _e21;
    }
    if ((face_3_ == 3u)) {
        const float3 _e27 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3(s_8_, -1.0, -(t_14_)));
        return _e27;
    }
    if ((face_3_ == 4u)) {
        const float3 _e33 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3(s_8_, -(t_14_), 1.0));
        return _e33;
    }
    const float3 _e38 = safe_normalizeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3(-(s_8_), -(t_14_), -1.0));
    return _e38;
}

float3 face_uv_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(uint face_4_, float2 uv_8_)
{
    const float3 _e7 = face_st_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_4_, ((uv_8_ * 2.0) - (1.0).xx));
    return _e7;
}

float3 face_texel_coord_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(uint face_5_, float2 coord_3_, uint face_size)
{
    uint size_2_ = max(face_size, 1u);
    float2 uv_13_ = ((coord_3_ + (0.5).xx) / (float(size_2_)).xx);
    const float3 _e11 = face_uv_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_5_, uv_13_);
    return _e11;
}

int3 canonical_texel_from_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(float3 dir_1_, uint face_size_1_)
{
    uint size_3_ = max(face_size_1_, 1u);
    const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX _e4 = dir_to_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_1_);
    int2 xy_3_ = clamp(naga_f2i32(floor((_e4.uv * float(size_3_)))), (int(0)).xx, (asint(asuint(int(size_3_)) - asuint(int(1)))).xx);
    return int3(xy_3_, int(_e4.face));
}

int3 canonical_texel_from_face_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(uint face_6_, float2 coord_4_, uint face_size_2_)
{
    const float3 _e3 = face_texel_coord_to_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_6_, coord_4_, face_size_2_);
    const int3 _e4 = canonical_texel_from_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(_e3, face_size_2_);
    return _e4;
}

float3 load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(Texture2DArray<float4> tex_1_, uint face_7_, float2 coord_5_, uint face_size_3_, uint mip_level, uint base_layer)
{
    const int3 _e6 = canonical_texel_from_face_coordX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(face_7_, coord_5_, face_size_3_);
    float4 _e12_1 = tex_1_.Load(int4(_e6.xy, asint(asuint(_e6.z) + asuint(int(base_layer))), int(mip_level)));
    return _e12_1.xyz;
}

float3 sample_bilinear_lod_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(Texture2DArray<float4> tex_2_, float3 dir_2_, uint face_size_4_, uint mip_level_1_, uint base_layer_1_)
{
    uint size_4_ = max((face_size_4_ >> mip_level_1_), 1u);
    const CubeAddressX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX _e8 = dir_to_face_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(dir_2_);
    float2 coord_6_ = ((_e8.uv * float(size_4_)) - (0.5).xx);
    float2 base_1_ = floor(coord_6_);
    float2 f_2_ = (coord_6_ - base_1_);
    const float3 _e18 = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2_, _e8.face, base_1_, size_4_, mip_level_1_, base_layer_1_);
    const float3 _e24 = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2_, _e8.face, (base_1_ + float2(1.0, 0.0)), size_4_, mip_level_1_, base_layer_1_);
    const float3 _e30 = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2_, _e8.face, (base_1_ + float2(0.0, 1.0)), size_4_, mip_level_1_, base_layer_1_);
    const float3 _e36 = load_face_coord_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_2_, _e8.face, (base_1_ + float2(1.0, 1.0)), size_4_, mip_level_1_, base_layer_1_);
    return lerp(lerp(_e18, _e24, f_2_.x), lerp(_e30, _e36, f_2_.x), f_2_.y);
}

float3 sample_trilinear_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(Texture2DArray<float4> tex_3_, float3 dir_3_, float lod, uint base_face_size, float max_lod, uint base_layer_2_)
{
    float clamped_lod = clamp(lod, 0.0, max_lod);
    float lod0f = floor(clamped_lod);
    uint lod0_ = naga_f2u32(lod0f);
    uint lod1_ = min((lod0_ + 1u), naga_f2u32(max_lod));
    const float3 _e14 = sample_bilinear_lod_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_3_, dir_3_, base_face_size, lod0_, base_layer_2_);
    const float3 _e15 = sample_bilinear_lod_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(tex_3_, dir_3_, base_face_size, lod1_, base_layer_2_);
    return lerp(_e14, _e15, (clamped_lod - lod0f));
}

float3 diffuse_from_raw_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(float3 sh_a, float3 sh_b, float3 sh_c, float3 sh_d, float3 sh_e, float3 sh_f, float3 sh_g, float3 sh_h, float3 sh_i, float3 normal_ws)
{
    float3 n_22_ = normalize(normal_ws);
    float3 sh = (((((((((sh_a * 0.2820948) + (sh_b * (0.32573503 * n_22_.y))) + (sh_c * (0.32573503 * n_22_.z))) + (sh_d * (0.32573503 * n_22_.x))) + (sh_e * ((0.27313712 * n_22_.x) * n_22_.y))) + (sh_f * ((0.27313712 * n_22_.y) * n_22_.z))) + (sh_g * (0.07884789 * (((3.0 * n_22_.z) * n_22_.z) - 1.0)))) + (sh_h * ((0.27313712 * n_22_.x) * n_22_.z))) + (sh_i * (0.13656856 * ((n_22_.x * n_22_.x) - (n_22_.y * n_22_.y)))));
    return max(sh, (0.0).xxx);
}

float3 ambient_probeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(float3 normal_ws_1_)
{
    float4 _e2_4 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_a;
    float4 _e6_2 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_b;
    float4 _e10_2 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_c;
    float4 _e14_ = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_d;
    float4 _e18_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_e;
    float4 _e22_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_f;
    float4 _e26_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_g;
    float4 _e30_2 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_h;
    float4 _e34_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.ambient_sh_i;
    const float3 _e37 = diffuse_from_raw_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(_e2_4.xyz, _e6_2.xyz, _e10_2.xyz, _e14_.xyz, _e18_1.xyz, _e22_1.xyz, _e26_1.xyz, _e30_2.xyz, _e34_1.xyz, normal_ws_1_);
    return _e37;
}

bool ambient_probe_is_validX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX()
{
    uint _e3_3 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.frame_tail.w;
    return (_e3_3 != 0u);
}

PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint view_layer_13_)
{
    const uint _e1 = draw_index_from_layerX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(view_layer_13_);
    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX _e2 = get_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e1);
    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX perdrawuniformsx_naga_oil_mod_xojsw4zdfojuwizj2hjsheylxhi5hi6lqmvzqx_1 = _e2;
    return perdrawuniformsx_naga_oil_mod_xojsw4zdfojuwizj2hjsheylxhi5hi6lqmvzqx_1;
}

bool has_indirect_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint view_layer_14_, bool enabled_7_)
{
    bool local_27_ = (bool)0;

    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX _e3 = selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_14_);
    if (enabled_7_) {
        const bool _e4 = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e3);
        local_27_ = _e4;
    } else {
        local_27_ = false;
    }
    bool _e7_2 = local_27_;
    return _e7_2;
}

float3 ambient_probe_or_zeroX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 normal_ws_2_)
{
    const bool _e1 = ambient_probe_is_validX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX();
    if (_e1) {
        const float3 _e2 = ambient_probeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(normal_ws_2_);
        return _e2;
    }
    return (0.0).xxx;
}

ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX ConstructProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 arg0, float arg1, float arg2) {
    ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX ret = (ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)0;
    ret.weighted_result = arg0;
    ret.weight_sum = arg1;
    ret.coverage = arg2;
    return ret;
}

ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX()
{
    const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx = ConstructProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX((0.0).xxx, 0.0, 0.0);
    return probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx;
}

float4 add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float4 total, ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance)
{
    bool local_28_ = (bool)0;

    if (!((importance.weight_sum < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX))) {
        local_28_ = (importance.coverage < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX);
    } else {
        local_28_ = true;
    }
    bool _e11_4 = local_28_;
    if (_e11_4) {
        return total;
    }
    float remaining = min(importance.coverage, (1.0 - total.w));
    float3 result = (importance.weighted_result / (importance.weight_sum).xxx);
    return (total + float4((result * remaining), remaining));
}

float probe_sh2_sourceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint atlas_index)
{
    if ((atlas_index == 0u)) {
        return PROBE_SH2_SOURCE_NONEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX;
    }
    float _e8_2 = asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(12+48+atlas_index*208));
    return _e8_2;
}

bool probe_has_any_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint atlas_index_1_)
{
    const float _e1 = probe_sh2_sourceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_1_);
    return (_e1 != PROBE_SH2_SOURCE_NONEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX);
}

GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX ConstructGpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float4 arg0, float4 arg1, float4 arg2, float4 arg3, float4 arg4, float4 arg5, float4 arg6, float4 arg7, float4 arg8, float4 arg9, float4 arg10, float4 arg11, float4 arg12) {
    GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX ret = (GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)0;
    ret.box_min = arg0;
    ret.box_max = arg1;
    ret.position = arg2;
    ret.params = arg3;
    ret.sh2_a = arg4;
    ret.sh2_b = arg5;
    ret.sh2_c = arg6;
    ret.sh2_d = arg7;
    ret.sh2_e = arg8;
    ret.sh2_f = arg9;
    ret.sh2_g = arg10;
    ret.sh2_h = arg11;
    ret.sh2_i = arg12;
    return ret;
}

float3 sample_probe_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint atlas_index_2_, float3 normal_ws_3_)
{
    const bool _e2 = probe_has_any_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_2_);
    if (!(_e2)) {
        return (0.0).xxx;
    }
    GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe_3_ = ConstructGpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+0)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+16)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+32)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+48)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+64)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+80)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+96)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+112)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+128)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+144)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+160)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+176)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_2_*208+192)));
    const float3 _e27 = diffuse_from_raw_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJZWQMS7MFWWE2LFNZ2AX(probe_3_.sh2_a.xyz, probe_3_.sh2_b.xyz, probe_3_.sh2_c.xyz, probe_3_.sh2_d.xyz, probe_3_.sh2_e.xyz, probe_3_.sh2_f.xyz, probe_3_.sh2_g.xyz, probe_3_.sh2_h.xyz, probe_3_.sh2_i.xyz, normal_ws_3_);
    return (_e27 * max(probe_3_.params.x, 0.0));
}

float3 sample_probe_sh2_or_ambientX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint atlas_index_3_, float3 normal_ws_4_)
{
    const bool _e2 = probe_has_any_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_3_);
    if (_e2) {
        const float3 _e3 = sample_probe_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(atlas_index_3_, normal_ws_4_);
        return _e3;
    }
    const float3 _e4 = ambient_probe_or_zeroX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(normal_ws_4_);
    return _e4;
}

float3 finalize_diffuse_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float4 total_1_, ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance_1_, uint fallback_index, float3 normal_ws_5_)
{
    const float4 _e4 = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total_1_, importance_1_);
    float remaining_weight = (1.0 - _e4.w);
    if ((remaining_weight < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        return _e4.xyz;
    }
    const float3 _e11 = sample_probe_sh2_or_ambientX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(fallback_index, normal_ws_5_);
    return (_e4.xyz + (remaining_weight * _e11));
}

float3 distance_from_aabbX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 world_pos_21_, float3 aabb_min, float3 aabb_max)
{
    return max(max((world_pos_21_ - aabb_max), (aabb_min - world_pos_21_)), (0.0).xxx);
}

float probe_blend_distanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe)
{
    return max(probe.box_min.w, 0.0);
}

float probe_edge_weightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe_1_, float3 world_pos_22_)
{
    const float3 _e6 = distance_from_aabbX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_22_, probe_1_.box_min.xyz, probe_1_.box_max.xyz);
    float outside_distance = length(_e6);
    const float _e8 = probe_blend_distanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_1_);
    if ((_e8 <= MIN_PROBE_BLEND_DISTANCEX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        return ((outside_distance > 0.0) ? 0.0 : 1.0);
    }
    return clamp((1.0 - (outside_distance / _e8)), 0.0, 1.0);
}

ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX add_diffuse_sh2_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance_2_, uint index, float3 world_pos_23_, float3 normal_ws_6_)
{
    GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe_4_ = ConstructGpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+0)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+16)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+32)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+48)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+64)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+80)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+96)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+112)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+128)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+144)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+160)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+176)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index*208+192)));
    const float _e7 = probe_edge_weightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_4_, world_pos_23_);
    if ((_e7 < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_1 = importance_2_;
        return probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_1;
    }
    const float3 _e10 = sample_probe_sh2_or_ambientX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(index, normal_ws_6_);
    const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_2 = ConstructProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX((importance_2_.weighted_result + (_e7 * _e10)), (importance_2_.weight_sum + _e7), max(importance_2_.coverage, _e7));
    return probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_2;
}

float3 indirect_diffuseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 world_pos_24_, float3 normal_ws_7_, uint view_layer_15_, bool enabled_8_)
{
    float4 total_2_ = (0.0).xxxx;
    ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance_3_ = (ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)0;
    uint i_1_ = 0u;

    if (!(enabled_8_)) {
        return (0.0).xxx;
    }
    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX _e13 = selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_15_);
    const bool _e14 = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e13);
    if (!(_e14)) {
        const float3 _e16 = ambient_probe_or_zeroX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(normal_ws_7_);
        return _e16;
    }
    const uint _e17 = reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e13);
    const uint4 _e18 = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e13);
    const uint _e19 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e13);
    const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e20 = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
    importance_3_ = _e20;
    uint2 loop_bound_5 = uint2(4294967295u, 4294967295u);
    bool loop_init_5 = true;
    while(true) {
        if (all(loop_bound_5 == uint2(0u, 0u))) { break; }
        loop_bound_5 -= uint2(loop_bound_5.y == 0u, 1u);
        if (!loop_init_5) {
            uint _e52_1 = i_1_;
            i_1_ = (_e52_1 + 1u);
        }
        loop_init_5 = false;
        uint _e19_4 = i_1_;
        if ((_e19_4 < 4u)) {
        } else {
            break;
        }
        {
            uint _e22_2 = i_1_;
            uint index_2_ = _e18[min(uint(_e22_2), 3u)];
            if ((index_2_ == 0u)) {
                float4 _e27_2 = total_2_;
                ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e28_2 = importance_3_;
                const float3 _e30 = finalize_diffuse_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e27_2, _e28_2, _e19, normal_ws_7_);
                return _e30;
            }
            uint _e31_2 = i_1_;
            if (((_e17 & (1u << _e31_2)) > 0u)) {
                float4 _e36_1 = total_2_;
                ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e37_ = importance_3_;
                const float4 _e39 = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e36_1, _e37_);
                total_2_ = _e39;
                float _e40_2 = total_2_.w;
                if (((1.0 - _e40_2) <= MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
                    float4 _e45_1 = total_2_;
                    return _e45_1.xyz;
                }
                const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e48 = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
                importance_3_ = _e48;
            }
            ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e48_ = importance_3_;
            const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e50 = add_diffuse_sh2_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e48_, index_2_, world_pos_24_, normal_ws_7_);
            importance_3_ = _e50;
        }
    }
    float4 _e54_1 = total_2_;
    ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e55_1 = importance_3_;
    const float3 _e56 = finalize_diffuse_sh2X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e54_1, _e55_1, _e19, normal_ws_7_);
    return _e56;
}

float3 dominant_reflection_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 n_16_, float3 v_9_, float perceptual_roughness_9_)
{
    float3 r = reflect(-(v_9_), n_16_);
    float roughness_6_ = clamp(perceptual_roughness_9_, 0.0, 1.0);
    float blend = (roughness_6_ * roughness_6_);
    return normalize(lerp(r, n_16_, blend));
}

float3 box_project_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe_2_, float3 world_pos_25_, float3 dir_4_, float perceptual_roughness_10_)
{
    if ((probe_2_.params.z < PROBE_FLAG_BOX_PROJECTIONX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        return dir_4_;
    }
    float projection_weight = (1.0 - smoothstep(BOX_PROJECTION_FULL_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, BOX_PROJECTION_OFF_ROUGHNESSX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX, clamp(perceptual_roughness_10_, 0.0, 1.0)));
    if ((projection_weight <= 0.0)) {
        return dir_4_;
    }
    const float _e18 = probe_blend_distanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_2_);
    float3 box_min = (probe_2_.box_min.xyz - (_e18).xxx);
    float3 box_max = (probe_2_.box_max.xyz + (_e18).xxx);
    float3 safe_dir = ((abs(dir_4_) > (1e-6).xxx) ? dir_4_ : (1e-6).xxx);
    float3 plane = ((safe_dir > (0.0).xxx) ? box_max : box_min);
    float3 t_15_ = ((plane - world_pos_25_) / safe_dir);
    float distance_ = min(t_15_.x, min(t_15_.y, t_15_.z));
    if ((distance_ <= 0.0)) {
        return dir_4_;
    }
    float3 projected = normalize(((world_pos_25_ + (safe_dir * distance_)) - probe_2_.position.xyz));
    return normalize(lerp(dir_4_, projected, projection_weight));
}

float roughness_lodX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float perceptual_roughness_11_, float max_lod_1_)
{
    float r_1_ = clamp(perceptual_roughness_11_, 0.0, 1.0);
    return clamp(((max_lod_1_ * r_1_) * (2.0 - r_1_)), 0.0, max_lod_1_);
}

uint2 NagaMipDimensions2DArray(Texture2DArray<float4> tex, uint mip_level)
{
    uint4 ret;
    tex.GetDimensions(mip_level, ret.x, ret.y, ret.z, ret.w);
    return ret.xy;
}

float3 sample_probe_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(uint atlas_index_4_, float3 world_pos_26_, float3 dir_5_, float perceptual_roughness_12_)
{
    if ((atlas_index_4_ == 0u)) {
        return (0.0).xxx;
    }
    GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe_5_ = ConstructGpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+0)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+16)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+32)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+48)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+64)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+80)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+96)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+112)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+128)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+144)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+160)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+176)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(atlas_index_4_*208+192)));
    float intensity = max(probe_5_.params.x, 0.0);
    if ((intensity <= 0.0)) {
        return (0.0).xxx;
    }
    const float3 _e19 = box_project_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_5_, world_pos_26_, dir_5_, perceptual_roughness_12_);
    const float _e24 = roughness_lodX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(perceptual_roughness_12_, max(probe_5_.params.y, 0.0));
    uint2 _e27_3 = NagaMipDimensions2DArray(reflection_probe_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, int(0));
    const float3 _e36 = sample_trilinear_baseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJUWE3B2HJRXKYTFNVQXAX3GNFWHIZLSX(reflection_probe_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX, _e19, _e24, _e27_3.x, max(probe_5_.params.y, 0.0), (atlas_index_4_ * 6u));
    return (_e36 * intensity);
}

float3 finalize_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float4 total_3_, ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance_4_, uint fallback_index_1_, float3 world_pos_27_, float3 dir_6_, float perceptual_roughness_13_)
{
    const float4 _e6 = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(total_3_, importance_4_);
    float remaining_weight_1_ = (1.0 - _e6.w);
    if ((remaining_weight_1_ < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        return _e6.xyz;
    }
    const float3 _e13 = sample_probe_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(fallback_index_1_, world_pos_27_, dir_6_, perceptual_roughness_13_);
    return (_e6.xyz + (remaining_weight_1_ * _e13));
}

ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX add_radiance_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance_5_, uint index_1_, float3 world_pos_28_, float3 dir_7_, float perceptual_roughness_14_)
{
    GpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX probe_6_ = ConstructGpuReflectionProbeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+0)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+16)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+32)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+48)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+64)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+80)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+96)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+112)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+128)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+144)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+160)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+176)), asfloat(reflection_probesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(index_1_*208+192)));
    const float _e8 = probe_edge_weightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(probe_6_, world_pos_28_);
    if ((_e8 < MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
        const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_3 = importance_5_;
        return probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_3;
    }
    const float3 _e11 = sample_probe_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(index_1_, world_pos_28_, dir_7_, perceptual_roughness_14_);
    const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_4 = ConstructProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX((importance_5_.weighted_result + (_e8 * _e11)), (importance_5_.weight_sum + _e8), max(importance_5_.coverage, _e8));
    return probeblendgroupx_naga_oil_mod_xojsw4zdfojuwizj2hjwgsz3ioruw4zz2hjzgkztmmvrxi2lpnzpxa4tpmjsxgx_4;
}

float3 indirect_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 world_pos_29_, float3 n_17_, float3 v_10_, float perceptual_roughness_15_, uint view_layer_16_, bool enabled_9_)
{
    float4 total_4_ = (0.0).xxxx;
    ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX importance_6_ = (ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)0;
    uint i_2_ = 0u;

    if (!(enabled_9_)) {
        return (0.0).xxx;
    }
    const PerDrawUniformsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HI6LQMVZQX _e15 = selected_drawX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_16_);
    const bool _e16 = has_reflection_probe_selectionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e15);
    if (!(_e16)) {
        return (0.0).xxx;
    }
    const uint _e20 = reflection_probe_importance_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e15);
    const float3 _e21 = dominant_reflection_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_17_, v_10_, perceptual_roughness_15_);
    const uint4 _e22 = local_reflection_probe_indicesX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e15);
    const uint _e23 = fallback_reflection_probe_indexX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJSHEYLXHI5HAZLSL5SHEYLXX(_e15);
    const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e24 = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
    importance_6_ = _e24;
    uint2 loop_bound_6 = uint2(4294967295u, 4294967295u);
    bool loop_init_6 = true;
    while(true) {
        if (all(loop_bound_6 == uint2(0u, 0u))) { break; }
        loop_bound_6 -= uint2(loop_bound_6.y == 0u, 1u);
        if (!loop_init_6) {
            uint _e56_ = i_2_;
            i_2_ = (_e56_ + 1u);
        }
        loop_init_6 = false;
        uint _e23_ = i_2_;
        if ((_e23_ < 4u)) {
        } else {
            break;
        }
        {
            uint _e26_2 = i_2_;
            uint index_3_ = _e22[min(uint(_e26_2), 3u)];
            if ((index_3_ == 0u)) {
                float4 _e31_3 = total_4_;
                ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e32_3 = importance_6_;
                const float3 _e34 = finalize_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e31_3, _e32_3, _e23, world_pos_29_, _e21, perceptual_roughness_15_);
                return _e34;
            }
            uint _e36_2 = i_2_;
            if (((_e20 & (1u << _e36_2)) > 0u)) {
                float4 _e41_1 = total_4_;
                ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e42_ = importance_6_;
                const float4 _e43 = add_importance_to_totalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e41_1, _e42_);
                total_4_ = _e43;
                float _e45_2 = total_4_.w;
                if (((1.0 - _e45_2) <= MIN_PROBE_BLEND_WEIGHTX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX)) {
                    float4 _e50_ = total_4_;
                    return _e50_.xyz;
                }
                const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e52 = empty_probe_blend_groupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX();
                importance_6_ = _e52;
            }
            ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e53_ = importance_6_;
            const ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e54 = add_radiance_contribution_to_importanceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e53_, index_3_, world_pos_29_, _e21, perceptual_roughness_15_);
            importance_6_ = _e54;
        }
    }
    float4 _e58_1 = total_4_;
    ProbeBlendGroupX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX _e59_2 = importance_6_;
    const float3 _e60 = finalize_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(_e58_1, _e59_2, _e23, world_pos_29_, _e21, perceptual_roughness_15_);
    return _e60;
}

float3 horizon_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 n_18_, float3 geometric_n)
{
    if ((dot(geometric_n, geometric_n) <= 1e-12)) {
        return normalize(n_18_);
    }
    return normalize(geometric_n);
}

float horizon_specular_occlusionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 n_19_, float3 geometric_n_1_, float3 v_11_, float perceptual_roughness_16_)
{
    const float3 _e4 = dominant_reflection_dirX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_19_, v_11_, perceptual_roughness_16_);
    const float3 _e5 = horizon_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_19_, geometric_n_1_);
    float horizon = clamp((1.0 + dot(_e4, _e5)), 0.0, 1.0);
    return (horizon * horizon);
}

float3 indirect_specular_with_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(float3 world_pos_30_, float3 n_20_, float3 geometric_n_2_, float3 v_12_, float perceptual_roughness_17_, float3 specular_energy_3_, float specular_occlusion, bool enabled_10_, uint view_layer_17_)
{
    const float3 _e9 = indirect_radianceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_30_, n_20_, v_12_, perceptual_roughness_17_, view_layer_17_, enabled_10_);
    const float _e10 = horizon_specular_occlusionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(n_20_, geometric_n_2_, v_12_, perceptual_roughness_17_);
    return (((_e9 * specular_energy_3_) * clamp(specular_occlusion, 0.0, 1.0)) * _e10);
}

float3 direct_energy_compensationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float perceptual_roughness_18_, float n_dot_v_5_, float3 f0_11_, bool enabled_11_)
{
    if (!(enabled_11_)) {
        return (1.0).xxx;
    }
    const float _e7 = direct_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_18_);
    const float2 _e8 = sample_ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e7, n_dot_v_5_);
    const float3 _e9 = energy_compensation_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e8, f0_11_);
    return _e9;
}

uint cluster_id_for_fragmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float2 frag_xy_1_, float3 world_pos_31_, uint view_layer_18_)
{
    float4 _e2_5 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_space_z_coeffs;
    float4 _e5_ = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.view_space_z_coeffs_right;
    uint _e8_3 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_width;
    uint _e11_5 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.viewport_height;
    uint _e14_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.cluster_count_x;
    uint _e17_5 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.cluster_count_y;
    uint _e20_ = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.cluster_count_z;
    float _e23_1 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.near_clip;
    float _e26_3 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.far_clip;
    const uint _e30 = cluster_id_from_fragX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(frag_xy_1_, world_pos_31_, _e2_5, _e5_, view_layer_18_, _e8_3, _e11_5, _e14_1, _e17_5, _e20_, _e23_1, _e26_3);
    return _e30;
}

bool light_enabled_for_optionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(uint light_type, ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX options)
{
    bool local_29_ = (bool)0;
    bool local_30_ = (bool)0;
    bool local_31_ = (bool)0;

    bool is_directional = (light_type == 1u);
    if (is_directional) {
        local_29_ = !(options.include_directional);
    } else {
        local_29_ = false;
    }
    bool _e9_1 = local_29_;
    if (!(_e9_1)) {
        if (!(is_directional)) {
            local_31_ = !(options.include_local);
        } else {
            local_31_ = false;
        }
        bool _e17_6 = local_31_;
        local_30_ = _e17_6;
    } else {
        local_30_ = true;
    }
    bool _e21_4 = local_30_;
    return !(_e21_4);
}

GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX ConstructGpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(float3 arg0, float arg1, float3 arg2, float arg3, float3 arg4, float arg5, float arg6, float arg7, uint arg8, float arg9, float arg10, float arg11, float arg12, float arg13, uint arg14, uint arg15, uint arg16, uint arg17, uint arg18, uint arg19, uint arg20, uint arg21, float4 arg22, float4 arg23) {
    GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX ret = (GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX)0;
    ret.position = arg0;
    ret.align_pad_vec3_pos = arg1;
    ret.direction = arg2;
    ret.align_pad_vec3_dir = arg3;
    ret.color = arg4;
    ret.intensity = arg5;
    ret.range = arg6;
    ret.spot_cos_half_angle = arg7;
    ret.light_type = arg8;
    ret.spot_angle_scale = arg9;
    ret.shadow_strength = arg10;
    ret.shadow_near_plane = arg11;
    ret.shadow_bias = arg12;
    ret.shadow_normal_bias = arg13;
    ret.shadow_type = arg14;
    ret.shadow_view_start = arg15;
    ret.shadow_view_count = arg16;
    ret.shadow_flags = arg17;
    ret.shadow_map_resolution = arg18;
    ret.cookie_kind = arg19;
    ret.cookie_layer = arg20;
    ret.cookie_reserved = arg21;
    ret.cookie_right_tan_half_angle = arg22;
    ret.cookie_up = arg23;
    return ret;
}

float3 direct_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float2 frag_xy_2_, float3 world_pos_32_, uint view_layer_19_, MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX s_2_, float3 view_dir, float filtered_roughness, float3 specular_color, float3 energy_compensation_3_, ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX options_1_)
{
    DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX basis_4_ = ConstructDirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(float3(0.0, 0.0, 1.0), float3(0.0, 0.0, 1.0));
    float alpha_6_ = 0.0;
    float f90_5_ = 0.0;
    float3 lo = (0.0).xxx;
    uint i_3_ = 0u;

    const uint _e28 = cluster_id_for_fragmentX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_2_, world_pos_32_, view_layer_19_);
    const uint2 _e29 = cluster_light_range_atX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(_e28);
    uint count_3_ = _e29.y;
    if ((count_3_ == 0u)) {
        return (0.0).xxx;
    }
    if (options_1_.specular_highlights_enabled) {
        const DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX _e37 = direct_brdf_basisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_2_.normal, view_dir);
        basis_4_ = _e37;
        const float _e38 = direct_alpha_from_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(filtered_roughness);
        alpha_6_ = _e38;
        const float _e39 = f90_from_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(specular_color);
        f90_5_ = _e39;
    }
    uint2 loop_bound_7 = uint2(4294967295u, 4294967295u);
    bool loop_init_7 = true;
    while(true) {
        if (all(loop_bound_7 == uint2(0u, 0u))) { break; }
        loop_bound_7 -= uint2(loop_bound_7.y == 0u, 1u);
        if (!loop_init_7) {
            uint _e65_1 = i_3_;
            i_3_ = (_e65_1 + 1u);
        }
        loop_init_7 = false;
        uint _e30_3 = i_3_;
        if ((_e30_3 < count_3_)) {
        } else {
            break;
        }
        {
            uint _e32_4 = i_3_;
            const uint _e43 = cluster_light_index_from_rangeX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRWY5LTORSXEX(_e29, _e32_4);
            uint _e36_3 = frameX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.light_count;
            if ((_e43 >= _e36_3)) {
                continue;
            }
            GpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX light_26_ = ConstructGpuLightX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DU5DZOBSXGX(asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load3(_e43*144+0)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+12)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load3(_e43*144+16)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+28)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load3(_e43*144+32)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+44)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+48)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+52)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+56)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+60)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+64)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+68)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+72)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+76)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+80)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+84)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+88)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+92)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+96)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+100)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+104)), asuint(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load(_e43*144+108)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(_e43*144+112)), asfloat(lightsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX.Load4(_e43*144+128)));
            const bool _e52 = light_enabled_for_optionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(light_26_.light_type, options_1_);
            if (!(_e52)) {
                continue;
            }
            if (options_1_.specular_highlights_enabled) {
                float3 _e46_2 = lo;
                DirectBrdfBasisX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX _e48_1 = basis_4_;
                float _e49_2 = alpha_6_;
                float _e50_1 = f90_5_;
                const float3 _e63 = direct_radiance_metallic_preparedX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_26_, world_pos_32_, s_2_.normal, _e48_1, _e49_2, _e50_1, s_2_.roughness, s_2_.metallic, s_2_.base_color, specular_color, energy_compensation_3_);
                lo = (_e46_2 + _e63);
            } else {
                float3 _e57_ = lo;
                const float3 _e70 = diffuse_only_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(light_26_, world_pos_32_, s_2_.normal, view_dir, s_2_.roughness, s_2_.base_color, s_2_.metallic);
                lo = (_e57_ + _e70);
            }
        }
    }
    float3 _e67_1 = lo;
    return _e67_1;
}

float3 indirect_specular_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float perceptual_roughness_19_, float n_dot_v_6_, float3 f0_12_, bool enabled_12_)
{
    if (!(enabled_12_)) {
        return (0.0).xxx;
    }
    const float2 _e7 = sample_ibl_dfg_lutX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(perceptual_roughness_19_, n_dot_v_6_);
    const float3 _e9 = indirect_specular_energy_from_dfgX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e7, f0_12_, true);
    return _e9;
}

float3 indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float n_dot_v_7_, float occlusion_3_, float perceptual_roughness_20_, float3 f0_13_, bool enabled_13_)
{
    if (!(enabled_13_)) {
        return (0.0).xxx;
    }
    const float3 _e8 = indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(n_dot_v_7_, occlusion_3_, perceptual_roughness_20_, f0_13_);
    return _e8;
}

float3 indirect_specular_contributionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float3 world_pos_33_, float3 normal_1_, float3 geometric_normal_2_, float3 view_dir_1_, float perceptual_roughness_21_, float3 specular_energy_4_, float3 specular_visibility, bool enabled_14_, uint view_layer_20_)
{
    if (!(enabled_14_)) {
        return (0.0).xxx;
    }
    const float3 _e15 = indirect_specular_with_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_33_, normal_1_, geometric_normal_2_, view_dir_1_, perceptual_roughness_21_, (specular_energy_4_ * specular_visibility), 1.0, true, view_layer_20_);
    return _e15;
}

float3 shade_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float2 frag_xy_3_, float3 world_pos_34_, uint view_layer_21_, MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX s_3_, ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX options_2_)
{
    const float3 _e5 = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_34_, view_layer_21_);
    const float3 _e7 = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_3_.normal, _e5);
    const float3 _e10 = metallic_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_3_.base_color, s_3_.metallic);
    float n_dot_v_12_ = clamp(dot(_e7, _e5), 0.0, 1.0);
    const float _e18 = filter_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_3_.roughness, s_3_.normal, s_3_.geometric_normal);
    const float3 _e21 = direct_energy_compensationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_3_.roughness, n_dot_v_12_, _e10, options_2_.specular_highlights_enabled);
    const float3 _e22 = direct_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_3_, world_pos_34_, view_layer_21_, s_3_, _e5, _e18, _e10, _e21, options_2_);
    const bool _e24 = has_indirect_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_21_, options_2_.glossy_reflections_enabled);
    const float3 _e25 = indirect_specular_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(_e18, n_dot_v_12_, _e10, _e24);
    const float3 _e27 = indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(n_dot_v_12_, s_3_.occlusion, _e18, _e10, _e24);
    const float3 _e30 = indirect_diffuseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_34_, s_3_.normal, view_layer_21_, options_2_.include_directional);
    const float3 _e34 = indirect_diffuse_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e30, s_3_.base_color, s_3_.metallic, _e25, s_3_.occlusion, _e24);
    const float3 _e36 = indirect_specular_contributionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(world_pos_34_, _e7, s_3_.geometric_normal, _e5, _e18, _e25, _e27, _e24, view_layer_21_);
    float3 extra = (options_2_.include_directional ? s_3_.emission : (0.0).xxx);
    return (((_e34 + _e36) + _e22) + extra);
}

MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX premultiplied_metallic_surfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX s_4_)
{
    float diffuse_alpha_1_ = clamp(s_4_.alpha, 0.0, 1.0);
    const MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX metallicsurfacex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjzxk4tgmfrwkx_1 = ConstructMetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX((s_4_.base_color * diffuse_alpha_1_), s_4_.alpha, s_4_.metallic, s_4_.roughness, s_4_.occlusion, s_4_.normal, s_4_.geometric_normal, s_4_.emission);
    return metallicsurfacex_naga_oil_mod_xojsw4zdfojuwizj2hjyge4z2hjzxk4tgmfrwkx_1;
}

float4 shade_metallic_transparent_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(float2 frag_xy_4_, float3 world_pos_35_, uint view_layer_22_, MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX s_5_, ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX options_3_)
{
    const MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX _e5 = premultiplied_metallic_surfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_5_);
    const float3 _e6 = view_dir_for_world_posX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJTHEYLNMU5DUZ3MN5RGC3DTX(world_pos_35_, view_layer_22_);
    const float3 _e8 = view_facing_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5_.normal, _e6);
    const float3 _e11 = metallic_f0X_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5_.base_color, s_5_.metallic);
    float n_dot_v_13_ = clamp(dot(_e8, _e6), 0.0, 1.0);
    const float _e19 = filter_perceptual_roughnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5_.roughness, s_5_.normal, s_5_.geometric_normal);
    const float3 _e22 = direct_energy_compensationX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(s_5_.roughness, n_dot_v_13_, _e11, options_3_.specular_highlights_enabled);
    const float3 _e23 = direct_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_xy_4_, world_pos_35_, view_layer_22_, _e5, _e6, _e19, _e11, _e22, options_3_);
    const bool _e25 = has_indirect_specularX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(view_layer_22_, options_3_.glossy_reflections_enabled);
    const float3 _e26 = indirect_specular_energyX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(_e19, n_dot_v_13_, _e11, _e25);
    const float3 _e28 = indirect_specular_visibilityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(n_dot_v_13_, s_5_.occlusion, _e19, _e11, _e25);
    const float3 _e31 = indirect_diffuseX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWGSZ3IORUW4ZZ2HJZGKZTMMVRXI2LPNZPXA4TPMJSXGX(world_pos_35_, s_5_.normal, view_layer_22_, options_3_.include_directional);
    const float3 _e35 = indirect_diffuse_metallicX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(_e31, _e5.base_color, s_5_.metallic, _e26, s_5_.occlusion, _e25);
    const float3 _e37 = indirect_specular_contributionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(world_pos_35_, _e8, s_5_.geometric_normal, _e6, _e19, _e26, _e28, _e25, view_layer_22_);
    float3 extra_1_ = (options_3_.include_directional ? s_5_.emission : (0.0).xxx);
    const float _e44 = metallic_one_minus_reflectivityX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5_.metallic);
    const float _e46 = unity_premultiplied_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJRHEZDGX(s_5_.alpha, _e44);
    return float4((((_e35 + _e37) + _e23) + extra_1_), _e46);
}

bool pbs_kw(uint mask_1_)
{
    uint _e2_6 = mat._RenderideVariantBits;
    const bool _e4 = enabledX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWC5DFOJUWC3B2HJ3GC4TJMFXHIX3CNF2HGX(_e2_6, mask_1_);
    return _e4;
}

bool alpha_test_enabled()
{
    const bool _e1 = pbs_kw(PBSMETALLIC_KW_ALPHATEST_ON);
    return _e1;
}

bool alpha_premultiply_enabled()
{
    const bool _e1 = pbs_kw(PBSMETALLIC_KW_ALPHAPREMULTIPLY_ON);
    return _e1;
}

bool specular_highlights_enabled()
{
    const bool _e1 = pbs_kw(PBSMETALLIC_KW_SPECULARHIGHLIGHTS_OFF);
    return !(_e1);
}

bool glossy_reflections_enabled_2_()
{
    const bool _e1 = pbs_kw(PBSMETALLIC_KW_GLOSSYREFLECTIONS_OFF);
    return !(_e1);
}

bool metallic_gloss_map_enabled()
{
    const bool _e1 = pbs_kw(PBSMETALLIC_KW_METALLICGLOSSMAP);
    return _e1;
}

bool smoothness_from_albedo_alpha_1_()
{
    const bool _e1 = pbs_kw(PBSMETALLIC_KW_SMOOTHNESS_TEXTURE_ALBEDO_CHANNEL_A);
    return _e1;
}

SurfaceData ConstructSurfaceData(float3 arg0, float arg1, float arg2, float arg3, float arg4, float3 arg5, float3 arg6) {
    SurfaceData ret = (SurfaceData)0;
    ret.base_color = arg0;
    ret.alpha = arg1;
    ret.metallic = arg2;
    ret.roughness = arg3;
    ret.occlusion = arg4;
    ret.normal = arg5;
    ret.emission = arg6;
    return ret;
}

SurfaceData sample_surface(float2 uv0_3_, float2 uv1_3_, float3 world_pos_36_, float3 world_n_6_, float4 world_t_6_, uint view_layer_23_)
{
    float3 base_color_9_ = (float3)0;
    float metallic_5_ = (float)0;
    float smoothness_2_ = (float)0;

    float4 _e2_7 = mat._MainTex_ST;
    const float2 _e12 = apply_stX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HK5QX(uv0_3_, _e2_7);
    const bool _e14 = pbs_kw(PBSMETALLIC_KW_PARALLAXMAP);
    float _e9_2 = mat._Parallax;
    float _e12_2 = mat._ParallaxMap_LodBias;
    uint _e24 = mat._ParallaxMap_WrapModeBits;
    const float2 _e26 = apply_parallaxX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e12, _e14, _e9_2, world_pos_36_, world_n_6_, world_t_6_, view_layer_23_, _ParallaxMap, _e24, _ParallaxMap_sampler, _e12_2);
    float _e22_3 = mat._UVSec;
    float4 _e25_2 = mat._DetailAlbedoMap_ST;
    const float2 _e33 = detail_uvX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(uv0_3_, uv1_3_, _e22_3, _e25_2);
    float _e30_4 = mat._MainTex_LodBias;
    uint _e40 = mat._MainTex_WrapModeBits;
    const float4 _e42 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(_MainTex, _e40, _MainTex_sampler, _e26, _e30_4);
    float _e37_1 = mat._Color.w;
    const bool _e47 = smoothness_from_albedo_alpha_1_();
    const float _e49 = standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e37_1, _e42.w, _e47);
    float _e43_ = mat._Cutoff;
    const bool _e53 = alpha_test_enabled();
    clip_standard_alphaX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e49, _e43_, _e53);
    float4 _e47_ = mat._Color;
    base_color_9_ = (_e47_.xyz * _e42.xyz);
    float _e54_2 = mat._Metallic;
    metallic_5_ = _e54_2;
    float _e58_2 = mat._Glossiness;
    smoothness_2_ = _e58_2;
    float smoothness_scale = mat._GlossMapScale;
    const bool _e69 = metallic_gloss_map_enabled();
    if (_e69) {
        float _e66_2 = mat._MetallicGlossMap_LodBias;
        uint _e76 = mat._MetallicGlossMap_WrapModeBits;
        const float4 _e78 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(_MetallicGlossMap, _e76, _MetallicGlossMap_sampler, _e26, _e66_2);
        metallic_5_ = _e78.x;
        smoothness_2_ = (_e78.w * smoothness_scale);
    }
    const bool _e82 = smoothness_from_albedo_alpha_1_();
    if (_e82) {
        smoothness_2_ = (_e42.w * smoothness_scale);
    }
    float _e76_ = metallic_5_;
    metallic_5_ = clamp(_e76_, 0.0, 1.0);
    float _e80_ = smoothness_2_;
    const float _e90 = roughness_from_smoothnessX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e80_);
    float _e84_ = mat._OcclusionMap_LodBias;
    uint _e97 = mat._OcclusionMap_WrapModeBits;
    const float4 _e99 = sample_tex_2dX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJRW64TFHI5HIZLYOR2XEZK7ONQW24DMNFXGOX(_OcclusionMap, _e97, _OcclusionMap_sampler, _e26, _e84_);
    float occlusion_sample = _e99.y;
    float _e91_ = mat._OcclusionStrength;
    const float _e104 = occlusion_from_sampleX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(occlusion_sample, _e91_);
    const bool _e106 = pbs_kw(PBSMETALLIC_KW_DETAIL_MULX2_);
    float _e97_ = mat._DetailMask_LodBias;
    uint _e113 = mat._DetailMask_WrapModeBits;
    const float _e115 = sample_detail_maskX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(_e106, _DetailMask, _e113, _DetailMask_sampler, _e26, _e97_);
    float3 _e101_ = base_color_9_;
    float _e104_ = mat._DetailAlbedoMap_LodBias;
    uint _e123 = mat._DetailAlbedoMap_WrapModeBits;
    const float3 _e125 = apply_detail_albedoX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJSGK5DBNFWAX(_e101_, _e106, _e115, _DetailAlbedoMap, _e123, _DetailAlbedoMap_sampler, _e33, _e104_);
    base_color_9_ = _e125;
    const bool _e127 = pbs_kw(PBSMETALLIC_KW_NORMALMAP);
    float _e112_ = mat._BumpMap_LodBias;
    float _e115_ = mat._BumpScale;
    float _e118_ = mat._DetailNormalMap_LodBias;
    float _e121_ = mat._DetailNormalMapScale;
    uint _e143 = mat._BumpMap_WrapModeBits;
    uint _e148 = mat._DetailNormalMap_WrapModeBits;
    const float3 _e151 = sample_world_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e127, _e106, _e115, _BumpMap, _e143, _BumpMap_sampler, _e26, _e112_, _e115_, _DetailNormalMap, _e148, _DetailNormalMap_sampler, _e33, _e118_, _e121_, world_n_6_, world_t_6_, normalize(world_n_6_));
    const bool _e153 = pbs_kw(PBSMETALLIC_KW_EMISSION);
    float4 _e132_ = mat._EmissionColor;
    float _e136_ = mat._EmissionMap_LodBias;
    uint _e164 = mat._EmissionMap_WrapModeBits;
    const float3 _e166 = sample_emissionX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXIYLOMRQXEZAX(_e153, _e132_.xyz, _EmissionMap, _e164, _EmissionMap_sampler, _e26, _e136_);
    float3 _e140_ = base_color_9_;
    float _e141_ = metallic_5_;
    const SurfaceData surfacedata = ConstructSurfaceData(_e140_, _e49, _e141_, _e90, _e104, _e151, _e166);
    return surfacedata;
}

VertexOutput_vs_main vs_main(uint instance_index : SV_InstanceID, float4 pos : LOC0, float4 n : LOC1, float2 uv0_ : LOC2, float4 t : LOC4, float2 uv1_ : LOC5)
{
    const WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX _e7 = world_uv2_vertex_mainX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX(instance_index, 0u, pos, n, t, uv0_, uv1_);
    const WorldUv2VertexOutputX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJWWK43IHI5HMZLSORSXQX worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1 = _e7;
    const VertexOutput_vs_main worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1_ = { worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.world_pos, worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.world_n, worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.world_t, worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.primary_uv, worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.secondary_uv, worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.view_layer, worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1.clip_pos };
    return worlduv2vertexoutputx_naga_oil_mod_xojsw4zdfojuwizj2hjwwk43ihi5hmzlsorsxqx_1_;
}

ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX ConstructClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(bool arg0, bool arg1, bool arg2, bool arg3) {
    ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX ret = (ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX)0;
    ret.include_directional = arg0;
    ret.include_local = arg1;
    ret.specular_highlights_enabled = arg2;
    ret.glossy_reflections_enabled = arg3;
    return ret;
}

float4 fs_forward_base(FragmentInput_fs_forward_base fragmentinput_fs_forward_base) : SV_Target0
{
    float4 frag_pos = fragmentinput_fs_forward_base.frag_pos_1;
    float3 world_pos = fragmentinput_fs_forward_base.world_pos_2;
    float3 world_n = fragmentinput_fs_forward_base.world_n_2;
    float4 world_t = fragmentinput_fs_forward_base.world_t_2;
    float2 uv0_1_ = fragmentinput_fs_forward_base.uv0_1_1;
    float2 uv1_1_ = fragmentinput_fs_forward_base.uv1_1_1;
    uint view_layer = fragmentinput_fs_forward_base.view_layer_2;
    const SurfaceData _e7 = sample_surface(uv0_1_, uv1_1_, world_pos, world_n, world_t, view_layer);
    const MetallicSurfaceX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX _e15 = metallic_with_geometric_normalX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJZXK4TGMFRWKX(_e7.base_color, _e7.alpha, _e7.metallic, _e7.roughness, _e7.occlusion, _e7.normal, world_n, _e7.emission);
    const bool _e16 = specular_highlights_enabled();
    const bool _e17 = glossy_reflections_enabled_2_();
    ClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX options_4_ = ConstructClusterLightingOptionsX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(true, true, _e16, _e17);
    const bool _e21 = alpha_premultiply_enabled();
    if (_e21) {
        const float4 _e23 = shade_metallic_transparent_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_pos.xy, world_pos, view_layer, _e15, options_4_);
        return _e23;
    }
    const float3 _e25 = shade_metallic_clusteredX_naga_oil_mod_XOJSW4ZDFOJUWIZJ2HJYGE4Z2HJWGSZ3IORUW4ZYX(frag_pos.xy, world_pos, view_layer, _e15, options_4_);
    return float4(_e25, _e7.alpha);
}
