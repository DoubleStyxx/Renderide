//! Non-blocking shadow compare of Rust vs Python gate outputs.
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::bench_gate;

#[derive(Debug, Parser)]
pub struct Args {
    /// Existing Python-authored gate.json (authoritative).
    #[arg(long = "python-json")]
    pub python_json: PathBuf,
    /// Where to write the Rust gate.json for comparison.
    #[arg(long = "rust-json")]
    pub rust_json: PathBuf,
    /// If set, also re-run `bench-gate` with the remaining args before diffing.
    /// When omitted, `--rust-json` must already exist.
    #[arg(long = "rerun-rust")]
    pub rerun_rust: bool,
    /// Forwarded to `bench-gate` when `--rerun-rust` is set (same flags as frame_bench_gate.py).
    #[arg(long = "base")]
    pub base: Vec<PathBuf>,
    #[arg(long = "head")]
    pub head: Vec<PathBuf>,
    #[arg(long = "confirm-base")]
    pub confirm_base: Vec<PathBuf>,
    #[arg(long = "confirm-head")]
    pub confirm_head: Vec<PathBuf>,
    #[arg(long = "flag-threshold", default_value_t = 5.0)]
    pub flag_threshold: f64,
    #[arg(long = "fail-threshold", default_value_t = 10.0)]
    pub fail_threshold: f64,
    #[arg(long = "mode", default_value = "first")]
    pub mode: String,
    #[arg(long = "fail-closed-drift")]
    pub fail_closed_drift: bool,
    #[arg(long = "confirm-round", default_value_t = 1)]
    pub confirm_round: i32,
    #[arg(long = "alpha", default_value_t = bench_gate::DEFAULT_ALPHA)]
    pub alpha: f64,
    #[arg(long = "abs-floor-ms", default_value_t = 0.0)]
    pub abs_floor_ms: f64,
    #[arg(long = "head-missing")]
    pub head_missing: Vec<String>,
    /// Optional merged confirm gates for merge-confirm shadow (repeatable).
    #[arg(long = "merge-gate")]
    pub merge_gate: Vec<PathBuf>,
    /// Newline-separated list of gate.json paths Python actually merged (preferred over existence checks).
    #[arg(long = "merge-gates-list")]
    pub merge_gates_list: Option<PathBuf>,
    #[arg(long = "after-rerun")]
    pub after_rerun: bool,
    /// Compare mode: `gate` (default) or `merge`.
    #[arg(long = "compare", default_value = "gate")]
    pub compare: String,
}

pub fn run(args: Args) -> i32 {
    let mut args = args;
    if let Some(list_path) = args.merge_gates_list.clone() {
        match load_gates_list(&list_path) {
            Ok(extra) => args.merge_gate.extend(extra),
            Err(e) => {
                eprintln!("xtask shadow-diff: merge-gates-list: {e}");
                return 1;
            }
        }
    }
    if args.rerun_rust {
        if args.compare == "merge" {
            let rc = crate::bench_pr_merge_confirm::run(crate::bench_pr_merge_confirm::Args {
                gates: args.merge_gate.clone(),
                out: args.rust_json.clone(),
                after_rerun: args.after_rerun,
                gate1: None,
                gate2: None,
            });
            if rc != 0 {
                eprintln!("xtask shadow-diff: rust merge-confirm exited {rc}");
                // Non-blocking informational: still exit 0 from workflow POV via wrapper;
                // here return 1 so a dedicated check can warn.
                return 1;
            }
        } else {
            let gate_args = bench_gate::Args {
                base: args.base.clone(),
                head: args.head.clone(),
                confirm_base: args.confirm_base.clone(),
                confirm_head: args.confirm_head.clone(),
                flag_threshold: args.flag_threshold,
                fail_threshold: args.fail_threshold,
                legacy_threshold: None,
                mode: args.mode.clone(),
                fail_closed_drift: args.fail_closed_drift,
                confirm_round: args.confirm_round,
                drift_out: None,
                alpha: args.alpha,
                abs_floor_ms: args.abs_floor_ms,
                head_missing: args.head_missing.clone(),
                title: "Frame bench gate (xtask shadow)".into(),
                suspects_out: None,
                json_out: Some(args.rust_json.clone()),
            };
            let rc = bench_gate::run(gate_args);
            // Gate exit 0/1 are both valid; only 2 is hard error.
            if rc == 2 {
                eprintln!("xtask shadow-diff: rust bench-gate input error");
                return 1;
            }
        }
    }

    let py = match load_json(&args.python_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("xtask shadow-diff: python json: {e}");
            return 1;
        }
    };
    let rs = match load_json(&args.rust_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("xtask shadow-diff: rust json: {e}");
            return 1;
        }
    };

    let mut mismatches = Vec::new();
    compare_field(&py, &rs, "exit_code", &mut mismatches);
    compare_field(&py, &rs, "gate_action", &mut mismatches);
    compare_field(&py, &rs, "verdict_reason", &mut mismatches);

    let py_rows = row_status_map(&py);
    let rs_rows = row_status_map(&rs);
    let names: std::collections::BTreeSet<_> =
        py_rows.keys().chain(rs_rows.keys()).cloned().collect();
    for name in names {
        let a = py_rows
            .get(&name)
            .map(String::as_str)
            .unwrap_or("<missing>");
        let b = rs_rows
            .get(&name)
            .map(String::as_str)
            .unwrap_or("<missing>");
        if a != b {
            mismatches.push(format!("row {name}: python={a:?} rust={b:?}"));
        }
    }

    if mismatches.is_empty() {
        println!("xtask shadow-diff: OK (Rust matches Python gate verdict/statuses)");
        0
    } else {
        eprintln!("xtask shadow-diff: MISMATCH (Python remains authoritative)");
        for m in &mismatches {
            eprintln!("  - {m}");
        }
        // Exit 1 so a dedicated informational CI check can flag mismatch without
        // failing the Python gate step (workflow should use continue-on-error).
        1
    }
}

fn load_gates_list(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let paths: Vec<PathBuf> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();
    if paths.is_empty() {
        return Err("merge-gates-list is empty".into());
    }
    Ok(paths)
}

fn load_json(path: &PathBuf) -> Result<Value, String> {
    let t = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&t).map_err(|e| e.to_string())
}

fn compare_field(py: &Value, rs: &Value, key: &str, mismatches: &mut Vec<String>) {
    let a = py.get(key);
    let b = rs.get(key);
    // gate_action / verdict_reason may be absent on raw first-pass gate.json.
    if a.is_none() && b.is_none() {
        return;
    }
    if a != b {
        mismatches.push(format!("{key}: python={a:?} rust={b:?}"));
    }
}

fn row_status_map(g: &Value) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    if let Some(rows) = g.get("rows").and_then(|v| v.as_array()) {
        for r in rows {
            if let Some(name) = r.get("scenario").and_then(|v| v.as_str()) {
                let st = r
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                m.insert(name.to_string(), st);
            }
        }
    }
    m
}

/// Helper used by tests: compare two gate.json values.
#[expect(dead_code, reason = "used by parity tests and future callers")]
pub fn diff_values(python: &Value, rust: &Value) -> Vec<String> {
    let mut mismatches = Vec::new();
    compare_field(python, rust, "exit_code", &mut mismatches);
    compare_field(python, rust, "gate_action", &mut mismatches);
    let py_rows = row_status_map(python);
    let rs_rows = row_status_map(rust);
    let names: std::collections::BTreeSet<_> =
        py_rows.keys().chain(rs_rows.keys()).cloned().collect();
    for name in names {
        let a = py_rows
            .get(&name)
            .map(String::as_str)
            .unwrap_or("<missing>");
        let b = rs_rows
            .get(&name)
            .map(String::as_str)
            .unwrap_or("<missing>");
        if a != b {
            mismatches.push(format!("row {name}: python={a:?} rust={b:?}"));
        }
    }
    mismatches
}
