//! Per-source shader composition.

use naga::ShaderStage;
use naga::valid::Capabilities;
use naga_oil::compose::{Composer, NagaModuleDescriptor, ShaderDefValue, ShaderType};

use super::directives::BuildPassDirective;
use super::error::BuildError;
use super::mirror_once::rewrite_material_mirror_once_wgsl;
use super::model::{
    CompiledShader, CompiledShaderTarget, ShaderJob, ShaderSourceClass, ShaderSourceManifest,
    ShaderVariant,
};
use super::modules::{ShaderModuleSources, register_composable_modules};
use super::source::shader_source_manifest;
use super::validation::{
    module_to_wgsl, validate_entry_points, validate_no_pipeline_state_uniform_fields,
    validate_pass_interfaces,
};

/// Composes one source variant through naga-oil.
fn compose_source_variant(
    modules: &ShaderModuleSources,
    source: &str,
    file_path: &str,
    variant: ShaderVariant,
) -> Result<naga::Module, BuildError> {
    compose_source_variant_with_keywords(modules, source, file_path, variant, &[])
}

/// Composes one view variant with extra `//#keyword` shader defs.
///
/// `#ifdef NAME` is true when `NAME` is present. The default variant omits every keyword, the
/// same way [`ShaderVariant::shader_defs`] omits `MULTIVIEW`.
fn compose_source_variant_with_keywords(
    modules: &ShaderModuleSources,
    source: &str,
    file_path: &str,
    variant: ShaderVariant,
    keywords: &[String],
) -> Result<naga::Module, BuildError> {
    let mut composer = Composer::default().with_capabilities(Capabilities::all());
    register_composable_modules(&mut composer, modules)?;
    let mut shader_defs = variant.shader_defs();
    for keyword in keywords {
        shader_defs.insert(keyword.clone(), ShaderDefValue::Bool(true));
    }
    composer
        .make_naga_module(NagaModuleDescriptor {
            source,
            file_path,
            shader_type: ShaderType::Wgsl,
            shader_defs: std::collections::HashMap::from_iter(shader_defs),
            ..Default::default()
        })
        .map_err(|e| {
            BuildError::Message(format!(
                "compose {file_path}: {}",
                e.emit_to_string(&composer)
            ))
        })
}

/// Checks the `@builtin(view_index)` contract for variant-sensitive outputs.
fn validate_view_index_contract(
    target_stem: &str,
    wgsl: &str,
    variant: ShaderVariant,
) -> Result<(), BuildError> {
    let has = wgsl.contains("@builtin(view_index)");
    if variant.expects_view_index() != has {
        return Err(BuildError::Message(format!(
            "{target_stem}: expected @builtin(view_index) {} in output (multiview shader_defs contract)",
            if variant.expects_view_index() {
                "present"
            } else {
                "absent"
            }
        )));
    }
    Ok(())
}

fn remapped_pass_directives_for_output(
    source_module: &naga::Module,
    output_wgsl: &str,
    pass_directives: &[BuildPassDirective],
    label: &str,
) -> Result<Vec<BuildPassDirective>, BuildError> {
    if pass_directives.is_empty() {
        return Ok(Vec::new());
    }
    let output_module = naga::front::wgsl::parse_str(output_wgsl).map_err(|e| {
        BuildError::Message(format!(
            "parse flattened WGSL {label}: {}",
            e.emit_to_string(output_wgsl)
        ))
    })?;
    let vertex_names =
        entry_point_name_pairs(source_module, &output_module, ShaderStage::Vertex, label)?;
    let fragment_names =
        entry_point_name_pairs(source_module, &output_module, ShaderStage::Fragment, label)?;
    pass_directives
        .iter()
        .map(|pass| {
            let mut remapped = pass.clone();
            remapped.vertex_entry =
                remapped_entry_point_name(&vertex_names, &pass.vertex_entry, label, "vertex")?;
            remapped.fragment_entry = remapped_entry_point_name(
                &fragment_names,
                &pass.fragment_entry,
                label,
                "fragment",
            )?;
            Ok(remapped)
        })
        .collect()
}

/// Converts a composed module to WGSL and applies material-only post-processing.
fn flattened_wgsl_for_job(
    module: &naga::Module,
    stem: &str,
    variant: ShaderVariant,
    source_class: ShaderSourceClass,
) -> Result<String, BuildError> {
    let label = format!("{stem} ({})", variant.label());
    let wgsl = module_to_wgsl(module, &label)?;
    if source_class == ShaderSourceClass::Material {
        rewrite_material_mirror_once_wgsl(&wgsl, &label)
    } else {
        Ok(wgsl)
    }
}

/// Runs source-level validation on one composed shader variant.
fn validate_composed_variant(
    stem: &str,
    pass_directives: &[BuildPassDirective],
    module: &naga::Module,
    variant: ShaderVariant,
) -> Result<(), BuildError> {
    validate_entry_points(
        module,
        &format!("{stem} ({})", variant.label()),
        pass_directives,
    )?;
    validate_pass_interfaces(
        module,
        &format!("{stem} ({})", variant.label()),
        pass_directives,
    )?;
    validate_no_pipeline_state_uniform_fields(module, &format!("{stem} ({})", variant.label()))
}

fn entry_point_name_pairs(
    source_module: &naga::Module,
    output_module: &naga::Module,
    stage: ShaderStage,
    label: &str,
) -> Result<Vec<(String, String)>, BuildError> {
    let source_names = entry_point_names(source_module, stage);
    let output_names = entry_point_names(output_module, stage);
    if source_names.len() != output_names.len() {
        return Err(BuildError::Message(format!(
            "{label}: flattened WGSL changed {stage:?} entry point count from {} to {}",
            source_names.len(),
            output_names.len(),
        )));
    }
    Ok(source_names.into_iter().zip(output_names).collect())
}

fn entry_point_names(module: &naga::Module, stage: ShaderStage) -> Vec<String> {
    module
        .entry_points
        .iter()
        .filter(|entry| entry.stage == stage)
        .map(|entry| entry.name.clone())
        .collect()
}

fn remapped_entry_point_name(
    name_pairs: &[(String, String)],
    requested: &str,
    label: &str,
    stage_name: &str,
) -> Result<String, BuildError> {
    name_pairs
        .iter()
        .find_map(|(source, output)| (source == requested).then(|| output.clone()))
        .ok_or_else(|| {
            BuildError::Message(format!(
                "{label}: requested {stage_name} entry point `{requested}` was not found before WGSL flattening"
            ))
        })
}

/// Compiles one source shader into one or two flattened WGSL targets without writing files.
pub(super) fn compile_shader_job(
    modules: &ShaderModuleSources,
    job: &ShaderJob,
) -> Result<CompiledShader, BuildError> {
    let stem = shader_source_stem(job)?;
    let manifest = shader_source_manifest(&job.source_path)?;
    let ShaderSourceManifest {
        source,
        file_path,
        pass_directives,
        texture_defaults,
        material_defaults,
        wgpu_features,
        default_render_queue,
    } = manifest;
    if job.validation.require_pass_directive && pass_directives.is_empty() {
        return Err(BuildError::Message(format!(
            "{file_path}: material WGSL must declare at least one //#pass directive (e.g. //#pass forward)"
        )));
    }

    let default_module =
        compose_source_variant(modules, &source, &file_path, ShaderVariant::Default)?;
    validate_composed_variant(
        stem,
        &pass_directives,
        &default_module,
        ShaderVariant::Default,
    )?;

    let targets = compile_variant_targets(
        modules,
        &source,
        &file_path,
        stem,
        job,
        &pass_directives,
        &default_module,
    )?;

    Ok(CompiledShader {
        compile_order: job.compile_order,
        source_stem: stem.to_string(),
        source_class: job.source_class,
        pass_directives,
        texture_defaults,
        material_defaults,
        wgpu_features,
        default_render_queue,
        targets,
    })
}

fn shader_source_stem(job: &ShaderJob) -> Result<&str, BuildError> {
    let source_path = &job.source_path;
    source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| BuildError::Message(format!("invalid stem: {}", source_path.display())))
}

fn compile_variant_targets(
    modules: &ShaderModuleSources,
    source: &str,
    file_path: &str,
    stem: &str,
    job: &ShaderJob,
    pass_directives: &[BuildPassDirective],
    default_module: &naga::Module,
) -> Result<Vec<CompiledShaderTarget>, BuildError> {
    let multiview_module =
        compose_source_variant(modules, source, file_path, ShaderVariant::Multiview)?;
    validate_composed_variant(
        stem,
        pass_directives,
        &multiview_module,
        ShaderVariant::Multiview,
    )?;
    let default_wgsl = flattened_wgsl_for_job(
        default_module,
        stem,
        ShaderVariant::Default,
        job.source_class,
    )?;
    let multiview_wgsl = flattened_wgsl_for_job(
        &multiview_module,
        stem,
        ShaderVariant::Multiview,
        job.source_class,
    )?;

    if default_wgsl == multiview_wgsl {
        let pass_directives = remapped_pass_directives_for_output(
            default_module,
            &default_wgsl,
            pass_directives,
            &format!("{stem} ({})", ShaderVariant::Default.label()),
        )?;
        return Ok(vec![CompiledShaderTarget {
            target_stem: stem.to_string(),
            wgsl: default_wgsl,
            pass_directives,
        }]);
    }

    let variants = [
        (ShaderVariant::Default, default_module, default_wgsl),
        (ShaderVariant::Multiview, &multiview_module, multiview_wgsl),
    ];
    let mut targets = Vec::with_capacity(variants.len());
    for (variant, module, wgsl) in variants {
        let target_stem = variant.target_stem(stem);
        if job.validation.validate_view_index {
            validate_view_index_contract(&target_stem, &wgsl, variant)?;
        }
        let pass_directives = remapped_pass_directives_for_output(
            module,
            &wgsl,
            pass_directives,
            &format!("{stem} ({})", variant.label()),
        )?;
        targets.push(CompiledShaderTarget {
            target_stem,
            wgsl,
            pass_directives,
        });
    }
    append_keyword_variant_targets(
        modules,
        source,
        file_path,
        stem,
        job,
        pass_directives,
        targets,
    )
}

/// Packaged shaders declare at most one `//#keyword`. That keyword is a compile-time def crossed
/// with the existing default/multiview targets. Custom materials still compose keyword masks at
/// runtime; this path only prebuilds the one declared keyword so pipeline warmup loads WGSL.
fn append_keyword_variant_targets(
    modules: &ShaderModuleSources,
    source: &str,
    file_path: &str,
    stem: &str,
    job: &ShaderJob,
    pass_directives: &[BuildPassDirective],
    mut targets: Vec<CompiledShaderTarget>,
) -> Result<Vec<CompiledShaderTarget>, BuildError> {
    let keywords = parse_packaged_keywords(source, file_path)?;
    if keywords.is_empty() {
        return Ok(targets);
    }
    for multiview in [false, true] {
        let variant = if multiview {
            ShaderVariant::Multiview
        } else {
            ShaderVariant::Default
        };
        let label = format!("{file_path} ({}; {})", variant.label(), keywords.join("+"));
        let module =
            compose_source_variant_with_keywords(modules, source, &label, variant, &keywords)?;
        validate_composed_variant(stem, pass_directives, &module, variant)?;
        let wgsl = flattened_wgsl_for_job(&module, stem, variant, job.source_class)?;
        let target_stem = keyword_target_stem(stem, &keywords, multiview);
        if job.validation.validate_view_index {
            validate_view_index_contract(&target_stem, &wgsl, variant)?;
        }
        let pass_directives =
            remapped_pass_directives_for_output(&module, &wgsl, pass_directives, &label)?;
        targets.push(CompiledShaderTarget {
            target_stem,
            wgsl,
            pass_directives,
        });
    }
    Ok(targets)
}

/// `//#keyword` names in source order. Packaged shaders take one name; a longer list would be a
/// full mask product and is rejected here.
fn parse_packaged_keywords(source: &str, file: &str) -> Result<Vec<String>, BuildError> {
    let mut keywords = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let line_no = line_idx + 1;
        let Some(rest) = line.trim_start().strip_prefix("//#keyword") else {
            continue;
        };
        if rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let mut tokens = rest.split_whitespace();
        let Some(name) = tokens.next() else {
            return Err(BuildError::Message(format!(
                "{file}:{line_no}: `//#keyword` requires a name"
            )));
        };
        if tokens.next().is_some() {
            return Err(BuildError::Message(format!(
                "{file}:{line_no}: `//#keyword` accepts one name"
            )));
        }
        if !is_packaged_keyword_name(name) {
            return Err(BuildError::Message(format!(
                "{file}:{line_no}: `//#keyword` name `{name}` is not an identifier"
            )));
        }
        if keywords.iter().any(|existing| existing == name) {
            return Err(BuildError::Message(format!(
                "{file}:{line_no}: duplicate `//#keyword` `{name}`"
            )));
        }
        keywords.push(name.to_string());
    }
    if keywords.len() > 1 {
        return Err(BuildError::Message(format!(
            "{file}: packaged shaders accept one `//#keyword` compile-time variant (found {})",
            keywords.len()
        )));
    }
    Ok(keywords)
}

fn is_packaged_keyword_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// `_ALPHAPREMULTIPLY_ON` becomes `alphapremultiply`, crossed with `_default` or `_multiview`.
fn keyword_target_stem(source_stem: &str, keywords: &[String], multiview: bool) -> String {
    let token = keywords
        .iter()
        .map(|keyword| {
            keyword
                .trim_start_matches('_')
                .trim_end_matches("_ON")
                .to_ascii_lowercase()
        })
        .collect::<Vec<_>>()
        .join("_");
    let view = if multiview { "multiview" } else { "default" };
    format!("{source_stem}_{token}_{view}")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn shader_job(source_path: std::path::PathBuf, source_class: ShaderSourceClass) -> ShaderJob {
        ShaderJob {
            compile_order: 0,
            source_class,
            source_path,
            validation: source_class.validation(),
        }
    }

    #[test]
    fn compute_shader_with_multiview_defs_emits_suffixed_targets() -> Result<(), BuildError> {
        let root = tempfile::tempdir()?;
        let source_path = root.path().join("variant_compute.wgsl");
        fs::write(
            &source_path,
            r#"
@group(0) @binding(0)
var<storage, read_write> output: array<u32>;

@compute @workgroup_size(1, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
#ifdef MULTIVIEW
    output[gid.x] = 2u;
#else
    output[gid.x] = 1u;
#endif
}
"#,
        )?;

        let compiled = compile_shader_job(
            &Vec::new(),
            &shader_job(source_path, ShaderSourceClass::Compute),
        )?;
        let stems = compiled
            .targets
            .iter()
            .map(|target| target.target_stem.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            stems,
            ["variant_compute_default", "variant_compute_multiview"]
        );
        Ok(())
    }

    #[test]
    fn packaged_keyword_crosses_the_multiview_variant() -> Result<(), BuildError> {
        let root = tempfile::tempdir()?;
        let source_path = root.path().join("keyword_compute.wgsl");
        fs::write(
            &source_path,
            r#"
//#keyword _ALPHAPREMULTIPLY_ON

@group(0) @binding(0)
var<storage, read_write> output: array<u32>;

@compute @workgroup_size(1, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
#ifdef _ALPHAPREMULTIPLY_ON
    output[gid.x] = 3u;
#else
    output[gid.x] = 1u;
#endif
#ifdef MULTIVIEW
    output[gid.x] = output[gid.x] + 1u;
#endif
}
"#,
        )?;

        let compiled = compile_shader_job(
            &Vec::new(),
            &shader_job(source_path, ShaderSourceClass::Compute),
        )?;
        let stems = compiled
            .targets
            .iter()
            .map(|target| target.target_stem.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            stems,
            [
                "keyword_compute_default",
                "keyword_compute_multiview",
                "keyword_compute_alphapremultiply_default",
                "keyword_compute_alphapremultiply_multiview",
            ]
        );
        let premul = compiled
            .targets
            .iter()
            .find(|target| target.target_stem == "keyword_compute_alphapremultiply_default")
            .expect("premultiply target");
        assert!(premul.wgsl.contains("3u"));
        assert!(!premul.wgsl.contains("output[gid.x] = 1u"));
        Ok(())
    }
}
