//! Shared OVERLAY depth-compare + tint-multiply pattern used by UI materials.
//!
//! When OVERLAY is enabled, a UI fragment that lies behind the scene depth gets
//! tinted by `_OverlayTint` (matching the `#ifdef OVERLAY` branch in the Unity
//! `UI/*` shaders). Returns the input color unchanged when the keyword is off
//! or when the fragment is in front of the scene.

#define_import_path renderide::ui::overlay_tint

#import renderide::frame::scene_depth_sample as sds

fn apply_overlay_tint(
    color: vec4<f32>,
    overlay_tint: vec4<f32>,
    clip_pos: vec4<f32>,
    world_pos: vec3<f32>,
    view_layer: u32,
    enabled: bool,
) -> vec4<f32> {
    if (!enabled) {
        return color;
    }
    let scene_z = sds::scene_linear_depth(clip_pos, view_layer);
    let part_z = sds::fragment_linear_depth(world_pos, view_layer);
    if (part_z > scene_z) {
        return color * overlay_tint;
    }
    return color;
}
