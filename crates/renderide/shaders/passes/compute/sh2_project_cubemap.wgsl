#import renderide::skybox::cubemap_storage as cubemap_storage
#import renderide::ibl::ggx_prefilter as ggx
#import renderide::ibl::sh2_project as sh2p

struct Params {
    sample_size: u32,
    mode: u32,
    gradient_count: u32,
    _pad0: u32,
    color0: vec4<f32>,
    color1: vec4<f32>,
    direction: vec4<f32>,
    scalars: vec4<f32>,
    dirs_spread: array<vec4<f32>, 16>,
    gradient_color0: array<vec4<f32>, 16>,
    gradient_color1: array<vec4<f32>, 16>,
    gradient_params: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var source_tex: texture_cube<f32>;
@group(0) @binding(2) var source_sampler: sampler;
@group(0) @binding(3) var<storage, read_write> output_coeffs: array<vec4<f32>, 9>;

var<workgroup> partial: array<vec4<f32>, sh2p::PARTIAL_COEFFS>;

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) local_id: vec3<u32>) {
    let n = max(params.sample_size, 1u);
    let face_size = n * n;
    let total = face_size * 6u;
    let base = local_id.x * sh2p::COEFFS;
    for (var c = 0u; c < sh2p::COEFFS; c = c + 1u) {
        partial[base + c] = vec4<f32>(0.0);
    }
    var i = local_id.x;
    while (i < total) {
        let face = i / face_size;
        let rem = i - face * face_size;
        let y = rem / n;
        let x = rem - y * n;
        let dir = ggx::cube_dir(face, x, y, n);
        let sample_dir = cubemap_storage::sample_dir(dir, params.scalars.x);
        let color = textureSampleLevel(source_tex, source_sampler, sample_dir, 0.0).rgb;
        let weight = sh2p::texel_solid_angle(x, y, n);
        for (var coeff = 0u; coeff < sh2p::COEFFS; coeff = coeff + 1u) {
            partial[base + coeff] = partial[base + coeff] + sh2p::project_coeff(coeff, color, dir, weight);
        }
        i = i + sh2p::WORKGROUP_SIZE;
    }
    workgroupBarrier();
    if (local_id.x == 0u) {
        for (var coeff = 0u; coeff < sh2p::COEFFS; coeff = coeff + 1u) {
            var sum = vec4<f32>(0.0);
            for (var lane = 0u; lane < sh2p::WORKGROUP_SIZE; lane = lane + 1u) {
                sum = sum + partial[lane * sh2p::COEFFS + coeff];
            }
            output_coeffs[coeff] = sum;
        }
    }
}
