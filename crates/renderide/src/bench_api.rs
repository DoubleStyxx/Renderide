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

pub use camera::{
    CAMERA_CLEAR_LOAD_OP_CAMERAS, CameraClearLoadOpInput, DOUBLE_BUFFER_CAMERAS,
    DoubleBufferWorkload, camera_clear_load_op_checksum, camera_clear_load_op_inputs,
};
pub use culling::{CULL_BATCH, CullWorkload};
pub use materials::{
    BIND_GROUP_BATCH, BindGroupChurnWorkload, CustomStemLookupWorkload,
    KeywordVariantCacheWorkload, MATERIAL_UNIFORM_BATCH, MaterialUniformUpdateWorkload,
};
pub use occlusion::{
    HYSTERESIS_BATCH, HYSTERESIS_HISTORY_COUNTS, HysteresisCullWorkload, HysteresisHistoryWorkload,
};
pub use pools::{
    BUFFER_POOL_SLOTS, BufferPoolWorkload, SHADOW_CHURN_CASTERS, SHADOW_CHURN_LIGHTS,
    ShadowChurnWorkload,
};
pub use skinned_geometry::SkinnedGeometryWorkload;
pub use sort::{TRANSPARENT_SORT_1K, TRANSPARENT_SORT_10K, TransparentSortWorkload};
pub use transforms::{TRANSFORM_BATCH, TransformWorkload, WORLD_MATRIX_NODES, WorldMatrixWorkload};
