//! Lavapipe check that a double-buffered render texture keeps last frame's color.
//!
//! This is a unit test, not a frame submission. Passes record timestamp queries so the
//! GPU instrumentation audit still sees real `timestamp_writes`. The copy and the queue
//! submit are outside primary frame timing and are allowlisted in `gpu_instrumentation_audit`.

use std::sync::Arc;

use crate::assets::texture::estimate_gpu_texture_bytes;
use crate::frame_contract::{OffscreenWriteTarget, RenderTextureSelfSampling};
use crate::gpu::GpuLimits;
use crate::gpu_pools::test_support::render_texture_format;
use crate::shared::TextureWrapMode;

use super::{GpuRenderTexture, RenderTexturePool};

const EXTENT: u32 = 4;

struct TestGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn timestamp_features() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES
}

fn open_test_gpu() -> Option<TestGpu> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapter = adapters.into_iter().next()?;
    let needed = timestamp_features();
    if !adapter.features().contains(needed) {
        return None;
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("render-texture-double-buffer"),
        required_features: needed,
        ..Default::default()
    }))
    .ok()?;
    Some(TestGpu { device, queue })
}

fn timestamp_query(device: &wgpu::Device) -> wgpu::QuerySet {
    device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("double-buffer-timestamps"),
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    })
}

fn pass_timestamps(query_set: &wgpu::QuerySet) -> wgpu::RenderPassTimestampWrites<'_> {
    wgpu::RenderPassTimestampWrites {
        query_set,
        beginning_of_pass_write_index: Some(0),
        end_of_pass_write_index: Some(1),
    }
}

fn limits() -> GpuLimits {
    GpuLimits::synthetic_for_tests(
        wgpu::Limits {
            max_texture_dimension_2d: 8192,
            ..Default::default()
        },
        wgpu::Features::empty(),
        hashbrown::HashMap::new(),
    )
}

fn render_texture(device: &wgpu::Device, asset_id: i32) -> GpuRenderTexture {
    let mut fmt = render_texture_format(TextureWrapMode::Clamp, TextureWrapMode::Clamp);
    fmt.asset_id = asset_id;
    fmt.size = glam::IVec2::new(EXTENT as i32, EXTENT as i32);
    GpuRenderTexture::new_from_format(device, &limits(), &fmt)
        .expect("4x4 render texture fits the synthetic limit")
}

fn clear_color(gpu: &TestGpu, view: &wgpu::TextureView, color: wgpu::Color) {
    let timestamps = timestamp_query(&gpu.device);
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("double-buffer-clear"),
        });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("double-buffer-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: Some(pass_timestamps(&timestamps)),
            multiview_mask: None,
        });
    }
    gpu.queue.submit(std::iter::once(encoder.finish()));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("clear submit");
}

fn sample_pipeline(device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("double-buffer-sample"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@group(0) @binding(0) var src: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
let x = f32((vertex_index << 1u) & 2u);
let y = f32(vertex_index & 2u);
return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
return textureLoad(src, vec2<i32>(position.xy), 0);
}
"#
            .into(),
        ),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("double-buffer-sample-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("double-buffer-sample-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("double-buffer-sample"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    (layout, pipeline)
}

fn sample_target(
    gpu: &TestGpu,
    layout: &wgpu::BindGroupLayout,
    source: &wgpu::TextureView,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("double-buffer-sample-target"),
        size: wgpu::Extent3d {
            width: EXTENT,
            height: EXTENT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("double-buffer-sample-bg"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(source),
        }],
    });
    (target, target_view, bind_group)
}

fn read_sampled_pixel(
    gpu: &TestGpu,
    layout: &wgpu::BindGroupLayout,
    pipeline: &wgpu::RenderPipeline,
    source: &wgpu::TextureView,
) -> [u8; 4] {
    let (target, target_view, bind_group) = sample_target(gpu, layout, source);
    let bytes_per_row = 256u32;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("double-buffer-sample-readback"),
        size: u64::from(bytes_per_row) * u64::from(EXTENT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encode_sample(
        gpu,
        pipeline,
        &target,
        &target_view,
        &bind_group,
        &buffer,
        bytes_per_row,
    );
    map_first_pixel(gpu, &buffer)
}

fn encode_sample(
    gpu: &TestGpu,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::Texture,
    target_view: &wgpu::TextureView,
    bind_group: &wgpu::BindGroup,
    buffer: &wgpu::Buffer,
    bytes_per_row: u32,
) {
    let timestamps = timestamp_query(&gpu.device);
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("double-buffer-sample"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("double-buffer-sample"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: Some(pass_timestamps(&timestamps)),
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(EXTENT),
            },
        },
        wgpu::Extent3d {
            width: EXTENT,
            height: EXTENT,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(std::iter::once(encoder.finish()));
}

fn map_first_pixel(gpu: &TestGpu, buffer: &wgpu::Buffer) -> [u8; 4] {
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("sample submit");
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("map callback")
        .expect("map readback");
    let data = slice.get_mapped_range().expect("mapped range");
    let pixel = [data[0], data[1], data[2], data[3]];
    drop(data);
    buffer.unmap();
    pixel
}

fn previous_sample_view(pool: &RenderTexturePool) -> Arc<wgpu::TextureView> {
    let allow_previous = OffscreenWriteTarget::host_render_texture_with_self_sampling(
        7,
        RenderTextureSelfSampling::AllowPreviousContents,
    );
    assert!(allow_previous.allows_previous_render_texture_contents(7));
    let texture = pool.get(7).expect("resident");
    let previous =
        texture.color_view_for_sampling(allow_previous.allows_previous_render_texture_contents(7));
    assert!(!Arc::ptr_eq(&previous, &texture.color_view));
    previous
}

/// A double-buffered camera samples the previous color image.
///
/// The second image is created on the first prepare and reused. A render texture that never
/// prepares double buffering keeps a single color allocation. Post-processing uses the same
/// `AllowPreviousContents` policy; that flag combination is covered by the view-planning test.
#[test]
fn previous_color_survives_write_and_is_allocated_once() {
    let Some(gpu) = open_test_gpu() else {
        logger::info!("double-buffer GPU sample skipped: no adapter");
        return;
    };
    let (mut pool, plain, single_bytes) = seed_pool(&gpu);
    let pointers = first_swap_allocates_history(&gpu, &mut pool, single_bytes);
    let (layout, pipeline) = sample_pipeline(&gpu.device);
    previous_image_ignores_published_write(&gpu, &pool, &layout, &pipeline);
    second_swap_reuses_history(&gpu, &mut pool, &layout, &pipeline, single_bytes, pointers);
    assert!(!plain.has_previous_color());
    assert_eq!(plain.resident_bytes, single_bytes);
}

fn seed_pool(gpu: &TestGpu) -> (RenderTexturePool, GpuRenderTexture, u64) {
    let mut pool = RenderTexturePool::new();
    let texture = render_texture(&gpu.device, 7);
    let single_bytes = texture.resident_bytes;
    assert!(!texture.has_previous_color());
    let plain = render_texture(&gpu.device, 8);
    assert!(!plain.has_previous_color());
    assert_eq!(plain.resident_bytes, single_bytes);
    pool.insert(texture);
    assert_eq!(
        pool.accounting().texture_resident_bytes(),
        single_bytes,
        "a render texture without the flag charges one color image plus depth"
    );
    (pool, plain, single_bytes)
}

fn first_swap_allocates_history(
    gpu: &TestGpu,
    pool: &mut RenderTexturePool,
    single_bytes: u64,
) -> (*const wgpu::Texture, *const wgpu::Texture) {
    let published = Arc::clone(&pool.get(7).expect("resident").color_view);
    clear_color(
        gpu,
        &published,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    let allocated = pool
        .begin_double_buffered_frame(&gpu.device, 7)
        .expect("resident");
    assert!(allocated, "the second color image is created on first use");
    let color_bytes =
        estimate_gpu_texture_bytes(wgpu::TextureFormat::Rgba16Float, EXTENT, EXTENT, 1);
    assert_eq!(
        pool.accounting().texture_resident_bytes(),
        single_bytes + color_bytes
    );
    let texture = pool.get(7).expect("resident");
    assert!(texture.has_previous_color());
    (
        Arc::as_ptr(&texture.color_texture),
        Arc::as_ptr(&texture.previous_color.as_ref().expect("history").texture),
    )
}

fn previous_image_ignores_published_write(
    gpu: &TestGpu,
    pool: &RenderTexturePool,
    layout: &wgpu::BindGroupLayout,
    pipeline: &wgpu::RenderPipeline,
) {
    let previous_view = previous_sample_view(pool);
    let red = read_sampled_pixel(gpu, layout, pipeline, &previous_view);
    assert_eq!(
        red,
        [255, 0, 0, 255],
        "previous image keeps the pre-swap clear"
    );
    let write_view = Arc::clone(&pool.get(7).expect("resident").color_view);
    clear_color(
        gpu,
        &write_view,
        wgpu::Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    );
    let still_red = read_sampled_pixel(gpu, layout, pipeline, &previous_view);
    assert_eq!(
        still_red,
        [255, 0, 0, 255],
        "writing the published image does not change the previous image"
    );
}

fn second_swap_reuses_history(
    gpu: &TestGpu,
    pool: &mut RenderTexturePool,
    layout: &wgpu::BindGroupLayout,
    pipeline: &wgpu::RenderPipeline,
    single_bytes: u64,
    pointers: (*const wgpu::Texture, *const wgpu::Texture),
) {
    let allocated_again = pool
        .begin_double_buffered_frame(&gpu.device, 7)
        .expect("resident");
    assert!(!allocated_again, "the second image is reused");
    let color_bytes =
        estimate_gpu_texture_bytes(wgpu::TextureFormat::Rgba16Float, EXTENT, EXTENT, 1);
    assert_eq!(
        pool.accounting().texture_resident_bytes(),
        single_bytes + color_bytes
    );
    let texture = pool.get(7).expect("resident");
    let published_now = Arc::as_ptr(&texture.color_texture);
    let previous_now = Arc::as_ptr(&texture.previous_color.as_ref().expect("history").texture);
    assert_eq!(published_now, pointers.1);
    assert_eq!(previous_now, pointers.0);
    let previous_after_second_swap = texture.color_view_for_sampling(true);
    let green = read_sampled_pixel(gpu, layout, pipeline, &previous_after_second_swap);
    assert_eq!(
        green,
        [0, 255, 0, 255],
        "the next frame samples the image written last frame"
    );
}
