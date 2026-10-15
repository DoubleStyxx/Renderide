//! Shared material Fresnel helpers.

#define_import_path renderide::material::fresnel

fn view_angle_fresnel(normal: vec3<f32>, view_dir: vec3<f32>, exponent: f32, gamma_curve: f32) -> f32 {
    let base = pow(max(1.0 - abs(dot(normalize(normal), normalize(view_dir))), 0.0), max(exponent, 1e-4));
    return pow(clamp(base, 0.0, 1.0), max(gamma_curve, 1e-4));
}

fn rim_factor(normal: vec3<f32>, view_dir: vec3<f32>, power: f32) -> f32 {
    return pow(max(1.0 - clamp(dot(normalize(view_dir), normalize(normal)), 0.0, 1.0), 0.0), max(power, 1e-4));
}

fn near_far_color(near_color: vec4<f32>, far_color: vec4<f32>, fresnel: f32) -> vec4<f32> {
    return mix(near_color, far_color, clamp(fresnel, 0.0, 1.0));
}
