//! Port of `scripts/ci/bench_pr_echo_verdict.py`.
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::bench_gate;

#[derive(Debug, Parser)]
pub struct Args {
    /// Path to gate.json.
    pub gate: PathBuf,
}

pub fn run(args: Args) -> i32 {
    let text = match fs::read_to_string(&args.gate) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("bench_pr_echo_verdict: {e}");
            return 2;
        }
    };
    let mut g: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bench_pr_echo_verdict: {e}");
            return 2;
        }
    };
    let mut action = g
        .get("gate_action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut reason = g
        .get("verdict_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if action.is_empty() {
        let exit_code = g.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(1);
        let mode = g.get("mode").and_then(|v| v.as_str()).unwrap_or("");
        if exit_code == 0 {
            action = "pass".into();
            if reason.is_empty() {
                reason = if mode == "first" {
                    "first pass clear (no confirm)".into()
                } else {
                    "gate clear".into()
                };
            }
        } else {
            action = "fail".into();
            if reason.is_empty() {
                reason = if mode == "first" {
                    "first pass FLAG with confirm_rounds=0".into()
                } else {
                    "gate fail".into()
                };
            }
        }
        g["gate_action"] = Value::String(action.clone());
        g["verdict_reason"] = Value::String(reason.clone());
        let mut out = match serde_json::to_string_pretty(&g) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("bench_pr_echo_verdict: {e}");
                return 2;
            }
        };
        out.push('\n');
        if let Err(e) = fs::write(&args.gate, out) {
            eprintln!("bench_pr_echo_verdict: {e}");
            return 2;
        }
    }
    println!("{}", bench_gate::format_summary_verdict(&action, &reason));
    0
}
