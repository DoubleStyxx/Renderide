//! Fatal failures encountered while starting the renderer (before or instead of a normal event-loop exit).

use std::io;

use thiserror::Error;
use winit::error::EventLoopError;

use crate::connection::InitError as ConnectionInitError;
use crate::gpu::GpuError;

/// Startup or early abort before the winit loop returns an optional process exit code.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct RunError(#[from] RunErrorKind);

#[derive(Debug, Error)]
enum RunErrorKind {
    /// Singleton guard, IPC connect, or other [`ConnectionInitError`] from bootstrap.
    #[error(transparent)]
    Connection(ConnectionInitError),
    /// File logging could not be initialized (see `logger::init_for`).
    #[error("failed to initialize logging: {0}")]
    LoggingInit(io::Error),
    /// The host did not send [`crate::shared::RendererInitData`](crate::shared::RendererInitData) within the startup timeout.
    #[error("timed out waiting for RendererInitData from host")]
    RendererInitDataTimeout,
    /// IPC reported a fatal error while waiting for init data.
    #[error("fatal IPC error while waiting for RendererInitData")]
    RendererInitDataFatalIpc,
    /// [`winit`] could not create the event loop (display backend unavailable, etc.).
    #[error(transparent)]
    EventLoopCreate(#[from] EventLoopError),
    /// [`crate::gpu::GpuContext`] initialization (desktop or headless) failed.
    #[error("GPU init: {0}")]
    Gpu(GpuError),
}

impl RunError {
    /// Wraps an IPC/bootstrap connection failure.
    pub(crate) fn connection(source: ConnectionInitError) -> Self {
        Self(RunErrorKind::Connection(source))
    }

    /// Wraps a file logging initialization failure.
    pub(crate) fn logging_init(source: io::Error) -> Self {
        Self(RunErrorKind::LoggingInit(source))
    }

    /// Builds the startup timeout error emitted while waiting for init data.
    pub(crate) const fn renderer_init_data_timeout() -> Self {
        Self(RunErrorKind::RendererInitDataTimeout)
    }

    /// Builds the fatal IPC error emitted while waiting for init data.
    pub(crate) const fn renderer_init_data_fatal_ipc() -> Self {
        Self(RunErrorKind::RendererInitDataFatalIpc)
    }

    /// Wraps winit event-loop creation failure.
    pub(crate) fn event_loop_create(source: EventLoopError) -> Self {
        Self(RunErrorKind::EventLoopCreate(source))
    }

    /// Wraps GPU initialization failure.
    pub(crate) fn gpu(source: GpuError) -> Self {
        Self(RunErrorKind::Gpu(source))
    }
}

#[cfg(test)]
mod tests {
    use super::{RunError, RunErrorKind};

    /// Constant-string variants have stable [`std::fmt::Display`] output used by the process exit
    /// log line -- regressions here change what operators see in `logs/renderer/*.log`.
    #[test]
    fn timeout_and_fatal_ipc_variants_have_stable_display_messages() {
        assert_eq!(
            RunError::renderer_init_data_timeout().to_string(),
            "timed out waiting for RendererInitData from host"
        );
        assert_eq!(
            RunError::renderer_init_data_fatal_ipc().to_string(),
            "fatal IPC error while waiting for RendererInitData"
        );
    }

    /// The [`std::io::Error`] conversion is wired via [`thiserror`] `#[from]`, so any
    /// [`std::io::Error`] can propagate into the logging-init branch without explicit mapping.
    #[test]
    fn logging_init_prefix_and_io_from_impl() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no write");
        let run_err = RunError::logging_init(io_err);
        let rendered = run_err.to_string();
        assert!(
            rendered.starts_with("failed to initialize logging:"),
            "expected logging prefix, got {rendered:?}"
        );
        assert!(
            rendered.contains("no write"),
            "io source message missing: {rendered:?}"
        );
        assert!(matches!(run_err.0, RunErrorKind::LoggingInit(_)));
    }
}
