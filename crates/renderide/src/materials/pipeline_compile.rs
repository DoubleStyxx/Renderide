//! Where a material pipeline build runs.
//!
//! A frame blocks on queued compiles only when its image will be read back:
//! a headless golden or capture, including a control-api capture on that same
//! headless frame. The drain compiles the whole render-thread queue at frame
//! start. It does not flush or poll the device first, and it does not compile
//! during a draw lookup. A lookup in that frame stays pending until the next drain.
//!
//! Every other frame uses the background pool, the same as before this gate.
//! Windowed frames never block, including windowed software adapters. Headless
//! frames that are not read back (frame bench included) do not block either, on
//! a real GPU or on lavapipe or WARP. Those frames return with the pipeline
//! still pending.

use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use super::cache::{
    MaterialPipelineCache, MaterialPipelineCacheKey, MaterialPipelineSet,
    MaterialPipelineVariantSpec, ShaderModuleCache,
};
use super::pipeline_kind::RasterPipelineKind;
use super::raster_pipeline::MaterialPipelineDesc;

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

/// `true` when this frame's image is read back, so queued compiles must finish first.
///
/// Headless golden and capture frames block. A headless control-api capture is
/// one of those frames: the headless driver read back every presented frame
/// except while the frame bench is running. Windowed frames never block.
/// Frame-bench frames do not block on a real GPU or a software adapter.
pub(crate) fn material_compile_blocks_frame(headless: bool, frame_bench: bool) -> bool {
    headless && !frame_bench
}

/// How many render-thread pipeline compiles one drain may run.
///
/// `0` keeps the background pool and leaves the pipeline pending. A capture
/// readback returns [`usize::MAX`] so the frame compiles every pipeline already
/// queued for this thread before it submits. Windowed frames return `0`.
pub(crate) fn render_thread_compiles_per_frame(headless: bool, frame_bench: bool) -> usize {
    if material_compile_blocks_frame(headless, frame_bench) {
        usize::MAX
    } else {
        0
    }
}

/// How many queued render-thread builds a drain with `budget` actually compiles.
///
/// A non-capture budget of zero compiles nothing, so the pipeline stays pending.
pub(crate) fn render_thread_compile_take(queued: usize, budget: usize) -> usize {
    budget.min(queued)
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
        let count = render_thread_compile_take(queue.len(), max_compiles);
        queue.drain(..count).collect::<Vec<_>>()
    };
    let mut deferred = Vec::new();
    for request in requests {
        if super::test_pipeline_stall::hold_render_thread_build(&request.kind, &request.key) {
            deferred.push(request);
        } else {
            finish_pipeline_build(request);
        }
    }
    if deferred.is_empty() {
        return;
    }
    // The held build stays queued. The render thread publishes this frame instead of sleeping.
    let mut queue = render_thread_pipeline_build_queue().lock();
    for request in deferred.into_iter().rev() {
        queue.insert(0, request);
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
        MATERIAL_PIPELINE_MAX_WORKERS, material_compile_blocks_frame,
        material_pipeline_compile_worker_count, render_thread_compile_take,
        render_thread_compiles_per_frame,
    };

    #[test]
    fn material_pipeline_worker_count_is_bounded() {
        let workers = material_pipeline_compile_worker_count();

        assert!(workers >= 1);
        assert!(workers <= MATERIAL_PIPELINE_MAX_WORKERS);
    }

    #[test]
    fn windowed_frames_never_block_on_a_queued_compile() {
        assert!(!material_compile_blocks_frame(false, false));
        assert!(!material_compile_blocks_frame(false, true));
        assert_eq!(render_thread_compiles_per_frame(false, false), 0);
        assert_eq!(render_thread_compiles_per_frame(false, true), 0);
    }

    #[test]
    fn non_capture_frame_leaves_the_pipeline_pending() {
        let frame_bench = true;
        let headless = true;
        let budget = render_thread_compiles_per_frame(headless, frame_bench);

        assert!(!material_compile_blocks_frame(headless, frame_bench));
        assert_eq!(budget, 0);
        assert_eq!(render_thread_compile_take(4, budget), 0);

        let windowed_budget = render_thread_compiles_per_frame(false, false);
        assert_eq!(render_thread_compile_take(4, windowed_budget), 0);
    }

    #[test]
    fn capture_frame_compiles_the_queued_set_on_every_gpu() {
        let frame_bench = false;
        let headless = true;

        assert!(material_compile_blocks_frame(headless, frame_bench));
        let budget = render_thread_compiles_per_frame(headless, frame_bench);
        assert_eq!(budget, usize::MAX);
        assert_eq!(render_thread_compile_take(4, budget), 4);
    }
}
