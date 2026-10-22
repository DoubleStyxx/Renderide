//! Synthetic many-light shadow planning workload for `renderide-bench`.
//!
//! The scene is a static grid of rigid casters and shadowed point lights, which is the steady
//! state reported for worlds such as the Lonely Lightkeep.

use glam::{Mat4, Vec3};

use crate::backend::HostShadowQuality;
use crate::backend::frame_resource_manager::per_view_state::PreparedViewLights;
use crate::camera::ViewId;
use crate::gpu::GpuLight;
use crate::shared::LightType;
use crate::world_mesh::draw_prep::WorldMeshDrawCollection;
use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};
use crate::world_mesh::{PrefetchedWorldMeshViewDraws, WorldMeshDrawItem, WorldMeshDrawPlan};

use super::super::manager::FrameResourceManager;
use super::light_type_u32;

/// Rigid shadow casters in [`ShadowChurnWorkload::standard`].
pub const SHADOW_CHURN_CASTERS: usize = 4_096;
/// Shadowed point lights in [`ShadowChurnWorkload::standard`].
pub const SHADOW_CHURN_LIGHTS: usize = 30;

/// Owned inputs for repeated shadow-frame planning.
pub struct ShadowChurnWorkload {
    manager: FrameResourceManager,
    quality: HostShadowQuality,
    plan: WorldMeshDrawPlan,
}

impl ShadowChurnWorkload {
    /// Builds a static grid: one rigid caster per cell and thirty ranged point lights.
    #[must_use]
    pub fn standard() -> Self {
        let mut manager = FrameResourceManager::new();
        manager
            .per_view_lights
            .get_or_insert_with(ViewId::Main, PreparedViewLights::default)
            .lights = churn_lights();
        Self {
            manager,
            quality: churn_quality(),
            plan: draw_plan(churn_casters()),
        }
    }

    /// Plans one shadow frame and returns the view count mixed with the reuse bit.
    ///
    /// The reuse bit keeps the cached path observable to criterion without changing the view count
    /// the planner reports.
    pub fn prepare(&mut self) -> u64 {
        self.manager.prepare_shadow_frame_for_views(
            self.quality,
            [(ViewId::Main, &self.plan)],
            &super::ZeroShadowContentRevisions,
        );
        let planned = self.manager.shadow_frame_plan();
        let views = planned.render_views.len() as u64;
        let reused = u64::from(planned.plan_reused);
        // Stands in for a successful GPU submit. The measured work is the CPU planner on the
        // following cache hit, not the shadow atlas pass.
        self.manager.note_shadow_maps_rendered();
        views + (reused << 32)
    }
}

fn churn_quality() -> HostShadowQuality {
    HostShadowQuality {
        per_pixel_lights: 32,
        cascade_count: 1,
        tile_resolution: 1024,
        shadow_distance: 50.0,
    }
}

fn churn_lights() -> Vec<GpuLight> {
    (0..SHADOW_CHURN_LIGHTS)
        .map(|index| {
            let x = (index % 5) as f32 * 9.0 + 1.0;
            let y = (index / 5) as f32 * 9.0 + 1.0;
            point_light([x, y, 12.0], 7.0)
        })
        .collect()
}

fn point_light(position: [f32; 3], range: f32) -> GpuLight {
    GpuLight {
        position,
        light_type: light_type_u32(LightType::Point),
        shadow_type: 1,
        shadow_strength: 1.0,
        shadow_near_plane: 0.05,
        shadow_bias: 0.05,
        range,
        ..GpuLight::default()
    }
}

fn churn_casters() -> Vec<WorldMeshDrawItem> {
    let side = 16i32;
    let mut items = Vec::with_capacity(SHADOW_CHURN_CASTERS);
    for index in 0..SHADOW_CHURN_CASTERS {
        let i = index as i32;
        let position = Vec3::new(
            (i % side) as f32 * 3.0,
            ((i / side) % side) as f32 * 3.0,
            (i / (side * side)) as f32 * 3.0,
        );
        items.push(caster_at(i, position));
    }
    items
}

fn caster_at(index: i32, position: Vec3) -> WorldMeshDrawItem {
    let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
        material_asset_id: 1,
        property_block: None,
        skinned: false,
        sorting_order: 0,
        mesh_asset_id: index,
        node_id: index,
        slot_index: 0,
        collect_order: index.max(0) as usize,
        alpha_blended: false,
    });
    item.rigid_world_matrix = Some(Mat4::from_translation(position));
    let extent = Vec3::splat(0.4);
    item.world_aabb = Some((position - extent, position + extent));
    item
}

fn draw_plan(items: Vec<WorldMeshDrawItem>) -> WorldMeshDrawPlan {
    let draws_pre_cull = items.len();
    WorldMeshDrawPlan::Prefetched(Box::new(PrefetchedWorldMeshViewDraws::new(
        WorldMeshDrawCollection {
            items,
            draws_pre_cull,
            draws_culled: 0,
            draws_hi_z_culled: 0,
            visibility: Default::default(),
            arrangement: Default::default(),
        },
        None,
    )))
}
