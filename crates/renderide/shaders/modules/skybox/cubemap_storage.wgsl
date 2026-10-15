//! Cubemap storage-orientation helpers.
//!
//! Cubemap storage-orientation helpers for sources that need shader-side compensation. Most cube
//! textures are stored in native orientation; callers pass a non-zero flag only for alternate
//! storage layouts.

#define_import_path renderide::skybox::cubemap_storage

/// Returns the source texture direction for a canonical cubemap sample direction.
fn sample_dir(dir: vec3<f32>, storage_v_inverted: f32) -> vec3<f32> {
    if (storage_v_inverted <= 0.5) {
        return dir;
    }
    let a = abs(dir);
    if (a.y >= a.x && a.y >= a.z) {
        return vec3<f32>(dir.x, dir.y, -dir.z);
    }
    return vec3<f32>(dir.x, -dir.y, dir.z);
}
