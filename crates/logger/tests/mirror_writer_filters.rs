//! Integration: severity-filtered mirror writers receive only selected log lines.

use std::sync::Mutex;
use std::time::SystemTime;

static MIRRORED: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn capture_mirror(bytes: &[u8]) {
    if let Ok(mut mirrored) = MIRRORED.lock() {
        mirrored.extend_from_slice(bytes);
    }
}

#[test]
fn mirror_writer_filters_by_level_and_preserves_file_output() {
    MIRRORED.lock().expect("mirror lock").clear();
    let path = std::env::temp_dir().join(format!(
        "logger_mirror_writer_{}.log",
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_file(&path);

    logger::init(&path, logger::LogLevel::Debug, false).expect("logger init");
    logger::set_mirror_writer(logger::LogLevel::Error, capture_mirror);

    logger::info!("mirror_writer_info_marker");
    logger::warn!("mirror_writer_warn_marker");
    logger::error!("mirror_writer_error_marker");
    logger::flush();

    let file_contents = std::fs::read_to_string(&path).expect("read log");
    assert!(file_contents.contains("mirror_writer_info_marker"));
    assert!(file_contents.contains("mirror_writer_warn_marker"));
    assert!(file_contents.contains("mirror_writer_error_marker"));

    let mirrored_bytes = MIRRORED.lock().expect("mirror lock").clone();
    let mirrored = String::from_utf8(mirrored_bytes).expect("mirror output is utf8");
    assert!(!mirrored.contains("mirror_writer_info_marker"));
    assert!(!mirrored.contains("mirror_writer_warn_marker"));
    assert!(mirrored.contains("mirror_writer_error_marker"));

    let _ = std::fs::remove_file(&path);
}
