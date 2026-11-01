//! Port of `scripts/ci/bench_pr_force_unstable.py`.
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Args {
    /// Path to gate.json to stamp.
    pub gate: PathBuf,
}

pub fn run(args: Args) -> i32 {
    let text = match fs::read_to_string(&args.gate) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("bench_pr_force_unstable: {e}");
            return 2;
        }
    };
    let mut g: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bench_pr_force_unstable: {e}");
            return 2;
        }
    };
    let prev_reason = g
        .get("verdict_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    g["gate_action"] = Value::String("unstable".into());
    g["verdict_reason"] = Value::String(format!(
        "{prev_reason} (forced unstable after full-gate rerun)"
    ));
    if let Some(rows) = g.get_mut("rows").and_then(|v| v.as_array_mut()) {
        for r in rows {
            let st = r
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if st.starts_with("INCONCLUSIVE") || st.starts_with("FAIL (inconclusive: base drift)") {
                r["status"] = Value::String("FAIL (inconclusive: unstable runner)".into());
            }
        }
    }
    g["exit_code"] = Value::from(1);
    let mut out = match serde_json::to_string_pretty(&g) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bench_pr_force_unstable: {e}");
            return 2;
        }
    };
    out.push('\n');
    if let Err(e) = fs::write(&args.gate, out) {
        eprintln!("bench_pr_force_unstable: {e}");
        return 2;
    }
    println!(
        "forced unstable on {}: {}",
        args.gate.display(),
        g.get("verdict_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    );
    0
}
