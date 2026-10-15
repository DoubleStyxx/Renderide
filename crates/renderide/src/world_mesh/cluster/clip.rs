//! Clustered-forward clip-plane and tile-grid constants.

/// Minimum positive near distance used only by clustered Z slicing.
///
/// Keep in sync with `CLUSTER_NEAR_CLIP_MIN` in `shaders/modules/lighting/cluster_math.wgsl`.
pub const CLUSTER_NEAR_CLIP_MIN: f32 = 0.0001;
/// Minimum positive far-minus-near span used only by clustered Z slicing.
///
/// Keep in sync with `CLUSTER_FAR_CLIP_MIN_SPAN` in `shaders/modules/lighting/cluster_math.wgsl`.
pub const CLUSTER_FAR_CLIP_MIN_SPAN: f32 = 0.0001;
/// Screen tile size in pixels (DOOM-style cluster grid XY).
///
/// Keep in sync with `TILE_SIZE` in `shaders/modules/pbs/cluster.wgsl`.
pub const TILE_SIZE: u32 = 32;
/// Exponential depth slice count (view-space Z bins).
pub const CLUSTER_COUNT_Z: u32 = 16;

/// Returns clip planes sanitized exactly like the WGSL clustered-light helpers.
pub fn sanitize_cluster_clip_planes(near_clip: f32, far_clip: f32) -> (f32, f32) {
    let near_safe = if near_clip.is_finite() {
        near_clip.max(CLUSTER_NEAR_CLIP_MIN)
    } else {
        CLUSTER_NEAR_CLIP_MIN
    };
    let far_safe = if far_clip.is_finite() {
        far_clip.max(near_safe + CLUSTER_FAR_CLIP_MIN_SPAN)
    } else {
        near_safe + CLUSTER_FAR_CLIP_MIN_SPAN
    };
    (near_safe, far_safe)
}

/// Maps view-space Z to a clustered depth slice using the shared sanitized clip planes.
#[cfg(test)]
pub fn cluster_z_slice_from_view_z(
    view_z: f32,
    near_clip: f32,
    far_clip: f32,
    cluster_count_z: u32,
) -> u32 {
    let z_count = cluster_count_z.max(1);
    let (near_safe, far_safe) = sanitize_cluster_clip_planes(near_clip, far_clip);
    let d = (-view_z).clamp(near_safe, far_safe);
    let z = (d / near_safe).log(far_safe / near_safe) * z_count as f32;
    z.clamp(0.0, z_count.saturating_sub(1) as f32) as u32
}
