#import renderide::skybox::evaluator as sky

@group(0) @binding(0) var<uniform> params: sky::SkyboxEvaluatorParams;
@group(0) @binding(1) var output_cube: texture_storage_2d_array<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let face_size = max(params.sample_size, 1u);
    if (gid.x >= face_size || gid.y >= face_size || gid.z >= 6u) {
        return;
    }
    let ray = sky::cube_dir(gid.z, gid.x, gid.y, face_size);
    textureStore(
        output_cube,
        vec2i(i32(gid.x), i32(gid.y)),
        i32(gid.z),
        vec4<f32>(sky::sample_sky(params, ray), 1.0)
    );
}
