//! Integration: dependency `log` facade records are routed into the Renderide logger.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct LogsRootOverride<'lock> {
    _guard: MutexGuard<'lock, ()>,
    prev: Option<std::ffi::OsString>,
}

impl Drop for LogsRootOverride<'_> {
    fn drop(&mut self) {
        // SAFETY: env mutation in test; serialized via the ENV_LOCK guard held by `_guard`.
        unsafe {
            match self.prev.take() {
                Some(prev) => std::env::set_var("RENDERIDE_LOGS_ROOT", prev),
                None => std::env::remove_var("RENDERIDE_LOGS_ROOT"),
            }
        }
    }
}

fn with_logs_root_override(root: &std::path::Path) -> LogsRootOverride<'static> {
    let guard = match ENV_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let prev = std::env::var_os("RENDERIDE_LOGS_ROOT");
    // SAFETY: env mutation in test; serialized via the ENV_LOCK guard held above.
    unsafe {
        std::env::set_var("RENDERIDE_LOGS_ROOT", root.as_os_str());
    }
    LogsRootOverride {
        _guard: guard,
        prev,
    }
}

#[test]
fn log_facade_records_use_targets_and_follow_runtime_filter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let _override = with_logs_root_override(dir.path());
    let log_path = logger::init_for(
        logger::LogComponent::Renderer,
        "log_facade_targets",
        logger::LogLevel::Info,
        false,
    )
    .expect("init_for");

    log::info!(target: "dependency.crate", "dependency_info_marker");
    log::debug!(target: "dependency.crate", "dependency_debug_before_filter_marker");

    logger::set_max_level(logger::LogLevel::Trace);
    log::debug!(target: "dependency.crate", "dependency_debug_after_filter_marker");
    log::trace!(target: "dependency.crate", "dependency_trace_after_filter_marker");

    logger::set_max_level(logger::LogLevel::Debug);
    log::debug!(target: "naga::front", "naga_debug_suppressed_marker");
    log::info!(target: "naga::front", "naga_info_visible_marker");
    logger::info!("project_macro_target_marker");
    logger::flush();

    let contents = std::fs::read_to_string(log_path).expect("read log");
    assert!(contents.contains("[dependency.crate] INFO dependency_info_marker"));
    assert!(!contents.contains("dependency_debug_before_filter_marker"));
    assert!(contents.contains("[dependency.crate] DEBUG dependency_debug_after_filter_marker"));
    assert!(contents.contains("[dependency.crate] TRACE dependency_trace_after_filter_marker"));
    assert!(!contents.contains("naga_debug_suppressed_marker"));
    assert!(contents.contains("[naga::front] INFO naga_info_visible_marker"));
    assert!(contents.contains("[log_facade] INFO project_macro_target_marker"));
    assert!(!contents.contains("[renderide] INFO project_macro_target_marker"));
}
