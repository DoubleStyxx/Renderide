//! Renderide custom-shader payload carried in [`crate::shared::ShaderUpload::file`].
//!
//! FrooxEngine uploads Unity shader AssetBundles through `ShaderUpload` (renderer command tag 24).
//! That command's type id is fixed by Renderite's generated polymorphic table, so RenderideMod
//! cannot add a new `RendererCommand` subclass the renderer would decode. Custom WGSL reuses the
//! same command. The production host only puts filesystem paths in `file`; a payload that starts
//! with [`RENDERIDE_CUSTOM_WGSL_PREFIX`] is authoring WGSL, a removal, or a per-material assign/clear.
//!
//! The byte layout is the contract `RenderideMod/CUSTOM_SHADERS.md` documents. Install and removal
//! bytes are unchanged from the first custom-shader payload. Assign and clear add an `op` line.

/// First line of a custom-shader `ShaderUpload.file` payload, including the trailing newline.
///
/// RenderideMod's `CustomShaderPayload` copies this string. Changing it is a wire break.
pub const RENDERIDE_CUSTOM_WGSL_PREFIX: &str = "renderide-custom-wgsl:v1\n";

/// Maximum accepted custom-shader `ShaderUpload.file` size, in bytes.
pub const MAX_CUSTOM_SHADER_PAYLOAD_BYTES: usize = 1024 * 1024;

/// What a [`CustomShaderPayload`] asks the renderer to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomShaderOp {
    /// Compile WGSL and register it under `key`.
    Install,
    /// Drop the program registered for `key`.
    Remove,
    /// Point one material asset id at `key` without retargeting stock routes.
    Assign,
    /// Drop the per-material override for one material asset id.
    Clear,
}

/// Parsed custom-shader body from [`crate::shared::ShaderUpload::file`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomShaderPayload {
    /// Install, remove, assign, or clear.
    pub op: CustomShaderOp,
    /// Shader key. Empty for [`CustomShaderOp::Clear`].
    pub key: String,
    /// Host material asset id. Set for assign and clear.
    pub material_asset_id: Option<i32>,
    /// Authoring WGSL. Empty unless [`Self::op`] is [`CustomShaderOp::Install`].
    pub wgsl: String,
}

/// Formats an install payload. `key` must be a single header token with no newlines.
///
/// Bytes: `renderide-custom-wgsl:v1\nkey=<key>\n---\n<wgsl>`.
pub fn format_custom_shader_payload(key: &str, wgsl: &str) -> String {
    format!("{RENDERIDE_CUSTOM_WGSL_PREFIX}key={key}\n---\n{wgsl}")
}

/// Formats a removal payload for `key`.
///
/// Bytes: `renderide-custom-wgsl:v1\nkey=<key>\nremove=1\n---\n`.
pub fn format_custom_shader_removal(key: &str) -> String {
    format!("{RENDERIDE_CUSTOM_WGSL_PREFIX}key={key}\nremove=1\n---\n")
}

/// Formats an assign payload.
///
/// Bytes: `renderide-custom-wgsl:v1\nop=assign\nmaterial=<id>\nkey=<key>\n---\n`.
/// `material` is a base-10 `i32`. `ShaderUpload.assetId` stays a carrier id; the material id
/// is only this header.
pub fn format_custom_shader_assign(material_asset_id: i32, key: &str) -> String {
    format!(
        "{RENDERIDE_CUSTOM_WGSL_PREFIX}op=assign\nmaterial={material_asset_id}\nkey={key}\n---\n"
    )
}

/// Formats a clear payload.
///
/// Bytes: `renderide-custom-wgsl:v1\nop=clear\nmaterial=<id>\n---\n`.
pub fn format_custom_shader_clear(material_asset_id: i32) -> String {
    format!("{RENDERIDE_CUSTOM_WGSL_PREFIX}op=clear\nmaterial={material_asset_id}\n---\n")
}

/// Parses a custom-shader payload.
///
/// Returns [`None`] when `file` is not a custom-shader payload. Returns `Some(Err)` when the
/// prefix matches but the header is invalid, so callers do not treat the text as an AssetBundle path.
pub fn parse_custom_shader_payload(file: &str) -> Option<Result<CustomShaderPayload, String>> {
    let rest = file.strip_prefix(RENDERIDE_CUSTOM_WGSL_PREFIX)?;
    if file.len() > MAX_CUSTOM_SHADER_PAYLOAD_BYTES {
        return Some(Err(format!(
            "custom shader payload is {} bytes; cap is {MAX_CUSTOM_SHADER_PAYLOAD_BYTES}",
            file.len()
        )));
    }
    Some(parse_custom_shader_body(rest))
}

struct HeaderFields {
    op: Option<CustomShaderOp>,
    key: Option<String>,
    material_asset_id: Option<i32>,
    saw_remove: bool,
    remove: bool,
}

fn parse_custom_shader_body(rest: &str) -> Result<CustomShaderPayload, String> {
    let (header, wgsl) = split_header(rest)?;
    let fields = parse_header(header)?;
    let op = resolve_op(fields.op, fields.remove)?;
    validate_op(op, &fields, wgsl)?;
    Ok(CustomShaderPayload {
        op,
        key: fields.key.unwrap_or_default(),
        material_asset_id: fields.material_asset_id,
        wgsl: if op == CustomShaderOp::Install {
            wgsl.to_string()
        } else {
            String::new()
        },
    })
}

fn parse_header(header: &str) -> Result<HeaderFields, String> {
    let mut fields = HeaderFields {
        op: None,
        key: None,
        material_asset_id: None,
        saw_remove: false,
        remove: false,
    };
    for (index, line) in header.lines().enumerate() {
        let line_no = index + 2;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or_else(|| {
            format!("custom shader payload line {line_no} expected key=value, got `{line}`")
        })?;
        let name = name.trim();
        let value = value.trim();
        match name {
            "key" => {
                if fields.key.is_some() {
                    return Err(format!(
                        "custom shader payload line {line_no} repeats `key`"
                    ));
                }
                if value.is_empty() || value.contains(['\n', '\r']) {
                    return Err(format!(
                        "custom shader payload line {line_no} has an empty key"
                    ));
                }
                fields.key = Some(value.to_string());
            }
            "remove" => {
                if fields.saw_remove {
                    return Err(format!(
                        "custom shader payload line {line_no} repeats `remove`"
                    ));
                }
                fields.saw_remove = true;
                fields.remove = matches!(value, "1" | "true" | "yes");
            }
            "op" => {
                if fields.op.is_some() {
                    return Err(format!("custom shader payload line {line_no} repeats `op`"));
                }
                fields.op = Some(parse_op(value, line_no)?);
            }
            "material" => {
                if fields.material_asset_id.is_some() {
                    return Err(format!(
                        "custom shader payload line {line_no} repeats `material`"
                    ));
                }
                let material_asset_id = value.parse::<i32>().map_err(|_error| {
                    format!(
                        "custom shader payload line {line_no} has an invalid material id `{value}`"
                    )
                })?;
                fields.material_asset_id = Some(material_asset_id);
            }
            other => {
                return Err(format!(
                    "custom shader payload line {line_no} has unknown header `{other}`"
                ));
            }
        }
    }
    Ok(fields)
}

fn parse_op(value: &str, line_no: usize) -> Result<CustomShaderOp, String> {
    match value {
        "install" => Ok(CustomShaderOp::Install),
        "remove" => Ok(CustomShaderOp::Remove),
        "assign" => Ok(CustomShaderOp::Assign),
        "clear" => Ok(CustomShaderOp::Clear),
        other => Err(format!(
            "custom shader payload line {line_no} has unknown op `{other}`"
        )),
    }
}

fn resolve_op(op: Option<CustomShaderOp>, remove: bool) -> Result<CustomShaderOp, String> {
    match (op, remove) {
        (None | Some(CustomShaderOp::Install), false) => Ok(CustomShaderOp::Install),
        (None, true) | (Some(CustomShaderOp::Remove), _) => Ok(CustomShaderOp::Remove),
        (Some(CustomShaderOp::Install), true) => {
            Err("custom shader payload sets both `op=install` and `remove=1`".to_string())
        }
        (Some(op), true) => Err(format!("custom shader payload sets `remove=1` on `{op:?}`")),
        (Some(op), false) => Ok(op),
    }
}

fn validate_op(op: CustomShaderOp, fields: &HeaderFields, wgsl: &str) -> Result<(), String> {
    match op {
        CustomShaderOp::Install => {
            if fields.key.is_none() {
                return Err("custom shader payload is missing `key=`".to_string());
            }
            if fields.material_asset_id.is_some() {
                return Err("custom shader install payload cannot set `material=`".to_string());
            }
            if wgsl.trim().is_empty() {
                return Err(format!(
                    "custom shader payload for `{}` has empty WGSL",
                    fields.key.as_deref().unwrap_or("")
                ));
            }
        }
        CustomShaderOp::Remove => {
            if fields.key.is_none() {
                return Err("custom shader payload is missing `key=`".to_string());
            }
            if fields.material_asset_id.is_some() {
                return Err("custom shader remove payload cannot set `material=`".to_string());
            }
        }
        CustomShaderOp::Assign => {
            if fields.key.is_none() {
                return Err("custom shader assign payload is missing `key=`".to_string());
            }
            if fields.material_asset_id.is_none() {
                return Err("custom shader assign payload is missing `material=`".to_string());
            }
            if !wgsl.trim().is_empty() {
                return Err("custom shader assign payload must have an empty body".to_string());
            }
        }
        CustomShaderOp::Clear => {
            if fields.key.is_some() {
                return Err("custom shader clear payload cannot set `key=`".to_string());
            }
            if fields.material_asset_id.is_none() {
                return Err("custom shader clear payload is missing `material=`".to_string());
            }
            if !wgsl.trim().is_empty() {
                return Err("custom shader clear payload must have an empty body".to_string());
            }
        }
    }
    Ok(())
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
        assert_eq!(
            encoded,
            "renderide-custom-wgsl:v1\nkey=Unlit\n---\n//#pass type=forward\n@fragment\nfn fs_main() {}\n"
        );
        let parsed = parse_custom_shader_payload(&encoded)
            .expect("prefix")
            .expect("header");
        assert_eq!(parsed.op, CustomShaderOp::Install);
        assert_eq!(parsed.key, "Unlit");
        assert_eq!(parsed.material_asset_id, None);
        assert_eq!(parsed.wgsl, wgsl);

        let removal = format_custom_shader_removal("Unlit");
        assert_eq!(
            removal,
            "renderide-custom-wgsl:v1\nkey=Unlit\nremove=1\n---\n"
        );
        let parsed = parse_custom_shader_payload(&removal)
            .expect("prefix")
            .expect("header");
        assert_eq!(parsed.op, CustomShaderOp::Remove);
        assert!(parsed.wgsl.is_empty());
        assert_eq!(parsed.material_asset_id, None);
    }

    #[test]
    fn assign_and_clear_use_the_documented_bytes() {
        let assign = format_custom_shader_assign(42, "ExampleRainbow");
        assert_eq!(
            assign,
            "renderide-custom-wgsl:v1\nop=assign\nmaterial=42\nkey=ExampleRainbow\n---\n"
        );
        let parsed = parse_custom_shader_payload(&assign)
            .expect("prefix")
            .expect("header");
        assert_eq!(parsed.op, CustomShaderOp::Assign);
        assert_eq!(parsed.material_asset_id, Some(42));
        assert_eq!(parsed.key, "ExampleRainbow");
        assert!(parsed.wgsl.is_empty());

        let clear = format_custom_shader_clear(-7);
        assert_eq!(
            clear,
            "renderide-custom-wgsl:v1\nop=clear\nmaterial=-7\n---\n"
        );
        let parsed = parse_custom_shader_payload(&clear)
            .expect("prefix")
            .expect("header");
        assert_eq!(parsed.op, CustomShaderOp::Clear);
        assert_eq!(parsed.material_asset_id, Some(-7));
        assert!(parsed.key.is_empty());
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

    #[test]
    fn oversized_payload_is_rejected() {
        let wgsl = "a".repeat(MAX_CUSTOM_SHADER_PAYLOAD_BYTES);
        let encoded = format_custom_shader_payload("Unlit", &wgsl);
        let err = parse_custom_shader_payload(&encoded)
            .expect("prefix")
            .expect_err("cap");
        assert!(err.contains("cap"));
    }

    #[test]
    fn assign_requires_material_and_an_empty_body() {
        let missing = parse_custom_shader_payload(
            "renderide-custom-wgsl:v1\nop=assign\nkey=ExampleRainbow\n---\n",
        )
        .expect("prefix")
        .expect_err("material");
        assert!(missing.contains("material"));

        let body = parse_custom_shader_payload(
            "renderide-custom-wgsl:v1\nop=assign\nmaterial=1\nkey=ExampleRainbow\n---\nfn x() {}\n",
        )
        .expect("prefix")
        .expect_err("body");
        assert!(body.contains("empty body"));
    }

    #[test]
    fn clear_rejects_a_shader_key() {
        let err = parse_custom_shader_payload(
            "renderide-custom-wgsl:v1\nop=clear\nmaterial=1\nkey=ExampleRainbow\n---\n",
        )
        .expect("prefix")
        .expect_err("key");
        assert!(err.contains("key"));
    }
}
