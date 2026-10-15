//! Shared GradientSkybox evaluator.

#define_import_path renderide::skybox::gradient

#import renderide::frame::globals as rg

fn gradient_sky_color(
    base_color: vec4<f32>,
    gradients: f32,
    dirs_spread_values: array<vec4<f32>, 16>,
    color0_values: array<vec4<f32>, 16>,
    color1_values: array<vec4<f32>, 16>,
    param_values: array<vec4<f32>, 16>,
    ray_in: vec3<f32>,
) -> vec4<f32> {
    let ray = normalize(ray_in);
    var col = base_color.rgb;
    let count = min(u32(max(gradients, 0.0)), 16u);
    for (var i = 0u; i < count; i = i + 1u) {
        let dirs_spread = dirs_spread_values[i];
        let params = param_values[i];
        var r = 0.5 - dot(ray, dirs_spread.xyz) * 0.5;
        r = r / dirs_spread.w;
        if (r <= 1.0) {
            r = pow(r, params.y);
            r = clamp((r - params.z) / (params.w - params.z), 0.0, 1.0);
            let c = mix(color0_values[i], color1_values[i], r);
            if (params.x == 0.0) {
                col = col * (1.0 - c.a) + c.rgb * c.a;
            } else {
                col = col + c.rgb * c.a;
            }
        }
    }
    return rg::retain_globals_additive(vec4<f32>(col, 1.0));
}
