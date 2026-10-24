//! LOD selection for a skinned renderer that has no posed box yet.
//!
//! The prepared group stores no world AABB until a posed box arrives. Selection then keeps the
//! first live LOD instead of treating the missing box as a failed screen-height test.

use glam::{Mat4, Quat, Vec3};

use super::super::super::prepared_renderables::{FramePreparedDraw, FramePreparedRenderables};
use super::super::{
    DrawCollectionFrameCaches, DrawCollectionInputs, DrawCollectionMaterialInputs,
    DrawCollectionSceneAssets, DrawCollectionViewInputs,
};
use super::{select_group_lod, select_prepared_group_lod};
use crate::camera::{EyeView, HostCameraFrame};
use crate::gpu_pools::MeshPool;
use crate::materials::host_data::{MaterialDictionary, MaterialPropertyStore, PropertyIdRegistry};
use crate::materials::{
    MaterialPipelinePropertyIds, MaterialRouter, RasterPipelineKind, ShaderPermutation,
};
use crate::particles::ParticleDrawParams;
use crate::scene::{
    LodEntry, LodGroupEntry, LodRendererKind, LodRendererRef, MeshRendererInstanceId,
    RenderSpaceId, SceneCoordinator, SceneMeshRendererRead, SkinnedMeshRenderer,
    StaticMeshRenderer,
};
use crate::shared::{RenderBoundingBox, RenderTransform, RenderingContext, ShadowCastMode};
use crate::world_mesh::culling::{
    MeshCullGeometry, MeshCullTarget, WorldMeshCullInput, WorldMeshCullProjParams,
    mesh_world_geometry_for_cull_with_head,
};
use crate::world_mesh::{ViewLayerPolicy, ViewRenderSpaceScope};

const SPACE: RenderSpaceId = RenderSpaceId(166);
const INSTANCE: MeshRendererInstanceId = MeshRendererInstanceId(7);
const HIGH_THRESHOLD: f32 = 0.9;

fn identity_scene() -> SceneCoordinator {
    let mut scene = SceneCoordinator::new();
    let identity = RenderTransform {
        position: Vec3::ZERO,
        scale: Vec3::ONE,
        rotation: Quat::IDENTITY,
    };
    scene.test_seed_space_identity_worlds(SPACE, vec![identity], vec![-1]);
    let renderer = SkinnedMeshRenderer {
        base: StaticMeshRenderer {
            instance_id: INSTANCE,
            node_id: 0,
            mesh_asset_id: 10,
            ..StaticMeshRenderer::default()
        },
        root_bone_transform_id: Some(0),
        posed_object_bounds: None,
        ..SkinnedMeshRenderer::default()
    };
    scene.test_set_skinned_mesh_renderers(SPACE, vec![renderer]);
    scene.test_set_lod_groups(
        SPACE,
        vec![LodGroupEntry {
            node_id: 0,
            lods: vec![LodEntry {
                screen_relative_transition_height: HIGH_THRESHOLD,
                renderers: vec![LodRendererRef {
                    kind: LodRendererKind::Skinned,
                    instance_id: INSTANCE,
                    renderable_index_hint: 0,
                }],
                ..LodEntry::default()
            }],
            ..LodGroupEntry::default()
        }],
    );
    scene
}

fn boxless_geometry(scene: &SceneCoordinator) -> MeshCullGeometry {
    let rest = RenderBoundingBox {
        center: Vec3::new(40.0, 0.0, 0.0),
        extents: Vec3::splat(0.5),
    };
    let renderer = scene
        .skinned_mesh_renderers(SPACE)
        .and_then(|renderers| renderers.first())
        .expect("skinned renderer");
    mesh_world_geometry_for_cull_with_head(
        &MeshCullTarget {
            scene,
            space_id: SPACE,
            mesh_bounds: &rest,
            skinned: true,
            skinned_renderer: Some(renderer),
            node_id: 0,
        },
        Mat4::IDENTITY,
        RenderingContext::UserView,
    )
}

fn boxless_draw(geometry: MeshCullGeometry) -> FramePreparedDraw {
    FramePreparedDraw {
        space_id: SPACE,
        renderable_index: 0,
        instance_id: INSTANCE,
        renderer_ordinal: 0,
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
        slot_index: 0,
        material_stack_order: None,
        first_index: 0,
        index_count: 3,
        material_asset_id: 1,
        property_block_id: None,
        cull_geometry: Some(geometry),
        rigid_world_matrix_override: None,
        particle_draw: ParticleDrawParams::default(),
    }
}

fn identity_cull(host_camera: &HostCameraFrame) -> WorldMeshCullInput<'_> {
    WorldMeshCullInput {
        proj: WorldMeshCullProjParams {
            world_proj: Mat4::IDENTITY,
            overlay_proj: Mat4::IDENTITY,
            vr_stereo: None,
        },
        host_camera,
        hi_z: None,
        hi_z_temporal: None,
    }
}

fn collection_inputs<'a>(
    scene: &'a SceneCoordinator,
    mesh_pool: &'a MeshPool,
    material_dict: &'a MaterialDictionary<'a>,
    router: &'a MaterialRouter,
    property_ids: &'a MaterialPipelinePropertyIds,
    culling: Option<&'a WorldMeshCullInput<'a>>,
) -> DrawCollectionInputs<'a> {
    DrawCollectionInputs {
        scene_assets: DrawCollectionSceneAssets { scene, mesh_pool },
        materials: DrawCollectionMaterialInputs {
            dict: material_dict,
            router,
            pipeline_property_ids: property_ids,
            shader_perm: ShaderPermutation::default(),
            property_registry: None,
        },
        view: DrawCollectionViewInputs {
            render_context: RenderingContext::UserView,
            head_output_transform: Mat4::IDENTITY,
            view_origin_world: Vec3::ZERO,
            culling,
            mesh_lod_bias: 1.0,
            transform_filter: None,
            transform_filter_space: None,
            render_space_scope: ViewRenderSpaceScope::AllActive,
            layer_policy: ViewLayerPolicy::MainView,
            reflection_probes: None,
        },
        caches: DrawCollectionFrameCaches {
            material_cache: None,
            prepared: None,
        },
    }
}

#[test]
fn boxless_skinned_lod_keeps_the_renderer_until_a_posed_box_arrives() {
    let scene = identity_scene();
    let geometry = boxless_geometry(&scene);
    assert!(geometry.world_aabb.is_none());
    let draw = boxless_draw(geometry);
    let mut prepared = FramePreparedRenderables::empty(RenderingContext::UserView);
    prepared.begin_cached_rebuild(RenderingContext::UserView);
    prepared.push_cached_space(SPACE);
    prepared.extend_cached_draws(std::slice::from_ref(&draw));
    prepared.finish_cached_rebuild(&scene);

    let groups = prepared.lod_groups();
    assert_eq!(groups.len(), 1);
    assert!(groups[0].world_aabb.is_none());
    assert_eq!(groups[0].lods[0].renderers.len(), 1);

    let mesh_pool = MeshPool::default_pool();
    let store = MaterialPropertyStore::new();
    let material_dict = MaterialDictionary::new(&store);
    let router = MaterialRouter::new(RasterPipelineKind::Null);
    let registry = PropertyIdRegistry::new();
    let property_ids = MaterialPipelinePropertyIds::new(&registry);
    let host_camera = HostCameraFrame {
        explicit_view: Some(EyeView::new(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Vec3::ZERO,
        )),
        ..HostCameraFrame::default()
    };
    let culling = identity_cull(&host_camera);
    let ctx = collection_inputs(
        &scene,
        &mesh_pool,
        &material_dict,
        &router,
        &property_ids,
        Some(&culling),
    );

    let mut visibility = super::LodVisibility::for_spaces(&ctx, &[SPACE]);
    select_prepared_group_lod(&ctx, &culling, &groups[0], &mut visibility);
    assert!(visibility.renderer_visible(SPACE, 0));

    let space = scene.space(SPACE).expect("seeded space");
    let mut scene_visibility = super::LodVisibility::for_spaces(&ctx, &[SPACE]);
    select_group_lod(
        &ctx,
        &culling,
        SPACE,
        space,
        &space.lod_groups()[0].lods,
        space.lod_groups()[0].cross_fade,
        &mut scene_visibility,
    );
    assert!(scene_visibility.renderer_visible_by_instance(SPACE, INSTANCE));

    let mut with_box = groups[0].clone();
    with_box.world_aabb = Some((Vec3::splat(-0.05), Vec3::splat(0.05)));
    let mut culled = super::LodVisibility::for_spaces(&ctx, &[SPACE]);
    select_prepared_group_lod(&ctx, &culling, &with_box, &mut culled);
    assert!(!culled.renderer_visible(SPACE, 0));
}
