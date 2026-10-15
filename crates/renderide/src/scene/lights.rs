//! Host-driven lights: CPU cache from [`FrameSubmitData`](crate::shared::FrameSubmitData) and light buffer submissions.
//!
//! Scene lights are logical state (poses, types, shadow params). GPU ABI types live in
//! [`crate::gpu`], and render-frame storage allocation lives in the backend.

mod apply;
mod cache;
mod types;

pub use apply::{apply_light_renderables_update, apply_lights_buffer_renderers_update};
pub use cache::LightCache;
pub use types::{ResolvedLight, light_contributes, light_has_negative_contribution};
