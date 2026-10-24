//! GPU golden for two cameras stacked into one texture.
//!
//! The first camera leaves a red/blue color split and a near depth of `0.8`. The second camera
//! draws green in the center at depth `0.2` with reverse-Z `Greater`. Clear-none keeps both the
//! color and the depth, so the green draw is occluded. Depth-only keeps the color and clears
//! depth, so the green draw appears. Skybox still clears both.

use std::path::PathBuf;

use crate::frame_contract::FrameViewClear;
use crate::shared::CameraClearMode;

use super::clear_load::{ScenePreserveQuery, forward_clear_loads};
use super::color_preserve::{copy_preserved_scene_color, preserved_color_texture};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;
const HALF_X: u32 = 32;
const SCISSOR_X: u32 = 16;
const SCISSOR_W: u32 = 32;
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const YELLOW: wgpu::Color = wgpu::Color {
    r: 1.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
const MAGENTA: [u8; 4] = [255, 0, 255, 255];
const MAGENTA_CLEAR: wgpu::Color = wgpu::Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

const STACK_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_prefill(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[index], 0.8, 1.0);
    return out;
}

@vertex
fn vs_overlay(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[index], 0.2, 1.0);
    return out;
}

@fragment
fn fs_split(in: VsOut) -> @location(0) vec4<f32> {
    if (in.pos.x < 32.0) {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(0.0, 0.0, 1.0, 1.0);
}

@fragment
fn fs_green(_in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
"#;

#[test]
fn stacked_cameras_preserve_color_and_depth_for_nothing_and_depth_only() {
    let Some((device, queue)) = adapter_device() else {
        logger::info!("camera clear stack golden skipped: no wgpu adapter");
        return;
    };
    let renderer = StackRenderer::new(&device);
    assert_stacked(
        &device,
        &queue,
        &renderer,
        CameraClearMode::Nothing,
        "camera_clear_stack_nothing.png",
        expected_nothing(),
    );
    assert_stacked(
        &device,
        &queue,
        &renderer,
        CameraClearMode::Depth,
        "camera_clear_stack_depth.png",
        expected_depth(),
    );
    let skybox = render_second_camera(&device, &queue, &renderer, CameraClearMode::Skybox);
    assert_eq!(skybox, expected_skybox());
}

#[test]
fn nothing_camera_over_double_buffered_texture_seeds_the_history_image() {
    let Some((device, queue)) = adapter_device() else {
        logger::info!("double-buffer clear seed skipped: no wgpu adapter");
        return;
    };
    let renderer = StackRenderer::new(&device);
    let write = color_texture(&device, "camera-clear-write-image");
    clear_solid(&device, &queue, &write, MAGENTA_CLEAR);
    renderer.prefill(&device, &queue);
    let sampling = Some(crate::frame_contract::RenderTextureSelfSampling::AllowPreviousContents);
    let source = preserved_color_texture(sampling, Some(&write), Some(&renderer.frame_color))
        .expect("double-buffered seed source");
    assert!(std::ptr::eq(
        std::ptr::from_ref(source),
        std::ptr::from_ref(&renderer.frame_color),
    ));
    let loads = forward_clear_loads(
        FrameViewClear {
            mode: CameraClearMode::Nothing,
            color: glam::Vec4::ZERO,
        },
        ScenePreserveQuery {
            sample_count: 1,
            has_color_texture: true,
            surface_format: wgpu::TextureFormat::Rgba8Unorm,
            scene_color_format: wgpu::TextureFormat::Rgba8Unorm,
            self_sampling: sampling,
            has_history_color: true,
            persistent_depth_target: true,
        },
    );
    assert!(loads.seed_color);
    assert!(matches!(loads.color, wgpu::LoadOp::Load));
    assert!(matches!(loads.depth, wgpu::LoadOp::Load));
    renderer.poison_scene_color(&device, &queue);
    renderer.copy_source(&device, &queue, source);
    renderer.draw_overlay(&device, &queue, loads.color, loads.depth);
    let actual = renderer.read_scene_color(&device, &queue);
    let write_pixels = read_color(&device, &queue, &write);
    assert_eq!(write_pixels, fill(|_x, _y| MAGENTA));
    assert_eq!(actual, expected_nothing());
    assert_ne!(actual, write_pixels);
}

fn assert_stacked(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &StackRenderer,
    mode: CameraClearMode,
    golden_name: &str,
    expected: Vec<u8>,
) {
    let actual = render_second_camera(device, queue, renderer, mode);
    assert_eq!(actual, expected, "{golden_name}");
    assert_golden(golden_name, &actual);
}

fn render_second_camera(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &StackRenderer,
    mode: CameraClearMode,
) -> Vec<u8> {
    let clear = FrameViewClear {
        mode,
        color: glam::Vec4::ZERO,
    };
    let loads = forward_clear_loads(
        clear,
        ScenePreserveQuery {
            sample_count: 1,
            has_color_texture: true,
            surface_format: wgpu::TextureFormat::Rgba8Unorm,
            scene_color_format: wgpu::TextureFormat::Rgba8Unorm,
            self_sampling: Some(crate::frame_contract::RenderTextureSelfSampling::Suppress),
            has_history_color: false,
            persistent_depth_target: true,
        },
    );
    let seed = loads.seed_color;
    let color_load = loads.color;
    let depth_load = loads.depth;

    renderer.prefill(device, queue);
    renderer.poison_scene_color(device, queue);
    if seed {
        renderer.copy_preserved_color(device, queue);
    }
    renderer.draw_overlay(device, queue, color_load, depth_load);
    renderer.read_scene_color(device, queue)
}

struct StackRenderer {
    frame_color: wgpu::Texture,
    scene_color: wgpu::Texture,
    depth: wgpu::Texture,
    prefill: wgpu::RenderPipeline,
    overlay: wgpu::RenderPipeline,
}

impl StackRenderer {
    fn new(device: &wgpu::Device) -> Self {
        let frame_color = color_texture(device, "camera-clear-frame");
        let scene_color = color_texture(device, "camera-clear-scene");
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("camera-clear-depth"),
            size: extent(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("camera-clear-stack"),
            source: wgpu::ShaderSource::Wgsl(STACK_WGSL.into()),
        });
        let prefill = pipeline(
            device,
            &shader,
            "vs_prefill",
            "fs_split",
            wgpu::CompareFunction::Always,
        );
        let overlay = pipeline(
            device,
            &shader,
            "vs_overlay",
            "fs_green",
            wgpu::CompareFunction::Greater,
        );
        Self {
            frame_color,
            scene_color,
            depth,
            prefill,
            overlay,
        }
    }

    fn prefill(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let color_view = self
            .frame_color
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("camera-clear-prefill"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("camera-clear-prefill"),
                color_attachments: &[Some(color_attachment(
                    &color_view,
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                ))],
                depth_stencil_attachment: Some(depth_attachment(
                    &depth_view,
                    wgpu::LoadOp::Clear(0.0),
                )),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.prefill);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    fn poison_scene_color(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let view = self
            .scene_color
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("camera-clear-poison"),
        });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("camera-clear-poison"),
                color_attachments: &[Some(color_attachment(&view, wgpu::LoadOp::Clear(YELLOW)))],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    fn copy_preserved_color(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.copy_source(device, queue, &self.frame_color);
    }

    fn copy_source(&self, device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::Texture) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("camera-clear-preserve"),
        });
        copy_preserved_scene_color(&mut encoder, source, &self.scene_color, (WIDTH, HEIGHT));
        queue.submit(std::iter::once(encoder.finish()));
    }

    fn draw_overlay(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_load: wgpu::LoadOp<wgpu::Color>,
        depth_load: wgpu::LoadOp<f32>,
    ) {
        let color_view = self
            .scene_color
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("camera-clear-overlay"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("camera-clear-overlay"),
                color_attachments: &[Some(color_attachment(&color_view, color_load))],
                depth_stencil_attachment: Some(depth_attachment(&depth_view, depth_load)),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.overlay);
            pass.set_scissor_rect(SCISSOR_X, 0, SCISSOR_W, HEIGHT);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    fn read_scene_color(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        read_color(device, queue, &self.scene_color)
    }
}

fn clear_solid(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    color: wgpu::Color,
) {
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("camera-clear-solid"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("camera-clear-solid"),
            color_attachments: &[Some(color_attachment(&view, wgpu::LoadOp::Clear(color)))],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
    queue.submit(std::iter::once(encoder.finish()));
}

fn color_texture(device: &wgpu::Device, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: extent(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    vertex_entry: &str,
    fragment_entry: &str,
    depth_compare: wgpu::CompareFunction,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("camera-clear-stack"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("camera-clear-stack"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(true),
            depth_compare: Some(depth_compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn color_attachment(
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        resolve_target: None,
        ops: wgpu::Operations {
            load,
            store: wgpu::StoreOp::Store,
        },
        depth_slice: None,
    }
}

fn depth_attachment(
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<f32>,
) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load,
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    }
}

fn extent() -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: WIDTH,
        height: HEIGHT,
        depth_or_array_layers: 1,
    }
}

fn expected_nothing() -> Vec<u8> {
    fill(|x, _y| if x < HALF_X { RED } else { BLUE })
}

fn expected_depth() -> Vec<u8> {
    fill(|x, _y| band_color(x, RED, GREEN, BLUE))
}

fn expected_skybox() -> Vec<u8> {
    fill(|x, _y| band_color(x, BLACK, GREEN, BLACK))
}

fn band_color(x: u32, left: [u8; 4], center: [u8; 4], right: [u8; 4]) -> [u8; 4] {
    if x < SCISSOR_X {
        left
    } else if x < SCISSOR_X + SCISSOR_W {
        center
    } else {
        right
    }
}

fn fill(pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            rgba.extend_from_slice(&pixel(x, y));
        }
    }
    rgba
}

fn read_color(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let bytes_per_row = WIDTH * 4;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("camera-clear-readback"),
        size: u64::from(bytes_per_row * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("camera-clear-readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(HEIGHT),
            },
        },
        extent(),
    );
    queue.submit(std::iter::once(encoder.finish()));
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll camera clear readback");
    receiver
        .recv()
        .expect("camera clear readback callback")
        .expect("map camera clear readback");
    let mapped = slice.get_mapped_range().expect("camera clear mapped range");
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    bytes
}

fn adapter_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapter = adapters.into_iter().next()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("camera-clear-stack"),
        ..Default::default()
    }))
    .ok()
}

fn assert_golden(name: &str, rgba: &[u8]) {
    let path = golden_path(name);
    if std::env::var_os("RENDERIDE_WRITE_CLEAR_GOLDENS").is_some() {
        let image = image::RgbaImage::from_raw(WIDTH, HEIGHT, rgba.to_vec())
            .expect("camera clear golden dimensions");
        image.save(&path).expect("write camera clear golden");
        return;
    }
    let image = image::open(&path)
        .unwrap_or_else(|err| panic!("open {name} at {}: {err}", path.display()))
        .into_rgba8();
    assert_eq!(image.dimensions(), (WIDTH, HEIGHT), "{name}");
    assert_eq!(image.as_raw(), rgba, "{name}");
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../renderide-test/goldens")
        .join(name)
}
