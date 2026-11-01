//! Port of `scripts/ci/bench_pr_merge_confirm.py`.
use clap::Parser;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::bench_gate::{self, DecideResult};

#[derive(Debug, Parser)]
pub struct Args {
    /// Confirm-round gate.json in order (repeatable; round 1 first).
    #[arg(long = "gate", required = true)]
    pub gates: Vec<PathBuf>,
    #[arg(long = "out", required = true)]
    pub out: PathBuf,
    /// This attempt follows a full-gate rerun; recurring drift/disagreement → unstable.
    #[arg(long = "after-rerun")]
    pub after_rerun: bool,
    /// Legacy pair flag.
    #[arg(long = "gate1", hide = true)]
    pub gate1: Option<PathBuf>,
    #[arg(long = "gate2", hide = true)]
    pub gate2: Option<PathBuf>,
}

pub fn merge_confirm_gates(gates: &[Value], after_rerun: bool) -> Value {
    assert!(!gates.is_empty());
    if gates.len() == 1 {
        let mut out = gates[0].clone();
        return annotate_action(&mut out, None, after_rerun);
    }

    let mut merged_by: BTreeMap<String, Value> = BTreeMap::new();
    let mut round_kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut drift_rounds: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for (gi, g) in gates.iter().enumerate() {
        let round_no = g
            .get("confirm_round")
            .and_then(|v| v.as_i64())
            .unwrap_or((gi + 1) as i64) as i32;
        let rows = g
            .get("rows")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let names_in_g: std::collections::BTreeSet<String> = rows
            .iter()
            .filter_map(|r| {
                r.get("scenario")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();
        for r in &rows {
            let name = r
                .get("scenario")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let status = r
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let kind = bench_gate::confirm_status_kind(&status).to_string();
            let rounds = drift_rounds.entry(name.clone()).or_default();
            if let Some(bdp) = r.get("base_drift_pct").filter(|v| !v.is_null()) {
                rounds.push(json!({"round": round_no, "base_drift_pct": bdp}));
            }
            let kinds = round_kinds.entry(name.clone()).or_default();
            if gi == 0 {
                kinds.push(kind);
                let mut row = r.clone();
                if !rounds.is_empty() {
                    row["base_drift_rounds"] = Value::Array(rounds.clone());
                }
                merged_by.insert(name, row);
            } else {
                let prev = merged_by.get(&name).cloned();
                let prev_status = prev
                    .as_ref()
                    .and_then(|p| p.get("status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if prev.is_none()
                    || bench_gate::is_inconclusive_drift(&prev_status)
                    || names_in_g.contains(&name)
                {
                    while kinds.len() < gi {
                        kinds.push("pass".into());
                    }
                    if kinds.len() == gi {
                        kinds.push(kind);
                    } else {
                        kinds[gi] = kind;
                    }
                    let mut row = r.clone();
                    if !rounds.is_empty() {
                        row["base_drift_rounds"] = Value::Array(rounds.clone());
                    }
                    if let Some(prev_v) = &prev
                        && prev_v.get("base_drift_pct").is_some_and(|v| !v.is_null())
                    {
                        let r1 = prev_v
                            .get("base_drift_pct_round1")
                            .cloned()
                            .or_else(|| prev_v.get("base_drift_pct").cloned())
                            .unwrap_or(Value::Null);
                        row["base_drift_pct_round1"] = r1;
                    }
                    merged_by.insert(name, row);
                }
            }
        }
    }

    let mut out = gates[0].clone();
    out["rows"] = Value::Array(merged_by.into_values().collect());
    let last_round = gates
        .last()
        .and_then(|g| g.get("confirm_round"))
        .and_then(|v| v.as_i64())
        .unwrap_or(gates.len() as i64);
    out["confirm_round"] = json!(last_round);
    annotate_action(&mut out, Some(&round_kinds), after_rerun)
}

fn annotate_action(
    out: &mut Value,
    round_kinds: Option<&BTreeMap<String, Vec<String>>>,
    after_rerun: bool,
) -> Value {
    let owned: BTreeMap<String, Vec<String>>;
    let round_kinds = if let Some(rk) = round_kinds {
        rk
    } else {
        let mut tmp = BTreeMap::new();
        if let Some(rows) = out.get("rows").and_then(|v| v.as_array()) {
            for r in rows {
                let name = r
                    .get("scenario")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let st = r.get("status").and_then(|v| v.as_str()).unwrap_or("");
                tmp.insert(name, vec![bench_gate::confirm_status_kind(st).to_string()]);
            }
        }
        owned = tmp;
        &owned
    };

    let mut scenario_results: BTreeMap<String, DecideResult> = BTreeMap::new();
    for (name, kinds) in round_kinds {
        let kind_refs: Vec<&str> = kinds.iter().map(String::as_str).collect();
        scenario_results.insert(
            name.clone(),
            bench_gate::post_drift_policy(&kind_refs, after_rerun),
        );
    }

    let priority = |v: &str| -> i32 {
        match v {
            "unstable" => 5,
            "fail" => 4,
            "rerun" => 3,
            "need_confirm" => 2,
            "pass" => 1,
            _ => 0,
        }
    };
    let mut best = DecideResult::new("pass", "all scenarios clear");
    for (name, res) in &scenario_results {
        if priority(&res.verdict) > priority(&best.verdict) {
            best = DecideResult::with_round(
                res.verdict.clone(),
                format!("{name}: {}", res.reason),
                res.deciding_round,
            );
        }
    }

    if best.verdict == "unstable"
        && let Some(rows) = out.get_mut("rows").and_then(|v| v.as_array_mut())
    {
        for r in rows.iter_mut() {
            let name = r
                .get("scenario")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if scenario_results
                .get(&name)
                .is_some_and(|res| res.verdict == "unstable")
            {
                r["status"] = Value::String("FAIL (inconclusive: unstable runner)".into());
                r["deciding_round"] = Value::Null;
            }
        }
    }

    out["gate_action"] = Value::String(best.verdict.clone());
    out["verdict_reason"] = Value::String(best.reason.clone());
    if let Some(dr) = best.deciding_round {
        out["deciding_round"] = json!(dr);
    }

    out["exit_code"] = Value::from(match best.verdict.as_str() {
        "pass" => 0,
        "fail" | "unstable" => 1,
        "rerun" => 3,
        "need_confirm" => 5,
        _ => 1,
    });

    let mut hit = Vec::new();
    if let Some(rows) = out.get("rows").and_then(|v| v.as_array()) {
        for r in rows {
            let st = r.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if (st.starts_with("FAIL") || st.starts_with("INCONCLUSIVE") || st.starts_with("FLAG"))
                && let Some(s) = r.get("scenario").and_then(|v| v.as_str())
            {
                hit.push(Value::String(s.into()));
            }
        }
    }
    out["hit"] = Value::Array(hit);
    out.clone()
}

pub fn run(args: Args) -> i32 {
    let paths = if let (Some(g1), Some(g2)) = (&args.gate1, &args.gate2) {
        vec![g1.clone(), g2.clone()]
    } else {
        args.gates.clone()
    };
    if paths.is_empty() {
        eprintln!("bench_pr_merge_confirm: need at least one --gate");
        return 2;
    }
    let mut gates = Vec::new();
    for p in &paths {
        match fs::read_to_string(p)
            .and_then(|t| serde_json::from_str::<Value>(&t).map_err(std::io::Error::other))
        {
            Ok(v) => gates.push(v),
            Err(e) => {
                eprintln!("bench_pr_merge_confirm: {e}");
                return 2;
            }
        }
    }
    let merged = merge_confirm_gates(&gates, args.after_rerun);
    if let Some(parent) = args.out.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
    let mut text = match serde_json::to_string_pretty(&merged) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bench_pr_merge_confirm: {e}");
            return 2;
        }
    };
    text.push('\n');
    if let Err(e) = fs::write(&args.out, text) {
        eprintln!("bench_pr_merge_confirm: {e}");
        return 2;
    }
    println!(
        "merged {} confirm round(s) into {}; gate_action={} exit_code={} reason={:?} hit={:?}",
        gates.len(),
        args.out.display(),
        merged
            .get("gate_action")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        merged
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1),
        merged
            .get("verdict_reason")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        merged.get("hit")
    );
    0
}
