//! CPU frustum and Hi-Z culling helpers for [`crate::world_mesh::draw_prep::queue_draws_with_parallelism`].
//!
//! Shares one bounds evaluation per draw slot using the same view-projection rules as the forward pass
//! ([`super::build_world_mesh_cull_proj_params`]), including
//! [`crate::camera::HostCameraFrame::explicit_world_to_view`] when an explicit camera view is
//! present (e.g. for secondary render-texture cameras).

use glam::{Mat4, Vec3, Vec4};

use crate::scene::{RenderSpaceId, SceneSpaceRead};

use super::frustum::world_aabb_visible_in_homogeneous_clip;
use super::geometry::MeshCullGeometry;
use super::{HiZTemporalState, WorldMeshCullInput};
use crate::camera::{overlay_camera_view_matrix, view_matrix_for_world_mesh_render_space};
use crate::hi_z_cpu::snapshot::HiZCpuSnapshot;
use crate::hi_z_cpu::{
    AabbHiZSample, HiZCullData, hi_z_view_proj_matrices, mesh_fully_occluded_in_hiz,
    sample_aabb_occlusion_with_pad, stereo_hiz_keeps_draw,
};
use crate::occlusion::hysteresis::{
    CasterRef, DepthClip, OcclusionFacts, ScreenRect, SpaceMotion,
    covering_depth_is_other_occluder, parallax_adjusted_pad, space_motion_pad,
};

/// Clip matrices for one render space, built once and reused across every AABB in that space.
///
/// The one-shot frustum helper used to invert the space view transform and multiply the
/// projection on every box. Spatial queries and per-renderer fine culls test thousands of boxes
/// against one view, so the matrix work is hoisted here. The accept/reject result matches the
/// per-call path, including "missing space stays visible" and stereo OR.
#[derive(Clone, Copy)]
pub(crate) struct PreparedSpaceCull<'a> {
    /// Frustum half-spaces for this space.
    frustum: SpaceFrustum,
    /// Previous-frame Hi-Z test, when the temporal snapshot applies to this space.
    hi_z: Option<PreparedSpaceHiZ<'a>>,
    /// History confirmation for this view. Absent on broadphase, overlays, and over-budget views.
    hysteresis: Option<&'a crate::occlusion::hysteresis::VisibilityHysteresis>,
    /// Camera pair for this space, resolved once when the hysteresis plan is attached.
    motion: Option<&'a SpaceMotion>,
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
            frustum: space_frustum(scene, space_id, is_overlay, culling),
            hi_z: None,
            hysteresis: None,
            motion: None,
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
            frustum: space_frustum(scene, space_id, is_overlay, culling),
            hi_z: if is_overlay {
                None
            } else {
                prepare_space_hi_z(space_id, culling)
            },
            hysteresis: if is_overlay { None } else { culling.hysteresis },
            motion: if is_overlay {
                None
            } else {
                culling
                    .hysteresis
                    .and_then(|frame| frame.space_motion(space_id))
            },
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

    /// Hi-Z coverage used by the hysteresis confirmation. Missing pyramids do not occlude.
    ///
    /// The query rectangle is the caster's footprint expanded by the screen motion since the
    /// pyramid was written. A closer covering surface widens that pad again. No motion bound
    /// means the previous pyramid is not confirmation, so the caster is kept.
    fn occlusion_facts(&self, wmin: Vec3, wmax: Vec3) -> OcclusionFacts {
        if self.hysteresis.is_none() {
            return OcclusionFacts::keep();
        }
        let Some(hi_z) = self.hi_z else {
            return OcclusionFacts::keep();
        };
        let Some(motion) = self.motion else {
            return OcclusionFacts::keep();
        };
        let Some(pad) = space_motion_pad(motion, wmin, wmax) else {
            return OcclusionFacts::keep();
        };
        let clip = motion.clip;
        match hi_z {
            PreparedSpaceHiZ::Desktop {
                snapshot,
                view_proj,
            } => facts_from_mono(
                confirm_sample(snapshot, view_proj, wmin, wmax, pad, clip),
                clip,
            ),
            PreparedSpaceHiZ::Stereo {
                left,
                right,
                left_vp,
                right_vp,
            } => facts_from_stereo(
                confirm_sample(left, left_vp, wmin, wmax, pad, clip),
                confirm_sample(right, right_vp, wmin, wmax, pad, clip),
                clip,
            ),
        }
    }
}

fn confirm_sample(
    snapshot: &HiZCpuSnapshot,
    view_proj: Mat4,
    min: Vec3,
    max: Vec3,
    base_pad: f32,
    clip: DepthClip,
) -> Option<AabbHiZSample> {
    let first = sample_aabb_occlusion_with_pad(snapshot, view_proj, min, max, base_pad)?;
    let adjusted = parallax_adjusted_pad(
        base_pad,
        first.closest_ndc_z,
        first.covering_depth,
        first.occluded,
        clip,
    )?;
    if (adjusted - base_pad).abs() <= f32::EPSILON {
        return Some(first);
    }
    sample_aabb_occlusion_with_pad(snapshot, view_proj, min, max, adjusted)
}

fn facts_from_mono(sample: Option<AabbHiZSample>, clip: DepthClip) -> OcclusionFacts {
    OcclusionFacts {
        occluded: sample.is_some_and(|sample| sample.occluded),
        other_occluder: sample.is_some_and(|sample| sample_is_other_occluder(sample, clip)),
        rects: [sample.and_then(sample_rect), None],
    }
}

fn facts_from_stereo(
    left: Option<AabbHiZSample>,
    right: Option<AabbHiZSample>,
    clip: DepthClip,
) -> OcclusionFacts {
    OcclusionFacts {
        occluded: left.is_some_and(|sample| sample.occluded)
            && right.is_some_and(|sample| sample.occluded),
        other_occluder: left.is_some_and(|sample| sample_is_other_occluder(sample, clip))
            && right.is_some_and(|sample| sample_is_other_occluder(sample, clip)),
        rects: [left.and_then(sample_rect), right.and_then(sample_rect)],
    }
}

fn sample_is_other_occluder(sample: AabbHiZSample, clip: DepthClip) -> bool {
    sample.occluded
        && sample.covering_depth.is_some_and(|depth| {
            covering_depth_is_other_occluder(sample.closest_ndc_z, depth, clip)
        })
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

    /// Runs frustum and history-based occlusion culling for `geom`.
    ///
    /// `caster` is recorded in the view's visibility history when a hysteresis plan is active.
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
        caster: Option<CasterRef>,
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

/// One-shot frustum and Hi-Z test for a [`MeshCullGeometry`] the caller already built.
///
/// Builds a fresh [`PreparedSpaceCull`] for this call. Prepared collection and the scene-walk
/// chunk pass one [`SpaceCullCache`] through the chunk, so each `(space, overlay)` pair is
/// prepared once. Tests use this helper for the one-shot result, including the comparison with
/// that cache.
#[cfg(test)]
pub(crate) fn mesh_cpu_cull_with_geometry<S>(
    geom: MeshCullGeometry,
    scene: &S,
    space_id: RenderSpaceId,
    is_overlay: bool,
    culling: &WorldMeshCullInput<'_>,
    ui_rect_clip_local: Option<Vec4>,
    caster: Option<CasterRef>,
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
    caster: Option<CasterRef>,
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
    caster: Option<CasterRef>,
    wmin: Vec3,
    wmax: Vec3,
) -> bool {
    let Some(frame) = prepared.hysteresis else {
        return !prepared.hi_z_occludes(wmin, wmax);
    };
    frame.allows_draw(caster, wmin, wmax, prepared.occlusion_facts(wmin, wmax))
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
include!("eval_tests.rs");
