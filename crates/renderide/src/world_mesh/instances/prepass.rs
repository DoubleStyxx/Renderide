//! Depth and normal prepass eligibility for world-mesh instance groups.

use crate::materials::{
    RasterPipelineKind, ShaderPermutation, UNITY_RENDER_QUEUE_ALPHA_TEST,
    embedded_stem_depth_prepass_pass,
};

use super::{DrawGroup, WorldMeshPhase};
use crate::world_mesh::draw_prep::WorldMeshDrawItem;

/// Returns whether a regular draw group may be mirrored by the generic opaque depth prepass.
pub(crate) fn depth_prepass_group_eligible(
    draws: &[WorldMeshDrawItem],
    slab_layout: &[usize],
    group: &DrawGroup,
    shader_perm: ShaderPermutation,
) -> bool {
    let start = group.instance_range.start as usize;
    let end = group.instance_range.end as usize;
    slab_layout.get(start..end).is_some_and(|members| {
        !members.is_empty()
            && members.iter().all(|&draw_idx| {
                draws
                    .get(draw_idx)
                    .is_some_and(|item| depth_prepass_item_eligible(item, shader_perm))
            })
    })
}

/// Returns whether a draw may be submitted through the conservative generic depth prepass.
pub(crate) fn depth_prepass_item_eligible(
    item: &WorldMeshDrawItem,
    shader_perm: ShaderPermutation,
) -> bool {
    let key = &item.batch_key;
    !item.is_overlay
        && key.render_queue < UNITY_RENDER_QUEUE_ALPHA_TEST
        && !key.alpha_blended
        && !key.blend_mode.is_transparent()
        && !key.embedded_requires_intersection_pass
        && !key.embedded_uses_scene_depth_snapshot
        && !key.embedded_uses_scene_color_snapshot
        && key.render_state.depth_write != Some(false)
        && key.render_state.depth_compare.is_none()
        && key.render_state.depth_offset.is_none()
        && !key.render_state.stencil.enabled
        && match &key.pipeline {
            RasterPipelineKind::Null => true,
            RasterPipelineKind::EmbeddedStem(stem) => {
                embedded_stem_depth_prepass_pass(stem.as_ref(), shader_perm).is_some()
            }
        }
}

pub(super) fn phase_is_pre_skybox_forward(phase: WorldMeshPhase) -> bool {
    matches!(
        phase,
        WorldMeshPhase::ForwardOpaque | WorldMeshPhase::ForwardAlphaTest
    )
}
