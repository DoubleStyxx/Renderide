//! Fullscreen CameraRenderTask alpha repair from reverse-Z depth coverage.

#import renderide::core::fullscreen as fs

@group(0) @binding(0) var task_depth: texture_depth_2d;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> fs::FullscreenVertexOutput {
    return fs::vertex_main(vertex_index);
}

fn depth_marks_coverage(reverse_z_depth: f32) -> bool {
    return reverse_z_depth > 0.0;
}

@fragment
fn fs_main(in: fs::FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dim = textureDimensions(task_depth);
    let max_pix = vec2<i32>(i32(dim.x) - 1, i32(dim.y) - 1);
    let pix = clamp(vec2<i32>(in.clip_pos.xy), vec2<i32>(0), max_pix);
    let alpha = select(0.0, 1.0, depth_marks_coverage(textureLoad(task_depth, pix, 0)));
    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}
