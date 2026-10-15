//! Shared billboard basis and corner helpers.

#define_import_path renderide::mesh::billboard

#import renderide::frame::globals as rg
#import renderide::core::math as rmath

struct BillboardBasis {
    right: vec3<f32>,
    up: vec3<f32>,
}

fn rotate_billboard_axes(angle: f32, right: vec3<f32>, up: vec3<f32>) -> BillboardBasis {
    let c = cos(angle);
    let s = sin(angle);
    return BillboardBasis(right * c - up * s, right * s + up * c);
}

fn billboard_axes(center_world: vec3<f32>, point_data: vec3<f32>, view_layer: u32, rotation_enabled: bool) -> BillboardBasis {
    let cam = rg::camera_world_pos_for_view(view_layer);
    let forward = rmath::safe_normalize(center_world - cam, vec3<f32>(0.0, 0.0, 1.0));
    var right = rmath::safe_normalize(cross(vec3<f32>(0.0, 1.0, 0.0), forward), vec3<f32>(1.0, 0.0, 0.0));
    var up = rmath::safe_normalize(cross(forward, right), vec3<f32>(0.0, 1.0, 0.0));

    if (rotation_enabled) {
        let rotated = rotate_billboard_axes(point_data.z, right, up);
        right = rotated.right;
        up = rotated.up;
    }

    return BillboardBasis(right, up);
}

fn model_uniform_scale(model: mat4x4<f32>) -> f32 {
    return max(length(model[0].xyz), 1e-6);
}

fn billboard_size(point_data: vec3<f32>, base_size: vec2<f32>, model: mat4x4<f32>, point_size_enabled: bool) -> vec2<f32> {
    var size = base_size;
    if (point_size_enabled) {
        size = size * point_data.xy;
    }
    return size * model_uniform_scale(model);
}

fn billboard_corner(pos: vec3<f32>, raw_uv: vec2<f32>) -> vec2<f32> {
    let from_uv = raw_uv * 2.0 - vec2<f32>(1.0, 1.0);
    let from_pos = vec2<f32>(
        select(-1.0, 1.0, pos.x >= 0.0),
        select(-1.0, 1.0, pos.y >= 0.0),
    );
    let uv_in_unit_square = all(raw_uv >= vec2<f32>(0.0, 0.0)) && all(raw_uv <= vec2<f32>(1.0, 1.0));
    return select(from_pos, from_uv, uv_in_unit_square);
}
