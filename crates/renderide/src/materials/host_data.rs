//! Material property batches from IPC (`MaterialsUpdateBatch`, property id interning).

mod properties;
mod property_registry;
mod update_batch;

pub(crate) use properties::MergedPropertyMaps;
pub use properties::{
    MaterialDictionary, MaterialPropertyLookupIds, MaterialPropertyStore, MaterialPropertyValue,
};
pub use property_registry::PropertyIdRegistry;
#[cfg(test)]
pub(crate) use update_batch::apply_material_set_instancing_for_test;
pub use update_batch::{
    MaterialBatchParseReport, ParseMaterialBatchOptions,
    parse_materials_update_batch_into_store_with_instance_changed,
};
