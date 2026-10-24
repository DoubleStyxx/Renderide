//! Transparent sorting for a skinned draw that has no world AABB yet.

use std::cmp::Ordering;

use glam::{Mat4, Vec3};

use super::super::DrawCollectionInputs;
use super::{DrawCandidate, evaluate_draw_candidate};
use crate::gpu_pools::MeshPool;
use crate::materials::host_data::{
    MaterialDictionary, MaterialPropertyStore, MaterialPropertyValue, PropertyIdRegistry,
};
use crate::materials::{
    MaterialPipelinePropertyIds, MaterialRouter, RasterFrontFace, RasterPipelineKind,
    RasterPrimitiveTopology, UNITY_TRANSPARENT_RENDER_QUEUE_MIN,
};
use crate::particles::ParticleDrawParams;
use crate::scene::{MeshRendererInstanceId, RenderSpaceId, SceneCoordinator};
use crate::shared::{RenderingContext, ShadowCastMode};
use crate::world_mesh::draw_prep::collect::{
    DrawCollectionFrameCaches, DrawCollectionMaterialInputs, DrawCollectionSceneAssets,
    DrawCollectionViewInputs,
};
use crate::world_mesh::draw_prep::sort::cmp_transparent_intra_run;
use crate::world_mesh::materials::FrameMaterialBatchCache;
use crate::world_mesh::{ViewLayerPolicy, ViewRenderSpaceScope};

fn transparent_context<'a>(
    scene: &'a SceneCoordinator,
    mesh_pool: &'a MeshPool,
    material_dict: &'a MaterialDictionary<'a>,
    router: &'a MaterialRouter,
    property_ids: &'a MaterialPipelinePropertyIds,
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

fn skinned_candidate(world_aabb: Option<(Vec3, Vec3)>) -> DrawCandidate {
    DrawCandidate {
        space_id: RenderSpaceId(166),
        node_id: 0,
        renderable_index: 0,
        instance_id: MeshRendererInstanceId(7),
        mesh_asset_id: 10,
        slot_index: 0,
        material_stack_order: None,
        first_index: 0,
        index_count: 3,
        is_overlay: false,
        sorting_order: 0,
        shadow_cast_mode: ShadowCastMode::On,
        skinned: true,
        world_space_deformed: true,
        blendshape_deformed: false,
        tangent_blendshape_deform_active: false,
        material_asset_id: 11,
        property_block_id: None,
        world_aabb,
        particle_draw: ParticleDrawParams::default(),
        lod_fade: 0.0,
    }
}

#[test]
fn boxless_skinned_transparent_draw_sorts_with_the_fallback_distance() {
    let scene = SceneCoordinator::new();
    let mesh_pool = MeshPool::default_pool();
    let mut store = MaterialPropertyStore::new();
    let registry = PropertyIdRegistry::new();
    let property_ids = MaterialPipelinePropertyIds::new(&registry);
    store.set_shader_asset_for_material(11, 22);
    store.set_material(
        11,
        property_ids.render_queue[0],
        MaterialPropertyValue::Float(UNITY_TRANSPARENT_RENDER_QUEUE_MIN as f32),
    );
    let material_dict = MaterialDictionary::new(&store);
    let mut router = MaterialRouter::new(RasterPipelineKind::Null);
    router.set_shader_pipeline(
        22,
        RasterPipelineKind::EmbeddedStem(std::sync::Arc::<str>::from("ui_unlit_default")),
    );
    let cache = FrameMaterialBatchCache::new();
    let ctx = transparent_context(&scene, &mesh_pool, &material_dict, &router, &property_ids);

    let boxless = evaluate_draw_candidate(
        &ctx,
        &cache,
        skinned_candidate(None),
        RasterFrontFace::Clockwise,
        RasterPrimitiveTopology::TriangleList,
        None,
        16.0,
    )
    .expect("box-less skinned draw");
    let posed = evaluate_draw_candidate(
        &ctx,
        &cache,
        skinned_candidate(Some((Vec3::new(9.0, -2.0, 0.0), Vec3::new(11.0, 2.0, 0.0)))),
        RasterFrontFace::Clockwise,
        RasterPrimitiveTopology::TriangleList,
        None,
        16.0,
    )
    .expect("posed skinned draw");

    assert!(boxless.skinned);
    assert!(boxless.world_aabb.is_none());
    assert!(boxless.batch_key.uses_transparent_sorting());
    assert_eq!(boxless.camera_distance_sq, 16.0);
    assert!(posed.batch_key.uses_transparent_sorting());
    assert_eq!(posed.camera_distance_sq, 125.0);
    assert_eq!(
        cmp_transparent_intra_run(&posed, &boxless),
        Ordering::Less,
        "the farther posed box sorts behind the empty-AABB fallback"
    );
    assert_eq!(
        cmp_transparent_intra_run(&boxless, &posed),
        Ordering::Greater
    );
}
