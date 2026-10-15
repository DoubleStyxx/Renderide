//! Regression coverage for skinned-renderer instance identity across dense table reindexing.

use crate::scene::meshes::types::MeshRendererInstanceId;
use crate::scene::render_space::RenderSpaceState;

use super::{
    ExtractedSkinnedMeshRenderablesUpdate, apply_skinned_removals_and_additions_extracted,
};

#[test]
fn skinned_instance_ids_are_fresh_and_survive_swap_remove() {
    let mut space = RenderSpaceState::default();
    apply_skinned_removals_and_additions_extracted(
        &mut space,
        &ExtractedSkinnedMeshRenderablesUpdate {
            additions: vec![20, 21, 22, -1],
            ..Default::default()
        },
    );
    let ids: Vec<_> = space
        .skinned_mesh_renderers
        .iter()
        .map(|renderer| renderer.base.instance_id)
        .collect();
    assert_eq!(
        ids,
        vec![
            MeshRendererInstanceId(1),
            MeshRendererInstanceId(2),
            MeshRendererInstanceId(3),
        ]
    );

    apply_skinned_removals_and_additions_extracted(
        &mut space,
        &ExtractedSkinnedMeshRenderablesUpdate {
            removals: vec![1, -1],
            additions: vec![23, -1],
            ..Default::default()
        },
    );
    let ids: Vec<_> = space
        .skinned_mesh_renderers
        .iter()
        .map(|renderer| renderer.base.instance_id)
        .collect();
    assert_eq!(
        ids,
        vec![
            MeshRendererInstanceId(1),
            MeshRendererInstanceId(3),
            MeshRendererInstanceId(4),
        ]
    );
}
