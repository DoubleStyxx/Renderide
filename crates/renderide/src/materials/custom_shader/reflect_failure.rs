//! A reflection failure must not unpublish the stem that is already installed.
//!
//! `prepare_install` reflects the replacement before `commit_install` takes the write lock.
//! Binding rejection already left the old stem in place, because that check ran before the
//! remove. Vertex-format conflicts are raised later, by `reflect_authored_material`, which
//! walks every pass vertex entry. `validate_composed_bindings` only reflects `vs_main`.

use std::sync::Arc;

use super::{install, remove, source_generation, take_invalidated_stems, target_wgsl};

const KEY: &str = "reflect-fail-keep";
const SOLID: &str = include_str!("../../../../../RenderideMod/Shaders/example_solid.wgsl");

#[test]
fn failed_reflection_keeps_the_previous_stem_and_wgsl() {
    let installed = install(KEY, SOLID).expect("seed");
    let stem = installed.default_stem;
    let multiview = format!("{}_multiview", installed.base_stem);
    let generation = installed.generation;
    let published = target_wgsl(&stem).expect("published wgsl");
    assert_eq!(source_generation(&stem), generation);
    assert_eq!(source_generation(&multiview), generation);

    let conflict = conflicting_vertex_formats(SOLID);
    let error = install(KEY, &conflict).expect_err("vertex formats disagree");
    assert!(
        error.contains("incompatible formats"),
        "expected reflect_authored_material to reject the replacement, got {error}"
    );

    let still = target_wgsl(&stem).expect("previous stem stays published");
    assert!(
        Arc::ptr_eq(&published, &still),
        "failed reflection replaced the published WGSL"
    );
    assert_eq!(source_generation(&stem), generation);
    assert_eq!(source_generation(&multiview), generation);
    assert!(target_wgsl(&multiview).is_some());
    let invalidated = take_invalidated_stems();
    assert!(
        !invalidated
            .iter()
            .any(|(name, _)| name == &stem || name == &multiview),
        "failed reflection invalidated the published stem: {invalidated:?}"
    );

    remove(KEY).expect("remove");
    assert!(target_wgsl(&stem).is_none());
    assert_eq!(source_generation(&stem), 0);
}

/// Adds a second forward pass whose vertex entry declares `@location(0)` as `vec2`.
///
/// The seeded program's `vs_main` declares that location as `vec4`. Compose and
/// `validate_composed_bindings` accept the replacement. `reflect_authored_material` does not.
fn conflicting_vertex_formats(solid: &str) -> String {
    const AUX: &str = r#"
@vertex
fn vs_aux(
    @builtin(instance_index) instance_index: u32,
#ifdef MULTIVIEW
    @builtin(view_index) view_idx: u32,
#endif
    @location(0) pos: vec2<f32>,
) -> VertexOutput {
    let d = pd::get_draw(instance_index);
    let world_p = mv::world_position(d, vec4<f32>(pos, 0.0, 1.0));
#ifdef MULTIVIEW
    let vp = mv::select_view_proj(d, view_idx);
#else
    let vp = mv::select_view_proj(d, 0u);
#endif
    var out: VertexOutput;
    out.clip_pos = vp * world_p;
    return out;
}

//#pass type=forward name=aux vs=vs_aux
@fragment
fn fs_aux(_in: VertexOutput) -> @location(0) vec4<f32> {
    return rg::retain_globals_additive(mat._Color);
}
"#;
    format!("{solid}{AUX}")
}
