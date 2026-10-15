//! Shader source audits for shared Camera360 equirectangular projection.

use super::*;

#[test]
fn camera360_projection_uses_shared_equirect_and_cubemap_storage_helpers() -> io::Result<()> {
    let src = source_file(manifest_dir().join("shaders/passes/post/camera360_equirect.wgsl"))?;

    for required in [
        "#import renderide::skybox::equirect as equirect",
        "#import renderide::skybox::cubemap_storage as cubemap_storage",
        "source_cube: texture_cube<f32>",
        "var dir = equirect::uv_to_dir(in.uv);",
        "params.rotation",
        "dir = -dir;",
        "cubemap_storage::sample_dir(dir, params.storage.x)",
    ] {
        assert!(
            src.contains(required),
            "camera360_equirect.wgsl must contain `{required}`"
        );
    }

    Ok(())
}

#[test]
fn cubemap_projection_material_reuses_shared_equirect_helper() -> io::Result<()> {
    let src = material_source("cubemapprojection.wgsl")?;

    assert!(src.contains("#import renderide::skybox::equirect as equirect"));
    assert!(src.contains("var dir = equirect::uv_to_dir(primary_uv);"));
    assert!(
        !src.contains("fn equirect_to_dir("),
        "CubemapProjection must not keep a private equirectangular direction implementation"
    );

    Ok(())
}
