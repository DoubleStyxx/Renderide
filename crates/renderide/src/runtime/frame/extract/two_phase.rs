//! Builds the per-view two-phase occlusion plan before draw collection.

use std::sync::Arc;

use glam::{Mat4, Vec3};
use hashbrown::HashMap;

use crate::camera::{HostCameraFrame, view_matrix_for_host_world_mesh_space};
use crate::cull_contract::HiZTemporalState;
use crate::hi_z_cpu::project_world_aabb_uv;
use crate::occlusion::two_phase::{
    CasterKey, MovedCaster, ScreenRect, TwoPhaseFrame, VisibilityClass, VisibilityHistory,
    bounds_moved, disocclusion_rects, matrices_match_for_occlusion,
};
use crate::scene::{RenderSpaceId, SceneCoordinator};
use crate::world_mesh::draw_prep::PreparedCasterBounds;
use crate::world_mesh::{FramePreparedRenderables, ViewRenderSpaceScope, WorldMeshCullProjParams};

use super::super::view_plan::FrameViewPlan;

/// One view's two-phase plan, retained until that view's cull has committed its history.
pub(super) struct ViewOcclusionPhase {
    /// Logical view whose history this plan updates.
    pub view_id: crate::camera::ViewId,
    /// Plan shared with the view's [`crate::world_mesh::WorldMeshCullInput`].
    pub frame: TwoPhaseFrame,
}

/// Builds one two-phase plan per prepared view that has cull inputs.
pub(super) fn build_view_occlusion_phases(
    scene: &SceneCoordinator,
    occlusion: &crate::occlusion::OcclusionSystem,
    prepared_views: &[FrameViewPlan<'_>],
    prepared_renderables: &[Option<&FramePreparedRenderables>],
    projections: &[Option<WorldMeshCullProjParams>],
    temporals: &[Option<HiZTemporalState>],
) -> Vec<Option<ViewOcclusionPhase>> {
    prepared_views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let proj = projections.get(index)?.as_ref()?;
            let temporal = temporals.get(index).and_then(|temporal| temporal.as_ref());
            let history = occlusion.visibility_history(view.view_id);
            let prepared = prepared_renderables.get(index).copied().flatten();
            Some(ViewOcclusionPhase {
                view_id: view.view_id,
                frame: build_frame(
                    scene,
                    prepared,
                    view.render_space_scope,
                    &view.host_camera,
                    proj,
                    temporal,
                    history,
                ),
            })
        })
        .collect()
}

fn build_frame(
    scene: &SceneCoordinator,
    prepared: Option<&FramePreparedRenderables>,
    scope: ViewRenderSpaceScope,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    temporal: Option<&HiZTemporalState>,
    history: Arc<VisibilityHistory>,
) -> TwoPhaseFrame {
    let mut stable_spaces = HashMap::new();
    let mut movers_by_space: HashMap<RenderSpaceId, Vec<MovedCaster>> = HashMap::new();
    if let Some(prepared) = prepared {
        let mut scan = CasterScan {
            scene,
            host_camera,
            proj,
            temporal,
            history: &history,
            stable_spaces: &mut stable_spaces,
            movers_by_space: &mut movers_by_space,
        };
        prepared.for_each_world_caster_bounds(
            |space_id| scope.includes(space_id),
            |caster| scan.note(caster),
        );
    }
    let stale_rects = stale_rects_for_movers(scene, host_camera, proj, &movers_by_space);
    TwoPhaseFrame::new(history, stable_spaces, stale_rects)
}

struct CasterScan<'a> {
    scene: &'a SceneCoordinator,
    host_camera: &'a HostCameraFrame,
    proj: &'a WorldMeshCullProjParams,
    temporal: Option<&'a HiZTemporalState>,
    history: &'a VisibilityHistory,
    stable_spaces: &'a mut HashMap<RenderSpaceId, bool>,
    movers_by_space: &'a mut HashMap<RenderSpaceId, Vec<MovedCaster>>,
}

impl CasterScan<'_> {
    fn note(&mut self, caster: PreparedCasterBounds) {
        let scene = self.scene;
        let host_camera = self.host_camera;
        let proj = self.proj;
        let temporal = self.temporal;
        let stable = *self
            .stable_spaces
            .entry(caster.space_id)
            .or_insert_with(|| {
                space_camera_stable(scene, caster.space_id, host_camera, proj, temporal)
            });
        if !stable {
            return;
        }
        let key = CasterKey::new(
            caster.space_id,
            caster.renderable_index,
            caster.instance_id,
            caster.node_id,
            caster.skinned,
        );
        let Some(previous) = self.history.get(&key) else {
            return;
        };
        if previous.class != VisibilityClass::Visible {
            return;
        }
        let current = (caster.aabb_min, caster.aabb_max);
        let remembered = (previous.aabb_min, previous.aabb_max);
        if !bounds_moved(remembered, current) {
            return;
        }
        self.movers_by_space
            .entry(caster.space_id)
            .or_default()
            .push(MovedCaster {
                previous_aabb: remembered,
                current_aabb: current,
            });
    }
}

fn stale_rects_for_movers(
    scene: &SceneCoordinator,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    movers_by_space: &HashMap<RenderSpaceId, Vec<MovedCaster>>,
) -> Vec<ScreenRect> {
    let spaces = movers_by_space.keys().copied().collect::<Vec<_>>();
    let mut stale = Vec::new();
    for space_id in spaces {
        let Some(movers) = movers_by_space.get(&space_id) else {
            continue;
        };
        stale.extend(disocclusion_rects(movers, |min, max| {
            project_union(scene, space_id, host_camera, proj, min, max)
        }));
    }
    stale
}

fn space_camera_stable(
    scene: &SceneCoordinator,
    space_id: RenderSpaceId,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    temporal: Option<&HiZTemporalState>,
) -> bool {
    let Some(temporal) = temporal else {
        return false;
    };
    if !projections_match(&temporal.prev_cull, proj) {
        return false;
    }
    let Some(space) = scene.space(space_id) else {
        return false;
    };
    let current = view_matrix_for_host_world_mesh_space(scene, space, host_camera);
    let Some(previous) = temporal.prev_view_by_space.get(&space_id).copied() else {
        return false;
    };
    matrices_match_for_occlusion(previous, current)
}

fn projections_match(
    previous: &WorldMeshCullProjParams,
    current: &WorldMeshCullProjParams,
) -> bool {
    if !matrices_match_for_occlusion(previous.world_proj, current.world_proj) {
        return false;
    }
    match (previous.vr_stereo, current.vr_stereo) {
        (None, None) => true,
        (Some((previous_left, previous_right)), Some((left, right))) => {
            matrices_match_for_occlusion(previous_left, left)
                && matrices_match_for_occlusion(previous_right, right)
        }
        _ => false,
    }
}

fn project_union(
    scene: &SceneCoordinator,
    space_id: RenderSpaceId,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    min: Vec3,
    max: Vec3,
) -> Option<ScreenRect> {
    let space = scene.space(space_id)?;
    if let Some((left, right)) = proj.vr_stereo {
        return union_rects(project_view(left, min, max), project_view(right, min, max));
    }
    let view = view_matrix_for_host_world_mesh_space(scene, space, host_camera);
    project_view(proj.world_proj * view, min, max)
}

fn project_view(view_proj: Mat4, min: Vec3, max: Vec3) -> Option<ScreenRect> {
    let (uv_min, uv_max) = project_world_aabb_uv(view_proj, min, max)?;
    ScreenRect::from_uv(uv_min, uv_max)
}

fn union_rects(left: Option<ScreenRect>, right: Option<ScreenRect>) -> Option<ScreenRect> {
    match (left, right) {
        (Some(left), Some(right)) => Some(ScreenRect {
            min_u: left.min_u.min(right.min_u),
            min_v: left.min_v.min(right.min_v),
            max_u: left.max_u.max(right.max_u),
            max_v: left.max_v.max(right.max_v),
        }),
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}
