//! Runtime custom material shaders installed from RenderideMod.
//!
//! Authoring WGSL uses the same `#import renderide::...` modules and `//#pass` directives as
//! `shaders/materials`. Installation composes that source, reflects the flattened WGSL, and
//! publishes it under a `renderide_custom_<key>` stem so the existing embedded pipeline, bind
//! group, and draw paths compile it with wgpu.

mod compose;
mod passes;

use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::RwLock;

use crate::assets::util::normalize_unity_shader_lookup_key;
use crate::embedded_shaders::{EmbeddedMaterialDefault, EmbeddedMaterialDefaultValue};
use crate::materials::MaterialPassDesc;
use crate::materials::shader_package::reflect_authored_material;

use self::compose::compose_custom_material;
use self::passes::OwnedMaterialPass;

/// Stem prefix for every installed custom material. Host routes store the `_default` stem.
pub(crate) const CUSTOM_STEM_PREFIX: &str = "renderide_custom_";

/// Result of installing one custom material program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstalledCustomShader {
    /// Normalized shader asset key.
    pub key: String,
    /// Base stem before the `_default` / `_multiview` suffix.
    pub base_stem: String,
    /// Stem stored on [`crate::materials::RasterPipelineKind::EmbeddedStem`].
    pub default_stem: String,
    /// `true` when the composed WGSL differed from the previously installed program.
    pub source_changed: bool,
}

/// Result of removing one custom material program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemovedCustomShader {
    /// Normalized shader asset key.
    pub key: String,
    /// Base stem that was removed.
    pub base_stem: String,
}

struct InstalledTarget {
    generation: u64,
    wgsl: Arc<str>,
    passes: Arc<[MaterialPassDesc]>,
    reflection: crate::embedded_shaders::EmbeddedShaderReflection,
    default_render_queue: i32,
    material_defaults: Arc<[EmbeddedMaterialDefault]>,
}

struct InstalledKey {
    base_stem: String,
    default_stem: String,
}

#[derive(Default)]
struct CustomShaderRegistry {
    targets: HashMap<String, InstalledTarget>,
    keys: HashMap<String, InstalledKey>,
    next_generation: u64,
    invalidated_stems: Vec<String>,
}

fn registry() -> &'static RwLock<CustomShaderRegistry> {
    static REGISTRY: std::sync::OnceLock<RwLock<CustomShaderRegistry>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(CustomShaderRegistry::default()))
}

/// Normalizes a Unity shader asset name or custom-shader key the same way route lookup does.
pub(crate) fn normalize_shader_key(name: &str) -> String {
    let trimmed = name.trim();
    let without_shader = trimmed
        .strip_suffix(".shader")
        .or_else(|| trimmed.strip_suffix(".SHADER"))
        .or_else(|| trimmed.strip_suffix(".Shader"))
        .unwrap_or(trimmed);
    let without_wgsl = without_shader
        .strip_suffix(".wgsl")
        .or_else(|| without_shader.strip_suffix(".WGSL"))
        .unwrap_or(without_shader);
    normalize_unity_shader_lookup_key(without_wgsl)
}

/// Installs or replaces the custom program for `key`.
pub(crate) fn install(key: &str, wgsl: &str) -> Result<InstalledCustomShader, String> {
    let key = normalize_shader_key(key);
    if key.is_empty() {
        return Err("custom shader key is empty".to_string());
    }
    let composed = compose_custom_material(wgsl, &format!("custom:{key}"))?;
    let base_stem = format!("{CUSTOM_STEM_PREFIX}{key}");
    let default_stem = format!("{base_stem}_default");
    let multiview_stem = format!("{base_stem}_multiview");
    if let Some(installed) =
        unchanged_install(&key, &base_stem, &default_stem, &multiview_stem, &composed)
    {
        return Ok(installed);
    }

    let default_wgsl = Arc::<str>::from(composed.default_wgsl);
    let multiview_wgsl = Arc::<str>::from(composed.multiview_wgsl);
    let default_passes = owned_passes(composed.default_passes);
    let multiview_passes = owned_passes(composed.multiview_passes);
    let material_defaults = owned_material_defaults(composed.material_defaults);

    // Free the previous generation before reflection. A reflect failure drops the
    // in-progress Arcs and does not put the replaced program back.
    {
        let mut registry = registry().write();
        if registry.targets.remove(&default_stem).is_some() {
            registry.invalidated_stems.push(default_stem.clone());
        }
        if registry.targets.remove(&multiview_stem).is_some() {
            registry.invalidated_stems.push(multiview_stem.clone());
        }
        registry.keys.remove(&key);
    }

    let default_reflection = reflect_authored_material(&default_wgsl, &default_passes)?;
    let multiview_reflection = reflect_authored_material(&multiview_wgsl, &multiview_passes)?;

    let mut registry = registry().write();
    let generation = registry.next_generation.saturating_add(1);
    registry.next_generation = generation;
    registry.targets.insert(
        default_stem.clone(),
        InstalledTarget {
            generation,
            wgsl: default_wgsl,
            passes: default_passes,
            reflection: default_reflection,
            default_render_queue: composed.default_render_queue,
            material_defaults: material_defaults.clone(),
        },
    );
    registry.targets.insert(
        multiview_stem,
        InstalledTarget {
            generation,
            wgsl: multiview_wgsl,
            passes: multiview_passes,
            reflection: multiview_reflection,
            default_render_queue: composed.default_render_queue,
            material_defaults,
        },
    );
    registry.keys.insert(
        key.clone(),
        InstalledKey {
            base_stem: base_stem.clone(),
            default_stem: default_stem.clone(),
        },
    );
    drop(registry);

    logger::info!("custom shader installed key={key} stem={default_stem}");
    Ok(InstalledCustomShader {
        key,
        base_stem,
        default_stem,
        source_changed: true,
    })
}

fn unchanged_install(
    key: &str,
    base_stem: &str,
    default_stem: &str,
    multiview_stem: &str,
    composed: &compose::ComposedCustomShader,
) -> Option<InstalledCustomShader> {
    let unchanged = {
        let registry = registry().read();
        let copied = match (
            registry.targets.get(default_stem),
            registry.targets.get(multiview_stem),
        ) {
            (Some(previous), Some(multiview)) => {
                Some((Arc::clone(&previous.wgsl), Arc::clone(&multiview.wgsl)))
            }
            _ => None,
        };
        drop(registry);
        copied.is_some_and(|(default_wgsl, multiview_wgsl)| {
            default_wgsl.as_ref() == composed.default_wgsl
                && multiview_wgsl.as_ref() == composed.multiview_wgsl
        })
    };
    if !unchanged {
        return None;
    }
    Some(InstalledCustomShader {
        key: key.to_string(),
        base_stem: base_stem.to_string(),
        default_stem: default_stem.to_string(),
        source_changed: false,
    })
}

/// Pipeline to store for a host shader name.
///
/// Installed custom programs replace the packaged stem. A route that still names a removed
/// custom stem falls back to the packaged material for that shader asset.
pub(crate) fn pipeline_for_shader_name(
    name: &str,
    incoming: crate::render_contract::RasterPipelineKind,
) -> crate::render_contract::RasterPipelineKind {
    use crate::render_contract::RasterPipelineKind;

    if let Some(stem) = default_stem_for_asset_name(name) {
        return RasterPipelineKind::EmbeddedStem(stem);
    }
    let RasterPipelineKind::EmbeddedStem(stem) = &incoming else {
        return incoming;
    };
    if !stem.starts_with(CUSTOM_STEM_PREFIX) {
        return incoming;
    }
    let key = normalize_shader_key(name);
    crate::materials::shader_package::default_material_stem_for_asset_key(&key)
        .map(RasterPipelineKind::EmbeddedStem)
        .unwrap_or(RasterPipelineKind::Null)
}

/// Removes the custom program for `key`, if one was installed.
pub(crate) fn remove(key: &str) -> Option<RemovedCustomShader> {
    let key = normalize_shader_key(key);
    let mut registry = registry().write();
    let installed = registry.keys.remove(&key)?;
    let multiview_stem = format!("{}_multiview", installed.base_stem);
    registry.targets.remove(&installed.default_stem);
    registry.targets.remove(&multiview_stem);
    registry
        .invalidated_stems
        .push(installed.default_stem.clone());
    registry.invalidated_stems.push(multiview_stem);
    let base_stem = installed.base_stem;
    drop(registry);
    logger::info!("custom shader removed key={key} stem={base_stem}");
    Some(RemovedCustomShader { key, base_stem })
}

/// Installs or removes one custom payload and returns the shader route for it.
pub(crate) fn resolve_custom_shader_payload(
    payload: Result<renderide_shared::custom_shader_ipc::CustomShaderPayload, String>,
    stem_lookup: crate::assets::shader::route::ShaderRouteStemLookup,
) -> crate::assets::ResolvedShaderUpload {
    use crate::assets::shader::route::{ResolvedShaderUpload, plan_shader_route};
    use crate::render_contract::RasterPipelineKind;

    let payload = match payload {
        Ok(payload) => payload,
        Err(error) => {
            logger::warn!("custom shader payload rejected: {error}");
            return ResolvedShaderUpload {
                shader_asset_name: None,
                shader_variant_bits: None,
                pipeline: RasterPipelineKind::Null,
                retarget_shader_key: None,
                rejected: true,
            };
        }
    };
    if payload.remove {
        remove(&payload.key);
        let mut resolved: ResolvedShaderUpload =
            plan_shader_route(Some(payload.key.clone()), None, stem_lookup).into();
        resolved.retarget_shader_key = Some(payload.key);
        return resolved;
    }
    match install(&payload.key, &payload.wgsl) {
        Ok(installed) => ResolvedShaderUpload {
            shader_asset_name: Some(installed.key.clone()),
            shader_variant_bits: None,
            pipeline: RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem)),
            retarget_shader_key: Some(installed.key),
            rejected: false,
        },
        Err(error) => {
            logger::warn!("custom shader install failed for {}: {error}", payload.key);
            plan_shader_route(Some(payload.key), None, stem_lookup).into()
        }
    }
}

/// Default embedded stem for a host shader asset name when a custom program is installed.
pub(crate) fn default_stem_for_asset_name(name: &str) -> Option<Arc<str>> {
    let key = normalize_shader_key(name);
    let registry = registry().read();
    registry
        .keys
        .get(&key)
        .map(|installed| Arc::from(installed.default_stem.as_str()))
}

/// Source generation for a composed custom stem, or `0` for packaged stems.
pub(crate) fn source_generation(stem: &str) -> u64 {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.generation)
        .unwrap_or(0)
}

pub(crate) fn target_wgsl(stem: &str) -> Option<Arc<str>> {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.wgsl.clone())
}

pub(crate) fn material_passes(stem: &str) -> Option<Arc<[MaterialPassDesc]>> {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.passes.clone())
}

pub(crate) fn material_reflection(
    stem: &str,
) -> Option<crate::embedded_shaders::EmbeddedShaderReflection> {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.reflection)
}

pub(crate) fn material_default_render_queue(stem: &str) -> Option<i32> {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.default_render_queue)
}

pub(crate) fn material_uniform_defaults(stem: &str) -> Option<Arc<[EmbeddedMaterialDefault]>> {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.material_defaults.clone())
}

/// Stems whose reflected `@group(1)` layout and bind groups must be dropped.
pub(crate) fn take_invalidated_stems() -> Vec<String> {
    std::mem::take(&mut registry().write().invalidated_stems)
}

fn owned_passes(passes: Vec<OwnedMaterialPass>) -> Arc<[MaterialPassDesc]> {
    passes
        .into_iter()
        .map(|pass| MaterialPassDesc {
            name: Arc::from(pass.name),
            pass_type: pass.pass_type,
            vertex_entry: Arc::from(pass.vertex_entry),
            fragment_entry: Arc::from(pass.fragment_entry),
            depth_compare: pass.depth_compare,
            depth_compare_domain: pass.depth_compare_domain,
            depth_write: pass.depth_write,
            cull_mode: pass.cull_mode,
            blend: pass.blend,
            write_mask: pass.write_mask,
            depth_bias_slope_scale: pass.depth_bias_slope_scale,
            depth_bias_constant: pass.depth_bias_constant,
            alpha_to_coverage: pass.alpha_to_coverage,
            material_state: pass.material_state,
            render_state_policy: pass.render_state_policy,
        })
        .collect::<Vec<_>>()
        .into()
}

fn owned_material_defaults(
    defaults: Vec<passes::OwnedMaterialDefault>,
) -> Arc<[EmbeddedMaterialDefault]> {
    defaults
        .into_iter()
        .map(|default| EmbeddedMaterialDefault {
            property: Arc::from(default.property),
            value: if default.is_vec4 {
                EmbeddedMaterialDefaultValue::vec4(default.values)
            } else {
                EmbeddedMaterialDefaultValue::float(default.values[0])
            },
        })
        .collect::<Vec<_>>()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::reflect_raster_material_wgsl;

    const EXAMPLE_SOLID: &str = include_str!("../../../../RenderideMod/Shaders/example_solid.wgsl");

    #[test]
    fn example_solid_composes_reflects_and_publishes_a_stem() {
        let installed = install("Example Solid", EXAMPLE_SOLID).expect("install example");
        assert_eq!(installed.key, "example-solid");
        assert_eq!(
            installed.default_stem,
            "renderide_custom_example-solid_default"
        );
        assert!(installed.source_changed);

        let wgsl = target_wgsl(&installed.default_stem).expect("wgsl");
        reflect_raster_material_wgsl(&wgsl).expect("group 0 and material reflection");
        let passes = material_passes(&installed.default_stem).expect("passes");
        assert_eq!(passes.len(), 1);
        assert!(!passes[0].fragment_entry.is_empty());
        assert!(default_stem_for_asset_name("example-solid").is_some());

        let again = install("example-solid", EXAMPLE_SOLID).expect("reinstall");
        assert!(!again.source_changed);

        let removed = remove("example-solid").expect("remove");
        assert_eq!(removed.key, "example-solid");
        assert!(default_stem_for_asset_name("example-solid").is_none());
        assert!(target_wgsl(&installed.default_stem).is_none());
    }

    #[test]
    fn example_solid_compiles_with_wgpu_when_an_adapter_exists() {
        let installed = install("wgpu-example-solid", EXAMPLE_SOLID).expect("install");
        let wgsl = target_wgsl(&installed.default_stem).expect("wgsl");
        reflect_raster_material_wgsl(&wgsl).expect("naga reflection is the compile frontend");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
        let Some(adapter) = adapters.into_iter().next() else {
            logger::info!("custom shader wgpu compile skipped: no adapter");
            return;
        };
        let (device, _queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("custom-shader-test"),
                ..Default::default()
            }))
            .expect("device");
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("custom-shader-example"),
            source: wgpu::ShaderSource::Wgsl(wgsl.as_ref().into()),
        });
        let error = pollster::block_on(error_scope.pop());
        assert!(
            error.is_none(),
            "wgpu rejected the custom shader: {error:?}"
        );
        let _ = remove("wgpu-example-solid");
    }

    #[test]
    fn replace_and_remove_drop_the_previous_program() {
        let installed = install("arc-owned", EXAMPLE_SOLID).expect("install");
        let first = target_wgsl(&installed.default_stem).expect("first wgsl");
        assert!(Arc::strong_count(&first) >= 2);

        let replaced = EXAMPLE_SOLID.replace(
            "return rg::retain_globals_additive(mat._Color);",
            "return rg::retain_globals_additive(mat._Color) * 0.5;",
        );
        install("arc-owned", &replaced).expect("replace");
        let second = target_wgsl(&installed.default_stem).expect("second wgsl");
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(Arc::strong_count(&first), 1);

        remove("arc-owned").expect("remove");
        assert!(target_wgsl(&installed.default_stem).is_none());
        assert_eq!(Arc::strong_count(&second), 1);
    }
}
