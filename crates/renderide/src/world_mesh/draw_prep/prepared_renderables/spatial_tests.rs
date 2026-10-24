#[cfg(test)]
mod tests {
    use super::*;

    use glam::Mat4;

    use crate::camera::HostCameraFrame;
    use crate::scene::{MeshRendererInstanceId, RenderSpaceId, SceneCoordinator};
    use crate::shared::RenderTransform;
    use crate::world_mesh::culling::{
        MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams,
    };

    fn prepared_draw_with_bounds(
        space_id: RenderSpaceId,
        renderable_index: usize,
        min: Vec3,
        max: Vec3,
    ) -> FramePreparedDraw {
        FramePreparedDraw {
            space_id,
            renderable_index,
            instance_id: MeshRendererInstanceId(renderable_index as u64 + 1),
            renderer_ordinal: renderable_index,
            node_id: renderable_index as i32,
            mesh_asset_id: 10,
            is_overlay: false,
            is_hidden: false,
            sorting_order: 0,
            shadow_cast_mode: ShadowCastMode::On,
            skinned: false,
            world_space_deformed: false,
            blendshape_deformed: false,
            tangent_blendshape_deform_active: false,
            slot_index: 0,
            material_stack_order: None,
            first_index: 0,
            index_count: 3,
            material_asset_id: 1,
            property_block_id: None,
            cull_geometry: Some(MeshCullGeometry {
                world_aabb: Some((min, max)),
                rigid_world_matrix: Some(Mat4::IDENTITY),
                front_face_world_matrix: Some(Mat4::IDENTITY),
            }),
            rigid_world_matrix_override: None,
            particle_draw: ParticleDrawParams::default(),
        }
    }

    fn set_world_aabb(draw: &mut FramePreparedDraw, aabb: Option<(Vec3, Vec3)>) {
        draw.cull_geometry = Some(MeshCullGeometry {
            world_aabb: aabb,
            rigid_world_matrix: Some(Mat4::IDENTITY),
            front_face_world_matrix: Some(Mat4::IDENTITY),
        });
    }

    fn one_draw_runs(draw_count: usize) -> Vec<FramePreparedRun> {
        (0..draw_count)
            .map(|idx| FramePreparedRun {
                start: idx as u32,
                end: idx as u32 + 1,
            })
            .collect()
    }

    fn caster_indices(spatial: &PreparedSpatialIndex) -> Vec<usize> {
        spatial
            .world_casters()
            .map(|bounds| bounds.renderable_index)
            .collect()
    }

    fn spatial_scene_and_cull(
        space_id: RenderSpaceId,
    ) -> (SceneCoordinator, HostCameraFrame, WorldMeshCullProjParams) {
        let mut scene = SceneCoordinator::new();
        scene.test_seed_space_identity_worlds(space_id, vec![RenderTransform::default()], vec![-1]);
        (
            scene,
            HostCameraFrame::default(),
            WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
        )
    }

    #[test]
    fn spatial_refit_updates_bounds_without_rebuilding_run_membership() {
        let space_id = RenderSpaceId(7);
        let (scene, host_camera, proj) = spatial_scene_and_cull(space_id);
        let culling = WorldMeshCullInput {
            proj,
            host_camera: &host_camera,
            hi_z: None,
            hi_z_temporal: None,
            hysteresis: None,
        };
        let mut draws = (0..80)
            .map(|idx| {
                prepared_draw_with_bounds(
                    space_id,
                    idx,
                    Vec3::new(2.0, -0.5, -0.5),
                    Vec3::new(3.0, 0.5, 0.5),
                )
            })
            .collect::<Vec<_>>();
        let runs = one_draw_runs(draws.len());
        let mut spatial = PreparedSpatialIndex::default();
        spatial.rebuild(&draws, &runs);
        let before = spatial.query_runs(&runs, &[space_id], &scene, Some(&culling));

        set_world_aabb(
            &mut draws[0],
            Some((Vec3::new(-0.25, -0.25, -0.25), Vec3::new(0.25, 0.25, 0.25))),
        );
        let refit_count = spatial.refit_spaces(&draws, &runs, [space_id]);
        let after = spatial.query_runs(&runs, &[space_id], &scene, Some(&culling));

        assert!(spatial.space_uses_bvh_for_tests(space_id));
        assert_eq!(before.runs.len(), 0);
        assert_eq!(refit_count, 1);
        assert_eq!(after.runs.len(), 1);
        assert_eq!(after.runs[0], FramePreparedRun { start: 0, end: 1 });
        let updated = spatial
            .world_casters()
            .find(|bounds| bounds.renderable_index == 0)
            .expect("refit keeps the caster");
        assert_eq!(updated.aabb_min, Vec3::new(-0.25, -0.25, -0.25));
        assert_eq!(updated.aabb_max, Vec3::new(0.25, 0.25, 0.25));
    }

    #[test]
    fn spatial_refit_inserts_a_caster_whose_bounds_appear_and_keeps_run_order() {
        let space_id = RenderSpaceId(7);
        let mut draws = vec![
            prepared_draw_with_bounds(space_id, 0, Vec3::ZERO, Vec3::ONE),
            prepared_draw_with_bounds(space_id, 1, Vec3::ZERO, Vec3::ONE),
            prepared_draw_with_bounds(space_id, 2, Vec3::ZERO, Vec3::ONE),
        ];
        set_world_aabb(&mut draws[1], None);
        let runs = one_draw_runs(draws.len());
        let mut spatial = PreparedSpatialIndex::default();
        spatial.rebuild(&draws, &runs);
        assert_eq!(caster_indices(&spatial), vec![0, 2]);

        set_world_aabb(&mut draws[1], Some((Vec3::ZERO, Vec3::ONE)));
        spatial.refit_spaces(&draws, &runs, [space_id]);
        assert_eq!(caster_indices(&spatial), vec![0, 1, 2]);

        set_world_aabb(&mut draws[0], None);
        spatial.refit_spaces(&draws, &runs, [space_id]);
        assert_eq!(caster_indices(&spatial), vec![1, 2]);
    }

    #[test]
    fn spatial_refit_reinserts_an_indexed_run_whose_bounds_return() {
        let space_id = RenderSpaceId(7);
        let mut draws = (0..80)
            .map(|idx| {
                let x = idx as f32;
                prepared_draw_with_bounds(
                    space_id,
                    idx,
                    Vec3::new(x, 0.0, 0.0),
                    Vec3::new(x + 0.5, 1.0, 1.0),
                )
            })
            .collect::<Vec<_>>();
        let runs = one_draw_runs(draws.len());
        let mut spatial = PreparedSpatialIndex::default();
        spatial.rebuild(&draws, &runs);
        assert!(spatial.space_uses_bvh_for_tests(space_id));
        assert_eq!(caster_indices(&spatial), (0..80).collect::<Vec<_>>());

        set_world_aabb(&mut draws[10], None);
        spatial.refit_spaces(&draws, &runs, [space_id]);
        let expected = (0..80).filter(|idx| *idx != 10).collect::<Vec<_>>();
        assert_eq!(caster_indices(&spatial), expected);

        let restored_min = Vec3::new(10.0, 2.0, 3.0);
        let restored_max = Vec3::new(10.5, 4.0, 5.0);
        set_world_aabb(&mut draws[10], Some((restored_min, restored_max)));
        spatial.refit_spaces(&draws, &runs, [space_id]);
        assert_eq!(caster_indices(&spatial), (0..80).collect::<Vec<_>>());
        let restored = spatial
            .world_casters()
            .find(|bounds| bounds.renderable_index == 10)
            .expect("indexed caster returns");
        assert_eq!(restored.aabb_min, restored_min);
        assert_eq!(restored.aabb_max, restored_max);
    }

    #[test]
    fn world_casters_keep_run_order_across_spaces_when_one_space_refits() {
        let space_a = RenderSpaceId(1);
        let space_b = RenderSpaceId(2);
        let mut draws = vec![
            prepared_draw_with_bounds(space_a, 0, Vec3::ZERO, Vec3::ONE),
            prepared_draw_with_bounds(
                space_b,
                1,
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(5.0, 1.0, 1.0),
            ),
            prepared_draw_with_bounds(
                space_a,
                2,
                Vec3::new(8.0, 0.0, 0.0),
                Vec3::new(9.0, 1.0, 1.0),
            ),
        ];
        let runs = one_draw_runs(draws.len());
        let mut spatial = PreparedSpatialIndex::default();
        spatial.rebuild(&draws, &runs);

        let moved_min = Vec3::new(0.5, 0.0, 0.0);
        let moved_max = Vec3::new(1.5, 1.0, 1.0);
        set_world_aabb(&mut draws[0], Some((moved_min, moved_max)));
        spatial.refit_spaces(&draws, &runs, [space_a]);
        assert_eq!(caster_indices(&spatial), vec![0, 1, 2]);
        let moved = spatial
            .world_casters()
            .find(|bounds| bounds.renderable_index == 0)
            .expect("refit keeps the moved caster");
        assert_eq!(moved.aabb_min, moved_min);
        assert_eq!(moved.aabb_max, moved_max);
        let other = spatial
            .world_casters()
            .find(|bounds| bounds.renderable_index == 1)
            .expect("the other space stays");
        assert_eq!(other.aabb_min, Vec3::new(4.0, 0.0, 0.0));

        set_world_aabb(&mut draws[2], None);
        spatial.refit_spaces(&draws, &runs, [space_a]);
        assert_eq!(caster_indices(&spatial), vec![0, 1]);
    }

    #[test]
    fn sparse_candidate_gather_preserves_prepared_order_and_dedups() {
        let runs = [
            FramePreparedRun { start: 0, end: 1 },
            FramePreparedRun { start: 1, end: 2 },
            FramePreparedRun { start: 2, end: 3 },
            FramePreparedRun { start: 3, end: 4 },
        ];
        let mut candidate_indices = vec![2, 0, 2];

        let gathered = gather_spatial_candidate_runs_sparse(&runs, &mut candidate_indices);

        assert_eq!(
            gathered,
            vec![
                FramePreparedRun { start: 0, end: 1 },
                FramePreparedRun { start: 2, end: 3 },
            ]
        );
    }

    #[test]
    fn spatial_candidate_gather_switches_to_dense_for_high_density() {
        assert!(!should_gather_spatial_candidates_dense(1, 3));
        assert!(should_gather_spatial_candidates_dense(2, 3));
    }
}
