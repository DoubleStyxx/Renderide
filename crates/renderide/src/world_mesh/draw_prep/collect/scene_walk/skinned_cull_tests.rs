//! Skinned frustum and Hi-Z decisions for the scene-walk collector.
//!
//! These tests call [`super::cull_cache::cull_mesh_target`], the function
//! [`super::cull_cache::compute_cached_cull`] and the single-slot inline path both use.
//! Posed bone bounds are the cull box. An extreme pose outside the rest AABB stays visible, and
//! a skinned draw with no posed box stays visible instead of being tested against the rest AABB.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use hashbrown::HashMap;

use super::super::{
    DrawCollectionFrameCaches, DrawCollectionInputs, DrawCollectionMaterialInputs,
    DrawCollectionSceneAssets, DrawCollectionViewInputs, ViewLayerPolicy, ViewRenderSpaceScope,
    render_space_visible_in_view,
};
use super::cull_cache::{cache_from_outcome, cull_mesh_target, result_from_cached};
use crate::camera::{EyeView, HostCameraFrame};
use crate::cull_contract::HiZTemporalState;
use crate::hi_z_cpu::{HiZCpuSnapshot, HiZCullData};
use crate::materials::host_data::{MaterialDictionary, MaterialPropertyStore, PropertyIdRegistry};
use crate::materials::{MaterialPipelinePropertyIds, MaterialRouter, RasterPipelineKind};
use crate::scene::{RenderSpaceId, SceneCoordinator, SkinnedMeshRenderer, StaticMeshRenderer};
use crate::shared::{RenderBoundingBox, RenderTransform, RenderingContext};
use crate::world_mesh::culling::{
    CpuCullFailure, MeshCullTarget, WorldMeshCullInput, WorldMeshCullProjParams,
    mesh_world_geometry_for_cull_with_head,
};

const SPACE: RenderSpaceId = RenderSpaceId(166);

fn golden(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../renderide-test/goldens")
        .join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn seeded_scene() -> SceneCoordinator {
    let mut scene = SceneCoordinator::new();
    let identity = RenderTransform {
        position: Vec3::ZERO,
        scale: Vec3::ONE,
        rotation: Quat::IDENTITY,
    };
    scene.test_seed_space_identity_worlds(SPACE, vec![identity], vec![-1]);
    scene
}

fn bounds(center: Vec3, extents: Vec3) -> RenderBoundingBox {
    RenderBoundingBox { center, extents }
}

fn skinned(posed: Option<RenderBoundingBox>) -> SkinnedMeshRenderer {
    SkinnedMeshRenderer {
        base: StaticMeshRenderer {
            node_id: 0,
            ..StaticMeshRenderer::default()
        },
        root_bone_transform_id: Some(0),
        posed_object_bounds: posed,
        ..SkinnedMeshRenderer::default()
    }
}

fn mono_culling(host_camera: &HostCameraFrame) -> WorldMeshCullInput<'_> {
    // Same clip as the prepared spatial golden: identity projection, default space view.
    // A unit box on the origin survives; a box at x=40 does not.
    WorldMeshCullInput {
        proj: WorldMeshCullProjParams {
            world_proj: Mat4::IDENTITY,
            overlay_proj: Mat4::IDENTITY,
            vr_stereo: None,
        },
        host_camera,
        hi_z: None,
        hi_z_temporal: None,
        hysteresis: None,
    }
}

fn decide(
    scene: &SceneCoordinator,
    rest: &RenderBoundingBox,
    skinned_renderer: Option<&SkinnedMeshRenderer>,
    culling: Option<&WorldMeshCullInput<'_>>,
    is_overlay: bool,
) -> Option<Result<Option<Mat4>, CpuCullFailure>> {
    cull_mesh_target(
        &MeshCullTarget {
            scene,
            space_id: SPACE,
            mesh_bounds: rest,
            skinned: skinned_renderer.is_some(),
            skinned_renderer,
            node_id: 0,
        },
        is_overlay,
        culling,
        RenderingContext::UserView,
        None,
    )
}

#[test]
fn scene_walk_cull_does_not_special_case_skinned_draws() {
    let src = include_str!("cull_cache.rs");
    assert!(
        !src.contains("if draw.skinned"),
        "skinned draws must share the rigid frustum and Hi-Z path"
    );
}

#[test]
fn extreme_pose_outside_rest_aabb_stays_visible_and_slots_share_it() {
    let contract = golden("skinned_onscreen_cull.json");
    let scene = seeded_scene();
    let rest = bounds(Vec3::new(40.0, 0.0, 0.0), Vec3::splat(0.5));
    let posed = skinned(Some(bounds(Vec3::ZERO, Vec3::splat(0.25))));
    let rest_as_posed = skinned(Some(rest));
    let host_camera = HostCameraFrame::default();
    let culling = mono_culling(&host_camera);

    let rest_only = decide(&scene, &rest, Some(&rest_as_posed), Some(&culling), false);
    let posed_visible = decide(&scene, &rest, Some(&posed), Some(&culling), false);
    assert!(
        matches!(rest_only, Some(Err(CpuCullFailure::Frustum))),
        "rest AABB at x=40 is outside the camera"
    );
    assert_eq!(posed_visible, Some(Ok(None)));
    let outcome = posed_visible.expect("cull ran");
    let cached = cache_from_outcome(outcome);
    let slot0 = result_from_cached(&cached);
    let slot1 = result_from_cached(&cached);
    assert_eq!(slot0, outcome);
    assert_eq!(slot1, outcome);
    assert!(outcome.unwrap().is_none());

    assert!(
        contract["extreme_pose_stays_visible"]
            .as_bool()
            .unwrap_or(false)
    );
    assert!(
        contract["rest_pose_would_be_frustum_rejected"]
            .as_bool()
            .unwrap_or(false)
    );
    assert!(
        contract["accepted_rigid_world_matrix_is_null"]
            .as_bool()
            .unwrap_or(false)
    );
    assert_eq!(
        contract["material_slots_sharing_one_cull"].as_u64(),
        Some(2)
    );
}

#[test]
fn missing_posed_bounds_stay_visible_when_the_rest_aabb_is_offscreen() {
    let scene = seeded_scene();
    let rest = bounds(Vec3::new(40.0, 0.0, 0.0), Vec3::splat(0.5));
    let host_camera = HostCameraFrame::default();
    let culling = mono_culling(&host_camera);
    let renderer = skinned(None);

    let decision = decide(&scene, &rest, Some(&renderer), Some(&culling), false);
    assert_eq!(
        decision,
        Some(Ok(None)),
        "a pose can leave the rest AABB, so a missing posed box must not frustum-reject"
    );

    let geometry = mesh_world_geometry_for_cull_with_head(
        &MeshCullTarget {
            scene: &scene,
            space_id: SPACE,
            mesh_bounds: &rest,
            skinned: true,
            skinned_renderer: Some(&renderer),
            node_id: 0,
        },
        Mat4::IDENTITY,
        RenderingContext::UserView,
    );
    assert!(geometry.world_aabb.is_none());
    assert!(geometry.rigid_world_matrix.is_none());
    assert!(geometry.front_face_world_matrix.is_some());
}

#[test]
fn posed_bounds_offscreen_cull_even_when_rest_bounds_are_onscreen() {
    let contract = golden("skinned_onscreen_cull.json");
    let scene = seeded_scene();
    let rest = bounds(Vec3::ZERO, Vec3::splat(0.25));
    let posed = skinned(Some(bounds(Vec3::new(40.0, 0.0, 0.0), Vec3::splat(0.5))));
    let rest_as_posed = skinned(Some(rest));
    let host_camera = HostCameraFrame::default();
    let culling = mono_culling(&host_camera);

    let rest_only = decide(&scene, &rest, Some(&rest_as_posed), Some(&culling), false);
    let decision = decide(&scene, &rest, Some(&posed), Some(&culling), false);
    assert_eq!(rest_only, Some(Ok(None)));
    assert_eq!(decision, Some(Err(CpuCullFailure::Frustum)));
    assert!(
        contract["posed_offscreen_while_rest_onscreen_is_frustum_rejected"]
            .as_bool()
            .unwrap_or(false)
    );
}

#[test]
fn hi_z_uses_the_posed_aabb_not_the_rest_aabb() {
    let contract = golden("skinned_onscreen_cull.json");
    let scene = seeded_scene();
    let rest = bounds(Vec3::ZERO, Vec3::splat(5.0));
    let posed = skinned(Some(bounds(
        Vec3::new(0.0, 0.0, 0.825),
        Vec3::new(0.01, 0.01, 0.025),
    )));
    let host_camera = HostCameraFrame {
        explicit_view: Some(EyeView::new(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Vec3::ZERO,
        )),
        ..HostCameraFrame::default()
    };
    let snapshot = HiZCpuSnapshot {
        base_width: 4,
        base_height: 4,
        mip_levels: 3,
        mips: Arc::from(vec![0.92f32; 21]),
    };
    assert!(snapshot.validate().is_some());
    let mut prev_view_by_space = HashMap::new();
    prev_view_by_space.insert(SPACE, Mat4::IDENTITY);
    let culling = WorldMeshCullInput {
        proj: WorldMeshCullProjParams {
            world_proj: Mat4::IDENTITY,
            overlay_proj: Mat4::IDENTITY,
            vr_stereo: None,
        },
        host_camera: &host_camera,
        hi_z: Some(HiZCullData::Desktop(snapshot)),
        hi_z_temporal: Some(HiZTemporalState {
            prev_cull: WorldMeshCullProjParams {
                world_proj: Mat4::IDENTITY,
                overlay_proj: Mat4::IDENTITY,
                vr_stereo: None,
            },
            prev_view_by_space: Arc::new(prev_view_by_space),
            depth_viewport_px: (4, 4),
        }),
        hysteresis: None,
    };

    let decision = decide(&scene, &rest, Some(&posed), Some(&culling), false);
    assert_eq!(decision, Some(Err(CpuCullFailure::HiZ)));
    let cached = cache_from_outcome(decision.expect("cull ran"));
    assert_eq!(result_from_cached(&cached), Err(CpuCullFailure::HiZ));
    assert_eq!(result_from_cached(&cached), result_from_cached(&cached));
    assert!(contract["hiz_uses_posed_aabb"].as_bool().unwrap_or(false));
    assert_eq!(contract["hiz_rejected_slots"].as_u64(), Some(2));
}

#[test]
fn shadow_and_probe_walks_without_culling_do_not_reject_skinned_draws() {
    let contract = golden("skinned_onscreen_cull.json");
    let scene = seeded_scene();
    let rest = bounds(Vec3::new(40.0, 0.0, 0.0), Vec3::splat(0.5));
    let posed = skinned(Some(bounds(Vec3::new(40.0, 0.0, 0.0), Vec3::splat(0.5))));
    let decision = decide(&scene, &rest, Some(&posed), None, false);
    assert!(decision.is_none());
    let overlay = {
        let host_camera = HostCameraFrame::default();
        let culling = mono_culling(&host_camera);
        decide(&scene, &rest, Some(&posed), Some(&culling), true)
    };
    assert_eq!(overlay, Some(Ok(None)));
    assert!(
        contract["shadow_without_culling_is_not_rejected"]
            .as_bool()
            .unwrap_or(false)
    );
}

#[test]
fn private_ui_gate_still_rejects_a_private_space_before_skinned_cull() {
    let mut scene = SceneCoordinator::new();
    scene.test_seed_space_identity_worlds(SPACE, vec![RenderTransform::default()], vec![-1]);
    scene.test_set_space_private(SPACE, true);
    let mesh_pool = crate::gpu_pools::MeshPool::default_pool();
    let store = MaterialPropertyStore::new();
    let material_dict = MaterialDictionary::new(&store);
    let router = MaterialRouter::new(RasterPipelineKind::Null);
    let registry = PropertyIdRegistry::new();
    let property_ids = MaterialPipelinePropertyIds::new(&registry);
    let hidden = draw_inputs(
        &scene,
        &mesh_pool,
        &material_dict,
        &router,
        &property_ids,
        ViewLayerPolicy::camera(false),
    );
    let shown = draw_inputs(
        &scene,
        &mesh_pool,
        &material_dict,
        &router,
        &property_ids,
        ViewLayerPolicy::camera(true),
    );
    let main = draw_inputs(
        &scene,
        &mesh_pool,
        &material_dict,
        &router,
        &property_ids,
        ViewLayerPolicy::MainView,
    );
    assert!(!render_space_visible_in_view(&hidden, SPACE));
    assert!(render_space_visible_in_view(&shown, SPACE));
    assert!(render_space_visible_in_view(&main, SPACE));
}

fn draw_inputs<'a>(
    scene: &'a SceneCoordinator,
    mesh_pool: &'a crate::gpu_pools::MeshPool,
    material_dict: &'a MaterialDictionary<'a>,
    router: &'a MaterialRouter,
    property_ids: &'a MaterialPipelinePropertyIds,
    layer_policy: ViewLayerPolicy,
) -> DrawCollectionInputs<'a> {
    DrawCollectionInputs {
        scene_assets: DrawCollectionSceneAssets { scene, mesh_pool },
        materials: DrawCollectionMaterialInputs {
            dict: material_dict,
            router,
            pipeline_property_ids: property_ids,
            shader_perm: crate::materials::ShaderPermutation::default(),
            property_registry: None,
        },
        view: DrawCollectionViewInputs {
            render_context: RenderingContext::UserView,
            head_output_transform: Mat4::IDENTITY,
            view_origin_world: Vec3::ZERO,
            culling: None,
            mesh_lod_bias: 1.0,
            transform_filter: None,
            transform_filter_space: Some(SPACE),
            render_space_scope: ViewRenderSpaceScope::single(SPACE),
            layer_policy,
            reflection_probes: None,
        },
        caches: DrawCollectionFrameCaches {
            material_cache: None,
            prepared: None,
        },
    }
}
