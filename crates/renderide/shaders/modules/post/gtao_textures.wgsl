//! Shared GTAO texture-load helpers.

#define_import_path renderide::post::gtao_textures

#import renderide::post::gtao_filter as gf

fn load_ao(
    ao_term: texture_2d_array<f32>,
    pix: vec2<i32>,
    view_layer: u32,
    viewport_max: vec2<i32>,
) -> f32 {
    let p = clamp(pix, vec2<i32>(0), viewport_max);
    return textureLoad(ao_term, p, i32(view_layer), 0).r;
}

fn load_edges_lrtb(
    ao_edges: texture_2d_array<f32>,
    pix: vec2<i32>,
    view_layer: u32,
    viewport_max: vec2<i32>,
) -> vec4<f32> {
    let p = clamp(pix, vec2<i32>(0), viewport_max);
    let packed = textureLoad(ao_edges, p, i32(view_layer), 0).r;
    return gf::gtao_unpack_edges(packed);
}
