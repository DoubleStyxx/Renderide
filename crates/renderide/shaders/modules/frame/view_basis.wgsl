//! View-basis reconstruction helpers derived from a view-projection matrix.

#define_import_path renderide::frame::view_basis

#import renderide::core::math as rmath

struct ViewBasis {
    x: vec3<f32>,
    y: vec3<f32>,
    z: vec3<f32>,
}

fn projection_row_xyz(m: mat4x4<f32>, row: u32) -> vec3<f32> {
    return vec3<f32>(m[0u][row], m[1u][row], m[2u][row]);
}

fn from_view_projection(vp: mat4x4<f32>) -> ViewBasis {
    let clip_x = projection_row_xyz(vp, 0u);
    let clip_y = projection_row_xyz(vp, 1u);
    let clip_w = projection_row_xyz(vp, 3u);

    let cross_fallback = rmath::safe_normalize(cross(clip_x, clip_y), vec3<f32>(0.0, 0.0, 1.0));
    let view_z = rmath::safe_normalize(-clip_w, cross_fallback);
    let view_x = rmath::safe_normalize(clip_x - view_z * dot(clip_x, view_z), vec3<f32>(1.0, 0.0, 0.0));
    let view_y_raw = clip_y - view_z * dot(clip_y, view_z);
    let view_y = rmath::safe_normalize(
        view_y_raw - view_x * dot(view_y_raw, view_x),
        rmath::safe_normalize(cross(view_z, view_x), vec3<f32>(0.0, 1.0, 0.0)),
    );

    return ViewBasis(view_x, view_y, view_z);
}

fn world_to_view_normal(world_n: vec3<f32>, vp: mat4x4<f32>) -> vec3<f32> {
    let basis = from_view_projection(vp);
    return rmath::safe_normalize(vec3<f32>(
        dot(world_n, basis.x),
        dot(world_n, basis.y),
        dot(world_n, basis.z),
    ), vec3<f32>(0.0, 0.0, 1.0));
}
