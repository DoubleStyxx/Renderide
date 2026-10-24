//! Steady-state allocation counts for the CPU debug HUD.
//!
//! A counting [`GlobalAlloc`] wraps [`std::alloc::System`] for this binary only.
//! [`HudUi`] is driven by fixed fixture snapshots (1920×1080, scale 1.0, default font).
//! 120 warmup frames are discarded, then allocations over frames 121–360 are compared
//! to [`HUD_STEADY_ALLOC_BUDGET`] ceilings recorded on master. Later PRs may only lower
//! a ceiling. H4 sets the open budgets to 0; hidden is 0 from H7 on.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use renderide::hud_measure::{HudMeasureView, HudUi};

const WARMUP_FRAMES: u32 = 120;
const MEASURED_FRAMES: u32 = 240;

/// Total allocations over frames 121–360 with the HUD hidden.
///
/// Ceiling recorded on master. Later PRs may only lower it.
const HUD_STEADY_ALLOC_BUDGET_HIDDEN: u64 = 0;
/// Total allocations over frames 121–360 with the Stats tab open.
const HUD_STEADY_ALLOC_BUDGET_OPEN_STATS: u64 = 36_000;
/// Total allocations over frames 121–360 with the Shader routes tab open.
const HUD_STEADY_ALLOC_BUDGET_OPEN_SHADER_ROUTES: u64 = 10_320;
/// Total allocations over frames 121–360 with the Draw state tab open.
const HUD_STEADY_ALLOC_BUDGET_OPEN_DRAW_STATE: u64 = 10_080;
/// Total allocations over frames 121–360 with the GPU memory tab open.
const HUD_STEADY_ALLOC_BUDGET_OPEN_GPU_MEMORY: u64 = 9_360;
/// Total allocations over frames 121–360 with the GPU passes tab open.
const HUD_STEADY_ALLOC_BUDGET_OPEN_GPU_PASSES: u64 = 8_880;
/// Total allocations over frames 121–360 for the default open HUD (renderer config).
const HUD_STEADY_ALLOC_BUDGET_OPEN_RENDERER_CONFIG: u64 = 7_440;

struct CountingAlloc;

static COUNT: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);

fn note_alloc(ptr: *mut u8) {
    if !ptr.is_null() && COUNT.load(Ordering::Relaxed) {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards to `System`, which implements `GlobalAlloc`.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is the layout the caller asked the global allocator to satisfy.
        let ptr = unsafe { System.alloc(layout) };
        note_alloc(ptr);
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is the layout the caller asked the global allocator to satisfy.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        note_alloc(ptr);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` came from a previous `System` allocation in this allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: `ptr` and `layout` came from a previous `System` allocation in this allocator,
        // and `new_size` is the caller's requested new size.
        let ptr = unsafe { System.realloc(ptr, layout, new_size) };
        note_alloc(ptr);
        ptr
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

fn measure(view: HudMeasureView) -> u64 {
    let mut ui = HudUi::from_measure_fixture(view);
    for _ in 0..WARMUP_FRAMES {
        ui.run_measure_frame();
    }
    ALLOCS.store(0, Ordering::Relaxed);
    COUNT.store(true, Ordering::Relaxed);
    for _ in 0..MEASURED_FRAMES {
        ui.run_measure_frame();
    }
    COUNT.store(false, Ordering::Relaxed);
    let measured = ALLOCS.load(Ordering::Relaxed);
    drop(ui);
    measured
}

#[expect(
    clippy::print_stderr,
    reason = "report measured steady-state allocation counts next to the recorded budget"
)]
fn check(name: &str, view: HudMeasureView, budget: u64) -> u64 {
    let measured = measure(view);
    let per_frame = measured as f64 / f64::from(MEASURED_FRAMES);
    eprintln!("{name}: measured={measured} budget={budget} per_frame={per_frame:.2}");
    assert!(
        measured <= budget,
        "{name} allocated {measured} times over {MEASURED_FRAMES} steady-state frames, above budget {budget}"
    );
    measured
}

#[test]
fn hud_steady_state_allocation_budget() {
    check(
        "hidden",
        HudMeasureView::Hidden,
        HUD_STEADY_ALLOC_BUDGET_HIDDEN,
    );
    check(
        "open_stats",
        HudMeasureView::Stats,
        HUD_STEADY_ALLOC_BUDGET_OPEN_STATS,
    );
    check(
        "open_shader_routes",
        HudMeasureView::ShaderRoutes,
        HUD_STEADY_ALLOC_BUDGET_OPEN_SHADER_ROUTES,
    );
    check(
        "open_draw_state",
        HudMeasureView::DrawState,
        HUD_STEADY_ALLOC_BUDGET_OPEN_DRAW_STATE,
    );
    check(
        "open_gpu_memory",
        HudMeasureView::GpuMemory,
        HUD_STEADY_ALLOC_BUDGET_OPEN_GPU_MEMORY,
    );
    check(
        "open_gpu_passes",
        HudMeasureView::GpuPasses,
        HUD_STEADY_ALLOC_BUDGET_OPEN_GPU_PASSES,
    );
    check(
        "open_renderer_config",
        HudMeasureView::RendererConfig,
        HUD_STEADY_ALLOC_BUDGET_OPEN_RENDERER_CONFIG,
    );
}
