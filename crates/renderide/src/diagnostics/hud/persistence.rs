//! Disk helpers for Dear ImGui HUD persistence.

use std::io;
use std::path::{Path, PathBuf};

/// Sidecar file containing Dear ImGui's raw window-layout `.ini` payload.
pub const IMGUI_INI_FILE_NAME: &str = "renderide-imgui.ini";

/// Places the ImGui sidecar next to the renderer config file.
pub fn imgui_ini_path_from_config_save_path(config_save_path: &Path) -> PathBuf {
    let parent = config_save_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(IMGUI_INI_FILE_NAME)
}

/// Reads UTF-8 text from `path`, returning `None` for an empty file.
pub fn read_nonempty_text(path: &Path) -> io::Result<Option<String>> {
    let contents = std::fs::read_to_string(path)?;
    if contents.is_empty() {
        Ok(None)
    } else {
        Ok(Some(contents))
    }
}

/// Writes UTF-8 text atomically unless `contents` is empty.
pub fn write_nonempty_text_atomic(path: &Path, contents: &str) -> io::Result<bool> {
    if contents.is_empty() {
        return Ok(false);
    }

    write_text_atomic(path, contents)?;
    Ok(true)
}

/// Writes UTF-8 text atomically using a hidden sibling temp file and rename.
fn write_text_atomic(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(IMGUI_INI_FILE_NAME);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, contents.as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        IMGUI_INI_FILE_NAME, imgui_ini_path_from_config_save_path, read_nonempty_text,
        write_nonempty_text_atomic,
    };

    #[test]
    fn imgui_ini_path_sits_next_to_config() {
        let p = imgui_ini_path_from_config_save_path(std::path::Path::new(
            "/tmp/renderide/config.toml",
        ));
        assert_eq!(
            p,
            std::path::Path::new("/tmp/renderide").join(IMGUI_INI_FILE_NAME)
        );
    }

    #[test]
    fn imgui_ini_path_handles_bare_config_filename() {
        let p = imgui_ini_path_from_config_save_path(std::path::Path::new("config.toml"));
        assert_eq!(p, std::path::Path::new(".").join(IMGUI_INI_FILE_NAME));
    }

    #[test]
    fn nonempty_read_returns_text() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(IMGUI_INI_FILE_NAME);
        std::fs::write(&path, "[Window][Renderer]\n").expect("write");

        let text = read_nonempty_text(&path).expect("read");

        assert_eq!(text.as_deref(), Some("[Window][Renderer]\n"));
    }

    #[test]
    fn empty_read_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(IMGUI_INI_FILE_NAME);
        std::fs::write(&path, "").expect("write");

        let text = read_nonempty_text(&path).expect("read");

        assert_eq!(text, None);
    }

    #[test]
    fn nonempty_atomic_write_roundtrips_text() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(IMGUI_INI_FILE_NAME);
        assert!(write_nonempty_text_atomic(&path, "[Window][Renderer]\n").expect("write"));

        let text = std::fs::read_to_string(path).expect("read");
        assert_eq!(text, "[Window][Renderer]\n");
    }

    #[test]
    fn empty_atomic_write_does_not_create_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(IMGUI_INI_FILE_NAME);

        assert!(!write_nonempty_text_atomic(&path, "").expect("write"));

        assert!(!path.exists());
    }

    #[test]
    fn empty_atomic_write_does_not_replace_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(IMGUI_INI_FILE_NAME);
        std::fs::write(&path, "[Window][Renderer]\n").expect("seed");

        assert!(!write_nonempty_text_atomic(&path, "").expect("write"));

        let text = std::fs::read_to_string(path).expect("read");
        assert_eq!(text, "[Window][Renderer]\n");
    }
}
