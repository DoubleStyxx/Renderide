use std::sync::Arc;

use glam::Vec3;
use hashbrown::HashMap;

use crate::backend::frame_resource_manager::per_view_state::PreparedViewLights;
use crate::backend::{AssetTransferQueue, HostShadowQuality, ShadowContentRevisionSources};
use crate::camera::ViewId;
use crate::gpu::GpuLight;
use crate::materials::host_data::{MaterialPropertyStore, MaterialPropertyValue};
use crate::materials::{
    MaterialCullOverride, RasterFrontFace, RasterPipelineKind, RasterPrimitiveTopology,
};
use crate::shared::{LightType, ShadowCastMode};
use crate::world_mesh::{WorldMeshDrawPlan, WorldMeshPhase};

use super::super::{
    FrameResourceManager, ShadowContentRevisions, ZeroShadowContentRevisions, light_type_u32,
};
use super::{positioned_caster, prefetched_plan, shadowed_light, spot_shadow_manager};

struct RevisionMap {
    mesh: HashMap<i32, u64>,
}

impl ShadowContentRevisions for RevisionMap {
    fn mesh_upload_generation(&self, mesh_asset_id: i32) -> u64 {
        self.mesh.get(&mesh_asset_id).copied().unwrap_or(0)
    }
}

fn prepare_with<R: ShadowContentRevisions>(
    manager: &mut FrameResourceManager,
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
        &ZeroShadowContentRevisions,
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
        &ZeroShadowContentRevisions,
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
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let empty = prefetched_plan(Vec::new());
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &empty,
        &ZeroShadowContentRevisions,
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
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();

    let mut off = draw.clone();
    off.shadow_cast_mode = ShadowCastMode::Off;
    let off_plan = prefetched_plan(vec![off]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &off_plan,
        &ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    let again = prefetched_plan(vec![draw.clone()]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &again,
        &ZeroShadowContentRevisions,
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
        &ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &again,
        &ZeroShadowContentRevisions,
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
        &ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn shadow_plan_misses_when_mesh_generation_changes() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw);
    let base = RevisionMap {
        mesh: HashMap::new(),
    };
    prepare_with(&mut manager, HostShadowQuality::default(), &plan, &base);
    manager.note_shadow_maps_rendered();
    let mut mesh = HashMap::new();
    mesh.insert(1, 7);
    let uploaded = RevisionMap { mesh };
    prepare_with(&mut manager, HostShadowQuality::default(), &plan, &uploaded);
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn animated_non_policy_material_property_keeps_the_cache_hit() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let ids = draw.lookup_ids;
    let (mut manager, plan) = spot_shadow_manager(draw);
    let mut transfers = AssetTransferQueue::new();
    let mut materials = MaterialPropertyStore::new();
    // A color/roughness float. Caster policy does not read the property store.
    materials.set_material(ids.material_asset_id, 7, MaterialPropertyValue::Float(0.2));
    let before = materials.mutation_generation(ids);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &ShadowContentRevisionSources::new(&transfers),
    );
    manager.note_shadow_maps_rendered();

    materials.set_material(ids.material_asset_id, 7, MaterialPropertyValue::Float(0.85));
    let after = materials.mutation_generation(ids);
    assert_ne!(before, after);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &ShadowContentRevisionSources::new(&transfers),
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(!manager.shadow_frame_plan().maps_need_render);

    transfers.begin_mesh_upload_generation(1);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &ShadowContentRevisionSources::new(&transfers),
    );
    assert!(!manager.shadow_frame_plan().plan_reused);
}

#[test]
fn shadow_plan_misses_when_front_face_topology_or_cull_changes() {
    let draw = positioned_caster(1, Vec3::ZERO);
    let (mut manager, plan) = spot_shadow_manager(draw.clone());
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();

    let mut flipped_face = draw.clone();
    flipped_face.batch_key.front_face = RasterFrontFace::CounterClockwise;
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &prefetched_plan(vec![flipped_face]),
        &ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    let again = prefetched_plan(vec![draw.clone()]);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &again,
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let mut points = draw.clone();
    points.batch_key.primitive_topology = RasterPrimitiveTopology::PointList;
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &prefetched_plan(vec![points]),
        &ZeroShadowContentRevisions,
    );
    assert!(!manager.shadow_frame_plan().plan_reused);

    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &again,
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let mut front_cull = draw;
    front_cull.batch_key.render_state.cull_override = MaterialCullOverride::Front;
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &prefetched_plan(vec![front_cull]),
        &ZeroShadowContentRevisions,
    );
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
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    let low = HostShadowQuality {
        tile_resolution: 512,
        ..HostShadowQuality::default()
    };
    prepare_with(&mut manager, low, &plan, &ZeroShadowContentRevisions);
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
        &ZeroShadowContentRevisions,
    );
    manager.note_shadow_maps_rendered();
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &ZeroShadowContentRevisions,
    );
    assert!(manager.shadow_frame_plan().plan_reused);
    assert!(!manager.shadow_frame_plan().maps_need_render);
    manager.mark_shadow_maps_for_render();
    assert!(manager.shadow_frame_plan().maps_need_render);
    prepare_with(
        &mut manager,
        HostShadowQuality::default(),
        &plan,
        &ZeroShadowContentRevisions,
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
    let mut manager = FrameResourceManager::new();
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
        &ZeroShadowContentRevisions,
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

fn assert_shadow_plans_match(left: &FrameResourceManager, right: &FrameResourceManager) {
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
        let mut cached = FrameResourceManager::new();
        let mut fresh = FrameResourceManager::new();
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
            &ZeroShadowContentRevisions,
        );
        cached.note_shadow_maps_rendered();
        prepare_with(
            &mut cached,
            HostShadowQuality::default(),
            &plan,
            &ZeroShadowContentRevisions,
        );
        prepare_with(
            &mut fresh,
            HostShadowQuality::default(),
            &plan,
            &ZeroShadowContentRevisions,
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
