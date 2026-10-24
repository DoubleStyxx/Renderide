//! Cached per-renderer CPU cull outcome shared across the renderer's material slots.
//!
//! The decision depends on per-renderer state (scene/space, bone or rest bounds, transform
//! chain, overlay flag, render context), not on which material slot is being expanded. Skinned
//! and rigid renderers share that result. `None` means no culling pass was active, which is how
//! shadow and unculled probe walks keep every draw.

use glam::{Mat4, Vec3};

use crate::occlusion::hysteresis::CasterRef;
use crate::scene::SceneTransformRead;
use crate::shared::RenderingContext;
use crate::world_mesh::culling::{
    CpuCullFailure, MeshCullTarget, SpaceCullCache, WorldMeshCullInput,
    mesh_world_geometry_for_cull_with_head,
};

use super::super::DrawCollectionInputs;
use super::StaticMeshDrawSource;

/// Per-renderer CPU cull outcome shared across the renderer's material slots.
pub(super) enum CachedCull {
    /// Cull ran and accepted; carries the optional rigid world matrix it produced.
    Accepted(Option<Mat4>),
    /// Cull ran and the renderer was rejected by the frustum stage.
    RejectedFrustum,
    /// Cull ran and the renderer was rejected by the Hi-Z stage.
    RejectedHiZ,
}

/// Runs the per-renderer CPU cull once and packages the outcome for downstream slot expansion.
///
/// Frames without a culling pass return `None`. Skinned draws use the posed bone-bounds AABB
/// and share one [`CachedCull`] across every material slot. A skinned draw with no posed box
/// stays visible. The caster is recorded in the view's visibility history when hysteresis is on.
pub(super) fn compute_cached_cull(
    ctx: &DrawCollectionInputs<'_>,
    draw: &StaticMeshDrawSource<'_>,
    is_overlay: bool,
) -> Option<CachedCull> {
    cull_outcome_for_draw(ctx, draw, is_overlay).map(cache_from_outcome)
}

/// Returns the cull outcome for a single slot, falling back to an inline cull for single-slot
/// renderers that bypassed the cache hoist.
pub(super) fn cull_result_for_slot(
    ctx: &DrawCollectionInputs<'_>,
    draw: &StaticMeshDrawSource<'_>,
    is_overlay: bool,
    cached_cull: Option<&CachedCull>,
) -> Option<Result<Option<Mat4>, CpuCullFailure>> {
    match cached_cull {
        Some(cached) => Some(result_from_cached(cached)),
        // Single-slot renderer bypassed the hoist: cull inline so per-slot work matches the
        // cached path. Skinned and rigid draws both take this branch.
        None => cull_outcome_for_draw(ctx, draw, is_overlay),
    }
}

fn cull_outcome_for_draw(
    ctx: &DrawCollectionInputs<'_>,
    draw: &StaticMeshDrawSource<'_>,
    is_overlay: bool,
) -> Option<Result<Option<Mat4>, CpuCullFailure>> {
    cull_mesh_target(
        &MeshCullTarget {
            scene: ctx.scene_assets.scene,
            space_id: draw.space_id,
            mesh_bounds: &draw.mesh.bounds,
            skinned: draw.skinned,
            skinned_renderer: draw.skinned_renderer,
            node_id: draw.renderer.node_id,
        },
        is_overlay,
        ctx.view.culling,
        ctx.view.render_context,
        Some(caster_key(draw)),
    )
}

/// Frustum and Hi-Z decision for one mesh target.
///
/// `None` means the caller did not ask for culling. Skinned targets read
/// [`crate::scene::SkinnedMeshRenderer::posed_object_bounds`]. `caster` is recorded when a
/// hysteresis plan is active; `None` keeps the occlusion test but does not update history.
pub(super) fn cull_mesh_target<S>(
    target: &MeshCullTarget<'_, S>,
    is_overlay: bool,
    culling: Option<&WorldMeshCullInput<'_>>,
    render_context: RenderingContext,
    caster: Option<CasterRef>,
) -> Option<Result<Option<Mat4>, CpuCullFailure>>
where
    S: SceneTransformRead + ?Sized,
{
    let culling = culling?;
    let geom = mesh_world_geometry_for_cull_with_head(
        target,
        culling.host_camera.head_output_transform,
        render_context,
    );
    let mut cache = SpaceCullCache::new();
    Some(cache.cull_geometry_for_caster(
        geom,
        target.scene,
        target.space_id,
        is_overlay,
        culling,
        None,
        caster,
    ))
}

pub(super) fn cache_from_outcome(outcome: Result<Option<Mat4>, CpuCullFailure>) -> CachedCull {
    match outcome {
        Ok(rigid_world_matrix) => CachedCull::Accepted(rigid_world_matrix),
        Err(CpuCullFailure::Frustum | CpuCullFailure::UiRectMask) => CachedCull::RejectedFrustum,
        Err(CpuCullFailure::HiZ) => CachedCull::RejectedHiZ,
    }
}

pub(super) fn result_from_cached(cached: &CachedCull) -> Result<Option<Mat4>, CpuCullFailure> {
    match cached {
        CachedCull::Accepted(matrix) => Ok(*matrix),
        CachedCull::RejectedFrustum => Err(CpuCullFailure::Frustum),
        CachedCull::RejectedHiZ => Err(CpuCullFailure::HiZ),
    }
}

fn caster_key(draw: &StaticMeshDrawSource<'_>) -> CasterRef {
    CasterRef {
        key: crate::occlusion::hysteresis::CasterKey::new(
            draw.space_id,
            draw.renderable_index,
            draw.instance_id.0,
            draw.renderer.node_id,
            draw.skinned,
        ),
        mesh_asset_id: draw.renderer.mesh_asset_id,
    }
}

/// Returns the world-space AABB for reflection-probe selection when the view tracks probes.
///
/// Returns `None` when no probe selection is active or when the renderer's geometry cannot be
/// expressed as an AABB (e.g. instantly skinned). Probe selection compares against the centroid
/// of the AABB.
pub(super) fn world_aabb_for_reflection_probe_selection(
    ctx: &DrawCollectionInputs<'_>,
    draw: &StaticMeshDrawSource<'_>,
) -> Option<(Vec3, Vec3)> {
    ctx.view.reflection_probes?;
    let target = MeshCullTarget {
        scene: ctx.scene_assets.scene,
        space_id: draw.space_id,
        mesh_bounds: &draw.mesh.bounds,
        skinned: draw.skinned,
        skinned_renderer: draw.skinned_renderer,
        node_id: draw.renderer.node_id,
    };
    mesh_world_geometry_for_cull_with_head(
        &target,
        ctx.view.head_output_transform,
        ctx.view.render_context,
    )
    .world_aabb
}
