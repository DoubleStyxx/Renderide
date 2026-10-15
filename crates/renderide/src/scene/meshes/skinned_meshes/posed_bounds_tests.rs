//! [`apply_skinned_posed_bounds_extracted`] writes per-renderable posed bounds onto
//! [`SkinnedMeshRenderer::posed_object_bounds`] and honours the `renderable_index < 0`
//! terminator used by the host.

use glam::Vec3;

use crate::scene::meshes::types::SkinnedMeshRenderer;
use crate::scene::render_space::RenderSpaceState;
use crate::shared::{RenderBoundingBox, SkinnedMeshBoundsUpdate};

use super::{ExtractedSkinnedMeshRenderablesUpdate, apply_skinned_posed_bounds_extracted};

fn make_space_with(n: usize) -> RenderSpaceState {
    let mut space = RenderSpaceState::default();
    for _ in 0..n {
        space
            .skinned_mesh_renderers
            .push(SkinnedMeshRenderer::default());
    }
    space
}

fn bounds(cx: f32, hx: f32) -> RenderBoundingBox {
    RenderBoundingBox {
        center: Vec3::new(cx, 0.0, 0.0),
        extents: Vec3::new(hx, hx, hx),
    }
}

fn extracted_with_rows(
    rows: Vec<SkinnedMeshBoundsUpdate>,
) -> ExtractedSkinnedMeshRenderablesUpdate {
    ExtractedSkinnedMeshRenderablesUpdate {
        bounds_updates: rows,
        ..Default::default()
    }
}

#[test]
fn posed_bounds_are_stored_per_renderable() {
    let mut space = make_space_with(3);
    let extracted = extracted_with_rows(vec![
        SkinnedMeshBoundsUpdate {
            renderable_index: 0,
            local_bounds: bounds(1.0, 0.5),
        },
        SkinnedMeshBoundsUpdate {
            renderable_index: 2,
            local_bounds: bounds(2.0, 0.25),
        },
    ]);
    apply_skinned_posed_bounds_extracted(&mut space, &extracted);
    assert_eq!(
        space.skinned_mesh_renderers[0]
            .posed_object_bounds
            .unwrap()
            .center,
        Vec3::new(1.0, 0.0, 0.0)
    );
    assert!(
        space.skinned_mesh_renderers[1]
            .posed_object_bounds
            .is_none()
    );
    assert_eq!(
        space.skinned_mesh_renderers[2]
            .posed_object_bounds
            .unwrap()
            .extents,
        Vec3::new(0.25, 0.25, 0.25)
    );
}

#[test]
fn negative_renderable_index_terminates_rows() {
    let mut space = make_space_with(2);
    let extracted = extracted_with_rows(vec![
        SkinnedMeshBoundsUpdate {
            renderable_index: 0,
            local_bounds: bounds(1.0, 0.5),
        },
        SkinnedMeshBoundsUpdate {
            renderable_index: -1,
            local_bounds: bounds(99.0, 99.0),
        },
        SkinnedMeshBoundsUpdate {
            renderable_index: 1,
            local_bounds: bounds(2.0, 0.5),
        },
    ]);
    apply_skinned_posed_bounds_extracted(&mut space, &extracted);
    assert!(
        space.skinned_mesh_renderers[0]
            .posed_object_bounds
            .is_some()
    );
    assert!(
        space.skinned_mesh_renderers[1]
            .posed_object_bounds
            .is_none()
    );
}

#[test]
fn out_of_range_index_is_ignored() {
    let mut space = make_space_with(1);
    let extracted = extracted_with_rows(vec![SkinnedMeshBoundsUpdate {
        renderable_index: 99,
        local_bounds: bounds(1.0, 0.5),
    }]);
    apply_skinned_posed_bounds_extracted(&mut space, &extracted);
    assert!(
        space.skinned_mesh_renderers[0]
            .posed_object_bounds
            .is_none()
    );
}
