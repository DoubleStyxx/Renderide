//! Verifies renderide-test logging initializes under the dedicated renderer-test folder.

#[test]
fn renderide_test_logging_writes_to_renderer_test_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    // SAFETY: env mutation is local to this integration-test process and happens before logger init.
    unsafe {
        std::env::set_var("RENDERIDE_LOGS_ROOT", dir.path().as_os_str());
    }

    let log_path = renderide_test::logging::init_renderer_test_logging()
        .expect("init renderer-test logging")
        .expect("logger should be installed by this test");

    assert_eq!(
        log_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str()),
        Some("renderer-test")
    );
    assert!(log_path.starts_with(dir.path()));

    logger::info!("renderide_test_logging_folder_marker");
    logger::flush();

    let contents = std::fs::read_to_string(&log_path).expect("read log");
    assert!(
        contents.contains("renderide_test_logging_folder_marker"),
        "expected marker in file: {contents:?}"
    );

    // SAFETY: env mutation is local to this integration-test process.
    unsafe {
        std::env::remove_var("RENDERIDE_LOGS_ROOT");
    }
}
