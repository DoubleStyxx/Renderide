//! Allocation and conservativeness checks for [`super::SkinnedCullWorkload`].
//!
//! The counter is thread-local and fail-closed. `try_with` returns an error while a thread-local
//! is initializing or being destroyed, and those paths are not counted.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use super::SkinnedCullWorkload;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

/// Process allocator that counts allocations on the thread that opted in.
pub struct CountingAlloc;

// SAFETY: `CountingAlloc` forwards every request to the system allocator. The counters are
// thread-local `Cell`s, which do not allocate, so the hook does not re-enter itself.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get().saturating_add(1)));
        }
        // SAFETY: `layout` is a non-zero allocation request from the compiler, and `System`
        // honors it. The counter update above does not allocate.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` was produced by this allocator with the same `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get().saturating_add(1)));
        }
        // SAFETY: same contract as [`GlobalAlloc::alloc_zeroed`] on `System`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get().saturating_add(1)));
        }
        // SAFETY: `ptr` came from this allocator and `new_size` is the requested size.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn record_allocs(sample: impl FnOnce() -> u32) -> (u32, u64) {
    let _ = COUNTING.try_with(|flag| flag.set(false));
    let _ = COUNT.try_with(|count| count.set(0));
    let _ = COUNTING.try_with(|flag| flag.set(true));
    let visible = sample();
    let allocs = COUNT.try_with(Cell::get).unwrap_or(u64::MAX);
    let _ = COUNTING.try_with(|flag| flag.set(false));
    (visible, allocs)
}

#[test]
fn posed_boxes_stay_visible_when_rest_boxes_are_outside() {
    let workload = SkinnedCullWorkload::standard();
    let posed = workload.visible_count();
    let rest = workload.rest_visible_count();
    assert_eq!(rest, 0);
    assert!(posed > rest);
}

#[test]
fn skinned_clip_loop_allocates_nothing() {
    let workload = SkinnedCullWorkload::standard();
    let _ = workload.visible_count();
    let (visible, allocs) = record_allocs(|| workload.visible_count());
    assert!(visible > 0);
    assert_eq!(allocs, 0);
}
