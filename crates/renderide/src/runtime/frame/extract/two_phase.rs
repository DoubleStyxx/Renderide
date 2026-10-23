//! Builds the per-view occlusion hysteresis plan before draw collection.
//!
//! The plan carries the previous classification, a per-space screen-motion bound, and
//! disocclusion rectangles. It does not run a second GPU depth pass.

use glam::{Mat4, Vec3};

use crate::camera::{CameraProjectionKind, HostCameraFrame, view_matrix_for_host_world_mesh_space};
use crate::cull_contract::HiZTemporalState;
use crate::hi_z_cpu::project_world_aabb_uv;
use crate::occlusion::two_phase::{
    CasterKey, DepthClip, MovedCaster, PresentCaster, ScreenRect, SpaceMotion, TwoPhaseFrame,
    VisibilityScratch, disocclusion_rects, plan_disocclusion,
};
use crate::scene::{RenderSpaceId, SceneCoordinator};
use crate::world_mesh::draw_prep::PreparedCasterBounds;
use crate::world_mesh::{FramePreparedRenderables, ViewRenderSpaceScope, WorldMeshCullProjParams};

use super::super::view_plan::FrameViewPlan;

/// One view's hysteresis plan, retained until that view's cull has committed its history.
pub(super) struct ViewOcclusionPhase {
    /// Logical view whose history this plan updates.
    pub view_id: crate::camera::ViewId,
    /// Plan shared with the view's [`crate::world_mesh::WorldMeshCullInput`].
    pub frame: TwoPhaseFrame,
}

/// Builds one hysteresis plan per prepared view that has cull inputs.
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
            let scratch = occlusion.begin_visibility(view.view_id);
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
                    scratch,
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
    scratch: VisibilityScratch,
) -> TwoPhaseFrame {
    let mut resolved_motion = Vec::new();
    let mut present = Vec::new();
    if let Some(prepared) = prepared {
        prepared.for_each_world_caster_bounds(
            |space_id| scope.includes(space_id),
            |caster| {
                note_caster(
                    scene,
                    host_camera,
                    proj,
                    temporal,
                    caster,
                    &mut resolved_motion,
                    &mut present,
                );
            },
        );
    }
    let movers = plan_disocclusion(&scratch.history, &present, |space_id| {
        scope.includes(space_id)
    });
    let stale_rects = stale_rects_for_movers(scene, host_camera, proj, &movers);
    let motions = resolved_motion
        .into_iter()
        .filter_map(|(space_id, motion)| motion.map(|motion| (space_id, motion)))
        .collect();
    TwoPhaseFrame::new(scratch.history, scratch.spare, motions, stale_rects)
}

fn note_caster(
    scene: &SceneCoordinator,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    temporal: Option<&HiZTemporalState>,
    caster: PreparedCasterBounds,
    resolved_motion: &mut Vec<(RenderSpaceId, Option<SpaceMotion>)>,
    present: &mut Vec<PresentCaster>,
) {
    if !resolved_motion
        .iter()
        .any(|(space_id, _)| *space_id == caster.space_id)
    {
        resolved_motion.push((
            caster.space_id,
            space_motion(scene, caster.space_id, host_camera, proj, temporal),
        ));
    }
    present.push(PresentCaster {
        key: CasterKey::new(
            caster.space_id,
            caster.renderable_index,
            caster.instance_id,
            caster.node_id,
            caster.skinned,
        ),
        mesh_asset_id: caster.mesh_asset_id,
        aabb_min: caster.aabb_min,
        aabb_max: caster.aabb_max,
        space_id: caster.space_id,
    });
}

fn stale_rects_for_movers(
    scene: &SceneCoordinator,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    movers: &[MovedCaster],
) -> Vec<ScreenRect> {
    let mut stale = Vec::new();
    for mover in movers {
        stale.extend(disocclusion_rects(
            std::slice::from_ref(mover),
            |min, max| project_union(scene, mover.space_id, host_camera, proj, min, max),
        ));
    }
    stale
}

fn space_motion(
    scene: &SceneCoordinator,
    space_id: RenderSpaceId,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    temporal: Option<&HiZTemporalState>,
) -> Option<SpaceMotion> {
    let temporal = temporal?;
    let space = scene.space(space_id)?;
    let current_view = view_matrix_for_host_world_mesh_space(scene, space, host_camera);
    let previous_view = temporal.prev_view_by_space.get(&space_id).copied()?;
    let clip = DepthClip {
        near: host_camera.clip.near,
        far: host_camera.clip.far,
        orthographic: host_camera.projection_kind == CameraProjectionKind::Orthographic,
    };
    match (temporal.prev_cull.vr_stereo, proj.vr_stereo) {
        (None, None) => Some(SpaceMotion {
            mono: Some((
                temporal.prev_cull.world_proj * previous_view,
                proj.world_proj * current_view,
            )),
            stereo: None,
            clip,
        }),
        (Some(previous), Some(current)) => Some(SpaceMotion {
            mono: None,
            stereo: Some((previous, current)),
            clip,
        }),
        _ => None,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use glam::Mat4;
    use hashbrown::HashMap;

    use super::space_motion;
    use crate::camera::{
        CameraClipPlanes, HostCameraFrame, reverse_z_perspective,
        view_matrix_for_host_world_mesh_space,
    };
    use crate::cull_contract::{HiZTemporalState, WorldMeshCullProjParams};
    use crate::occlusion::two_phase::screen_motion_uv_pad;
    use crate::scene::{RenderSpaceId, SceneCoordinator};
    use crate::shared::RenderTransform;

    fn host() -> HostCameraFrame {
        HostCameraFrame {
            clip: CameraClipPlanes::new(0.01, 100.0),
            ..HostCameraFrame::default()
        }
    }

    fn proj_with(fov_degrees: f32, stereo: Option<(Mat4, Mat4)>) -> WorldMeshCullProjParams {
        WorldMeshCullProjParams {
            world_proj: reverse_z_perspective(1.0, fov_degrees.to_radians(), 0.01, 100.0),
            overlay_proj: Mat4::IDENTITY,
            vr_stereo: stereo,
        }
    }

    fn seeded() -> (SceneCoordinator, RenderSpaceId) {
        let mut scene = SceneCoordinator::new();
        let space_id = RenderSpaceId(1);
        scene.test_seed_space_identity_worlds(space_id, vec![RenderTransform::default()], vec![-1]);
        (scene, space_id)
    }

    fn temporal_with_view(
        scene: &SceneCoordinator,
        space_id: RenderSpaceId,
        host: &HostCameraFrame,
        previous: &WorldMeshCullProjParams,
    ) -> HiZTemporalState {
        let space = scene.space(space_id).expect("seeded space");
        let view = view_matrix_for_host_world_mesh_space(scene, space, host);
        let mut views = HashMap::new();
        views.insert(space_id, view);
        HiZTemporalState {
            prev_cull: *previous,
            prev_view_by_space: Arc::new(views),
            depth_viewport_px: (32, 32),
        }
    }

    #[test]
    fn resize_keeps_a_motion_bound_for_the_same_view() {
        let (scene, space_id) = seeded();
        let host = host();
        let previous = proj_with(60.0, None);
        let temporal = temporal_with_view(&scene, space_id, &host, &previous);
        let motion = space_motion(
            &scene,
            space_id,
            &host,
            &proj_with(90.0, None),
            Some(&temporal),
        )
        .expect("resize still pairs the cameras");
        let (previous, current) = motion.mono.expect("desktop motion");
        let pad = screen_motion_uv_pad(
            previous,
            current,
            glam::Vec3::new(-0.2, -0.2, 2.0),
            glam::Vec3::new(0.2, 0.2, 2.4),
        )
        .expect("box stays in front");
        assert!(pad > 0.0);
        assert_eq!(motion.clip.near, 0.01);
    }

    #[test]
    fn stereo_mismatch_has_no_motion_bound() {
        let (scene, space_id) = seeded();
        let host = host();
        let eye = Mat4::IDENTITY;
        let previous = proj_with(60.0, Some((eye, eye)));
        let temporal = temporal_with_view(&scene, space_id, &host, &previous);
        assert!(
            space_motion(
                &scene,
                space_id,
                &host,
                &proj_with(60.0, None),
                Some(&temporal),
            )
            .is_none()
        );
    }

    #[test]
    fn missing_space_has_no_motion_bound() {
        let scene = SceneCoordinator::new();
        let host = host();
        let temporal = HiZTemporalState {
            prev_cull: proj_with(60.0, None),
            prev_view_by_space: Arc::new(HashMap::new()),
            depth_viewport_px: (32, 32),
        };
        assert!(
            space_motion(
                &scene,
                RenderSpaceId(4),
                &host,
                &proj_with(60.0, None),
                Some(&temporal),
            )
            .is_none()
        );
    }

    #[test]
    fn missing_previous_view_has_no_motion_bound() {
        let (scene, space_id) = seeded();
        let host = host();
        let temporal = HiZTemporalState {
            prev_cull: proj_with(60.0, None),
            prev_view_by_space: Arc::new(HashMap::new()),
            depth_viewport_px: (32, 32),
        };
        assert!(
            space_motion(
                &scene,
                space_id,
                &host,
                &proj_with(60.0, None),
                Some(&temporal),
            )
            .is_none()
        );
    }
}
