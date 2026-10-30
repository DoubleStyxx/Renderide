//! CPU-only entry points for the `renderide-bench` crate.
//!
//! These helpers are not part of the renderer process API. They call the same transform, frustum,
//! transient-pool, material-uniform, bind-group, and keyword-variant cache bookkeeping the frame
//! loop uses, and they never open a wgpu device.

mod camera;
mod culling;
mod materials;
mod occlusion;
mod pools;
mod skinned_geometry;
mod sort;
mod transforms;

/// One synthetic camera for [`camera_clear_load_op_checksum`].
pub use camera::CameraClearLoadOpInput;
pub use camera::{
    CAMERA_CLEAR_LOAD_OP_CAMERAS, DOUBLE_BUFFER_CAMERAS, DoubleBufferWorkload,
    camera_clear_load_op_checksum, camera_clear_load_op_inputs,
};
pub use culling::{CULL_BATCH, CullWorkload};
/// Custom-material uniform packing and bind-group cache workloads.
pub use materials::{
    BIND_GROUP_BATCH, BindGroupChurnWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
};
/// Custom-shader keyword variant cache and per-frame stem lookup workloads.
pub use materials::{CustomStemLookupWorkload, KeywordVariantCacheWorkload};
pub use occlusion::{
    HYSTERESIS_BATCH, HYSTERESIS_HISTORY_COUNTS, HysteresisCullWorkload, HysteresisHistoryWorkload,
};
pub use pools::{BUFFER_POOL_SLOTS, BufferPoolWorkload};
/// Static many-light shadow planning workload.
pub use pools::{SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS, ShadowChurnWorkload};
pub use skinned_geometry::SkinnedGeometryWorkload;
pub use sort::{TRANSPARENT_SORT_1K, TRANSPARENT_SORT_10K, TransparentSortWorkload};
pub use transforms::{TRANSFORM_BATCH, TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload};
