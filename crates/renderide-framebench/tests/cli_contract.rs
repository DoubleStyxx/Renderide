//! Process-level contract for `renderide-framebench`: exit codes and `--json` stdout purity.
//!
//! Exit codes: 0 ok, 1 compare regression, 2 usage or input error, 3 run failure, 5 environment.

#![forbid(unsafe_code)]
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers outside #[test] fns; a failed spawn or bad JSON should abort the test"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_renderide-framebench"))
}

/// Runs `renderide-framebench` with logs kept out of the checkout.
fn run(args: &[&str], logs: &Path) -> Output {
    bin()
        .args(args)
        .env("RENDERIDE_LOGS_ROOT", logs)
        .output()
        .expect("spawn renderide-framebench")
}

fn stdout_json(output: &Output) -> Value {
    let text = String::from_utf8(output.stdout.clone()).expect("utf8 stdout");
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("stdout is not one JSON doc: {err}\n{text}"))
}

fn assert_error_document(doc: &Value, code: i32, kind: &str) {
    assert_eq!(doc["schema"], 2, "{doc}");
    assert_eq!(doc["error"]["exit_code"], code, "{doc}");
    assert_eq!(doc["error"]["kind"], kind, "{doc}");
    assert!(
        doc["error"]["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty()),
        "{doc}"
    );
    assert!(
        doc["runner"]["cpu_model"]
            .as_str()
            .is_some_and(|m| !m.is_empty()),
        "{doc}"
    );
    assert!(doc["runner"]["icd"]["source"].is_string(), "{doc}");
    assert!(
        doc["runner"]["rayon_threads"]
            .as_u64()
            .is_some_and(|n| n >= 1),
        "{doc}"
    );
    assert!(doc.get("scenarios").is_none(), "{doc}");
}

#[test]
fn run_with_missing_renderer_exits_5_with_one_json_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("fb.json");
    let output = run(
        &[
            "run",
            "--renderer",
            "/nonexistent/renderide-renderer",
            "--json",
            "--output",
            out.to_str().expect("utf8"),
        ],
        dir.path(),
    );
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert_error_document(&stdout_json(&output), 5, "environment");
    assert!(!out.exists(), "no report is written on failure");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("env: os="),
        "env line goes to stderr: {stderr}"
    );
}

#[test]
fn run_with_unknown_scenario_exits_2_with_one_json_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = run(
        &[
            "run",
            "--renderer",
            "/nonexistent/renderide-renderer",
            "--scenario",
            "no_such_scenario",
            "--json",
        ],
        dir.path(),
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_error_document(&stdout_json(&output), 2, "usage");
}

#[test]
fn run_with_bad_resolution_or_zero_runs_exits_2() {
    let dir = tempfile::tempdir().expect("tempdir");
    for args in [
        &["run", "--resolution", "256", "--json"][..],
        &["run", "--runs", "0", "--json"][..],
    ] {
        let output = run(args, dir.path());
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert_error_document(&stdout_json(&output), 2, "usage");
    }
}

#[test]
fn run_without_json_keeps_stdout_free_of_error_documents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = run(
        &["run", "--renderer", "/nonexistent/renderide-renderer"],
        dir.path(),
    );
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
}

/// A renderer that exits at once never completes the handshake (15 s harness timeout): the
/// binary exists and spawns, so this is a run failure, not an environment error.
#[cfg(unix)]
#[test]
fn renderer_that_never_handshakes_exits_3() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    // A script, not /bin/true: macOS has no /bin/true (it is /usr/bin/true).
    let renderer = dir.path().join("exit-at-once.sh");
    std::fs::write(&renderer, "#!/bin/sh\nexit 0\n").expect("write renderer stub");
    std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o755))
        .expect("chmod renderer stub");
    let renderer = renderer.to_str().expect("utf8 tempdir path");
    let output = run(
        &[
            "run",
            "--renderer",
            renderer,
            "--scenario",
            "unlit_sphere",
            "--timeout-seconds",
            "1",
            "--json",
        ],
        dir.path(),
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert_error_document(&stdout_json(&output), 3, "run_failure");
}

fn report(schema: u32, p95: f64) -> Value {
    let pct = json!({"p50": p95 * 0.5, "p95": p95, "p99": p95 * 1.1});
    let count = json!({"p50": 1.0, "p95": 1.0, "p99": 1.0});
    let mut doc = json!({
        "schema": schema,
        "scale": "relative",
        "git_sha": "abc",
        "renderer_commit": "abc",
        "mode": "headless",
        "resolution": [256, 256],
        "warmup_frames": 10,
        "measured_frames": 20,
        "runs": 1,
        "adapter": {"name": "llvmpipe", "backend": "Vulkan", "device_type": "Cpu",
                    "driver": "llvmpipe", "driver_info": "Mesa"},
        "timestamp_query": false,
        "control_api": {"implemented": false, "transport": "localhost-tcp-jsonrpc", "methods": []},
        "scenarios": [{
            "name": "unlit_sphere",
            "cpu_frame_ms": pct,
            "draw_calls": count,
            "upload_count": count,
            "upload_bytes": count,
            "gpu_passes_ms": null,
            "per_run_cpu_frame_ms_p95": [p95],
            "script": []
        }],
        "notes": []
    });
    if schema >= 2 {
        doc["runner"] = json!({
            "os": "linux", "cpu_model": "AMD EPYC 7763 64-Core Processor",
            "icd": {"path": "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json", "source": "env",
                    "var": "VK_DRIVER_FILES"},
            "rayon_threads": 2, "rayon_threads_source": "env"
        });
    }
    doc
}

fn write(dir: &Path, name: &str, doc: &Value) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_vec(doc).expect("json")).expect("write");
    path
}

fn compare(base: &Path, head: &Path, logs: &Path) -> Output {
    run(
        &[
            "compare",
            "--baseline",
            base.to_str().expect("utf8"),
            "--candidate",
            head.to_str().expect("utf8"),
        ],
        logs,
    )
}

#[test]
fn compare_exit_codes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let base1 = write(d, "base1.json", &report(1, 2.0));
    let same2 = write(d, "same2.json", &report(2, 2.0));
    let slow2 = write(d, "slow2.json", &report(2, 2.5));
    let tiny_base = write(d, "tiny_base.json", &report(2, 0.2));
    let tiny_slow = write(d, "tiny_slow.json", &report(2, 0.24));
    let future = write(d, "future.json", &report(3, 2.0));

    assert_eq!(
        compare(&base1, &same2, d).status.code(),
        Some(0),
        "schema 1 vs 2 passes"
    );
    assert_eq!(
        compare(&base1, &slow2, d).status.code(),
        Some(1),
        "+25% fails"
    );
    assert_eq!(
        compare(&tiny_base, &tiny_slow, d).status.code(),
        Some(0),
        "+20% but 0.04 ms is under the 0.05 ms floor"
    );
    assert_eq!(
        compare(&base1, &future, d).status.code(),
        Some(2),
        "schema 3"
    );
    assert_eq!(
        compare(&base1, &d.join("missing.json"), d).status.code(),
        Some(2),
        "missing file"
    );
}
