//! Lavapipe coverage for stacked double-buffered cameras and a shared portal.
//!
//! The stacked test plans two cameras, then seeds the second through
//! [`forward_clear_loads_for_view`] and the color-preserve copy. The portal test plans a mirror
//! before that swap and checks the published handles were rewritten.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use super::super::super::RendererRuntime;
use super::super::view_plan::{FrameViewPlan, FrameViewPlanTarget, OffscreenTargetHandles};
use crate::camera::{CameraClipPlanes, EyeView, ViewId};
use crate::config::{RendererSettings, RendererSettingsHandle};
use crate::connection::ConnectionParams;
use crate::frame_contract::ViewPostProcessing;
use crate::graph_inputs::GraphPassFrameView;
use crate::occlusion::gpu::HiZGpuState;
use crate::passes::{
    copy_preserved_scene_color, forward_clear_loads_for_view, preserved_color_texture,
};
use crate::scene::{CameraRenderableEntry, RenderSpaceId, SceneCoordinator, StaticMeshRenderer};
use crate::shared::{CameraClearMode, CameraState, RenderRect, RenderTransform};

const TEXTURE_W: u32 = 32;
const TEXTURE_H: u32 = 2;
const ASSET: i32 = 11;
const SPACE: RenderSpaceId = RenderSpaceId(77);

fn build_runtime() -> RendererRuntime {
    let settings: RendererSettingsHandle =
        Arc::new(std::sync::RwLock::new(RendererSettings::default()));
    RendererRuntime::new(
        Option::<ConnectionParams>::None,
        settings,
        PathBuf::from("test_config.toml"),
    )
}

fn open_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapter = adapters.into_iter().next()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("double-buffer-seed"),
        ..Default::default()
    }))
    .ok()
}

fn full_viewport() -> RenderRect {
    RenderRect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    }
}

fn scene_camera(
    depth: f32,
    clear_mode: CameraClearMode,
    renderable_index: i32,
    double_buffered: bool,
) -> CameraRenderableEntry {
    let mut flags = 1_u16;
    if double_buffered {
        flags |= 1 << 2;
    }
    CameraRenderableEntry {
        renderable_index,
        transform_id: 0,
        state: CameraState {
            flags,
            render_texture_asset_id: ASSET,
            depth,
            clear_mode,
            viewport: full_viewport(),
            ..CameraState::default()
        },
        selective_transform_ids: Vec::new(),
        exclude_transform_ids: Vec::new(),
    }
}

fn double_buffered_camera(
    depth: f32,
    clear_mode: CameraClearMode,
    renderable_index: i32,
) -> CameraRenderableEntry {
    scene_camera(depth, clear_mode, renderable_index, true)
}

fn seed_space(scene: &mut SceneCoordinator) {
    scene.test_seed_space_identity_worlds(
        SPACE,
        vec![RenderTransform {
            position: glam::Vec3::ZERO,
            scale: glam::Vec3::ONE,
            rotation: glam::Quat::IDENTITY,
        }],
        vec![-1],
    );
    scene.test_set_space_active(SPACE, true);
}

fn insert_texture(runtime: &mut RendererRuntime, device: &wgpu::Device) {
    let limits = crate::gpu::GpuLimits::synthetic_for_tests(
        wgpu::Limits {
            max_texture_dimension_2d: 8192,
            ..Default::default()
        },
        wgpu::Features::empty(),
        hashbrown::HashMap::new(),
    );
    let mut fmt = crate::gpu_pools::test_support::render_texture_format(
        crate::shared::TextureWrapMode::Clamp,
        crate::shared::TextureWrapMode::Clamp,
    );
    fmt.asset_id = ASSET;
    fmt.size = glam::IVec2::new(TEXTURE_W as i32, TEXTURE_H as i32);
    let texture = crate::gpu_pools::GpuRenderTexture::new_from_format(device, &limits, &fmt)
        .expect("render texture");
    runtime
        .backend
        .asset_transfers
        .render_texture_pool_mut()
        .insert(texture);
}

fn aim_host_camera(runtime: &mut RendererRuntime) {
    let clip = CameraClipPlanes::new(0.05, 500.0);
    let position = glam::Vec3::new(0.0, 0.0, 5.0);
    let view = glam::Mat4::from_translation(-position);
    let proj = crate::camera::reverse_z_perspective(
        crate::camera::Viewport::new(1280, 720).aspect(),
        60f32.to_radians(),
        clip.near,
        clip.far,
    );
    runtime.host_camera.clip = clip;
    runtime.host_camera.explicit_view = Some(EyeView::new(view, proj, proj * view, position));
}

fn plan_offscreen<'a>(
    runtime: &mut RendererRuntime,
    device: &wgpu::Device,
) -> Vec<FrameViewPlan<'a>> {
    runtime.collect_offscreen_scene_views_with_device(device, (1280, 720))
}

fn offscreen<'a>(plan: &'a FrameViewPlan<'_>) -> &'a OffscreenTargetHandles {
    let FrameViewPlanTarget::Offscreen(handles) = &plan.target else {
        panic!("offscreen plan");
    };
    handles
}

fn view_from_plan<'a>(plan: &'a FrameViewPlan<'_>) -> GraphPassFrameView<'a> {
    let handles = offscreen(plan);
    GraphPassFrameView {
        depth_texture: &handles.depth_texture,
        depth_view: &handles.depth_view,
        depth_sample_view: None,
        surface_format: handles.color_format,
        scene_color_format: handles.color_format,
        viewport_px: plan.viewport_px,
        host_camera: plan.host_camera,
        render_context: plan.render_context,
        frame_time_seconds: plan.frame_time_seconds,
        multiview_stereo: false,
        offscreen_write_target: handles.write_target,
        view_winding: plan.view_winding,
        view_id: plan.view_id,
        hi_z_slot: Arc::new(Mutex::new(HiZGpuState::default())),
        sample_count: 1,
        gpu_limits: None,
        msaa_depth_resolve: None,
        clear: plan.clear,
        color_texture: Some(&handles.color_texture),
        history_color_texture: handles.history_color_texture.as_ref(),
        seed_published_color: handles.seed_published_color,
        persistent_depth_target: true,
        clear_load_inputs: full_offscreen_clear_load_inputs(
            plan.viewport_px,
            handles.color_format,
            handles.write_target,
        ),
        post_processing: ViewPostProcessing::disabled(),
    }
}

/// Full-target offscreen inputs for a test view that always attaches color and depth.
fn full_offscreen_clear_load_inputs(
    viewport_px: (u32, u32),
    surface_format: wgpu::TextureFormat,
    write_target: crate::frame_contract::OffscreenWriteTarget,
) -> crate::render_graph::compiled::ClearLoadDecisionInputs {
    use crate::camera::CameraRenderRect;
    use crate::render_graph::compiled::{
        ClearLoadViewSource, FrameViewTargetKind, clear_load_inputs_from_view,
    };
    clear_load_inputs_from_view(ClearLoadViewSource {
        kind: FrameViewTargetKind::OffscreenRt,
        viewport: CameraRenderRect {
            origin_px: (0, 0),
            extent_px: viewport_px,
        },
        target_extent_px: viewport_px,
        copies_partial_color: false,
        sample_count: 1,
        surface_format,
        scene_color_format: surface_format,
        write_target,
    })
}

fn clear_color(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    color: wgpu::Color,
) {
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("double-buffer-seed-clear"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("double-buffer-seed-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
    queue.submit(std::iter::once(encoder.finish()));
}

fn scene_target(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("double-buffer-seed-scene"),
        size: wgpu::Extent3d {
            width: TEXTURE_W,
            height: TEXTURE_H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn read_color(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let bytes_per_row = 256_u32;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("double-buffer-seed-readback"),
        size: u64::from(bytes_per_row) * u64::from(TEXTURE_H),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("double-buffer-seed-readback"),
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
                rows_per_image: Some(TEXTURE_H),
            },
        },
        wgpu::Extent3d {
            width: TEXTURE_W,
            height: TEXTURE_H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll seed readback");
    receiver
        .recv()
        .expect("seed readback callback")
        .expect("map seed readback");
    let mapped = slice.get_mapped_range().expect("seed mapped range");
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    bytes
}

fn record_preserve(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &GraphPassFrameView<'_>,
    destination: &wgpu::Texture,
) {
    let source = preserved_color_texture(view).expect("preserve source");
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("double-buffer-seed-preserve"),
    });
    copy_preserved_scene_color(&mut encoder, source, destination, view.viewport_px);
    queue.submit(std::iter::once(encoder.finish()));
}

fn paint_history_and_published(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    history: &wgpu::Texture,
    published: &wgpu::Texture,
) {
    clear_color(
        device,
        queue,
        history,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
    );
    clear_color(
        device,
        queue,
        published,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
}

fn assert_preserved_is_published(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    plan: &FrameViewPlan<'_>,
    history: &wgpu::Texture,
) {
    let view = view_from_plan(plan);
    let loads = forward_clear_loads_for_view(&view);
    assert!(loads.seed_color);
    assert!(matches!(loads.color, wgpu::LoadOp::Load));
    let published = view.color_texture.expect("published color");
    let scene = scene_target(device);
    record_preserve(device, queue, &view, &scene);
    let preserved = read_color(device, queue, &scene);
    assert_eq!(preserved, read_color(device, queue, published));
    assert_ne!(preserved, read_color(device, queue, history));
}

fn assert_cookie_samples_history(runtime: &RendererRuntime) {
    let texture = runtime
        .backend
        .render_texture_pool()
        .get(ASSET)
        .expect("resident");
    let early: *const wgpu::TextureView = texture.color_view_for_early_sampling();
    let history = texture.color_view_for_sampling(true);
    let published: *const wgpu::TextureView = texture.color_view.as_ref();
    assert_eq!(early, Arc::as_ptr(&history));
    assert_ne!(early, published);
}

fn assert_cookie_samples_published(runtime: &RendererRuntime) {
    let texture = runtime
        .backend
        .render_texture_pool()
        .get(ASSET)
        .expect("resident");
    let early: *const wgpu::TextureView = texture.color_view_for_early_sampling();
    let published: *const wgpu::TextureView = texture.color_view.as_ref();
    assert_eq!(early, published);
}

/// The second Nothing camera keeps the image the first camera is writing this frame.
#[test]
fn stacked_double_buffered_nothing_preserves_this_frame_write() {
    let Some((device, queue)) = open_device() else {
        logger::info!("stacked double-buffer seed skipped: no adapter");
        return;
    };
    let mut runtime = build_runtime();
    insert_texture(&mut runtime, &device);
    seed_space(&mut runtime.scene);
    runtime.scene.test_push_cameras(
        SPACE,
        [
            double_buffered_camera(0.0, CameraClearMode::Skybox, 0),
            double_buffered_camera(1.0, CameraClearMode::Nothing, 1),
        ],
    );

    let plans = plan_offscreen(&mut runtime, &device);
    assert_eq!(plans.len(), 2);
    let first = offscreen(&plans[0]);
    let second = offscreen(&plans[1]);
    assert!(first.history_color_texture.is_some());
    assert!(!first.seed_published_color);
    assert!(second.history_color_texture.is_none());
    assert!(second.seed_published_color);
    assert_eq!(&first.color_texture, &second.color_texture);
    assert_eq!(plans[0].clear.mode, CameraClearMode::Skybox);
    assert_eq!(plans[1].clear.mode, CameraClearMode::Nothing);

    let history = first.history_color_texture.clone().expect("history image");
    clear_color(
        &device,
        &queue,
        &history,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
    );
    clear_color(
        &device,
        &queue,
        &second.color_texture,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );

    let view = view_from_plan(&plans[1]);
    let loads = forward_clear_loads_for_view(&view);
    assert!(loads.seed_color);
    assert!(matches!(loads.color, wgpu::LoadOp::Load));
    assert!(matches!(loads.depth, wgpu::LoadOp::Load));
    let scene = scene_target(&device);
    record_preserve(&device, &queue, &view, &scene);
    let preserved = read_color(&device, &queue, &scene);
    let write_pixels = read_color(&device, &queue, &second.color_texture);
    let history_pixels = read_color(&device, &queue, &history);
    assert_eq!(preserved, write_pixels);
    assert_ne!(preserved, history_pixels);
    assert_cookie_samples_history(&runtime);
    runtime
        .backend
        .asset_transfers
        .render_texture_pool_mut()
        .clear_history_completed_frame_flags();
    assert_cookie_samples_published(&runtime);
}

/// A normal Skybox at depth 0 draws this frame. The double-buffered Nothing behind it keeps that
/// output instead of copying history over it.
#[test]
fn normal_skybox_then_double_buffered_nothing_keeps_this_frame() {
    let Some((device, queue)) = open_device() else {
        logger::info!("normal skybox then double-buffer seed skipped: no adapter");
        return;
    };
    let mut runtime = build_runtime();
    insert_texture(&mut runtime, &device);
    seed_space(&mut runtime.scene);
    runtime.scene.test_push_cameras(
        SPACE,
        [
            scene_camera(0.0, CameraClearMode::Skybox, 0, false),
            scene_camera(1.0, CameraClearMode::Nothing, 1, true),
        ],
    );

    let plans = plan_offscreen(&mut runtime, &device);
    assert_eq!(plans.len(), 2);
    let first = offscreen(&plans[0]);
    let second = offscreen(&plans[1]);
    assert!(first.history_color_texture.is_some());
    assert!(!first.seed_published_color);
    assert!(second.history_color_texture.is_none());
    assert!(second.seed_published_color);
    assert_eq!(&first.color_texture, &second.color_texture);

    let sky = view_from_plan(&plans[0]);
    assert!(!forward_clear_loads_for_view(&sky).seed_color);
    assert!(preserved_color_texture(&sky).is_none());

    let history = first.history_color_texture.clone().expect("history image");
    paint_history_and_published(&device, &queue, &history, &second.color_texture);
    assert_preserved_is_published(&device, &queue, &plans[1], &history);
}

/// A normal Nothing camera sorted before the claimer is the first writer, so it seeds history.
#[test]
fn normal_nothing_before_double_buffered_camera_seeds_history() {
    let Some((device, queue)) = open_device() else {
        logger::info!("normal nothing before double-buffer seed skipped: no adapter");
        return;
    };
    let mut runtime = build_runtime();
    insert_texture(&mut runtime, &device);
    seed_space(&mut runtime.scene);
    runtime.scene.test_push_cameras(
        SPACE,
        [
            scene_camera(0.0, CameraClearMode::Nothing, 0, false),
            scene_camera(1.0, CameraClearMode::Skybox, 1, true),
        ],
    );

    let plans = plan_offscreen(&mut runtime, &device);
    assert_eq!(plans.len(), 2);
    let first = offscreen(&plans[0]);
    let second = offscreen(&plans[1]);
    assert!(first.history_color_texture.is_some());
    assert!(!first.seed_published_color);
    assert!(second.history_color_texture.is_none());
    assert!(second.seed_published_color);

    let history = first.history_color_texture.clone().expect("history image");
    paint_history_and_published(&device, &queue, &history, &first.color_texture);
    let view = view_from_plan(&plans[0]);
    let loads = forward_clear_loads_for_view(&view);
    assert!(loads.seed_color);
    assert!(matches!(loads.color, wgpu::LoadOp::Load));
    assert!(matches!(loads.depth, wgpu::LoadOp::Load));
    let scene = scene_target(&device);
    record_preserve(&device, &queue, &view, &scene);
    let preserved = read_color(&device, &queue, &scene);
    assert_eq!(preserved, read_color(&device, &queue, &history));
    assert_ne!(preserved, read_color(&device, &queue, &first.color_texture));
}

/// A mirror planned before the swap writes the post-swap published image.
#[test]
fn portal_on_double_buffered_texture_refreshes_published_handles() {
    let Some((device, queue)) = open_device() else {
        logger::info!("portal double-buffer refresh skipped: no adapter");
        return;
    };
    let mut runtime = build_runtime();
    insert_texture(&mut runtime, &device);
    seed_space(&mut runtime.scene);
    runtime.scene.test_push_static_mesh_renderer(
        SPACE,
        StaticMeshRenderer {
            node_id: 0,
            ..StaticMeshRenderer::default()
        },
    );
    runtime.scene.test_push_camera_portal(
        SPACE,
        0,
        0,
        crate::shared::CameraPortalState {
            mesh_renderer_index: 0,
            plane_normal: glam::Vec3::Z,
            render_texture_id: ASSET,
            ..crate::shared::CameraPortalState::default()
        },
    );
    runtime.scene.test_push_cameras(
        SPACE,
        [double_buffered_camera(0.0, CameraClearMode::Nothing, 2)],
    );
    aim_host_camera(&mut runtime);

    let plans = plan_offscreen(&mut runtime, &device);
    let portal = plans
        .iter()
        .find(|plan| matches!(plan.view_id, ViewId::CameraPortal(_)))
        .expect("portal view");
    let camera = plans
        .iter()
        .find(|plan| matches!(plan.view_id, ViewId::SecondaryCamera(_)))
        .expect("double-buffered camera");
    let published = runtime
        .backend
        .render_texture_pool()
        .get(ASSET)
        .expect("resident")
        .color_texture
        .clone();
    let history = runtime
        .backend
        .render_texture_pool()
        .get(ASSET)
        .expect("resident")
        .history_color_texture()
        .expect("history after the camera swap");
    let portal_color = &offscreen(portal).color_texture;
    let camera_color = &offscreen(camera).color_texture;
    assert_eq!(portal_color, published.as_ref());
    assert_eq!(camera_color, published.as_ref());
    assert_ne!(portal_color, history.as_ref());
    assert!(offscreen(portal).history_color_texture.is_some());
    assert!(!offscreen(portal).seed_published_color);
    assert!(offscreen(camera).history_color_texture.is_none());
    assert!(offscreen(camera).seed_published_color);

    paint_history_and_published(&device, &queue, history.as_ref(), camera_color);
    assert_preserved_is_published(&device, &queue, camera, history.as_ref());
}
