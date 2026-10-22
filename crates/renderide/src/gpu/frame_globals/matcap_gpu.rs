//! Lavapipe (or any other Vulkan adapter) check that the composed Matcap UV helper matches
//! the CPU per-eye uniform oracle.
//!
//! This is not an OpenXR swapchain test. It composes `frame/globals.wgsl` twice — mono and
//! multiview — and dispatches the same world normal / world position through both permutations.

use std::path::PathBuf;

use glam::{Mat4, Vec3};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue, ShaderLanguage,
    ShaderType,
};

use super::matcap::{frame_from_eye_views, matcap_uv};
use super::uniforms::FrameGpuUniforms;
use crate::camera::apply_view_handedness_fix;
use crate::gpu::instance_setup::required_limits_for_adapter;

const PROBE_WGSL: &str = r#"
#import renderide::frame::globals as rg

struct MatcapQuery {
    world_n: vec3<f32>,
    view_layer: u32,
    world_pos: vec3<f32>,
    _pad: u32,
}

@group(1) @binding(0) var<storage, read> queries: array<MatcapQuery>;
@group(1) @binding(1) var<storage, read_write> uvs: array<vec4<f32>>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let query = queries[id.x];
    let n_view = rg::world_to_view_normal_for_view(query.world_n, query.view_layer);
    let uv = rg::matcap_uv_for_view(n_view, query.world_pos, query.view_layer);
    uvs[id.x] = vec4<f32>(uv, 0.0, 0.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QueryGpu {
    world_n: [f32; 3],
    view_layer: u32,
    world_pos: [f32; 3],
    _pad: u32,
}

struct ProbeCase {
    world_n: Vec3,
    world_pos: Vec3,
    view_layer: u32,
    orthographic: bool,
}

#[test]
fn composed_multiview_matcap_uv_uses_the_view_ray_cross() {
    let multiview = compose_probe(true).expect("multiview probe");
    let mono = compose_probe(false).expect("mono probe");
    assert!(
        multiview.contains("cross("),
        "multiview Matcap UV must perspective-correct with a view-ray cross product"
    );
    assert!(
        !mono.contains("cross("),
        "mono Matcap UV must stay on the view-normal XY lookup so cameras do not change"
    );
}

#[test]
fn lavapipe_matcap_uv_matches_per_eye_uniform_rows() {
    let Some(device) = open_probe_device() else {
        assert!(
            std::env::var_os("CI").is_none(),
            "CI must expose a Vulkan adapter (lavapipe) for the Matcap stereo UV probe"
        );
        eprintln!("skipping Matcap stereo UV probe: no Vulkan adapter");
        return;
    };

    let left =
        apply_view_handedness_fix(Mat4::from_translation(Vec3::new(-0.032, 0.0, 0.0)).inverse());
    let right = Mat4::from_rotation_y(-0.15)
        * apply_view_handedness_fix(Mat4::from_translation(Vec3::new(0.032, 0.05, 0.0)).inverse());
    let normal = Vec3::new(0.0, 0.0, -1.0);
    let point = Vec3::new(0.0, 0.0, 1.0);
    let up = Vec3::Y;
    let cases = [
        ProbeCase {
            world_n: normal,
            world_pos: point,
            view_layer: 0,
            orthographic: false,
        },
        ProbeCase {
            world_n: normal,
            world_pos: point,
            view_layer: 1,
            orthographic: false,
        },
        ProbeCase {
            world_n: up,
            world_pos: Vec3::new(-0.032, 0.0, 1.25),
            view_layer: 0,
            orthographic: false,
        },
        ProbeCase {
            world_n: normal,
            world_pos: Vec3::new(0.4, -0.25, 1.0),
            view_layer: 0,
            orthographic: true,
        },
        ProbeCase {
            world_n: normal,
            world_pos: Vec3::new(0.4, -0.25, 1.0),
            view_layer: 1,
            orthographic: true,
        },
    ];

    let perspective = frame_from_eye_views(left, right, false);
    let orthographic = frame_from_eye_views(left, right, true);
    for multiview in [false, true] {
        let got = dispatch_probe(&device, multiview, &perspective, &orthographic, &cases);
        for (case, uv) in cases.iter().zip(got) {
            let frame = if case.orthographic {
                &orthographic
            } else {
                &perspective
            };
            let expected = matcap_uv(
                frame,
                case.world_n,
                case.world_pos,
                case.view_layer,
                multiview,
            );
            let delta = (glam::Vec2::new(uv[0], uv[1]) - expected).length();
            assert!(
                delta < 2e-4,
                "multiview={multiview} layer={} ortho={} gpu {uv:?} != cpu {expected:?} (delta {delta})",
                case.view_layer,
                case.orthographic
            );
        }
    }
}

fn compose_probe(multiview: bool) -> Result<String, String> {
    let mut composer = Composer::default().with_capabilities(Capabilities::all());
    for relative in [
        "shaders/modules/frame/types.wgsl",
        "shaders/modules/frame/globals.wgsl",
    ] {
        add_shader_module(&mut composer, relative)?;
    }
    let module = make_probe_module(&mut composer, multiview)?;
    assert!(
        module
            .entry_points
            .iter()
            .any(|entry| entry.name == "main" && entry.stage == naga::ShaderStage::Compute),
        "matcap probe must keep the compute entry point `main`"
    );
    assert_probe_bindings(&module);
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = validator
        .validate(&module)
        .map_err(|err| format!("validate matcap probe: {err}"))?;
    naga::back::wgsl::write_string(
        &module,
        &info,
        naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
    )
    .map_err(|err| format!("wgsl out: {err}"))
}

fn add_shader_module(composer: &mut Composer, relative: &str) -> Result<(), String> {
    let source = read_shader(relative);
    let added = composer.add_composable_module(ComposableModuleDescriptor {
        source: &source,
        file_path: relative,
        language: ShaderLanguage::Wgsl,
        ..Default::default()
    });
    if let Err(err) = added {
        return Err(err.emit_to_string(composer));
    }
    Ok(())
}

fn make_probe_module(composer: &mut Composer, multiview: bool) -> Result<naga::Module, String> {
    let mut shader_defs = std::collections::HashMap::new();
    if multiview {
        shader_defs.insert("MULTIVIEW".to_string(), ShaderDefValue::Bool(true));
    }
    match composer.make_naga_module(NagaModuleDescriptor {
        source: PROBE_WGSL,
        file_path: "matcap_uv_probe.wgsl",
        shader_type: ShaderType::Wgsl,
        shader_defs,
        ..Default::default()
    }) {
        Ok(module) => Ok(module),
        Err(err) => Err(err.emit_to_string(composer)),
    }
}

fn assert_probe_bindings(module: &naga::Module) {
    let mut bindings = Vec::new();
    for (_, var) in module.global_variables.iter() {
        if let Some(binding) = var.binding {
            bindings.push((binding.group, binding.binding, var.space));
        }
    }
    bindings.sort_by_key(|(group, binding, _)| (*group, *binding));
    assert_eq!(
        bindings.len(),
        3,
        "Matcap probe should keep the frame uniform plus two storage buffers, got {bindings:?}"
    );
    assert!(
        matches!(bindings[0].2, naga::AddressSpace::Uniform),
        "group 0 binding 0 must be the frame uniform, got {bindings:?}"
    );
    assert_eq!(bindings[0].0, 0);
    assert_eq!(bindings[0].1, 0);
}

fn read_shader(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

struct ProbeDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn open_probe_device() -> Option<ProbeDevice> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    let mut ordered = adapters;
    ordered.sort_by_key(|adapter| match adapter.get_info().device_type {
        wgpu::DeviceType::Cpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::DiscreteGpu => 2,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Other => 4,
    });
    for adapter in ordered {
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("matcap-uv-probe"),
            required_features: wgpu::Features::empty(),
            required_limits: required_limits_for_adapter(&adapter),
            ..Default::default()
        };
        if let Ok((device, queue)) = pollster::block_on(adapter.request_device(&descriptor)) {
            return Some(ProbeDevice { device, queue });
        }
    }
    None
}

struct ProbePipeline {
    pipeline: wgpu::ComputePipeline,
    frame_layout: wgpu::BindGroupLayout,
    query_layout: wgpu::BindGroupLayout,
}

struct ProbeBuffers {
    queries: Vec<QueryGpu>,
    query_buffer: wgpu::Buffer,
    uv_buffer: wgpu::Buffer,
    staging: wgpu::Buffer,
}

fn dispatch_probe(
    gpu: &ProbeDevice,
    multiview: bool,
    perspective: &FrameGpuUniforms,
    orthographic: &FrameGpuUniforms,
    cases: &[ProbeCase],
) -> Vec<[f32; 4]> {
    let pipeline = create_probe_pipeline(gpu, multiview);
    let buffers = create_probe_buffers(gpu, cases);
    let mut output = vec![[0.0_f32; 4]; cases.len()];
    // The orthographic flag lives in the frame uniform, so each projection mode dispatches
    // against the uniform that carries that flag.
    record_projection_pass(
        gpu,
        &pipeline,
        &buffers,
        cases,
        perspective,
        false,
        &mut output,
    );
    record_projection_pass(
        gpu,
        &pipeline,
        &buffers,
        cases,
        orthographic,
        true,
        &mut output,
    );
    output
}

fn create_probe_pipeline(gpu: &ProbeDevice, multiview: bool) -> ProbePipeline {
    let wgsl = compose_probe(multiview).expect("compose probe");
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matcap-uv-probe"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
    let frame_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matcap-uv-frame"),
            entries: &[uniform_entry()],
        });
    let query_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matcap-uv-queries"),
            entries: &[storage_entry(0, true), storage_entry(1, false)],
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matcap-uv-probe"),
            bind_group_layouts: &[Some(&frame_layout), Some(&query_layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("matcap-uv-probe"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
    ProbePipeline {
        pipeline,
        frame_layout,
        query_layout,
    }
}

fn create_probe_buffers(gpu: &ProbeDevice, cases: &[ProbeCase]) -> ProbeBuffers {
    let queries: Vec<QueryGpu> = cases
        .iter()
        .map(|case| QueryGpu {
            world_n: case.world_n.to_array(),
            view_layer: case.view_layer,
            world_pos: case.world_pos.to_array(),
            _pad: 0,
        })
        .collect();
    let query_bytes = bytemuck::cast_slice(&queries);
    let query_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("matcap-uv-queries"),
        size: query_bytes.len() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue.write_buffer(&query_buffer, 0, query_bytes);
    let uv_size = (cases.len() * size_of::<[f32; 4]>()) as u64;
    let uv_buffer = storage_buffer(gpu, "matcap-uv-output", uv_size);
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("matcap-uv-staging"),
        size: uv_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ProbeBuffers {
        queries,
        query_buffer,
        uv_buffer,
        staging,
    }
}

fn record_projection_pass(
    gpu: &ProbeDevice,
    pipeline: &ProbePipeline,
    buffers: &ProbeBuffers,
    cases: &[ProbeCase],
    frame: &FrameGpuUniforms,
    orthographic: bool,
    output: &mut [[f32; 4]],
) {
    let indices: Vec<usize> = cases
        .iter()
        .enumerate()
        .filter_map(|(index, case)| (case.orthographic == orthographic).then_some(index))
        .collect();
    if indices.is_empty() {
        return;
    }
    let pass_queries: Vec<QueryGpu> = indices
        .iter()
        .map(|index| buffers.queries[*index])
        .collect();
    gpu.queue.write_buffer(
        &buffers.query_buffer,
        0,
        bytemuck::cast_slice(&pass_queries),
    );
    let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("matcap-uv-frame"),
        size: size_of::<FrameGpuUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue
        .write_buffer(&frame_buffer, 0, bytemuck::bytes_of(frame));
    let frame_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("matcap-uv-frame"),
        layout: &pipeline.frame_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: frame_buffer.as_entire_binding(),
        }],
    });
    let query_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("matcap-uv-queries"),
        layout: &pipeline.query_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.query_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers.uv_buffer.as_entire_binding(),
            },
        ],
    });
    let written = (pass_queries.len() * size_of::<[f32; 4]>()) as u64;
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("matcap-uv-probe"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("matcap-uv-probe"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &frame_group, &[]);
        pass.set_bind_group(1, &query_group, &[]);
        pass.dispatch_workgroups(pass_queries.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&buffers.uv_buffer, 0, &buffers.staging, 0, written);
    gpu.queue.submit(std::iter::once(encoder.finish()));
    let values = read_staging_prefix(gpu, &buffers.staging, written);
    for (slot, index) in indices.iter().enumerate() {
        output[*index] = values[slot];
    }
}

fn read_staging_prefix(gpu: &ProbeDevice, staging: &wgpu::Buffer, written: u64) -> Vec<[f32; 4]> {
    let slice = staging.slice(..written);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("matcap probe device poll");
    receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("matcap probe map")
        .expect("matcap probe map result");
    let mapped = slice.get_mapped_range();
    let values: Vec<[f32; 4]> = bytemuck::cast_slice(&mapped).to_vec();
    drop(mapped);
    staging.unmap();
    values
}

fn storage_buffer(gpu: &ProbeDevice, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn uniform_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
