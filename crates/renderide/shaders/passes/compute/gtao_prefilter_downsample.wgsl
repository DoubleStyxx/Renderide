#import renderide::post::gtao_params as gparams
//! Compute pass: GTAO weighted view-space depth downsample.

#ifdef MULTIVIEW
@group(0) @binding(0) var src_mip: texture_2d_array<f32>;
#else
@group(0) @binding(0) var src_mip: texture_2d<f32>;
#endif
@group(0) @binding(1) var<uniform> gtao: gparams::GtaoParams;
#ifdef MULTIVIEW
@group(0) @binding(2) var dst_mip: texture_storage_2d_array<r32float, write>;
#else
@group(0) @binding(2) var dst_mip: texture_storage_2d<r32float, write>;
#endif

fn load_src(pix: vec2<i32>, src_max: vec2<i32>, layer: u32) -> f32 {
#ifdef MULTIVIEW
    return textureLoad(src_mip, clamp(pix, vec2<i32>(0), src_max), i32(layer), 0).r;
#else
    return textureLoad(src_mip, clamp(pix, vec2<i32>(0), src_max), 0).r;
#endif
}

fn depth_mip_filter(d0: f32, d1: f32, d2: f32, d3: f32) -> f32 {
    let max_depth = max(max(d0, d1), max(d2, d3));
    if (max_depth <= 0.0) {
        return 0.0;
    }

    let effect_radius = max(gtao.radius_world * gtao.radius_multiplier, 1e-4) * 0.75;
    let falloff_fraction = clamp(gtao.falloff_range, 0.05, 1.0);
    let falloff_range = max(falloff_fraction * effect_radius, 1e-4);
    let falloff_from = effect_radius * (1.0 - falloff_fraction);
    let falloff_mul = -1.0 / falloff_range;
    let falloff_add = falloff_from / falloff_range + 1.0;

    let w0 = clamp((max_depth - d0) * falloff_mul + falloff_add, 0.0, 1.0);
    let w1 = clamp((max_depth - d1) * falloff_mul + falloff_add, 0.0, 1.0);
    let w2 = clamp((max_depth - d2) * falloff_mul + falloff_add, 0.0, 1.0);
    let w3 = clamp((max_depth - d3) * falloff_mul + falloff_add, 0.0, 1.0);
    let weight_sum = max(w0 + w1 + w2 + w3, 1e-5);
    return (w0 * d0 + w1 * d1 + w2 * d2 + w3 * d3) / weight_sum;
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_dim = textureDimensions(dst_mip);
    if (gid.x >= dst_dim.x || gid.y >= dst_dim.y) {
        return;
    }

    let src_dim = textureDimensions(src_mip);
    let src_max = vec2<i32>(i32(src_dim.x) - 1, i32(src_dim.y) - 1);
    let base = vec2<i32>(i32(gid.x) * 2, i32(gid.y) * 2);
    let layer = gid.z;
    let d0 = load_src(base + vec2<i32>(0, 0), src_max, layer);
    let d1 = load_src(base + vec2<i32>(1, 0), src_max, layer);
    let d2 = load_src(base + vec2<i32>(0, 1), src_max, layer);
    let d3 = load_src(base + vec2<i32>(1, 1), src_max, layer);
#ifdef MULTIVIEW
    textureStore(
        dst_mip,
        vec2<i32>(i32(gid.x), i32(gid.y)),
        i32(layer),
        vec4<f32>(depth_mip_filter(d0, d1, d2, d3), 0.0, 0.0, 1.0),
    );
#else
    textureStore(
        dst_mip,
        vec2<i32>(i32(gid.x), i32(gid.y)),
        vec4<f32>(depth_mip_filter(d0, d1, d2, d3), 0.0, 0.0, 1.0),
    );
#endif
}
