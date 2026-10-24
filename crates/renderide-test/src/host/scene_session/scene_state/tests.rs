
use super::*;

#[test]
fn scene_submission_single_mesh_matches_historical_region_sizes() {
    let scene = SceneSubmission::single_mesh(2, 4);
    let regions = SceneSharedMemoryRegions::build(&scene);
    assert_eq!(regions.pose_updates_bytes.len(), 44);
    assert_eq!(regions.mesh_additions_bytes.len(), 8);
    assert_eq!(regions.mesh_states_bytes.len(), 48);
    assert_eq!(regions.packed_material_ids_bytes.len(), 4);
    assert_eq!(regions.light_additions_bytes.len(), 4);
    assert_eq!(regions.light_states_bytes.len(), LIGHT_STATE_HOST_ROW_BYTES);
}

#[test]
fn multi_renderable_scene_uses_transform_ids_and_material_order() {
    let scene = SceneSubmission {
        camera_world_pose: default_camera_world_pose(),
        renderables: vec![
            SceneRenderable {
                transform_id: 2,
                pose: identity_transform(),
                mesh_asset_id: 10,
                material_asset_id: 20,
                sorting_order: 0,
                shadow_cast_mode: ShadowCastMode::Off,
            },
            SceneRenderable {
                transform_id: 4,
                pose: identity_transform(),
                mesh_asset_id: 11,
                material_asset_id: 21,
                sorting_order: 3,
                shadow_cast_mode: ShadowCastMode::Off,
            },
        ],
        lights: Vec::new(),
        skinned: Vec::new(),
        cameras: Vec::new(),
    };
    let regions = SceneSharedMemoryRegions::build(&scene);
    assert_eq!(regions.mesh_additions_bytes.len(), 12);
    assert_eq!(regions.mesh_states_bytes.len(), 72);
    assert_eq!(regions.packed_material_ids_bytes.len(), 8);
    assert_eq!(target_transform_count(&scene, &[]), 5);
}

#[test]
fn skinned_regions_are_sized_and_terminated() {
    let avatar = |transform_id| SceneSkinned {
        transform_id,
        pose: identity_transform(),
        mesh_asset_id: 3,
        material_asset_id: 4,
        shadow_cast_mode: ShadowCastMode::On,
        posed_bounds: RenderBoundingBox::default(),
    };
    let scene = SceneSubmission {
        camera_world_pose: default_camera_world_pose(),
        renderables: Vec::new(),
        lights: Vec::new(),
        skinned: vec![avatar(7), avatar(9)],
        cameras: Vec::new(),
    };
    let regions = SceneSharedMemoryRegions::build(&scene);
    assert_eq!(regions.skinned_additions_bytes.len(), 12);
    assert_eq!(regions.skinned_states_bytes.len(), 72);
    assert_eq!(regions.skinned_material_ids_bytes.len(), 8);
    assert_eq!(regions.bone_assignments_bytes.len(), 24);
    assert_eq!(regions.bone_indexes_bytes.len(), 8);
    assert_eq!(
        regions.skinned_bounds_bytes.len(),
        3 * SKINNED_MESH_BOUNDS_UPDATE_HOST_ROW_BYTES
    );
    assert_eq!(target_transform_count(&scene, &[]), 10);
}

#[test]
fn light_state_rows_are_indexed_and_terminated() {
    let scene = SceneSubmission {
        camera_world_pose: default_camera_world_pose(),
        renderables: Vec::new(),
        lights: vec![
            SceneLight {
                transform_id: 0,
                pose: identity_transform(),
                state: point_light([1.0, 0.5, 0.25], 2.0, 3.0),
            },
            SceneLight {
                transform_id: 1,
                pose: identity_transform(),
                state: directional_light([0.5, 0.5, 1.0], 1.0),
            },
        ],
        skinned: Vec::new(),
        cameras: Vec::new(),
    };
    let bytes = encode_light_states(&light_state_rows(&scene));
    assert_eq!(bytes.len(), 3 * LIGHT_STATE_HOST_ROW_BYTES);
    assert_eq!(i32::from_le_bytes(bytes[0..4].try_into().unwrap()), 0);
    assert_eq!(
        i32::from_le_bytes(
            bytes[LIGHT_STATE_HOST_ROW_BYTES..LIGHT_STATE_HOST_ROW_BYTES + 4]
                .try_into()
                .unwrap()
        ),
        1
    );
    assert_eq!(
        i32::from_le_bytes(
            bytes[2 * LIGHT_STATE_HOST_ROW_BYTES..2 * LIGHT_STATE_HOST_ROW_BYTES + 4]
                .try_into()
                .unwrap()
        ),
        -1
    );
}
