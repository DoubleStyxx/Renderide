//! Cargo artifact-directory helpers shared by build-script asset copy steps.

use std::path::{Path, PathBuf};

/// Derives the Cargo artifact profile directory from `OUT_DIR`.
///
/// Cargo always sets `OUT_DIR = .../target/<profile-dir>/build/<pkg>-<hash>/out`; walking up
/// three components recovers `.../target/<profile-dir>/` even when `PROFILE` is `debug` for a
/// custom profile that inherits from `dev` (like this workspace's `dev-fast`). Cross-target
/// builds are covered because their `OUT_DIR` already includes `target/<triple>/<profile-dir>/`.
pub fn artifact_dir_from_out_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}
