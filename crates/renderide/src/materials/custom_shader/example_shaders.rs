//! Exotic example shaders compose through the same naga-oil path as [`super::install`].

use super::{install, keyword_names, remove, target_wgsl, variant_wgsl};
use crate::materials::{ReflectedUniformScalarKind, reflect_raster_material_wgsl};

struct ExampleShader {
    key: &'static str,
    source: &'static str,
    keyword: &'static str,
    fields: &'static [(&'static str, ReflectedUniformScalarKind)],
}

const EXAMPLES: &[ExampleShader] = &[
    ExampleShader {
        key: "example-poincare",
        source: include_str!("../../../../../RenderideMod/Shaders/example_poincare.wgsl"),
        keyword: "TILE",
        fields: &[
            ("_Color", ReflectedUniformScalarKind::Vec4),
            ("_MainTex_ST", ReflectedUniformScalarKind::Vec4),
            ("_Schlafli", ReflectedUniformScalarKind::Vec4),
            ("_Speed", ReflectedUniformScalarKind::F32),
        ],
    },
    ExampleShader {
        key: "example-gyroid",
        source: include_str!("../../../../../RenderideMod/Shaders/example_gyroid.wgsl"),
        keyword: "GYROID",
        fields: &[
            ("_Color", ReflectedUniformScalarKind::Vec4),
            ("_MainTex_ST", ReflectedUniformScalarKind::Vec4),
            ("_Frequency", ReflectedUniformScalarKind::F32),
            ("_Speed", ReflectedUniformScalarKind::F32),
        ],
    },
    ExampleShader {
        key: "example-hologram",
        source: include_str!("../../../../../RenderideMod/Shaders/example_hologram.wgsl"),
        keyword: "HOLO",
        fields: &[
            ("_Color", ReflectedUniformScalarKind::Vec4),
            ("_MainTex_ST", ReflectedUniformScalarKind::Vec4),
            ("_Scan", ReflectedUniformScalarKind::Vec4),
            ("_Fresnel", ReflectedUniformScalarKind::F32),
            ("_Speed", ReflectedUniformScalarKind::F32),
        ],
    },
    ExampleShader {
        key: "example-hieroglyph",
        source: include_str!("../../../../../RenderideMod/Shaders/example_hieroglyph.wgsl"),
        keyword: "EMIT",
        fields: &[
            ("_Color", ReflectedUniformScalarKind::Vec4),
            ("_MainTex_ST", ReflectedUniformScalarKind::Vec4),
            ("_Grid", ReflectedUniformScalarKind::Vec4),
            ("_Emission", ReflectedUniformScalarKind::F32),
            ("_Speed", ReflectedUniformScalarKind::F32),
        ],
    },
    ExampleShader {
        key: "example-nebula",
        source: include_str!("../../../../../RenderideMod/Shaders/example_nebula.wgsl"),
        keyword: "NEBULA",
        fields: &[
            ("_Color", ReflectedUniformScalarKind::Vec4),
            ("_MainTex_ST", ReflectedUniformScalarKind::Vec4),
            ("_Warp", ReflectedUniformScalarKind::Vec4),
            ("_Speed", ReflectedUniformScalarKind::F32),
        ],
    },
    ExampleShader {
        key: "example-dispersion",
        source: include_str!("../../../../../RenderideMod/Shaders/example_dispersion.wgsl"),
        keyword: "SPLIT",
        fields: &[
            ("_Color", ReflectedUniformScalarKind::Vec4),
            ("_MainTex_ST", ReflectedUniformScalarKind::Vec4),
            ("_Spread", ReflectedUniformScalarKind::F32),
            ("_Speed", ReflectedUniformScalarKind::F32),
        ],
    },
];

#[test]
fn exotic_examples_compose_through_naga_oil() {
    for example in EXAMPLES {
        let installed = install(example.key, example.source)
            .unwrap_or_else(|error| panic!("naga-oil install {}: {error}", example.key));
        let names = keyword_names(&installed.default_stem).expect("keyword");
        assert_eq!(names.as_ref(), [example.keyword.to_string()]);

        let plain = target_wgsl(&installed.default_stem).expect("mask 0");
        let plain_reflected =
            reflect_raster_material_wgsl(plain.as_ref()).expect("reflect keyword off");
        assert!(
            !plain.contains("frame_time.x"),
            "{} keyword off should not read frame time",
            example.key
        );
        assert_uniforms(&plain_reflected, example);

        let enabled = variant_wgsl(&installed.default_stem, 1)
            .unwrap_or_else(|error| panic!("naga-oil keyword {}: {error}", example.key))
            .expect("mask 1");
        let enabled_reflected =
            reflect_raster_material_wgsl(enabled.as_ref()).expect("reflect keyword on");
        assert!(
            enabled.contains("frame_time.x"),
            "{} keyword on should read frame time",
            example.key
        );
        assert!(
            enabled.contains("texture_2d"),
            "{} should bind _MainTex",
            example.key
        );
        assert_uniforms(&enabled_reflected, example);

        remove(example.key).expect("remove");
    }
}

fn assert_uniforms(reflected: &crate::materials::ReflectedRasterLayout, example: &ExampleShader) {
    let fields = &reflected.material_uniform.as_ref().expect("uniform").fields;
    assert_eq!(
        fields.len(),
        example.fields.len(),
        "{} uniform count",
        example.key
    );
    for (name, kind) in example.fields {
        let field = fields
            .get(*name)
            .unwrap_or_else(|| panic!("{} missing uniform {name}", example.key));
        assert_eq!(field.kind, *kind, "{} {name}", example.key);
    }
}
