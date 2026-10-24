//! Prepared LOD-group snapshots derived from frame-prepared renderer runs.

use std::sync::OnceLock;

use hashbrown::HashMap;

use crate::scene::{MeshRendererInstanceId, RenderSpaceId, WorldMeshSceneRead};

use super::{FramePreparedDraw, FramePreparedRenderables, FramePreparedRun};

/// One renderer referenced by a prepared LOD entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::world_mesh::draw_prep) struct FramePreparedLodRenderer {
    /// Stable per-space renderer ordinal used by dense visibility bitsets.
    pub(in crate::world_mesh::draw_prep) renderer_ordinal: usize,
}

/// One prepared LOD row with live renderer membership pre-resolved.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::world_mesh::draw_prep) struct FramePreparedLodEntry {
    /// Threshold copied from scene LOD state.
    pub(in crate::world_mesh::draw_prep) screen_relative_transition_height: f32,
    /// Host fade width copied from scene LOD state. Read per view without a scene-space lookup.
    pub(in crate::world_mesh::draw_prep) fade_transition_width: f32,
    /// Live renderer ordinals selected by this LOD row.
    pub(in crate::world_mesh::draw_prep) renderers: Vec<FramePreparedLodRenderer>,
}

/// Whether each cross-fade group can stipple, keyed by `(space, scene group index)`.
///
/// Built once per prepared snapshot, from one pass over the draws, and only when a view finds a
/// group inside a fade band. [`FramePreparedRenderables::rebuild_lod_groups`] drops it.
#[derive(Clone, Debug, Default)]
pub(in crate::world_mesh::draw_prep) struct LodStippleIndex {
    by_group: HashMap<(RenderSpaceId, usize), bool>,
}

impl LodStippleIndex {
    /// Stores the precomputed per-group answers.
    pub(in crate::world_mesh::draw_prep) fn new(
        by_group: HashMap<(RenderSpaceId, usize), bool>,
    ) -> Self {
        Self { by_group }
    }

    /// `true` when every member of this group was color unlit when the index was built.
    #[inline]
    pub(in crate::world_mesh::draw_prep) fn group_stipples(
        &self,
        space_id: RenderSpaceId,
        scene_group_index: usize,
    ) -> bool {
        self.by_group
            .get(&(space_id, scene_group_index))
            .copied()
            .unwrap_or(false)
    }
}

/// One prepared LOD group with membership and view-invariant bounds pre-resolved.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::world_mesh::draw_prep) struct FramePreparedLodGroup {
    /// Render space that owns the LOD group.
    pub(in crate::world_mesh::draw_prep) space_id: RenderSpaceId,
    /// Index into the scene render space's LOD group table.
    pub(in crate::world_mesh::draw_prep) scene_group_index: usize,
    /// Host cross-fade flag copied when the group was prepared.
    ///
    /// View selection branches on this bool. A false flag skips the scene-space lookup and the
    /// unlit stipple scan for that group.
    pub(in crate::world_mesh::draw_prep) cross_fade: bool,
    /// Whether any referenced renderer is in the overlay layer.
    pub(in crate::world_mesh::draw_prep) any_overlay: bool,
    /// Cached group bounds when every referenced renderer has view-invariant geometry.
    pub(in crate::world_mesh::draw_prep) world_aabb: Option<(glam::Vec3, glam::Vec3)>,
    /// Ordered LOD entries with stale renderer references removed.
    pub(in crate::world_mesh::draw_prep) lods: Vec<FramePreparedLodEntry>,
}

impl FramePreparedRenderables {
    /// Rebuilds pre-resolved LOD groups from the active scene spaces and current prepared draws.
    pub(super) fn rebuild_lod_groups<S>(&mut self, scene: Option<&S>)
    where
        S: WorldMeshSceneRead + ?Sized,
    {
        self.lod_groups.clear();
        self.lod_stipple = OnceLock::new();
        let Some(scene) = scene else {
            return;
        };
        profiling::scope!("mesh::prepared_renderables::rebuild_lod_groups");
        let renderer_lookup = build_lod_renderer_lookup(&self.draws, &self.runs);
        for &space_id in &self.active_space_ids {
            let Some(lod_groups) = scene.lod_groups(space_id) else {
                continue;
            };
            for (scene_group_index, group) in lod_groups.iter().enumerate() {
                let mut view_dependent_bounds = false;
                let mut prepared_group = FramePreparedLodGroup {
                    space_id,
                    scene_group_index,
                    cross_fade: group.cross_fade,
                    any_overlay: false,
                    world_aabb: None,
                    lods: Vec::new(),
                };
                for lod in &group.lods {
                    let mut prepared_lod = FramePreparedLodEntry {
                        screen_relative_transition_height: lod.screen_relative_transition_height,
                        fade_transition_width: lod.fade_transition_width,
                        renderers: Vec::with_capacity(lod.renderers.len()),
                    };
                    for renderer_ref in &lod.renderers {
                        let key = (space_id, renderer_ref.instance_id);
                        let Some(renderer) = renderer_lookup.get(&key).copied() else {
                            continue;
                        };
                        prepared_group.any_overlay |= renderer.is_overlay;
                        if let Some(bounds) = renderer.world_aabb {
                            if !view_dependent_bounds {
                                union_prepared_lod_aabb(&mut prepared_group.world_aabb, bounds);
                            }
                        } else {
                            view_dependent_bounds = true;
                            prepared_group.world_aabb = None;
                        }
                        prepared_lod.renderers.push(FramePreparedLodRenderer {
                            renderer_ordinal: renderer.renderer_ordinal,
                        });
                    }
                    prepared_group.lods.push(prepared_lod);
                }
                if prepared_group
                    .lods
                    .iter()
                    .any(|lod| !lod.renderers.is_empty())
                {
                    self.lod_groups.push(prepared_group);
                }
            }
        }
    }
}

/// Renderer metadata used while rebuilding prepared LOD groups.
#[derive(Clone, Copy)]
struct PreparedLodRendererLookup {
    /// Stable renderer ordinal.
    renderer_ordinal: usize,
    /// Whether the renderer is in the overlay layer.
    is_overlay: bool,
    /// View-invariant renderer AABB when available.
    world_aabb: Option<(glam::Vec3, glam::Vec3)>,
}

/// Builds a lookup from stable renderer identity to prepared LOD metadata.
fn build_lod_renderer_lookup(
    draws: &[FramePreparedDraw],
    runs: &[FramePreparedRun],
) -> HashMap<(RenderSpaceId, MeshRendererInstanceId), PreparedLodRendererLookup> {
    let mut lookup = HashMap::with_capacity(runs.len());
    for run in runs {
        let Some(first) = draws.get(run.start as usize) else {
            continue;
        };
        lookup.insert(
            (first.space_id, first.instance_id),
            PreparedLodRendererLookup {
                renderer_ordinal: first.renderer_ordinal,
                is_overlay: first.is_overlay,
                world_aabb: first.cull_geometry.and_then(|geometry| geometry.world_aabb),
            },
        );
    }
    lookup
}

/// Expands `dst` to include a prepared renderer AABB.
fn union_prepared_lod_aabb(
    dst: &mut Option<(glam::Vec3, glam::Vec3)>,
    bounds: (glam::Vec3, glam::Vec3),
) {
    match dst {
        Some((min, max)) => {
            *min = min.min(bounds.0);
            *max = max.max(bounds.1);
        }
        None => *dst = Some(bounds),
    }
}
