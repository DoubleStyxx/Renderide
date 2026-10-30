//! GPU hierarchical depth pyramid construction and readback for Hi-Z occlusion culling.
//!
//! Used by [`crate::occlusion::OcclusionSystem`] and [`crate::passes::HiZBuildPass`].

mod encode;
mod pipelines;
mod readback;
mod readback_ring;
mod scratch;
mod state;

pub use encode::{HiZBuildRecord, HiZHistoryTarget, encode_hi_z_build};

/// Compiles Hi-Z compute pipelines before a pass records them.
pub(crate) fn precompile_hi_z_pipelines(device: &wgpu::Device) {
    let _ = pipelines::HiZPipelines::get(device);
}
pub(crate) use scratch::HIZ_MAX_MIPS;
pub use state::HiZGpuState;
