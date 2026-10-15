//! Integration: [`logger::init_for`] for [`logger::LogComponent::RendererTest`] under a temp logs root.

use std::time::{SystemTime, UNIX_EPOCH};

/// Ensures renderer-test harness logs land under `<temp>/renderer-test/<ts>.log`.
#[test]
fn init_for_renderer_test_under_temp_logs_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    // SAFETY: env mutation is local to this integration-test process.
    unsafe {
        std::env::set_var("RENDERIDE_LOGS_ROOT", dir.path().as_os_str());
    }

    let ts = format!(
        "init_for_renderer_test_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    let log_path = logger::init_for(
        logger::LogComponent::RendererTest,
        &ts,
        logger::LogLevel::Info,
        false,
    )
    .expect("init_for");

    assert!(
        log_path.starts_with(dir.path()),
        "expected {:?} under {:?}",
        log_path,
        dir.path()
    );
    assert!(
        log_path.ends_with(format!("{ts}.log")),
        "expected {log_path:?} to end with {ts}.log"
    );
    assert_eq!(
        log_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str()),
        Some("renderer-test")
    );
    assert!(log_path.parent().expect("parent").is_dir());

    logger::info!("renderer_test_init_for_marker");
    logger::flush();

    let contents = std::fs::read_to_string(&log_path).expect("read log");
    assert!(
        contents.contains("renderer_test_init_for_marker"),
        "expected marker in file: {contents:?}"
    );

    // SAFETY: env mutation is local to this integration-test process.
    unsafe {
        std::env::remove_var("RENDERIDE_LOGS_ROOT");
    }
}
