//! Depth-equal rigid object velocity draws.
//!
//! Reuses the forward per-draw slab and position stream. The pass loads the camera velocity
//! target and overwrites only pixels whose depth matches the forward depth buffer.

use std::sync::{Arc, LazyLock};

use crate::embedded_shaders::embedded_wgsl;
use crate::gpu::GpuLimits;
use crate::gpu_resource::{OnceGpu, RenderPipelineMap};
use crate::materials::{RasterFrontFace, RasterPrimitiveTopology};
use crate::mesh_deform::PER_DRAW_UNIFORM_STRIDE;
use crate::render_graph::gpu_cache::multiview_mask;
use crate::shared::ShadowCastMode;
use crate::world_mesh::WorldMeshDrawItem;

use super::super::depth_like_pipeline::{DepthLikePipelineSpec, create_depth_like_pipeline};
use super::super::{PreparedWorldMeshForwardFrame, WorldMeshForwardEncodeRefs};

const POSITION_ATTRIBUTES: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
    offset: 0,
    shader_location: 0,
    format: wgpu::VertexFormat::Float32x4,
}];

const PREVIOUS_VP_UNIFORM_SIZE: u64 = 128;

/// Draw inputs for one object-velocity raster pass.
pub(crate) struct ObjectMotionDraw<'pass, 'frame, 'data> {
    /// Active render pass. Velocity is already bound as a load/store color target.
    pub(crate) rpass: &'pass mut wgpu::RenderPass<'frame>,
    /// Device used to create the velocity pipelines.
    pub(crate) device: &'data wgpu::Device,
    /// Prepared forward draws and slab layout for this view.
    pub(crate) prepared: &'data PreparedWorldMeshForwardFrame,
    /// Mesh pool used to bind the position stream.
    pub(crate) encode: &'data WorldMeshForwardEncodeRefs<'data>,
    /// Storage-buffer alignment for the per-draw slab.
    pub(crate) gpu_limits: &'data GpuLimits,
    /// Forward per-draw slab bind group.
    pub(crate) per_draw_bind_group: &'data wgpu::BindGroup,
    /// Previous view-projection uniform bind group.
    pub(crate) previous_vp_bind_group: &'data wgpu::BindGroup,
    /// Single-sample frame depth format used by the depth-equal test.
    pub(crate) depth_format: wgpu::TextureFormat,
    /// Whether this view records both eyes in one multiview pass.
    pub(crate) multiview_stereo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ObjectMotionPipelineKey {
    depth_format: wgpu::TextureFormat,
    front_face: RasterFrontFace,
    cull_mode: Option<wgpu::Face>,
    primitive_topology: RasterPrimitiveTopology,
    multiview_stereo: bool,
}

#[derive(Debug, Default)]
struct ObjectMotionPipelineCache {
    per_draw_layout: OnceGpu<wgpu::BindGroupLayout>,
    previous_vp_layout: OnceGpu<wgpu::BindGroupLayout>,
    pipelines: RenderPipelineMap<ObjectMotionPipelineKey>,
}

impl ObjectMotionPipelineCache {
    fn pipeline(
        &self,
        device: &wgpu::Device,
        key: ObjectMotionPipelineKey,
    ) -> Arc<wgpu::RenderPipeline> {
        self.pipelines
            .get_or_create(key, |key| self.create_pipeline(device, *key))
    }

    fn create_pipeline(
        &self,
        device: &wgpu::Device,
        key: ObjectMotionPipelineKey,
    ) -> wgpu::RenderPipeline {
        let (label, source) = if key.multiview_stereo {
            (
                "object_motion_multiview",
                embedded_wgsl!("object_motion_multiview"),
            )
        } else {
            (
                "object_motion_default",
                embedded_wgsl!("object_motion_default"),
            )
        };
        let per_draw_layout = self.per_draw_layout(device);
        let previous_vp_layout = self.previous_vp_layout(device);
        let bind_group_layouts = [per_draw_layout, previous_vp_layout];
        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &POSITION_ATTRIBUTES,
        }];
        let color_targets = [Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rg16Float,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        create_depth_like_pipeline(
            device,
            DepthLikePipelineSpec {
                label,
                shader_source: source,
                bind_group_layouts: &bind_group_layouts,
                vertex_buffers: &vertex_buffers,
                fragment_entry_point: Some("fs_main"),
                color_targets: &color_targets,
                primitive_topology: key.primitive_topology,
                front_face: key.front_face,
                cull_mode: key.cull_mode,
                depth_stencil_format: key.depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Equal,
                sample_count: 1,
                multiview_mask: multiview_mask(key.multiview_stereo),
            },
        )
    }

    fn per_draw_layout(&self, device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        self.per_draw_layout.get_or_create(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("object_motion_per_draw"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(PER_DRAW_UNIFORM_STRIDE as u64),
                    },
                    count: None,
                }],
            })
        })
    }

    fn previous_vp_layout(&self, device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        self.previous_vp_layout.get_or_create(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("object_motion_previous_vp"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(PREVIOUS_VP_UNIFORM_SIZE),
                    },
                    count: None,
                }],
            })
        })
    }
}

fn pipelines() -> &'static ObjectMotionPipelineCache {
    static CACHE: LazyLock<ObjectMotionPipelineCache> =
        LazyLock::new(ObjectMotionPipelineCache::default);
    &CACHE
}

/// Layout for the previous view-projection uniform consumed by the object-velocity shader.
fn object_motion_previous_vp_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
    pipelines().previous_vp_layout(device)
}

/// Bind group for one reused previous view-projection uniform buffer.
pub(crate) fn object_motion_previous_vp_bind_group(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("object_motion_previous_vp"),
        layout: object_motion_previous_vp_layout(device),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

/// Overwrites camera velocity for rigid draws that carry a previous model matrix.
pub(crate) fn record_object_motion_vectors(draw: ObjectMotionDraw<'_, '_, '_>) {
    profiling::scope!("world_mesh::object_motion_vectors");
    let ObjectMotionDraw {
        rpass,
        device,
        prepared,
        encode,
        gpu_limits,
        per_draw_bind_group,
        previous_vp_bind_group,
        depth_format,
        multiview_stereo,
    } = draw;
    rpass.set_bind_group(1, previous_vp_bind_group, &[]);
    let mut state = super::DepthLikeDrawState::new();
    let cache = pipelines();
    for (slot, &draw_idx) in prepared.plan.slab_layout.iter().enumerate() {
        let Some(item) = prepared.draws.get(draw_idx) else {
            continue;
        };
        if !object_motion_draw_eligible(item) {
            continue;
        }
        let Some(probe) = super::WorldMeshForwardDepthPrepassPipelineKey::for_draw_with_compare(
            item,
            &prepared.pipeline,
            wgpu::CompareFunction::Equal,
        ) else {
            continue;
        };
        let key = ObjectMotionPipelineKey {
            depth_format,
            front_face: probe.front_face,
            cull_mode: probe.cull_mode,
            primitive_topology: probe.primitive_topology,
            multiview_stereo,
        };
        super::bind_depth_like_per_draw_slab(
            rpass,
            super::DepthLikePerDrawBind {
                bind_group_index: 0,
                bind_group: per_draw_bind_group,
                gpu_limits,
                slab_first_instance: slot,
                instance_count: 1,
                supports_base_instance: prepared.supports_base_instance,
            },
            &mut state,
        );
        let pipeline = cache.pipeline(device, key);
        super::set_depth_like_pipeline_if_changed(rpass, pipeline.as_ref(), &mut state);
        let instances = if prepared.supports_base_instance {
            let slot = u32::try_from(slot).unwrap_or(u32::MAX);
            slot..slot.saturating_add(1)
        } else {
            0..1
        };
        super::draw_mesh_submesh_depth_instanced(
            rpass,
            item,
            super::gpu_refs_for_encode(encode),
            instances,
            &mut state.last_mesh,
        );
    }
}

fn object_motion_draw_eligible(item: &WorldMeshDrawItem) -> bool {
    item.previous_rigid_world_matrix.is_some()
        && item.shadow_cast_mode != ShadowCastMode::ShadowOnly
}
