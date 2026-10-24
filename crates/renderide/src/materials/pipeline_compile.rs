//! Where a material pipeline build runs.
//!
//! Real GPUs keep the background pool. Headless captures and software adapters
//! (WARP, lavapipe, llvmpipe) compile on the render thread instead: a background
//! `create_render_pipeline` deadlocks DX12 WARP while the render thread is inside
//! a frame. Headless compiles the whole queued set at the start of the frame and
//! does not flush or poll first. Doing that wait on DX12 WARP cached a PBS
//! pipeline that drew only the lower hemisphere of each sphere (1119 unique
//! colors, 5.9% non-background). The same queue compiled here with no flush and
//! no poll drew the full spheres (3780 colors, 8.8%).
//!
//! A windowed software adapter compiles at most
//! [`RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME`] pipelines per drain so one frame
//! cannot absorb a whole scene, and its idle wait is one short budget. Queued
//! builds that do not fit in this drain stay pending.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use super::cache::{
    MaterialPipelineCache, MaterialPipelineCacheKey, MaterialPipelineSet,
    MaterialPipelineVariantSpec, ShaderModuleCache,
};
use super::pipeline_kind::RasterPipelineKind;
use super::raster_pipeline::MaterialPipelineDesc;

/// Set after this frame's idle wait succeeds on a headless capture.
///
/// Draw lookup then compiles the pipeline before returning, so the frame is not
/// counted pending and the build does not move into command recording. Cleared
/// before a pass is opened and again after the frame submits.
static RENDER_THREAD_INLINE_COMPILE: AtomicBool = AtomicBool::new(false);

/// Allows headless draw lookup to compile a queued pipeline on this thread.
pub(crate) fn set_render_thread_inline_compile(enabled: bool) {
    RENDER_THREAD_INLINE_COMPILE.store(enabled, Ordering::Relaxed);
}

pub(crate) fn render_thread_inline_compile_enabled() -> bool {
    RENDER_THREAD_INLINE_COMPILE.load(Ordering::Relaxed)
}

/// Pipelines compiled on the render thread during one
/// [`MaterialPipelineCache::drain_pipeline_build_completions`] call.
///
/// One compile per frame is the budget for headless and software adapters.
/// Real GPUs do not use this path.
pub(in crate::materials) const RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME: usize = 1;

/// Maximum number of background workers dedicated to material pipeline compilation.
const MATERIAL_PIPELINE_MAX_WORKERS: usize = 4;

pub(in crate::materials) struct PipelineBuildRequest {
    pub(in crate::materials) key: MaterialPipelineCacheKey,
    pub(in crate::materials) kind: RasterPipelineKind,
    pub(in crate::materials) desc: MaterialPipelineDesc,
    pub(in crate::materials) variant: MaterialPipelineVariantSpec,
    /// Optional WGSL source override loaded by development hot reload.
    pub(in crate::materials) wgsl_override: Option<Arc<str>>,
    pub(in crate::materials) resources: PipelineBuildResources,
    pub(in crate::materials) tx: crossbeam_channel::Sender<PipelineBuildOutcome>,
}

pub(in crate::materials) struct PipelineBuildResources {
    pub(in crate::materials) shader_module_cache: Arc<ShaderModuleCache>,
    pub(in crate::materials) device: Arc<wgpu::Device>,
    pub(in crate::materials) limits: Arc<crate::gpu::GpuLimits>,
}

pub(in crate::materials) struct PipelineBuildOutcome {
    pub(in crate::materials) key: MaterialPipelineCacheKey,
    pub(in crate::materials) kind: RasterPipelineKind,
    pub(in crate::materials) result: Result<MaterialPipelineSet, String>,
}

/// Headless and software adapters compile on the render thread. A real GPU does not.
pub(crate) fn compile_on_render_thread(
    headless: bool,
    device_type: wgpu::DeviceType,
    adapter_name: &str,
) -> bool {
    headless || device_type == wgpu::DeviceType::Cpu || adapter_name_is_software(adapter_name)
}

/// How many render-thread pipeline compiles one drain may run.
///
/// `0` keeps the background pool. Headless returns [`usize::MAX`] so the capture
/// compiles every pipeline queued for this thread before the frame submits. A
/// windowed software adapter returns [`RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME`].
/// Real GPUs are unchanged: cold-load cost stays on the background pool.
pub(crate) fn render_thread_compiles_per_frame(
    headless: bool,
    device_type: wgpu::DeviceType,
    adapter_name: &str,
) -> usize {
    if !compile_on_render_thread(headless, device_type, adapter_name) {
        return 0;
    }
    if headless {
        usize::MAX
    } else {
        RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME
    }
}

fn adapter_name_is_software(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("llvmpipe")
        || name.contains("lavapipe")
        || name.contains("swiftshader")
        || name.contains("microsoft basic render")
}

pub(in crate::materials) fn spawn_pipeline_build(
    request: PipelineBuildRequest,
    on_render_thread: bool,
) -> Result<(), String> {
    if on_render_thread {
        render_thread_pipeline_build_queue().lock().push(request);
    } else {
        let pool = material_pipeline_compile_pool()?;
        pool.spawn(move || finish_pipeline_build(request));
    }
    Ok(())
}

/// Compiles up to `max_compiles` builds that were queued for the render thread.
pub(in crate::materials) fn drain_render_thread_pipeline_builds(max_compiles: usize) {
    let requests = {
        let mut queue = render_thread_pipeline_build_queue().lock();
        let count = max_compiles.min(queue.len());
        queue.drain(..count).collect::<Vec<_>>()
    };
    for request in requests {
        finish_pipeline_build(request);
    }
}

fn finish_pipeline_build(request: PipelineBuildRequest) {
    profiling::scope!("materials::async_pipeline_compile");
    let PipelineBuildRequest {
        key,
        kind,
        desc,
        variant,
        wgsl_override,
        resources,
        tx,
    } = request;
    let shader_source_generation = key.shader_source_generation;
    let result = MaterialPipelineCache::build_pipeline_set_for(
        resources,
        &kind,
        &desc,
        variant,
        shader_source_generation,
        wgsl_override,
    )
    .map_err(|e| e.to_string());
    let _ = tx.send(PipelineBuildOutcome { key, kind, result });
}

fn render_thread_pipeline_build_queue() -> &'static Mutex<Vec<PipelineBuildRequest>> {
    static QUEUE: Mutex<Vec<PipelineBuildRequest>> = Mutex::new(Vec::new());
    &QUEUE
}

/// True when a render-thread compile is waiting for the next drain.
pub(crate) fn render_thread_builds_queued() -> bool {
    !render_thread_pipeline_build_queue().lock().is_empty()
}

fn material_pipeline_compile_worker_count() -> usize {
    std::thread::available_parallelism().map_or(1, |threads| {
        (threads.get() / 2).clamp(1, MATERIAL_PIPELINE_MAX_WORKERS)
    })
}

fn material_pipeline_compile_pool() -> Result<&'static rayon::ThreadPool, String> {
    static POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(material_pipeline_compile_worker_count())
            .thread_name(|idx| format!("material-pipeline-worker-{idx}"))
            .build()
            .map_err(|e| format!("material pipeline worker pool creation failed: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

#[cfg(test)]
mod tests {
    use super::{
        MATERIAL_PIPELINE_MAX_WORKERS, RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME,
        compile_on_render_thread, material_pipeline_compile_worker_count,
        render_thread_compiles_per_frame,
    };

    #[test]
    fn material_pipeline_worker_count_is_bounded() {
        let workers = material_pipeline_compile_worker_count();

        assert!(workers >= 1);
        assert!(workers <= MATERIAL_PIPELINE_MAX_WORKERS);
    }

    #[test]
    fn render_thread_compile_is_headless_or_software_only() {
        let discrete = wgpu::DeviceType::DiscreteGpu;
        let cpu = wgpu::DeviceType::Cpu;

        assert!(compile_on_render_thread(
            true,
            discrete,
            "NVIDIA GeForce RTX 4090"
        ));
        assert!(!compile_on_render_thread(
            false,
            discrete,
            "NVIDIA GeForce RTX 4090"
        ));
        assert!(compile_on_render_thread(
            false,
            discrete,
            "llvmpipe (LLVM 20.1.2, 256 bits)"
        ));
        assert!(compile_on_render_thread(false, discrete, "lavapipe"));
        assert!(compile_on_render_thread(
            false,
            discrete,
            "Microsoft Basic Render Driver"
        ));
        assert!(compile_on_render_thread(false, cpu, "Unknown Adapter"));
        assert!(!compile_on_render_thread(
            false,
            wgpu::DeviceType::IntegratedGpu,
            "Intel(R) UHD Graphics"
        ));
    }

    #[test]
    fn headless_compiles_the_queued_set_and_windowed_software_compiles_one() {
        let discrete = wgpu::DeviceType::DiscreteGpu;
        let cpu = wgpu::DeviceType::Cpu;

        assert_eq!(
            render_thread_compiles_per_frame(true, discrete, "NVIDIA GeForce RTX 4090"),
            usize::MAX
        );
        assert_eq!(
            render_thread_compiles_per_frame(false, cpu, "Microsoft Basic Render Driver"),
            RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME
        );
        assert_eq!(
            render_thread_compiles_per_frame(false, discrete, "NVIDIA GeForce RTX 4090"),
            0
        );
        assert_eq!(RENDER_THREAD_PIPELINE_COMPILES_PER_FRAME, 1);
    }
}
