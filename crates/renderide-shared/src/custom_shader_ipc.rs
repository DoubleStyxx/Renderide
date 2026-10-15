//! Renderide custom-shader payload carried in [`crate::shared::ShaderUpload::file`].
//!
//! FrooxEngine uploads Unity shader AssetBundles through `ShaderUpload` (renderer command tag 24).
//! That command's type id is fixed by Renderite's generated polymorphic table, so RenderideMod
//! cannot add a new `RendererCommand` subclass the renderer would decode. Custom WGSL reuses the
//! same command. The production host only puts filesystem paths in `file`; a payload that starts
//! with [`RENDERIDE_CUSTOM_WGSL_PREFIX`] is authoring WGSL plus the shader asset key it replaces.

/// First line of a custom-shader `ShaderUpload.file` payload, including the trailing newline.
///
/// RenderideMod's `CustomShaderPayload` copies this string. Changing it is a wire break.
pub const RENDERIDE_CUSTOM_WGSL_PREFIX: &str = "renderide-custom-wgsl:v1\n";

/// Parsed custom-shader body from [`crate::shared::ShaderUpload::file`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomShaderPayload {
    /// Unity shader asset key, before renderer normalization.
    pub key: String,
    /// Drop a previously installed program for [`Self::key`].
    pub remove: bool,
    /// Authoring WGSL. Empty when [`Self::remove`] is set.
    pub wgsl: String,
}

/// Formats an install payload. `key` must be a single header token with no newlines.
pub fn format_custom_shader_payload(key: &str, wgsl: &str) -> String {
    format!("{RENDERIDE_CUSTOM_WGSL_PREFIX}key={key}\n---\n{wgsl}")
}

/// Formats a removal payload for `key`.
pub fn format_custom_shader_removal(key: &str) -> String {
    format!("{RENDERIDE_CUSTOM_WGSL_PREFIX}key={key}\nremove=1\n---\n")
}

/// Parses a custom-shader payload.
///
/// Returns [`None`] when `file` is not a custom-shader payload. Returns `Some(Err)` when the
/// prefix matches but the header is invalid, so callers do not treat the text as an AssetBundle path.
pub fn parse_custom_shader_payload(file: &str) -> Option<Result<CustomShaderPayload, String>> {
    let rest = file.strip_prefix(RENDERIDE_CUSTOM_WGSL_PREFIX)?;
    Some(parse_custom_shader_body(rest))
}

fn parse_custom_shader_body(rest: &str) -> Result<CustomShaderPayload, String> {
    let (header, wgsl) = split_header(rest)?;
    let mut key = None;
    let mut remove = false;
    for (index, line) in header.lines().enumerate() {
        let line_no = index + 2;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or_else(|| {
            format!("custom shader payload line {line_no} expected key=value, got `{line}`")
        })?;
        match name.trim() {
            "key" => {
                let value = value.trim();
                if value.is_empty() || value.contains(['\n', '\r']) {
                    return Err(format!(
                        "custom shader payload line {line_no} has an empty key"
                    ));
                }
                key = Some(value.to_string());
            }
            "remove" => {
                remove = matches!(value.trim(), "1" | "true" | "yes");
            }
            other => {
                return Err(format!(
                    "custom shader payload line {line_no} has unknown header `{other}`"
                ));
            }
        }
    }
    let key = key.ok_or_else(|| "custom shader payload is missing `key=`".to_string())?;
    if !remove && wgsl.trim().is_empty() {
        return Err(format!("custom shader payload for `{key}` has empty WGSL"));
    }
    Ok(CustomShaderPayload {
        key,
        remove,
        wgsl: wgsl.to_string(),
    })
}

fn split_header(rest: &str) -> Result<(&str, &str), String> {
    if let Some((header, wgsl)) = rest.split_once("\n---\n") {
        return Ok((header, wgsl));
    }
    if let Some(header) = rest.strip_suffix("\n---") {
        return Ok((header, ""));
    }
    Err("custom shader payload is missing the `---` separator before WGSL".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_install_and_removal_payloads() {
        let wgsl = "//#pass type=forward\n@fragment\nfn fs_main() {}\n";
        let encoded = format_custom_shader_payload("Unlit", wgsl);
        let parsed = parse_custom_shader_payload(&encoded)
            .expect("prefix")
            .expect("header");
        assert_eq!(parsed.key, "Unlit");
        assert!(!parsed.remove);
        assert_eq!(parsed.wgsl, wgsl);

        let removal = format_custom_shader_removal("Unlit");
        let parsed = parse_custom_shader_payload(&removal)
            .expect("prefix")
            .expect("header");
        assert!(parsed.remove);
        assert!(parsed.wgsl.is_empty());
    }

    #[test]
    fn ordinary_paths_are_not_custom_payloads() {
        assert!(parse_custom_shader_payload("/tmp/unlit.shader").is_none());
        assert!(parse_custom_shader_payload("RENDERIDE_TEST_STEM:unlit").is_none());
    }

    #[test]
    fn prefix_without_separator_is_an_error() {
        let err = parse_custom_shader_payload("renderide-custom-wgsl:v1\nkey=unlit\n")
            .expect("prefix")
            .expect_err("separator");
        assert!(err.contains("---"));
    }
}
