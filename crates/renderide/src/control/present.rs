//! Presented-frame accounting shared by the headless and windowed drivers.

use std::collections::BTreeMap;

use renderide_control_client::{CpuPhaseMs, FrameStats, MetaInfo};

use crate::build_info::renderer_commit_sha8;
use crate::gpu::GpuContext;

use super::probe::{self, FrameProbe};
use super::session::ControlSession;

/// Whether a headless tick produced a presented frame.
///
/// Lockstep-only ticks and skipped full frames do not count. `frames.wait`
/// uses the same predicate the headless driver uses before it records a sample.
pub(crate) fn headless_tick_presents(full_frame: bool, render_skipped: bool) -> bool {
    full_frame && !render_skipped
}

/// Records one presented frame, then serves a pending PNG capture against it.
pub(crate) fn finish_presented_frame(
    session: &ControlSession,
    gpu: &GpuContext,
    cpu_frame_ms: f64,
) {
    let probe = probe::take();
    let stats = stats_from_probe(cpu_frame_ms, probe, gpu_pass_ms(gpu));
    let frame_index = session.note_presented(stats);
    service_capture(session, gpu, frame_index);
}

/// Builds the one-shot `meta` payload from the live device.
pub(crate) fn meta_from_gpu(gpu: &GpuContext) -> MetaInfo {
    let info = gpu.adapter_info();
    let (width, height) = gpu.surface_extent_px();
    MetaInfo {
        adapter: field_or_unreported(&info.name),
        driver: driver_label(&info.driver, &info.driver_info),
        backend: format!("{:?}", info.backend),
        present_mode: format!("{:?}", gpu.present_mode()),
        resolution: <[u32; 2]>::from((width, height)),
        git_sha: renderer_commit_sha8().unwrap_or("unavailable").to_owned(),
    }
}

fn stats_from_probe(
    cpu_frame_ms: f64,
    probe: FrameProbe,
    gpu_pass_ms: Option<BTreeMap<String, f64>>,
) -> FrameStats {
    FrameStats {
        frame_index: 0,
        cpu_frame_ms,
        cpu_phase_ms: CpuPhaseMs {
            draw_prep: probe.draw_prep_ms,
            cull: probe.cull_ms,
            upload: probe.upload_ms,
            submit: probe.submit_ms,
        },
        gpu_pass_ms,
        draw_calls: probe.draw_calls,
        instances: probe.instances,
        upload_count: probe.upload_count,
        upload_bytes: probe.upload_bytes,
        triangles: probe.triangles,
    }
}

fn gpu_pass_ms(gpu: &GpuContext) -> Option<BTreeMap<String, f64>> {
    if !gpu
        .device()
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY)
    {
        return None;
    }
    let handle = gpu.latest_gpu_profiler_snapshot_handle();
    let Ok(snapshot) = handle.lock() else {
        return Some(BTreeMap::new());
    };
    let mut passes = BTreeMap::new();
    for entry in &snapshot.entries {
        let ms = f64::from(entry.ms);
        passes
            .entry(entry.name.clone())
            .and_modify(|total| *total += ms)
            .or_insert(ms);
    }
    Some(passes)
}

fn service_capture(session: &ControlSession, gpu: &GpuContext, frame_index: u64) {
    let Some(job) = session.take_capture() else {
        return;
    };
    let result = match gpu.control_color_texture() {
        Some(texture) => crate::app::headless_readback_png(gpu, texture, job.path())
            .map(|()| frame_index)
            .map_err(|err| err.to_string()),
        None => Err("color target is not ready".to_owned()),
    };
    job.complete(result);
}

fn driver_label(driver: &str, driver_info: &str) -> String {
    if !driver.trim().is_empty() {
        driver.to_owned()
    } else {
        field_or_unreported(driver_info)
    }
}

fn field_or_unreported(value: &str) -> String {
    if value.trim().is_empty() {
        "unreported".to_owned()
    } else {
        value.to_owned()
    }
}
