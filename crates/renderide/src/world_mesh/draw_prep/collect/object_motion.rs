//! Rigid object-motion selection for draw collection.
//!
//! Skinned meshes, particles, overlays, and render-context matrices that do not match the cached
//! hierarchy matrix stay on camera reprojection. [`crate::shared::MotionVectorMode::Object`] is
//! the only mode that receives a previous matrix.

use glam::Mat4;

use crate::render_contract::ParticleDrawKind;
use crate::scene::{RenderSpaceId, SceneCoordinator, SceneMeshRendererRead};
use crate::shared::MotionVectorMode;

/// Inputs needed to decide whether one draw carries a previous rigid matrix.
pub(super) struct RigidMotionQuery {
    /// Render space that owns the renderer.
    pub(super) space_id: RenderSpaceId,
    /// Dense transform index. Negative ids have no hierarchy matrix.
    pub(super) node_id: i32,
    /// Static renderer index. Skinned draws never qualify.
    pub(super) renderable_index: usize,
    /// Skinned draws are a separate follow-up and stay on camera velocity.
    pub(super) skinned: bool,
    /// Overlay draws use a re-rooted matrix that is not the cached hierarchy pose.
    pub(super) is_overlay: bool,
    /// World-space deformed streams do not use the rigid model matrix.
    pub(super) world_space_deformed: bool,
    /// Particle family. Ordinary mesh draws use [`ParticleDrawKind::None`].
    pub(super) particle_kind: ParticleDrawKind,
    /// Current rigid matrix that the forward pass will upload.
    pub(super) current: Option<Mat4>,
    /// False for views whose velocity target is not sampled by motion blur.
    pub(super) consumes_motion_vectors: bool,
}

/// Returns the previous rigid matrix when this draw should overwrite camera velocity.
pub(super) fn previous_for_draw(scene: &SceneCoordinator, query: RigidMotionQuery) -> Option<Mat4> {
    if !query.consumes_motion_vectors
        || !scene.rigid_motion_history_enabled()
        || !query.shape_eligible()
    {
        return None;
    }
    let current = query.current?;
    if query.node_id < 0 {
        return None;
    }
    let index = query.node_id as usize;
    let cached = scene.world_matrix(query.space_id, index)?;
    if cached != current {
        return None;
    }
    let renderer = scene
        .static_mesh_renderers(query.space_id)?
        .get(query.renderable_index)?;
    if renderer.motion_vector_mode != MotionVectorMode::Object {
        return None;
    }
    let previous = scene.previous_world_matrix(query.space_id, index)?;
    select_rigid_object_previous(true, true, true, Some(previous), current)
}

impl RigidMotionQuery {
    fn shape_eligible(&self) -> bool {
        !self.skinned
            && !self.is_overlay
            && !self.world_space_deformed
            && self.particle_kind == ParticleDrawKind::None
    }
}

/// Selects a previous matrix for a rigid object-motion draw.
///
/// Equal matrices produce no object velocity, so the caller leaves the camera vector in place.
pub(crate) fn select_rigid_object_previous(
    mode_is_object: bool,
    shape_eligible: bool,
    current_matches_cached: bool,
    previous: Option<Mat4>,
    current: Mat4,
) -> Option<Mat4> {
    if !mode_is_object || !shape_eligible || !current_matches_cached {
        return None;
    }
    let previous = previous?;
    if previous == current {
        None
    } else {
        Some(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_mode_keeps_a_different_previous_matrix() {
        let current = Mat4::from_translation(glam::Vec3::X);
        let previous = Mat4::IDENTITY;
        assert_eq!(
            select_rigid_object_previous(true, true, true, Some(previous), current),
            Some(previous)
        );
    }

    #[test]
    fn identical_matrices_and_ineligible_draws_have_no_object_velocity() {
        let current = Mat4::IDENTITY;
        assert_eq!(
            select_rigid_object_previous(true, true, true, Some(current), current),
            None
        );
        assert_eq!(
            select_rigid_object_previous(
                false,
                true,
                true,
                Some(Mat4::from_scale(glam::Vec3::splat(2.0))),
                current
            ),
            None
        );
        assert_eq!(
            select_rigid_object_previous(
                true,
                false,
                true,
                Some(Mat4::from_scale(glam::Vec3::splat(2.0))),
                current
            ),
            None
        );
        assert_eq!(
            select_rigid_object_previous(
                true,
                true,
                false,
                Some(Mat4::from_scale(glam::Vec3::splat(2.0))),
                current
            ),
            None
        );
        assert_eq!(
            select_rigid_object_previous(true, true, true, None, current),
            None
        );
    }
}
