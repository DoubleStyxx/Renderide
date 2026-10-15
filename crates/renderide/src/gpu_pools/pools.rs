//! Concrete resident GPU pools, one per asset kind.
//!
//! Each submodule is a thin newtype around [`crate::gpu_pools::resource_pool::GpuResourcePool`]
//! that wires the unified [`crate::gpu_pools::SamplerState`], unified
//! [`crate::gpu_pools::TextureResidencyMeta`] builder, and one of the two facade macros
//! (`impl_streaming_pool_facade!` / `impl_resident_pool_facade!`) declared in
//! [`crate::gpu_pools::resource_pool`].

pub mod cubemap;
pub mod mesh;
pub mod render_texture;
pub mod texture2d;
pub mod texture3d;
pub mod video_texture;
