//! Pure layering machinery for the config loader: no file I/O, no migrations.
//!
//! The pipeline is an explicit ordered [`Vec<ConfigLayer>`] so the precedence chain
//! (defaults -> TOML -> `RENDERIDE_*` env -> post-extract overrides) is visible in one place.
//! Pre-extract layers feed the figment merge; post-extract layers run as mutators on the
//! extracted [`RendererSettings`].

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};

use crate::config::types::RendererSettings;

/// One step in the [`LoadPipeline`].
///
/// The pre-extract variants ([`Self::Defaults`], [`Self::Toml`], [`Self::EnvPrefixed`]) feed the
/// figment merge; [`Self::PostExtract`] runs as a mutator on the extracted
/// [`RendererSettings`] after extraction. Layers are applied in the order they appear in the
/// pipeline.
pub enum ConfigLayer {
    /// Insert struct defaults via [`Serialized::defaults`].
    Defaults,
    /// Merge an in-memory TOML string. Use this when the resolver located a file on disk and
    /// loaded its contents.
    Toml(String),
    /// Merge `RENDERIDE_*`-style environment variables. `prefix` is the env-var prefix
    /// (typically `"RENDERIDE_"`), `separator` is the nested-key separator
    /// (typically `"__"`).
    EnvPrefixed {
        /// Environment variable prefix (e.g. `"RENDERIDE_"`).
        prefix: &'static str,
        /// Nested-key separator (e.g. `"__"` to map `RENDERIDE_DEBUG__GPU_VALIDATION_LAYERS`
        /// to `debug.gpu_validation_layers`).
        separator: &'static str,
    },
    /// Post-extract mutator. Runs after [`figment::Figment::extract`] succeeds; useful for
    /// special-case env overrides that don't fit the structured `RENDERIDE_*` namespace
    /// (currently only `RENDERIDE_GPU_VALIDATION`).
    PostExtract(fn(&mut RendererSettings)),
}

/// An ordered chain of [`ConfigLayer`] entries. Construct with [`LoadPipeline::new`] then push
/// layers, or build the canonical chain via [`canonical_layers`] /
/// [`super::load_renderer_settings`].
#[derive(Default)]
pub struct LoadPipeline {
    layers: Vec<ConfigLayer>,
}

impl LoadPipeline {
    /// Empty pipeline (no defaults inserted yet -- the canonical chain always starts with
    /// [`ConfigLayer::Defaults`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a layer to the pipeline.
    pub fn push(&mut self, layer: ConfigLayer) -> &mut Self {
        self.layers.push(layer);
        self
    }

    /// Runs the pipeline: builds the figment from pre-extract layers, extracts
    /// [`RendererSettings`], then runs post-extract mutators in order.
    pub fn extract(self) -> Result<RendererSettings, Box<figment::Error>> {
        let mut figment = Figment::new();
        let mut mutators: Vec<fn(&mut RendererSettings)> = Vec::new();
        for layer in self.layers {
            match layer {
                ConfigLayer::Defaults => {
                    figment = figment.merge(Serialized::defaults(RendererSettings::default()));
                }
                ConfigLayer::Toml(content) => {
                    figment = figment.merge(Toml::string(&content));
                }
                ConfigLayer::EnvPrefixed { prefix, separator } => {
                    figment = figment.merge(Env::prefixed(prefix).split(separator));
                }
                ConfigLayer::PostExtract(f) => {
                    mutators.push(f);
                }
            }
        }
        let mut settings = figment.extract::<RendererSettings>().map_err(Box::new)?;
        for f in mutators {
            f(&mut settings);
        }
        Ok(settings)
    }
}

/// Builds the canonical `RENDERIDE_*` env layering with post-extract mutators, optionally
/// including a TOML layer when `toml_content` is provided.
pub fn canonical_layers(toml_content: Option<String>) -> Vec<ConfigLayer> {
    let mut v = Vec::with_capacity(5);
    v.push(ConfigLayer::Defaults);
    if let Some(content) = toml_content {
        v.push(ConfigLayer::Toml(content));
    }
    v.push(ConfigLayer::EnvPrefixed {
        prefix: "RENDERIDE_",
        separator: "__",
    });
    v.push(ConfigLayer::PostExtract(apply_renderide_gpu_validation_env));
    v.push(ConfigLayer::PostExtract(apply_current_config_version));
    v
}

/// Overrides [`crate::config::DebugSettings::gpu_validation_layers`] when
/// `RENDERIDE_GPU_VALIDATION` is set.
///
/// Truthy values (`1`, `true`, `yes`) force validation on; falsey (`0`, `false`, `no`) force
/// off. If unset, the value from config or defaults is unchanged. Wired into the canonical
/// pipeline as a [`ConfigLayer::PostExtract`] entry so the precedence rule lives next to the
/// other layers.
pub fn apply_renderide_gpu_validation_env(settings: &mut RendererSettings) {
    match std::env::var("RENDERIDE_GPU_VALIDATION").as_deref() {
        Ok("1" | "true" | "yes") => settings.debug.gpu_validation_layers = true,
        Ok("0" | "false" | "no") => settings.debug.gpu_validation_layers = false,
        _ => {}
    }
}

/// Pins runtime settings to the config schema version emitted by this renderer build.
pub fn apply_current_config_version(settings: &mut RendererSettings) {
    RendererSettings::CURRENT_CONFIG_VERSION.clone_into(&mut settings.config_version);
}

/// Runs the canonical pipeline with optional TOML content.
pub(super) fn run_pipeline(
    toml_content: Option<String>,
) -> Result<RendererSettings, Box<figment::Error>> {
    let mut pipeline = LoadPipeline::new();
    for layer in canonical_layers(toml_content) {
        pipeline.push(layer);
    }
    pipeline.extract()
}
