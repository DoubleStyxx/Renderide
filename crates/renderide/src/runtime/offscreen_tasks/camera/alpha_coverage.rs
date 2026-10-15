//! Camera render-task alpha repair from reverse-Z depth coverage.

use std::sync::LazyLock;

use crate::embedded_shaders::embedded_wgsl;
use crate::gpu::GpuContext;
use crate::gpu::bind_layout::texture_layout_entry;
use crate::gpu_resource::{OnceGpu, RenderPipelineMap};
use crate::render_graph::gpu_cache::create_wgsl_shader_module;

use super::CameraTaskTargets;

/// Returns whether a reverse-Z depth sample marks rendered geometry coverage.
#[cfg(test)]
pub(in crate::runtime) fn depth_marks_coverage(reverse_z_depth: f32) -> bool {
    reverse_z_depth > crate::gpu::MAIN_FORWARD_DEPTH_CLEAR
}

/// Writes alpha 1 to covered CameraRenderTask pixels while preserving existing alpha elsewhere.
pub(in crate::runtime) fn apply_camera_task_alpha_coverage(
    gpu: &mut GpuContext,
    targets: &CameraTaskTargets,
) {
    apply_alpha_coverage_to_target(
        gpu,
        targets.color_view.as_ref(),
        targets.depth_texture.as_ref(),
        targets.color_format,
        "camera_task_alpha_coverage",
    );
}

/// Writes alpha 1 to covered pixels on an arbitrary camera-task render target.
pub(in crate::runtime) fn apply_alpha_coverage_to_target(
    gpu: &mut GpuContext,
    color_view: &wgpu::TextureView,
    depth_texture: &wgpu::Texture,
    color_format: wgpu::TextureFormat,
    label: &'static str,
) {
    profiling::scope!("camera_task::alpha_coverage");
    let pipelines = pipeline_cache();
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("camera_task_alpha_coverage_depth"),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::DepthOnly,
        ..Default::default()
    });
    crate::profiling::note_resource_churn!(
        TextureView,
        "runtime::camera_task_alpha_coverage_depth_view"
    );
    let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: pipelines.bind_group_layout(gpu.device()),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&depth_view),
        }],
    });
    crate::profiling::note_resource_churn!(
        BindGroup,
        "runtime::camera_task_alpha_coverage_bind_group"
    );

    let mut encoder = gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    let pass_query = gpu
        .gpu_profiler_mut()
        .map(|p| p.begin_pass_query("camera_task::alpha_coverage.pass", &mut encoder));
    let timestamp_writes = crate::profiling::render_pass_timestamp_writes(pass_query.as_ref());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipelines.pipeline(gpu.device(), color_format).as_ref());
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    if let Some(query) = pass_query
        && let Some(prof) = gpu.gpu_profiler_mut()
    {
        prof.end_query(&mut encoder, query);
        prof.resolve_queries(&mut encoder);
    }
    let command_buffer = {
        profiling::scope!("CommandEncoder::finish::camera_task_alpha_coverage");
        encoder.finish()
    };
    gpu.queue().submit(std::iter::once(command_buffer));
}

fn pipeline_cache() -> &'static CameraTaskAlphaCoveragePipelineCache {
    static CACHE: LazyLock<CameraTaskAlphaCoveragePipelineCache> =
        LazyLock::new(CameraTaskAlphaCoveragePipelineCache::default);
    &CACHE
}

#[derive(Default)]
struct CameraTaskAlphaCoveragePipelineCache {
    bind_group_layout: OnceGpu<wgpu::BindGroupLayout>,
    pipelines: RenderPipelineMap<wgpu::TextureFormat>,
}

impl CameraTaskAlphaCoveragePipelineCache {
    fn bind_group_layout(&self, device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        self.bind_group_layout.get_or_create(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_task_alpha_coverage"),
                entries: &[texture_layout_entry(
                    0,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2,
                    false,
                )],
            })
        })
    }

    fn pipeline(
        &self,
        device: &wgpu::Device,
        output_format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        self.pipelines.get_or_create(output_format, |format| {
            logger::debug!(
                "camera_task_alpha_coverage: building pipeline (dst format = {:?})",
                format
            );
            create_camera_task_alpha_coverage_pipeline(
                device,
                self.bind_group_layout(device),
                *format,
            )
        })
    }
}

fn create_camera_task_alpha_coverage_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    output_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = create_wgsl_shader_module(
        device,
        "camera_task_alpha_coverage",
        embedded_wgsl!("camera_task_alpha_coverage"),
    );
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("camera_task_alpha_coverage"),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("camera_task_alpha_coverage"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: output_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Max,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALPHA,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    crate::profiling::note_resource_churn!(
        RenderPipeline,
        "runtime::camera_task_alpha_coverage_pipeline"
    );
    pipeline
}
