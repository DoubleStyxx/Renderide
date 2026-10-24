//! Shadow atlas texture views and the per-layer depth encode helpers.

use std::sync::Arc;

use glam::Mat4;
use rayon::prelude::*;

use crate::cpu_parallelism::{
    admit_render_command_items, current_reference_worker_count, record_parallel_admission,
};
use crate::gpu::GpuLimits;
use crate::materials::{MaterialPipelineDesc, ShaderPermutation};
use crate::mesh_deform::PER_DRAW_UNIFORM_STRIDE;
use crate::passes::WorldMeshForwardPipelineState;

use super::{
    PaddedShadowCasterDraw, SHADOW_SLAB_PARALLEL_CHUNK_DRAWS, SHADOW_SLAB_PARALLEL_CHUNKS_PER_TASK,
    SHADOW_SLAB_PARALLEL_MIN_DRAWS, ShadowCasterSet,
};

pub(super) fn create_shadow_texture(
    device: &wgpu::Device,
    resolution: u32,
    layers: u32,
    format: wgpu::TextureFormat,
    renderable: bool,
) -> (
    Arc<wgpu::Texture>,
    Arc<wgpu::TextureView>,
    Vec<Arc<wgpu::TextureView>>,
) {
    let usage = if renderable {
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
    } else {
        wgpu::TextureUsages::TEXTURE_BINDING
    };
    let texture = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow_depth_atlas"),
        size: wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }));
    let atlas_view =
        Arc::new(texture.create_view(&shadow_atlas_array_view_descriptor(layers, format)));
    crate::profiling::note_resource_churn!(TextureView, "backend::shadow_depth_atlas_array");
    let layer_views = if renderable {
        let mut layer_views = Vec::with_capacity(layers as usize);
        for layer in 0..layers {
            layer_views.push(Arc::new(
                texture.create_view(&shadow_atlas_layer_view_descriptor(layer, format)),
            ));
            crate::profiling::note_resource_churn!(
                TextureView,
                "backend::shadow_depth_atlas_layer"
            );
        }
        layer_views
    } else {
        Vec::new()
    };
    (texture, atlas_view, layer_views)
}

pub(super) fn shadow_atlas_array_view_descriptor(
    layers: u32,
    format: wgpu::TextureFormat,
) -> wgpu::TextureViewDescriptor<'static> {
    wgpu::TextureViewDescriptor {
        label: Some("shadow_depth_atlas_array"),
        format: Some(format),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(layers),
    }
}

pub(super) fn shadow_atlas_layer_view_descriptor(
    layer: u32,
    format: wgpu::TextureFormat,
) -> wgpu::TextureViewDescriptor<'static> {
    wgpu::TextureViewDescriptor {
        label: Some("shadow_depth_atlas_layer"),
        format: Some(format),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: layer,
        array_layer_count: Some(1),
    }
}

pub(super) fn clear_shadow_layer(
    encoder: &mut wgpu::CommandEncoder,
    layer_view: &wgpu::TextureView,
    profiler: Option<&crate::profiling::GpuProfilerHandle>,
) {
    let pass_query = profiler.map(|p| p.begin_pass_query("shadows::atlas_clear", encoder));
    let timestamp_writes = crate::profiling::render_pass_timestamp_writes(pass_query.as_ref());
    {
        let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow_atlas_clear"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: layer_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes,
            multiview_mask: None,
        });
    }
    if let Some(query) = pass_query
        && let Some(p) = profiler
    {
        p.end_query(encoder, query);
    }
}

pub(super) fn pack_shadow_uniforms(
    uniforms: &mut [PaddedShadowCasterDraw],
    caster_set: &ShadowCasterSet,
    gpu_limits: &GpuLimits,
) {
    let admission =
        admit_render_command_items(caster_set.draws.len(), current_reference_worker_count());
    record_parallel_admission(
        "shadow_slab_pack",
        caster_set.draws.len(),
        caster_set.draws.len(),
        admission,
    );
    let slab_layout = &caster_set.instance_plan.slab_layout;
    let pack_one = |slot: &mut PaddedShadowCasterDraw, draw_idx: usize| {
        let item = &caster_set.draws[draw_idx];
        *slot = PaddedShadowCasterDraw::new(item);
    };
    if caster_set.draws.len() >= SHADOW_SLAB_PARALLEL_MIN_DRAWS && admission.is_parallel() {
        uniforms
            .par_chunks_mut(SHADOW_SLAB_PARALLEL_CHUNK_DRAWS)
            .with_min_len(SHADOW_SLAB_PARALLEL_CHUNKS_PER_TASK)
            .zip(
                slab_layout
                    .par_chunks(SHADOW_SLAB_PARALLEL_CHUNK_DRAWS)
                    .with_min_len(SHADOW_SLAB_PARALLEL_CHUNKS_PER_TASK),
            )
            .for_each(|(slots, layout)| {
                profiling::scope!("shadows::pack_slab::worker");
                for (slot, &draw_idx) in slots.iter_mut().zip(layout.iter()) {
                    pack_one(slot, draw_idx);
                }
            });
    } else {
        for (slot, &draw_idx) in uniforms.iter_mut().zip(slab_layout.iter()) {
            pack_one(slot, draw_idx);
        }
    }
    if !gpu_limits.supports_base_instance {
        debug_assert_eq!(
            uniforms.len(),
            slab_layout.len(),
            "downlevel shadow slabs still pack one slot per singleton draw group"
        );
    }
}

pub(super) fn shadow_layer_uniform_offset(layer: u32) -> Option<u32> {
    layer.checked_mul(PER_DRAW_UNIFORM_STRIDE as u32)
}

pub(super) fn shadow_caster_model(item: &crate::world_mesh::WorldMeshDrawItem) -> Mat4 {
    if item.world_space_deformed {
        Mat4::IDENTITY
    } else {
        item.rigid_world_matrix.unwrap_or(Mat4::IDENTITY)
    }
}

pub(super) fn shadow_pipeline_state(format: wgpu::TextureFormat) -> WorldMeshForwardPipelineState {
    WorldMeshForwardPipelineState {
        use_multiview: false,
        pass_desc: MaterialPipelineDesc {
            surface_format: wgpu::TextureFormat::Rgba8Unorm,
            depth_stencil_format: Some(format),
            sample_count: 1,
            multiview_mask: None,
        },
        shader_perm: ShaderPermutation::default(),
        front_face_flip: false,
    }
}
