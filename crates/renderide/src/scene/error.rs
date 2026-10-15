//! Errors from scene / hierarchy operations.

use thiserror::Error;

/// Failure applying host scene or transform data.
#[derive(Debug, Error)]
pub enum SceneError {
    /// Shared memory read for a transform batch failed.
    #[error("shared memory: {0}")]
    SharedMemoryAccess(String),
    /// Per-transform hierarchy cycle while computing world matrices.
    #[error("cycle in scene {scene_id} at transform {transform_id}")]
    #[cfg(test)]
    CycleDetected {
        /// Host render space id.
        scene_id: i32,
        /// Dense transform index.
        transform_id: i32,
    },
    /// A scene was missing from the registry when required.
    #[error("scene {scene_id} not found")]
    #[cfg(test)]
    SceneNotFound {
        /// Host render space id.
        scene_id: i32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_memory_access_includes_inner_message() {
        let err = SceneError::SharedMemoryAccess("descriptor missing".to_string());
        assert_eq!(format!("{err}"), "shared memory: descriptor missing");
    }

    #[test]
    fn cycle_detected_formats_scene_and_transform_ids() {
        let err = SceneError::CycleDetected {
            scene_id: 4,
            transform_id: 17,
        };
        assert_eq!(format!("{err}"), "cycle in scene 4 at transform 17");
    }

    #[test]
    fn scene_not_found_formats_scene_id() {
        let err = SceneError::SceneNotFound { scene_id: 9 };
        assert_eq!(format!("{err}"), "scene 9 not found");
    }
}
