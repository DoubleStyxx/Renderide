//! Opt-in #216 lighting-term scenes.
//!
//! They are not in [`super::cases::registry`] and not in the #216 quarantine, so a
//! run reports a real pass or fail. Select them with `--group warp_216_debug` or
//! with `--case <name>` on `run` or `check-suite`.
//!
//! ```text
//! renderide-test run --group warp_216_debug
//! renderide-test run --case warp216_directional_diffuse_g
//! renderide-test run --bless --force --case warp216_constant
//! ```
//!
//! Each case writes `actual.png` plus `actual.scene_color.rgba16f` and
//! `actual.scene_color.json` (pre-sRGB `Rgba16Float`, 2×2 quadrant stats).
//! The default suite does not spawn these cases, so Windows CI time is unchanged.
//! One WARP capture can wait 300s, and the group has one case per term, channel,
//! and light rig, so running the whole group on WARP does not fit a 20-minute step.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::cases::{CaseTemplate, IntegrationCase, default_goldens_dir};
use super::tolerance::{Combine, Tolerance};

/// CLI group name for [`cases`].
pub(crate) const GROUP: &str = "warp_216_debug";

/// Capture width and height. Even, so the 2×2 float grid splits in half.
pub(crate) const RESOLUTION: u32 = 256;

/// Which lights the sphere sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightRig {
    /// One directional light. Still binned through the clustered loop.
    Directional,
    /// One point light and no directional. Empty clusters leave this black.
    Point,
    /// Positive directional plus the negative directional from `negative_light_physically_based`.
    ///
    /// The renderer clamp is on, so the negative intensity does not reach the shader.
    NegativePair,
    /// Same lights as [`Self::NegativePair`], with the physically based clamp off.
    ///
    /// Intensity `-1.45` is uploaded. That is the #216 parity setting.
    NegativeParity,
    /// That same positive directional, without the negative light.
    PositiveOnly,
    /// No lights. Used by the constant-colour control.
    None,
}

/// Lighting term written by the debug shader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightingTerm {
    /// `shade_metallic_clustered` sum. The control.
    Composed,
    /// `fd * radiance` from `direct_metallic_term_parts` (N·L diffuse).
    Diffuse,
    /// `specular_brdf * radiance` from the same split.
    Specular,
    /// `indirect_diffuse_metallic`.
    Ambient,
    /// `indirect_specular_contribution`.
    Indirect,
    /// Schlick Fresnel `F`, summed over the clustered lights.
    Fresnel,
    /// Emission. The debug material writes zero, so this term is a zero control.
    Emission,
    /// Shader constant `(0.25, 0.5, 0.75)`, no lighting.
    Constant,
}

/// Where an isolated term's signal is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputChannel {
    /// Full `vec3` (composed and constant).
    Raw,
    /// Signal in R. G and B are 0 before the frame-binding epsilon.
    R,
    /// Signal in G. R and B are 0 before the frame-binding epsilon.
    G,
    /// Signal in B. R and G are 0 before the frame-binding epsilon.
    B,
    /// The same signal replicated into R, G, and B.
    Grey,
}

/// One debug capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    /// Light rig.
    pub rig: LightRig,
    /// Term written by the shader.
    pub term: LightingTerm,
    /// Channel encoding.
    pub channel: OutputChannel,
}

impl LightingTerm {
    /// Uniform `_TermId`. Matches `DEBUG_TERM_*` in the debug shader and lighting module.
    pub(crate) const fn id(self) -> f32 {
        match self {
            Self::Composed => 0.0,
            Self::Diffuse => 1.0,
            Self::Specular => 2.0,
            Self::Ambient => 3.0,
            Self::Indirect => 4.0,
            Self::Fresnel => 5.0,
            Self::Emission => 6.0,
            Self::Constant => 7.0,
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::Composed => "composed",
            Self::Diffuse => "diffuse",
            Self::Specular => "specular",
            Self::Ambient => "ambient",
            Self::Indirect => "indirect",
            Self::Fresnel => "fresnel",
            Self::Emission => "emission",
            Self::Constant => "constant",
        }
    }
}

impl OutputChannel {
    /// Uniform `_Channel`. `0` is R, which composed and constant ignore.
    pub(crate) const fn id(self) -> f32 {
        match self {
            Self::Raw | Self::R => 0.0,
            Self::G => 1.0,
            Self::B => 2.0,
            Self::Grey => 3.0,
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::Raw | Self::R => "r",
            Self::G => "g",
            Self::B => "b",
            Self::Grey => "grey",
        }
    }

    /// Channels that should carry the term signal. The others are authored as 0.
    pub(crate) const fn carries_signal(self) -> [bool; 3] {
        match self {
            Self::R => [true, false, false],
            Self::G => [false, true, false],
            Self::B => [false, false, true],
            Self::Grey | Self::Raw => [true, true, true],
        }
    }
}

impl LightRig {
    const fn slug(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Point => "point",
            Self::NegativePair => "negative",
            Self::NegativeParity => "negparity",
            Self::PositiveOnly => "positive",
            Self::None => "none",
        }
    }
}

impl Spec {
    /// `_TermId` for this capture.
    pub(crate) const fn term_id(self) -> f32 {
        self.term.id()
    }

    /// `_Channel` for this capture.
    pub(crate) const fn channel_id(self) -> f32 {
        self.channel.id()
    }

    /// Whether the renderer clamps radiance before the shader.
    ///
    /// The parity rig leaves the clamp off so intensity `-1.45` reaches the GPU.
    pub(crate) const fn physically_based(self) -> bool {
        !matches!(self.rig, LightRig::NegativeParity)
    }

    /// Stable case name.
    pub(crate) fn case_name(self) -> String {
        if self.term == LightingTerm::Constant {
            return "warp216_constant".to_string();
        }
        if self.term == LightingTerm::Composed {
            return format!("warp216_{}_composed", self.rig.slug());
        }
        format!(
            "warp216_{}_{}_{}",
            self.rig.slug(),
            self.term.slug(),
            self.channel.slug()
        )
    }
}

const RIGS: [LightRig; 5] = [
    LightRig::Directional,
    LightRig::Point,
    LightRig::NegativePair,
    LightRig::PositiveOnly,
    LightRig::NegativeParity,
];

const ISOLATED: [LightingTerm; 6] = [
    LightingTerm::Diffuse,
    LightingTerm::Specular,
    LightingTerm::Ambient,
    LightingTerm::Indirect,
    LightingTerm::Fresnel,
    LightingTerm::Emission,
];

const CHANNELS: [OutputChannel; 4] = [
    OutputChannel::R,
    OutputChannel::G,
    OutputChannel::B,
    OutputChannel::Grey,
];

/// Every opt-in capture, in rig order, composed first, then each isolated term.
pub(crate) fn specs() -> Vec<Spec> {
    let mut specs = Vec::new();
    for rig in RIGS {
        specs.push(Spec {
            rig,
            term: LightingTerm::Composed,
            channel: OutputChannel::Raw,
        });
        for term in ISOLATED {
            for channel in CHANNELS {
                specs.push(Spec { rig, term, channel });
            }
        }
    }
    specs.push(Spec {
        rig: LightRig::None,
        term: LightingTerm::Constant,
        channel: OutputChannel::Raw,
    });
    specs
}

/// Integration cases for [`GROUP`]. Not part of the default registry.
pub(crate) fn cases() -> Vec<IntegrationCase> {
    specs().into_iter().map(integration_case).collect()
}

/// One debug case by name.
pub(crate) fn lookup(name: &str) -> Option<IntegrationCase> {
    cases().into_iter().find(|case| case.name == name)
}

/// Scene spec for a debug case name.
#[cfg(test)]
pub(crate) fn spec_for(name: &str) -> Option<Spec> {
    specs().into_iter().find(|spec| spec.case_name() == name)
}

fn integration_case(spec: Spec) -> IntegrationCase {
    let name = spec.case_name();
    IntegrationCase {
        description: description(spec),
        golden_path: default_goldens_dir().join(format!("{name}.png")),
        resolution: (RESOLUTION, RESOLUTION),
        tolerance: debug_tolerance(),
        template: CaseTemplate::Warp216Debug { spec },
        name,
    }
}

fn description(spec: Spec) -> String {
    let carries = spec.channel.carries_signal();
    let signal = match (spec.channel, carries) {
        (OutputChannel::Raw, _) => "RGB carries the authored colour".to_string(),
        (OutputChannel::Grey, _) => {
            "the same signal is in R, G, and B. A channel that is 0 while another is not was dropped."
                .to_string()
        }
        (_, [true, false, false]) => "signal in R; G and B authored 0".to_string(),
        (_, [false, true, false]) => "signal in G; R and B authored 0".to_string(),
        (_, [false, false, true]) => "signal in B; R and G authored 0".to_string(),
        _ => "signal follows the term selector".to_string(),
    };
    let clamp = if spec.physically_based() {
        "Physically based clamp is on."
    } else {
        "Physically based clamp is off, so intensity -1.45 reaches the shader."
    };
    format!(
        "WARP #216 term probe ({} / {}). {signal}. Float readback is actual.scene_color.json. {} {clamp}",
        spec.rig.slug(),
        spec.term.slug(),
        term_source(spec.term)
    )
}

fn term_source(term: LightingTerm) -> &'static str {
    match term {
        LightingTerm::Composed => {
            "Composed calls shade_metallic_clustered in shaders/modules/pbs/lighting.wgsl."
        }
        LightingTerm::Diffuse => {
            "Diffuse is fd * radiance in direct_metallic_term_parts, shaders/modules/pbs/brdf.wgsl."
        }
        LightingTerm::Specular => {
            "Specular is specular_brdf * radiance in direct_metallic_term_parts, shaders/modules/pbs/brdf.wgsl."
        }
        LightingTerm::Ambient => {
            "Ambient is indirect_diffuse_metallic, shaders/modules/pbs/brdf.wgsl, called from shade_metallic_term_debug."
        }
        LightingTerm::Indirect => {
            "Indirect is indirect_specular_contribution in shaders/modules/pbs/lighting.wgsl."
        }
        LightingTerm::Fresnel => {
            "Fresnel is the Schlick F on DirectSpecularEval, summed in shade_metallic_term_debug."
        }
        LightingTerm::Emission => {
            "Emission is the select() of surface emission in shade_metallic_term_debug. This material's emission is 0."
        }
        LightingTerm::Constant => {
            "Constant writes (0.25, 0.5, 0.75) in warp_216_term_debug.wgsl. The frame-binding epsilon is about 1e-10."
        }
    }
}

fn debug_tolerance() -> Tolerance {
    Tolerance {
        ssim_min: Some(0.985),
        max_abs_diff: Some(8),
        max_failing_pixel_fraction: Some(0.03),
        min_luma_range: Some(12),
        min_unique_colors: Some(6),
        min_non_background_pixel_fraction: Some(0.04),
        region_floors: Vec::new(),
        region_colors: Vec::new(),
        combine: Combine::And,
    }
}

/// JSON and raw paths the renderer writes beside `actual.png`.
pub(crate) fn scene_color_json_path(png: &std::path::Path) -> PathBuf {
    sidecar(png, "scene_color.json")
}

/// Raw `Rgba16Float` bytes beside `actual.png`.
pub(crate) fn scene_color_raw_path(png: &std::path::Path) -> PathBuf {
    sidecar(png, "scene_color.rgba16f")
}

fn sidecar(png: &std::path::Path, suffix: &str) -> PathBuf {
    let stem = png
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".to_string());
    png.with_file_name(format!("{stem}.{suffix}"))
}

/// `Ok(())` when this case is not a debug capture.
///
/// A debug capture must have a 2×2 `rgba16float` stats file beside the PNG.
pub(crate) fn scene_color_sidecar_failure(
    case: &IntegrationCase,
    actual_png: &std::path::Path,
) -> Result<(), String> {
    if !matches!(case.template, CaseTemplate::Warp216Debug { .. }) {
        return Ok(());
    }
    let json_path = scene_color_json_path(actual_png);
    let raw_path = scene_color_raw_path(actual_png);
    if !raw_path.is_file() {
        return Err(format!(
            "pre-sRGB scene color missing at {}",
            raw_path.display()
        ));
    }
    let text = std::fs::read_to_string(&json_path).map_err(|err| {
        format!(
            "pre-sRGB scene color stats missing at {}: {err}",
            json_path.display()
        )
    })?;
    validate_scene_color_json(&text)?;
    if let CaseTemplate::Warp216Debug { spec } = case.template
        && requires_negative_scene_color(spec)
    {
        require_negative_scene_color(&case.name, &text)?;
    }
    Ok(())
}

/// Composed, diffuse, and specular on the parity rig multiply by signed radiance.
///
/// Fresnel is Schlick `F`, which is not multiplied by the light. Ambient and
/// indirect do not use the direct light. Emission is authored as 0. On lavapipe
/// those four stay non-negative, so requiring a negative sample there would
/// fail a correct shader.
pub(crate) fn requires_negative_scene_color(spec: Spec) -> bool {
    spec.rig == LightRig::NegativeParity
        && matches!(
            spec.term,
            LightingTerm::Composed | LightingTerm::Diffuse | LightingTerm::Specular
        )
}

/// `Err` when every pre-sRGB `negative` count is 0.
///
/// The string is the suite failure. A missing or non-integer count is the same
/// kind of failure. This does not panic.
pub(crate) fn require_negative_scene_color(case_name: &str, text: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|err| format!("{case_name}: pre-sRGB scene color stats are not JSON: {err}"))?;
    let regions = value
        .get("regions")
        .and_then(|region| region.as_array())
        .ok_or_else(|| format!("{case_name}: pre-sRGB scene color stats have no regions"))?;
    let mut total = 0u64;
    for region in regions {
        let name = region
            .get("name")
            .and_then(|name| name.as_str())
            .unwrap_or("region");
        let Some(counts) = region.get("negative").and_then(|counts| counts.as_array()) else {
            return Err(format!(
                "{case_name}: pre-sRGB region {name} is missing negative counts"
            ));
        };
        for count in counts {
            let Some(count) = count.as_u64() else {
                return Err(format!(
                    "{case_name}: pre-sRGB region {name} has a negative count that is not an integer"
                ));
            };
            total = total.saturating_add(count);
        }
    }
    if total == 0 {
        return Err(format!(
            "{case_name}: pre-sRGB scene color has no negative samples; signed radiance did not reach the shader"
        ));
    }
    Ok(())
}

/// Checks the quadrant JSON written by the renderer.
pub(crate) fn validate_scene_color_json(text: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    if value.get("format").and_then(|v| v.as_str()) != Some("rgba16float") {
        return Err("scene color stats format is not rgba16float".to_string());
    }
    let regions = value
        .get("regions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "scene color stats have no regions".to_string())?;
    if regions.len() != 4 {
        return Err(format!("scene color stats have {} regions", regions.len()));
    }
    for (index, name) in ["top_left", "top_right", "bottom_left", "bottom_right"]
        .into_iter()
        .enumerate()
    {
        let region = &regions[index];
        if region.get("name").and_then(|v| v.as_str()) != Some(name) {
            return Err(format!("region {index} is not {name}"));
        }
        for key in ["mean", "negative", "nan", "inf"] {
            let array = region
                .get(key)
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("{name}.{key} is not an array"))?;
            if array.len() != 4 {
                return Err(format!("{name}.{key} has {} entries", array.len()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::quarantine_216::apply_quarantine;
    use super::{
        CHANNELS, GROUP, ISOLATED, LightingTerm, OutputChannel, RIGS, cases,
        require_negative_scene_color, requires_negative_scene_color, scene_color_json_path,
        scene_color_raw_path, scene_color_sidecar_failure, spec_for, specs,
        validate_scene_color_json,
    };
    use crate::scene_dsl::cases::registry;
    use crate::scene_dsl::suite::{SuiteCaseReport, SuiteReport};
    use std::path::Path;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn group_name_is_the_cli_flag_value() {
        assert_eq!(GROUP, "warp_216_debug");
    }

    #[test]
    fn group_and_case_name_select_the_debug_cases_only() {
        let grouped = crate::scene_dsl::suite::select_cases_in(&[], Some(GROUP)).expect("group");
        assert_eq!(grouped.len(), cases().len());
        let named =
            crate::scene_dsl::suite::select_cases_in(&["warp216_constant".to_string()], None)
                .expect("name");
        assert_eq!(named.len(), 1);
        assert_eq!(named[0].name, "warp216_constant");
        let default_cases = crate::scene_dsl::suite::select_cases(&[]).expect("default");
        assert!(
            default_cases
                .iter()
                .all(|case| !case.name.starts_with("warp216_"))
        );
        let err = crate::scene_dsl::suite::select_cases_in(&[], Some("no_such_group"))
            .expect_err("unknown group");
        assert!(err.to_string().contains("no_such_group"));
    }

    #[test]
    fn default_registry_does_not_include_the_debug_group() {
        let debug_names: Vec<_> = cases().into_iter().map(|case| case.name).collect();
        assert!(debug_names.len() > 4);
        for case in registry() {
            assert!(
                !debug_names.iter().any(|name| name == &case.name),
                "{} is in the default registry",
                case.name
            );
        }
    }

    #[test]
    fn term_ids_match_the_shader_constants() {
        let shader = include_str!("../host/scene_session/warp_216_term_debug.wgsl");
        let lighting = include_str!("../../../renderide/shaders/modules/pbs/lighting.wgsl");
        for (term, token) in [
            (
                LightingTerm::Composed,
                "const DEBUG_TERM_COMPOSED: u32 = 0u;",
            ),
            (LightingTerm::Diffuse, "const DEBUG_TERM_DIFFUSE: u32 = 1u;"),
            (
                LightingTerm::Specular,
                "const DEBUG_TERM_SPECULAR: u32 = 2u;",
            ),
            (LightingTerm::Ambient, "const DEBUG_TERM_AMBIENT: u32 = 3u;"),
            (
                LightingTerm::Indirect,
                "const DEBUG_TERM_INDIRECT: u32 = 4u;",
            ),
            (LightingTerm::Fresnel, "const DEBUG_TERM_FRESNEL: u32 = 5u;"),
            (
                LightingTerm::Emission,
                "const DEBUG_TERM_EMISSION: u32 = 6u;",
            ),
        ] {
            assert!(lighting.contains(token), "{token}");
            assert_eq!(term.id(), token_value(token));
        }
        assert!(shader.contains("const DEBUG_TERM_COMPOSED: u32 = 0u;"));
        assert!(shader.contains("const DEBUG_TERM_CONSTANT: u32 = 7u;"));
        assert_eq!(LightingTerm::Constant.id(), 7.0);
        assert!(shader.contains("const DEBUG_CHANNEL_G: u32 = 1u;"));
        assert!(shader.contains("const DEBUG_CHANNEL_B: u32 = 2u;"));
        assert!(shader.contains("const DEBUG_CHANNEL_GREY: u32 = 3u;"));
        assert_eq!(OutputChannel::R.id(), 0.0);
        assert_eq!(OutputChannel::G.id(), 1.0);
        assert_eq!(OutputChannel::B.id(), 2.0);
        assert_eq!(OutputChannel::Grey.id(), 3.0);
    }

    fn token_value(token: &str) -> f32 {
        let digits = token
            .split("= ")
            .nth(1)
            .and_then(|rest| rest.strip_suffix("u;"))
            .expect("u32 literal");
        digits.parse::<u32>().expect("id") as f32
    }

    #[test]
    fn channel_selector_maps_to_the_authored_output() {
        assert_eq!(OutputChannel::R.carries_signal(), [true, false, false]);
        assert_eq!(OutputChannel::G.carries_signal(), [false, true, false]);
        assert_eq!(OutputChannel::B.carries_signal(), [false, false, true]);
        assert_eq!(OutputChannel::Grey.carries_signal(), [true, true, true]);
        assert_eq!(OutputChannel::Raw.carries_signal(), [true, true, true]);
    }

    #[test]
    fn names_are_unique_and_cover_each_rig_term_and_channel() {
        let specs = specs();
        let names: Vec<_> = specs.iter().map(|spec| spec.case_name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
        assert!(names.contains(&"warp216_directional_composed".to_string()));
        assert!(names.contains(&"warp216_point_composed".to_string()));
        assert!(names.contains(&"warp216_negative_composed".to_string()));
        assert!(names.contains(&"warp216_negparity_composed".to_string()));
        assert!(names.contains(&"warp216_negparity_diffuse_g".to_string()));
        assert!(names.contains(&"warp216_negparity_specular_grey".to_string()));
        assert!(names.contains(&"warp216_positive_composed".to_string()));
        let parity = spec_for("warp216_negparity_diffuse_r").expect("parity");
        assert!(!parity.physically_based());
        assert!(
            spec_for("warp216_negative_composed")
                .expect("clamped")
                .physically_based()
        );
        assert!(names.contains(&"warp216_constant".to_string()));
        assert!(names.contains(&"warp216_directional_diffuse_g".to_string()));
        assert!(names.contains(&"warp216_point_fresnel_grey".to_string()));
        assert_eq!(
            names.len(),
            1 + RIGS.len() * (1 + ISOLATED.len() * CHANNELS.len())
        );
        let diffuse_g = spec_for("warp216_directional_diffuse_g").expect("spec");
        assert_eq!(diffuse_g.term, LightingTerm::Diffuse);
        assert_eq!(diffuse_g.channel, OutputChannel::G);
        assert_eq!(diffuse_g.term_id(), 1.0);
        assert_eq!(diffuse_g.channel_id(), 1.0);
    }

    #[test]
    fn quarantine_does_not_relabel_a_debug_case() {
        let case = cases().into_iter().next().expect("case");
        let mut report = SuiteReport::from_cases(vec![SuiteCaseReport::plain(
            case.name,
            false,
            Path::new("out").to_path_buf(),
            Path::new("out/report.json").to_path_buf(),
            Some("case failed tolerance".to_string()),
        )]);
        report.cases[0].image_compared = true;
        report.cases[0].failure_kind =
            Some(crate::scene_dsl::suite::SuiteFailureKind::FlatOrUnstableFrame);
        report.cases[0].frame_pending = Some(0);
        report.cases[0].gpu_done = Some(true);
        report.cases[0].dropped_draws = Some(0);
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        apply_quarantine(&mut report, now, |_| true);
        assert!(report.cases[0].status.is_none());
        assert!(report.cases[0].reason.is_none());
    }

    #[test]
    fn sidecar_json_requires_four_named_quadrants() {
        let ok = r#"{
            "format": "rgba16float",
            "regions": [
                {"name": "top_left", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]},
                {"name": "top_right", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]},
                {"name": "bottom_left", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]},
                {"name": "bottom_right", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]}
            ]
        }"#;
        assert!(validate_scene_color_json(ok).is_ok());
        assert!(validate_scene_color_json(r#"{"format":"rgba8"}"#).is_err());
        assert_eq!(
            scene_color_json_path(Path::new("out/case/actual.png")),
            Path::new("out/case/actual.scene_color.json")
        );
    }

    fn quadrant_json(top_right_negative: &str) -> String {
        format!(
            r#"{{
                "format": "rgba16float",
                "regions": [
                    {{"name": "top_left", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]}},
                    {{"name": "top_right", "mean": [0, 0, 0, 1], "negative": {top_right_negative}, "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]}},
                    {{"name": "bottom_left", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]}},
                    {{"name": "bottom_right", "mean": [0, 0, 0, 1], "negative": [0, 0, 0, 0], "nan": [0, 0, 0, 0], "inf": [0, 0, 0, 0]}}
                ]
            }}"#
        )
    }

    #[test]
    fn signed_radiance_check_fails_cleanly_when_every_count_is_zero() {
        let text = quadrant_json("[0, 0, 0, 0]");
        let err = require_negative_scene_color("warp216_negparity_composed", &text)
            .expect_err("zero counts fail the case");
        assert!(err.contains("warp216_negparity_composed"), "{err}");
        assert!(err.contains("no negative samples"), "{err}");
        assert!(err.contains("signed radiance"), "{err}");
    }

    #[test]
    fn signed_radiance_check_passes_when_one_channel_is_negative() {
        let text = quadrant_json("[0, 0, 1, 0]");
        assert!(require_negative_scene_color("warp216_negparity_diffuse_b", &text).is_ok());
    }

    #[test]
    fn signed_radiance_check_rejects_a_non_integer_without_panicking() {
        let text = quadrant_json("[0, 0, \"nope\", 0]");
        let err = require_negative_scene_color("warp216_negparity_specular_g", &text)
            .expect_err("bad count");
        assert!(err.contains("not an integer"), "{err}");
        assert!(err.contains("warp216_negparity_specular_g"), "{err}");
    }

    #[test]
    fn only_signed_direct_terms_on_the_parity_rig_require_negatives() {
        for name in [
            "warp216_negparity_composed",
            "warp216_negparity_diffuse_r",
            "warp216_negparity_diffuse_g",
            "warp216_negparity_diffuse_b",
            "warp216_negparity_diffuse_grey",
            "warp216_negparity_specular_r",
            "warp216_negparity_specular_g",
            "warp216_negparity_specular_b",
            "warp216_negparity_specular_grey",
        ] {
            let spec = spec_for(name).expect(name);
            assert!(requires_negative_scene_color(spec), "{name}");
        }
        for name in [
            "warp216_negparity_fresnel_grey",
            "warp216_negparity_ambient_r",
            "warp216_negparity_indirect_b",
            "warp216_negparity_emission_grey",
            "warp216_negative_composed",
            "warp216_constant",
        ] {
            let spec = spec_for(name).expect(name);
            assert!(!requires_negative_scene_color(spec), "{name}");
        }
    }

    #[test]
    fn sidecar_failure_reports_a_missing_sign_and_skips_unsigned_terms() {
        let temp = tempfile::tempdir().expect("tempdir");
        let png = temp.path().join("actual.png");
        let text = quadrant_json("[0, 0, 0, 0]");
        std::fs::write(scene_color_raw_path(&png), b"R16F").expect("raw");
        std::fs::write(scene_color_json_path(&png), &text).expect("json");
        let composed = super::lookup("warp216_negparity_composed").expect("composed");
        let err = scene_color_sidecar_failure(&composed, &png).expect_err("signed case");
        assert!(err.contains("no negative samples"), "{err}");
        let emission = super::lookup("warp216_negparity_emission_r").expect("emission");
        assert!(scene_color_sidecar_failure(&emission, &png).is_ok());
        let clamped = super::lookup("warp216_negative_composed").expect("clamped");
        assert!(scene_color_sidecar_failure(&clamped, &png).is_ok());
    }
}
