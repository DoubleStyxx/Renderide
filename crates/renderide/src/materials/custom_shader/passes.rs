//! Runtime `//#pass` parser for custom material WGSL.
//!
//! The syntax matches the build-time material pass directives so a stock `shaders/materials`
//! source can be loaded without a rebuild. Parsed passes still use the host's material
//! property overrides when a field's policy says so.

use crate::gpu::MAIN_FORWARD_DEPTH_COMPARE;
use crate::materials::{
    COLOR_WRITES_NONE, MaterialAlphaToCoverageMode, MaterialDepthCompareDomain, MaterialPassState,
    MaterialRenderStatePolicy, PASS_BLEND_ONE_ONE, PASS_BLEND_ONE_ONE_MINUS_SRC_ALPHA,
    PASS_BLEND_OVERLAY_NOOP_COLOR_MAX_ALPHA, PASS_BLEND_SRC_ALPHA_ONE_MINUS_SRC_ALPHA, PassType,
};

/// One authored pass before entry-point names are leaked for [`crate::materials::MaterialPassDesc`].
#[derive(Clone, Debug, PartialEq)]
pub(super) struct OwnedMaterialPass {
    /// Debug label.
    pub name: String,
    /// Semantic pass role.
    pub pass_type: PassType,
    /// Vertex entry point in the authoring source.
    pub vertex_entry: String,
    /// Fragment entry point in the authoring source.
    pub fragment_entry: String,
    /// Depth comparison fallback.
    pub depth_compare: wgpu::CompareFunction,
    /// `_ZTest` enum layout used when host material state overrides this pass.
    pub depth_compare_domain: MaterialDepthCompareDomain,
    /// Whether this pass writes depth by default.
    pub depth_write: bool,
    /// Backface culling mode (`None` = disabled).
    pub cull_mode: Option<wgpu::Face>,
    /// Color + alpha blend state, or `None` for no blending.
    pub blend: Option<wgpu::BlendState>,
    /// Color attachment write mask.
    pub write_mask: wgpu::ColorWrites,
    /// Slope-scaled depth bias.
    pub depth_bias_slope_scale: f32,
    /// Constant depth bias.
    pub depth_bias_constant: i32,
    /// Hardware alpha-to-coverage policy.
    pub alpha_to_coverage: MaterialAlphaToCoverageMode,
    /// Material-driven blend materialization mode.
    pub material_state: MaterialPassState,
    /// Per-field host render-state override policy.
    pub render_state_policy: MaterialRenderStatePolicy,
}

/// Parses every `//#pass` directive and the fragment entry it sits above.
pub(super) fn parse_pass_directives(
    source: &str,
    file: &str,
) -> Result<Vec<OwnedMaterialPass>, String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut passes = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        let line_no = line_idx + 1;
        let Some(rest) = line.trim_start().strip_prefix("//#pass") else {
            continue;
        };
        let body = rest.trim();
        if body.is_empty() {
            return Err(format!(
                "{file}:{line_no}: `//#pass` requires `type=forward` or `type=depth_prepass`"
            ));
        }
        let mut tokens = body.split_whitespace();
        let first = tokens.next().unwrap_or("");
        let Some((first_key, first_value)) = first.split_once('=') else {
            return Err(format!(
                "{file}:{line_no}: `//#pass` must start with `type=...`, got `{first}`"
            ));
        };
        if !first_key.eq_ignore_ascii_case("type") {
            return Err(format!(
                "{file}:{line_no}: first `//#pass` token must be `type=...`, got `{first}`"
            ));
        }
        let mut draft = OwnedPassDraft::for_type(parse_pass_type(first_value, file, line_no)?);
        for token in tokens {
            let (key, value) = token.split_once('=').ok_or_else(|| {
                format!("{file}:{line_no}: expected `key=value` in `//#pass`, got `{token}`")
            })?;
            apply_pass_token(key.trim(), value.trim(), file, line_no, &mut draft)?;
        }
        draft.fragment_entry = next_fragment_entry_after(&lines, line_idx + 1, file, line_no)?;
        passes.push(draft.finish());
    }
    Ok(passes)
}

/// Parses an optional `//#render_queue` directive. Absent directives use Unity Geometry (`2000`).
pub(super) fn parse_render_queue(source: &str, file: &str) -> Result<i32, String> {
    const BACKGROUND: i32 = 1000;
    const GEOMETRY: i32 = 2000;
    const ALPHA_TEST: i32 = 2450;
    const TRANSPARENT: i32 = 3000;
    const OVERLAY: i32 = 4000;

    let mut directive = None;
    for (line_idx, line) in source.lines().enumerate() {
        let line_no = line_idx + 1;
        let Some(rest) = line.trim_start().strip_prefix("//#render_queue") else {
            continue;
        };
        if directive.is_some() {
            return Err(format!(
                "{file}:{line_no}: duplicate `//#render_queue` directive"
            ));
        }
        let expression: String = rest.split_whitespace().collect();
        if expression.is_empty() {
            return Err(format!(
                "{file}:{line_no}: `//#render_queue` requires a Unity queue name"
            ));
        }
        directive = Some(parse_render_queue_expression(
            &expression,
            file,
            line_no,
            BACKGROUND,
            GEOMETRY,
            ALPHA_TEST,
            TRANSPARENT,
            OVERLAY,
        )?);
    }
    Ok(directive.unwrap_or(GEOMETRY))
}

fn parse_render_queue_expression(
    value: &str,
    file: &str,
    line: usize,
    background: i32,
    geometry: i32,
    alpha_test: i32,
    transparent: i32,
    overlay: i32,
) -> Result<i32, String> {
    let (queue_name, offset) = if let Some((offset_start, sign)) =
        value.char_indices().find(|(_, c)| *c == '+' || *c == '-')
    {
        let queue_name = &value[..offset_start];
        let offset = value[offset_start..].parse::<i32>().map_err(|err| {
            format!(
                "{file}:{line}: `//#render_queue` offset must be an integer, got `{sign}{}`: {err}",
                &value[offset_start + sign.len_utf8()..]
            )
        })?;
        (queue_name, offset)
    } else {
        (value, 0)
    };
    let base = match queue_name.to_ascii_lowercase().as_str() {
        "background" => background,
        "geometry" => geometry,
        "alphatest" => alpha_test,
        "transparent" => transparent,
        "overlay" => overlay,
        _ => {
            return Err(format!(
                "{file}:{line}: unknown `//#render_queue` queue `{queue_name}`"
            ));
        }
    };
    base.checked_add(offset)
        .ok_or_else(|| format!("{file}:{line}: `//#render_queue` value `{value}` overflows i32"))
}

/// One `//#mat_default` uniform fallback.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct OwnedMaterialDefault {
    /// Host property name.
    pub property: String,
    /// `true` when the value is a vec4. Floats use only `values[0]`.
    pub is_vec4: bool,
    /// Parsed finite components.
    pub values: [f32; 4],
}

/// Parses `//#mat_default` directives from authoring WGSL.
pub(super) fn parse_material_defaults(
    source: &str,
    file: &str,
) -> Result<Vec<OwnedMaterialDefault>, String> {
    let mut defaults = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let line_no = line_idx + 1;
        let Some(rest) = line.trim_start().strip_prefix("//#mat_default") else {
            continue;
        };
        let mut tokens = rest.split_whitespace();
        let property = tokens.next().ok_or_else(|| {
            format!("{file}:{line_no}: `//#mat_default` requires a property name")
        })?;
        let kind = tokens.next().ok_or_else(|| {
            format!("{file}:{line_no}: `//#mat_default` requires `float` or `vec4`")
        })?;
        if defaults.iter().any(|d| d.property == property) {
            return Err(format!(
                "{file}:{line_no}: duplicate `//#mat_default` for `{property}`"
            ));
        }
        let numbers: Vec<&str> = tokens.collect();
        let default = match kind.to_ascii_lowercase().as_str() {
            "float" | "f32" => {
                if numbers.len() != 1 {
                    return Err(format!(
                        "{file}:{line_no}: `//#mat_default` float requires one value"
                    ));
                }
                OwnedMaterialDefault {
                    property: property.to_string(),
                    is_vec4: false,
                    values: [parse_finite_f32(numbers[0], file, line_no)?, 0.0, 0.0, 0.0],
                }
            }
            "vec4" | "float4" => {
                if numbers.len() != 4 {
                    return Err(format!(
                        "{file}:{line_no}: `//#mat_default` vec4 requires four values"
                    ));
                }
                OwnedMaterialDefault {
                    property: property.to_string(),
                    is_vec4: true,
                    values: [
                        parse_finite_f32(numbers[0], file, line_no)?,
                        parse_finite_f32(numbers[1], file, line_no)?,
                        parse_finite_f32(numbers[2], file, line_no)?,
                        parse_finite_f32(numbers[3], file, line_no)?,
                    ],
                }
            }
            other => {
                return Err(format!(
                    "{file}:{line_no}: unknown `//#mat_default` kind `{other}`"
                ));
            }
        };
        defaults.push(default);
    }
    Ok(defaults)
}

struct OwnedPassDraft {
    pass_type: PassType,
    name: String,
    vertex_entry: String,
    fragment_entry: String,
    depth_compare: wgpu::CompareFunction,
    depth_compare_domain: MaterialDepthCompareDomain,
    depth_write: bool,
    cull_mode: Option<wgpu::Face>,
    blend: Option<wgpu::BlendState>,
    write_mask: wgpu::ColorWrites,
    depth_bias_slope_scale: f32,
    depth_bias_constant: i32,
    alpha_to_coverage: MaterialAlphaToCoverageMode,
    material_state: MaterialPassState,
    render_state_policy: MaterialRenderStatePolicy,
}

impl OwnedPassDraft {
    fn for_type(pass_type: PassType) -> Self {
        match pass_type {
            PassType::Forward => Self {
                pass_type,
                name: "forward".to_string(),
                vertex_entry: "vs_main".to_string(),
                fragment_entry: String::new(),
                depth_compare: MAIN_FORWARD_DEPTH_COMPARE,
                depth_compare_domain: MaterialDepthCompareDomain::FrooxZTest,
                depth_write: true,
                cull_mode: Some(wgpu::Face::Back),
                blend: None,
                write_mask: wgpu::ColorWrites::COLOR,
                depth_bias_slope_scale: 0.0,
                depth_bias_constant: 0,
                alpha_to_coverage: MaterialAlphaToCoverageMode::Off,
                material_state: MaterialPassState::Forward,
                render_state_policy: MaterialRenderStatePolicy::ALL_MATERIAL,
            },
            PassType::DepthPrepass => {
                let mut policy = MaterialRenderStatePolicy::ALL_MATERIAL;
                policy.color_mask = false;
                policy.depth_write = false;
                Self {
                    pass_type,
                    name: "depth_prepass".to_string(),
                    vertex_entry: "vs_main".to_string(),
                    fragment_entry: String::new(),
                    depth_compare: MAIN_FORWARD_DEPTH_COMPARE,
                    depth_compare_domain: MaterialDepthCompareDomain::FrooxZTest,
                    depth_write: true,
                    cull_mode: Some(wgpu::Face::Back),
                    blend: None,
                    write_mask: COLOR_WRITES_NONE,
                    depth_bias_slope_scale: 0.0,
                    depth_bias_constant: 0,
                    alpha_to_coverage: MaterialAlphaToCoverageMode::Off,
                    material_state: MaterialPassState::Static,
                    render_state_policy: policy,
                }
            }
        }
    }

    fn finish(self) -> OwnedMaterialPass {
        OwnedMaterialPass {
            name: self.name,
            pass_type: self.pass_type,
            vertex_entry: self.vertex_entry,
            fragment_entry: self.fragment_entry,
            depth_compare: self.depth_compare,
            depth_compare_domain: self.depth_compare_domain,
            depth_write: self.depth_write,
            cull_mode: self.cull_mode,
            blend: self.blend,
            write_mask: self.write_mask,
            depth_bias_slope_scale: self.depth_bias_slope_scale,
            depth_bias_constant: self.depth_bias_constant,
            alpha_to_coverage: self.alpha_to_coverage,
            material_state: self.material_state,
            render_state_policy: self.render_state_policy,
        }
    }
}

fn apply_pass_token(
    key: &str,
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    match key.to_ascii_lowercase().as_str() {
        "type" => Err(format!(
            "{file}:{line}: `//#pass` accepts exactly one `type=` token"
        )),
        "name" => {
            if value.is_empty()
                || !value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(format!(
                    "{file}:{line}: `//#pass` name must be ASCII letters, digits, `_`, or `-`"
                ));
            }
            draft.name = value.to_string();
            Ok(())
        }
        "vs" | "vertex" => {
            draft.vertex_entry = value.to_string();
            Ok(())
        }
        "blend" => apply_blend(value, file, line, draft),
        "zwrite" | "z_write" | "depth_write" | "depthwrite" => {
            apply_zwrite(value, file, line, draft)
        }
        "ztest" | "z_test" | "depth_compare" | "depthcompare" => {
            apply_ztest(value, file, line, draft)
        }
        "cull" => apply_cull(value, file, line, draft),
        "color_mask" | "colormask" | "write_mask" | "writemask" => {
            apply_color_mask(value, file, line, draft)
        }
        "a2c" | "alpha_to_coverage" => {
            draft.alpha_to_coverage = parse_alpha_to_coverage(value, file, line)?;
            Ok(())
        }
        "stencil" => apply_stencil(value, file, line, draft),
        "offset" => apply_offset(value, file, line, draft),
        other => Err(format!("{file}:{line}: unknown `//#pass` key `{other}`")),
    }
}

fn apply_blend(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    match value.to_ascii_lowercase().as_str() {
        "off" | "none" => {
            draft.blend = None;
            draft.material_state = MaterialPassState::Static;
        }
        "alpha" | "src_alpha" | "srcalpha" => {
            draft.blend = Some(PASS_BLEND_SRC_ALPHA_ONE_MINUS_SRC_ALPHA);
            draft.write_mask = wgpu::ColorWrites::ALL;
            draft.material_state = MaterialPassState::Static;
        }
        "additive" | "one_one" | "oneone" => {
            draft.blend = Some(PASS_BLEND_ONE_ONE);
            draft.write_mask = wgpu::ColorWrites::ALL;
            draft.material_state = MaterialPassState::Static;
        }
        "premul" | "premultiplied" | "premultiplied_alpha" => {
            draft.blend = Some(PASS_BLEND_ONE_ONE_MINUS_SRC_ALPHA);
            draft.write_mask = wgpu::ColorWrites::ALL;
            draft.material_state = MaterialPassState::Static;
        }
        "material" => {
            draft.blend = None;
            draft.material_state = MaterialPassState::Forward;
        }
        "material_filter" | "filter" => {
            draft.blend = None;
            draft.material_state = MaterialPassState::Filter;
        }
        "material_overlay" | "overlay" => {
            draft.blend = Some(PASS_BLEND_OVERLAY_NOOP_COLOR_MAX_ALPHA);
            draft.write_mask = wgpu::ColorWrites::ALL;
            draft.material_state = MaterialPassState::Overlay;
        }
        "transparent_material" | "material_transparent" => {
            draft.blend = Some(PASS_BLEND_ONE_ONE_MINUS_SRC_ALPHA);
            draft.write_mask = wgpu::ColorWrites::ALL;
            draft.depth_write = false;
            draft.material_state = MaterialPassState::TransparentForward;
        }
        other => {
            return Err(format!(
                "{file}:{line}: `//#pass` blend value `{other}` is not supported"
            ));
        }
    }
    Ok(())
}

fn apply_zwrite(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    if let Some(inner) = material_inner(value, "material") {
        draft.depth_write = parse_bool_like(inner, file, line, "zwrite")?;
        draft.render_state_policy.depth_write = true;
        return Ok(());
    }
    draft.depth_write = parse_bool_like(value, file, line, "zwrite")?;
    draft.render_state_policy.depth_write = false;
    Ok(())
}

fn apply_ztest(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    if let Some(inner) =
        material_inner(value, "material_froox").or_else(|| material_inner(value, "material"))
    {
        draft.depth_compare = parse_depth_compare(inner, file, line)?;
        draft.depth_compare_domain = MaterialDepthCompareDomain::FrooxZTest;
        draft.render_state_policy.depth_compare = true;
        return Ok(());
    }
    if let Some(inner) = material_inner(value, "material_unity") {
        draft.depth_compare = parse_depth_compare(inner, file, line)?;
        draft.depth_compare_domain = MaterialDepthCompareDomain::UnityCompareFunction;
        draft.render_state_policy.depth_compare = true;
        return Ok(());
    }
    draft.depth_compare = parse_depth_compare(value, file, line)?;
    draft.depth_compare_domain = MaterialDepthCompareDomain::FrooxZTest;
    draft.render_state_policy.depth_compare = false;
    Ok(())
}

fn apply_cull(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    if let Some(inner) = material_inner(value, "material") {
        draft.cull_mode = parse_cull(inner, file, line)?;
        draft.render_state_policy.cull = true;
        return Ok(());
    }
    draft.cull_mode = parse_cull(value, file, line)?;
    draft.render_state_policy.cull = false;
    Ok(())
}

fn apply_color_mask(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    if let Some(inner) = material_inner(value, "material") {
        draft.write_mask = parse_color_mask(inner, file, line)?;
        draft.render_state_policy.color_mask = true;
        return Ok(());
    }
    draft.write_mask = parse_color_mask(value, file, line)?;
    draft.render_state_policy.color_mask = false;
    Ok(())
}

fn apply_stencil(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    match value.to_ascii_lowercase().as_str() {
        "material" => draft.render_state_policy.stencil = true,
        "off" | "none" => draft.render_state_policy.stencil = false,
        other => {
            return Err(format!(
                "{file}:{line}: `//#pass` stencil expects material or off, got `{other}`"
            ));
        }
    }
    Ok(())
}

fn apply_offset(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    if let Some(inner) = material_inner(value, "material") {
        apply_offset_pair(inner, file, line, draft)?;
        draft.render_state_policy.depth_offset = true;
        return Ok(());
    }
    apply_offset_pair(value, file, line, draft)?;
    draft.render_state_policy.depth_offset = false;
    Ok(())
}

fn apply_offset_pair(
    value: &str,
    file: &str,
    line: usize,
    draft: &mut OwnedPassDraft,
) -> Result<(), String> {
    let (factor, units) = value.split_once(',').ok_or_else(|| {
        format!("{file}:{line}: `//#pass` offset expects `factor,units`, got `{value}`")
    })?;
    let factor = parse_finite_f32(factor.trim(), file, line)?;
    let units = parse_finite_f32(units.trim(), file, line)?;
    draft.depth_bias_slope_scale = if factor == 0.0 { 0.0 } else { -factor };
    draft.depth_bias_constant = unity_offset_units(units).saturating_neg();
    Ok(())
}

fn parse_pass_type(value: &str, file: &str, line: usize) -> Result<PassType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "forward" => Ok(PassType::Forward),
        "depth_prepass" | "depthprepass" | "prepass" => Ok(PassType::DepthPrepass),
        other => Err(format!(
            "{file}:{line}: unknown `//#pass` type `{other}` (allowed: forward, depth_prepass)"
        )),
    }
}

fn parse_depth_compare(
    value: &str,
    file: &str,
    line: usize,
) -> Result<wgpu::CompareFunction, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "main" | "forward" | "default" => Ok(MAIN_FORWARD_DEPTH_COMPARE),
        "always" => Ok(wgpu::CompareFunction::Always),
        "less" => Ok(wgpu::CompareFunction::Less),
        "greater_equal" | "greaterequal" | "gequal" | "g_equal" => {
            Ok(wgpu::CompareFunction::GreaterEqual)
        }
        other => Err(format!(
            "{file}:{line}: `//#pass` ztest expects main, always, less, or greater_equal, got `{other}`"
        )),
    }
}

fn parse_cull(value: &str, file: &str, line: usize) -> Result<Option<wgpu::Face>, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "back" => Ok(Some(wgpu::Face::Back)),
        "front" => Ok(Some(wgpu::Face::Front)),
        "off" | "none" => Ok(None),
        other => Err(format!(
            "{file}:{line}: `//#pass` cull expects back, front, or off, got `{other}`"
        )),
    }
}

fn parse_color_mask(value: &str, file: &str, line: usize) -> Result<wgpu::ColorWrites, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "rgba" | "all" | "15" => Ok(wgpu::ColorWrites::ALL),
        "rgb" | "color" | "7" => Ok(wgpu::ColorWrites::COLOR),
        "0" | "none" | "off" => Ok(COLOR_WRITES_NONE),
        other => Err(format!(
            "{file}:{line}: `//#pass` color_mask expects rgba, rgb, or 0, got `{other}`"
        )),
    }
}

fn parse_alpha_to_coverage(
    value: &str,
    file: &str,
    line: usize,
) -> Result<MaterialAlphaToCoverageMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" | "always" => Ok(MaterialAlphaToCoverageMode::Always),
        "false" | "0" | "no" | "off" => Ok(MaterialAlphaToCoverageMode::Off),
        "cutout" | "alpha_test" | "alphatest" => Ok(MaterialAlphaToCoverageMode::Cutout),
        other => Err(format!(
            "{file}:{line}: `//#pass` a2c expects off, on, or cutout, got `{other}`"
        )),
    }
}

fn parse_bool_like(value: &str, file: &str, line: usize, key: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!(
            "{file}:{line}: `//#pass` `{key}` expects on/off, got `{other}`"
        )),
    }
}

fn material_inner<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .strip_prefix(prefix)?
        .strip_prefix('(')?
        .strip_suffix(')')
        .map(str::trim)
}

fn parse_finite_f32(value: &str, file: &str, line: usize) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|err| format!("{file}:{line}: expected a finite f32, got `{value}`: {err}"))?;
    if !parsed.is_finite() {
        return Err(format!(
            "{file}:{line}: expected a finite f32, got `{value}`"
        ));
    }
    Ok(parsed)
}

fn unity_offset_units(value: f32) -> i32 {
    let rounded = value.round();
    if rounded >= i32::MAX as f32 {
        i32::MAX
    } else if rounded <= i32::MIN as f32 {
        i32::MIN
    } else {
        rounded as i32
    }
}

fn next_fragment_entry_after(
    source_lines: &[&str],
    start_line: usize,
    file: &str,
    directive_line_no: usize,
) -> Result<String, String> {
    let mut saw_attribute = false;
    for line in &source_lines[start_line..] {
        let trimmed = line.trim_start();
        if !saw_attribute {
            if trimmed.starts_with("//") || trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("@fragment") {
                if let Some(name) = parse_fn_name(rest.trim_start()) {
                    return Ok(name);
                }
                saw_attribute = true;
                continue;
            }
            return Err(format!(
                "{file}:{directive_line_no}: `//#pass` must immediately precede an `@fragment` entry point"
            ));
        }
        if trimmed.starts_with("//") || trimmed.is_empty() {
            continue;
        }
        if let Some(name) = parse_fn_name(trimmed) {
            return Ok(name);
        }
        return Err(format!(
            "{file}:{directive_line_no}: expected `fn <name>` after `@fragment`"
        ));
    }
    Err(format!(
        "{file}:{directive_line_no}: `//#pass` has no following `@fragment` entry point"
    ))
}

fn parse_fn_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("fn ")?.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stock_unlit_forward_pass_metadata() {
        let source = r#"
//#pass type=forward name=forward_filter blend=material_filter zwrite=material(on) ztest=material_froox(main) cull=material(back) color_mask=rgba offset=material(0,0) a2c=cutout
@fragment
fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }
"#;
        let passes = parse_pass_directives(source, "unlit.wgsl").expect("pass");
        assert_eq!(passes.len(), 1);
        let pass = &passes[0];
        assert_eq!(pass.name, "forward_filter");
        assert_eq!(pass.fragment_entry, "fs_main");
        assert_eq!(pass.vertex_entry, "vs_main");
        assert_eq!(pass.material_state, MaterialPassState::Filter);
        assert!(pass.depth_write);
        assert!(pass.render_state_policy.depth_write);
        assert_eq!(pass.cull_mode, Some(wgpu::Face::Back));
        assert!(pass.render_state_policy.cull);
        assert_eq!(pass.write_mask, wgpu::ColorWrites::ALL);
        assert!(!pass.render_state_policy.color_mask);
        assert!(pass.render_state_policy.depth_offset);
        assert_eq!(pass.alpha_to_coverage, MaterialAlphaToCoverageMode::Cutout);
        assert_eq!(
            pass.depth_compare_domain,
            MaterialDepthCompareDomain::FrooxZTest
        );
    }

    #[test]
    fn parses_named_render_queue_and_material_default() {
        let source = "//#render_queue Transparent-100\n//#mat_default _Color vec4 0.2 0.4 0.6 1\n";
        assert_eq!(parse_render_queue(source, "s.wgsl").expect("queue"), 2900);
        let defaults = parse_material_defaults(source, "s.wgsl").expect("defaults");
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].property, "_Color");
        assert_eq!(defaults[0].values, [0.2, 0.4, 0.6, 1.0]);
    }
}
