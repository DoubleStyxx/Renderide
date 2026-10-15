//! Composes custom material WGSL through the same naga-oil modules the build script uses.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use hashbrown::HashMap;
use naga::ShaderStage;
use naga::back::wgsl::WriterFlags;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, NagaModuleDescriptor, ShaderDefValue, ShaderLanguage,
    ShaderType,
};

use super::passes::{OwnedMaterialDefault, OwnedMaterialPass};

/// Flattened default and multiview targets for one custom material.
#[derive(Clone, Debug)]
pub(super) struct ComposedCustomShader {
    /// WGSL composed without the `MULTIVIEW` shader def.
    pub default_wgsl: String,
    /// WGSL composed with the `MULTIVIEW` shader def.
    pub multiview_wgsl: String,
    /// Passes whose entry points match [`Self::default_wgsl`].
    pub default_passes: Vec<OwnedMaterialPass>,
    /// Passes whose entry points match [`Self::multiview_wgsl`].
    pub multiview_passes: Vec<OwnedMaterialPass>,
    /// Unity render queue from `//#render_queue`, or Geometry.
    pub default_render_queue: i32,
    /// `//#mat_default` fallbacks.
    pub material_defaults: Vec<OwnedMaterialDefault>,
}

/// Composes `source` against the shipped `renderide::` shader modules.
pub(super) fn compose_custom_material(
    source: &str,
    label: &str,
) -> Result<ComposedCustomShader, String> {
    let modules = shader_modules()?;
    let passes = super::passes::parse_pass_directives(source, label)?;
    if passes.is_empty() {
        return Err(format!(
            "{label}: material WGSL must declare at least one //#pass directive"
        ));
    }
    let default_render_queue = super::passes::parse_render_queue(source, label)?;
    let material_defaults = super::passes::parse_material_defaults(source, label)?;

    let default_module = compose_variant(modules, source, label, false)?;
    let multiview_module = compose_variant(modules, source, label, true)?;
    let default_wgsl = module_to_wgsl(&default_module, label)?;
    let multiview_wgsl = module_to_wgsl(&multiview_module, &format!("{label} multiview"))?;
    if default_wgsl != multiview_wgsl {
        if default_wgsl.contains("@builtin(view_index)") {
            return Err(format!(
                "{label}: the non-multiview compose contains @builtin(view_index); wrap it in #ifdef MULTIVIEW"
            ));
        }
        if !multiview_wgsl.contains("@builtin(view_index)") {
            return Err(format!(
                "{label}: the multiview compose is missing @builtin(view_index)"
            ));
        }
    }
    let default_passes = remap_passes(&default_module, &default_wgsl, &passes, label)?;
    let multiview_passes = remap_passes(
        &multiview_module,
        &multiview_wgsl,
        &passes,
        &format!("{label} multiview"),
    )?;
    Ok(ComposedCustomShader {
        default_wgsl,
        multiview_wgsl,
        default_passes,
        multiview_passes,
        default_render_queue,
        material_defaults,
    })
}

fn compose_variant(
    modules: &[(String, String)],
    source: &str,
    label: &str,
    multiview: bool,
) -> Result<naga::Module, String> {
    let mut composer = Composer::default().with_capabilities(Capabilities::all());
    for (file_path, module_source) in modules {
        if let Err(err) = composer.add_composable_module(ComposableModuleDescriptor {
            source: module_source.as_str(),
            file_path: file_path.as_str(),
            language: ShaderLanguage::Wgsl,
            ..Default::default()
        }) {
            return Err(format!(
                "custom shader module {file_path}: {}",
                err.emit_to_string(&composer)
            ));
        }
    }
    let mut shader_defs = std::collections::HashMap::new();
    if multiview {
        shader_defs.insert("MULTIVIEW".to_string(), ShaderDefValue::Bool(true));
    }
    match composer.make_naga_module(NagaModuleDescriptor {
        source,
        file_path: label,
        shader_type: ShaderType::Wgsl,
        shader_defs,
        ..Default::default()
    }) {
        Ok(module) => Ok(module),
        Err(err) => Err(format!(
            "compose {label}: {}",
            err.emit_to_string(&composer)
        )),
    }
}

fn module_to_wgsl(module: &naga::Module, label: &str) -> Result<String, String> {
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = validator
        .validate(module)
        .map_err(|err| format!("validate {label}: {err}"))?;
    naga::back::wgsl::write_string(module, &info, WriterFlags::EXPLICIT_TYPES)
        .map_err(|err| format!("wgsl out {label}: {err}"))
}

fn remap_passes(
    source_module: &naga::Module,
    output_wgsl: &str,
    passes: &[OwnedMaterialPass],
    label: &str,
) -> Result<Vec<OwnedMaterialPass>, String> {
    let output_module = naga::front::wgsl::parse_str(output_wgsl).map_err(|err| {
        format!(
            "parse flattened {label}: {}",
            err.emit_to_string(output_wgsl)
        )
    })?;
    let vertex_names =
        entry_point_name_pairs(source_module, &output_module, ShaderStage::Vertex, label)?;
    let fragment_names =
        entry_point_name_pairs(source_module, &output_module, ShaderStage::Fragment, label)?;
    passes
        .iter()
        .map(|pass| {
            let mut remapped = pass.clone();
            remapped.vertex_entry =
                remapped_entry(&vertex_names, &pass.vertex_entry, label, "vertex")?;
            remapped.fragment_entry =
                remapped_entry(&fragment_names, &pass.fragment_entry, label, "fragment")?;
            Ok(remapped)
        })
        .collect()
}

fn entry_point_name_pairs(
    source_module: &naga::Module,
    output_module: &naga::Module,
    stage: ShaderStage,
    label: &str,
) -> Result<Vec<(String, String)>, String> {
    let source_names = entry_point_names(source_module, stage);
    let output_names = entry_point_names(output_module, stage);
    if source_names.len() != output_names.len() {
        return Err(format!(
            "{label}: flattened WGSL changed {stage:?} entry point count from {} to {}",
            source_names.len(),
            output_names.len()
        ));
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

fn remapped_entry(
    name_pairs: &[(String, String)],
    requested: &str,
    label: &str,
    stage_name: &str,
) -> Result<String, String> {
    name_pairs
        .iter()
        .find_map(|(source, output)| (source == requested).then(|| output.clone()))
        .ok_or_else(|| {
            format!(
                "{label}: {stage_name} entry `{requested}` was not found before WGSL flattening"
            )
        })
}

fn shader_modules() -> Result<&'static [(String, String)], String> {
    static MODULES: OnceLock<Result<Vec<(String, String)>, String>> = OnceLock::new();
    match MODULES.get_or_init(load_shader_modules) {
        Ok(modules) => Ok(modules.as_slice()),
        Err(err) => Err(err.clone()),
    }
}

fn load_shader_modules() -> Result<Vec<(String, String)>, String> {
    let mut searched = Vec::new();
    if let Some(dir) = crate::materials::shader_package::default_package_dir() {
        let modules = dir.join("modules");
        searched.push(modules.clone());
        if modules.is_dir() {
            return read_module_tree(&modules);
        }
    }
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let modules = PathBuf::from(manifest).join("shaders/modules");
        searched.push(modules.clone());
        if modules.is_dir() {
            return read_module_tree(&modules);
        }
    }
    Err(format!(
        "custom shader modules were not found; searched {searched:?}"
    ))
}

fn read_module_tree(modules_dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut paths = Vec::new();
    collect_wgsl_paths(modules_dir, &mut paths)?;
    paths.sort();
    let mut modules = Vec::with_capacity(paths.len());
    for path in paths {
        let source = std::fs::read_to_string(&path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        let relative = path.strip_prefix(modules_dir).map_err(|err| {
            format!(
                "module {} is outside {}: {err}",
                path.display(),
                modules_dir.display()
            )
        })?;
        let file_path = format!(
            "shaders/modules/{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        modules.push((file_path, source));
    }
    if modules.is_empty() {
        return Err(format!("no WGSL modules under {}", modules_dir.display()));
    }
    topo_sort_modules(&modules)
}

fn collect_wgsl_paths(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|err| format!("read {}: {err}", dir.display()))? {
        let entry = entry.map_err(|err| format!("read {} entry: {err}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("stat {}: {err}", path.display()))?;
        if file_type.is_dir() {
            collect_wgsl_paths(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "wgsl") {
            out.push(path);
        }
    }
    Ok(())
}

fn topo_sort_modules(modules: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut path_to_idx: HashMap<String, usize> = HashMap::default();
    let mut imports_per_module = Vec::with_capacity(modules.len());
    for (index, (file_path, source)) in modules.iter().enumerate() {
        let define = parse_define_import_path(source)
            .ok_or_else(|| format!("module {file_path} has no `#define_import_path` directive"))?;
        if let Some(prev) = path_to_idx.insert(define.clone(), index) {
            return Err(format!(
                "duplicate `#define_import_path {define}` in {file_path} and {}",
                modules[prev].0
            ));
        }
        imports_per_module.push(parse_import_paths(source));
    }

    let mut in_degree = vec![0usize; modules.len()];
    let mut children_of = vec![Vec::new(); modules.len()];
    for (index, imports) in imports_per_module.iter().enumerate() {
        for import_path in imports {
            if let Some(&dependency) = path_to_idx.get(import_path) {
                if index == dependency {
                    continue;
                }
                children_of[dependency].push(index);
                in_degree[index] += 1;
            }
        }
    }

    let mut ready: Vec<usize> = (0..modules.len())
        .filter(|&index| in_degree[index] == 0)
        .collect();
    let mut sorted = Vec::with_capacity(modules.len());
    while let Some(index) = ready.first().copied() {
        ready.remove(0);
        sorted.push(index);
        for &child in &children_of[index] {
            in_degree[child] -= 1;
            if in_degree[child] == 0 {
                let pos = ready
                    .binary_search_by(|&other| modules[other].0.cmp(&modules[child].0))
                    .unwrap_or_else(|err| err);
                ready.insert(pos, child);
            }
        }
    }
    if sorted.len() != modules.len() {
        return Err("shader module import graph has a cycle".to_string());
    }
    Ok(sorted
        .into_iter()
        .map(|index| modules[index].clone())
        .collect())
}

fn parse_define_import_path(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("#define_import_path") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn parse_import_paths(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("#import") else {
            continue;
        };
        let Some(path) = rest.trim().split_whitespace().next() else {
            continue;
        };
        let path = path.trim_end_matches('{');
        let path = path.split("::{").next().unwrap_or(path);
        if !path.is_empty() {
            out.push(path.to_string());
        }
    }
    out
}
