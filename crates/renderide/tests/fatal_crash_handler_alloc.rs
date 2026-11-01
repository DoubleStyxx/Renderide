//! Counting-allocator check for the fatal-crash handler writer.
//!
//! This binary installs its own [`GlobalAlloc`] so the count is the handler
//! path's Rust allocations and nothing else. One test keeps the count from
//! racing another test in the same process.

#![cfg(any(target_os = "linux", target_os = "android", windows))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering, compiler_fence};

struct CountingAlloc;

static COUNT: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);

fn note_alloc(ptr: *mut u8) {
    if !ptr.is_null() && COUNT.load(Ordering::Acquire) {
        ALLOCS.fetch_add(1, Ordering::Release);
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

fn copy_to_stack(chunk: &[u8], scratch: &mut [u8], used: &mut usize) {
    let room = scratch.len().saturating_sub(*used);
    let n = chunk.len().min(room);
    scratch[*used..*used + n].copy_from_slice(&chunk[..n]);
    *used += n;
}

#[test]
fn handler_path_allocates_nothing() {
    use renderide::fatal_crash_test_support::{
        publish_module_span_for_test, reset_handler_reentry_for_test, write_handler_stack,
    };

    publish_module_span_for_test();
    reset_handler_reentry_for_test();
    // First walk may initialize unwind state. That is outside the measured call.
    write_handler_stack(|_| {});

    reset_handler_reentry_for_test();
    ALLOCS.store(0, Ordering::SeqCst);
    compiler_fence(Ordering::SeqCst);
    COUNT.store(true, Ordering::SeqCst);
    compiler_fence(Ordering::SeqCst);

    let mut scratch = [0u8; 8192];
    let mut used = 0usize;
    write_handler_stack(|chunk| copy_to_stack(chunk, &mut scratch, &mut used));

    compiler_fence(Ordering::SeqCst);
    COUNT.store(false, Ordering::SeqCst);
    compiler_fence(Ordering::SeqCst);
    let allocs = ALLOCS.load(Ordering::SeqCst);

    let record = std::str::from_utf8(&scratch[..used]).expect("handler record is utf8");
    assert!(
        record.contains("STACK ("),
        "measured call did not write a stack record ({used} bytes): {record:?}"
    );
    assert!(
        record.contains("SYMBOLS: deferred"),
        "handler record missing the deferred notice: {record}"
    );
    assert_eq!(
        allocs, 0,
        "handler path allocated {allocs} times while writing"
    );
    assert!(
        !record.contains("SYMBOLS:\n"),
        "handler path symbolicated inside the writer: {record}"
    );
}
