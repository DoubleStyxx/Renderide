//! Shared math for the PBS Slice material family.

#define_import_path renderide::pbs::families::slice

#import renderide::core::math as rmath
#import renderide::core::normal_decode as nd
#import renderide::core::texture_sampling as ts
#import renderide::pbs::normal as pnorm

/// Result of evaluating up to eight slice planes against a fragment position.
struct SliceEvalResult {
    /// Signed minimum plane distance (negative means the fragment is clipped).
    min_distance: f32,
    /// `[0, 1]` factor that grows toward `1` inside the edge transition band.
    edge_lerp: f32,
}

fn plane_distance(p: vec3<f32>, normal: vec3<f32>, offset: f32) -> f32 {
    return dot(p, normal) + offset;
}

/// Walk the slicer plane array (stops at the first zero-normal entry) and pair
/// the minimum signed distance with the `[start, end]`-band edge transition.
fn evaluate_planes(
    slicers: array<vec4<f32>, 8>,
    slice_p: vec3<f32>,
    edge_start: f32,
    edge_end: f32,
) -> SliceEvalResult {
    var min_distance: f32 = 60000.0;
    for (var si: i32 = 0; si < 8; si = si + 1) {
        let slicer = slicers[si];
        if (all(slicer.xyz == vec3<f32>(0.0))) {
            break;
        }
        min_distance = min(min_distance, plane_distance(slice_p, slicer.xyz, slicer.w));
    }
    let edge_lerp = 1.0 - rmath::safe_lerp_factor(edge_start, edge_end, min_distance);
    return SliceEvalResult(min_distance, edge_lerp);
}

fn use_world_space(world_space_enabled: bool, object_space_enabled: bool) -> bool {
    if (object_space_enabled) {
        return false;
    }
    return world_space_enabled || (!object_space_enabled);
}

fn slice_position(
    world_pos: vec3<f32>,
    object_pos: vec3<f32>,
    world_space_enabled: bool,
    object_space_enabled: bool,
) -> vec3<f32> {
    return select(object_pos, world_pos, use_world_space(world_space_enabled, object_space_enabled));
}

fn blend_detail_normal(base_ts: vec3<f32>, detail_ts: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(base_ts.xy + detail_ts.xy, base_ts.z * detail_ts.z));
}

fn sample_world_normal(
    normal_map_enabled: bool,
    detail_normal_map_enabled: bool,
    normal_map: texture_2d<f32>,
    normal_map_sampler: sampler,
    detail_normal_map: texture_2d<f32>,
    detail_normal_map_sampler: sampler,
    uv_main: vec2<f32>,
    uv_detail: vec2<f32>,
    normal_lod_bias: f32,
    detail_normal_lod_bias: f32,
    normal_scale: f32,
    detail_normal_scale: f32,
    world_n: vec3<f32>,
    world_t: vec4<f32>,
    front_facing: bool,
) -> vec3<f32> {
    if (normal_map_enabled || detail_normal_map_enabled) {
        let tbn = pnorm::orthonormal_tbn(world_n, world_t);
        var ts_n = nd::decode_ts_normal_with_placeholder_sample(
            ts::sample_tex_2d(normal_map, normal_map_sampler, uv_main, normal_lod_bias),
            normal_scale,
        );
        if (detail_normal_map_enabled) {
            let detail = nd::decode_ts_normal_with_placeholder_sample(
                ts::sample_tex_2d(
                    detail_normal_map,
                    detail_normal_map_sampler,
                    uv_detail,
                    detail_normal_lod_bias,
                ),
                detail_normal_scale,
            );
            ts_n = blend_detail_normal(ts_n, detail);
        }
        if (!front_facing) {
            ts_n = vec3<f32>(ts_n.x, ts_n.y, -ts_n.z);
        }
        return normalize(tbn * ts_n);
    }

    var n = normalize(world_n);
    if (!front_facing) {
        n = -n;
    }
    return n;
}
