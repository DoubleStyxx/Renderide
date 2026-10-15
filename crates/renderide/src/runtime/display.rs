//! Runtime-side presentation surface for the app driver.
//!
//! Owns the lazy GPU resources for the host `BlitToDisplay` desktop pass so the app driver does
//! not name backend GPU types directly. The app driver borrows the cache from [`RendererRuntime`]
//! and supplies a [`DisplayBlitSource`] for the per-call texture / orientation parameters.

pub(crate) use crate::gpu::DisplayBlitResources;
pub(crate) use crate::gpu::display_blit::DisplayBlitSource;
