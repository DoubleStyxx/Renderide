//! Frame-benchmark sample recorder.
//!
//! Enabled by `--frame-bench-out`. The process records warmup frames, then measured
//! frames, writes one JSON file, and requests shutdown. GPU pass times are null when
//! the adapter has no `TIMESTAMP_QUERY` support.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::gpu::GpuContext;
use crate::runtime::{RendererRuntime, TickOutcome};

const DEFAULT_WARMUP_FRAMES: u32 = 8;
const DEFAULT_MEASURED_FRAMES: u32 = 32;
const SCHEMA: u32 = 1;

struct Params {
    output_path: PathBuf,
    warmup_frames: u32,
    measured_frames: u32,
}

struct FrameSample {
    cpu_frame_ms: f64,
    draw_calls: u64,
    upload_count: u64,
    upload_bytes: u64,
    gpu_passes_ms: Option<BTreeMap<String, f64>>,
}

struct Recorder {
    params: Params,
    warmup_seen: u32,
    samples: Vec<FrameSample>,
    adapter: AdapterJson,
    finished: bool,
}

static RECORDER: Mutex<Option<Recorder>> = Mutex::new(None);

/// Parses process arguments and arms the recorder when `--frame-bench-out` is set.
pub(crate) fn install_from_args() {
    let args: Vec<String> = std::env::args().collect();
    let Some(params) = parse_frame_bench_args(&args) else {
        return;
    };
    logger::info!(
        "frame bench: output={} warmup={} measured={}",
        params.output_path.display(),
        params.warmup_frames,
        params.measured_frames
    );
    crate::runtime::set_frame_bench_draw_stats(true);
    crate::runtime::set_frame_bench_active(true);
    let mut guard = lock();
    if guard.is_none() {
        *guard = Some(Recorder {
            params,
            warmup_seen: 0,
            samples: Vec::new(),
            adapter: AdapterJson::empty(),
            finished: false,
        });
    }
}

/// Whether a frame-bench output path was requested.
pub(crate) fn active() -> bool {
    lock().is_some()
}

/// Closes one headless full-frame tick and maybe appends a measured sample.
pub(crate) fn finish_headless_full_frame(
    gpu: &GpuContext,
    runtime: &mut RendererRuntime,
    outcome: &TickOutcome,
) {
    let excluded = runtime.drain_frame_timing_excluded_wait();
    gpu.record_frame_timing_excluded_wait(excluded);
    // B3 gate-noise canary slowdown (~80 µs busy-wait). CANARY - DO NOT MERGE.
    {
        let spin_start = std::time::Instant::now();
        while spin_start.elapsed() < std::time::Duration::from_micros(80) {
            std::hint::spin_loop();
        }
    }
    gpu.record_main_thread_cpu_end(std::time::Instant::now());
    gpu.end_frame_timing();
    let record = !outcome.render_skipped && outcome.graph_error.is_none() && !outcome.fatal_error;
    if record {
        observe(gpu, runtime);
    } else {
        let _ = crate::gpu::frame_bench_queries::resolve_pass_ms(gpu);
    }
}

/// Records one windowed or headless frame after CPU timing has been published.
pub(crate) fn observe(gpu: &GpuContext, runtime: &mut RendererRuntime) {
    if !active() {
        return;
    }
    let Some(cpu_frame_ms) = gpu.current_cpu_frame_ms() else {
        let _ = crate::gpu::frame_bench_queries::resolve_pass_ms(gpu);
        return;
    };
    let (draw_calls, upload_count, upload_bytes) = runtime.frame_bench_counters();
    let gpu_passes_ms = crate::gpu::frame_bench_queries::resolve_pass_ms(gpu);
    let adapter = AdapterJson::from_gpu(gpu);
    let sample = FrameSample {
        cpu_frame_ms,
        draw_calls,
        upload_count,
        upload_bytes,
        gpu_passes_ms,
    };
    let mut guard = lock();
    let Some(recorder) = guard.as_mut() else {
        return;
    };
    if recorder.finished {
        return;
    }
    recorder.adapter = adapter;
    // Lockstep ticks before the host scene draws are not frame-time samples. Counting them
    // finishes the window during asset upload and shuts the process down early.
    if recorder.warmup_seen == 0 && recorder.samples.is_empty() && sample.draw_calls == 0 {
        return;
    }
    if recorder.warmup_seen < recorder.params.warmup_frames {
        recorder.warmup_seen += 1;
        return;
    }
    recorder.samples.push(sample);
    if recorder.samples.len()
        < usize::try_from(recorder.params.measured_frames).unwrap_or(usize::MAX)
    {
        return;
    }
    recorder.finished = true;
    let report = recorder.build_report();
    let path = recorder.params.output_path.clone();
    drop(guard);
    if let Err(err) = write_report(&path, &report) {
        logger::error!("frame bench: failed to write {}: {err}", path.display());
    } else {
        logger::info!("frame bench: wrote {}", path.display());
    }
    runtime.request_bench_shutdown();
}

fn lock() -> std::sync::MutexGuard<'static, Option<Recorder>> {
    RECORDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Serialize)]
struct RawReport {
    schema: u32,
    complete: bool,
    warmup_frames: u32,
    measured_frames: u32,
    timestamp_query: bool,
    renderer_commit: String,
    adapter: AdapterJson,
    frames: Vec<FrameJson>,
}

#[derive(Clone, Serialize)]
struct AdapterJson {
    name: String,
    backend: String,
    device_type: String,
    driver: String,
    driver_info: String,
}

impl AdapterJson {
    fn empty() -> Self {
        Self {
            name: String::new(),
            backend: String::new(),
            device_type: String::new(),
            driver: String::new(),
            driver_info: String::new(),
        }
    }

    fn from_gpu(gpu: &GpuContext) -> Self {
        let info = gpu.adapter_info();
        Self {
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            driver: info.driver.clone(),
            driver_info: info.driver_info.clone(),
        }
    }
}

#[derive(Serialize)]
struct FrameJson {
    cpu_frame_ms: f64,
    draw_calls: u64,
    upload_count: u64,
    upload_bytes: u64,
    gpu_passes_ms: Option<BTreeMap<String, f64>>,
}

impl Recorder {
    fn build_report(&self) -> RawReport {
        RawReport {
            schema: SCHEMA,
            complete: true,
            warmup_frames: self.params.warmup_frames,
            measured_frames: self.params.measured_frames,
            timestamp_query: crate::gpu::frame_bench_queries::timestamps_usable()
                || self.samples.iter().any(|sample| {
                    sample
                        .gpu_passes_ms
                        .as_ref()
                        .is_some_and(|passes| !passes.is_empty())
                }),
            renderer_commit: crate::build_info::renderer_commit_sha8_label().to_string(),
            adapter: self.adapter.clone(),
            frames: self
                .samples
                .iter()
                .map(|sample| FrameJson {
                    cpu_frame_ms: sample.cpu_frame_ms,
                    draw_calls: sample.draw_calls,
                    upload_count: sample.upload_count,
                    upload_bytes: sample.upload_bytes,
                    gpu_passes_ms: sample.gpu_passes_ms.clone(),
                })
                .collect(),
        }
    }
}

fn write_report(path: &Path, report: &RawReport) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let body = serde_json::to_vec_pretty(report).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body).map_err(|err| err.to_string())?;
    std::fs::rename(&tmp, path).map_err(|err| err.to_string())?;
    Ok(())
}

/// Returns parsed bench parameters when `--frame-bench-out` is present.
fn parse_frame_bench_args(args: &[String]) -> Option<Params> {
    let mut enabled = false;
    let mut output_path = None;
    let mut warmup_frames = DEFAULT_WARMUP_FRAMES;
    let mut measured_frames = DEFAULT_MEASURED_FRAMES;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg.eq_ignore_ascii_case("--frame-bench-out") {
            enabled = true;
            if let Some(value) = args.get(index + 1) {
                output_path = Some(PathBuf::from(value));
                index += 2;
                continue;
            }
        } else if arg.eq_ignore_ascii_case("--frame-bench-warmup") {
            if let Some(value) = args.get(index + 1)
                && let Ok(parsed) = value.parse::<u32>()
            {
                warmup_frames = parsed;
            }
            index += 2;
            continue;
        } else if arg.eq_ignore_ascii_case("--frame-bench-frames")
            && let Some(value) = args.get(index + 1)
        {
            if let Ok(parsed) = value.parse::<u32>()
                && parsed > 0
            {
                measured_frames = parsed;
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    enabled.then_some(Params {
        output_path: output_path?,
        warmup_frames,
        measured_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MEASURED_FRAMES, DEFAULT_WARMUP_FRAMES, parse_frame_bench_args};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn absent_flag_disables_bench() {
        assert!(parse_frame_bench_args(&args(&["renderide"])).is_none());
    }

    #[test]
    fn parses_path_and_frame_counts() {
        let parsed = parse_frame_bench_args(&args(&[
            "renderide",
            "--frame-bench-out",
            "out/bench.json",
            "--frame-bench-warmup",
            "3",
            "--frame-bench-frames",
            "5",
        ]))
        .expect("enabled");
        assert_eq!(parsed.output_path.as_os_str(), "out/bench.json");
        assert_eq!(parsed.warmup_frames, 3);
        assert_eq!(parsed.measured_frames, 5);
    }

    #[test]
    fn zero_measured_frames_keep_the_default() {
        let parsed = parse_frame_bench_args(&args(&[
            "renderide",
            "--frame-bench-out",
            "a.json",
            "--frame-bench-frames",
            "0",
            "--frame-bench-warmup",
            "0",
        ]))
        .expect("enabled");
        assert_eq!(parsed.warmup_frames, 0);
        assert_eq!(parsed.measured_frames, DEFAULT_MEASURED_FRAMES);
        assert_eq!(DEFAULT_WARMUP_FRAMES, 8);
    }
}
