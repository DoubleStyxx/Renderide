//! Shared helpers for material grab-pass and scene-depth filter shaders.

#define_import_path renderide::post::filter_common

#import renderide::frame::globals as rg
#import renderide::frame::grab_pass as gp
#import renderide::ui::rect_clip as uirc

fn discard_rect_if_enabled(obj_xy: vec2<f32>, rect: vec4<f32>, enabled: bool) {
    if (uirc::should_clip_rect_kw(obj_xy, rect, enabled)) {
        discard;
    }
}

fn screen_uv(clip_pos: vec4<f32>) -> vec2<f32> {
    return gp::frag_screen_uv(clip_pos);
}

fn sample_scene_color_at_clip(clip_pos: vec4<f32>, view_layer: u32) -> vec4<f32> {
    return gp::sample_scene_color(screen_uv(clip_pos), view_layer);
}

fn sample_clipped_scene_color_at_clip(
    obj_xy: vec2<f32>,
    rect: vec4<f32>,
    rect_enabled: bool,
    clip_pos: vec4<f32>,
    view_layer: u32,
) -> vec4<f32> {
    discard_rect_if_enabled(obj_xy, rect, rect_enabled);
    return sample_scene_color_at_clip(clip_pos, view_layer);
}

fn retain_scene_alpha(scene_color: vec4<f32>, filtered_rgb: vec3<f32>) -> vec4<f32> {
    return retain_globals(vec4<f32>(filtered_rgb, scene_color.a));
}

fn retain_globals(color: vec4<f32>) -> vec4<f32> {
    return rg::retain_globals_additive(color);
}
