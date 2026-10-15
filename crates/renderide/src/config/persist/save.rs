//! Atomic TOML persistence for [`super::types::RendererSettings`].
//!
//! Splits the file IO out of [`super::load`] so the load and save sides can be reasoned about
//! independently. The atomic-write path uses a `.<file>.tmp` sibling and `rename`, which is
//! atomic on every supported OS.

use std::io;
use std::path::Path;

use toml_edit::{DocumentMut, Table};

use super::resolve::FILE_NAME_TOML;
use crate::config::types::RendererSettings;

/// Writes `settings` to `path` as TOML atomically while preserving unknown existing keys.
///
/// Known keys are replaced with the current serialized settings. Keys and tables that are not
/// emitted by this renderer version are left intact so developers can move between renderer builds
/// during bisection without losing newer or older config knobs.
pub fn save_renderer_settings(path: &Path, settings: &RendererSettings) -> io::Result<()> {
    let mut document = serialized_settings_document(settings)?;
    match std::fs::read_to_string(path) {
        Ok(existing) => {
            if let Ok(mut existing_document) = existing.parse::<DocumentMut>() {
                merge_known_settings(existing_document.as_table_mut(), document.as_table());
                document = existing_document;
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    atomic_write_toml(path, &document.to_string())
}

/// Writes only the current renderer settings schema to `path`, pruning unknown existing keys.
///
/// This is intended for default config creation and explicit user cleanup. Normal config saves
/// should use [`save_renderer_settings`] so version-specific keys survive renderer bisection.
pub fn save_renderer_settings_pruned(path: &Path, settings: &RendererSettings) -> io::Result<()> {
    let document = serialized_settings_document(settings)?;
    atomic_write_toml(path, &document.to_string())
}

/// Writes a pre-edited renderer config TOML document atomically.
pub(super) fn save_migrated_renderer_config(path: &Path, contents: &str) -> io::Result<()> {
    atomic_write_toml(path, contents)
}

fn serialized_settings_document(settings: &RendererSettings) -> io::Result<DocumentMut> {
    let contents = toml::to_string_pretty(settings).map_err(toml_serialize_error)?;
    contents.parse::<DocumentMut>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Serialized renderer config was not valid TOML: {e}"),
        )
    })
}

fn toml_serialize_error(e: toml::ser::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("TOML serialization failed: {e}"),
    )
}

fn atomic_write_toml(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(FILE_NAME_TOML);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, contents.as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn merge_known_settings(existing: &mut Table, canonical: &Table) {
    for (key, canonical_item) in canonical {
        if let Some(existing_item) = existing.get_mut(key)
            && let (Some(existing_table), Some(canonical_table)) =
                (existing_item.as_table_mut(), canonical_item.as_table())
        {
            merge_known_settings(existing_table, canonical_table);
            continue;
        }
        existing.insert(key, canonical_item.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::{save_renderer_settings, save_renderer_settings_pruned};
    use crate::config::types::RendererSettings;

    #[test]
    fn atomic_save_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let s = RendererSettings::from_defaults();
        save_renderer_settings(&path, &s).expect("save");
        let text = std::fs::read_to_string(&path).expect("read");
        let s2: RendererSettings = toml::from_str(&text).expect("toml");
        assert_eq!(s, s2);
    }

    #[test]
    fn default_save_writes_current_config_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");

        save_renderer_settings(&path, &RendererSettings::from_defaults()).expect("save");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(
            text.contains(&format!(
                "config_version = \"{}\"",
                RendererSettings::CURRENT_CONFIG_VERSION
            )),
            "got:\n{text}"
        );
    }

    #[test]
    fn toml_roundtrip_string() {
        let s = RendererSettings::from_defaults();
        let text = toml::to_string_pretty(&s).expect("ser");
        let s2: RendererSettings = toml::from_str(&text).expect("de");
        assert_eq!(s, s2);
    }

    #[test]
    fn normal_save_preserves_unknown_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[display]
focused_fps = 30
future_display_key = "keep"

[future_renderer]
mode = "keep"

[post_processing.tonemap]
mode = "none"
future_curve_knob = 7
"#,
        )
        .expect("write fixture");

        let mut settings = RendererSettings::from_defaults();
        settings.display.focused_fps_cap = 144;
        save_renderer_settings(&path, &settings).expect("save");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains("focused_fps = 144"), "got:\n{text}");
        assert!(
            text.contains("future_display_key = \"keep\""),
            "got:\n{text}"
        );
        assert!(text.contains("[future_renderer]"), "got:\n{text}");
        assert!(text.contains("future_curve_knob = 7"), "got:\n{text}");
        let decoded: RendererSettings = toml::from_str(&text).expect("deserialize");
        assert_eq!(decoded.display.focused_fps_cap, 144);
    }

    #[test]
    fn pruned_save_removes_unknown_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[display]
future_display_key = "drop"

[future_renderer]
mode = "drop"
"#,
        )
        .expect("write fixture");

        save_renderer_settings_pruned(&path, &RendererSettings::from_defaults()).expect("save");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(
            !text.contains("future_display_key"),
            "expected unknown key pruned, got:\n{text}"
        );
        assert!(
            !text.contains("[future_renderer]"),
            "expected unknown table pruned, got:\n{text}"
        );
        let decoded: RendererSettings = toml::from_str(&text).expect("deserialize");
        assert_eq!(decoded, RendererSettings::from_defaults());
    }
}
