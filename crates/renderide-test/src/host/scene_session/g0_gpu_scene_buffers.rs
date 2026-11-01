//! Judge for milestone G0, GPU scene buffers.
//!
//! Judge test lands before the code it judges. This module does not add
//! `experimental.gpu_scene_buffers` and does not change renderer behavior.
//!
//! What it judges: a later change may upload a persistent GPU instance buffer
//! (transform, bounds, material index) once per frame from dirty ranges, plus
//! a GPU material table. The CPU path stays the fallback. Pixels must match.
//!
//! Milestone: G0.
//! Flag: `experimental.gpu_scene_buffers` (`bool`, default `false`) on
//! `ExperimentalSettings` in `crates/renderide/src/config/types/experimental.rs`
//! (`[experimental]`). The harness key is `RENDERIDE_EXPERIMENTAL__GPU_SCENE_BUFFERS`.
//! Scenes: `many_instances` (docs/perf-parity-plan.md S10) and
//! `material_variety` (S12). Both scenes come from merged PR #295.
//! Equality: exact pixel equality, zero tolerance. No tolerance argument.
//! CPU golden: when `goldens/{scene}.png` exists, the off render must match
//! it byte for byte. Those files are not in the tree this judge was written
//! against, so a missing golden is not a failure.
//! Authored against master: a557b624c489d3b87afe832a1023841ddeeb1159.
//!
//! Expected-fail: `scene_dsl/warp_pbs.rs` labels one WARP image mismatch
//! (`expected failure (WARP, #212)`). That label is not a general expected-fail
//! switch. It does not run on lavapipe or Metal, and it does not cover a
//! missing setting. Cargo reports this case as ignored on the rust job
//! (lavapipe, WARP, and Metal) via
//! `#[ignore = "expected-fail until G0 code PR: gpu_scene_buffers"]`.
//! The code change removes the ignore. Until then, running the case fails
//! before any on-path render: unknown setting. Unknown `RENDERIDE_*` keys are
//! serde-ignored, so spawning the on variant would silently take the CPU path.

use std::path::{Path, PathBuf};
use std::time::Duration;

use image::RgbaImage;

use super::{SceneSessionConfig, SessionTemplate};

const MILESTONE: &str = "G0";
const FLAG_PATH: &str = "experimental.gpu_scene_buffers";
const ENV_KEY: &str = "RENDERIDE_EXPERIMENTAL__GPU_SCENE_BUFFERS";
const SCENES: [&str; 2] = ["many_instances", "material_variety"];
const AUTHORED_AGAINST_MASTER: &str = "a557b624c489d3b87afe832a1023841ddeeb1159";
const EQUALITY_RULE: &str = "exact pixel equality, zero tolerance";
const CAPTURE_WIDTH: u32 = 256;
const CAPTURE_HEIGHT: u32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingStatus {
    Missing,
    NotBool,
    NotDefaultOff,
    Ready,
}

fn experimental_settings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../renderide/src/config/types/experimental.rs")
}

fn cpu_golden_path(scene: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("goldens")
        .join(format!("{scene}.png"))
}

fn classify(source: &str) -> SettingStatus {
    let source = strip_line_comments(source);
    let Some(struct_body) = item_body(&source, "struct ExperimentalSettings") else {
        return SettingStatus::Missing;
    };
    match field_type(struct_body, "gpu_scene_buffers") {
        None => SettingStatus::Missing,
        Some(ty) if !is_type_name(ty, "bool") => SettingStatus::NotBool,
        Some(_) => {
            let Some(default_body) = item_body(&source, "impl Default for ExperimentalSettings")
            else {
                return SettingStatus::NotDefaultOff;
            };
            if default_sets_false(default_body, "gpu_scene_buffers") {
                SettingStatus::Ready
            } else {
                SettingStatus::NotDefaultOff
            }
        }
    }
}

fn blocked_reason(path: &Path, status: SettingStatus) -> Option<String> {
    let (detail, shape) = match status {
        SettingStatus::Ready => return None,
        SettingStatus::Missing => ("unknown setting", "no bool field on ExperimentalSettings"),
        SettingStatus::NotBool => ("setting is not a bool", "field is not bool"),
        SettingStatus::NotDefaultOff => (
            "setting is not default-off",
            "Default does not set gpu_scene_buffers: false",
        ),
    };
    Some(format!(
        "{detail}: {FLAG_PATH} ({shape}) in {}. \
         RendererSettings serde-ignores unknown RENDERIDE_* keys, so \
         {ENV_KEY}=true would silently take the CPU path. \
         Refusing to render the on variant.",
        path.display(),
    ))
}

fn run_parity() -> Result<(), String> {
    let path = experimental_settings_path();
    let source =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    if let Some(reason) = blocked_reason(&path, classify(&source)) {
        return Err(reason);
    }
    render_scenes()
}

fn render_scenes() -> Result<(), String> {
    let root = tempfile::tempdir().map_err(|err| format!("temp dir: {err}"))?;
    for scene in SCENES {
        let off = capture(scene, false, root.path())?;
        let on = capture(scene, true, root.path())?;
        exact_rgba(&off, &on).map_err(|err| format!("{scene} off vs on: {err}"))?;
        match_cpu_golden(scene, &off)?;
    }
    Ok(())
}

fn capture(scene: &str, enabled: bool, root: &Path) -> Result<RgbaImage, String> {
    let renderer = crate::scene_dsl::runner::renderer_next_to_current_exe().ok_or_else(|| {
        "renderer binary not found next to the test binary; build with `cargo build -p renderide`"
            .to_string()
    })?;
    let label = if enabled { "on" } else { "off" };
    let output = root.join(format!("{scene}-{label}.png"));
    let value = if enabled { "true" } else { "false" };
    let cfg = SceneSessionConfig {
        renderer_path: renderer,
        output_path: output,
        width: CAPTURE_WIDTH,
        height: CAPTURE_HEIGHT,
        interval_ms: 500,
        timeout: Duration::from_secs(180),
        verbose_renderer: false,
        renderer_stdout_to_stderr: false,
        child_env: vec![(ENV_KEY.to_string(), value.to_string())],
        physically_based: false,
        scene_color_readback: false,
    };
    let outcome =
        super::run_session(&cfg, template_for(scene)?).map_err(|err| format!("{scene}: {err}"))?;
    crate::image_io::load_rgba(&outcome.png_path).map_err(|err| format!("{scene}: {err}"))
}

fn template_for(scene: &str) -> Result<SessionTemplate, String> {
    match scene {
        "many_instances" => Ok(SessionTemplate::ManyInstances),
        "material_variety" => Ok(SessionTemplate::MaterialVariety),
        other => Err(format!("unknown judge scene: {other}")),
    }
}

/// Byte equality of two captures. There is no tolerance argument.
fn exact_rgba(left: &RgbaImage, right: &RgbaImage) -> Result<(), String> {
    if left.dimensions() != right.dimensions() {
        let (lw, lh) = left.dimensions();
        let (rw, rh) = right.dimensions();
        return Err(format!(
            "pixel mismatch: dimensions {lw}x{lh} vs {rw}x{rh} ({EQUALITY_RULE})"
        ));
    }
    let mut pixels = 0u64;
    for (left_px, right_px) in left.pixels().zip(right.pixels()) {
        if left_px.0 != right_px.0 {
            pixels += 1;
        }
    }
    if pixels != 0 {
        return Err(format!(
            "pixel mismatch: {pixels} pixels differ ({EQUALITY_RULE})"
        ));
    }
    Ok(())
}

fn match_cpu_golden(scene: &str, off: &RgbaImage) -> Result<(), String> {
    match_cpu_golden_at(off, &cpu_golden_path(scene))
}

fn match_cpu_golden_at(off: &RgbaImage, golden_path: &Path) -> Result<(), String> {
    if !golden_path.is_file() {
        return Ok(());
    }
    let golden = crate::image_io::load_rgba(golden_path)
        .map_err(|err| format!("CPU golden {}: {err}", golden_path.display()))?;
    exact_rgba(off, &golden).map_err(|err| {
        format!(
            "off render does not match CPU golden {}: {err}",
            golden_path.display()
        )
    })
}

fn strip_line_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        out.push_str(line_without_comment(line));
        out.push('\n');
    }
    out
}

fn line_without_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '/' && line[index + ch.len_utf8()..].starts_with('/') {
            return &line[..index];
        }
    }
    line
}

fn item_body<'a>(source: &'a str, header: &str) -> Option<&'a str> {
    let start = source.find(header)?;
    let after = &source[start + header.len()..];
    let brace = after.find('{')?;
    brace_body(&after[brace + 1..])
}

fn brace_body(source: &str) -> Option<&str> {
    let mut depth = 1i32;
    for (index, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&source[..index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn field_type<'a>(body: &'a str, field: &str) -> Option<&'a str> {
    for line in body.lines() {
        let Some(rest) = after_visibility(line.trim()) else {
            continue;
        };
        let Some(after_name) = rest.trim_start().strip_prefix(field) else {
            continue;
        };
        let after_name = after_name.trim_start();
        let Some(ty) = after_name.strip_prefix(':') else {
            continue;
        };
        let ty = ty.trim().trim_end_matches(',').trim();
        return Some(ty);
    }
    None
}

fn after_visibility(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("pub")?;
    let rest = rest.trim_start();
    if let Some(rest) = rest.strip_prefix('(') {
        let end = rest.find(')')?;
        return Some(rest[end + 1..].trim_start());
    }
    Some(rest)
}

fn is_type_name(ty: &str, name: &str) -> bool {
    let Some(rest) = ty.strip_prefix(name) else {
        return false;
    };
    rest.chars()
        .next()
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}

fn default_sets_false(body: &str, field: &str) -> bool {
    for line in body.lines() {
        if assignment_is_false(line, field) {
            return true;
        }
    }
    false
}

/// `gpu_scene_buffers: false` on its own line, or on `Self { gpu_scene_buffers: false }`.
fn assignment_is_false(line: &str, field: &str) -> bool {
    let Some(index) = line.find(field) else {
        return false;
    };
    if line[..index]
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return false;
    }
    let after = line[index + field.len()..].trim_start();
    let Some(after) = after.strip_prefix(':') else {
        return false;
    };
    let value = after.trim().trim_end_matches([',', '}']).trim();
    value.split_whitespace().next() == Some("false")
}

#[test]
#[ignore = "expected-fail until G0 code PR: gpu_scene_buffers"]
fn gpu_scene_buffers_parity() {
    run_parity().unwrap_or_else(|err| panic!("{err}"));
}

#[cfg(test)]
mod tests {
    use super::{
        AUTHORED_AGAINST_MASTER, ENV_KEY, EQUALITY_RULE, FLAG_PATH, MILESTONE, SCENES,
        SettingStatus, blocked_reason, classify, exact_rgba, match_cpu_golden_at,
    };
    use image::RgbaImage;

    const WITHOUT_FLAG: &str = r#"
pub struct ExperimentalSettings {
    pub max_local_reflection_probes: usize,
    pub reflection_probe_sh2_enabled: bool,
}
impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self {
            max_local_reflection_probes: 8,
            reflection_probe_sh2_enabled: false,
        }
    }
}
"#;

    const DEFAULT_OFF: &str = r#"
pub struct ExperimentalSettings {
    /// experimental.gpu_scene_buffers stays a comment here.
    // pub gpu_scene_buffers: bool,
    pub gpu_scene_buffers: bool,
}
impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self { gpu_scene_buffers: false }
    }
}
"#;

    #[test]
    fn missing_flag_is_an_unknown_setting_and_does_not_render() {
        let status = classify(WITHOUT_FLAG);
        assert_eq!(status, SettingStatus::Missing);
        let reason = blocked_reason(std::path::Path::new("experimental.rs"), status)
            .expect("missing flag blocks the on variant");
        assert!(reason.starts_with("unknown setting: experimental.gpu_scene_buffers"));
        assert!(reason.contains(ENV_KEY));
        assert!(reason.contains("Refusing to render the on variant"));
        assert!(reason.contains("silently take the CPU path"));
    }

    #[test]
    fn comment_does_not_declare_the_flag() {
        let source = r#"
pub struct ExperimentalSettings {
    /// gpu_scene_buffers is not a field yet.
    // pub gpu_scene_buffers: bool,
    pub reflection_probe_sh2_enabled: bool,
}
impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self { reflection_probe_sh2_enabled: false }
    }
}
"#;
        assert_eq!(classify(source), SettingStatus::Missing);
    }

    #[test]
    fn default_off_bool_is_ready_to_compare() {
        assert_eq!(classify(DEFAULT_OFF), SettingStatus::Ready);
        assert!(
            blocked_reason(
                std::path::Path::new("experimental.rs"),
                SettingStatus::Ready
            )
            .is_none()
        );
    }

    #[test]
    fn default_true_is_not_the_off_path() {
        let source = r#"
pub struct ExperimentalSettings {
    pub gpu_scene_buffers: bool,
}
impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self { gpu_scene_buffers: true }
    }
}
"#;
        let status = classify(source);
        assert_eq!(status, SettingStatus::NotDefaultOff);
        let reason = blocked_reason(std::path::Path::new("experimental.rs"), status)
            .expect("default true blocks");
        assert!(reason.contains("not default-off"));
        assert!(reason.contains("Refusing to render the on variant"));
    }

    #[test]
    fn non_bool_field_is_rejected() {
        let source = r#"
pub struct ExperimentalSettings {
    pub gpu_scene_buffers: u32,
}
impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self { gpu_scene_buffers: false }
    }
}
"#;
        assert_eq!(classify(source), SettingStatus::NotBool);
    }

    #[test]
    fn exact_equality_rejects_one_byte_and_accepts_the_same_buffer() {
        let left = RgbaImage::from_pixel(2, 2, image::Rgba([9, 8, 7, 255]));
        assert!(exact_rgba(&left, &left).is_ok(), "{EQUALITY_RULE}");
        let mut right = left.clone();
        right.put_pixel(1, 0, image::Rgba([9, 8, 6, 255]));
        let err = exact_rgba(&left, &right).expect_err("one byte");
        assert!(err.contains("1 pixels differ"));
        assert!(err.contains("zero tolerance"));
        let other = RgbaImage::from_pixel(3, 2, image::Rgba([9, 8, 7, 255]));
        let err = exact_rgba(&left, &other).expect_err("dimensions");
        assert!(err.contains("dimensions"));
        assert!(err.contains("zero tolerance"));
    }

    #[test]
    fn missing_cpu_golden_is_not_a_failure_and_a_present_one_must_match() {
        let image = RgbaImage::from_pixel(2, 1, image::Rgba([4, 5, 6, 255]));
        let dir = tempfile::tempdir().expect("temp");
        let missing = dir.path().join("many_instances.png");
        assert!(match_cpu_golden_at(&image, &missing).is_ok());

        let golden = dir.path().join("material_variety.png");
        crate::image_io::save_rgba(&image, &golden).expect("save golden");
        assert!(match_cpu_golden_at(&image, &golden).is_ok());

        let drifted = RgbaImage::from_pixel(2, 1, image::Rgba([4, 5, 7, 255]));
        let err = match_cpu_golden_at(&drifted, &golden).expect_err("drift");
        assert!(err.contains("CPU golden"));
        assert!(err.contains("zero tolerance"));
    }

    #[test]
    fn provenance_records_milestone_scenes_flag_and_master() {
        assert_eq!(MILESTONE, "G0");
        assert_eq!(FLAG_PATH, "experimental.gpu_scene_buffers");
        assert_eq!(SCENES, ["many_instances", "material_variety"]);
        assert_eq!(
            AUTHORED_AGAINST_MASTER,
            "a557b624c489d3b87afe832a1023841ddeeb1159"
        );
        assert_eq!(AUTHORED_AGAINST_MASTER.len(), 40);
        assert!(
            AUTHORED_AGAINST_MASTER
                .chars()
                .all(|ch| ch.is_ascii_hexdigit())
        );
        assert_eq!(EQUALITY_RULE, "exact pixel equality, zero tolerance");
        assert_eq!(ENV_KEY, "RENDERIDE_EXPERIMENTAL__GPU_SCENE_BUFFERS");
    }
}
