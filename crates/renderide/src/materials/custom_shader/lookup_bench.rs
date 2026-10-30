//! CPU workload for the per-frame custom-stem registry reads.
//!
//! Setup installs one program. Timed samples then call the same reads the frame path uses:
//! `source_generation`, `target_wgsl`, `material_passes`, `material_reflection`,
//! `material_default_render_queue`, and `material_uniform_defaults`. Each call takes one
//! `parking_lot::RwLock` read of the process registry. `target_wgsl`, `material_passes`, and
//! `material_uniform_defaults` clone the `Arc` already stored on the stem. They do not allocate
//! a new string, pass list, or default list. The other three return `Copy` values.

use std::sync::Arc;

const SHADER_KEY: &str = "bench-stem-lookup";
const SHADER_WGSL: &str = include_str!("../../../../../RenderideMod/Shaders/example_solid.wgsl");

/// Published custom stem whose per-frame registry reads are timed.
pub struct CustomStemLookupWorkload {
    stem: Option<Arc<str>>,
    setup_error: Option<String>,
}

impl CustomStemLookupWorkload {
    /// Installs the bench shader once. Later calls with the same source do not republish it.
    #[must_use]
    pub fn standard() -> Self {
        match super::install(SHADER_KEY, SHADER_WGSL) {
            Ok(installed) => Self {
                stem: Some(Arc::from(installed.default_stem)),
                setup_error: None,
            },
            Err(error) => Self {
                stem: None,
                setup_error: Some(error),
            },
        }
    }

    /// Why [`Self::standard`] did not publish a stem, when setup failed.
    #[must_use]
    pub fn setup_error(&self) -> Option<&str> {
        self.setup_error.as_deref()
    }

    /// Folds one sample of the per-frame custom-stem reads into a checksum.
    #[must_use]
    pub fn frame_reads(&self) -> u64 {
        let Some(stem) = self.stem.as_deref() else {
            return 0;
        };
        let generation = super::source_generation(stem);
        let wgsl_len = super::target_wgsl(stem).map_or(0, |wgsl| wgsl.len());
        let pass_count = super::material_passes(stem).map_or(0, |passes| passes.len());
        let queue = super::material_default_render_queue(stem).unwrap_or(0);
        let reflection = super::material_reflection(stem).map_or(0, reflection_bits);
        let defaults = super::material_uniform_defaults(stem).map_or(0, |values| values.len());
        generation
            .wrapping_add(wgsl_len as u64)
            .wrapping_add(pass_count as u64)
            .wrapping_add(u64::from(queue.cast_unsigned()))
            .wrapping_add(reflection)
            .wrapping_add(defaults as u64)
    }
}

fn reflection_bits(reflection: crate::embedded_shaders::EmbeddedShaderReflection) -> u64 {
    u64::from(reflection.supports_generic_depth_prepass)
        | u64::from(reflection.uses_renderide_variant_bits) << 1
        | u64::from(reflection.vertex_stream_mask.uv0) << 2
        | u64::from(reflection.snapshot_requirements.uses_scene_color) << 3
}
