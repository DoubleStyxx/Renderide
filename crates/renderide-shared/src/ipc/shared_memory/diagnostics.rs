//! Shared diagnostic helpers and a throttled logger for the
//! [`super::accessor::SharedMemoryAccessor`] read path.
//!
//! All `access_*` methods route their failure messages through these helpers so that the wording,
//! field layout, and rate limiting stay consistent across read flavours. The helpers are private
//! to the `shared_memory` subtree and never surface in any cross-crate public API.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::buffer::SharedMemoryBufferDescriptor;

/// Process-wide counter for shared-memory read failures (used for log throttling).
static SHARED_MEMORY_READ_FAILURES: AtomicU64 = AtomicU64::new(0);

/// Always log the first `N` failures so transient regressions are visible.
const SHARED_MEMORY_READ_FAILURE_FIRST_LOGS: u64 = 8;

/// After the first burst, only log every `M`-th failure to bound log volume.
const SHARED_MEMORY_READ_FAILURE_LOG_EVERY: u64 = 256;

/// Builds a closure that prepends `context: ` to error messages when `context` is `Some`,
/// and passes them through unchanged otherwise.
pub(super) fn make_context_prefixer(context: Option<&str>) -> impl Fn(&str) -> String + '_ {
    move |msg: &str| match context {
        Some(ctx) => format!("{ctx}: {msg}"),
        None => msg.to_string(),
    }
}

/// Renders the diagnostic string used when `SharedMemoryAccessor::get_view` fails to map a
/// shared buffer (the most common cause is a missing or truncated backing file).
pub(super) fn describe_get_view_failure(buffer_id: i32, path_or_name: &str) -> String {
    format!("get_view failed buffer_id={buffer_id} path/name={path_or_name}")
}

/// Renders the diagnostic string used when slicing a successfully-mapped view fails (the requested
/// offset/length range fell outside the mapped capacity).
pub(super) fn describe_slice_failure(
    buffer_id: i32,
    offset: i32,
    length: i32,
    view_len: usize,
) -> String {
    format!(
        "slice failed buffer_id={buffer_id} offset={offset} length={length} view_len={view_len}"
    )
}

/// Renders the read-failure summary used by `with_read_bytes`: a single line that includes the
/// reason, full descriptor, and the mapping path (or `<not-mapped>` if `get_view` was skipped).
pub(super) fn describe_descriptor_failure(
    descriptor: &SharedMemoryBufferDescriptor,
    reason: &'static str,
    path_or_name: Option<&str>,
) -> String {
    format!(
        "shared memory read failed: reason={reason} buffer_id={} offset={} length={} capacity={} path/name={}",
        descriptor.buffer_id,
        descriptor.offset,
        descriptor.length,
        descriptor.buffer_capacity,
        path_or_name.unwrap_or("<not-mapped>")
    )
}

/// Renders the read-failure summary for slice failures that retain the descriptor context.
pub(super) fn describe_slice_failure_with_descriptor(
    descriptor: &SharedMemoryBufferDescriptor,
    view_len: usize,
) -> String {
    format!(
        "shared memory read failed: reason=slice failed buffer_id={} offset={} length={} capacity={} view_len={}",
        descriptor.buffer_id,
        descriptor.offset,
        descriptor.length,
        descriptor.buffer_capacity,
        view_len
    )
}

/// Records and conditionally logs a shared-memory read failure under
/// [`SHARED_MEMORY_READ_FAILURES`]. The first burst is always logged; subsequent failures are
/// throttled to one in [`SHARED_MEMORY_READ_FAILURE_LOG_EVERY`].
pub(super) fn log_shared_memory_read_failure(message: &str) {
    let occurrence = SHARED_MEMORY_READ_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
    if occurrence <= SHARED_MEMORY_READ_FAILURE_FIRST_LOGS
        || (occurrence - SHARED_MEMORY_READ_FAILURE_FIRST_LOGS)
            .is_multiple_of(SHARED_MEMORY_READ_FAILURE_LOG_EVERY)
    {
        logger::warn!("{message} occurrence={occurrence}");
    }
}
