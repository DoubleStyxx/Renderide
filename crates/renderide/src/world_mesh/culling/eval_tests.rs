#[cfg(test)]
mod hi_z_temporal_match_tests {
    //! [`super::hi_z_snapshot_matches_temporal`] dimension checks (stale-pyramid guard).

    use std::sync::Arc;

    use glam::Mat4;
    use hashbrown::HashMap;

    use super::hi_z_snapshot_matches_temporal;
    use crate::hi_z_cpu::pyramid::total_float_count;
    use crate::hi_z_cpu::{HiZCpuSnapshot, HiZCullData};
    use crate::world_mesh::culling::{HiZTemporalState, WorldMeshCullProjParams};

    fn dummy_temporal(depth_viewport_px: (u32, u32)) -> HiZTemporalState {
        HiZTemporalState {
            prev_cull: WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
            prev_view_by_space: Arc::new(HashMap::new()),
            depth_viewport_px,
        }
    }

    fn snapshot(wx: u32, hy: u32) -> HiZCpuSnapshot {
        let mip_levels = 1u32;
        let n = total_float_count(wx, hy, mip_levels);
        HiZCpuSnapshot {
            base_width: wx,
            base_height: hy,
            mip_levels,
            mips: Arc::from(vec![0.0; n]),
        }
    }

    #[test]
    fn desktop_matches_when_mip0_matches_temporal_viewport() {
        let t = dummy_temporal((128, 96));
        let hi = HiZCullData::Desktop(snapshot(128, 96));
        assert!(hi_z_snapshot_matches_temporal(&hi, &t));
    }

    #[test]
    fn desktop_mismatches_when_pyramid_resolution_differs() {
        let t = dummy_temporal((128, 96));
        let hi = HiZCullData::Desktop(snapshot(64, 96));
        assert!(!hi_z_snapshot_matches_temporal(&hi, &t));
    }

    #[test]
    fn stereo_matches_left_eye_mip0_against_temporal_viewport() {
        let t = dummy_temporal((256, 144));
        let left = snapshot(256, 144);
        let right = snapshot(1, 1);
        let hi = HiZCullData::Stereo { left, right };
        assert!(hi_z_snapshot_matches_temporal(&hi, &t));
    }

    #[test]
    fn stereo_mismatches_when_left_eye_size_differs() {
        let t = dummy_temporal((256, 144));
        let left = snapshot(128, 144);
        let right = snapshot(256, 144);
        let hi = HiZCullData::Stereo { left, right };
        assert!(!hi_z_snapshot_matches_temporal(&hi, &t));
    }
}

#[cfg(test)]
mod overlay_cull_tests {
    use glam::{Mat4, Vec3, Vec4};

    use super::{CpuCullFailure, mesh_cpu_cull_with_geometry};
    use crate::camera::{CameraClipPlanes, HostCameraFrame, Viewport};
    use crate::scene::{RenderSpaceId, SceneCoordinator};
    use crate::world_mesh::culling::{
        MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams,
    };

    fn culling_with_overlay_proj(host_camera: &HostCameraFrame) -> WorldMeshCullInput<'_> {
        let overlay_proj = HostCameraFrame::overlay_projection(
            Viewport::from_tuple((100, 100)),
            CameraClipPlanes::new(3.0, 4.0),
        );
        WorldMeshCullInput {
            proj: WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj,
                vr_stereo: None,
            },
            host_camera,
            hi_z: None,
            hi_z_temporal: None,
            hysteresis: None,
        }
    }

    #[test]
    fn overlay_draws_bypass_world_space_frustum_checks() {
        let scene = SceneCoordinator::new();
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
            hysteresis: None,
        };
        let model = Mat4::from_translation(Vec3::new(1234.0, 5678.0, 0.0));
        let geom = MeshCullGeometry {
            world_aabb: Some((Vec3::splat(10_000.0), Vec3::splat(10_001.0))),
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let Ok(accepted) = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            None,
            None,
        ) else {
            panic!("overlay items should skip frustum/Hi-Z rejection");
        };

        assert_eq!(accepted, Some(model));
    }

    #[test]
    fn overlay_rect_outside_viewport_is_culled() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = culling_with_overlay_proj(&host_camera);
        // Translate the rect 10 units to the right -- well outside the desktop overlay frustum.
        let model = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let rect = Vec4::new(0.0, 0.0, 0.5, 0.5);
        let geom = MeshCullGeometry {
            world_aabb: None,
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let res = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            Some(rect),
            None,
        );
        assert!(matches!(res, Err(CpuCullFailure::UiRectMask)));
    }

    #[test]
    fn overlay_rect_inside_viewport_passes() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = culling_with_overlay_proj(&host_camera);
        let model = Mat4::IDENTITY;
        let rect = Vec4::new(-0.5, -0.5, 0.5, 0.5);
        let geom = MeshCullGeometry {
            world_aabb: None,
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let res = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            Some(rect),
            None,
        );
        assert!(matches!(res, Ok(Some(m)) if m == model));
    }

    #[test]
    fn overlay_without_rect_clip_still_passes() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = culling_with_overlay_proj(&host_camera);
        // Same off-screen model as the culled case -- without a rect the overlay fast path must
        // still accept the draw, otherwise non-`_RectClip` overlay UI would regress.
        let model = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let geom = MeshCullGeometry {
            world_aabb: None,
            rigid_world_matrix: Some(model),
            front_face_world_matrix: Some(model),
        };

        let res = mesh_cpu_cull_with_geometry(
            geom,
            &scene,
            RenderSpaceId(999),
            true,
            &culling,
            None,
            None,
        );
        assert!(matches!(res, Ok(Some(m)) if m == model));
    }
}

#[cfg(test)]
mod prepared_space_cull_tests {
    use glam::{Mat4, Vec3};

    use super::{
        PreparedSpaceCull, SpaceCullCache, mesh_cpu_cull_with_geometry, world_aabb_visible_for_cull,
    };
    use crate::camera::{
        HostCameraFrame, overlay_camera_view_matrix, reverse_z_perspective,
        view_matrix_for_world_mesh_render_space,
    };
    use crate::scene::{RenderSpaceId, SceneCoordinator};
    use crate::shared::RenderTransform;
    use crate::world_mesh::culling::frustum::world_aabb_visible_in_homogeneous_clip;
    use crate::world_mesh::culling::{
        MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams,
    };

    fn seeded_scene(space_id: RenderSpaceId) -> SceneCoordinator {
        let mut scene = SceneCoordinator::new();
        scene.test_seed_space_identity_worlds(space_id, vec![RenderTransform::default()], vec![-1]);
        scene
    }

    fn mono_culling(host_camera: &HostCameraFrame) -> WorldMeshCullInput<'_> {
        let proj = reverse_z_perspective(16.0 / 9.0, 60f32.to_radians(), 0.1, 100.0);
        WorldMeshCullInput {
            proj: WorldMeshCullProjParams {
                world_proj: proj,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
            host_camera,
            hi_z: None,
            hi_z_temporal: None,
            hysteresis: None,
        }
    }

    #[test]
    fn prepared_frustum_matches_direct_view_projection() {
        let space_id = RenderSpaceId(3);
        let scene = seeded_scene(space_id);
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let space = scene.space(space_id).expect("seeded space");
        let view_proj =
            culling.proj.world_proj * view_matrix_for_world_mesh_render_space(&scene, space);
        let prepared = PreparedSpaceCull::frustum(&scene, space_id, false, &culling);

        let boxes = [
            (Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)),
            (Vec3::new(40.0, 0.0, 0.0), Vec3::new(41.0, 1.0, 1.0)),
            (Vec3::new(-2.0, -2.0, -8.0), Vec3::new(2.0, 2.0, -1.0)),
            (Vec3::splat(-100.0), Vec3::splat(-99.0)),
        ];
        for (min, max) in boxes {
            let direct = world_aabb_visible_in_homogeneous_clip(view_proj, min, max);
            assert_eq!(prepared.aabb_visible(min, max), direct, "{min:?} {max:?}");
            assert_eq!(
                world_aabb_visible_for_cull(&scene, space_id, false, &culling, min, max),
                direct,
                "{min:?} {max:?}"
            );
        }
    }

    #[test]
    fn missing_space_stays_visible_before_hi_z() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let min = Vec3::splat(10_000.0);
        let max = Vec3::splat(10_001.0);
        let prepared = PreparedSpaceCull::frustum(&scene, RenderSpaceId(404), false, &culling);

        assert!(prepared.aabb_visible(min, max));
        assert!(world_aabb_visible_for_cull(
            &scene,
            RenderSpaceId(404),
            false,
            &culling,
            min,
            max
        ));
    }

    #[test]
    fn space_cull_cache_matches_per_call_geometry_cull() {
        let space_id = RenderSpaceId(8);
        let scene = seeded_scene(space_id);
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let mut cache = SpaceCullCache::new();
        let boxes = [
            (Vec3::new(-0.25, -0.25, -0.25), Vec3::new(0.25, 0.25, 0.25)),
            (Vec3::new(30.0, 30.0, 30.0), Vec3::new(31.0, 31.0, 31.0)),
        ];
        for (min, max) in boxes {
            let geom = MeshCullGeometry {
                world_aabb: Some((min, max)),
                rigid_world_matrix: Some(Mat4::IDENTITY),
                front_face_world_matrix: Some(Mat4::IDENTITY),
            };
            let cached = cache.cull_geometry(geom, &scene, space_id, false, &culling, None);
            let direct =
                mesh_cpu_cull_with_geometry(geom, &scene, space_id, false, &culling, None, None);
            assert_eq!(cached, direct);
        }
    }

    #[test]
    fn prepared_overlay_frustum_matches_overlay_camera_view_projection() {
        let scene = SceneCoordinator::new();
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let view_proj = culling.proj.overlay_proj * overlay_camera_view_matrix();
        let prepared = PreparedSpaceCull::frustum(&scene, RenderSpaceId(1), true, &culling);
        let min = Vec3::new(-0.2, -0.2, 0.0);
        let max = Vec3::new(0.2, 0.2, 0.0);

        assert_eq!(
            prepared.aabb_visible(min, max),
            world_aabb_visible_in_homogeneous_clip(view_proj, min, max)
        );
    }

    /// Compares one-shot frustum preparation with a reused plan.
    ///
    /// Run with `cargo test -p renderide --lib prepared_frustum_reuse_throughput -- --ignored --nocapture`.
    #[test]
    #[ignore = "prints a local throughput comparison; run with --ignored --nocapture"]
    fn prepared_frustum_reuse_throughput() {
        use std::time::Instant;

        let space_id = RenderSpaceId(3);
        let scene = seeded_scene(space_id);
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        let boxes: Vec<(Vec3, Vec3)> = (0..20_000)
            .map(|index| {
                let x = (index as f32 * 0.37) - 100.0;
                (Vec3::new(x, -0.5, -2.0), Vec3::new(x + 0.8, 0.5, 2.0))
            })
            .collect();

        let once = Instant::now();
        let mut once_visible = 0usize;
        for &(min, max) in &boxes {
            if world_aabb_visible_for_cull(&scene, space_id, false, &culling, min, max) {
                once_visible += 1;
            }
        }
        let once_elapsed = once.elapsed();

        let reused_start = Instant::now();
        let prepared = PreparedSpaceCull::frustum(&scene, space_id, false, &culling);
        let mut reused_visible = 0usize;
        for &(min, max) in &boxes {
            if prepared.aabb_visible(min, max) {
                reused_visible += 1;
            }
        }
        let reused_elapsed = reused_start.elapsed();

        assert_eq!(once_visible, reused_visible);
        eprintln!(
            "frustum boxes={} once={once_elapsed:?} reused={reused_elapsed:?} visible={once_visible}",
            boxes.len()
        );
    }
}

#[cfg(test)]
mod hysteresis_cull_integration {
    use std::sync::Arc;

    use glam::{Mat4, Vec3};
    use hashbrown::HashMap;

    use super::{CpuCullFailure, SpaceCullCache};
    use crate::camera::{
        HostCameraFrame, reverse_z_perspective, view_matrix_for_host_world_mesh_space,
    };
    use crate::hi_z_cpu::pyramid::total_float_count;
    use crate::hi_z_cpu::{HiZCpuSnapshot, HiZCullData};
    use crate::occlusion::hysteresis::{
        CasterKey, CasterRef, DepthClip, SpaceMotion, VisibilityClass, VisibilityHistory,
        VisibilityHysteresis,
    };
    use crate::scene::{RenderSpaceId, SceneCoordinator};
    use crate::shared::RenderTransform;
    use crate::world_mesh::culling::{
        HiZTemporalState, MeshCullGeometry, WorldMeshCullInput, WorldMeshCullProjParams,
    };

    fn occluding_pyramid(depth: f32) -> HiZCpuSnapshot {
        let mip_levels = 1;
        let texels = total_float_count(32, 32, mip_levels);
        HiZCpuSnapshot {
            base_width: 32,
            base_height: 32,
            mip_levels,
            mips: Arc::from(vec![depth; texels]),
        }
    }

    fn desktop_motion(previous: Mat4, current: Mat4, host: &HostCameraFrame) -> SpaceMotion {
        SpaceMotion {
            mono: Some((previous, current)),
            stereo: None,
            clip: DepthClip {
                near: host.clip.near,
                far: host.clip.far,
                orthographic: false,
            },
        }
    }

    fn frame_for(
        history: VisibilityHistory,
        motion: &SpaceMotion,
        space_id: RenderSpaceId,
    ) -> VisibilityHysteresis {
        let mut motions = HashMap::new();
        motions.insert(space_id, *motion);
        VisibilityHysteresis::new(history, HashMap::new(), motions, Vec::new())
    }

    struct CullCase<'a> {
        scene: &'a SceneCoordinator,
        space_id: RenderSpaceId,
        host: &'a HostCameraFrame,
        proj: Mat4,
        authoring_view: Mat4,
        snapshot: &'a HiZCpuSnapshot,
        frame: &'a VisibilityHysteresis,
        aabb: (Vec3, Vec3),
        caster: CasterRef,
    }

    fn cull_caster(case: CullCase<'_>) -> Result<Option<Mat4>, CpuCullFailure> {
        let mut views = HashMap::new();
        views.insert(case.space_id, case.authoring_view);
        let projection = WorldMeshCullProjParams {
            world_proj: case.proj,
            overlay_proj: Mat4::IDENTITY,
            vr_stereo: None,
        };
        let culling = WorldMeshCullInput {
            proj: projection,
            host_camera: case.host,
            hi_z: Some(HiZCullData::Desktop(case.snapshot.clone())),
            hi_z_temporal: Some(HiZTemporalState {
                prev_cull: projection,
                prev_view_by_space: Arc::new(views),
                depth_viewport_px: (case.snapshot.base_width, case.snapshot.base_height),
            }),
            hysteresis: Some(case.frame),
        };
        let mut cache = SpaceCullCache::new();
        cache.cull_geometry_for_caster(
            MeshCullGeometry {
                world_aabb: Some(case.aabb),
                rigid_world_matrix: None,
                front_face_world_matrix: None,
            },
            case.scene,
            case.space_id,
            false,
            &culling,
            None,
            Some(case.caster),
        )
    }

    #[test]
    fn multi_frame_hysteresis_keeps_small_motion_and_fails_open_on_a_cut() {
        let space_id = RenderSpaceId(1);
        let mut scene = SceneCoordinator::new();
        scene.test_seed_space_identity_worlds(space_id, vec![RenderTransform::default()], vec![-1]);
        let host = HostCameraFrame::default();
        let space = scene.space(space_id).expect("seeded space");
        let view = view_matrix_for_host_world_mesh_space(&scene, space, &host);
        let proj = reverse_z_perspective(1.0, 60_f32.to_radians(), host.clip.near, host.clip.far);
        let previous_vp = proj * view;
        // About 1 m of covering depth. The caster sits near 3 m, so the parallax ratio stays
        // under 8 and a stable camera may still cull. A pyramid at 0.99 is the near plane and
        // must fail open; that case is a separate test.
        let snapshot = occluding_pyramid(0.01);
        let aabb = (Vec3::new(-0.15, -0.15, 2.8), Vec3::new(0.15, 0.15, 3.2));
        let key = CasterKey::new(space_id, 0, 1, 0, false);
        let caster = CasterRef {
            key,
            mesh_asset_id: 3,
        };

        let stable = desktop_motion(previous_vp, previous_vp, &host);
        let frame = frame_for(VisibilityHistory::default(), &stable, space_id);
        assert!(
            cull_caster(CullCase {
                scene: &scene,
                space_id,
                host: &host,
                proj,
                authoring_view: view,
                snapshot: &snapshot,
                frame: &frame,
                aabb,
                caster,
            })
            .is_ok(),
            "the first frame draws a caster with no history"
        );
        let history = frame.finish();
        assert_eq!(
            history.get(&key).map(|record| record.class),
            Some(VisibilityClass::Occluded)
        );

        let frame = frame_for(history, &stable, space_id);
        assert!(matches!(
            cull_caster(CullCase {
                scene: &scene,
                space_id,
                host: &host,
                proj,
                authoring_view: view,
                snapshot: &snapshot,
                frame: &frame,
                aabb,
                caster,
            }),
            Err(CpuCullFailure::HiZ)
        ));
        let history = frame.finish();

        let nudged = proj * (Mat4::from_translation(Vec3::new(0.002, 0.0, 0.0)) * view);
        let nudged_motion = desktop_motion(previous_vp, nudged, &host);
        let frame = frame_for(history, &nudged_motion, space_id);
        assert!(
            matches!(
                cull_caster(CullCase {
                    scene: &scene,
                    space_id,
                    host: &host,
                    proj,
                    authoring_view: view,
                    snapshot: &snapshot,
                    frame: &frame,
                    aabb,
                    caster,
                }),
                Err(CpuCullFailure::HiZ)
            ),
            "a small head translation still culls against a fully covering pyramid"
        );
        let history = frame.finish();

        let away = proj
            * glam::camera::rh::view::look_at_mat4(
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(0.0, 0.0, 20.0),
                Vec3::Y,
            );
        let away_motion = desktop_motion(previous_vp, away, &host);
        let frame = frame_for(history, &away_motion, space_id);
        assert!(
            cull_caster(CullCase {
                scene: &scene,
                space_id,
                host: &host,
                proj,
                authoring_view: view,
                snapshot: &snapshot,
                frame: &frame,
                aabb,
                caster,
            })
            .is_ok(),
            "a camera that loses the caster fails open"
        );
    }

    #[test]
    fn extreme_parallax_ratio_keeps_a_previously_occluded_caster() {
        let space_id = RenderSpaceId(1);
        let mut scene = SceneCoordinator::new();
        scene.test_seed_space_identity_worlds(space_id, vec![RenderTransform::default()], vec![-1]);
        let host = HostCameraFrame::default();
        let space = scene.space(space_id).expect("seeded space");
        let view = view_matrix_for_host_world_mesh_space(&scene, space, &host);
        let proj = reverse_z_perspective(1.0, 60_f32.to_radians(), host.clip.near, host.clip.far);
        let previous_vp = proj * view;
        let aabb = (Vec3::new(-0.15, -0.15, 2.8), Vec3::new(0.15, 0.15, 3.2));
        let key = CasterKey::new(space_id, 0, 1, 0, false);
        let caster = CasterRef {
            key,
            mesh_asset_id: 3,
        };
        let stable = desktop_motion(previous_vp, previous_vp, &host);
        let modest = occluding_pyramid(0.01);
        let frame = frame_for(VisibilityHistory::default(), &stable, space_id);
        assert!(
            cull_caster(CullCase {
                scene: &scene,
                space_id,
                host: &host,
                proj,
                authoring_view: view,
                snapshot: &modest,
                frame: &frame,
                aabb,
                caster,
            })
            .is_ok()
        );
        let history = frame.finish();
        assert_eq!(
            history.get(&key).map(|record| record.class),
            Some(VisibilityClass::Occluded)
        );

        let near_plane = occluding_pyramid(0.99);
        let frame = frame_for(history, &stable, space_id);
        assert!(
            cull_caster(CullCase {
                scene: &scene,
                space_id,
                host: &host,
                proj,
                authoring_view: view,
                snapshot: &near_plane,
                frame: &frame,
                aabb,
                caster,
            })
            .is_ok(),
            "a covering depth near the camera under-pads if the ratio is clamped, so the draw stays"
        );
        assert_eq!(
            frame.finish().get(&key).map(|record| record.class),
            Some(VisibilityClass::Visible)
        );
    }
}
