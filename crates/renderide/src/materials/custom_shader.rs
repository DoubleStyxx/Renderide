//! Runtime custom material shaders installed from RenderideMod.
//!
//! Authoring WGSL uses the same `#import renderide::...` modules and `//#pass` directives as
//! `shaders/materials`. Installation composes that source, reflects the flattened WGSL, and
//! publishes it under a `renderide_custom_<key>` stem so the existing embedded pipeline, bind
//! group, and draw paths compile it with wgpu.

#[cfg(feature = "bench-api")]
mod bench;
mod bindings;
mod compose;
mod keywords;
mod overrides;
mod passes;

#[cfg(test)]
mod example_shaders;

use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::RwLock;

use crate::assets::util::normalize_unity_shader_lookup_key;
use crate::embedded_shaders::{EmbeddedMaterialDefault, EmbeddedMaterialDefaultValue};
use crate::materials::MaterialPassDesc;
use crate::materials::shader_package::reflect_authored_material;

use self::compose::compose_custom_material;
use self::passes::OwnedMaterialPass;

#[cfg(feature = "bench-api")]
pub use bench::KeywordVariantCacheWorkload;
pub(crate) use keywords::is_toggle_style_property_name;

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
    /// Source generation published for this install.
    pub generation: u64,
    /// Generation replaced by this install, when a previous program was dropped.
    pub replaced_generation: Option<u64>,
}

/// Result of removing one custom material program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemovedCustomShader {
    /// Normalized shader asset key.
    pub key: String,
    /// Base stem that was removed.
    pub base_stem: String,
    /// Source generation that was dropped. Bind caches must evict this generation.
    pub generation: u64,
}

struct InstalledTarget {
    generation: u64,
    wgsl: Arc<str>,
    source: Arc<str>,
    keywords: Arc<[String]>,
    multiview: bool,
    passes: Arc<[MaterialPassDesc]>,
    reflection: crate::embedded_shaders::EmbeddedShaderReflection,
    default_render_queue: i32,
    material_defaults: Arc<[EmbeddedMaterialDefault]>,
    variants: keywords::KeywordVariantCache,
}

struct InstalledKey {
    base_stem: String,
    default_stem: String,
}

#[derive(Default)]
pub(super) struct CustomShaderRegistry {
    targets: HashMap<String, InstalledTarget>,
    keys: HashMap<String, InstalledKey>,
    /// Material asset id -> normalized custom shader key.
    material_overrides: HashMap<i32, String>,
    next_generation: u64,
    invalidated_stems: Vec<(String, u64)>,
}

pub(super) fn registry() -> &'static RwLock<CustomShaderRegistry> {
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
    let label = format!("custom:{key}");
    let parsed_keywords = keywords::parse_keywords(wgsl, &label)?;
    let composed = compose_custom_material(wgsl, &label).map_err(|error| {
        // `array<f32, N>` fails naga uniform validation (stride 4) before reflection.
        // That is a binding rejection, not a packaged-shader fallback.
        if error.contains("address space Uniform") {
            bindings::binding_rejected(error)
        } else {
            error
        }
    })?;
    let base_stem = format!("{CUSTOM_STEM_PREFIX}{key}");
    let default_stem = format!("{base_stem}_default");
    let multiview_stem = format!("{base_stem}_multiview");
    if let Some(installed) = unchanged_install(
        &key,
        &base_stem,
        &default_stem,
        &multiview_stem,
        wgsl,
        &parsed_keywords,
        &composed,
    ) {
        return Ok(installed);
    }

    let default_wgsl = Arc::<str>::from(composed.default_wgsl);
    let multiview_wgsl = Arc::<str>::from(composed.multiview_wgsl);
    // Reflect and reject unsupported @group(1) types before swapping the published program,
    // so a bad replace leaves the previous stem in place.
    validate_composed_bindings(&default_wgsl, &composed.material_defaults)?;
    validate_composed_bindings(&multiview_wgsl, &composed.material_defaults)?;
    let default_passes = owned_passes(composed.default_passes);
    let multiview_passes = owned_passes(composed.multiview_passes);
    let material_defaults = owned_material_defaults(composed.material_defaults);
    let source = Arc::<str>::from(wgsl);
    let keywords = Arc::<[String]>::from(parsed_keywords);

    let replaced_generation = {
        let mut registry = registry().write();
        let replaced_generation = remove_target(&mut registry, &default_stem);
        let _multiview_generation = remove_target(&mut registry, &multiview_stem);
        registry.keys.remove(&key);
        replaced_generation
    };

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
            source: Arc::clone(&source),
            keywords: Arc::clone(&keywords),
            multiview: false,
            passes: default_passes,
            reflection: default_reflection,
            default_render_queue: composed.default_render_queue,
            material_defaults: material_defaults.clone(),
            variants: keywords::KeywordVariantCache::default(),
        },
    );
    registry.targets.insert(
        multiview_stem,
        InstalledTarget {
            generation,
            wgsl: multiview_wgsl,
            source,
            keywords,
            multiview: true,
            passes: multiview_passes,
            reflection: multiview_reflection,
            default_render_queue: composed.default_render_queue,
            material_defaults,
            variants: keywords::KeywordVariantCache::default(),
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
        generation,
        replaced_generation,
    })
}

fn remove_target(registry: &mut CustomShaderRegistry, stem: &str) -> Option<u64> {
    let previous = registry.targets.remove(stem)?;
    registry
        .invalidated_stems
        .push((stem.to_string(), previous.generation));
    Some(previous.generation)
}

fn unchanged_install(
    key: &str,
    base_stem: &str,
    default_stem: &str,
    multiview_stem: &str,
    source: &str,
    keywords: &[String],
    composed: &compose::ComposedCustomShader,
) -> Option<InstalledCustomShader> {
    let copied = {
        let registry = registry().read();
        match (
            registry.targets.get(default_stem),
            registry.targets.get(multiview_stem),
        ) {
            (Some(previous), Some(multiview)) => Some((
                Arc::clone(&previous.wgsl),
                Arc::clone(&multiview.wgsl),
                Arc::clone(&previous.source),
                Arc::clone(&previous.keywords),
                previous.generation,
            )),
            _ => None,
        }
    };
    let (default_wgsl, multiview_wgsl, previous_source, previous_keywords, generation) = copied?;
    if previous_source.as_ref() != source
        || previous_keywords.as_ref() != keywords
        || default_wgsl.as_ref() != composed.default_wgsl
        || multiview_wgsl.as_ref() != composed.multiview_wgsl
    {
        return None;
    }
    Some(InstalledCustomShader {
        key: key.to_string(),
        base_stem: base_stem.to_string(),
        default_stem: default_stem.to_string(),
        source_changed: false,
        generation,
        replaced_generation: None,
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
pub(super) struct RemoveResult {
    /// Program that was installed, when this key had one.
    program: Option<RemovedCustomShader>,
    /// `true` when at least one material override for this key was dropped.
    cleared_overrides: bool,
}

/// Removes the custom program for `key`, if one was installed.
#[cfg(test)]
pub(crate) fn remove(key: &str) -> Option<RemovedCustomShader> {
    remove_result(key).program
}

fn remove_result(key: &str) -> RemoveResult {
    let key = normalize_shader_key(key);
    let mut registry = registry().write();
    let before = registry.material_overrides.len();
    registry
        .material_overrides
        .retain(|_, stored| stored != &key);
    let cleared_overrides = registry.material_overrides.len() != before;
    let Some(installed) = registry.keys.remove(&key) else {
        drop(registry);
        return RemoveResult {
            program: None,
            cleared_overrides,
        };
    };
    let multiview_stem = format!("{}_multiview", installed.base_stem);
    let generation = remove_target(&mut registry, &installed.default_stem).unwrap_or(0);
    let _multiview_generation = remove_target(&mut registry, &multiview_stem);
    let base_stem = installed.base_stem;
    drop(registry);
    logger::info!("custom shader removed key={key} stem={base_stem}");
    RemoveResult {
        program: Some(RemovedCustomShader {
            key,
            base_stem,
            generation,
        }),
        cleared_overrides,
    }
}

/// Drops the per-material override for `material_asset_id`.
///
/// Called when the host unloads that material so a reused asset id does not keep the old key.
pub(crate) fn clear_material_override(material_asset_id: i32) -> bool {
    overrides::clear(material_asset_id)
}

/// Replaces `stock` when `material_asset_id` has an installed custom program assigned.
pub(crate) fn apply_material_shader_override(
    material_asset_id: i32,
    stock: crate::render_contract::RasterPipelineKind,
) -> crate::render_contract::RasterPipelineKind {
    match overrides::stem_for_material(material_asset_id) {
        Some(stem) => crate::render_contract::RasterPipelineKind::EmbeddedStem(stem),
        None => stock,
    }
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
            return rejected_shader_upload();
        }
    };
    use renderide_shared::custom_shader_ipc::CustomShaderOp;
    match payload.op {
        CustomShaderOp::Assign => {
            return apply_material_payload(payload.material_asset_id, |material_asset_id| {
                overrides::assign(material_asset_id, &payload.key)
            });
        }
        CustomShaderOp::Clear => {
            return apply_material_payload(payload.material_asset_id, overrides::clear);
        }
        CustomShaderOp::Remove => {
            let removed = remove_result(&payload.key);
            let _ = removed.program;
            let mut resolved: ResolvedShaderUpload =
                plan_shader_route(Some(payload.key.clone()), None, stem_lookup).into();
            resolved.retarget_shader_key = Some(payload.key);
            resolved.material_override_changed = removed.cleared_overrides;
            return resolved;
        }
        CustomShaderOp::Install => {}
    }
    match install(&payload.key, &payload.wgsl) {
        Ok(installed) => ResolvedShaderUpload {
            shader_asset_name: Some(installed.key.clone()),
            shader_variant_bits: None,
            pipeline: RasterPipelineKind::EmbeddedStem(Arc::from(installed.default_stem)),
            retarget_shader_key: Some(installed.key.clone()),
            material_override_changed: overrides::key_has_material(&installed.key),
            rejected: false,
        },
        Err(error) => {
            if bindings::is_binding_rejection(&error) {
                logger::error!("custom shader install failed for {}: {error}", payload.key);
                return rejected_shader_upload();
            }
            logger::warn!("custom shader install failed for {}: {error}", payload.key);
            plan_shader_route(Some(payload.key), None, stem_lookup).into()
        }
    }
}

fn rejected_shader_upload() -> crate::assets::ResolvedShaderUpload {
    use crate::assets::shader::route::ResolvedShaderUpload;
    use crate::render_contract::RasterPipelineKind;
    ResolvedShaderUpload {
        shader_asset_name: None,
        shader_variant_bits: None,
        pipeline: RasterPipelineKind::Null,
        retarget_shader_key: None,
        material_override_changed: false,
        rejected: true,
    }
}

fn apply_material_payload(
    material_asset_id: Option<i32>,
    apply: impl FnOnce(i32) -> bool,
) -> crate::assets::ResolvedShaderUpload {
    use crate::assets::shader::route::ResolvedShaderUpload;
    use crate::render_contract::RasterPipelineKind;
    let Some(material_asset_id) = material_asset_id else {
        logger::warn!("custom shader material payload is missing a material id");
        return rejected_shader_upload();
    };
    let changed = apply(material_asset_id);
    ResolvedShaderUpload {
        shader_asset_name: None,
        shader_variant_bits: None,
        pipeline: RasterPipelineKind::Null,
        retarget_shader_key: None,
        material_override_changed: changed,
        rejected: false,
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

/// Specialization key for a custom stem's keyword mask, or `None` when the stem has no keywords.
pub(crate) fn keyword_specialization_key(
    stem: &str,
    variant_bits: Option<u32>,
    property_registry: Option<&crate::materials::host_data::PropertyIdRegistry>,
    material_map: crate::materials::PropertyMapRef<'_>,
    property_block_map: crate::materials::PropertyMapRef<'_>,
) -> Option<crate::materials::MaterialShaderSpecializationKey> {
    let mask = keyword_mask(stem, variant_bits, |name| {
        let registry = property_registry?;
        let property_id = registry.intern(name);
        crate::materials::first_float_from_maps(material_map, property_block_map, &[property_id])
    })?;
    Some(crate::materials::MaterialShaderSpecializationKey::from_variant_bits(mask))
}

/// Keyword names in declaration order, or `None` when `stem` has no `//#keyword` directives.
pub(crate) fn keyword_names(stem: &str) -> Option<Arc<[String]>> {
    let registry = registry().read();
    let keywords = registry
        .targets
        .get(stem)
        .and_then(|target| (!target.keywords.is_empty()).then(|| Arc::clone(&target.keywords)));
    drop(registry);
    keywords
}

/// Variant mask for a custom stem.
///
/// Returns `None` when `stem` has no keywords, so packaged specialization stays on pipeline
/// constants. Bit `i` is set when host `variant_bits` has that bit or `float_for_name` for
/// keyword `i` is nonzero.
pub(crate) fn keyword_mask(
    stem: &str,
    variant_bits: Option<u32>,
    mut float_for_name: impl FnMut(&str) -> Option<f32>,
) -> Option<u32> {
    let keywords = keyword_names(stem)?;
    let mut mask = 0u32;
    for (index, name) in keywords.iter().enumerate() {
        let bit = 1u32 << index;
        let from_bits = variant_bits.is_some_and(|bits| bits & bit != 0);
        let from_float = float_for_name(name).is_some_and(|value| value != 0.0);
        if from_bits || from_float {
            mask |= bit;
        }
    }
    Some(mask)
}

/// Composed WGSL for `mask`.
///
/// `Ok(None)` means `stem` is not a keyword-bearing custom shader, so the caller keeps the
/// packaged or mask-zero source. A nonzero mask on a keyword stem composes `#ifdef` defs and
/// stores the result in a per-stem cache of [`keywords::MAX_CUSTOM_KEYWORD_VARIANTS`].
enum VariantProbe {
    Missing,
    Hit(Arc<str>),
    Compose {
        source: Arc<str>,
        keywords: Arc<[String]>,
        multiview: bool,
    },
}

pub(crate) fn variant_wgsl(stem: &str, mask: u32) -> Result<Option<Arc<str>>, String> {
    if mask == 0 || !stem.starts_with(CUSTOM_STEM_PREFIX) {
        return Ok(None);
    }
    let probe = {
        let mut registry = registry().write();
        let probe = match registry.targets.get_mut(stem) {
            None => VariantProbe::Missing,
            Some(target) if target.keywords.is_empty() => VariantProbe::Missing,
            Some(target) => {
                if let Some(cached) = target.variants.get(mask) {
                    VariantProbe::Hit(cached)
                } else {
                    VariantProbe::Compose {
                        source: Arc::clone(&target.source),
                        keywords: Arc::clone(&target.keywords),
                        multiview: target.multiview,
                    }
                }
            }
        };
        drop(registry);
        probe
    };
    let (source, keywords, multiview) = match probe {
        VariantProbe::Hit(wgsl) => return Ok(Some(wgsl)),
        VariantProbe::Missing => return Ok(None),
        VariantProbe::Compose {
            source,
            keywords,
            multiview,
        } => (source, keywords, multiview),
    };
    let wgsl = keywords::compose_variant_wgsl(source.as_ref(), stem, multiview, &keywords, mask)?;
    let mut registry = registry().write();
    let stored = registry.targets.get_mut(stem).map(|target| {
        if let Some(cached) = target.variants.get(mask) {
            cached
        } else {
            target.variants.insert(mask, Arc::clone(&wgsl));
            wgsl
        }
    });
    drop(registry);
    Ok(stored)
}

/// Number of cached keyword variants for `stem`.
#[cfg(test)]
pub(crate) fn keyword_variant_cache_len(stem: &str) -> usize {
    registry()
        .read()
        .targets
        .get(stem)
        .map(|target| target.variants.len())
        .unwrap_or(0)
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
pub(crate) fn take_invalidated_stems() -> Vec<(String, u64)> {
    std::mem::take(&mut registry().write().invalidated_stems)
}

fn validate_composed_bindings(
    wgsl: &str,
    defaults: &[passes::OwnedMaterialDefault],
) -> Result<(), String> {
    let reflected = crate::materials::reflect_raster_material_wgsl(wgsl).map_err(|error| {
        let text = error.to_string();
        if text.contains("unsupported") {
            bindings::binding_rejected(text)
        } else {
            text
        }
    })?;
    bindings::validate_custom_material_bindings(&reflected, defaults)
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
    fn example_rainbow_keyword_gates_the_hue_cycle() {
        let wgsl = include_str!("../../../../RenderideMod/Shaders/example_rainbow.wgsl");
        let installed = install("example-rainbow-src", wgsl).expect("install rainbow");
        let names = keyword_names(&installed.default_stem).expect("keyword");
        assert_eq!(names.as_ref(), ["RAINBOW".to_string()]);
        let defaults = material_uniform_defaults(&installed.default_stem).expect("defaults");
        let speed = defaults
            .iter()
            .find(|default| default.property.as_ref() == "_Speed")
            .expect("_Speed");
        assert_eq!(speed.value.values[0], 0.2);
        let reflected = reflect_raster_material_wgsl(
            target_wgsl(&installed.default_stem)
                .expect("mask 0")
                .as_ref(),
        )
        .expect("reflect");
        let fields = &reflected.material_uniform.expect("uniform").fields;
        assert!(fields.contains_key("_MainTex_ST"));
        assert!(fields.contains_key("_Color"));
        assert!(fields.contains_key("_Speed"));
        let plain = target_wgsl(&installed.default_stem).expect("mask 0");
        assert!(
            !plain.contains("frame_time.x"),
            "RAINBOW off should not sample the hue cycle"
        );
        let rainbow = variant_wgsl(&installed.default_stem, 1)
            .expect("compose")
            .expect("mask 1");
        assert!(
            rainbow.contains("frame_time.x"),
            "RAINBOW on should sample the hue cycle"
        );
        remove("example-rainbow-src").expect("remove");
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

    #[test]
    fn reinstall_bumps_generation_and_remove_reports_the_dropped_generation() {
        let installed = install("lifetime-solid", EXAMPLE_SOLID).expect("install");
        assert!(installed.replaced_generation.is_none());
        assert!(installed.generation >= 1);
        assert_eq!(
            source_generation(&installed.default_stem),
            installed.generation
        );

        let replaced = EXAMPLE_SOLID.replace(
            "return rg::retain_globals_additive(mat._Color);",
            "return rg::retain_globals_additive(mat._Color) * 0.5;",
        );
        let again = install("lifetime-solid", &replaced).expect("reinstall");
        assert!(again.source_changed);
        assert_eq!(again.replaced_generation, Some(installed.generation));
        assert_ne!(again.generation, installed.generation);
        assert_eq!(source_generation(&installed.default_stem), again.generation);

        let same = install("lifetime-solid", &replaced).expect("unchanged");
        assert!(!same.source_changed);
        assert_eq!(same.generation, again.generation);
        assert!(same.replaced_generation.is_none());

        let removed = remove("lifetime-solid").expect("remove");
        assert_eq!(removed.generation, again.generation);
        assert_eq!(source_generation(&installed.default_stem), 0);
        assert!(target_wgsl(&installed.default_stem).is_none());
    }
}
