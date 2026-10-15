// Resolves multisampled depth (reverse-Z) to a single-sample R32Float image for depth blit to Depth32Float.

@group(0) @binding(0) var src: texture_depth_multisampled_2d;
@group(0) @binding(1) var dst: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3u) {
    let dims = textureDimensions(src);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }
    let samples = textureNumSamples(src);
    var best = 0.0;
    for (var s = 0u; s < samples; s++) {
        best = max(best, textureLoad(src, vec2i(i32(gid.x), i32(gid.y)), i32(s)));
    }
    textureStore(dst, vec2i(i32(gid.x), i32(gid.y)), vec4f(best, 0.0, 0.0, 1.0));
}
