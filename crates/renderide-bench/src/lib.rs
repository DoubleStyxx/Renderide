//! Shared fixtures for Renderide CPU microbenchmarks.
//!
//! The benches mirror renderer hot paths without needing a Resonite host or GPU:
//! IPC row packing/unpacking, transform composition, sphere/frustum culling, and
//! per-frame scratch buffer reuse vs. fresh allocation.

use glam::{Mat4, Quat, Vec3, Vec4};
use renderide_shared::RenderTransform;

/// Deterministic pseudo-random transforms (xorshift) so runs are comparable.
pub fn make_transforms(n: usize) -> Vec<RenderTransform> {
    let mut s: u32 = 0x9E37_79B9;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        (s as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    (0..n)
        .map(|_| RenderTransform {
            position: Vec3::new(next() * 100.0, next() * 100.0, next() * 100.0),
            scale: Vec3::splat(0.5 + next().abs()),
            rotation: Quat::from_euler(
                glam::EulerRot::YXZ,
                next() * 3.0,
                next() * 3.0,
                next() * 3.0,
            )
            .normalize(),
        })
        .collect()
}

/// Extracts the six normalized frustum planes from a view-projection matrix (Gribb/Hartmann).
pub fn frustum_planes(view_proj: Mat4) -> [Vec4; 6] {
    let r0 = view_proj.row(0);
    let r1 = view_proj.row(1);
    let r2 = view_proj.row(2);
    let r3 = view_proj.row(3);
    let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
    for p in &mut planes {
        *p /= p.truncate().length();
    }
    planes
}

/// Returns true when a bounding sphere intersects the frustum.
#[inline]
pub fn sphere_visible(planes: &[Vec4; 6], center: Vec3, radius: f32) -> bool {
    planes
        .iter()
        .all(|p| p.truncate().dot(center) + p.w >= -radius)
}
