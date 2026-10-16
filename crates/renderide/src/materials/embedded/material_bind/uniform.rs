//! Embedded `@group(1)` material uniform arena and upload.

mod arena;
mod key;
mod retire;
mod upload;

pub(super) use arena::MaterialUniformArena;
pub(super) use key::{MaterialUniformArenaSlotBinding, MaterialUniformCacheKey};
pub(super) use retire::UniformArenaRetireClock;
pub(super) use upload::EmbeddedUniformArenaRequest;

#[cfg(feature = "bench-api")]
pub(crate) use arena::CpuMaterialUniformArena;
