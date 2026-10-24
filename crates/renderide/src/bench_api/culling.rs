use glam::{Mat4, Vec3};

use crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip;

/// World AABBs tested by one [`CullWorkload::visible_count`] sample.
pub const CULL_BATCH: usize = 8_192;

/// Grid width used to scatter cull boxes around the origin.
const CULL_GRID_SIDE: usize = 96;

/// Prebuilt world AABBs and the view-projection used by homogeneous clip culling.
#[derive(Clone, Debug)]
pub struct CullWorkload {
    view_proj: Mat4,
    mins: Vec<Vec3>,
    maxs: Vec<Vec3>,
}

impl CullWorkload {
    /// Builds [`CULL_BATCH`] boxes, some inside the camera frustum and some outside it.
    #[must_use]
    pub fn standard() -> Self {
        Self::grid(CULL_BATCH)
    }

    /// Scatters `count` unit boxes on the XZ plane around the origin.
    #[must_use]
    pub fn grid(count: usize) -> Self {
        let mut mins = Vec::with_capacity(count);
        let mut maxs = Vec::with_capacity(count);
        for index in 0..count {
            let column = index % CULL_GRID_SIDE;
            let row = index / CULL_GRID_SIDE;
            let x = f32::from(u16::try_from(column).unwrap_or(0)) - 48.0;
            let z = f32::from(u16::try_from(row).unwrap_or(0)) - 48.0;
            let min = Vec3::new(x, 0.0, z);
            mins.push(min);
            maxs.push(min + Vec3::splat(0.5));
        }
        Self {
            view_proj: bench_view_proj(),
            mins,
            maxs,
        }
    }

    /// Counts boxes that survive the homogeneous clip AABB test.
    #[must_use]
    pub fn visible_count(&self) -> u32 {
        let mut visible = 0_u32;
        for (min, max) in self.mins.iter().zip(self.maxs.iter()) {
            if world_aabb_visible_in_homogeneous_clip(self.view_proj, *min, *max) {
                visible += 1;
            }
        }
        visible
    }
}

/// Perspective view-projection aimed at the origin from `+Z`.
///
/// Depth is wgpu NDC z in [0, 1] (near → 0, far → 1), the same interval
/// [`crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip`] tests.
fn bench_view_proj() -> Mat4 {
    let view = glam::camera::rh::view::look_at_mat4(Vec3::new(0.0, 1.5, 6.0), Vec3::ZERO, Vec3::Y);
    let projection = glam::camera::rh::proj::directx::perspective(
        60.0_f32.to_radians(),
        16.0 / 9.0,
        0.05,
        250.0,
    );
    projection * view
}
