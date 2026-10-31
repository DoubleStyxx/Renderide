//! Material shaders drawn by the golden scene-DSL cases.
//!
//! The list is built by constructing each suite case's [`SessionTemplate`] and reading
//! the shader bindings from [`crate::host::scene_session::material_draws`]. That is the
//! same scene setup and custom-shader assignment the harness captures. It is not a
//! separate table of root names.

use crate::host::scene_session::{SessionTemplate, material_draws};
use crate::scene::gltf_fixture::default_fixture_path;
use crate::scene::mesh::Mesh;

use super::cases::{self, CaseTemplate, IntegrationCase};

/// One shader a golden case still has bound at capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoldenShaderDraw {
    /// Unity shader asset filename, such as `Unlit.shader`.
    pub shader_asset: &'static str,
    /// Host variant bits, when the case sets them.
    pub variant_bits: Option<u32>,
}

/// Shaders one golden case draws, after custom-shader assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoldenCaseMaterials {
    /// [`IntegrationCase::name`].
    pub case: String,
    /// Shaders still bound at capture.
    pub shaders: Vec<GoldenShaderDraw>,
    /// A renderable uploaded no shader, so the frame uses the null material.
    pub null_fallback: bool,
}

/// Materials drawn by every suite golden, including `shadowed_caster_move` on Windows.
///
/// [`cases::registry`] omits that case on Windows because WARP does not produce its
/// golden. The case and its golden still exist, and coverage follows the case
/// definition. The optional GLB case is included only when its fixture file exists,
/// matching the registry.
pub fn golden_case_materials() -> Vec<GoldenCaseMaterials> {
    coverage_cases()
        .into_iter()
        .map(|case| {
            let draws = material_draws(session_template(&case.template));
            GoldenCaseMaterials {
                case: case.name,
                shaders: draws
                    .shaders
                    .into_iter()
                    .map(|shader| GoldenShaderDraw {
                        shader_asset: shader.shader_asset,
                        variant_bits: shader.variant_bits,
                    })
                    .collect(),
                null_fallback: draws.null_fallback,
            }
        })
        .collect()
}

fn coverage_cases() -> Vec<IntegrationCase> {
    let mut cases = cases::registry();
    if !cases.iter().any(|case| case.name == "shadowed_caster_move") {
        cases.push(cases::shadowed_caster_move());
    }
    let fixture = default_fixture_path();
    if fixture.is_file()
        && !cases
            .iter()
            .any(|case| case.name == "gltf_textured_static_mesh")
    {
        cases.push(cases::gltf_textured_static_mesh(fixture));
    }
    cases
}

fn session_template(template: &CaseTemplate) -> SessionTemplate {
    let (rgba, size) = dummy_rgba();
    match template {
        CaseTemplate::SphereNull => SessionTemplate::Sphere,
        CaseTemplate::TorusUnlitPerlin { .. } => SessionTemplate::Torus {
            texture_rgba: rgba,
            texture_size: size,
        },
        CaseTemplate::MultiPrimitiveUnlitGrid => SessionTemplate::MultiPrimitiveUnlitGrid {
            checker_rgba: rgba.clone(),
            uv_ramp_rgba: rgba,
            texture_size: size,
        },
        CaseTemplate::PbsLitMaterialMatrix => SessionTemplate::PbsLitMaterialMatrix,
        CaseTemplate::PbsMetallicPremultiply => SessionTemplate::PbsMetallicPremultiply,
        CaseTemplate::AlphaCutoutMaskedQuads => SessionTemplate::AlphaCutoutMaskedQuads {
            mask_rgba: rgba,
            texture_size: size,
        },
        CaseTemplate::GltfTexturedStaticMesh { .. } => SessionTemplate::GltfTexturedStaticMesh {
            mesh: Mesh::default(),
            texture_rgba: rgba,
            texture_size: size,
        },
        CaseTemplate::CustomShaderPropertyUpdate => SessionTemplate::CustomShaderPropertyUpdate,
        CaseTemplate::ExoticShaderPair => SessionTemplate::ExoticShaderPair,
        CaseTemplate::CoplanarCanvasUiQuads => SessionTemplate::CoplanarCanvasUiQuads,
        CaseTemplate::TiltedCanvasUiQuads => SessionTemplate::TiltedCanvasUiQuads,
        CaseTemplate::CanvasOffsetBiasQuad => SessionTemplate::CanvasOffsetBiasQuad,
        CaseTemplate::TiltedCanvasDistanceBias => SessionTemplate::TiltedCanvasDistanceBias,
        CaseTemplate::ShadowedCasterMove => SessionTemplate::ShadowedCasterMove,
        CaseTemplate::DoubleBufferNothingOnSkybox => SessionTemplate::DoubleBufferNothingOnSkybox,
        CaseTemplate::NegativeLightPair {
            physically_based,
            include_negative,
            negative_color,
        } => SessionTemplate::NegativeLightPair {
            physically_based: *physically_based,
            include_negative: *include_negative,
            negative_color: *negative_color,
        },
        CaseTemplate::Warp216Debug { spec } => SessionTemplate::Warp216Debug { spec: *spec },
    }
}

fn dummy_rgba() -> (Vec<u8>, (u32, u32)) {
    (vec![255, 0, 0, 255], (1, 1))
}

/// Spec used only so the `Warp216Debug` template arm stays constructible in tests.
#[cfg(test)]
fn sample_warp_spec() -> super::warp_216_debug::Spec {
    use super::warp_216_debug::{LightRig, LightingTerm, OutputChannel, Spec};

    Spec {
        rig: LightRig::Directional,
        term: LightingTerm::Composed,
        channel: OutputChannel::Raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_draws_null_unlit_pbs_and_ui_unlit() {
        let draws = golden_case_materials();
        let by_name = |name: &str| {
            draws
                .iter()
                .find(|case| case.case == name)
                .unwrap_or_else(|| panic!("missing {name}"))
        };
        let sphere = by_name("unlit_sphere");
        assert!(sphere.null_fallback);
        assert!(sphere.shaders.is_empty());
        assert_eq!(
            shader_names(by_name("torus_unlit_perlin")),
            ["Unlit.shader"]
        );
        assert_eq!(
            shader_names(by_name("pbs_lit_material_matrix")),
            ["PBSMetallic.shader"]
        );
        let premultiply = by_name("pbs_metallic_premultiply");
        assert_eq!(shader_names(premultiply), ["PBSMetallic.shader"]);
        assert_eq!(premultiply.shaders[0].variant_bits, Some(0x0000_0002));
        assert_eq!(
            shader_names(by_name("coplanar_canvas_ui_quads")),
            ["UI_Unlit.shader"]
        );
        assert_eq!(
            shader_names(by_name("canvas_offset_bias_quad")),
            ["UI_Unlit.shader", "Unlit.shader"]
        );
    }

    fn shader_names(case: &GoldenCaseMaterials) -> Vec<&str> {
        case.shaders
            .iter()
            .map(|shader| shader.shader_asset)
            .collect()
    }

    #[test]
    fn custom_shader_goldens_replace_the_placeholder() {
        let draws = golden_case_materials();
        for name in ["custom_shader_property_update", "exotic_shader_pair"] {
            let case = draws
                .iter()
                .find(|case| case.case == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert!(case.shaders.is_empty(), "{name}");
            assert!(!case.null_fallback, "{name}");
        }
    }

    #[test]
    fn shadowed_caster_is_included_even_when_the_registry_omits_it() {
        let draws = golden_case_materials();
        assert!(draws.iter().any(|case| case.case == "shadowed_caster_move"));
    }

    #[test]
    fn warp216_backdrop_stays_unlit_and_the_sphere_does_not() {
        let draws = material_draws(SessionTemplate::Warp216Debug {
            spec: sample_warp_spec(),
        });
        assert_eq!(
            draws
                .shaders
                .iter()
                .map(|shader| shader.shader_asset)
                .collect::<Vec<_>>(),
            ["Unlit.shader"]
        );
        assert!(!draws.null_fallback);
    }

    #[test]
    fn session_template_match_accepts_every_case_template() {
        let _ = session_template(&CaseTemplate::Warp216Debug {
            spec: sample_warp_spec(),
        });
        for case in coverage_cases() {
            let _ = session_template(&case.template);
        }
    }
}
