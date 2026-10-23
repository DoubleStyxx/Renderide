//! Prepared-path draw counts for a scene of mostly off-screen skinned meshes.
//!
//! The production collector stores one bone-bounds AABB on the renderer row and the spatial
//! index rejects that box once for every material slot. This golden records that drop. An
//! on-screen extreme pose (bounds pulled in from a rest box that would be off-screen) stays.

use std::fs;
use std::path::Path;

use glam::{Mat4, Vec3};

use super::spatial::PreparedSpatialIndex;
use super::{FramePreparedDraw, FramePreparedRun};
use crate::camera::HostCameraFrame;
use crate::particles::ParticleDrawParams;
use crate::scene::{MeshRendererInstanceId, RenderSpaceId, SceneCoordinator};
use crate::shared::{RenderTransform, ShadowCastMode};
use crate::world_mesh::culling::{MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams};

const SPACE: RenderSpaceId = RenderSpaceId(166);
const OFFSCREEN_RENDERERS: usize = 64;
const SLOTS: usize = 2;

fn golden() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../renderide-test/goldens/skinned_offscreen_cull.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn skinned_draw(index: usize, slot: usize, min: Vec3, max: Vec3) -> FramePreparedDraw {
    FramePreparedDraw {
        space_id: SPACE,
        renderable_index: index,
        instance_id: MeshRendererInstanceId(u64::try_from(index).unwrap_or(0).saturating_add(1)),
        renderer_ordinal: index,
        node_id: 0,
        mesh_asset_id: 10,
        is_overlay: false,
        is_hidden: false,
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::On,
        skinned: true,
        world_space_deformed: true,
        blendshape_deformed: false,
        tangent_blendshape_deform_active: false,
        slot_index: slot,
        material_stack_order: None,
        first_index: 0,
        index_count: 3,
        material_asset_id: 1 + i32::try_from(slot).unwrap_or(0),
        property_block_id: None,
        cull_geometry: Some(MeshCullGeometry {
            world_aabb: Some((min, max)),
            rigid_world_matrix: None,
            front_face_world_matrix: Some(Mat4::IDENTITY),
        }),
        rigid_world_matrix_override: None,
        particle_draw: ParticleDrawParams::default(),
    }
}

#[test]
fn offscreen_skinned_slots_drop_and_extreme_pose_stays() {
    let contract = golden();
    let mut scene = SceneCoordinator::new();
    scene.test_seed_space_identity_worlds(SPACE, vec![RenderTransform::default()], vec![-1]);
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
    };

    let onscreen_min = Vec3::new(-0.25, -0.25, -0.25);
    let onscreen_max = Vec3::new(0.25, 0.25, 0.25);
    let offscreen_min = Vec3::new(40.0, 0.0, 0.0);
    let offscreen_max = Vec3::new(41.0, 1.0, 1.0);
    let renderer_count = OFFSCREEN_RENDERERS + 1;
    let mut draws = Vec::with_capacity(renderer_count * SLOTS);
    let mut runs = Vec::with_capacity(renderer_count);
    for index in 0..renderer_count {
        let start = draws.len() as u32;
        let (min, max) = if index == 0 {
            (onscreen_min, onscreen_max)
        } else {
            (offscreen_min, offscreen_max)
        };
        for slot in 0..SLOTS {
            draws.push(skinned_draw(index, slot, min, max));
        }
        runs.push(FramePreparedRun {
            start,
            end: draws.len() as u32,
        });
    }

    let mut spatial = PreparedSpatialIndex::default();
    spatial.rebuild(&draws, &runs);
    assert!(spatial.space_uses_bvh_for_tests(SPACE));
    let candidates = spatial.query_runs(&runs, &[SPACE], &scene, Some(&culling));
    let candidate_slots: usize = candidates
        .runs
        .iter()
        .map(|run| (run.end - run.start) as usize)
        .sum();

    assert_eq!(candidates.runs.len(), 1);
    assert_eq!(candidates.runs[0], runs[0]);
    assert!(draws[candidates.runs[0].start as usize].skinned);
    assert_eq!(
        draws[candidates.runs[0].start as usize]
            .cull_geometry
            .and_then(|g| g.rigid_world_matrix),
        None
    );
    assert_eq!(
        candidates.cull_stats,
        (OFFSCREEN_RENDERERS * SLOTS, OFFSCREEN_RENDERERS * SLOTS, 0)
    );
    assert_eq!(candidate_slots, SLOTS);

    assert_eq!(
        contract["offscreen_skinned_renderers"].as_u64(),
        Some(OFFSCREEN_RENDERERS as u64)
    );
    assert_eq!(
        contract["onscreen_extreme_pose_renderers"].as_u64(),
        Some(1)
    );
    assert_eq!(
        contract["material_slots_per_renderer"].as_u64(),
        Some(SLOTS as u64)
    );
    assert_eq!(
        contract["slots_before_cull"].as_u64(),
        Some((renderer_count * SLOTS) as u64)
    );
    assert_eq!(
        contract["broadphase_candidate_runs"].as_u64(),
        Some(candidates.runs.len() as u64)
    );
    assert_eq!(
        contract["broadphase_candidate_slots"].as_u64(),
        Some(candidate_slots as u64)
    );
    assert_eq!(
        contract["frustum_rejected_slots"].as_u64(),
        Some(candidates.cull_stats.1 as u64)
    );
}
