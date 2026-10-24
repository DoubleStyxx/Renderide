//! Lavapipe readback of the object-velocity shader.
//!
//! A still camera plus a moving rigid draw must write non-zero velocity only on the draw's
//! pixels. The scene harness defaults to `MotionVectorMode::NoMotion`; this test opts into object
//! motion by packing a previous model itself.

use wgpu::util::DeviceExt;

use super::{ObjectMotionPipelineKey, pipelines};
use crate::materials::{RasterFrontFace, RasterPrimitiveTopology};
use crate::mesh_deform::{PER_DRAW_UNIFORM_STRIDE, PaddedPerDrawUniforms};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 32;
const ROW_BYTES: u32 = 256;

struct PixelTargets {
    velocity: wgpu::Texture,
    velocity_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

struct UploadedDraw {
    vertices: wgpu::Buffer,
    per_draw: wgpu::BindGroup,
    previous_vp: wgpu::BindGroup,
}

#[test]
fn still_camera_moving_object_writes_velocity_only_on_covered_pixels() {
    let Some((device, queue)) = adapter_device() else {
        require_lavapipe_on_linux();
        return;
    };
    let targets = pixel_targets(&device);
    let uploaded = upload_moving_quad(&device);
    let pipeline = object_pipeline(&device);
    let depth_pipeline = depth_pipeline(&device);
    let bytes = render_velocity(
        &device,
        &queue,
        &targets,
        &uploaded,
        &pipeline,
        &depth_pipeline,
    );
    assert!(
        !pixel_nonzero(&bytes, 4, 16),
        "background pixel must keep zero velocity while the camera is still"
    );
    assert!(
        pixel_nonzero(&bytes, 24, 16),
        "moving object pixel must have non-zero velocity while the camera is still"
    );
}

fn pixel_targets(device: &wgpu::Device) -> PixelTargets {
    let velocity = device.create_texture(&texture_desc(
        "object_motion_pixels",
        wgpu::TextureFormat::Rg16Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    ));
    let depth = device.create_texture(&texture_desc(
        "object_motion_pixels_depth",
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    ));
    let velocity_view = velocity.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    PixelTargets {
        velocity,
        velocity_view,
        depth_view,
    }
}

fn texture_desc(
    label: &str,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureDescriptor<'_> {
    wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

fn upload_moving_quad(device: &wgpu::Device) -> UploadedDraw {
    let vertices = right_half_quad();
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("object_motion_pixels_vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let previous = glam::Mat4::from_translation(glam::Vec3::new(0.2, 0.0, 0.0));
    let draw = PaddedPerDrawUniforms::new_single(glam::Mat4::IDENTITY, glam::Mat4::IDENTITY)
        .with_previous_model(Some(previous));
    let mut slab = vec![0u8; PER_DRAW_UNIFORM_STRIDE];
    slab.copy_from_slice(bytemuck::bytes_of(&draw));
    let slab_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("object_motion_pixels_slab"),
        contents: &slab,
        usage: wgpu::BufferUsages::STORAGE,
    });
    let previous_vp = [
        glam::Mat4::IDENTITY.to_cols_array(),
        glam::Mat4::IDENTITY.to_cols_array(),
    ];
    let previous_vp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("object_motion_pixels_previous_vp"),
        contents: bytemuck::cast_slice(&previous_vp),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let cache = pipelines();
    UploadedDraw {
        vertices: vertex_buffer,
        per_draw: per_draw_bind_group(device, cache.per_draw_layout(device), &slab_buffer),
        previous_vp: previous_vp_bind_group(
            device,
            cache.previous_vp_layout(device),
            &previous_vp_buffer,
        ),
    }
}

fn per_draw_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    slab: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("object_motion_pixels_per_draw"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: slab,
                offset: 0,
                size: wgpu::BufferSize::new(PER_DRAW_UNIFORM_STRIDE as u64),
            }),
        }],
    })
}

fn previous_vp_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("object_motion_pixels_previous"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

fn object_pipeline(device: &wgpu::Device) -> std::sync::Arc<wgpu::RenderPipeline> {
    pipelines().pipeline(
        device,
        ObjectMotionPipelineKey {
            depth_format: wgpu::TextureFormat::Depth32Float,
            front_face: RasterFrontFace::CounterClockwise,
            cull_mode: None,
            primitive_topology: RasterPrimitiveTopology::TriangleList,
            multiview_stereo: false,
        },
    )
}

fn render_velocity(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    targets: &PixelTargets,
    uploaded: &UploadedDraw,
    pipeline: &wgpu::RenderPipeline,
    depth_pipeline: &wgpu::RenderPipeline,
) -> Vec<u8> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("object_motion_pixels"),
    });
    encode_depth_pass(&mut encoder, &targets.depth_view, depth_pipeline, uploaded);
    encode_velocity_pass(&mut encoder, targets, uploaded, pipeline);
    let readback = copy_velocity(&mut encoder, device, &targets.velocity);
    queue.submit(Some(encoder.finish()));
    map_readback(device, &readback)
}

fn encode_depth_pass(
    encoder: &mut wgpu::CommandEncoder,
    depth_view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    uploaded: &UploadedDraw,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("object_motion_pixels_depth"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_vertex_buffer(0, uploaded.vertices.slice(..));
    pass.draw(0..6, 0..1);
}

fn encode_velocity_pass(
    encoder: &mut wgpu::CommandEncoder,
    targets: &PixelTargets,
    uploaded: &UploadedDraw,
    pipeline: &wgpu::RenderPipeline,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("object_motion_pixels_velocity"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &targets.velocity_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &targets.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &uploaded.per_draw, &[0]);
    pass.set_bind_group(1, &uploaded.previous_vp, &[]);
    pass.set_vertex_buffer(0, uploaded.vertices.slice(..));
    pass.draw(0..6, 0..1);
}

fn copy_velocity(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    velocity: &wgpu::Texture,
) -> wgpu::Buffer {
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("object_motion_pixels_readback"),
        size: u64::from(ROW_BYTES * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: velocity,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW_BYTES),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    readback
}

fn map_readback(device: &wgpu::Device, readback: &wgpu::Buffer) -> Vec<u8> {
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll object-velocity readback");
    receiver
        .recv()
        .expect("map callback")
        .expect("map readback");
    slice.get_mapped_range().expect("mapped range").to_vec()
}

fn pixel_nonzero(bytes: &[u8], x: u32, y: u32) -> bool {
    let offset = (y * ROW_BYTES + x * 4) as usize;
    bytes[offset..offset + 4].iter().any(|byte| *byte != 0)
}

fn right_half_quad() -> [[f32; 4]; 6] {
    let left = [0.0, -1.0, 0.5, 1.0];
    let bottom = [1.0, -1.0, 0.5, 1.0];
    let top = [1.0, 1.0, 0.5, 1.0];
    let upper_left = [0.0, 1.0, 0.5, 1.0];
    [left, bottom, top, left, top, upper_left]
}

fn require_lavapipe_on_linux() {
    assert!(
        !host_is_linux(),
        "linux CI must expose a lavapipe adapter for the object-velocity pixel test"
    );
}

fn host_is_linux() -> bool {
    cfg!(target_os = "linux")
}

fn adapter_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapter = adapters.into_iter().next()?;
    Some(
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("object-motion-pixels"),
            ..Default::default()
        }))
        .expect("device"),
    )
}

fn depth_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("object_motion_pixels_depth"),
        source: wgpu::ShaderSource::Wgsl(
            "@vertex fn vs_main(@location(0) pos: vec4<f32>) -> @builtin(position) vec4<f32> { return pos; }"
                .into(),
        ),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("object_motion_pixels_depth"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 16,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                }],
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}
