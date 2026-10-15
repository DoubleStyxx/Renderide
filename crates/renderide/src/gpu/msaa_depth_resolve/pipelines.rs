//! MSAA depth resolve pipeline construction (compute + per-format blit pipelines).
//!
//! Init-time effects: shader module + bind-group layout + pipeline creation. The
//! per-format pipeline tables live here; the per-frame `encode_resolve*` paths in
//! [`super::encode`] consume them and remain free of pipeline-creation code.

use crate::embedded_shaders::embedded_wgsl;

/// Desktop (non-multiview) depth blit pipelines for each depth/stencil format variant.
pub(super) struct DesktopBlitPipelines {
    pub depth32: wgpu::RenderPipeline,
    pub depth24_stencil8: wgpu::RenderPipeline,
    pub depth32_stencil8: Option<wgpu::RenderPipeline>,
}

/// Optional multiview stereo blit pipelines and bind-group layout.
pub(super) struct StereoMultiviewBlitPipelines {
    pub depth32: Option<wgpu::RenderPipeline>,
    pub depth24_stencil8: Option<wgpu::RenderPipeline>,
    pub depth32_stencil8: Option<wgpu::RenderPipeline>,
    pub bgl: Option<wgpu::BindGroupLayout>,
}

pub(super) fn create_desktop_blit_pipelines(
    device: &wgpu::Device,
    blit_shader: &wgpu::ShaderModule,
    blit_layout: &wgpu::PipelineLayout,
) -> DesktopBlitPipelines {
    DesktopBlitPipelines {
        depth32: create_depth_blit_pipeline(
            device,
            blit_shader,
            blit_layout,
            "msaa_depth_blit_depth32",
            wgpu::TextureFormat::Depth32Float,
            None,
        ),
        depth24_stencil8: create_depth_blit_pipeline(
            device,
            blit_shader,
            blit_layout,
            "msaa_depth_blit_depth24_stencil8",
            wgpu::TextureFormat::Depth24PlusStencil8,
            None,
        ),
        depth32_stencil8: device
            .features()
            .contains(wgpu::Features::DEPTH32FLOAT_STENCIL8)
            .then(|| {
                create_depth_blit_pipeline(
                    device,
                    blit_shader,
                    blit_layout,
                    "msaa_depth_blit_depth32_stencil8",
                    wgpu::TextureFormat::Depth32FloatStencil8,
                    None,
                )
            }),
    }
}

pub(super) fn create_stereo_multiview_blit_pipelines(
    device: &wgpu::Device,
) -> StereoMultiviewBlitPipelines {
    if !device.features().contains(wgpu::Features::MULTIVIEW) {
        return StereoMultiviewBlitPipelines {
            depth32: None,
            depth24_stencil8: None,
            depth32_stencil8: None,
            bgl: None,
        };
    }
    let blit_stereo_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("msaa_depth_resolve_blit_stereo"),
        source: wgpu::ShaderSource::Wgsl(
            embedded_wgsl!("depth_blit_r32_to_depth_multiview").into(),
        ),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("msaa_depth_blit_stereo_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                multisampled: false,
                view_dimension: wgpu::TextureViewDimension::D2Array,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("msaa_depth_blit_stereo_pl"),
        bind_group_layouts: &[Some(&bgl)],
        ..Default::default()
    });
    let multiview_mask = std::num::NonZeroU32::new(3);
    let depth32 = create_depth_blit_pipeline(
        device,
        &blit_stereo_shader,
        &layout,
        "msaa_depth_blit_stereo_depth32",
        wgpu::TextureFormat::Depth32Float,
        multiview_mask,
    );
    let depth24_stencil8 = create_depth_blit_pipeline(
        device,
        &blit_stereo_shader,
        &layout,
        "msaa_depth_blit_stereo_depth24_stencil8",
        wgpu::TextureFormat::Depth24PlusStencil8,
        multiview_mask,
    );
    let depth32_stencil8 = device
        .features()
        .contains(wgpu::Features::DEPTH32FLOAT_STENCIL8)
        .then(|| {
            create_depth_blit_pipeline(
                device,
                &blit_stereo_shader,
                &layout,
                "msaa_depth_blit_stereo_depth32_stencil8",
                wgpu::TextureFormat::Depth32FloatStencil8,
                multiview_mask,
            )
        });
    StereoMultiviewBlitPipelines {
        depth32: Some(depth32),
        depth24_stencil8: Some(depth24_stencil8),
        depth32_stencil8,
        bgl: Some(bgl),
    }
}

pub(super) fn create_depth_blit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    format: wgpu::TextureFormat,
    multiview_mask: Option<std::num::NonZeroU32>,
) -> wgpu::RenderPipeline {
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask,
        cache: None,
    });
    crate::profiling::note_resource_churn!(RenderPipeline, "gpu::msaa_depth_blit_pipeline");
    pipeline
}
