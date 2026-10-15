//! Renderer-reserved uniform writers for embedded material packing.

use crate::materials::host_data::{MaterialPropertyLookupIds, MaterialPropertyStore};

use super::super::layout::StemEmbeddedPropertyIds;

/// Renderer-reserved material uniform field carrying the raw shader-specific Froox variant bitmask.
const RENDERIDE_VARIANT_BITS_FIELD: &str = "_RenderideVariantBits";

/// Returns the raw renderer-reserved shader variant bitfield, when the reflected field requests it.
pub(super) fn inferred_shader_variant_bits_u32(
    field_name: &str,
    shader_variant_bits: Option<u32>,
    _store: &MaterialPropertyStore,
    _lookup: MaterialPropertyLookupIds,
    _ids: &StemEmbeddedPropertyIds,
) -> Option<u32> {
    if field_name != RENDERIDE_VARIANT_BITS_FIELD {
        return None;
    }
    shader_variant_bits
}
