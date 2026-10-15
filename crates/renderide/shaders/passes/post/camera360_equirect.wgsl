//! Fullscreen Camera360 cubemap-to-equirectangular projection.

#import renderide::core::fullscreen as fs
#import renderide::skybox::cubemap_storage as cubemap_storage
#import renderide::skybox::equirect as equirect

struct Camera360Projection {
    rotation: mat4x4<f32>,
    storage: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Camera360Projection;
@group(0) @binding(1) var source_cube: texture_cube<f32>;
@group(0) @binding(2) var source_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> fs::FullscreenVertexOutput {
    return fs::vertex_main(vertex_index);
}

@fragment
fn fs_main(in: fs::FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var dir = equirect::uv_to_dir(in.uv);
    let rot3 = mat3x3<f32>(params.rotation[0].xyz, params.rotation[1].xyz, params.rotation[2].xyz);
    dir = rot3 * dir;
    dir = -dir;
    return textureSample(
        source_cube,
        source_sampler,
        cubemap_storage::sample_dir(dir, params.storage.x),
    );
}
