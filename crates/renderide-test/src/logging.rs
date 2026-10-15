//! Logging bootstrap for the renderide-test harness.

use std::path::PathBuf;

/// Initializes the renderide-test log stream under `logs/renderer-test`.
///
/// Returns [`Some`] with the active log path when this call installs the logger. Returns [`None`]
/// if a logger was already installed by the current process.
///
/// # Errors
///
/// Returns [`Err`] if the log directory or file cannot be created.
pub fn init_renderer_test_logging() -> std::io::Result<Option<PathBuf>> {
    if logger::is_initialized() {
        return Ok(None);
    }

    let timestamp = logger::log_filename_timestamp();
    let log_path = logger::init_for(
        logger::LogComponent::RendererTest,
        &timestamp,
        logger::LogLevel::Info,
        false,
    )?;
    logger::info!("Logging to {}", log_path.display());
    Ok(Some(log_path))
}
