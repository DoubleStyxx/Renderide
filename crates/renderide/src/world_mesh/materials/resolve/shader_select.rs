//! Stock shader route plus a per-material custom shader override.

use crate::materials::host_data::MaterialDictionary;
use crate::materials::{MaterialRouter, RasterPipelineKind, resolve_raster_pipeline};

/// Shader asset id and the raster pipeline for one material.
///
/// The stock route comes from the material's shader asset. An assigned custom key replaces it
/// when that program is installed. The stem's `shader_source_generation` still keys pipelines
/// and bind groups.
pub(super) fn resolve_material_shader(
    material_asset_id: i32,
    dict: &MaterialDictionary<'_>,
    router: &MaterialRouter,
) -> (i32, RasterPipelineKind) {
    let shader_asset_id = dict
        .shader_asset_for_material(material_asset_id)
        .unwrap_or(-1);
    let stock = resolve_raster_pipeline(shader_asset_id, router);
    let pipeline =
        crate::materials::custom_shader::apply_material_shader_override(material_asset_id, stock);
    (shader_asset_id, pipeline)
}

/// Host variant bits that may OR into custom keyword declaration indices.
///
/// A material whose stem came from [`crate::materials::custom_shader::apply_material_shader_override`]
/// returns `None`. Unlit `_ALPHATEST` is bit 0 and is not custom keyword 0. A stem reached by
/// shader name still returns the route's bits.
pub(super) fn keyword_host_bits(
    material_asset_id: i32,
    shader_asset_id: i32,
    router: &MaterialRouter,
) -> Option<u32> {
    if crate::materials::custom_shader::has_installed_material_override(material_asset_id) {
        return None;
    }
    router.variant_bits_for_shader_asset(shader_asset_id)
}
