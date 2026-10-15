//! Coverage for [`apply_skinned_blendshape_weight_batches_extracted`] across the serial path,
//! the parallel path, and partial-failure batch-stream handling.

use crate::scene::meshes::types::SkinnedMeshRenderer;
use crate::scene::render_space::RenderSpaceState;
use crate::shared::{BlendshapeUpdate, BlendshapeUpdateBatch};

use super::{
    BLENDSHAPE_APPLY_PARALLEL_MIN, BLENDSHAPE_APPLY_PARALLEL_MIN_RENDERERS,
    ExtractedSkinnedMeshRenderablesUpdate, apply_skinned_blendshape_weight_batches_extracted,
    should_parallelize_blendshape_apply,
};

fn space_with_n_renderers(n: usize) -> RenderSpaceState {
    let mut space = RenderSpaceState::default();
    for _ in 0..n {
        space
            .skinned_mesh_renderers
            .push(SkinnedMeshRenderer::default());
    }
    space
}

fn one_update_per_renderer(n: usize) -> (Vec<BlendshapeUpdateBatch>, Vec<BlendshapeUpdate>) {
    let mut batches = Vec::with_capacity(n + 1);
    let mut updates = Vec::with_capacity(n);
    for i in 0..n {
        batches.push(BlendshapeUpdateBatch {
            renderable_index: i as i32,
            blendshape_update_count: 1,
        });
        updates.push(BlendshapeUpdate {
            blendshape_index: 0,
            weight: i as f32 * 0.25,
        });
    }
    batches.push(BlendshapeUpdateBatch {
        renderable_index: -1,
        blendshape_update_count: 0,
    });
    (batches, updates)
}

#[test]
fn parallel_gate_requires_enough_batches_and_renderers() {
    assert!(!should_parallelize_blendshape_apply(
        BLENDSHAPE_APPLY_PARALLEL_MIN - 1,
        BLENDSHAPE_APPLY_PARALLEL_MIN_RENDERERS
    ));
    assert!(!should_parallelize_blendshape_apply(
        BLENDSHAPE_APPLY_PARALLEL_MIN,
        BLENDSHAPE_APPLY_PARALLEL_MIN_RENDERERS - 1
    ));
    assert!(should_parallelize_blendshape_apply(
        BLENDSHAPE_APPLY_PARALLEL_MIN,
        BLENDSHAPE_APPLY_PARALLEL_MIN_RENDERERS
    ));
}

#[test]
fn serial_path_writes_one_update_per_renderer() {
    let n = 4;
    let mut space = space_with_n_renderers(n);
    let (batches, updates) = one_update_per_renderer(n);
    let extracted = ExtractedSkinnedMeshRenderablesUpdate {
        blendshape_update_batches: batches,
        blendshape_updates: updates,
        ..Default::default()
    };
    apply_skinned_blendshape_weight_batches_extracted(&mut space, &extracted);
    for i in 0..n {
        assert_eq!(
            space.skinned_mesh_renderers[i].base.blend_shape_weights,
            vec![i as f32 * 0.25]
        );
    }
}

#[test]
fn parallel_path_matches_serial_for_large_touched_count() {
    let n = BLENDSHAPE_APPLY_PARALLEL_MIN + 13;
    let (batches, updates) = one_update_per_renderer(n);

    let mut serial_space = space_with_n_renderers(n);
    // The parallel path triggers because `n` is above the threshold; we compare its output
    // to the expected per-renderer weight that the same batch stream describes.
    let extracted = ExtractedSkinnedMeshRenderablesUpdate {
        blendshape_update_batches: batches,
        blendshape_updates: updates,
        ..Default::default()
    };
    apply_skinned_blendshape_weight_batches_extracted(&mut serial_space, &extracted);

    for i in 0..n {
        assert_eq!(
            serial_space.skinned_mesh_renderers[i]
                .base
                .blend_shape_weights,
            vec![i as f32 * 0.25]
        );
    }
}

#[test]
fn out_of_range_batch_is_skipped_and_does_not_consume_updates_for_following_batches() {
    let mut space = space_with_n_renderers(2);
    let batches = vec![
        BlendshapeUpdateBatch {
            renderable_index: 99,
            blendshape_update_count: 1,
        },
        BlendshapeUpdateBatch {
            renderable_index: 1,
            blendshape_update_count: 1,
        },
        BlendshapeUpdateBatch {
            renderable_index: -1,
            blendshape_update_count: 0,
        },
    ];
    let updates = vec![
        BlendshapeUpdate {
            blendshape_index: 0,
            weight: 100.0,
        },
        BlendshapeUpdate {
            blendshape_index: 0,
            weight: 0.5,
        },
    ];
    let extracted = ExtractedSkinnedMeshRenderablesUpdate {
        blendshape_update_batches: batches,
        blendshape_updates: updates,
        ..Default::default()
    };
    apply_skinned_blendshape_weight_batches_extracted(&mut space, &extracted);
    assert_eq!(
        space.skinned_mesh_renderers[1].base.blend_shape_weights,
        vec![0.5]
    );
    assert!(
        space.skinned_mesh_renderers[0]
            .base
            .blend_shape_weights
            .is_empty()
    );
}

#[test]
fn out_of_bounds_blendshape_index_is_dropped() {
    let mut space = space_with_n_renderers(1);
    let extracted = ExtractedSkinnedMeshRenderablesUpdate {
        blendshape_update_batches: vec![
            BlendshapeUpdateBatch {
                renderable_index: 0,
                blendshape_update_count: 2,
            },
            BlendshapeUpdateBatch {
                renderable_index: -1,
                blendshape_update_count: 0,
            },
        ],
        blendshape_updates: vec![
            BlendshapeUpdate {
                blendshape_index: i32::MAX,
                weight: 99.0,
            },
            BlendshapeUpdate {
                blendshape_index: 3,
                weight: 0.75,
            },
        ],
        ..Default::default()
    };
    apply_skinned_blendshape_weight_batches_extracted(&mut space, &extracted);
    assert_eq!(
        space.skinned_mesh_renderers[0].base.blend_shape_weights,
        vec![0.0, 0.0, 0.0, 0.75]
    );
}
