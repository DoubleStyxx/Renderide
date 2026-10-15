//! Clustered forward helpers: screen tile XY and exponential Z slice (matches clustered light compute).
//!
//! Import with `#import renderide::pbs::cluster`.

#define_import_path renderide::pbs::cluster

#import renderide::lighting::cluster_math as cmath
#import renderide::frame::globals as rg

const TILE_SIZE: u32 = cmath::TILE_SIZE;

/// Fetches the light count written by clustered-light compute for `cluster_id`.
fn cluster_light_count_at(cluster_id: u32) -> u32 {
    return rg::cluster_light_ranges[cluster_id].y;
}

/// Fetches the compact light index at `slot` within cluster `cluster_id`. Must stay in sync with
/// the compute-side writer in `shaders/passes/compute/clustered_light.wgsl` and the Rust-side
/// layout documented on
/// `ClusterBufferRefs::cluster_light_indices`.
fn cluster_light_index_at(cluster_id: u32, slot: u32) -> u32 {
    let range = rg::cluster_light_ranges[cluster_id];
    return rg::cluster_light_indices[range.x + slot];
}

/// Integer pixel -> tile index. Uses `floor(pxy / TILE_SIZE)` so tile `k` covers pixels
/// `[TILE_SIZE*k, TILE_SIZE*(k+1))` -- this must match [`get_cluster_aabb`] in
/// `compute/clustered_light.wgsl`, where AABB extents are computed at pixel-edge boundaries
/// (`px_min = TILE_SIZE*k`, `px_max = TILE_SIZE*(k+1)`). Any gap between the two formulations
/// shows as pixelated seams where fragments reach lights assigned only to a neighbor.
fn cluster_xy_from_frag(frag_xy: vec2<f32>, viewport_w: u32, viewport_h: u32) -> vec2<u32> {
    let vw = max(viewport_w, 1u);
    let vh = max(viewport_h, 1u);
    let px = min(u32(max(frag_xy.x, 0.0)), vw - 1u);
    let py = min(u32(max(frag_xy.y, 0.0)), vh - 1u);
    return vec2<u32>(px / TILE_SIZE, py / TILE_SIZE);
}

fn cluster_z_from_view_z(view_z: f32, near_clip: f32, far_clip: f32, cluster_count_z: u32) -> u32 {
    return cmath::cluster_z_from_view_z(view_z, near_clip, far_clip, cluster_count_z);
}

fn cluster_id_from_frag(
    clip_xy: vec2<f32>,
    world_pos: vec3<f32>,
    view_space_z_coeffs: vec4<f32>,
    view_space_z_coeffs_right: vec4<f32>,
    view_index: u32,
    viewport_w: u32,
    viewport_h: u32,
    cluster_count_x: u32,
    cluster_count_y: u32,
    cluster_count_z: u32,
    near_clip: f32,
    far_clip: f32,
) -> u32 {
    let count_x = max(cluster_count_x, 1u);
    let count_y = max(cluster_count_y, 1u);
    let count_z = max(cluster_count_z, 1u);
    let physical_view_index = rg::view_index_from_layer(view_index);
    let z_coeffs = select(view_space_z_coeffs, view_space_z_coeffs_right, physical_view_index != 0u);
    let view_z = dot(z_coeffs.xyz, world_pos) + z_coeffs.w;
    let cluster_z = cluster_z_from_view_z(view_z, near_clip, far_clip, count_z);
    let cluster_xy = cluster_xy_from_frag(clip_xy, viewport_w, viewport_h);
    let cx = min(cluster_xy.x, count_x - 1u);
    let cy = min(cluster_xy.y, count_y - 1u);
    let local_id = cx + count_x * (cy + count_y * cluster_z);
    let cluster_offset = physical_view_index * count_x * count_y * count_z;
    return cluster_offset + local_id;
}
