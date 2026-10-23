//! Hi-Z occlusion subsystem: GPU pyramid build and the [`OcclusionSystem`] facade that owns
//! per-view temporal state.

pub mod gpu;
mod hook;
pub(crate) mod hysteresis;
mod system;

pub use hook::OcclusionGraphHook;
pub(crate) use system::HiZBuildInput;
pub use system::OcclusionSystem;
