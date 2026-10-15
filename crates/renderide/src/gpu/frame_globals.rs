//! CPU layout for `shaders/modules/frame/globals.wgsl` (`FrameGlobals` at `@group(0) @binding(0)`).
//!
//! Submodules:
//! - [`uniforms`] -- the [`FrameGpuUniforms`] WGSL-matched Pod struct + per-eye / SH math.
//! - [`skybox_specular`] -- reserved [`SkyboxSpecularUniformParams`] /
//!   [`SkyboxSpecularSourceKind`] packing for the disabled direct skybox specular slot.
//! - [`clustered`] -- [`ClusteredFrameGlobalsParams`] input bundle and the
//!   [`FrameGpuUniforms::new_clustered`] constructor.

mod clustered;
mod skybox_specular;
mod uniforms;

#[cfg(test)]
mod tests;

pub use clustered::ClusteredFrameGlobalsParams;
pub use skybox_specular::SkyboxSpecularUniformParams;
pub use uniforms::{FRAME_PROJECTION_FLAG_ORTHOGRAPHIC, FrameGpuUniforms};
