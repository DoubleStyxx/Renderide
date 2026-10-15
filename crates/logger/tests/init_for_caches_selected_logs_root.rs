//! Integration: a successful [`logger::init_for`] pins later component paths to the same logs root.

use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn init_for_caches_selected_root_for_later_log_file_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    // SAFETY: env mutation in test; this integration test runs in its own process.
    unsafe {
        std::env::set_var("RENDERIDE_LOGS_ROOT", dir.path().as_os_str());
    }

    let ts = format!(
        "cache_root_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let path = logger::init_for(
        logger::LogComponent::Bootstrapper,
        &ts,
        logger::LogLevel::Info,
        false,
    )
    .expect("init_for");
    assert!(path.starts_with(dir.path()));

    // SAFETY: env mutation in test; the selected logs root should now come from logger state.
    unsafe {
        std::env::remove_var("RENDERIDE_LOGS_ROOT");
    }

    let host_path = logger::log_file_path(logger::LogComponent::Host, "same_run");
    assert_eq!(host_path, dir.path().join("host").join("same_run.log"));
}
