//! CPU frustum and Hi-Z culling helpers for [`crate::world_mesh::draw_prep::queue_draws_with_parallelism`].
//!
//! Shares one bounds evaluation per draw slot using the same view-projection rules as the forward pass
//! ([`super::build_world_mesh_cull_proj_params`]), including
//! [`crate::camera::HostCameraFrame::explicit_world_to_view`] when an explicit camera view is
//! present (e.g. for secondary render-texture cameras).

use glam::{Mat4, Vec3, Vec4};

use crate::scene::{RenderSpaceId, SceneSpaceRead};
#[cfg(test)]
use crate::shared::RenderingContext;

use super::frustum::world_aabb_visible_in_homogeneous_clip;
use super::geometry::MeshCullGeometry;
#[cfg(test)]
use super::geometry::{MeshCullTarget, mesh_world_geometry_for_cull};
use super::{HiZTemporalState, WorldMeshCullInput};
use crate::camera::{overlay_camera_view_matrix, view_matrix_for_world_mesh_render_space};
use crate::hi_z_cpu::snapshot::HiZCpuSnapshot;
use crate::hi_z_cpu::{
    AabbHiZSample, HiZCullData, hi_z_view_proj_matrices, mesh_fully_occluded_in_hiz,
    sample_aabb_occlusion, stereo_hiz_keeps_draw,
};
use crate::occlusion::two_phase::{
    CasterKey, OcclusionFacts, ScreenRect, covering_depth_is_other_occluder,
};

/// Clip matrices for one render space, built once and reused across every AABB in that space.
///
/// The one-shot frustum helper used to invert the space view transform and multiply the
/// projection on every box. Spatial queries and per-renderer fine culls test thousands of boxes
/// against one view, so the matrix work is hoisted here. The accept/reject result matches the
/// per-call path, including "missing space stays visible" and stereo OR.
#[derive(Clone, Copy)]
pub(crate) struct PreparedSpaceCull<'a> {
    /// Render space this plan was built for.
    space_id: RenderSpaceId,
    /// Frustum half-spaces for this space.
    frustum: SpaceFrustum,
    /// Previous-frame Hi-Z test, when the temporal snapshot applies to this space.
    hi_z: Option<PreparedSpaceHiZ<'a>>,
    /// Two-phase confirmation for this view. Absent on broadphase and legacy callers.
    two_phase: Option<&'a crate::occlusion::two_phase::TwoPhaseFrame>,
}

/// Precomputed homogeneous clip matrices for one space.
#[derive(Clone, Copy)]
enum SpaceFrustum {
    /// No render space: keep the draw, matching the historical early accept.
    AcceptAll,
    /// Single view-projection (desktop world camera or overlay camera).
    Mono(Mat4),
    /// VR: visible when either eye's clip volume contains the box.
    Stereo(Mat4, Mat4),
}

/// Previous-frame Hi-Z matrices and pyramid borrows for one space.
#[derive(Clone, Copy)]
enum PreparedSpaceHiZ<'a> {
    /// Desktop pyramid tested with the previous mono view-projection.
    Desktop {
        /// Pyramid sampled by the occlusion test.
        snapshot: &'a HiZCpuSnapshot,
        /// Previous-frame clip matrix.
        view_proj: Mat4,
    },
    /// Both eyes must occlude the box before it is rejected.
    Stereo {
        /// Left-eye pyramid.
        left: &'a HiZCpuSnapshot,
        /// Right-eye pyramid.
        right: &'a HiZCpuSnapshot,
        /// Left-eye clip matrix from the frame that built the pyramid.
        left_vp: Mat4,
        /// Right-eye clip matrix from the frame that built the pyramid.
        right_vp: Mat4,
    },
}

impl<'a> PreparedSpaceCull<'a> {
    /// Builds the frustum for `space_id` without preparing Hi-Z.
    ///
    /// Broadphase queries (mesh runs, light volumes) only need the clip test. Skipping Hi-Z
    /// avoids the temporal lookup on paths that never occlusion-cull.
    pub(crate) fn frustum<S>(
        scene: &S,
        space_id: RenderSpaceId,
        is_overlay: bool,
        culling: &'a WorldMeshCullInput<'_>,
    ) -> Self
    where
        S: SceneSpaceRead + ?Sized,
    {
        Self {
            space_id,
            frustum: space_frustum(scene, space_id, is_overlay, culling),
            hi_z: None,
            two_phase: None,
        }
    }

    /// Builds the frustum and the reusable Hi-Z test for one fine-cull space.
    pub(crate) fn with_hi_z<S>(
        scene: &S,
        space_id: RenderSpaceId,
        is_overlay: bool,
        culling: &'a WorldMeshCullInput<'_>,
    ) -> Self
    where
        S: SceneSpaceRead + ?Sized,
    {
        Self {
            space_id,
            frustum: space_frustum(scene, space_id, is_overlay, culling),
            hi_z: if is_overlay {
                None
            } else {
                prepare_space_hi_z(space_id, culling)
            },
            two_phase: if is_overlay { None } else { culling.two_phase },
        }
    }

    /// Returns whether `wmin`/`wmax` intersects this space's clip volume.
    #[inline]
    pub(crate) fn aabb_visible(&self, wmin: Vec3, wmax: Vec3) -> bool {
        match self.frustum {
            SpaceFrustum::AcceptAll => true,
            SpaceFrustum::Mono(view_proj) => {
                world_aabb_visible_in_homogeneous_clip(view_proj, wmin, wmax)
            }
            SpaceFrustum::Stereo(left, right) => {
                world_aabb_visible_in_homogeneous_clip(left, wmin, wmax)
                    || world_aabb_visible_in_homogeneous_clip(right, wmin, wmax)
            }
        }
    }

    /// Returns whether Hi-Z fully occludes the box. `false` when this space has no usable pyramid.
    #[inline]
    pub(crate) fn hi_z_occludes(&self, wmin: Vec3, wmax: Vec3) -> bool {
        match self.hi_z {
            None => false,
            Some(PreparedSpaceHiZ::Desktop {
                snapshot,
                view_proj,
            }) => mesh_fully_occluded_in_hiz(snapshot, view_proj, wmin, wmax),
            Some(PreparedSpaceHiZ::Stereo {
                left,
                right,
                left_vp,
                right_vp,
            }) => {
                let occluded_left = mesh_fully_occluded_in_hiz(left, left_vp, wmin, wmax);
                let occluded_right = mesh_fully_occluded_in_hiz(right, right_vp, wmin, wmax);
                !stereo_hiz_keeps_draw(occluded_left, occluded_right)
            }
        }
    }

    /// Hi-Z coverage used by the two-phase confirmation. Missing pyramids do not occlude.
    fn occlusion_facts(&self, wmin: Vec3, wmax: Vec3) -> OcclusionFacts {
        match self.hi_z {
            None => OcclusionFacts::keep(),
            Some(PreparedSpaceHiZ::Desktop {
                snapshot,
                view_proj,
            }) => facts_from_mono(sample_aabb_occlusion(snapshot, view_proj, wmin, wmax)),
            Some(PreparedSpaceHiZ::Stereo {
                left,
                right,
                left_vp,
                right_vp,
            }) => facts_from_stereo(
                sample_aabb_occlusion(left, left_vp, wmin, wmax),
                sample_aabb_occlusion(right, right_vp, wmin, wmax),
            ),
        }
    }
}

fn facts_from_mono(sample: Option<AabbHiZSample>) -> OcclusionFacts {
    OcclusionFacts {
        occluded: sample.is_some_and(|sample| sample.occluded),
        other_occluder: sample.is_some_and(sample_is_other_occluder),
        rects: [sample.and_then(sample_rect), None],
    }
}

fn facts_from_stereo(left: Option<AabbHiZSample>, right: Option<AabbHiZSample>) -> OcclusionFacts {
    OcclusionFacts {
        occluded: left.is_some_and(|sample| sample.occluded)
            && right.is_some_and(|sample| sample.occluded),
        other_occluder: left.is_some_and(sample_is_other_occluder)
            && right.is_some_and(sample_is_other_occluder),
        rects: [left.and_then(sample_rect), right.and_then(sample_rect)],
    }
}

fn sample_is_other_occluder(sample: AabbHiZSample) -> bool {
    sample.occluded
        && sample
            .covering_depth
            .is_some_and(|depth| covering_depth_is_other_occluder(sample.closest_ndc_z, depth))
}

fn sample_rect(sample: AabbHiZSample) -> Option<ScreenRect> {
    ScreenRect::from_uv(sample.uv_min, sample.uv_max)
}

/// Reuses one [`PreparedSpaceCull`] across the renderer runs in a collection chunk.
///
/// A chunk usually stays inside one or two render spaces. The cache keeps the inverted view
/// matrix and the Hi-Z clip matrix for those spaces instead of rebuilding them per renderer.
pub(crate) struct SpaceCullCache<'a> {
    /// Prepared cull state in first-seen `(space, overlay)` order.
    slots: Vec<SpaceCullSlot<'a>>,
}

/// One cached space inside [`SpaceCullCache`].
struct SpaceCullSlot<'a> {
    /// Render space this slot was prepared for.
    space_id: RenderSpaceId,
    /// Overlay flag used while preparing the slot. The same space can be visited both ways.
    is_overlay: bool,
    /// Frustum and Hi-Z state for [`Self::space_id`].
    prepared: PreparedSpaceCull<'a>,
}

impl<'a> SpaceCullCache<'a> {
    /// Builds an empty cache. Slots are filled on the first draw from each space.
    pub(crate) fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Runs frustum and Hi-Z culling for `geom`, reusing a prepared space when one exists.
    #[cfg(test)]
    pub(crate) fn cull_geometry<S>(
        &mut self,
        geom: MeshCullGeometry,
        scene: &S,
        space_id: RenderSpaceId,
        is_overlay: bool,
        culling: &'a WorldMeshCullInput<'_>,
        ui_rect_clip_local: Option<Vec4>,
    ) -> Result<Option<Mat4>, CpuCullFailure>
    where
        S: SceneSpaceRead + ?Sized,
    {
        self.cull_geometry_for_caster(
            geom,
            scene,
            space_id,
            is_overlay,
            culling,
            ui_rect_clip_local,
            None,
        )
    }

    /// Runs frustum and two-phase occlusion culling for `geom`.
    ///
    /// `caster` is recorded in the view's visibility history when a two-phase plan is active.
    /// `None` keeps the draw without updating that history.
    #[expect(
        clippy::too_many_arguments,
        reason = "caster is the one extra identity on the existing per-draw cull signature"
    )]
    pub(crate) fn cull_geometry_for_caster<S>(
        &mut self,
        geom: MeshCullGeometry,
        scene: &S,
        space_id: RenderSpaceId,
        is_overlay: bool,
        culling: &'a WorldMeshCullInput<'_>,
        ui_rect_clip_local: Option<Vec4>,
        caster: Option<CasterKey>,
    ) -> Result<Option<Mat4>, CpuCullFailure>
    where
        S: SceneSpaceRead + ?Sized,
    {
        let slot = self.slot_index(scene, space_id, is_overlay, culling);
        mesh_cpu_cull_with_prepared(
            geom,
            is_overlay,
            &self.slots[slot].prepared,
            ui_rect_clip_local,
            caster,
        )
    }

    /// Returns the slot for `(space_id, is_overlay)`, preparing it on first use.
    fn slot_index<S>(
        &mut self,
        scene: &S,
        space_id: RenderSpaceId,
        is_overlay: bool,
        culling: &'a WorldMeshCullInput<'_>,
    ) -> usize
    where
        S: SceneSpaceRead + ?Sized,
    {
        if let Some(index) = self
            .slots
            .iter()
            .position(|slot| slot.space_id == space_id && slot.is_overlay == is_overlay)
        {
            return index;
        }
        self.slots.push(SpaceCullSlot {
            space_id,
            is_overlay,
            prepared: PreparedSpaceCull::with_hi_z(scene, space_id, is_overlay, culling),
        });
        self.slots.len() - 1
    }
}

/// Clip matrices for one space.
///
/// Overlay draws use the fixed overlay camera. A missing render space accepts every box. Stereo
/// keeps a box when either eye can see it and does not multiply by the space view. Desktop mono
/// uses `world_proj * view`, with an explicit camera view winning over the space view.
fn space_frustum<S>(
    scene: &S,
    space_id: RenderSpaceId,
    is_overlay: bool,
    culling: &WorldMeshCullInput<'_>,
) -> SpaceFrustum
where
    S: SceneSpaceRead + ?Sized,
{
    if is_overlay {
        return SpaceFrustum::Mono(culling.proj.overlay_proj * overlay_camera_view_matrix());
    }
    if scene.space(space_id).is_none() {
        return SpaceFrustum::AcceptAll;
    }
    if let Some((left, right)) = culling.proj.vr_stereo {
        return SpaceFrustum::Stereo(left, right);
    }
    let Some(space) = scene.space(space_id) else {
        return SpaceFrustum::AcceptAll;
    };
    let view = culling
        .host_camera
        .explicit_world_to_view()
        .unwrap_or_else(|| view_matrix_for_world_mesh_render_space(scene, space));
    SpaceFrustum::Mono(culling.proj.world_proj * view)
}

/// Hoists the per-draw Hi-Z matrix multiply and pyramid lookup for one space.
///
/// [`hi_z_view_proj_matrices`] allocates, and the old fine cull called it for every surviving
/// draw. The matrices only depend on the space and the temporal snapshot, so one call serves
/// every box in that space. An empty matrix list or a stereo/desktop mismatch keeps the draw.
fn prepare_space_hi_z<'a>(
    space_id: RenderSpaceId,
    culling: &'a WorldMeshCullInput<'_>,
) -> Option<PreparedSpaceHiZ<'a>> {
    let hi_z = culling.hi_z.as_ref()?;
    let temporal = culling.hi_z_temporal.as_ref()?;
    if !hi_z_snapshot_matches_temporal(hi_z, temporal) {
        return None;
    }
    // Stereo tests do not multiply by this view, but a missing entry still disables Hi-Z.
    let prev_view = temporal.prev_view_by_space.get(&space_id).copied()?;
    match hi_z {
        HiZCullData::Desktop(snapshot) => {
            if temporal.prev_cull.vr_stereo.is_some() {
                return None;
            }
            let view_proj = hi_z_view_proj_matrices(&temporal.prev_cull, prev_view, false)
                .first()
                .copied()?;
            Some(PreparedSpaceHiZ::Desktop {
                snapshot,
                view_proj,
            })
        }
        HiZCullData::Stereo { left, right } => {
            let (left_vp, right_vp) = temporal.prev_cull.vr_stereo?;
            Some(PreparedSpaceHiZ::Stereo {
                left,
                right,
                left_vp,
                right_vp,
            })
        }
    }
}

/// Returns whether a world-space AABB is visible to the culling frustum for one render space.
///
/// This is the frustum-only portion of [`mesh_cpu_cull_with_geometry`]. Production spatial
/// queries build a [`PreparedSpaceCull`] once and call [`PreparedSpaceCull::aabb_visible`].
#[cfg(test)]
pub(crate) fn world_aabb_visible_for_cull<S>(
    scene: &S,
    space_id: RenderSpaceId,
    is_overlay: bool,
    culling: &WorldMeshCullInput<'_>,
    wmin: Vec3,
    wmax: Vec3,
) -> bool
where
    S: SceneSpaceRead + ?Sized,
{
    PreparedSpaceCull::frustum(scene, space_id, is_overlay, culling).aabb_visible(wmin, wmax)
}

/// Which CPU cull stage rejected the draw (for diagnostics counters).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CpuCullFailure {
    Frustum,
    HiZ,
    /// Overlay UI draw rejected by the object-local rect-clip mask projected to screen space.
    UiRectMask,
}

/// Frustum + optional Hi-Z culling using a single [`mesh_world_geometry_for_cull`] evaluation.
///
/// On success, returns the rigid world matrix when the draw is non-skinned and the matrix was
/// computed while building bounds (reuse in the forward pass).
///
/// `ui_rect_clip_local` is the object-local UI rect (`xMin, yMin, xMax, yMax`) for overlay draws
/// that opt in to `_RectClip`. When `Some`, the overlay path projects the rect's four corners
/// through the same desktop overlay camera view-projection used by the draw pass and rejects the
/// draw when the projected screen-space AABB doesn't intersect the viewport.
#[cfg(test)]
pub(crate) fn mesh_draw_passes_cpu_cull<S>(
    target: &MeshCullTarget<'_, S>,
    is_overlay: bool,
    culling: &WorldMeshCullInput<'_>,
    render_context: RenderingContext,
    ui_rect_clip_local: Option<Vec4>,
    caster: Option<CasterKey>,
) -> Result<Option<Mat4>, CpuCullFailure>
where
    S: crate::scene::SceneTransformRead + ?Sized,
{
    let geom = mesh_world_geometry_for_cull(target, culling, render_context);
    mesh_cpu_cull_with_geometry(
        geom,
        target.scene,
        target.space_id,
        is_overlay,
        culling,
        ui_rect_clip_local,
        caster,
    )
}

/// Like [`mesh_draw_passes_cpu_cull`] but skips the [`mesh_world_geometry_for_cull`] call when
/// the caller already has a frame-time precomputed [`MeshCullGeometry`].
///
/// Production collection reuses one [`PreparedSpaceCull`] per space through [`SpaceCullCache`].
/// This wrapper stays for tests that compare the one-shot path with that cache.
#[cfg(test)]
pub(crate) fn mesh_cpu_cull_with_geometry<S>(
    geom: MeshCullGeometry,
    scene: &S,
    space_id: RenderSpaceId,
    is_overlay: bool,
    culling: &WorldMeshCullInput<'_>,
    ui_rect_clip_local: Option<Vec4>,
    caster: Option<CasterKey>,
) -> Result<Option<Mat4>, CpuCullFailure>
where
    S: SceneSpaceRead + ?Sized,
{
    let prepared = PreparedSpaceCull::with_hi_z(scene, space_id, is_overlay, culling);
    mesh_cpu_cull_with_prepared(geom, is_overlay, &prepared, ui_rect_clip_local, caster)
}

/// Frustum, Hi-Z, and overlay rect tests against a space cull plan built once for many draws.
fn mesh_cpu_cull_with_prepared(
    geom: MeshCullGeometry,
    is_overlay: bool,
    prepared: &PreparedSpaceCull<'_>,
    ui_rect_clip_local: Option<Vec4>,
    caster: Option<CasterKey>,
) -> Result<Option<Mat4>, CpuCullFailure> {
    if is_overlay {
        return cull_overlay_draw(ui_rect_clip_local, &geom, prepared);
    }

    let Some((wmin, wmax)) = geom.world_aabb else {
        return Ok(geom.rigid_world_matrix);
    };
    if !prepared.aabb_visible(wmin, wmax) {
        return Err(CpuCullFailure::Frustum);
    }
    if !occlusion_allows_draw(prepared, caster, wmin, wmax) {
        return Err(CpuCullFailure::HiZ);
    }
    Ok(geom.rigid_world_matrix)
}

fn occlusion_allows_draw(
    prepared: &PreparedSpaceCull<'_>,
    caster: Option<CasterKey>,
    wmin: Vec3,
    wmax: Vec3,
) -> bool {
    let Some(frame) = prepared.two_phase else {
        return !prepared.hi_z_occludes(wmin, wmax);
    };
    frame.allows_draw(
        prepared.space_id,
        caster,
        wmin,
        wmax,
        prepared.occlusion_facts(wmin, wmax),
    )
}

/// Cull decision for overlay-layer draws: optional `_RectClip` projection check, then accept.
///
/// Overlay draws bypass the world-space frustum and Hi-Z stages by design -- their model matrix
/// already encodes screen-space position via
/// [`crate::scene::SceneCoordinator::overlay_layer_model_matrix_for_context`].
fn cull_overlay_draw(
    ui_rect_clip_local: Option<Vec4>,
    geom: &MeshCullGeometry,
    prepared: &PreparedSpaceCull<'_>,
) -> Result<Option<Mat4>, CpuCullFailure> {
    if let (Some(rect), Some(model)) = (ui_rect_clip_local, geom.rigid_world_matrix) {
        let corners = [
            model.transform_point3(Vec3::new(rect.x, rect.y, 0.0)),
            model.transform_point3(Vec3::new(rect.z, rect.y, 0.0)),
            model.transform_point3(Vec3::new(rect.z, rect.w, 0.0)),
            model.transform_point3(Vec3::new(rect.x, rect.w, 0.0)),
        ];
        let (wmin, wmax) = aabb_from_points(&corners);
        if !prepared.aabb_visible(wmin, wmax) {
            return Err(CpuCullFailure::UiRectMask);
        }
    }
    Ok(geom.rigid_world_matrix)
}

/// Projects the object-local UI rect through `model` then through the overlay projection and
/// returns `true` when its screen-space AABB intersects the viewport.
///
/// `_Rect` is in **object-local** space (matches `obj_xy` in `ui_unlit.wgsl`); the four corners
/// are `(rect.x|z, rect.y|w, 0)`. Overlay models already encode screen-space-relative position via
/// [`crate::scene::SceneCoordinator::overlay_layer_model_matrix_for_context`], so this uses the
/// fixed desktop overlay camera view matrix rather than the world camera view.
///
pub(crate) fn overlay_rect_clip_visible(
    _scene: &(impl SceneSpaceRead + ?Sized),
    _space_id: RenderSpaceId,
    culling: &WorldMeshCullInput<'_>,
    rect: Vec4,
    model: Mat4,
) -> bool {
    let corners = [
        model.transform_point3(Vec3::new(rect.x, rect.y, 0.0)),
        model.transform_point3(Vec3::new(rect.z, rect.y, 0.0)),
        model.transform_point3(Vec3::new(rect.z, rect.w, 0.0)),
        model.transform_point3(Vec3::new(rect.x, rect.w, 0.0)),
    ];
    let (wmin, wmax) = aabb_from_points(&corners);
    let view_proj = culling.proj.overlay_proj * overlay_camera_view_matrix();
    world_aabb_visible_in_homogeneous_clip(view_proj, wmin, wmax)
}

#[inline]
fn aabb_from_points(points: &[Vec3]) -> (Vec3, Vec3) {
    let mut min = points[0];
    let mut max = points[0];
    for &p in &points[1..] {
        min = min.min(p);
        max = max.max(p);
    }
    (min, max)
}

/// Ensures CPU Hi-Z dimensions match the temporal viewport used when the pyramid was built.
fn hi_z_snapshot_matches_temporal(hi: &HiZCullData, t: &HiZTemporalState) -> bool {
    let (w, h) = t.depth_viewport_px;
    match hi {
        HiZCullData::Desktop(s) => s.base_width == w && s.base_height == h,
        HiZCullData::Stereo { left, .. } => left.base_width == w && left.base_height == h,
    }
}

#[cfg(test)]
mod hi_z_temporal_match_tests {
    //! [`super::hi_z_snapshot_matches_temporal`] dimension checks (stale-pyramid guard).

    use std::sync::Arc;

    use glam::Mat4;
    use hashbrown::HashMap;

    use super::hi_z_snapshot_matches_temporal;
    use crate::hi_z_cpu::pyramid::total_float_count;
    use crate::hi_z_cpu::{HiZCpuSnapshot, HiZCullData};
    use crate::world_mesh::culling::{HiZTemporalState, WorldMeshCullProjParams};

    fn dummy_temporal(depth_viewport_px: (u32, u32)) -> HiZTemporalState {
        HiZTemporalState {
            prev_cull: WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
            prev_view_by_space: Arc::new(HashMap::new()),
            depth_viewport_px,
        }
    }

    fn snapshot(wx: u32, hy: u32) -> HiZCpuSnapshot {
        let mip_levels = 1u32;
        let n = total_float_count(wx, hy, mip_levels);
        HiZCpuSnapshot {
            base_width: wx,
            base_height: hy,
            mip_levels,
            mips: Arc::from(vec![0.0; n]),
        }
    }

    #[test]
    fn desktop_matches_when_mip0_matches_temporal_viewport() {
        let t = dummy_temporal((128, 96));
        let hi = HiZCullData::Desktop(snapshot(128, 96));
        assert!(hi_z_snapshot_matches_temporal(&hi, &t));
    }

    #[test]
    fn desktop_mismatches_when_pyramid_resolution_differs() {
        let t = dummy_temporal((128, 96));
        let hi = HiZCullData::Desktop(snapshot(64, 96));
        assert!(!hi_z_snapshot_matches_temporal(&hi, &t));
    }

    #[test]
    fn stereo_matches_left_eye_mip0_against_temporal_viewport() {
        let t = dummy_temporal((256, 144));
        let left = snapshot(256, 144);
        let right = snapshot(1, 1);
        let hi = HiZCullData::Stereo { left, right };
        assert!(hi_z_snapshot_matches_temporal(&hi, &t));
    }

    #[test]
    fn stereo_mismatches_when_left_eye_size_differs() {
        let t = dummy_temporal((256, 144));
        let left = snapshot(128, 144);
        let right = snapshot(256, 144);
        let hi = HiZCullData::Stereo { left, right };
        assert!(!hi_z_snapshot_matches_temporal(&hi, &t));
    }
}

#[cfg(test)]
mod overlay_cull_tests {
    use glam::{Mat4, Vec3, Vec4};

    use super::{CpuCullFailure, mesh_cpu_cull_with_geometry};
    use crate::camera::{CameraClipPlanes, HostCameraFrame, Viewport};
    use crate::scene::{RenderSpaceId, SceneCoordinator};
    use crate::world_mesh::culling::{
        MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams,
    };

    fn culling_with_overlay_proj(host_camera: &HostCameraFrame) -> WorldMeshCullInput<'_> {
        let overlay_proj = HostCameraFrame::overlay_projection(
            Viewport::from_tuple((100, 100)),
            CameraClipPlanes::new(3.0, 4.0),
        );
        WorldMeshCullInput {
            proj: WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj,
                vr_stereo: None,
            },
            host_camera,
            hi_z: None,
            hi_z_temporal: None,
            two_phase: None,
        }
    }

    #[test]
    fn overlay_draws_bypass_world_space_frustum_checks() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = WorldMeshCullInput {
            proj: WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
            host_camera: &host_camera,
            hi_z: None,
            hi_z_temporal: None,
            two_phase: None,
        };
        let model = Mat4::from_translation(Vec3::new(1234.0, 5678.0, 0.0));
        let geom = MeshCullGeometry {
            world_aabb: Some((Vec3::splat(10_000.0), Vec3::splat(10_001.0))),
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let Ok(accepted) = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            None,
            None,
        ) else {
            panic!("overlay items should skip frustum/Hi-Z rejection");
        };

        assert_eq!(accepted, Some(model));
    }

    #[test]
    fn overlay_rect_outside_viewport_is_culled() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = culling_with_overlay_proj(&host_camera);
        // Translate the rect 10 units to the right -- well outside the desktop overlay frustum.
        let model = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let rect = Vec4::new(0.0, 0.0, 0.5, 0.5);
        let geom = MeshCullGeometry {
            world_aabb: None,
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let res = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            Some(rect),
            None,
        );
        assert!(matches!(res, Err(CpuCullFailure::UiRectMask)));
    }

    #[test]
    fn overlay_rect_inside_viewport_passes() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = culling_with_overlay_proj(&host_camera);
        let model = Mat4::IDENTITY;
        let rect = Vec4::new(-0.5, -0.5, 0.5, 0.5);
        let geom = MeshCullGeometry {
            world_aabb: None,
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let res = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            Some(rect),
            None,
        );
        assert!(matches!(res, Ok(Some(m)) if m == model));
    }

    #[test]
    fn overlay_without_rect_clip_still_passes() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = culling_with_overlay_proj(&host_camera);
        // Same off-screen model as the culled case -- without a rect the overlay fast path must
        // still accept the draw, otherwise non-`_RectClip` overlay UI would regress.
        let model = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let geom = MeshCullGeometry {
            world_aabb: None,
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let res = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            None,
            None,
        );
        assert!(matches!(res, Ok(Some(m)) if m == model));
    }
}

#[cfg(test)]
mod prepared_space_cull_tests {
    use glam::{Mat4, Vec3};

    use super::{
        PreparedSpaceCull, SpaceCullCache, mesh_cpu_cull_with_geometry, world_aabb_visible_for_cull,
    };
    use crate::camera::{HostCameraFrame, reverse_z_perspective};
    use crate::camera::{overlay_camera_view_matrix, view_matrix_for_world_mesh_render_space};
    use crate::scene::{RenderSpaceId, SceneCoordinator};
    use crate::shared::RenderTransform;
    use crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip;
    use crate::world_mesh::culling::{
        MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams,
    };

    fn seeded_scene(space_id: RenderSpaceId) -> SceneCoordinator {
        let mut scene = SceneCoordinator::new();
        scene.test_seed_space_identity_worlds(space_id, vec![RenderTransform::default()], vec![-1]);
        scene
    }

    fn mono_culling(host_camera: &HostCameraFrame) -> WorldMeshCullInput<'_> {
        let proj = reverse_z_perspective(16.0 / 9.0, 60f32.to_radians(), 0.1, 100.0);
        WorldMeshCullInput {
            proj: WorldMeshCullProjParams {
                world_proj: proj,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
            host_camera,
            hi_z: None,
            hi_z_temporal: None,
            two_phase: None,
        }
    }

    #[test]
    fn prepared_frustum_matches_direct_view_projection() {
        let space_id = RenderSpaceId(3);
        let scene = seeded_scene(space_id);
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let space = scene.space(space_id).expect("seeded space");
        let view_proj =
            culling.proj.world_proj * view_matrix_for_world_mesh_render_space(&scene, space);
        let prepared = PreparedSpaceCull::frustum(&scene, space_id, false, &culling);

        let boxes = [
            (Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)),
            (Vec3::new(40.0, 0.0, 0.0), Vec3::new(41.0, 1.0, 1.0)),
            (Vec3::new(-2.0, -2.0, -8.0), Vec3::new(2.0, 2.0, -1.0)),
            (Vec3::splat(-100.0), Vec3::splat(-99.0)),
        ];
        for (min, max) in boxes {
            let direct = world_aabb_visible_in_homogeneous_clip(view_proj, min, max);
            assert_eq!(prepared.aabb_visible(min, max), direct, "{min:?} {max:?}");
            assert_eq!(
                world_aabb_visible_for_cull(&scene, space_id, false, &culling, min, max),
                direct,
                "{min:?} {max:?}"
            );
        }
    }

    #[test]
    fn missing_space_stays_visible_before_hi_z() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let min = Vec3::splat(10_000.0);
        let max = Vec3::splat(10_001.0);
        let prepared = PreparedSpaceCull::frustum(&scene, RenderSpaceId(404), false, &culling);

        assert!(prepared.aabb_visible(min, max));
        assert!(world_aabb_visible_for_cull(
            &scene,
            RenderSpaceId(404),
            false,
            &culling,
            min,
            max
        ));
    }

    #[test]
    fn space_cull_cache_matches_per_call_geometry_cull() {
        let space_id = RenderSpaceId(8);
        let scene = seeded_scene(space_id);
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let mut cache = SpaceCullCache::new();
        let boxes = [
            (Vec3::new(-0.25, -0.25, -0.25), Vec3::new(0.25, 0.25, 0.25)),
            (Vec3::new(30.0, 30.0, 30.0), Vec3::new(31.0, 31.0, 31.0)),
        ];
        for (min, max) in boxes {
            let geom = MeshCullGeometry {
                world_aabb: Some((min, max)),
                rigid_world_matrix: Some(Mat4::IDENTITY),
                front_face_world_matrix: Some(Mat4::IDENTITY),
            };
            let cached = cache.cull_geometry(geom, &scene, space_id, false, &culling, None);
            let direct =
                mesh_cpu_cull_with_geometry(geom, &scene, space_id, false, &culling, None, None);
            assert_eq!(cached, direct);
        }
    }

    #[test]
    fn prepared_overlay_frustum_matches_overlay_camera_view_projection() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let view_proj = culling.proj.overlay_proj * overlay_camera_view_matrix();
        let prepared = PreparedSpaceCull::frustum(&scene, RenderSpaceId(1), true, &culling);
        let min = Vec3::new(-0.2, -0.2, 0.0);
        let max = Vec3::new(0.2, 0.2, 0.0);

        assert_eq!(
            prepared.aabb_visible(min, max),
            world_aabb_visible_in_homogeneous_clip(view_proj, min, max)
        );
    }

    /// Compares one-shot frustum preparation with a reused plan.
    ///
    /// Run with `cargo test -p renderide --lib prepared_frustum_reuse_throughput -- --ignored --nocapture`.
    #[test]
    #[ignore = "prints a local throughput comparison; run with --ignored --nocapture"]
    fn prepared_frustum_reuse_throughput() {
        use std::time::Instant;

        let space_id = RenderSpaceId(3);
        let scene = seeded_scene(space_id);
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let boxes: Vec<(Vec3, Vec3)> = (0..20_000)
            .map(|index| {
                let x = (index as f32 * 0.37) - 100.0;
                (Vec3::new(x, -0.5, -2.0), Vec3::new(x + 0.8, 0.5, 2.0))
            })
            .collect();

        let once = Instant::now();
        let mut once_visible = 0usize;
        for &(min, max) in &boxes {
            if world_aabb_visible_for_cull(&scene, space_id, false, &culling, min, max) {
                once_visible += 1;
            }
        }
        let once_elapsed = once.elapsed();

        let reused_start = Instant::now();
        let prepared = PreparedSpaceCull::frustum(&scene, space_id, false, &culling);
        let mut reused_visible = 0usize;
        for &(min, max) in &boxes {
            if prepared.aabb_visible(min, max) {
                reused_visible += 1;
            }
        }
        let reused_elapsed = reused_start.elapsed();

        assert_eq!(once_visible, reused_visible);
        eprintln!(
            "frustum boxes={} once={once_elapsed:?} reused={reused_elapsed:?} visible={once_visible}",
            boxes.len()
        );
    }
}
