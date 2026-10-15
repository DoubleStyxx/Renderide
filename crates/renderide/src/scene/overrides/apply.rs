//! Shared-memory apply steps for transform and material override updates.
//!
//! Split into [`transforms`] (transform overrides), [`materials`] (material overrides), and
//! [`fixup`] (the generic transform-removal id sweep both call before applying their dense
//! updates). The barrel re-exports the public extraction functions, payload structs, and apply
//! entry points used by [`crate::scene::coordinator::apply`].

mod fixup;
mod materials;
mod transforms;

pub use materials::ExtractedRenderMaterialOverridesUpdate;
pub use transforms::ExtractedRenderTransformOverridesUpdate;

pub(crate) use materials::{
    apply_render_material_overrides_update_extracted, extract_render_material_overrides_update,
};
pub(crate) use transforms::{
    apply_render_transform_overrides_update_extracted, extract_render_transform_overrides_update,
};

#[cfg(test)]
mod tests;
