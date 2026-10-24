//! View-local mesh-swap LOD selection for world mesh draw collection.

use glam::{Mat4, Vec3};
use hashbrown::HashMap;

use crate::camera::{overlay_camera_view_matrix, view_matrix_for_world_mesh_render_space};
use crate::scene::{
    LodEntry, LodRendererKind, LodRendererRef, MeshRendererInstanceId, RenderSpaceId,
    RenderSpaceView, SceneCoordinator, SceneTransformRead, SkinnedMeshRenderer, StaticMeshRenderer,
};
use crate::world_mesh::culling::{
    MeshCullTarget, WorldMeshCullInput, mesh_world_geometry_for_cull_with_head,
};

use super::super::bitset::DenseBitSet;
use super::super::prepared_renderables::{FramePreparedLodEntry, FramePreparedLodGroup};
use super::DrawCollectionInputs;
use super::lod_fade::{
    LodFadeDecision, LodFadeTable, biased_relative_height, commit_prepared_decision,
    commit_resolved_decision, fade_decision_for_prepared_group, fade_decision_for_rows,
    prepared_group_stipples, sanitize_unit_height,
};

/// Conservative relative screen height used when bounds cross the camera plane.
const CAMERA_INTERSECTING_RELATIVE_HEIGHT: f32 = 1.0;
/// Minimum homogeneous `w` accepted for screen-height projection.
const CLIP_W_EPS: f32 = 1e-6;
/// Per-view LOD decision map.
#[derive(Clone, Debug, Default)]
pub(super) struct LodVisibility {
    /// Per-space LOD bitsets and instance-id ordinal lookups.
    spaces: HashMap<RenderSpaceId, LodVisibilitySpace>,
    /// Set when this view stored a non-zero stipple weight.
    ///
    /// Draw collection branches on this flag and skips the per-run space map when it is false,
    /// which is the whole view whenever no group is cross-fading.
    fade_active: bool,
}

impl LodVisibility {
    /// Creates a visibility map with stable renderer ordinals for the queried spaces.
    ///
    /// Weight tables stay empty. The first non-zero stipple weight allocates that space's buffer.
    fn for_spaces(ctx: &DrawCollectionInputs<'_>, space_ids: &[RenderSpaceId]) -> Self {
        let mut spaces = HashMap::with_capacity(space_ids.len());
        for &space_id in space_ids {
            let Some(space) = ctx.scene_assets.scene.space(space_id) else {
                continue;
            };
            if !space.is_active() {
                continue;
            }
            let mut instance_to_ordinal = HashMap::with_capacity(
                space
                    .static_mesh_renderers()
                    .len()
                    .saturating_add(space.skinned_mesh_renderers().len()),
            );
            for renderer in space.static_mesh_renderers() {
                let ordinal = instance_to_ordinal.len();
                instance_to_ordinal.insert(renderer.instance_id, ordinal);
            }
            for renderer in space.skinned_mesh_renderers() {
                let ordinal = instance_to_ordinal.len();
                instance_to_ordinal.insert(renderer.base.instance_id, ordinal);
            }
            let renderer_count = instance_to_ordinal.len();
            let mut grouped = DenseBitSet::default();
            let mut selected = DenseBitSet::default();
            grouped.clear_and_resize(renderer_count);
            selected.clear_and_resize(renderer_count);
            spaces.insert(
                space_id,
                LodVisibilitySpace {
                    instance_to_ordinal,
                    grouped,
                    selected,
                    renderer_count,
                    fades: LodFadeTable::default(),
                },
            );
        }
        Self {
            spaces,
            fade_active: false,
        }
    }

    /// Whether any renderer in this view has a non-zero stipple weight.
    #[cfg(test)]
    #[inline]
    pub(super) fn fade_active(&self) -> bool {
        self.fade_active
    }

    /// Returns whether `instance_id` may emit draws in this view.
    #[inline]
    pub(super) fn renderer_visible(
        &self,
        space_id: RenderSpaceId,
        renderer_ordinal: usize,
    ) -> bool {
        let Some(space) = self.spaces.get(&space_id) else {
            return true;
        };
        !space.grouped.contains(renderer_ordinal) || space.selected.contains(renderer_ordinal)
    }

    /// Returns whether `instance_id` may emit draws when only scene state is available.
    #[inline]
    #[cfg(test)]
    pub(super) fn renderer_visible_by_instance(
        &self,
        space_id: RenderSpaceId,
        instance_id: MeshRendererInstanceId,
    ) -> bool {
        let Some(space) = self.spaces.get(&space_id) else {
            return true;
        };
        let Some(&ordinal) = space.instance_to_ordinal.get(&instance_id) else {
            return true;
        };
        !space.grouped.contains(ordinal) || space.selected.contains(ordinal)
    }

    /// Marks a renderer as owned by an LOD group.
    fn mark_grouped(&mut self, space_id: RenderSpaceId, instance_id: MeshRendererInstanceId) {
        let Some(space) = self.spaces.get_mut(&space_id) else {
            return;
        };
        if let Some(&ordinal) = space.instance_to_ordinal.get(&instance_id) {
            space.grouped.insert(ordinal);
        }
    }

    /// Signed stipple weight for a renderer ordinal. `0` leaves the shader unchanged.
    ///
    /// When [`Self::fade_active`] is false this returns immediately. Callers do not hash the
    /// space map on a view where every group is outside a fade.
    #[inline]
    pub(super) fn lod_fade(&self, space_id: RenderSpaceId, renderer_ordinal: usize) -> f32 {
        if !self.fade_active {
            return 0.0;
        }
        self.spaces
            .get(&space_id)
            .map(|space| space.fades.weight(renderer_ordinal))
            .unwrap_or(0.0)
    }

    /// Signed stipple weight for a stable renderer id. `0` leaves the shader unchanged.
    ///
    /// The scene-walk collector is test-only, so this lookup is compiled with the tests.
    #[cfg(test)]
    #[inline]
    pub(super) fn lod_fade_by_instance(
        &self,
        space_id: RenderSpaceId,
        instance_id: MeshRendererInstanceId,
    ) -> f32 {
        if !self.fade_active {
            return 0.0;
        }
        let Some(space) = self.spaces.get(&space_id) else {
            return 0.0;
        };
        let Some(&ordinal) = space.instance_to_ordinal.get(&instance_id) else {
            return 0.0;
        };
        space.fades.weight(ordinal)
    }

    /// Marks a grouped renderer as selected by the active LOD row.
    pub(super) fn mark_selected(
        &mut self,
        space_id: RenderSpaceId,
        instance_id: MeshRendererInstanceId,
    ) {
        let Some(space) = self.spaces.get_mut(&space_id) else {
            return;
        };
        if let Some(&ordinal) = space.instance_to_ordinal.get(&instance_id) {
            space.selected.insert(ordinal);
        }
    }

    /// Marks a stable renderer ordinal as owned by an LOD group.
    fn mark_grouped_ordinal(&mut self, space_id: RenderSpaceId, renderer_ordinal: usize) {
        let Some(space) = self.spaces.get_mut(&space_id) else {
            return;
        };
        space.grouped.insert(renderer_ordinal);
    }

    /// Dense ordinal for a stable renderer id, when this space was indexed.
    pub(super) fn instance_ordinal(
        &self,
        space_id: RenderSpaceId,
        instance_id: MeshRendererInstanceId,
    ) -> Option<usize> {
        self.spaces
            .get(&space_id)?
            .instance_to_ordinal
            .get(&instance_id)
            .copied()
    }

    /// Records a non-zero stipple weight for one renderer ordinal.
    pub(super) fn record_fade_ordinal(
        &mut self,
        space_id: RenderSpaceId,
        renderer_ordinal: usize,
        weight: f32,
    ) {
        if weight == 0.0 || !weight.is_finite() {
            return;
        }
        let stored = {
            let Some(space) = self.spaces.get_mut(&space_id) else {
                return;
            };
            if space.fades.is_empty() {
                let renderer_count = space.renderer_count;
                space.fades = LodFadeTable::acquire(renderer_count);
            }
            space.fades.record(renderer_ordinal, weight)
        };
        if stored {
            self.fade_active = true;
        }
    }

    /// Marks a stable renderer ordinal as selected by the active LOD row.
    pub(super) fn mark_selected_ordinal(
        &mut self,
        space_id: RenderSpaceId,
        renderer_ordinal: usize,
    ) {
        let Some(space) = self.spaces.get_mut(&space_id) else {
            return;
        };
        space.selected.insert(renderer_ordinal);
    }

    /// Number of grouped renderer bits set across all spaces.
    #[cfg(test)]
    fn grouped_count(&self) -> usize {
        self.spaces
            .values()
            .map(|space| space.grouped.count_ones())
            .sum()
    }

    /// Number of selected renderer bits set across all spaces.
    #[cfg(test)]
    fn selected_count(&self) -> usize {
        self.spaces
            .values()
            .map(|space| space.selected.count_ones())
            .sum()
    }
}

/// One render space's LOD visibility bitsets.
#[derive(Clone, Debug, Default)]
struct LodVisibilitySpace {
    /// Stable renderer identity to current dense ordinal.
    instance_to_ordinal: HashMap<MeshRendererInstanceId, usize>,
    /// All live renderers referenced by at least one LOD group.
    grouped: DenseBitSet,
    /// Renderers selected by their owning group's chosen LOD.
    selected: DenseBitSet,
    /// Renderer count used to size [`Self::fades`] on the first non-zero weight.
    renderer_count: usize,
    /// Stipple weights. Empty, and not zero-filled, until a renderer in this view fades.
    fades: LodFadeTable,
}

/// One LOD group scheduled for view-local selection.
#[derive(Clone, Copy)]
struct LodGroupWork<'a> {
    /// Render space that owns the LOD group.
    space_id: RenderSpaceId,
    /// Borrowed render-space tables used to resolve renderer references.
    space: RenderSpaceView<'a>,
    /// LOD rows in priority order.
    lods: &'a [LodEntry],
    /// Host cross-fade flag. Off keeps the historical single-LOD swap.
    cross_fade: bool,
}

/// Renderer resolved from a stable LOD membership row.
struct ResolvedLodRenderer<'a> {
    /// Stable renderer-local identity.
    pub instance_id: MeshRendererInstanceId,
    /// Static or skinned renderer.
    pub kind: LodRendererKind,
    /// Renderer node id.
    pub node_id: i32,
    /// Whether this renderer is in the overlay layer.
    pub is_overlay: bool,
    /// Resident mesh asset id.
    pub mesh_asset_id: i32,
    /// Skinned renderer payload when `kind` is [`LodRendererKind::Skinned`].
    pub skinned_renderer: Option<&'a SkinnedMeshRenderer>,
}

/// LOD entry after resolving stale renderer ids against current scene tables.
pub(super) struct ResolvedLodEntry {
    /// Threshold copied from scene LOD state.
    pub screen_relative_transition_height: f32,
    /// Live renderer ids in this entry.
    pub renderers: Vec<MeshRendererInstanceId>,
}

/// Builds the view-local LOD visibility map used by prepared and scene-walk collection.
pub(super) fn build_lod_visibility(
    ctx: &DrawCollectionInputs<'_>,
    space_ids: &[RenderSpaceId],
) -> LodVisibility {
    profiling::scope!("mesh::lod_visibility");
    let Some(culling) = ctx.view.culling else {
        return LodVisibility::default();
    };
    if let Some(prepared) = ctx.caches.prepared {
        let group_work = collect_prepared_lod_group_work(prepared.lod_groups(), space_ids);
        if group_work.is_empty() {
            return LodVisibility::default();
        }
        let mut visibility = LodVisibility::for_spaces(ctx, space_ids);
        {
            profiling::scope!("mesh::lod_visibility::select_prepared_groups");
            for group in group_work {
                select_prepared_group_lod(ctx, culling, group, &mut visibility);
            }
        }
        return visibility;
    }
    let capacity = lod_renderer_ref_capacity(ctx, space_ids);
    if capacity == 0 {
        return LodVisibility::default();
    }
    let group_work = collect_lod_group_work(ctx, space_ids);
    if group_work.is_empty() {
        return LodVisibility::default();
    }
    let mut visibility = LodVisibility::for_spaces(ctx, space_ids);
    {
        profiling::scope!("mesh::lod_visibility::select_scene_groups");
        for work in group_work {
            select_group_lod(
                ctx,
                culling,
                work.space_id,
                work.space,
                work.lods,
                work.cross_fade,
                &mut visibility,
            );
        }
    }
    visibility
}

/// Collects prepared LOD groups relevant to the requested render spaces.
fn collect_prepared_lod_group_work<'a>(
    groups: &'a [FramePreparedLodGroup],
    space_ids: &[RenderSpaceId],
) -> Vec<&'a FramePreparedLodGroup> {
    let mut work = Vec::new();
    for &space_id in space_ids {
        work.extend(groups.iter().filter(|group| group.space_id == space_id));
    }
    work
}

/// Selects one pre-resolved LOD group.
fn select_prepared_group_lod(
    ctx: &DrawCollectionInputs<'_>,
    culling: &WorldMeshCullInput<'_>,
    group: &FramePreparedLodGroup,
    visibility: &mut LodVisibility,
) {
    if group.lods.is_empty() {
        return;
    }
    for lod in &group.lods {
        for renderer in &lod.renderers {
            visibility.mark_grouped_ordinal(group.space_id, renderer.renderer_ordinal);
        }
    }

    let view_bounds = if group.world_aabb.is_some() {
        group.world_aabb.map(|bounds| (group.any_overlay, bounds))
    } else {
        scene_lod_group_view_bounds(ctx, group)
    };
    let measured_height = view_bounds.and_then(|(any_overlay, (wmin, wmax))| {
        relative_screen_height_for_group(ctx, culling, group.space_id, any_overlay, wmin, wmax)
            .map(|relative_height| biased_relative_height(relative_height, ctx.view.mesh_lod_bias))
    });
    let selected_index = match measured_height {
        Some(effective_height) => select_prepared_lod_index_at(&group.lods, effective_height),
        None => first_non_empty_prepared_lod(&group.lods),
    };

    let Some(selected_index) = selected_index else {
        return;
    };
    // `cross_fade` was copied onto the prepared group. The off path does not touch the scene
    // space map. The stipple index is read only after the height is inside the fade band.
    let decision = if group.cross_fade {
        let geometric =
            fade_decision_for_prepared_group(group, selected_index, measured_height, true);
        if geometric.partner.is_none() || prepared_group_stipples(ctx, group) {
            geometric
        } else {
            LodFadeDecision::single(selected_index)
        }
    } else {
        LodFadeDecision::single(selected_index)
    };
    commit_prepared_decision(visibility, group, &decision);
}

/// Recomputes view-dependent bounds for a prepared LOD group when cached bounds are unavailable.
fn scene_lod_group_view_bounds(
    ctx: &DrawCollectionInputs<'_>,
    group: &FramePreparedLodGroup,
) -> Option<(bool, (Vec3, Vec3))> {
    let space = ctx.scene_assets.scene.space(group.space_id)?;
    let scene_group = space.lod_groups().get(group.scene_group_index)?;
    let mut world_aabb = None;
    let mut any_overlay = false;
    for lod in &scene_group.lods {
        for renderer_ref in &lod.renderers {
            let Some(renderer) = resolve_lod_renderer(ctx, group.space_id, space, renderer_ref)
            else {
                continue;
            };
            any_overlay |= renderer.is_overlay;
            if let Some(bounds) = world_aabb_for_lod_renderer(ctx, group.space_id, &renderer) {
                union_aabb(&mut world_aabb, bounds);
            }
        }
    }
    world_aabb.map(|bounds| (any_overlay, bounds))
}

/// Estimates the renderer-ref capacity needed by one LOD worker chunk.
#[cfg(test)]
fn lod_group_chunk_capacity(
    total_renderer_refs: usize,
    total_groups: usize,
    chunk_groups: usize,
) -> usize {
    if total_renderer_refs == 0 || total_groups == 0 || chunk_groups == 0 {
        return 0;
    }
    total_renderer_refs
        .div_ceil(total_groups)
        .saturating_mul(chunk_groups)
}

/// Collects active LOD groups into a dense work list for serial or parallel selection.
fn collect_lod_group_work<'a>(
    ctx: &DrawCollectionInputs<'a>,
    space_ids: &[RenderSpaceId],
) -> Vec<LodGroupWork<'a>> {
    let mut work = Vec::new();
    for &space_id in space_ids {
        let Some(space) = ctx.scene_assets.scene.space(space_id) else {
            continue;
        };
        if !space.is_active() || space.lod_groups().is_empty() {
            continue;
        }
        for group in space.lod_groups() {
            work.push(LodGroupWork {
                space_id,
                space,
                lods: group.lods.as_slice(),
                cross_fade: group.cross_fade,
            });
        }
    }
    work
}

/// Counts renderer refs present in active LOD groups for visibility capacity planning.
fn lod_renderer_ref_capacity(ctx: &DrawCollectionInputs<'_>, space_ids: &[RenderSpaceId]) -> usize {
    space_ids
        .iter()
        .filter_map(|&space_id| ctx.scene_assets.scene.space(space_id))
        .filter(|space| space.is_active())
        .flat_map(|space| space.lod_groups().iter())
        .flat_map(|group| group.lods.iter())
        .map(|lod| lod.renderers.len())
        .sum()
}

/// Resolves, bounds, and selects one LOD group.
fn select_group_lod(
    ctx: &DrawCollectionInputs<'_>,
    culling: &WorldMeshCullInput<'_>,
    space_id: RenderSpaceId,
    space: RenderSpaceView<'_>,
    lods: &[LodEntry],
    cross_fade: bool,
    visibility: &mut LodVisibility,
) {
    if lods.is_empty() {
        return;
    }

    let mut resolved_lods = Vec::with_capacity(lods.len());
    let mut world_aabb: Option<(Vec3, Vec3)> = None;
    let mut any_overlay = false;

    for lod in lods {
        let mut resolved_renderers = Vec::with_capacity(lod.renderers.len());
        for renderer_ref in &lod.renderers {
            let Some(renderer) = resolve_lod_renderer(ctx, space_id, space, renderer_ref) else {
                continue;
            };
            visibility.mark_grouped(space_id, renderer.instance_id);
            any_overlay |= renderer.is_overlay;
            if let Some(bounds) = world_aabb_for_lod_renderer(ctx, space_id, &renderer) {
                union_aabb(&mut world_aabb, bounds);
            }
            resolved_renderers.push(renderer.instance_id);
        }
        resolved_lods.push(ResolvedLodEntry {
            screen_relative_transition_height: lod.screen_relative_transition_height,
            renderers: resolved_renderers,
        });
    }

    if resolved_lods.iter().all(|lod| lod.renderers.is_empty()) {
        return;
    }

    let measured_height = world_aabb.and_then(|(wmin, wmax)| {
        relative_screen_height_for_group(ctx, culling, space_id, any_overlay, wmin, wmax)
            .map(|relative_height| biased_relative_height(relative_height, ctx.view.mesh_lod_bias))
    });
    let selected_index = match measured_height {
        Some(effective_height) => select_lod_index_at(&resolved_lods, effective_height),
        None => first_non_empty_lod(&resolved_lods),
    };

    let Some(selected_index) = selected_index else {
        return;
    };
    let geometric = fade_decision_for_rows(
        selected_index,
        resolved_lods.len(),
        Some(lods),
        measured_height,
        cross_fade,
    );
    let decision = if geometric.partner.is_none()
        || super::lod_fade::scene_group_can_cross_fade(ctx, space_id, space, lods)
    {
        geometric
    } else {
        LodFadeDecision::single(selected_index)
    };
    commit_resolved_decision(visibility, space_id, &resolved_lods, &decision);
}

/// Resolves a scene renderer from a stable LOD renderer reference.
fn resolve_lod_renderer<'a>(
    ctx: &DrawCollectionInputs<'_>,
    space_id: RenderSpaceId,
    space: RenderSpaceView<'a>,
    renderer_ref: &LodRendererRef,
) -> Option<ResolvedLodRenderer<'a>> {
    match renderer_ref.kind {
        LodRendererKind::Static => {
            let renderer = static_renderer_for_lod_ref(space, renderer_ref)?;
            if !renderer.emits_visible_color_draws()
                || renderer.mesh_asset_id < 0
                || renderer.node_id < 0
                || ctx
                    .scene_assets
                    .mesh_pool
                    .get(renderer.mesh_asset_id)
                    .is_none()
            {
                return None;
            }
            Some(ResolvedLodRenderer {
                instance_id: renderer.instance_id,
                kind: LodRendererKind::Static,
                node_id: renderer.node_id,
                is_overlay: renderer_is_overlay(ctx.scene_assets.scene, space_id, renderer.node_id),
                mesh_asset_id: renderer.mesh_asset_id,
                skinned_renderer: None,
            })
        }
        LodRendererKind::Skinned => {
            let renderer = skinned_renderer_for_lod_ref(space, renderer_ref)?;
            let base = &renderer.base;
            if !base.emits_visible_color_draws()
                || base.mesh_asset_id < 0
                || base.node_id < 0
                || ctx.scene_assets.mesh_pool.get(base.mesh_asset_id).is_none()
            {
                return None;
            }
            Some(ResolvedLodRenderer {
                instance_id: base.instance_id,
                kind: LodRendererKind::Skinned,
                node_id: base.node_id,
                is_overlay: renderer_is_overlay(ctx.scene_assets.scene, space_id, base.node_id),
                mesh_asset_id: base.mesh_asset_id,
                skinned_renderer: Some(renderer),
            })
        }
    }
}

/// Finds a static renderer by hint first, then by stable instance id.
pub(super) fn static_renderer_for_lod_ref<'a>(
    space: RenderSpaceView<'a>,
    renderer_ref: &LodRendererRef,
) -> Option<&'a StaticMeshRenderer> {
    space
        .static_mesh_renderers()
        .get(renderer_ref.renderable_index_hint)
        .filter(|renderer| renderer.instance_id == renderer_ref.instance_id)
        .or_else(|| {
            space
                .static_mesh_renderers()
                .iter()
                .find(|renderer| renderer.instance_id == renderer_ref.instance_id)
        })
}

/// Finds a skinned renderer by hint first, then by stable instance id.
pub(super) fn skinned_renderer_for_lod_ref<'a>(
    space: RenderSpaceView<'a>,
    renderer_ref: &LodRendererRef,
) -> Option<&'a SkinnedMeshRenderer> {
    space
        .skinned_mesh_renderers()
        .get(renderer_ref.renderable_index_hint)
        .filter(|renderer| renderer.base.instance_id == renderer_ref.instance_id)
        .or_else(|| {
            space
                .skinned_mesh_renderers()
                .iter()
                .find(|renderer| renderer.base.instance_id == renderer_ref.instance_id)
        })
}

/// Returns whether a renderer node is on the overlay layer.
fn renderer_is_overlay(scene: &SceneCoordinator, space_id: RenderSpaceId, node_id: i32) -> bool {
    node_id >= 0
        && SceneTransformRead::transform_is_in_overlay_layer(scene, space_id, node_id as usize)
}

/// Computes a renderer's world-space AABB for LOD group bounds.
fn world_aabb_for_lod_renderer(
    ctx: &DrawCollectionInputs<'_>,
    space_id: RenderSpaceId,
    renderer: &ResolvedLodRenderer<'_>,
) -> Option<(Vec3, Vec3)> {
    let mesh = ctx.scene_assets.mesh_pool.get(renderer.mesh_asset_id)?;
    let target = MeshCullTarget {
        scene: ctx.scene_assets.scene,
        space_id,
        mesh,
        skinned: renderer.kind == LodRendererKind::Skinned,
        skinned_renderer: renderer.skinned_renderer,
        node_id: renderer.node_id,
    };
    mesh_world_geometry_for_cull_with_head(
        &target,
        ctx.view.head_output_transform,
        ctx.view.render_context,
    )
    .world_aabb
}

/// Expands `dst` to include `bounds`.
fn union_aabb(dst: &mut Option<(Vec3, Vec3)>, bounds: (Vec3, Vec3)) {
    match dst {
        Some((min, max)) => {
            *min = min.min(bounds.0);
            *max = max.max(bounds.1);
        }
        None => *dst = Some(bounds),
    }
}

/// Computes group relative screen height for the active view.
fn relative_screen_height_for_group(
    ctx: &DrawCollectionInputs<'_>,
    culling: &WorldMeshCullInput<'_>,
    space_id: RenderSpaceId,
    is_overlay: bool,
    wmin: Vec3,
    wmax: Vec3,
) -> Option<f32> {
    if is_overlay {
        let view_proj = culling.proj.overlay_proj * overlay_camera_view_matrix();
        return Some(projected_aabb_relative_height(view_proj, wmin, wmax));
    }
    let space = ctx.scene_assets.scene.space(space_id)?;
    let view = culling
        .host_camera
        .explicit_world_to_view()
        .unwrap_or_else(|| view_matrix_for_world_mesh_render_space(ctx.scene_assets.scene, space));
    let first = if let Some((left, right)) = culling.proj.vr_stereo {
        let left_height = projected_aabb_relative_height(left, wmin, wmax);
        let right_height = projected_aabb_relative_height(right, wmin, wmax);
        return Some(left_height.max(right_height));
    } else {
        culling.proj.world_proj * view
    };
    Some(projected_aabb_relative_height(first, wmin, wmax))
}

/// Projects a world AABB and returns Unity-style relative screen height.
fn projected_aabb_relative_height(view_proj: Mat4, wmin: Vec3, wmax: Vec3) -> f32 {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for corner in aabb_corners(wmin, wmax) {
        let clip = view_proj * corner.extend(1.0);
        if !clip.is_finite() || clip.w <= CLIP_W_EPS {
            return CAMERA_INTERSECTING_RELATIVE_HEIGHT;
        }
        let y = clip.y / clip.w;
        if !y.is_finite() {
            return CAMERA_INTERSECTING_RELATIVE_HEIGHT;
        }
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    ((max_y - min_y) * 0.5).clamp(0.0, CAMERA_INTERSECTING_RELATIVE_HEIGHT)
}

/// Returns the eight corners of an AABB.
fn aabb_corners(wmin: Vec3, wmax: Vec3) -> [Vec3; 8] {
    [
        Vec3::new(wmin.x, wmin.y, wmin.z),
        Vec3::new(wmax.x, wmin.y, wmin.z),
        Vec3::new(wmin.x, wmax.y, wmin.z),
        Vec3::new(wmax.x, wmax.y, wmin.z),
        Vec3::new(wmin.x, wmin.y, wmax.z),
        Vec3::new(wmax.x, wmin.y, wmax.z),
        Vec3::new(wmin.x, wmax.y, wmax.z),
        Vec3::new(wmax.x, wmax.y, wmax.z),
    ]
}

/// Selects the first LOD whose biased relative height meets its threshold.
#[cfg(test)]
fn select_lod_index(
    lods: &[ResolvedLodEntry],
    relative_height: f32,
    mesh_lod_bias: f32,
) -> Option<usize> {
    select_lod_index_at(lods, biased_relative_height(relative_height, mesh_lod_bias))
}

/// Selects the first LOD whose already-biased height meets its threshold.
fn select_lod_index_at(lods: &[ResolvedLodEntry], effective_height: f32) -> Option<usize> {
    lods.iter().position(|lod| {
        effective_height >= sanitize_unit_height(lod.screen_relative_transition_height)
    })
}

/// Returns the first LOD row that still has any live renderer.
fn first_non_empty_lod(lods: &[ResolvedLodEntry]) -> Option<usize> {
    lods.iter().position(|lod| !lod.renderers.is_empty())
}

/// Selects the first prepared LOD whose already-biased height meets its threshold.
fn select_prepared_lod_index_at(
    lods: &[FramePreparedLodEntry],
    effective_height: f32,
) -> Option<usize> {
    lods.iter().position(|lod| {
        effective_height >= sanitize_unit_height(lod.screen_relative_transition_height)
    })
}

/// Returns the first prepared LOD row that still has any live renderer.
fn first_non_empty_prepared_lod(lods: &[FramePreparedLodEntry]) -> Option<usize> {
    lods.iter().position(|lod| !lod.renderers.is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::lod_fade::lod_cross_fade_coverage;
    use super::*;

    /// Builds a resolved LOD entry with one renderer so selection can ignore emptiness.
    fn lod(threshold: f32, id: u64) -> ResolvedLodEntry {
        ResolvedLodEntry {
            screen_relative_transition_height: threshold,
            renderers: vec![MeshRendererInstanceId(id)],
        }
    }

    #[test]
    fn inactive_fade_lookup_returns_zero_without_a_space_map_hit() {
        let visibility = LodVisibility::default();
        assert!(!visibility.fade_active());
        assert_eq!(visibility.lod_fade(RenderSpaceId(7), 3), 0.0);
    }

    #[test]
    fn select_lod_index_uses_first_matching_threshold() {
        let lods = [lod(0.6, 1), lod(0.2, 2)];

        assert_eq!(select_lod_index(&lods, 0.35, 2.0), Some(0));
        assert_eq!(select_lod_index(&lods, 0.25, 1.0), Some(1));
    }

    #[test]
    fn select_lod_index_returns_none_below_final_threshold() {
        let lods = [lod(0.6, 1), lod(0.2, 2)];

        assert_eq!(select_lod_index(&lods, 0.1, 1.0), None);
    }

    #[test]
    fn first_non_empty_lod_falls_back_when_bounds_are_unavailable() {
        let lods = [
            ResolvedLodEntry {
                screen_relative_transition_height: 0.8,
                renderers: Vec::new(),
            },
            lod(0.2, 2),
        ];

        assert_eq!(first_non_empty_lod(&lods), Some(1));
    }

    #[test]
    fn desktop_lod_bias_keeps_the_golden_quad_inside_the_fade_band() {
        use crate::camera::{reverse_z_perspective, view_matrix_from_render_transform};
        use crate::shared::RenderTransform;
        use glam::Quat;

        let camera = RenderTransform {
            position: Vec3::new(0.0, 0.0, -3.0),
            scale: Vec3::ONE,
            rotation: Quat::IDENTITY,
        };
        let view = view_matrix_from_render_transform(&camera);
        let proj = reverse_z_perspective(1.0, 60.0_f32.to_radians(), 0.05, 500.0);
        let half = 0.5 * 0.7;
        let height = projected_aabb_relative_height(
            proj * view,
            Vec3::new(-half, -half, -0.02),
            Vec3::new(half, half, 0.02),
        );
        let biased = biased_relative_height(height, 2.0);
        let coverage = lod_cross_fade_coverage(0.02, 1.0, 1.0, true, biased, true);
        assert!(
            coverage > 0.05 && coverage < 0.95,
            "height {height} biased {biased} coverage {coverage}"
        );
    }

    #[test]
    fn projected_aabb_relative_height_measures_ndc_height() {
        let height = projected_aabb_relative_height(
            Mat4::IDENTITY,
            Vec3::new(-0.5, -0.25, 0.0),
            Vec3::new(0.5, 0.25, 0.0),
        );

        assert!((height - 0.25).abs() < 1e-6);
    }

    #[test]
    fn lod_visibility_keeps_ungrouped_renderers_visible() {
        let visibility = LodVisibility::default();

        assert!(
            visibility.renderer_visible_by_instance(RenderSpaceId(1), MeshRendererInstanceId(42))
        );
    }

    #[test]
    fn lod_visibility_hides_grouped_unselected_renderers() {
        let mut visibility = LodVisibility::default();
        let grouped = MeshRendererInstanceId(1);
        let selected = MeshRendererInstanceId(2);
        let space_id = RenderSpaceId(1);
        let mut instance_to_ordinal = HashMap::new();
        instance_to_ordinal.insert(grouped, 0);
        instance_to_ordinal.insert(selected, 1);
        let mut space = LodVisibilitySpace {
            instance_to_ordinal,
            grouped: DenseBitSet::default(),
            selected: DenseBitSet::default(),
            renderer_count: 2,
            fades: LodFadeTable::default(),
        };
        space.grouped.insert(0);
        space.grouped.insert(1);
        space.selected.insert(1);
        visibility.spaces.insert(space_id, space);

        assert!(!visibility.renderer_visible_by_instance(space_id, grouped));
        assert!(visibility.renderer_visible_by_instance(space_id, selected));
        assert_eq!(visibility.grouped_count(), 2);
        assert_eq!(visibility.selected_count(), 1);
    }

    #[test]
    fn lod_group_chunk_capacity_scales_refs_by_chunk_size() {
        assert_eq!(lod_group_chunk_capacity(0, 8, 4), 0);
        assert_eq!(lod_group_chunk_capacity(17, 8, 4), 12);
        assert_eq!(lod_group_chunk_capacity(17, 8, 0), 0);
    }
}
