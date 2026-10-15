//! Errors building [`super::material_bind::EmbeddedMaterialBindResources`] or resolving stem layouts.

use thiserror::Error;

/// Embedded material bind-group construction failed (layout cache, GPU resources, or stem resolution).
#[derive(Debug, Clone, Error)]
pub enum EmbeddedMaterialBindError {
    /// Human-readable bind-group or layout resolution failure.
    #[error("{0}")]
    Message(String),
}

impl From<String> for EmbeddedMaterialBindError {
    fn from(msg: String) -> Self {
        Self::Message(msg)
    }
}

impl From<&str> for EmbeddedMaterialBindError {
    fn from(msg: &str) -> Self {
        Self::Message(msg.to_string())
    }
}
