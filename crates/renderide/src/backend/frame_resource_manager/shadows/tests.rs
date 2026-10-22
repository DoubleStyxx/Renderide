#![expect(clippy::float_cmp, reason = "tests compare exact constructed values")]

use std::sync::Arc;

use hashbrown::HashMap;

use crate::backend::HostShadowQuality;
use crate::backend::frame_resource_manager::per_view_state::PreparedViewLights;
use crate::camera::ViewId;
use crate::gpu::{GpuLight, GpuLimits};
use crate::materials::RasterPipelineKind;
use crate::materials::host_data::MaterialPropertyLookupIds;
use crate::shared::{LightType, ShadowCastMode};
use crate::world_mesh::draw_prep::WorldMeshDrawCollection;
use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};
use crate::world_mesh::{
    PrefetchedWorldMeshViewDraws, WorldMeshDrawItem, WorldMeshDrawPlan, WorldMeshPhase,
};
use glam::{Mat4, Vec3};

use super::{
    POINT_FACE_COUNT, light_type_u32, point_shadow_projection, shadow_view_count_for_light,
};

fn limits_with_shadow_format_usages<const N: usize>(
    features: [(wgpu::TextureFormat, wgpu::TextureUsages); N],
) -> GpuLimits {
    let mut format_features = HashMap::new();
    for (format, allowed_usages) in features {
        format_features.insert(
            format,
            wgpu::TextureFormatFeatures {
                allowed_usages,
                flags: wgpu::TextureFormatFeatureFlags::empty(),
            },
        );
    }
    GpuLimits::synthetic_for_tests(
        wgpu::Limits {
            max_texture_dimension_2d: 1024,
            max_texture_array_layers: 8,
            ..Default::default()
        },
        wgpu::Features::empty(),
        format_features,
    )
}

fn shadowed_light(light_type: LightType) -> GpuLight {
    GpuLight {
        position: [3.0, 4.0, 5.0],
        light_type: light_type_u32(light_type),
        shadow_type: 1,
        shadow_strength: 1.0,
        shadow_near_plane: 0.05,
        shadow_bias: 0.25,
        range: 8.0,
        spot_cos_half_angle: 0.5,
        ..GpuLight::default()
    }
}

fn pbs_draw(node_id: i32, shadow_cast_mode: ShadowCastMode) -> WorldMeshDrawItem {
    let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
        material_asset_id: 1,
        property_block: None,
        skinned: false,
        sorting_order: 0,
        mesh_asset_id: 1,
        node_id,
        slot_index: 0,
        collect_order: node_id.max(0) as usize,
        alpha_blended: false,
    });
    item.shadow_cast_mode = shadow_cast_mode;
    item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from("pbsmetallic_default"));
    item
}

fn prefetched_plan(items: Vec<WorldMeshDrawItem>) -> WorldMeshDrawPlan {
    WorldMeshDrawPlan::Prefetched(Box::new(PrefetchedWorldMeshViewDraws::new(
        WorldMeshDrawCollection {
            draws_pre_cull: items.len(),
            items,
            draws_culled: 0,
            draws_hi_z_culled: 0,
            visibility: Default::default(),
            arrangement: Default::default(),
        },
        None,
    )))
}

#[test]
fn point_lights_plan_six_shadow_views() {
    assert_eq!(
        shadow_view_count_for_light(
            light_type_u32(LightType::Point),
            HostShadowQuality::default()
        ),
        POINT_FACE_COUNT
    );
}

#[test]
fn directional_lights_use_host_cascade_count() {
    let quality = HostShadowQuality {
        cascade_count: 2,
        ..HostShadowQuality::default()
    };
    assert_eq!(
        shadow_view_count_for_light(light_type_u32(LightType::Directional), quality),
        2
    );
}

#[test]
fn clear_assignment_removes_shadow_view_link() {
    let mut light = GpuLight {
        shadow_view_start: 4,
        shadow_view_count: 2,
        shadow_flags: 7,
        ..GpuLight::default()
    };
    super::clear_light_shadow_assignment(&mut light);
    assert_eq!(light.shadow_view_start, 0);
    assert_eq!(light.shadow_view_count, 0);
    assert_eq!(light.shadow_flags, 0);
}

#[test]
fn point_shadow_faces_have_distinct_projection_matrices() {
    let light = GpuLight {
        range: 8.0,
        shadow_near_plane: 0.05,
        ..GpuLight::default()
    };
    let position = Vec3::new(1.0, 2.0, 3.0);
    let mut seen = Vec::new();
    for face in 0..POINT_FACE_COUNT {
        let matrix = point_shadow_projection(&light, position, face).to_cols_array();
        assert!(
            !seen.iter().any(|existing| existing == &matrix),
            "point shadow face {face} reused a previous projection"
        );
        seen.push(matrix);
    }
}

#[test]
fn point_shadow_faces_share_caster_set_slab_range() {
    let mut manager = super::FrameResourceManager::new();
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(GpuLight {
            position: [3.0, 4.0, 5.0],
            light_type: light_type_u32(LightType::Point),
            shadow_type: 1,
            shadow_strength: 1.0,
            shadow_near_plane: 0.05,
            shadow_bias: 0.25,
            range: 8.0,
            ..GpuLight::default()
        });

    let mut first = dummy_world_mesh_draw_item(DummyDrawItemSpec {
        material_asset_id: 1,
        property_block: None,
        skinned: false,
        sorting_order: 0,
        mesh_asset_id: 1,
        node_id: 1,
        slot_index: 0,
        collect_order: 0,
        alpha_blended: false,
    });
    first.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from("pbsmetallic_default"));
    let mut second = first.clone();
    second.node_id = 2;
    second.collect_order = 1;

    let draw_plan = WorldMeshDrawPlan::Prefetched(Box::new(PrefetchedWorldMeshViewDraws::new(
        WorldMeshDrawCollection {
            items: vec![first, second],
            draws_pre_cull: 2,
            draws_culled: 0,
            draws_hi_z_culled: 0,
            visibility: Default::default(),
            arrangement: Default::default(),
        },
        None,
    )));
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );

    let plan = manager.shadow_frame_plan();
    assert_eq!(plan.render_views.len(), POINT_FACE_COUNT as usize);
    assert_eq!(plan.caster_sets.len(), 1);
    assert_eq!(plan.requested_draw_slots, 2);

    let caster_set = &plan.caster_sets[0];
    assert_eq!(caster_set.slab_slot_offset, 0);
    assert_eq!(caster_set.draws.len(), 2);
    assert_eq!(caster_set.instance_plan.slab_layout.len(), 2);

    for view in &plan.render_views {
        assert_eq!(view.kind, crate::gpu::SHADOW_VIEW_KIND_POINT);
        assert_eq!(view.light_position, Vec3::new(3.0, 4.0, 5.0));
        assert_eq!(view.light_range, 8.0);
        assert_eq!(view.shadow_bias, 0.25);
        assert_eq!(view.caster_set_index, 0);
        assert_eq!(view.groups(WorldMeshPhase::ForwardOpaque).len(), 1);
        assert_eq!(
            view.groups(WorldMeshPhase::ForwardOpaque)[0].instance_range,
            0..2
        );
    }
}

#[test]
fn shadow_planning_excludes_shadow_cast_mode_off_draws() {
    let mut manager = super::FrameResourceManager::new();
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(shadowed_light(LightType::Spot));
    let draw_plan = prefetched_plan(vec![
        pbs_draw(1, ShadowCastMode::Off),
        pbs_draw(2, ShadowCastMode::On),
        pbs_draw(3, ShadowCastMode::ShadowOnly),
    ]);

    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );

    let plan = manager.shadow_frame_plan();
    assert_eq!(plan.render_views.len(), 1);
    assert_eq!(plan.caster_sets.len(), 1);
    assert_eq!(plan.render_views[0].caster_set_index, 0);
    let nodes = plan.caster_sets[0]
        .draws
        .iter()
        .map(|item| item.node_id)
        .collect::<Vec<_>>();
    assert_eq!(nodes, vec![2, 3]);
    assert_eq!(plan.requested_draw_slots, 2);
}

#[test]
fn shadow_caster_plan_merges_forward_material_state_changes() {
    let first = pbs_draw(1, ShadowCastMode::On);
    let mut second = pbs_draw(2, ShadowCastMode::On);
    second.batch_key.material_asset_id = 44;
    second.batch_key.shader_asset_id = 55;
    second.batch_key.property_block_slot0 = Some(66);

    let mut scratch = super::ShadowCasterPlanScratch::default();
    let plan = super::build_shadow_caster_plan(&[first, second], true, &mut scratch);

    assert_eq!(plan.slab_layout, vec![0, 1]);
    assert_eq!(plan.phase_len(WorldMeshPhase::ForwardOpaque), 1);
    assert_eq!(
        plan.phase(WorldMeshPhase::ForwardOpaque)[0].instance_range,
        0..2
    );
}

#[test]
fn shadow_caster_plan_keeps_deformed_draws_singleton() {
    let mut first = pbs_draw(1, ShadowCastMode::On);
    first.world_space_deformed = true;
    let mut second = pbs_draw(2, ShadowCastMode::On);
    second.blendshape_deformed = true;

    let mut scratch = super::ShadowCasterPlanScratch::default();
    let plan = super::build_shadow_caster_plan(&[first, second], true, &mut scratch);

    assert_eq!(plan.slab_layout, vec![0, 1]);
    assert_eq!(plan.phase_len(WorldMeshPhase::ForwardOpaque), 2);
    assert_eq!(
        plan.phase(WorldMeshPhase::ForwardOpaque)[0].instance_range,
        0..1
    );
    assert_eq!(
        plan.phase(WorldMeshPhase::ForwardOpaque)[1].instance_range,
        1..2
    );
}

#[test]
fn shadow_caster_plan_keeps_downlevel_draws_singleton() {
    let first = pbs_draw(1, ShadowCastMode::On);
    let second = pbs_draw(2, ShadowCastMode::On);

    let mut scratch = super::ShadowCasterPlanScratch::default();
    let plan = super::build_shadow_caster_plan(&[first, second], false, &mut scratch);

    assert_eq!(plan.slab_layout, vec![0, 1]);
    assert_eq!(plan.phase_len(WorldMeshPhase::ForwardOpaque), 2);
    assert!(
        plan.phase(WorldMeshPhase::ForwardOpaque)
            .iter()
            .all(|group| group.instance_range.end - group.instance_range.start == 1)
    );
}

#[test]
fn shadow_planning_uses_per_light_resolution_and_metadata_bias() {
    let mut manager = super::FrameResourceManager::new();
    let lights = &mut manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights;
    let mut low_resolution = shadowed_light(LightType::Spot);
    low_resolution.shadow_map_resolution = 512;
    low_resolution.shadow_normal_bias = 2.0;
    lights.push(low_resolution);
    lights.push(shadowed_light(LightType::Spot));
    let draw_plan = prefetched_plan(vec![pbs_draw(1, ShadowCastMode::On)]);

    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );

    let plan = manager.shadow_frame_plan();
    assert_eq!(plan.render_views.len(), 2);
    assert_eq!(
        plan.requested_resolution,
        HostShadowQuality::default().tile_resolution
    );
    assert_eq!(plan.render_views[0].resolution, 512);
    assert_eq!(
        plan.render_views[1].resolution,
        HostShadowQuality::default().tile_resolution
    );
    assert_eq!(plan.metadata[0].params[1], 1.0 / 512.0);
    assert_eq!(plan.metadata[0].atlas_rect, [0.0, 0.0, 0.25, 0.25]);
    assert_eq!(plan.metadata[1].atlas_rect, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(plan.metadata[0].light_params[3], 0.25);
    assert!(plan.metadata[0].light_params[2] > 0.0);
}

#[test]
fn shadow_planning_requests_only_custom_resolution_when_all_lights_override() {
    let mut manager = super::FrameResourceManager::new();
    let mut light = shadowed_light(LightType::Spot);
    light.shadow_map_resolution = 512;
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(light);
    let draw_plan = prefetched_plan(vec![pbs_draw(1, ShadowCastMode::On)]);

    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );

    let plan = manager.shadow_frame_plan();
    assert_eq!(plan.requested_resolution, 512);
    assert_eq!(plan.render_views[0].resolution, 512);
    assert_eq!(plan.metadata[0].atlas_rect, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn shadow_planning_disables_when_depth_atlas_format_is_not_renderable() {
    let mut manager = super::FrameResourceManager::new();
    manager.limits = Some(Arc::new(limits_with_shadow_format_usages([
        (
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ),
        (
            wgpu::TextureFormat::Depth24Plus,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ),
        (
            wgpu::TextureFormat::Depth16Unorm,
            wgpu::TextureUsages::empty(),
        ),
    ])));
    let mut light = shadowed_light(LightType::Spot);
    light.shadow_view_start = 7;
    light.shadow_view_count = 3;
    light.shadow_flags = 5;
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(light);
    let draw_plan = prefetched_plan(vec![pbs_draw(1, ShadowCastMode::On)]);

    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );

    let plan = manager.shadow_frame_plan();
    assert!(plan.render_views.is_empty());
    assert!(plan.metadata.is_empty());
    assert_eq!(plan.requested_draw_slots, 0);
    assert_eq!(manager.shadow_resource_request(), None);

    let light = &manager.per_view_lights.get(ViewId::Main).unwrap().lights[0];
    assert_eq!(light.shadow_view_start, 0);
    assert_eq!(light.shadow_view_count, 0);
    assert_eq!(light.shadow_flags, 0);
}

fn positioned_caster(node_id: i32, position: Vec3) -> WorldMeshDrawItem {
    let mut item = pbs_draw(node_id, ShadowCastMode::On);
    item.mesh_asset_id = node_id;
    item.rigid_world_matrix = Some(Mat4::from_translation(position));
    let extent = Vec3::splat(0.05);
    item.world_aabb = Some((position - extent, position + extent));
    item
}

fn spot_shadow_manager(
    draw: WorldMeshDrawItem,
) -> (super::FrameResourceManager, WorldMeshDrawPlan) {
    let mut manager = super::FrameResourceManager::new();
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(shadowed_light(LightType::Spot));
    let plan = prefetched_plan(vec![draw]);
    (manager, plan)
}

#[test]
fn static_shadow_plan_is_reused_and_skips_redraw_after_encode() {
    let draw = positioned_caster(1, Vec3::new(1.0, 0.0, 0.0));
    let (mut manager, draw_plan) = spot_shadow_manager(draw.clone());
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    let first_draws = Arc::clone(&manager.shadow_frame_plan().caster_sets[0].draws);
    assert!(!manager.shadow_frame_plan().plan_reused);
    assert!(manager.shadow_frame_plan().maps_need_render);

    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(manager.shadow_frame_plan().maps_need_render);
    assert!(Arc::ptr_eq(
        &first_draws,
        &manager.shadow_frame_plan().caster_sets[0].draws
    ));

    manager.note_shadow_maps_rendered();
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(!manager.shadow_frame_plan().maps_need_render);
    assert_eq!(
        manager.per_view_lights.get(ViewId::Main).unwrap().lights[0].shadow_view_count,
        1
    );

    let reversed = prefetched_plan(vec![draw.clone(), {
        let mut other = draw.clone();
        other.node_id = 2;
        other.instance_id = crate::scene::MeshRendererInstanceId(99);
        other
    }]);
    // A pure reorder of the same caster still hits. A new caster misses below.
    let same_order = prefetched_plan(vec![draw]);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &same_order)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);

    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &reversed)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn shadow_plan_ignores_caster_order_and_light_color() {
    let first = positioned_caster(1, Vec3::new(0.0, 0.0, 1.0));
    let second = positioned_caster(2, Vec3::new(0.0, 1.0, 0.0));
    let (mut manager, _) = spot_shadow_manager(first.clone());
    let both = prefetched_plan(vec![first.clone(), second.clone()]);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &both)],
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let reversed = prefetched_plan(vec![second, first]);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &reversed)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(!manager.shadow_frame_plan().maps_need_render);

    let light = &mut manager
        .per_view_lights
        .get_mut(ViewId::Main)
        .unwrap()
        .lights[0];
    light.color = [0.2, 0.3, 0.4];
    light.intensity = 4.0;
    light.shadow_strength = 0.4;
    let both_again = prefetched_plan(vec![
        positioned_caster(1, Vec3::new(0.0, 0.0, 1.0)),
        positioned_caster(2, Vec3::new(0.0, 1.0, 0.0)),
    ]);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &both_again)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert_eq!(
        manager.per_view_lights.get(ViewId::Main).unwrap().lights[0].shadow_view_count,
        1
    );
}

#[test]
fn shadow_plan_misses_when_bias_matrix_or_strength_drop() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, draw_plan) = spot_shadow_manager(draw);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();

    manager
        .per_view_lights
        .get_mut(ViewId::Main)
        .unwrap()
        .lights[0]
        .shadow_bias = 0.9;
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
    manager.note_shadow_maps_rendered();

    let moved = prefetched_plan(vec![positioned_caster(1, Vec3::new(1.0, 0.0, 0.0))]);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &moved)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
    manager.note_shadow_maps_rendered();

    manager
        .per_view_lights
        .get_mut(ViewId::Main)
        .unwrap()
        .lights[0]
        .shadow_strength = 0.0;
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &moved)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().render_views.is_empty());
    assert_eq!(
        manager.per_view_lights.get(ViewId::Main).unwrap().lights[0].shadow_view_count,
        0
    );
}

#[test]
fn deformed_casters_reuse_the_plan_and_still_redraw() {
    let mut draw = positioned_caster(1, Vec3::ZERO);
    draw.world_space_deformed = true;
    let (mut manager, draw_plan) = spot_shadow_manager(draw);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(manager.shadow_frame_plan().maps_need_render);
}

#[test]
fn point_shadow_influence_keeps_near_casters_and_drops_distant_ones() {
    let mut manager = super::FrameResourceManager::new();
    let mut light = shadowed_light(LightType::Point);
    light.position = [0.0, 0.0, 0.0];
    light.range = 4.0;
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(light);
    let draw_plan = prefetched_plan(vec![
        positioned_caster(1, Vec3::new(2.0, 0.0, 0.0)),
        positioned_caster(2, Vec3::new(40.0, 0.0, 0.0)),
    ]);
    manager.prepare_shadow_frame_for_views(
        HostShadowQuality::default(),
        [(ViewId::Main, &draw_plan)],
        &super::ZeroShadowContentRevisions,
    );

    let plan = manager.shadow_frame_plan();
    assert_eq!(plan.render_views.len(), POINT_FACE_COUNT as usize);
    assert!(
        !plan.render_views[0]
            .groups(WorldMeshPhase::ForwardOpaque)
            .is_empty()
    );
    assert!(
        plan.render_views[1..]
            .iter()
            .all(|view| view.groups(WorldMeshPhase::ForwardOpaque).is_empty())
    );
    let visible = &plan.render_views[0].groups(WorldMeshPhase::ForwardOpaque)[0];
    let members = &plan.caster_sets[0].instance_plan.slab_layout
        [visible.instance_range.start as usize..visible.instance_range.end as usize];
    assert_eq!(members, &[0]);
}

#[test]
fn point_influence_radius_covers_cube_far_corners() {
    let mut light = shadowed_light(LightType::Point);
    light.range = 8.0;
    light.shadow_near_plane = 0.05;
    let radius_sq = super::range_cull::influence_radius_sq(&light).unwrap();
    let corner = 8.0 * 3.0_f32.sqrt();
    assert!(corner * corner < radius_sq);
    let outside = 3.1_f32.sqrt() * 8.0 + 0.05;
    assert!(outside * outside > radius_sq);

    light.light_type = light_type_u32(LightType::Directional);
    assert!(super::range_cull::influence_radius_sq(&light).is_none());
}

struct RevisionMap {
    mesh: HashMap<i32, u64>,
    property: u64,
}

impl super::ShadowContentRevisions for RevisionMap {
    fn mesh_upload_generation(&self, mesh_asset_id: i32) -> u64 {
        self.mesh.get(&mesh_asset_id).copied().unwrap_or(0)
    }

    fn material_property_revision(&self, _ids: MaterialPropertyLookupIds) -> u64 {
        self.property
    }
}

fn prepare_with<R: super::ShadowContentRevisions>(
    manager: &mut super::FrameResourceManager,
    quality: HostShadowQuality,
    plan: &WorldMeshDrawPlan,
    revisions: &R,
) {
    manager.prepare_shadow_frame_for_views(quality, [(ViewId::Main, plan)], revisions);
}

fn note_then_edit_light(edit: impl FnOnce(&mut GpuLight)) {
    let draw = positioned_caster(1, Vec3::new(0.0, 0.0, -1.0));
    let (mut manager, plan) = spot_shadow_manager(draw);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    edit(
        &mut manager
            .per_view_lights
            .get_mut(ViewId::Main)
            .unwrap()
            .lights[0],
    );
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn shadow_plan_misses_when_light_position_changes() {
    note_then_edit_light(|light| light.position = [9.0, 1.0, 2.0]);
}

#[test]
fn shadow_plan_misses_when_light_range_changes() {
    note_then_edit_light(|light| light.range = 2.5);
}

#[test]
fn shadow_plan_misses_when_spot_angle_changes() {
    note_then_edit_light(|light| light.spot_cos_half_angle = 0.9);
}

#[test]
fn shadow_plan_misses_when_light_type_changes() {
    note_then_edit_light(|light| light.light_type = light_type_u32(LightType::Point));
}

#[test]
fn shadow_plan_misses_when_a_caster_is_removed() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let empty = prefetched_plan(Vec::new());
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &empty,
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
    assert!(manager.shadow_frame_plan().render_views.is_empty());
}

#[test]
fn shadow_plan_misses_when_cast_mode_pipeline_or_material_changes() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw.clone());
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();

    let mut off = draw.clone();
    off.shadow_cast_mode = ShadowCastMode::Off;
    let off_plan = prefetched_plan(vec![off]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &off_plan,
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    let again = prefetched_plan(vec![draw.clone()]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &again,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let mut other_pipeline = draw.clone();
    other_pipeline.batch_key.pipeline =
        RasterPipelineKind::EmbeddedStem(Arc::from("xstoon2.0_default"));
    let pipeline_plan = prefetched_plan(vec![other_pipeline]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &pipeline_plan,
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &again,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let mut other_material = draw;
    other_material.batch_key.material_asset_id = 44;
    other_material.lookup_ids.material_asset_id = 44;
    let material_plan = prefetched_plan(vec![other_material]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &material_plan,
        &super::ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn shadow_plan_misses_when_property_revision_or_mesh_generation_changes() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw);
    let base = RevisionMap {
        mesh: HashMap::new(),
        property: 1,
    };
    prepare_with(&mut manager, HostShadowQuality::default(), &plan, &base);
    manager.note_shadow_maps_rendered();
    let property_edit = RevisionMap {
        mesh: HashMap::new(),
        property: 2,
    };
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &property_edit,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &property_edit,
    );
    manager.note_shadow_maps_rendered();
    let mut mesh = HashMap::new();
    mesh.insert(1, 7);
    let uploaded = RevisionMap { mesh, property: 2 };
    prepare_with(&mut manager, HostShadowQuality::default(), &plan, &uploaded);
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn shadow_plan_misses_when_quality_tier_changes() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let low = HostShadowQuality {
        tile_resolution: 512,
        ..HostShadowQuality::default()
    };
    prepare_with(&mut manager, low, &plan, &super::ZeroShadowContentRevisions);
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn mark_shadow_maps_for_render_forces_a_redraw_of_the_cached_plan() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(!manager.shadow_frame_plan().maps_need_render);
    manager.mark_shadow_maps_for_render();
    assert!(manager.shadow_frame_plan().maps_need_render);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(manager.shadow_frame_plan().maps_need_render);
}

fn spot_groups_for(position: Vec3) -> usize {
    let mut light = shadowed_light(LightType::Spot);
    light.position = [0.0, 0.0, 0.0];
    light.direction = [0.0, 0.0, -1.0];
    light.range = 8.0;
    light.shadow_near_plane = 0.05;
    light.spot_cos_half_angle = 0.5;
    let mut manager = super::FrameResourceManager::new();
    manager
        .per_view_lights
        .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
        .lights
        .push(light);
    let plan = prefetched_plan(vec![positioned_caster(1, position)]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &super::ZeroShadowContentRevisions,
    );
    manager.shadow_frame_plan().render_views[0]
        .groups(WorldMeshPhase::ForwardOpaque)
        .len()
}

#[test]
fn spot_range_cull_keeps_the_cone_interior_and_far_corner() {
    assert_eq!(spot_groups_for(Vec3::new(0.0, 0.0, -2.0)), 1);
    assert_eq!(spot_groups_for(Vec3::new(0.0, 0.0, -40.0)), 0);
    let cos_half = 0.5_f32;
    let tan_half = (1.0 - cos_half * cos_half).sqrt() / cos_half;
    let far = 8.0 * 0.95;
    let corner = Vec3::new(far * tan_half * 0.85, 0.0, -far);
    assert_eq!(spot_groups_for(corner), 1);
}

struct Lcg(u64);

impl Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (self.0 >> 32) as u32
    }

    fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
}

fn assert_shadow_plans_match(
    left: &super::FrameResourceManager,
    right: &super::FrameResourceManager,
) {
    let a = left.shadow_frame_plan();
    let b = right.shadow_frame_plan();
    assert_eq!(a.render_views.len(), b.render_views.len());
    assert_eq!(a.requested_resolution, b.requested_resolution);
    assert_eq!(a.requested_layers, b.requested_layers);
    assert_eq!(a.metadata.len(), b.metadata.len());
    for (view_a, view_b) in a.render_views.iter().zip(b.render_views.iter()) {
        assert_eq!(view_a.kind, view_b.kind);
        assert_eq!(view_a.resolution, view_b.resolution);
        assert_eq!(
            view_a.view_proj.to_cols_array(),
            view_b.view_proj.to_cols_array()
        );
        let groups_a = view_a.groups(WorldMeshPhase::ForwardOpaque);
        let groups_b = view_b.groups(WorldMeshPhase::ForwardOpaque);
        assert_eq!(groups_a.len(), groups_b.len());
        for (group_a, group_b) in groups_a.iter().zip(groups_b) {
            assert_eq!(group_a.instance_range, group_b.instance_range);
            assert_eq!(
                group_a.representative_draw_idx,
                group_b.representative_draw_idx
            );
        }
    }
    let lights_a = &left.per_view_lights.get(ViewId::Main).unwrap().lights;
    let lights_b = &right.per_view_lights.get(ViewId::Main).unwrap().lights;
    for (light_a, light_b) in lights_a.iter().zip(lights_b) {
        assert_eq!(light_a.shadow_view_start, light_b.shadow_view_start);
        assert_eq!(light_a.shadow_view_count, light_b.shadow_view_count);
    }
}

#[test]
fn cache_hit_matches_a_full_rebuild() {
    let mut rng = Lcg(0x5348_4144_4F57);
    for _trial in 0..8 {
        let light_type = match rng.next_u32() % 3 {
            0 => LightType::Point,
            1 => LightType::Directional,
            _ => LightType::Spot,
        };
        let mut light = shadowed_light(light_type);
        light.position = [rng.unit() * 2.0, rng.unit(), rng.unit() * 2.0];
        light.direction = [0.0, -0.2, -1.0];
        light.range = 6.0 + rng.unit() * 4.0;
        light.spot_cos_half_angle = 0.35 + rng.unit() * 0.4;
        let mut casters = Vec::new();
        for index in 0..4i32 {
            let position = Vec3::new(rng.unit() - 0.5, rng.unit() * 0.4, -rng.unit());
            casters.push(positioned_caster(index, position));
        }
        let plan = prefetched_plan(casters);
        let mut cached = super::FrameResourceManager::new();
        let mut fresh = super::FrameResourceManager::new();
        for manager in [&mut cached, &mut fresh] {
            manager
                .per_view_lights
                .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
                .lights
                .push(light);
        }
        prepare_with(
            &mut cached,
            HostShadowQuality::default(),
            &plan,
            &super::ZeroShadowContentRevisions,
        );
        cached.note_shadow_maps_rendered();
        prepare_with(
            &mut cached,
            HostShadowQuality::default(),
            &plan,
            &super::ZeroShadowContentRevisions,
        );
        prepare_with(
            &mut fresh,
            HostShadowQuality::default(),
            &plan,
            &super::ZeroShadowContentRevisions,
        );
        assert!(cached.shadow_frame_plan().plan_reused);
        assert!(!fresh.shadow_frame_plan().plan_reused);
        assert_shadow_plans_match(&cached, &fresh);
        #[cfg(debug_assertions)]
        {
            assert_eq!(
                cached.shadow_frame_plan().render_views.len(),
                fresh.shadow_frame_plan().render_views.len()
            );
        }
    }
}
