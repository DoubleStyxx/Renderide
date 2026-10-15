//! Source audits for reflection-probe IBL and SH2 source ownership.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files_recursive(relative_dir: &str) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![manifest_dir().join(relative_dir)];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn source_file(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path)
}

#[test]
fn reflection_probes_do_not_resolve_active_skyboxes_for_ibl_or_sh2() -> io::Result<()> {
    let forbidden = [
        "resolve_skybox_material_ibl_source",
        "SkyboxIblSource::Analytic",
        "SkyboxIblSource::Equirect",
        "GpuSh2Source::SkyParams",
        "GpuSh2Source::EquirectTexture2D",
        "Sh2SourceKey::SkyParams",
        "Sh2SourceKey::EquirectTexture2D",
    ];
    let source_dirs = [
        "src/reflection_probes",
        "src/skybox/specular",
        "src/skybox/ibl_cache",
    ];
    let root = manifest_dir();

    for source_dir in source_dirs {
        for path in rust_files_recursive(source_dir)? {
            let src = source_file(&path)?;
            for pattern in forbidden {
                assert!(
                    !src.contains(pattern),
                    "{} must not contain direct active-skybox IBL/SH2 source path `{pattern}`",
                    path.strip_prefix(&root).unwrap_or(path.as_path()).display()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn removed_direct_skybox_projection_compute_shaders_stay_removed() {
    for shader in [
        "shaders/passes/compute/skybox_mip0_equirect_params.wgsl",
        "shaders/passes/compute/sh2_project_equirect.wgsl",
        "shaders/passes/compute/sh2_project_sky_params.wgsl",
    ] {
        assert!(
            !manifest_dir().join(shader).exists(),
            "{shader} should not be rebuilt into the embedded shader set"
        );
    }
}
