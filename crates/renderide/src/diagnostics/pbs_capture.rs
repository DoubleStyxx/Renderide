//! Throwaway per-frame log for the Windows `pbs_lit` canaries.
//!
//! Armed only when the headless output path contains `pbs_lit`. Every other
//! capture, and every real-GPU frame, returns before formatting a line.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static ARMED: AtomicBool = AtomicBool::new(false);
static FRAME: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static PASS: Cell<&'static str> = const { Cell::new("unset") };
}

/// Arms logging when `path` is the `pbs_lit` capture. Other outputs stay quiet.
pub(crate) fn arm_if_pbs_capture(path: &std::path::Path) {
    let text = path.to_string_lossy();
    if text.contains("pbs_lit") {
        ARMED.store(true, Ordering::Relaxed);
        logger::info!("pbs-diag armed path={}", path.display());
    }
}

/// Whether this process is the `pbs_lit` headless capture.
pub(crate) fn enabled() -> bool {
    ARMED.load(Ordering::Relaxed)
}

/// Starts one headless frame. The number is what lookup and draw lines report.
pub(crate) fn begin_frame() {
    if !enabled() {
        return;
    }
    let frame = FRAME.fetch_add(1, Ordering::Relaxed);
    logger::info!("pbs-diag frame-begin n={frame}");
}

fn frame() -> u64 {
    FRAME.load(Ordering::Relaxed).saturating_sub(1)
}

/// Names the pass whose draws follow, until the next call on this thread.
pub(crate) fn set_pass(pass: &'static str) {
    if enabled() {
        PASS.with(|cell| cell.set(pass));
    }
}

/// Records the viewport and the scissor the pass actually bound.
pub(crate) fn note_viewport(
    pass: &'static str,
    viewport: (u32, u32),
    scissor: (u32, u32, u32, u32),
) {
    if !enabled() {
        return;
    }
    let frame = frame();
    logger::info!(
        "pbs-diag viewport frame={frame} pass={pass} viewport={}x{} scissor={},{},{},{}",
        viewport.0,
        viewport.1,
        scissor.0,
        scissor.1,
        scissor.2,
        scissor.3
    );
}

/// One caster's frustum or Hi-Z decision. `visible` means neither test rejected it.
pub(crate) fn note_cull(caster: usize, result: &'static str) {
    if !enabled() {
        return;
    }
    let frame = frame();
    logger::info!("pbs-diag cull frame={frame} caster={caster} result={result}");
}

/// One indexed draw that was submitted. The pass is the last [`set_pass`] on this thread.
pub(crate) fn note_draw(caster: usize, indices: u32, instances: u32) {
    if !enabled() {
        return;
    }
    let frame = frame();
    let pass = PASS.with(|cell| cell.get());
    logger::info!(
        "pbs-diag draw frame={frame} pass={pass} caster={caster} indices={indices} instances={instances}"
    );
}

/// A material pipeline lookup. `result` is `ready`, `pending`, or `failed`.
pub(crate) fn note_lookup(
    kind: &dyn std::fmt::Debug,
    depth: Option<wgpu::TextureFormat>,
    samples: u32,
    result: &'static str,
) {
    if !enabled() {
        return;
    }
    let frame = frame();
    logger::info!(
        "pbs-diag lookup frame={frame} result={result} samples={samples} depth={depth:?} kind={kind:?}"
    );
}

/// The draw resolver skipped a batch after lookup. `result` is `empty` or `none`.
pub(crate) fn note_resolve(
    shader: &dyn std::fmt::Debug,
    kind: &dyn std::fmt::Debug,
    result: &'static str,
) {
    if !enabled() {
        return;
    }
    let frame = frame();
    logger::info!("pbs-diag resolve frame={frame} result={result} shader={shader:?} kind={kind:?}");
}

/// A pipeline set was inserted into the material cache.
pub(crate) fn note_created(
    kind: &dyn std::fmt::Debug,
    depth: Option<wgpu::TextureFormat>,
    samples: u32,
    pipelines: usize,
) {
    if !enabled() {
        return;
    }
    let frame = frame();
    logger::info!(
        "pbs-diag created frame={frame} pipelines={pipelines} empty={} samples={samples} depth={depth:?} kind={kind:?}",
        pipelines == 0
    );
}

/// Row occupancy of the depth attachment after the frame, plus where the raw dump went.
pub(crate) fn note_depth(summary: &str) {
    if !enabled() {
        return;
    }
    let frame = frame();
    logger::info!("pbs-diag depth frame={frame} {summary}");
}
