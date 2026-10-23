//! Catalog of named integration cases.
//!
//! Each [`IntegrationCase`] pairs a name + golden + tolerance with one of the built-in
//! [`CaseTemplate`] variants. The runner ([`super::runner::run_integration_case`]) dispatches
//! on the template to drive the harness.
//!
//! New cases land here as additional builder functions returning an [`IntegrationCase`]; the
//! [`registry`] / [`lookup`] entry points expose them to the CLI and to integration `#[test]`
//! shims.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::scene::gltf_fixture::default_fixture_path;
use crate::scene::perlin::PerlinTextureSpec;

use super::tolerance::{Combine, Tolerance};

/// A single named integration test case.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrationCase {
    /// Stable identifier used for golden filenames and output directory names.
    pub name: String,
    /// Human-readable description for diagnostics and report output.
    pub description: String,
    /// Path to the committed golden PNG that `actual.png` is compared against.
    pub golden_path: PathBuf,
    /// Render target dimensions (width, height) in pixels.
    pub resolution: (u32, u32),
    /// Comparison tolerance applied during `check`.
    pub tolerance: Tolerance,
    /// Scene template selector -- drives which harness flow runs.
    pub template: CaseTemplate,
}

/// Built-in scene templates. New cases extend this enum and add a matching arm in
/// [`super::runner::run_integration_case`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseTemplate {
    /// Single procedurally-tessellated UV sphere on the renderer's `Null` fallback pipeline
    /// (no shader uploaded). Smallest end-to-end smoke test of IPC, mesh upload, frame loop,
    /// and PNG capture.
    SphereNull,
    /// Procedural torus rendered through the harness; a deterministic CPU-generated Perlin
    /// noise RGBA texture is also written to the per-case output directory as a side artifact
    /// (`perlin_texture.png`).
    TorusUnlitPerlin {
        /// Perlin noise generator parameters used for the side artifact.
        perlin: PerlinTextureSpec,
    },
    /// Four unlit procedural primitives using checker, UV-ramp, and solid-color material paths.
    MultiPrimitiveUnlitGrid,
    /// Four PBS metallic spheres that vary color, metallic, glossiness, and regular lights.
    PbsLitMaterialMatrix,
    /// Overlapping alpha-test quads using a generated binary mask texture and scalar cutoff.
    AlphaCutoutMaskedQuads,
    /// Optional textured static mesh imported from a GLB fixture.
    GltfTexturedStaticMesh {
        /// Fixture path loaded by the runner.
        path: PathBuf,
    },
    /// Custom rainbow material rendered twice. `actual.png` is the frame after `_Color` and
    /// `RAINBOW` change. `plain.png` is the frame before that update.
    CustomShaderPropertyUpdate,
    /// Poincare-disk tiling and a raymarched gyroid, each with `_Speed` 0.
    ExoticShaderPair,
    /// Rainbow custom shader beside packaged Unlit. `plain.png` is the assigned pair.
    /// `actual.png` is the frame after the left material is cleared.
    CustomShaderBesideUnlit,
}

/// Default golden directory, relative to the workspace root: `crates/renderide-test/goldens/`.
pub fn default_goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("goldens")
}

/// Single UV sphere on the renderer's `Null` fallback pipeline; smallest end-to-end smoke
/// test of IPC, mesh upload, frame loop, and PNG capture.
pub fn unlit_sphere() -> IntegrationCase {
    IntegrationCase {
        name: "unlit_sphere".to_string(),
        description:
            "Single UV sphere on the Null fallback pipeline; smallest end-to-end smoke test."
                .to_string(),
        golden_path: default_goldens_dir().join("unlit_sphere.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.65, 64, 0.40, 8, 8, 0.02),
        template: CaseTemplate::SphereNull,
    }
}

/// Procedural torus rendered with an embedded unlit shader.
///
/// `Unlit.shader` is resolved through
/// the test-only `RENDERIDE_TEST_STEM:` sentinel with the `_TEXTURE` variant bit, with the
/// CPU-generated Perlin noise bound to the material's `_Tex` slot via the IPC upload chain.
pub fn torus_unlit_perlin() -> IntegrationCase {
    IntegrationCase {
        name: "torus_unlit_perlin".to_string(),
        description:
            "Procedural torus rendered with the embedded unlit shader and a CPU-generated Perlin noise texture bound to `_Tex`; the same Perlin PNG is emitted as a per-case side artifact."
                .to_string(),
        golden_path: default_goldens_dir().join("torus_unlit_perlin.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.85, 32, 0.10, 16, 16, 0.04),
        template: CaseTemplate::TorusUnlitPerlin {
            perlin: PerlinTextureSpec {
                width: 256,
                height: 256,
                seed: 0x00C0_FFEE,
                octaves: 5,
                lacunarity: 2.0,
                gain: 0.5,
                scale: 4.0,
                tint: [240, 200, 96],
            },
        },
    }
}

/// Multi-object unlit case that exercises cube, sphere, torus, and quad mesh paths plus
/// texture/color Unlit variants in one compact render.
pub fn multi_primitive_unlit_grid() -> IntegrationCase {
    IntegrationCase {
        name: "multi_primitive_unlit_grid".to_string(),
        description:
            "Four procedural unlit primitives with checker, UV-ramp, and color material variants."
                .to_string(),
        golden_path: default_goldens_dir().join("multi_primitive_unlit_grid.png"),
        resolution: (320, 320),
        tolerance: visual_tolerance(0.82, 42, 0.16, 24, 32, 0.08),
        template: CaseTemplate::MultiPrimitiveUnlitGrid,
    }
}

/// Lit PBS material matrix that covers scalar material updates, color packing, and regular
/// scene-light submission without requiring texture fixtures.
pub fn pbs_lit_material_matrix() -> IntegrationCase {
    IntegrationCase {
        name: "pbs_lit_material_matrix".to_string(),
        description:
            "Four PBS metallic spheres varying color, metallic, and glossiness under directional and point lights."
                .to_string(),
        golden_path: default_goldens_dir().join("pbs_lit_material_matrix.png"),
        resolution: (320, 320),
        tolerance: visual_tolerance(0.78, 56, 0.22, 18, 24, 0.06),
        template: CaseTemplate::PbsLitMaterialMatrix,
    }
}

/// Alpha-test case that verifies scalar `_Cutoff` material updates, alpha sampling, and
/// overlapping transparent-ish geometry ordering through a binary mask texture.
pub fn alpha_cutout_masked_quads() -> IntegrationCase {
    IntegrationCase {
        name: "alpha_cutout_masked_quads".to_string(),
        description:
            "Overlapping alpha-test quads using a generated mask texture and scalar cutoff."
                .to_string(),
        golden_path: default_goldens_dir().join("alpha_cutout_masked_quads.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.82, 48, 0.18, 20, 18, 0.05),
        template: CaseTemplate::AlphaCutoutMaskedQuads,
    }
}

/// Custom rainbow shader assigned to one material. The first frame samples `_MainTex * _Color`
/// with `RAINBOW` off. The second frame changes `_Color` and turns `RAINBOW` on. `_Speed` is 0.
pub fn custom_shader_property_update() -> IntegrationCase {
    IntegrationCase {
        name: "custom_shader_property_update".to_string(),
        description:
            "Rainbow custom shader on one material. The pre-update frame is _MainTex * _Color with RAINBOW off. The compared frame changes _Color and turns RAINBOW on. _Speed stays 0."
                .to_string(),
        golden_path: default_goldens_dir().join("custom_shader_property_update.png"),
        resolution: (320, 320),
        tolerance: Tolerance {
            ssim_min: Some(0.72),
            max_abs_diff: Some(56),
            max_failing_pixel_fraction: Some(0.22),
            min_luma_range: Some(20),
            min_unique_colors: Some(8),
            min_non_background_pixel_fraction: Some(0.12),
            combine: Combine::And,
        },
        template: CaseTemplate::CustomShaderPropertyUpdate,
    }
}

/// Rainbow on the left (`RAINBOW` off) and packaged textured Unlit on the right, then clear.
pub fn custom_shader_beside_unlit() -> IntegrationCase {
    IntegrationCase {
        name: "custom_shader_beside_unlit".to_string(),
        description:
            "Left quad is ExampleRainbowWgsl with RAINBOW off. Right quad is packaged Unlit on _Tex. The compared frame clears the left material."
                .to_string(),
        golden_path: default_goldens_dir().join("custom_shader_beside_unlit.png"),
        resolution: (320, 320),
        tolerance: Tolerance {
            ssim_min: Some(0.72),
            max_abs_diff: Some(56),
            max_failing_pixel_fraction: Some(0.025),
            min_luma_range: Some(20),
            min_unique_colors: Some(8),
            min_non_background_pixel_fraction: Some(0.12),
            combine: Combine::And,
        },
        template: CaseTemplate::CustomShaderBesideUnlit,
    }
}

/// Poincare disk on the left and a gyroid SDF on the right. Both materials keep `_Speed` at 0.
pub fn exotic_shader_pair() -> IntegrationCase {
    IntegrationCase {
        name: "exotic_shader_pair".to_string(),
        description:
            "Poincare-disk tiling and a raymarched gyroid on two quads. TILE and GYROID are on. _Speed is 0 on both."
                .to_string(),
        golden_path: default_goldens_dir().join("exotic_shader_pair.png"),
        resolution: (320, 320),
        tolerance: Tolerance {
            ssim_min: Some(0.75),
            max_abs_diff: Some(64),
            max_failing_pixel_fraction: Some(0.28),
            min_luma_range: Some(24),
            min_unique_colors: Some(16),
            min_non_background_pixel_fraction: Some(0.15),
            combine: Combine::Or,
        },
        template: CaseTemplate::ExoticShaderPair,
    }
}

/// Optional GLB fixture case. The registry includes it only when the fixture file is present,
/// so the suite remains useful before a commit-safe model has been added.
pub fn gltf_textured_static_mesh(path: PathBuf) -> IntegrationCase {
    IntegrationCase {
        name: "gltf_textured_static_mesh".to_string(),
        description: "Static textured mesh imported from a GLB fixture and rendered through Unlit."
            .to_string(),
        golden_path: default_goldens_dir().join("gltf_textured_static_mesh.png"),
        resolution: (320, 320),
        tolerance: visual_tolerance(0.82, 48, 0.18, 18, 18, 0.05),
        template: CaseTemplate::GltfTexturedStaticMesh { path },
    }
}

/// Returns every case in the suite. The order is stable; CLI listings and reports rely on it.
pub fn registry() -> Vec<IntegrationCase> {
    let mut cases = vec![
        unlit_sphere(),
        torus_unlit_perlin(),
        multi_primitive_unlit_grid(),
        pbs_lit_material_matrix(),
        alpha_cutout_masked_quads(),
        custom_shader_property_update(),
        custom_shader_beside_unlit(),
        exotic_shader_pair(),
    ];
    let gltf_fixture = default_fixture_path();
    if gltf_fixture.is_file() {
        cases.push(gltf_textured_static_mesh(gltf_fixture));
    }
    cases
}

/// Looks up a case by [`IntegrationCase::name`].
pub fn lookup(name: &str) -> Option<IntegrationCase> {
    registry().into_iter().find(|c| c.name == name)
}

impl IntegrationCase {
    /// Golden for the pre-update capture, when this case records two frames.
    pub fn plain_capture_golden(&self) -> Option<PathBuf> {
        match self.template {
            CaseTemplate::CustomShaderPropertyUpdate | CaseTemplate::CustomShaderBesideUnlit => {
                Some(plain_png_path(&self.golden_path))
            }
            _ => None,
        }
    }
}

fn plain_png_path(golden: &std::path::Path) -> PathBuf {
    let mut name = golden.file_stem().unwrap_or_default().to_os_string();
    name.push("_plain.png");
    golden.with_file_name(name)
}

fn visual_tolerance(
    ssim_min: f64,
    max_abs_diff: u8,
    max_failing_pixel_fraction: f64,
    min_luma_range: u8,
    min_unique_colors: usize,
    min_non_background_pixel_fraction: f64,
) -> Tolerance {
    Tolerance {
        ssim_min: Some(ssim_min),
        max_abs_diff: Some(max_abs_diff),
        max_failing_pixel_fraction: Some(max_failing_pixel_fraction),
        min_luma_range: Some(min_luma_range),
        min_unique_colors: Some(min_unique_colors),
        min_non_background_pixel_fraction: Some(min_non_background_pixel_fraction),
        combine: Combine::Or,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_non_empty() {
        assert!(!registry().is_empty());
    }

    #[test]
    fn unlit_sphere_is_registered() {
        assert!(lookup("unlit_sphere").is_some());
    }

    #[test]
    fn new_procedural_cases_are_registered() {
        assert!(lookup("multi_primitive_unlit_grid").is_some());
        assert!(lookup("pbs_lit_material_matrix").is_some());
        assert!(lookup("alpha_cutout_masked_quads").is_some());
    }

    #[test]
    fn unknown_case_is_none() {
        assert!(lookup("nonexistent_case").is_none());
    }

    #[test]
    fn case_names_are_unique() {
        let cases = registry();
        let mut names: Vec<_> = cases.iter().map(|c| c.name.clone()).collect();
        names.sort();
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped, "duplicate case name");
    }

    #[test]
    fn golden_filenames_match_case_names() {
        for case in registry() {
            let expected = format!("{}.png", case.name);
            assert_eq!(
                case.golden_path.file_name().and_then(|name| name.to_str()),
                Some(expected.as_str()),
                "{} must use goldens/{expected}",
                case.name
            );
        }
    }

    #[test]
    fn registered_goldens_exist() {
        for case in registry() {
            assert!(
                case.golden_path.is_file(),
                "{} golden missing at {}",
                case.name,
                case.golden_path.display()
            );
            if let Some(plain) = case.plain_capture_golden() {
                assert!(
                    plain.is_file(),
                    "{} plain golden missing at {}",
                    case.name,
                    plain.display()
                );
            }
        }
    }

    #[test]
    fn custom_shader_property_update_has_a_plain_golden() {
        let case = lookup("custom_shader_property_update").expect("registered");
        assert_eq!(
            case.plain_capture_golden()
                .and_then(|path| path.file_name().map(|name| name.to_os_string())),
            Some(std::ffi::OsString::from(
                "custom_shader_property_update_plain.png"
            ))
        );
    }

    /// The two goldens must miss the case tolerance, and the failing-pixel fraction must sit
    /// above the configured limit. A no-op clear would otherwise pass.
    #[test]
    fn beside_unlit_goldens_disagree_and_the_left_quad_flattens() {
        let case = lookup("custom_shader_beside_unlit").expect("registered");
        let plain_path = case.plain_capture_golden().expect("plain golden");
        let plain = load_rgba(&plain_path);
        let cleared = load_rgba(&case.golden_path);
        let plain_vs_cleared = case
            .tolerance
            .evaluate(&plain, &cleared)
            .expect("plain against cleared");
        let cleared_vs_plain = case
            .tolerance
            .evaluate(&cleared, &plain)
            .expect("cleared against plain");
        assert!(
            !plain_vs_cleared.passed && !cleared_vs_plain.passed,
            "the two frames must fail the case tolerance against each other"
        );
        let limit = case
            .tolerance
            .max_failing_pixel_fraction
            .expect("pixel fraction");
        let fraction = plain_vs_cleared
            .failing_pixel_fraction
            .expect("failing fraction");
        let reverse = cleared_vs_plain
            .failing_pixel_fraction
            .expect("reverse failing fraction");
        assert!(
            fraction > limit && reverse > limit,
            "cross-golden fractions {fraction} and {reverse} must exceed tolerance {limit}"
        );

        let (plain_colors, plain_luma) = left_quad_center(&plain);
        let (cleared_colors, cleared_luma) = left_quad_center(&cleared);
        assert!(
            plain_colors >= 8 && plain_luma >= 20,
            "plain left centre should be a checker, colors={plain_colors} luma={plain_luma}"
        );
        assert!(
            cleared_colors <= 2 && cleared_luma <= 2,
            "cleared left centre should be flat, colors={cleared_colors} luma={cleared_luma}"
        );
    }

    fn load_rgba(path: &std::path::Path) -> image::RgbaImage {
        image::open(path)
            .unwrap_or_else(|err| panic!("open {}: {err}", path.display()))
            .to_rgba8()
    }

    /// Unique RGB colors and luma range in a window at the left quad's centre.
    fn left_quad_center(image: &image::RgbaImage) -> (usize, u8) {
        let half = image.width() / 2;
        let mut min_x = half;
        let mut max_x = 0;
        let mut min_y = image.height();
        let mut max_y = 0;
        for y in 0..image.height() {
            for x in 0..half {
                let pixel = image.get_pixel(x, y);
                if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
                    continue;
                }
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
        let center_x = u32::midpoint(min_x, max_x);
        let center_y = u32::midpoint(min_y, max_y);
        let radius = 12;
        let mut colors = std::collections::HashSet::new();
        let mut luma_min = 255u8;
        let mut luma_max = 0u8;
        let y0 = center_y.saturating_sub(radius);
        let y1 = (center_y + radius).min(image.height());
        let x0 = center_x.saturating_sub(radius);
        let x1 = (center_x + radius).min(image.width());
        for y in y0..y1 {
            for x in x0..x1 {
                let pixel = image.get_pixel(x, y);
                colors.insert((pixel[0], pixel[1], pixel[2]));
                let luma = ((77 * u32::from(pixel[0])
                    + 150 * u32::from(pixel[1])
                    + 29 * u32::from(pixel[2]))
                    / 256) as u8;
                luma_min = luma_min.min(luma);
                luma_max = luma_max.max(luma);
            }
        }
        (colors.len(), luma_max.saturating_sub(luma_min))
    }
}
