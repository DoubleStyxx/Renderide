//! Equirectangular projection direction helpers.

#define_import_path renderide::skybox::equirect

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

/// Converts normalized equirectangular UVs to a canonical world-space sample direction.
fn uv_to_dir(uv: vec2<f32>) -> vec3<f32> {
    let h_angle = uv.x * TAU;
    let v_angle = ((1.0 - uv.y) - 0.5) * PI;
    let cv = cos(v_angle);
    let sv = sin(v_angle);
    let ch = cos(h_angle);
    let sh = sin(h_angle);
    var dir = vec3<f32>(0.0, 0.0, 1.0);
    dir = vec3<f32>(dir.x, cv * dir.y - sv * dir.z, sv * dir.y + cv * dir.z);
    dir = vec3<f32>(ch * dir.x + sh * dir.z, dir.y, -sh * dir.x + ch * dir.z);
    return dir;
}
