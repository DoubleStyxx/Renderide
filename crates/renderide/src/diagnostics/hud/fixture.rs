//! Fixed HUD snapshots for the steady-state allocation test.
//!
//! Numbers, viewport, scale, and font do not change between frames. Layout ini
//! saving is off. The GPU pass is not part of this fixture: [`super::ui::HudUi`]
//! builds the draw list on the CPU.

use std::sync::Arc;

use wgpu::wgt::{AllocationReport, AllocatorReport, MemoryBlockReport};

use crate::config::{DebugHudMainTab, DebugHudMainTabVisibility, RendererSettings};
use crate::diagnostics::snapshots::frame_diagnostics::shader_routes::ShaderRouteRow;
use crate::diagnostics::snapshots::frame_diagnostics::{
    GpuAllocatorHud, GpuAllocatorReportHud, HostCpuMemoryHud,
};
use crate::diagnostics::snapshots::frame_timing::{
    FrameTimingHistoryStats, FrameTimingOnePercentStats,
};
use crate::diagnostics::snapshots::{
    FrameDiagnosticsSnapshot, FrameTimingHudSnapshot, RendererInfoSnapshot,
};
use crate::frontend::InitState;
use crate::materials::{MaterialBlendMode, RasterPipelineKind};
use crate::profiling::{GpuPassEntry, GpuProfilerFrameStats, GpuProfilerSnapshot};
use crate::world_mesh::{TransparentMaterialClass, WorldMeshDrawStateRow};

use super::ui::{self, HudUi};

/// Which HUD surface the allocation fixture builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudMeasureView {
    /// `imgui_visible = false`. No windows are built.
    Hidden,
    /// Main debug window, **Stats** tab, plus the default open windows.
    Stats,
    /// Main debug window, **Shader routes** tab, plus the default open windows.
    ShaderRoutes,
    /// Main debug window, **Draw state** tab, plus the default open windows.
    DrawState,
    /// Main debug window, **GPU memory** tab, plus the default open windows.
    GpuMemory,
    /// Main debug window, **GPU passes** tab, plus the default open windows.
    GpuPasses,
    /// Default open HUD: frame timing, feedback, and renderer config. Main debug is off.
    RendererConfig,
}

const RAM_TOTAL: u64 = 16 * 1024 * 1024 * 1024;
const RAM_USED: u64 = 8 * 1024 * 1024 * 1024;
const PROCESS_RAM: u64 = 512 * 1024 * 1024;
const ALLOCATED: u64 = 4096 + 1024 * 1024;
const RESERVED: u64 = 2 * 1024 * 1024;

pub(super) fn settings_for(view: HudMeasureView) -> RendererSettings {
    let mut settings = RendererSettings::default();
    settings.debug.hud.ui_scale = ui::MEASURE_UI_SCALE;
    settings.debug.hud.persist_layout = false;
    settings.debug.hud.imgui_visible = !matches!(view, HudMeasureView::Hidden);
    let main = main_tab(view);
    settings.debug.debug_hud_enabled = main.is_some();
    if let Some(tab) = main {
        settings.debug.hud.main_tab = tab;
        settings.debug.hud.main_tabs = only_tab(tab);
    }
    settings
}

pub(super) fn install(ui: &mut HudUi, view: HudMeasureView) {
    ui.set_frame_timing(frame_timing());
    ui.set_snapshot(renderer_info());
    if let Some(tab) = main_tab(view) {
        ui.set_frame_diagnostics(frame_diagnostics(tab));
    }
    ui.set_gpu_profiler_snapshot(gpu_profiler());
}

fn main_tab(view: HudMeasureView) -> Option<DebugHudMainTab> {
    match view {
        HudMeasureView::Hidden | HudMeasureView::RendererConfig => None,
        HudMeasureView::Stats => Some(DebugHudMainTab::Stats),
        HudMeasureView::ShaderRoutes => Some(DebugHudMainTab::ShaderRoutes),
        HudMeasureView::DrawState => Some(DebugHudMainTab::DrawState),
        HudMeasureView::GpuMemory => Some(DebugHudMainTab::GpuMemory),
        HudMeasureView::GpuPasses => Some(DebugHudMainTab::GpuPasses),
    }
}

fn only_tab(tab: DebugHudMainTab) -> DebugHudMainTabVisibility {
    DebugHudMainTabVisibility {
        stats: tab == DebugHudMainTab::Stats,
        shader_routes: tab == DebugHudMainTab::ShaderRoutes,
        draw_state: tab == DebugHudMainTab::DrawState,
        gpu_memory: tab == DebugHudMainTab::GpuMemory,
        gpu_passes: tab == DebugHudMainTab::GpuPasses,
    }
}

fn frame_timing() -> FrameTimingHudSnapshot {
    let pair = |low: f64, high: f64| FrameTimingOnePercentStats {
        low: Some(low),
        high: Some(high),
    };
    FrameTimingHudSnapshot {
        wall_frame_time_ms_smoothed: 16.67,
        cpu_frame_ms_smoothed: Some(8.25),
        gpu_frame_ms_smoothed: Some(6.50),
        host_frame_ms_smoothed: Some(1.25),
        frame_time_history: vec![16.67_f32; 32],
        history_stats: FrameTimingHistoryStats {
            fps: pair(58.0, 60.0),
            frame_ms: pair(16.9, 16.4),
            cpu_ms: pair(9.0, 7.5),
            gpu_ms: pair(7.1, 6.0),
            host_ms: pair(1.6, 1.0),
        },
        host_ram_total_bytes: RAM_TOTAL,
        host_ram_used_bytes: RAM_USED,
        process_ram_bytes: Some(PROCESS_RAM),
        gpu_allocator_allocated_bytes: Some(ALLOCATED),
        gpu_allocator_reserved_bytes: Some(RESERVED),
    }
}

fn renderer_info() -> RendererInfoSnapshot {
    RendererInfoSnapshot {
        ipc_connected: true,
        init_state: InitState::Finalized,
        last_frame_index: 1200,
        adapter_name: "Fixture GPU".to_string(),
        adapter_backend: wgpu::Backend::Vulkan,
        adapter_device_type: wgpu::DeviceType::DiscreteGpu,
        adapter_driver: "fixture".to_string(),
        adapter_driver_info: "fixture 1.0".to_string(),
        surface_format: wgpu::TextureFormat::Bgra8Unorm,
        viewport_px: ui::MEASURE_VIEWPORT_PX,
        present_mode: wgpu::PresentMode::Fifo,
        render_space_count: 2,
        mesh_renderable_count: 128,
        resident_mesh_count: 64,
        resident_texture_count: 32,
        resident_render_texture_count: 4,
        material_property_slots: 16,
        property_block_slots: 8,
        material_shader_bindings: 12,
        material_shader_graph: Default::default(),
        material_pipeline_cache: Default::default(),
        embedded_material_bind_cache: Default::default(),
        frame_graph_pass_count: 18,
        frame_graph_registered_pass_count: 20,
        frame_graph_topo_levels: 6,
        frame_graph_culled_pass_count: 2,
        frame_graph_compile_skipped_pass_count: 0,
        frame_graph_attachment_resolve_count: 3,
        frame_graph_transient_store_count: 4,
        frame_graph_transient_discard_count: 1,
        frame_graph_estimated_bandwidth_bytes: 8 * 1024 * 1024,
        gpu_light_count: 4,
        signed_scene_color_active: false,
        upload_arena: Default::default(),
        gpu_max_texture_dim_2d: 8192,
        gpu_max_buffer_size: 1 << 30,
        gpu_max_storage_binding: 1 << 27,
        gpu_supports_base_instance: true,
        gpu_supports_multiview: true,
        gpu_supports_float32_filterable: false,
        gpu_texture_compression_features: wgpu::Features::empty(),
        msaa_requested_samples: 4,
        msaa_effective_samples: 4,
        msaa_max_samples: 4,
        msaa_effective_samples_stereo: 1,
        msaa_max_samples_stereo: 1,
    }
}

fn frame_diagnostics(tab: DebugHudMainTab) -> FrameDiagnosticsSnapshot {
    let mut snapshot = FrameDiagnosticsSnapshot {
        main_tab: tab,
        host: HostCpuMemoryHud {
            cpu_model: "Fixture CPU".to_string(),
            logical_cpus: 8,
            cpu_usage_percent: 12.5,
            ram_total_bytes: RAM_TOTAL,
            ram_used_bytes: RAM_USED,
            process_ram_bytes: Some(PROCESS_RAM),
        },
        ..FrameDiagnosticsSnapshot::default()
    };
    snapshot.gpu_allocator.totals = GpuAllocatorHud {
        allocated_bytes: Some(ALLOCATED),
        reserved_bytes: Some(RESERVED),
    };
    snapshot.gpu_allocator.report_next_refresh_in_secs = 0.5;
    snapshot.gpu_allocator.report = Some(allocator_report());
    snapshot.mesh_draw.last_submit_render_task_count = 3;
    snapshot.mesh_draw.textures_gpu_resident = 32;
    snapshot.mesh_draw.mesh_pool_entry_count = 64;
    snapshot.shader_routes.rows = vec![
        ShaderRouteRow {
            shader_asset_id: 42,
            shader_asset_name: Some("FixtureUnlit.shader".to_string()),
            shader_variant_bits: Some(0),
            pipeline_label: "unlit_default".to_string(),
            implemented: true,
        },
        ShaderRouteRow {
            shader_asset_id: 43,
            shader_asset_name: Some("FixtureMissing.shader".to_string()),
            shader_variant_bits: None,
            pipeline_label: "null".to_string(),
            implemented: false,
        },
    ];
    snapshot.mesh_draw.draw_state_rows = vec![draw_row()];
    snapshot
}

fn allocator_report() -> GpuAllocatorReportHud {
    let report = AllocatorReport {
        allocations: vec![
            AllocationReport {
                name: "frame-uniforms".to_string(),
                offset: 0,
                size: 4096,
            },
            AllocationReport {
                name: "mesh-vb".to_string(),
                offset: 4096,
                size: 1024 * 1024,
            },
        ],
        blocks: vec![MemoryBlockReport {
            size: RESERVED,
            allocations: 0..2,
        }],
        total_allocated_bytes: ALLOCATED,
        total_reserved_bytes: RESERVED,
    };
    GpuAllocatorReportHud {
        report: Arc::new(report),
        allocation_indices_by_size: Arc::from([1usize, 0]),
    }
}

fn draw_row() -> WorldMeshDrawStateRow {
    WorldMeshDrawStateRow {
        draw_index: 0,
        space_id: 1,
        node_id: 7,
        mesh_asset_id: 100,
        slot_index: 0,
        material_stack_first_slot_index: None,
        shader_asset_id: 42,
        material_asset_id: 9,
        property_block_slot0: None,
        pipeline: RasterPipelineKind::EmbeddedStem(Arc::from("pbs_metallic")),
        blend_mode: MaterialBlendMode::Opaque,
        is_overlay: false,
        sorting_order: 0,
        collect_order: 0,
        skinned: false,
        alpha_blended: false,
        transparent_class: TransparentMaterialClass::Opaque,
        requires_intersection_pass: false,
        depth_write: Some(true),
        depth_compare: None,
        depth_offset: None,
        stencil_enabled: false,
        stencil_reference: 0,
        stencil_compare: 0,
        stencil_pass_op: 0,
        stencil_read_mask: 0xff,
        stencil_write_mask: 0xff,
        color_mask: None,
    }
}

fn gpu_profiler() -> GpuProfilerSnapshot {
    GpuProfilerSnapshot {
        entries: vec![
            GpuPassEntry {
                name: "world_mesh".to_string(),
                ms: 4.5_f32,
                depth: 0,
            },
            GpuPassEntry {
                name: "post".to_string(),
                ms: 1.25_f32,
                depth: 0,
            },
        ],
        stats: GpuProfilerFrameStats {
            frame_order: 1200,
            opened_queries: 2,
            skipped_queries: 0,
            soft_query_budget: 64,
        },
    }
}
