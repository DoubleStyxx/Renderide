//! Material uniform, bind-group, keyword-variant, and stem-lookup workloads.

/// Custom-material uniform packing and bind-group cache workloads.
pub use crate::materials::{
    BIND_GROUP_BATCH, BindGroupChurnWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
};

/// Custom-shader keyword variant cache and per-frame stem lookup workloads.
pub use crate::materials::{CustomStemLookupWorkload, KeywordVariantCacheWorkload};
