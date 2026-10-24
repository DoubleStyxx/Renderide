//! Flatten scene mesh renderables into arranged draw items for backend world-mesh frame planning.
//!
//! Batches are keyed by raster pipeline kind (from host shader -> [`crate::materials::resolve_raster_pipeline`]),
//! material asset id, property block slot0, and skinned--ordering mirrors Unity-style batch boundaries so
//! pipeline and future per-material bind groups change only on boundaries.
//!
//! Optional CPU frustum and Hi-Z culling share one bounds evaluation per draw slot
//! ([`super::culling::mesh_draw_passes_cpu_cull`]) using the same view-projection rules as the forward pass
//! ([`super::culling::build_world_mesh_cull_proj_params`]).
//!
//! Per-space draw collection runs in parallel ([`rayon`]) by default; the merged list is arranged
//! into nontransparent phase bins while the transparent tail keeps strict ordering. When
//! [`queue_draws_with_parallelism`] uses [`WorldMeshDrawCollectParallelism::SerialInnerForNestedBatch`]
//! (e.g. prefetching multiple secondary RTs under an outer `par_iter`), inner collection stays
//! serial while final chunked arrangement still makes its own Rayon admission decision.

mod arrange;
mod bitset;
mod canvas_depth;
mod collect;
mod command_cache;
mod filter;
pub(crate) mod item;
mod prepared_renderables;
mod render_world;
mod sort;

#[cfg(feature = "bench-api")]
pub(crate) use canvas_depth::snap_canvas_sibling_plane_depths;
#[cfg(any(test, feature = "bench-api"))]
pub(crate) use canvas_depth::sync_transparent_depth_key;
pub(crate) use canvas_depth::{
    apply_reverse_z_ndc_bias, assign_canvas_depth_steps, canvas_bias_projection, canvas_ndc_z_bias,
    canvas_plane_distance_meters, transparent_depth_key_from_parts,
};
pub(crate) use collect::queue_prepared_draws_for_views_with_parallelism;
pub use collect::{
    DrawCollectionFrameCaches, DrawCollectionInputs, DrawCollectionMaterialInputs,
    DrawCollectionSceneAssets, DrawCollectionViewInputs, QueuedWorldMeshDraws, ViewLayerPolicy,
    ViewRenderSpaceScope, WorldMeshDrawArrangeParallelism, WorldMeshDrawCollectParallelism,
    queue_draws_with_parallelism,
};
#[cfg(feature = "bench-api")]
pub(crate) use collect::{lod_cross_fade_coverage, lod_fade_decision};
pub use command_cache::WorldMeshCommandCacheStats;
pub(crate) use command_cache::{WorldMeshCommandCache, fingerprint_world_mesh_draws};
pub use filter::{CameraTransformDrawFilter, draw_filter_from_camera_entry};
pub use item::{
    WorldMeshDrawArrangementStats, WorldMeshDrawCollection, WorldMeshDrawItem,
    WorldMeshVisibilityStats,
};
pub use prepared_renderables::FramePreparedRenderables;
pub use render_world::{RenderWorld, RenderWorldMaintenanceStats};
#[cfg(any(test, feature = "bench-api"))]
pub use sort::pack_sort_prefix;
#[cfg(test)]
pub(crate) use sort::sort_draws;
#[cfg(feature = "bench-api")]
pub(crate) use sort::{
    sort_order_sensitive_draws_master, sort_order_sensitive_draws_precomputed,
    sort_order_sensitive_draws_recomputing_depth,
};

#[cfg(test)]
mod tests;
