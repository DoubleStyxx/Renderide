//! Exercises the build-time OpenXR loader copy helpers under `cargo test` by path-including the
//! build-script modules.

#![allow(
    dead_code,
    reason = "the path-included build-script modules expose helpers outside these focused tests"
)]
#![allow(
    clippy::print_stdout,
    reason = "the included build-script module emits Cargo directives through println!"
)]

use std::fs;
use std::path::{Path, PathBuf};

#[path = "../build_support/artifacts.rs"]
mod artifacts;
#[path = "../build_support/openxr_loader.rs"]
mod openxr_loader;

#[test]
fn artifact_dir_from_out_dir_handles_debug_dev_fast_and_cross_target() {
    for (out_dir, expected) in [
        ("target/debug/build/renderide-abc/out", "target/debug"),
        ("target/dev-fast/build/renderide-abc/out", "target/dev-fast"),
        (
            "target/x86_64-pc-windows-msvc/dev-fast/build/renderide-abc/out",
            "target/x86_64-pc-windows-msvc/dev-fast",
        ),
    ] {
        assert_eq!(
            artifacts::artifact_dir_from_out_dir(Path::new(out_dir)),
            Some(PathBuf::from(expected))
        );
    }
}

#[test]
fn loader_destination_uses_out_dir_profile_for_windows_and_macos() {
    let out_dir = Path::new("target/dev-fast/build/renderide-abc/out");

    assert_eq!(
        openxr_loader::openxr_loader_destination_path(out_dir, "windows"),
        Some(PathBuf::from("target/dev-fast/openxr_loader.dll"))
    );
    assert_eq!(
        openxr_loader::openxr_loader_destination_path(out_dir, "macos"),
        Some(PathBuf::from("target/dev-fast/libopenxr_loader.dylib"))
    );
    assert_eq!(
        openxr_loader::openxr_loader_destination_path(out_dir, "linux"),
        None
    );
}

#[test]
fn windows_loader_copy_uses_out_dir_profile_not_profile_env()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    let manifest_dir = root.join("crates/renderide");
    fs::create_dir_all(&manifest_dir)?;

    let source_dir = root.join("third_party/openxr_loader/openxr_loader_windows-1.1.58/x64");
    fs::create_dir_all(&source_dir)?;
    fs::write(source_dir.join("openxr_loader.dll"), b"windows loader")?;

    let out_dir = root.join("target/dev-fast/build/renderide-abc/out");
    fs::create_dir_all(&out_dir)?;

    assert!(openxr_loader::copy_vendored_openxr_loader_for_target(
        &manifest_dir,
        &out_dir,
        "windows",
        Some("x86_64")
    ));
    assert_eq!(
        fs::read(root.join("target/dev-fast/openxr_loader.dll"))?,
        b"windows loader"
    );
    assert!(!root.join("target/debug/openxr_loader.dll").exists());

    Ok(())
}

#[test]
fn macos_loader_copy_uses_out_dir_profile() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    let manifest_dir = root.join("crates/renderide");
    fs::create_dir_all(&manifest_dir)?;

    let source_dir = root.join("third_party/openxr_loader/openxr_loader_macos-1.1.58");
    fs::create_dir_all(&source_dir)?;
    fs::write(source_dir.join("libopenxr_loader.dylib"), b"macos loader")?;

    let out_dir = root.join("target/dev-fast/build/renderide-abc/out");
    fs::create_dir_all(&out_dir)?;

    assert!(openxr_loader::copy_vendored_openxr_loader_for_target(
        &manifest_dir,
        &out_dir,
        "macos",
        None
    ));
    assert_eq!(
        fs::read(root.join("target/dev-fast/libopenxr_loader.dylib"))?,
        b"macos loader"
    );

    Ok(())
}
