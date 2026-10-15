//! SH2 projection helpers shared by environment-map compute passes.

#define_import_path renderide::ibl::sh2_project

const WORKGROUP_SIZE: u32 = 64u;
const COEFFS: u32 = 9u;
const PARTIAL_COEFFS: u32 = WORKGROUP_SIZE * COEFFS;
const SH_C0: f32 = 0.2820947917;
const SH_C1: f32 = 0.4886025119;
const SH_C2: f32 = 1.0925484306;
const SH_C3: f32 = 0.3153915652;
const SH_C4: f32 = 0.5462742153;

fn area_element(x: f32, y: f32) -> f32 {
    return atan2(x * y, sqrt(x * x + y * y + 1.0));
}

fn texel_solid_angle(x: u32, y: u32, n: u32) -> f32 {
    let inv = 1.0 / f32(n);
    let x0 = (f32(x) * inv) * 2.0 - 1.0;
    let y0 = (f32(y) * inv) * 2.0 - 1.0;
    let x1 = (f32(x + 1u) * inv) * 2.0 - 1.0;
    let y1 = (f32(y + 1u) * inv) * 2.0 - 1.0;
    return abs(area_element(x0, y0) - area_element(x0, y1) - area_element(x1, y0) + area_element(x1, y1));
}

fn project_coeff(coeff: u32, c: vec3<f32>, dir: vec3<f32>, weight: f32) -> vec4<f32> {
    if (coeff == 0u) {
        return vec4<f32>(c * (SH_C0 * weight), 0.0);
    }
    if (coeff == 1u) {
        return vec4<f32>(c * (SH_C1 * dir.y * weight), 0.0);
    }
    if (coeff == 2u) {
        return vec4<f32>(c * (SH_C1 * dir.z * weight), 0.0);
    }
    if (coeff == 3u) {
        return vec4<f32>(c * (SH_C1 * dir.x * weight), 0.0);
    }
    if (coeff == 4u) {
        return vec4<f32>(c * (SH_C2 * dir.x * dir.y * weight), 0.0);
    }
    if (coeff == 5u) {
        return vec4<f32>(c * (SH_C2 * dir.y * dir.z * weight), 0.0);
    }
    if (coeff == 6u) {
        return vec4<f32>(c * (SH_C3 * (3.0 * dir.z * dir.z - 1.0) * weight), 0.0);
    }
    if (coeff == 7u) {
        return vec4<f32>(c * (SH_C2 * dir.x * dir.z * weight), 0.0);
    }
    return vec4<f32>(c * (SH_C4 * (dir.x * dir.x - dir.y * dir.y) * weight), 0.0);
}
