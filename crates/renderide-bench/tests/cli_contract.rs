//! Process-level contract for `renderide-bench`: exit codes and `--json` stdout purity.
//!
//! Exit codes: 0 ok, 1 compare regression, 2 usage or input error, 3 run failure, 5 environment.

#![forbid(unsafe_code)]
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers outside #[test] fns; a failed spawn or bad JSON should abort the test"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_renderide-bench"))
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("spawn renderide-bench")
}

fn stdout_json(output: &Output) -> Value {
    let text = String::from_utf8(output.stdout.clone()).expect("utf8 stdout");
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("stdout is not one JSON doc: {err}\n{text}"))
}

#[test]
fn list_json_parses_and_covers_catalog() {
    let output = run(&["list", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["schema"], "renderide-bench-list/1");
    let cpu = doc["cpu"].as_array().expect("cpu array");
    let frame = doc["frame"].as_array().expect("frame array");
    assert_eq!(cpu.len(), 44, "{doc}");
    assert_eq!(frame.len(), 10, "{doc}");
    assert!(
        cpu.iter()
            .any(|v| v.as_str() == Some("culling/homogeneous_clip_aabb")),
        "{doc}"
    );
    assert!(
        cpu.iter()
            .any(|v| v.as_str() == Some("transparent_sort/master_1k")),
        "{doc}"
    );
}

#[test]
fn unknown_suite_exits_2_with_json_document() {
    let output = run(&["run", "--suite", "nope", "--json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["schema"], "renderide-bench/1");
    assert_eq!(doc["error"]["exit_code"], 2);
    assert_eq!(doc["error"]["kind"], "usage");
}

#[test]
fn clap_usage_error_with_json_prints_one_error_document() {
    let output = run(&["run", "--json", "--no-such-flag"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["error"]["kind"], "usage");
    assert!(
        doc["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("--no-such-flag")),
        "{doc}"
    );
}

#[test]
fn run_frame_with_missing_renderer_exits_5() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("out.json");
    let output = run(&[
        "run",
        "--suite",
        "frame",
        "--renderer",
        "/nonexistent/renderide-renderer",
        "--json",
        "--output",
        out.to_str().expect("utf8"),
    ]);
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["error"]["exit_code"], 5);
    assert_eq!(doc["error"]["kind"], "environment");
}

#[test]
fn run_cpu_from_criterion_dir_emits_full_catalog() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_full_criterion_tree(dir.path());
    let output = run(&[
        "run",
        "--suite",
        "cpu",
        "--json",
        "--criterion-dir",
        dir.path().to_str().expect("utf8"),
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["schema"], "renderide-bench/1");
    assert_eq!(doc["exit_code"], 0);
    let cpu = doc["cpu"].as_array().expect("cpu");
    assert_eq!(cpu.len(), 44, "{doc}");
    assert!(doc.get("frame").is_none() || doc["frame"].is_null());
}

#[test]
fn compare_fixtures_exit_codes_and_json_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let base = write_frame(d, "base.json", 2.0, 1);
    let same = write_frame(d, "same.json", 2.0, 2);
    let slow = write_frame(d, "slow.json", 2.5, 2);
    let tiny_base = write_frame(d, "tiny_base.json", 0.2, 2);
    let tiny_slow = write_frame(d, "tiny_slow.json", 0.24, 2);

    let output = run(&[
        "compare",
        "--base",
        base.to_str().unwrap(),
        "--head",
        same.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["schema"], "renderide-bench-compare/1");
    assert_eq!(doc["exit_code"], 0);
    assert_eq!(doc["abs_floor_ms"], 0.0);
    assert!(doc["decision_rule"].as_str().is_some_and(|s| !s.is_empty()));

    let output = run(&[
        "compare",
        "--base",
        base.to_str().unwrap(),
        "--head",
        slow.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["exit_code"], 1);
    assert_eq!(doc["rows"][0]["verdict"], "fail");

    let output = run(&[
        "compare",
        "--base",
        tiny_base.to_str().unwrap(),
        "--head",
        tiny_slow.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "floor off by default: {output:?}"
    );

    let output = run(&[
        "compare",
        "--base",
        tiny_base.to_str().unwrap(),
        "--head",
        tiny_slow.to_str().unwrap(),
        "--abs-floor-ms",
        "0.05",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["rows"][0]["verdict"], "pass");
}

#[test]
fn compare_cpu_criterion_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("base");
    let head = dir.path().join("head");
    write_one_estimate(&base, "culling", "homogeneous_clip_aabb", 1000.0);
    write_one_estimate(&head, "culling", "homogeneous_clip_aabb", 1200.0);
    let output = run(&[
        "compare",
        "--base",
        base.to_str().unwrap(),
        "--head",
        head.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let doc = stdout_json(&output);
    assert_eq!(doc["rows"][0]["suite"], "cpu");
    assert_eq!(doc["rows"][0]["verdict"], "fail");
}

fn write_frame(dir: &Path, name: &str, p95: f64, schema: u32) -> PathBuf {
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
            "cpu_frame_ms": {"p50": p95 * 0.5, "p95": p95, "p99": p95 * 1.1},
            "draw_calls": {"p50": 1.0, "p95": 1.0, "p99": 1.0},
            "upload_count": {"p50": 0.0, "p95": 0.0, "p99": 0.0},
            "upload_bytes": {"p50": 0.0, "p95": 0.0, "p99": 0.0},
            "gpu_passes_ms": null,
            "per_run_cpu_frame_ms_p95": [p95],
            "script": []
        }],
        "notes": []
    });
    if schema >= 2 {
        doc["runner"] = json!({
            "os": "linux", "cpu_model": "test",
            "icd": {"path": null, "source": "none", "var": null},
            "rayon_threads": 1, "rayon_threads_source": "default"
        });
    }
    let path = dir.join(name);
    fs::write(&path, serde_json::to_vec(&doc).expect("json")).expect("write");
    path
}

fn estimates_json(median_ns: f64) -> String {
    format!(
        r#"{{"mean":{{"confidence_interval":{{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0}},"point_estimate":1.5,"standard_error":0.1}},"median":{{"confidence_interval":{{"confidence_level":0.95,"lower_bound":{lo},"upper_bound":{hi}}},"point_estimate":{median},"standard_error":1.0}},"median_abs_dev":{{"confidence_interval":{{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0}},"point_estimate":1.5,"standard_error":0.1}},"slope":{{"confidence_interval":{{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0}},"point_estimate":1.5,"standard_error":0.1}},"std_dev":{{"confidence_interval":{{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0}},"point_estimate":1.5,"standard_error":0.1}}}}"#,
        median = median_ns,
        lo = median_ns * 0.99,
        hi = median_ns * 1.01
    )
}

fn write_one_estimate(root: &Path, group: &str, id: &str, median_ns: f64) {
    let dir = root.join(group).join(id).join("new");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("estimates.json"), estimates_json(median_ns)).expect("write");
}

fn write_full_criterion_tree(root: &Path) {
    // Keep in sync with crates/renderide-bench/src/catalog.rs CPU_BENCH_IDS.
    let ids = [
        ("transforms", "trs_and_degenerate_scale"),
        ("culling", "homogeneous_clip_aabb"),
        ("culling", "skinned_mesh_world_geometry"),
        ("hysteresis_classification", "decide_visibility"),
        ("hysteresis_history", "8192_quarter_occluded"),
        ("hysteresis_history", "8192_after_motion_all_visible"),
        ("hysteresis_history", "10000_stable"),
        ("hysteresis_history", "10000_moving"),
        ("hysteresis_history", "50000_stable"),
        ("hysteresis_history", "50000_moving"),
        ("hysteresis_history", "100000_stable"),
        ("hysteresis_history", "100000_moving"),
        ("hysteresis_history", "8192_all_visible"),
        ("hysteresis_history", "10000_all_visible_over_cap"),
        ("hysteresis_history", "50000_all_visible_over_cap"),
        ("hysteresis_history", "100000_all_visible_over_cap"),
        ("hysteresis_history", "676_of_1600_superset"),
        ("world_matrices", "forest_recompute"),
        ("world_matrices", "reverse_chain_recompute"),
        ("world_matrices", "motion_history_publish"),
        ("world_matrices", "motion_history_publish_100k_sparse"),
        ("world_matrices", "motion_history_publish_100k_dense"),
        ("material_uniforms", "property_change_pack"),
        ("material_uniforms", "one_material_repack"),
        ("bind_groups", "cache_hit"),
        ("bind_groups", "miss_rebuild"),
        ("keyword_variants", "cache_hit"),
        ("keyword_variants", "cache_miss"),
        ("keyword_variants", "compose"),
        ("custom_stem_lookup", "frame_reads"),
        ("shadow_churn", "static_repaint"),
        ("shadow_churn", "cold_plan"),
        ("shadow_churn", "static_repaint_revisions"),
        ("render_texture_double_buffer", "steady_swap"),
        ("camera_clear", "load_op_checksum"),
        ("buffer_pool", "free_list_reuse"),
        ("transparent_sort", "master_1k"),
        ("transparent_sort", "recompute_1k"),
        ("transparent_sort", "precomputed_1k"),
        ("transparent_sort", "snap_1k"),
        ("transparent_sort", "master_10k"),
        ("transparent_sort", "recompute_10k"),
        ("transparent_sort", "precomputed_10k"),
        ("transparent_sort", "snap_10k"),
    ];
    assert_eq!(ids.len(), 44);
    for (i, (group, id)) in ids.iter().enumerate() {
        write_one_estimate(root, group, id, 1000.0 + i as f64);
    }
}
