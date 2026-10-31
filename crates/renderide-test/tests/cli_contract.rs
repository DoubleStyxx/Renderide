//! CLI contract for `renderide-test run` and `list`.
//!
//! Image, capture, and unstable exits use `RENDERIDE_TEST_CLI_FIXTURE` on a
//! child built with the default-off `test-cli-fixture` feature. That hook is
//! documented on `cli::run` and is absent from the CI `check-suite` binary.
//! These tests never call `std::env::set_var`.

#![allow(
    clippy::expect_used,
    reason = "a failed spawn or unreadable JSON should abort the contract test"
)]

use std::process::Command;

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_renderide-test"));
    // Explicit loader variable, so these tests do not probe the host's ICD dirs.
    cmd.env(
        "VK_DRIVER_FILES",
        "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json",
    );
    cmd
}

fn output(cmd: &mut Command) -> std::process::Output {
    cmd.output().expect("spawn renderide-test")
}

/// Runs the harness with `test-cli-fixture` in its own target dir.
///
/// `cargo test --workspace --locked` does not pass that feature, so the binary
/// CI uses for `check-suite` stays free of the hook. This rebuild does not
/// replace `target/debug/renderide-test`.
fn hooked(args: &[&str], fixture: &str) -> std::process::Output {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(&root);
    cmd.args([
        "run",
        "--quiet",
        "--locked",
        "--offline",
        "-p",
        "renderide-test",
        "--features",
        "test-cli-fixture",
        "--target-dir",
    ]);
    cmd.arg(root.join("target/cli-fixture"));
    cmd.arg("--");
    cmd.args(args);
    cmd.env("RENDERIDE_TEST_CLI_FIXTURE", fixture);
    cmd.output().expect("cargo run with test-cli-fixture")
}

fn status(output: &std::process::Output) -> u8 {
    u8::try_from(output.status.code().unwrap_or(255)).unwrap_or(255)
}

#[test]
fn list_json_parses() {
    let output = output(bin().args(["list", "--json"]));
    assert_eq!(
        status(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    let scenes = value["scenes"].as_array().expect("scenes");
    assert!(scenes.iter().any(|name| name == "unlit_sphere"));
}

#[test]
fn missing_renderer_json_exits_5() {
    let output = output(bin().args([
        "run",
        "--json",
        "--renderer",
        "/nonexistent/renderide",
        "unlit_sphere",
    ]));
    assert_eq!(
        status(&output),
        5,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 5);
    assert_eq!(value["cases"][0]["exit_code"], 5);
}

#[test]
fn unknown_scene_exits_2() {
    let output = output(bin().args(["run", "not_a_scene"]));
    assert_eq!(
        status(&output),
        2,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not_a_scene"), "{stderr}");
}

#[test]
fn fixture_golden_missing_exits_4() {
    let output = hooked(&["run", "--json", "unlit_sphere"], "golden_missing");
    assert_eq!(
        status(&output),
        4,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 4);
}

#[test]
fn fixture_mismatch_exits_1() {
    let output = hooked(&["run", "--json", "unlit_sphere"], "mismatch");
    assert_eq!(
        status(&output),
        1,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 1);
    assert_eq!(value["cases"][0]["exit_code"], 1);
}

#[test]
fn fixture_capture_exits_3() {
    let output = hooked(&["run", "--json", "unlit_sphere"], "capture");
    assert_eq!(
        status(&output),
        3,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 3);
}

#[test]
fn fixture_unstable_exits_6() {
    let output = hooked(&["run", "--json", "unlit_sphere"], "unstable");
    assert_eq!(
        status(&output),
        6,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 6);
}

#[test]
fn bless_force_refuses_every_listed_quarantine_case() {
    for name in renderide_test::scene_dsl::bless_refused_scenes() {
        let output = output(bin().args(["run", "--json", "--bless", "--force", name]));
        assert_eq!(
            status(&output),
            0,
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("bless refused"), "{name}: {stderr}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
        assert_eq!(value["exit_code"], 0, "{name}");
        let skip = value["cases"][0]["bless_skip"].as_str().unwrap_or("");
        assert!(skip.contains("bless refused"), "{name}: {value}");
        assert!(value["cases"][0]["blessed"].is_null(), "{name}");
    }
}

#[test]
fn json_env_line_stays_on_stderr_and_fills_runner_icd() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_str().expect("utf8");
    let output = output(
        bin()
            .args([
                "run",
                "--json",
                "--renderer",
                "/nonexistent/renderide",
                "--output-root",
                root,
                "unlit_sphere",
            ])
            .env("VK_DRIVER_FILES", "/opt/icd/lvp_icd.x86_64.json")
            .env("VK_ICD_FILENAMES", "/opt/other.json")
            .env("RAYON_NUM_THREADS", "3"),
    );
    assert_eq!(
        status(&output),
        5,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("env:"),
        "stdout must stay one JSON document: {stdout}"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 5);
    assert_eq!(
        value["runner"]["icd"]["path"],
        "/opt/icd/lvp_icd.x86_64.json"
    );
    assert_eq!(value["runner"]["icd"]["source"], "env");
    assert_eq!(value["runner"]["icd"]["var"], "VK_DRIVER_FILES");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("icd=/opt/icd/lvp_icd.x86_64.json (env)"),
        "{stderr}"
    );
    assert!(stderr.contains("rayon_threads=3 (env)"), "{stderr}");
    assert!(stderr.contains("renderer="), "{stderr}");
    assert!(stderr.contains("nonexistent"), "{stderr}");
    assert!(!stderr.contains("dx12-warp"), "{stderr}");
    let case: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("unlit_sphere/report.json")).expect("report"),
    )
    .expect("case json");
    assert_eq!(case["runner"]["icd"]["source"], "env");
    assert_eq!(
        case["runner"]["icd"]["path"],
        "/opt/icd/lvp_icd.x86_64.json"
    );
}

#[test]
fn icd_none_skips_the_probe_and_dx12_warp_is_usage() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_renderide-test"));
    cmd.args([
        "run",
        "--json",
        "--icd",
        "none",
        "--renderer",
        "/nonexistent/renderide",
        "unlit_sphere",
    ]);
    cmd.env_remove("VK_DRIVER_FILES");
    cmd.env_remove("VK_ICD_FILENAMES");
    let none = output(&mut cmd);
    assert_eq!(
        status(&none),
        5,
        "{}",
        String::from_utf8_lossy(&none.stderr)
    );
    let stderr = String::from_utf8_lossy(&none.stderr);
    assert!(stderr.contains("icd=none (none)"), "{stderr}");
    assert!(!stderr.contains("no ICD"), "{stderr}");
    let value: serde_json::Value = serde_json::from_slice(&none.stdout).expect("json");
    assert!(
        value["cases"][0]["error"]
            .as_str()
            .unwrap_or("")
            .contains("renderer binary not found"),
        "{value}"
    );

    let warp = output(bin().args(["run", "--backend", "dx12-warp", "unlit_sphere"]));
    assert_eq!(
        status(&warp),
        2,
        "{}",
        String::from_utf8_lossy(&warp.stderr)
    );
    assert!(
        String::from_utf8_lossy(&warp.stderr).contains("dx12-warp"),
        "{}",
        String::from_utf8_lossy(&warp.stderr)
    );
}

#[test]
fn bless_refuses_to_overwrite_without_force() {
    let output = output(bin().args(["run", "--json", "--bless", "unlit_sphere"]));
    assert_eq!(
        status(&output),
        4,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--force"), "{stderr}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 4);
    assert!(value["cases"][0]["blessed"].is_null());
}
