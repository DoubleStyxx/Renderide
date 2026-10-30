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
    /// Coplanar UI quads: a small earlier quad must not paint over the later larger quad.
    CoplanarCanvasUiQuads,
    /// Tilted parent/child UI canvases at two camera-relative yaws.
    TiltedCanvasUiQuads,
    /// UI quad that stays behind an opaque quad unless the reverse-Z NDC bias is applied.
    CanvasOffsetBiasQuad,
    /// Tilted canvas siblings at 18 m. The later child wins by the sibling-step bias.
    TiltedCanvasDistanceBias,
    /// Hard-shadowed cube on a PBS floor, static then moved.
    ShadowedCasterMove,
    /// Double-buffered Nothing camera stacked above a normal Skybox camera on one texture.
    DoubleBufferNothingOnSkybox,
    /// PBS sphere under a positive directional light and an optional negative directional.
    ///
    /// `physically_based` selects the renderer's opt-in intensity clamp. `include_negative`
    /// is true for the committed goldens and false for the positive-only reference render.
    NegativeLightPair {
        /// Spawns the renderer with physically based light upload.
        physically_based: bool,
        /// Includes the negative directional light in the submitted scene.
        include_negative: bool,
    },
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

/// Hard spot shadow on a PBS cube and floor.
///
/// The companion golden is the static frame. The compared golden is the same scene after the
/// cube moves. The session rejects a static recapture unless it is a lit pixel match with zero
/// atlas passes. Windows CI uses DX12 WARP, which does not render this scene, so the case is
/// not in the default suite there.
pub fn shadowed_caster_move() -> IntegrationCase {
    IntegrationCase {
        name: "shadowed_caster_move".to_string(),
        description:
            "PBS floor and cube under a hard spot shadow on lavapipe. plain.png is the static frame. The harness requires the next static frame to match it exactly and to record zero shadow-atlas passes, then actual.png is the frame after the cube moves and the pass log must show one atlas submit."
                .to_string(),
        golden_path: default_goldens_dir().join("shadowed_caster_move.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.78, 56, 0.22, 18, 24, 0.06),
        template: CaseTemplate::ShadowedCasterMove,
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

/// Coplanar `UI/Unlit` quads that share a center and sorting order.
///
/// The smaller red quad is submitted first. Canvas order plus a reverse-Z depth step must draw
/// the larger blue quad on top. The red quad sits entirely inside the blue one, so a correct
/// capture is background plus one blue color.
pub fn coplanar_canvas_ui_quads() -> IntegrationCase {
    IntegrationCase {
        name: "coplanar_canvas_ui_quads".to_string(),
        description:
            "Two coplanar UI/Unlit quads. The later larger quad covers the earlier smaller one."
                .to_string(),
        golden_path: default_goldens_dir().join("coplanar_canvas_ui_quads.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.9, 24, 0.08, 16, 2, 0.04),
        template: CaseTemplate::CoplanarCanvasUiQuads,
    }
}

/// Tilted parent and child `UI/Unlit` quads at +35° and −40° yaw.
///
/// The child is offset inside the parent's plane so its AABB center is farther along camera +Z.
/// A camera-forward center key draws the child first and the parent covers it. The plane-distance
/// key keeps collect order, so each child stays blue on its red parent. The face-on coplanar
/// golden still passes with no bias; this one is the tilted ordering probe.
pub fn tilted_canvas_ui_quads() -> IntegrationCase {
    IntegrationCase {
        name: "tilted_canvas_ui_quads".to_string(),
        description: "Tilted parent/child UI quads at two yaws. Each later child stays visible."
            .to_string(),
        golden_path: default_goldens_dir().join("tilted_canvas_ui_quads.png"),
        resolution: (256, 256),
        // `And` so a covered child fails even when SSIM stays high. The center-depth head
        // (`74bd3657`) scores about 0.94 SSIM and a 0.056 failing fraction on this image.
        tolerance: Tolerance {
            ssim_min: Some(0.98),
            max_abs_diff: Some(24),
            max_failing_pixel_fraction: Some(0.02),
            min_luma_range: Some(16),
            min_unique_colors: Some(3),
            min_non_background_pixel_fraction: Some(0.04),
            combine: Combine::And,
        },
        template: CaseTemplate::TiltedCanvasUiQuads,
    }
}

/// Opaque red quad with a smaller UI quad 0.6 mm farther and `_OffsetUnits = -64`.
///
/// Four D24 quanta times 64 units pull the UI about 1.37 mm closer at the harness camera
/// (3 m, near 0.1, far 100), so the center is blue. Hardware bias alone does not clear 0.6 mm,
/// so the same scene stays red without the NDC term. The coplanar golden is not this probe:
/// it passes on `LEqual` with no bias.
pub fn canvas_offset_bias_quad() -> IntegrationCase {
    IntegrationCase {
        name: "canvas_offset_bias_quad".to_string(),
        description: "A UI quad 0.6 mm behind an opaque quad. Offset units must pull it in front."
            .to_string(),
        golden_path: default_goldens_dir().join("canvas_offset_bias_quad.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.9, 24, 0.08, 16, 3, 0.04),
        template: CaseTemplate::CanvasOffsetBiasQuad,
    }
}

/// Tilted parent and child `UI/Unlit` quads at 18 m, yawed 60°.
///
/// The child is 0.15 mm farther along the plane normal, still inside one 0.5 mm quantum, so it
/// takes sibling step 1. That step is about 0.77 mm toward the camera at 18 m (near 0.1, far 100)
/// and clears the 0.30 mm view-ray gap. One D24 ulp does not. `_OffsetUnits` stays 0, so the
/// pixel is the sibling step and not the authored offset. Both quads set `_ZWrite` to 1.
pub fn tilted_canvas_distance_bias() -> IntegrationCase {
    IntegrationCase {
        name: "tilted_canvas_distance_bias".to_string(),
        description: "Tilted UI siblings at 18 m. The later child's depth step must cover it."
            .to_string(),
        golden_path: default_goldens_dir().join("tilted_canvas_distance_bias.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.9, 24, 0.08, 16, 3, 0.04),
        template: CaseTemplate::TiltedCanvasDistanceBias,
    }
}

/// Normal Skybox camera at depth 0 and a double-buffered Nothing camera at depth 1.
///
/// Both write one render texture. The captured main view is a quad sampling that texture, with a
/// red sphere left visible against the sky clear. The Nothing camera must keep this frame's sky
/// instead of copying history over it.
pub fn double_buffer_nothing_on_skybox() -> IntegrationCase {
    IntegrationCase {
        name: "double_buffer_nothing_on_skybox".to_string(),
        description:
            "Double-buffered Nothing camera stacked above a normal Skybox camera on one render texture. The main view samples that texture. The sky clear must survive the later Nothing camera."
                .to_string(),
        golden_path: default_goldens_dir().join("double_buffer_nothing_on_skybox.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.9, 24, 0.08, 16, 2, 0.03),
        template: CaseTemplate::DoubleBufferNothingOnSkybox,
    }
}

/// PBS sphere with a positive directional light and a negative directional, parity mode.
///
/// The negative intensity is uploaded unchanged, so that light darkens one side of the sphere.
pub fn negative_light_parity() -> IntegrationCase {
    IntegrationCase {
        name: "negative_light_parity".to_string(),
        description:
            "PBS sphere under a positive directional light and a negative directional. Parity mode keeps the negative intensity, so one side of the sphere darkens."
                .to_string(),
        golden_path: default_goldens_dir().join("negative_light_parity.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.78, 56, 0.22, 18, 24, 0.06),
        template: CaseTemplate::NegativeLightPair {
            physically_based: false,
            include_negative: true,
        },
    }
}

/// Same sphere and lights as [`negative_light_parity`], with physically based light upload.
///
/// Negative intensity is clamped to zero. The capture must match the same scene rendered
/// without the negative light, within this case's tolerance.
pub fn negative_light_physically_based() -> IntegrationCase {
    IntegrationCase {
        name: "negative_light_physically_based".to_string(),
        description:
            "PBS sphere under a positive directional light and a negative directional. Physically based upload clamps the negative intensity to zero. The capture must match the same scene without that light."
                .to_string(),
        golden_path: default_goldens_dir().join("negative_light_physically_based.png"),
        resolution: (256, 256),
        tolerance: visual_tolerance(0.78, 56, 0.22, 18, 24, 0.06),
        template: CaseTemplate::NegativeLightPair {
            physically_based: true,
            include_negative: true,
        },
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
        negative_light_parity(),
        negative_light_physically_based(),
        alpha_cutout_masked_quads(),
        custom_shader_property_update(),
        exotic_shader_pair(),
        coplanar_canvas_ui_quads(),
        tilted_canvas_ui_quads(),
        canvas_offset_bias_quad(),
        tilted_canvas_distance_bias(),
        double_buffer_nothing_on_skybox(),
    ];
    // WARP limitation, not a shadow-cache diff. Windows CI is DX12 Microsoft Basic Render Driver
    // (Vulkan reports ERROR_INCOMPATIBLE_DRIVER). On that adapter the static frame was a 48-color
    // gray slab and the post-move frame stayed clear-black for the case timeout, while unshadowed
    // PBS goldens passed. The case does not exist on master, so there is no master WARP image.
    // Lavapipe still runs it on Linux and macOS.
    #[cfg(not(windows))]
    cases.push(shadowed_caster_move());
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
            CaseTemplate::CustomShaderPropertyUpdate | CaseTemplate::ShadowedCasterMove => {
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
    fn negative_light_cases_are_registered() {
        let parity = lookup("negative_light_parity").expect("parity case");
        let physically_based = lookup("negative_light_physically_based").expect("pb case");
        assert!(matches!(
            parity.template,
            CaseTemplate::NegativeLightPair {
                physically_based: false,
                include_negative: true,
            }
        ));
        assert!(matches!(
            physically_based.template,
            CaseTemplate::NegativeLightPair {
                physically_based: true,
                include_negative: true,
            }
        ));
        assert_eq!(parity.resolution, physically_based.resolution);
        assert_eq!(
            parity.tolerance.ssim_min,
            physically_based.tolerance.ssim_min
        );
    }

    #[test]
    fn double_buffer_nothing_on_skybox_is_registered() {
        let case = lookup("double_buffer_nothing_on_skybox").expect("registered");
        assert!(matches!(
            case.template,
            CaseTemplate::DoubleBufferNothingOnSkybox
        ));
        assert_eq!(case.resolution, (256, 256));
    }

    #[test]
    fn shadowed_caster_move_stays_out_of_the_windows_suite() {
        let present = lookup("shadowed_caster_move").is_some();
        #[cfg(windows)]
        assert!(!present);
        #[cfg(not(windows))]
        assert!(present);
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
}
