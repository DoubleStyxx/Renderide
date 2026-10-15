//! Error and environment helpers for shader build support.

use thiserror::Error;

/// Errors from shader discovery, composition, validation, and generated code I/O.
#[derive(Debug, Error)]
pub enum BuildError {
    /// User-facing message for directive parsing, validation, and naga errors.
    #[error("{0}")]
    Message(String),
    /// Filesystem I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Required Cargo environment variable missing.
    #[error("missing build environment variable `{0}`")]
    MissingEnv(&'static str),
}

/// Reads a required build-script environment variable.
pub fn env_var(name: &'static str) -> Result<String, BuildError> {
    #[expect(
        clippy::map_err_ignore,
        reason = "MissingEnv carries the variable name; VarError adds no useful build context"
    )]
    std::env::var(name).map_err(|_| BuildError::MissingEnv(name))
}
