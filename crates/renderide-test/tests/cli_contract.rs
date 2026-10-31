//! CLI contract for `renderide-test run` and `list`.
//!
//! Image, capture, and unstable exits use `RENDERIDE_TEST_CLI_FIXTURE` on the
//! child process. That hook exists only in debug builds and is documented on
//! `cli::run`. These tests never call `std::env::set_var`.

#![allow(
    clippy::expect_used,
    reason = "a failed spawn or unreadable JSON should abort the contract test"
)]

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_renderide-test"))
}

fn output(cmd: &mut Command) -> std::process::Output {
    cmd.output().expect("spawn renderide-test")
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
    let output = output(
        bin()
            .args(["run", "--json", "unlit_sphere"])
            .env("RENDERIDE_TEST_CLI_FIXTURE", "golden_missing"),
    );
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
    let output = output(
        bin()
            .args(["run", "--json", "unlit_sphere"])
            .env("RENDERIDE_TEST_CLI_FIXTURE", "mismatch"),
    );
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
    let output = output(
        bin()
            .args(["run", "--json", "unlit_sphere"])
            .env("RENDERIDE_TEST_CLI_FIXTURE", "capture"),
    );
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
    let output = output(
        bin()
            .args(["run", "--json", "unlit_sphere"])
            .env("RENDERIDE_TEST_CLI_FIXTURE", "unstable"),
    );
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
fn bless_refuses_a_quarantined_case() {
    let output = output(bin().args([
        "run",
        "--json",
        "--bless",
        "--force",
        "pbs_lit_material_matrix",
    ]));
    assert_eq!(
        status(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bless refused"), "{stderr}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["exit_code"], 0);
    assert!(
        value["cases"][0]["bless_skip"]
            .as_str()
            .unwrap_or("")
            .contains("#216"),
        "{value}"
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
