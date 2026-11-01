//! Port of `scripts/ci/bench_pr_assemble.py`.
use clap::Parser;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

const COMPARE_SCHEMA: &str = "renderide-bench-compare/1";
const DECISION_RULE: &str =
    "scripts/ci/frame_bench_gate.py (frame); renderide-bench compare (cpu, report-only)";

#[derive(Debug, Parser)]
pub struct Args {
    /// frame_bench_gate --json-out document.
    #[arg(long = "gate")]
    pub gate: PathBuf,
    /// Optional renderide-bench compare --json document.
    #[arg(long = "cpu-compare")]
    pub cpu_compare: Option<PathBuf>,
    /// bench-compare.json path.
    #[arg(long = "out")]
    pub out: PathBuf,
}

pub fn gate_status_to_verdict(status: &str) -> &'static str {
    if status.starts_with("FAIL") || status.starts_with("INCONCLUSIVE") {
        "fail"
    } else if status == "FLAG" {
        "flag"
    } else if status.starts_with("ok") {
        "pass"
    } else if status.contains("N/A") || status == "n/a" {
        "missing"
    } else {
        "pass"
    }
}

pub fn rows_from_gate(gate: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    let Some(arr) = gate.get("rows").and_then(|v| v.as_array()) else {
        return rows;
    };
    for r in arr {
        let p_value = r
            .get("p_value")
            .filter(|v| !v.is_null())
            .cloned()
            .or_else(|| r.get("confirm_p").cloned());
        rows.push(json!({
            "suite": "frame",
            "name": r.get("scenario"),
            "metric": "cpu_frame_ms.p95",
            "base": r.get("base"),
            "head": r.get("head"),
            "delta_pct": r.get("delta_pct"),
            "p_value": p_value,
            "verdict": gate_status_to_verdict(r.get("status").and_then(|v| v.as_str()).unwrap_or("ok")),
            "gate_status": r.get("status"),
            "base_drift_pct": r.get("base_drift_pct"),
            "base_drift_pct_round1": r.get("base_drift_pct_round1"),
            "base_drift_rounds": r.get("base_drift_rounds"),
            "confirm_round": r.get("confirm_round"),
            "deciding_round": r.get("deciding_round"),
        }));
    }
    rows
}

pub fn rows_from_cpu(compare: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    let Some(arr) = compare.get("rows").and_then(|v| v.as_array()) else {
        return rows;
    };
    for r in arr {
        if r.get("suite").and_then(|v| v.as_str()) != Some("cpu") {
            continue;
        }
        rows.push(json!({
            "suite": "cpu",
            "name": r.get("name"),
            "metric": r.get("metric").cloned().unwrap_or_else(|| json!("median_ns")),
            "base": r.get("base"),
            "head": r.get("head"),
            "delta_pct": r.get("delta_pct"),
            "p_value": r.get("p_value"),
            "verdict": r.get("verdict").cloned().unwrap_or_else(|| json!("pass")),
        }));
    }
    rows
}

pub fn run(args: Args) -> i32 {
    if !args.gate.is_file() {
        eprintln!(
            "bench_pr_assemble: missing gate input {}",
            args.gate.display()
        );
        return 2;
    }
    let gate: Value = match fs::read_to_string(&args.gate)
        .and_then(|t| serde_json::from_str(&t).map_err(std::io::Error::other))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bench_pr_assemble: invalid gate JSON: {e}");
            return 2;
        }
    };
    if gate.get("schema").and_then(|v| v.as_str()) != Some("frame-bench-gate/1") {
        eprintln!(
            "bench_pr_assemble: unexpected gate schema {:?}",
            gate.get("schema")
        );
        return 2;
    }
    let mut rows = rows_from_gate(&gate);
    if let Some(cpu_path) = &args.cpu_compare
        && cpu_path.is_file()
    {
        match fs::read_to_string(cpu_path)
            .and_then(|t| serde_json::from_str::<Value>(&t).map_err(std::io::Error::other))
        {
            Ok(cpu) => rows.extend(rows_from_cpu(&cpu)),
            Err(e) => {
                eprintln!("bench_pr_assemble: invalid cpu-compare JSON: {e}");
                return 2;
            }
        }
    }
    let exit_code = gate.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0);
    let payload = json!({
        "schema": COMPARE_SCHEMA,
        "fail_threshold": gate.get("fail_threshold").cloned().unwrap_or_else(|| json!(10.0)),
        "flag_threshold": gate.get("flag_threshold").cloned().unwrap_or_else(|| json!(5.0)),
        "abs_floor_ms": gate.get("abs_floor_ms").cloned().unwrap_or_else(|| json!(0.0)),
        "decision_rule": DECISION_RULE,
        "gate_mode": gate.get("mode"),
        "rows": rows,
        "exit_code": exit_code,
    });
    if let Some(parent) = args.out.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
    let mut text = match serde_json::to_string_pretty(&payload) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bench_pr_assemble: {e}");
            return 2;
        }
    };
    text.push('\n');
    if let Err(e) = fs::write(&args.out, text) {
        eprintln!("bench_pr_assemble: {e}");
        return 2;
    }
    0
}
