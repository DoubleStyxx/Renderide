//! Screen-door LOD cross-fade.
//!
//! The signed weight lives in `PerDrawUniforms.particle_padding[5].x` (CPU `_pad2[5][0]`).
//! Rows `[0..4]` and `[4].x` are reserved for the object-velocity previous model.
//! `0` keeps every fragment. A positive weight `t` keeps fragments whose Bayer threshold is
//! below `t`. A negative weight keeps the complementary fragments, so two coplanar LOD draws
//! partition the pixels.

#define_import_path renderide::draw::lod_fade

/// 4x4 Bayer matrix as `(entry + 0.5) / 16`, indexed `y * 4 + x`.
const BAYER4: array<f32, 16> = array<f32, 16>(
    0.03125, 0.53125, 0.15625, 0.65625,
    0.78125, 0.28125, 0.90625, 0.40625,
    0.21875, 0.71875, 0.09375, 0.59375,
    0.96875, 0.46875, 0.84375, 0.34375,
);

/// Ordered-dither threshold for the fragment's framebuffer coordinate.
///
/// Fragment `@builtin(position).xy` is the pixel coordinate, even when the vertex stage stored
/// clip space in the same builtin.
fn bayer4_threshold(frag_xy: vec2<f32>) -> f32 {
    let x = u32(frag_xy.x) & 3u;
    let y = u32(frag_xy.y) & 3u;
    return BAYER4[y * 4u + x];
}

/// `true` when this fragment should be discarded for `fade`.
///
/// `fade == 0` returns false without reading the Bayer matrix, so a disabled cross-fade
/// takes the same coverage as a shader that never stipples.
fn lod_fade_discards(frag_xy: vec2<f32>, fade: f32) -> bool {
    if (fade == 0.0) {
        return false;
    }
    let threshold = bayer4_threshold(frag_xy);
    if (fade > 0.0) {
        return threshold >= fade;
    }
    return threshold < -fade;
}
