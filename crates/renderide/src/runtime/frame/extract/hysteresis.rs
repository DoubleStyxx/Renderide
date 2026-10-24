//! Builds the per-view visibility-hysteresis plan before draw collection.
//!
//! The plan carries the previous classification, a per-space screen-motion bound, and
//! disocclusion rectangles. It does not run a second GPU depth pass. Views with more casters
//! than [`HISTORY_CASTER_BUDGET`] publish an empty history and leave the legacy Hi-Z test in
//! place. The count is prepared casters in the view's render-space scope, not casters that
//! survive the frustum. After the count crosses the cap, the pass stays off until it falls to
//! [`crate::occlusion::hysteresis::HISTORY_CASTER_RESUME`].

use glam::{Mat4, Vec3};
use hashbrown::HashMap;

use crate::camera::{CameraProjectionKind, HostCameraFrame, view_matrix_for_host_world_mesh_space};
use crate::cull_contract::HiZTemporalState;
use crate::hi_z_cpu::project_world_aabb_uv;
use crate::occlusion::hysteresis::{
    CasterKey, DepthClip, HISTORY_CASTER_BUDGET, MovedCaster, PresentCaster, ScreenRect,
    SpaceMotion, VisibilityFrameScratch, VisibilityHysteresis, VisibilityScratch,
    disocclusion_rects, history_confirmation_applies, plan_disocclusion,
};
use crate::scene::{RenderSpaceId, SceneCoordinator};
use crate::world_mesh::draw_prep::PreparedCasterBounds;
use crate::world_mesh::{FramePreparedRenderables, ViewRenderSpaceScope, WorldMeshCullProjParams};

use super::super::view_plan::FrameViewPlan;

/// One view's hysteresis plan, retained until that view's cull has committed its history.
pub(super) struct ViewHysteresisPlan {
    /// Logical view whose history this plan updates.
    pub view_id: crate::camera::ViewId,
    /// Plan shared with the view's [`crate::world_mesh::WorldMeshCullInput`] when [`Self::apply`]
    /// is set.
    pub frame: VisibilityHysteresis,
    /// `false` when the prepared caster count is above [`HISTORY_CASTER_BUDGET`], or still inside
    /// the resume band after a previous over-cap frame. Cull then uses the legacy Hi-Z test.
    /// [`Self::frame`] is still finished so the spare map is recycled and the published history
    /// is empty.
    pub apply: bool,
}

/// Builds one hysteresis plan per prepared view that has cull inputs.
pub(super) fn build_view_hysteresis_plans(
    scene: &SceneCoordinator,
    occlusion: &crate::occlusion::OcclusionSystem,
    prepared_views: &[FrameViewPlan<'_>],
    prepared_renderables: &[Option<&FramePreparedRenderables>],
    projections: &[Option<WorldMeshCullProjParams>],
    temporals: &[Option<HiZTemporalState>],
) -> Vec<Option<ViewHysteresisPlan>> {
    prepared_views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let proj = projections.get(index)?.as_ref()?;
            let temporal = temporals.get(index).and_then(|temporal| temporal.as_ref());
            let scratch = occlusion.begin_visibility(view.view_id);
            let prepared = prepared_renderables.get(index).copied().flatten();
            let (frame, frame_scratch, apply) = build_frame(
                scene,
                prepared,
                view.render_space_scope,
                &view.host_camera,
                proj,
                temporal,
                scratch,
            );
            occlusion.restore_frame_scratch(view.view_id, frame_scratch);
            Some(ViewHysteresisPlan {
                view_id: view.view_id,
                frame,
                apply,
            })
        })
        .collect()
}

/// History plan the fine cull should consult.
///
/// `None` when the plan is absent or [`ViewHysteresisPlan::apply`] is false. The cull then uses
/// the previous-frame Hi-Z test on its own, the same reject as a build without this pass.
pub(super) fn attached_hysteresis(
    plan: Option<&ViewHysteresisPlan>,
) -> Option<&VisibilityHysteresis> {
    plan.filter(|plan| plan.apply).map(|plan| &plan.frame)
}

fn build_frame(
    scene: &SceneCoordinator,
    prepared: Option<&FramePreparedRenderables>,
    scope: ViewRenderSpaceScope,
    host_camera: &HostCameraFrame,
    proj: &WorldMeshCullProjParams,
    temporal: Option<&HiZTemporalState>,
    scratch: VisibilityScratch,
) -> (VisibilityHysteresis, VisibilityFrameScratch, bool) {
    let VisibilityScratch {
        history,
        spare,
        suspended,
        mut frame_scratch,
    } = scratch;
    frame_scratch.clear();
    let caster_count = prepared
        .map(|prepared| {
            prepared.world_caster_count_at_most(HISTORY_CASTER_BUDGET, |space_id| {
                scope.includes(space_id)
            })
        })
        .unwrap_or(Some(0));
    if !history_confirmation_applies(caster_count, suspended) {
        return (
            VisibilityHysteresis::new(history, spare, HashMap::new(), Vec::new()),
            frame_scratch,
            false,
        );
    }
    let track_presence = history.any_visible();
    if let Some(prepared) = prepared {
        for &space_id in prepared.active_space_ids() {
            if !scope.includes(space_id) {
                continue;
            }
            frame_scratch.resolved_motion.push((
                space_id,
                space_motion(scene, space_id, host_camera, proj, temporal),
            ));
        }
        if track_presence {
            prepared.for_each_world_caster_bounds(
                |space_id| scope.includes(space_id),
                |caster| {
                    frame_scratch.present.push(present_caster(caster));
                },
            );
        }
    }
    let movers = if track_presence {
        plan_disocclusion(
            &history,
            &frame_scratch.present,
            &mut frame_scratch.present_keys,
            |space_id| scope.includes(space_id),
        )
    } else {
        Vec::new()
    };
    let stale_rects = stale_rects_for_movers(scene, host_camera, proj, &movers);
    let mut motions = HashMap::with_capacity(frame_scratch.resolved_motion.len());
    for (space_id, motion) in &frame_scratch.resolved_motion {
        if let Some(motion) = motion {
            motions.insert(*space_id, *motion);
        }
    }
    (
        VisibilityHysteresis::new(history, spare, motions, stale_rects),
        frame_scratch,
        true,
    )
}

fn present_caster(caster: PreparedCasterBounds) -> PresentCaster {
    PresentCaster {
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
    }
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
    use crate::occlusion::hysteresis::screen_motion_uv_pad;
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

    fn hysteresis_attached(
        groups: &[(RenderSpaceId, usize)],
        scope: crate::world_mesh::ViewRenderSpaceScope,
        suspended: bool,
    ) -> bool {
        use crate::camera::ViewId;
        use crate::occlusion::hysteresis::{
            VisibilityFrameScratch, VisibilityHistory, VisibilityScratch,
        };
        use crate::world_mesh::culling::WorldMeshCullInput;
        use crate::world_mesh::draw_prep::FramePreparedRenderables;

        let host = HostCameraFrame::default();
        let proj = proj_with(60.0, None);
        let prepared = FramePreparedRenderables::test_with_caster_runs(groups);
        let scratch = VisibilityScratch {
            history: Arc::new(VisibilityHistory::default()),
            spare: HashMap::new(),
            suspended,
            frame_scratch: VisibilityFrameScratch::default(),
        };
        let (frame, _frame_scratch, apply) = super::build_frame(
            &SceneCoordinator::new(),
            Some(&prepared),
            scope,
            &host,
            &proj,
            None,
            scratch,
        );
        let plan = super::ViewHysteresisPlan {
            view_id: ViewId::Main,
            frame,
            apply,
        };
        let culling = WorldMeshCullInput {
            proj,
            host_camera: &host,
            hi_z: None,
            hi_z_temporal: None,
            hysteresis: super::attached_hysteresis(Some(&plan)),
        };
        culling.hysteresis.is_some()
    }

    #[test]
    fn prepared_count_above_the_cap_leaves_hysteresis_unset() {
        use crate::occlusion::hysteresis::{HISTORY_CASTER_BUDGET, HISTORY_CASTER_RESUME};
        use crate::world_mesh::ViewRenderSpaceScope;

        let budget = HISTORY_CASTER_BUDGET;
        assert!(
            !hysteresis_attached(
                &[(RenderSpaceId(1), budget + 1)],
                ViewRenderSpaceScope::AllActive,
                false
            ),
            "above the cap the cull input matches a build with no history plan"
        );
        assert!(hysteresis_attached(
            &[(RenderSpaceId(1), budget)],
            ViewRenderSpaceScope::AllActive,
            false
        ));
        assert!(hysteresis_attached(
            &[(RenderSpaceId(1), 2), (RenderSpaceId(2), budget + 1)],
            ViewRenderSpaceScope::single(RenderSpaceId(1)),
            false
        ));
        assert!(!hysteresis_attached(
            &[(RenderSpaceId(1), 2), (RenderSpaceId(2), budget + 1)],
            ViewRenderSpaceScope::single(RenderSpaceId(2)),
            false
        ));
        assert!(
            !hysteresis_attached(
                &[(RenderSpaceId(1), HISTORY_CASTER_RESUME + 1)],
                ViewRenderSpaceScope::AllActive,
                true
            ),
            "a suspended view stays on the legacy Hi-Z test through the resume band"
        );
    }

    #[test]
    fn visibility_frame_scratch_reuses_storage_across_a_second_frame() {
        use crate::camera::ViewId;
        use crate::occlusion::OcclusionSystem;
        use crate::occlusion::hysteresis::{
            CasterKey, CasterRecord, VisibilityClass, VisibilityHistory,
        };
        use crate::world_mesh::ViewRenderSpaceScope;
        use crate::world_mesh::draw_prep::FramePreparedRenderables;

        let occlusion = OcclusionSystem::new();
        let view = ViewId::Main;
        let space_id = RenderSpaceId(1);
        let key = CasterKey::new(space_id, 0, 0, 0, false);
        let visible_history = || {
            let mut casters = HashMap::new();
            casters.insert(
                key,
                CasterRecord {
                    class: VisibilityClass::Visible,
                    aabb_min: glam::Vec3::ZERO,
                    aabb_max: glam::Vec3::ONE,
                    mesh_asset_id: 1,
                },
            );
            VisibilityHistory::from_casters(casters)
        };
        occlusion.store_visibility_history(view, visible_history(), false);
        let prepared = FramePreparedRenderables::test_with_caster_runs(&[(space_id, 3)]);
        let host = HostCameraFrame::default();
        let proj = proj_with(60.0, None);

        let drive = || {
            let scratch = occlusion.begin_visibility(view);
            let (frame, frame_scratch, apply) = super::build_frame(
                &SceneCoordinator::new(),
                Some(&prepared),
                ViewRenderSpaceScope::AllActive,
                &host,
                &proj,
                None,
                scratch,
            );
            assert!(apply);
            assert!(!frame_scratch.present.is_empty());
            assert!(frame_scratch.present_keys.contains(&key));
            drop(frame);
            occlusion.store_visibility_history(view, visible_history(), false);
            frame_scratch
        };

        let first = drive();
        let motion_ptr = first.resolved_motion.as_ptr();
        let motion_cap = first.resolved_motion.capacity();
        let present_ptr = first.present.as_ptr();
        let present_cap = first.present.capacity();
        let stored = first.present_keys.get(&key).expect("present key");
        let key_ptr: *const CasterKey = stored;
        let key_cap = first.present_keys.capacity();
        assert!(motion_cap > 0);
        assert!(present_cap > 0);
        assert!(key_cap > 0);
        occlusion.restore_frame_scratch(view, first);

        let second = drive();
        assert_eq!(second.resolved_motion.as_ptr(), motion_ptr);
        assert_eq!(second.resolved_motion.capacity(), motion_cap);
        assert_eq!(second.present.as_ptr(), present_ptr);
        assert_eq!(second.present.capacity(), present_cap);
        let stored = second.present_keys.get(&key).expect("present key");
        let second_ptr: *const CasterKey = stored;
        assert_eq!(second_ptr, key_ptr);
        assert_eq!(second.present_keys.capacity(), key_cap);
    }
}
