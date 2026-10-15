//! Render-space query ordering tests.

use super::*;

/// Render-space iteration is stable so draw collection and transparent fallback ordering do not
/// depend on hash seed or host insertion order.
#[test]
fn render_space_ids_are_sorted_by_host_id() {
    let mut scene = SceneCoordinator::new();
    for id in [RenderSpaceId(42), RenderSpaceId(-2), RenderSpaceId(7)] {
        scene.spaces.insert(
            id,
            RenderSpaceState {
                id,
                is_active: true,
                ..Default::default()
            },
        );
    }

    let ids: Vec<RenderSpaceId> = scene.render_space_ids().collect();
    assert_eq!(
        ids,
        vec![RenderSpaceId(-2), RenderSpaceId(7), RenderSpaceId(42)]
    );

    scene.spaces.remove(&RenderSpaceId(7));
    scene.spaces.insert(
        RenderSpaceId(3),
        RenderSpaceState {
            id: RenderSpaceId(3),
            is_active: true,
            ..Default::default()
        },
    );

    let ids_after_reinsert: Vec<RenderSpaceId> = scene.render_space_ids().collect();
    assert_eq!(
        ids_after_reinsert,
        vec![RenderSpaceId(-2), RenderSpaceId(3), RenderSpaceId(42)]
    );
}
