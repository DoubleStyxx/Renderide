//! Ported policy / replay / core cases from `test_frame_bench_gate.py`.
use super::*;
use crate::bench_pr_merge_confirm;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;

const FLAG: f64 = 5.0;
const FAIL: f64 = 10.0;

fn map_one(name: &str, samples: Vec<f64>) -> BTreeMap<String, Vec<f64>> {
    let mut m = BTreeMap::new();
    m.insert(name.into(), samples);
    m
}

#[test]
fn post_drift_disagreement_rerun() {
    let r = post_drift_policy(&["drift", "fail", "pass"], false);
    assert_ne!(r.verdict, "fail");
    assert_eq!(r.verdict, "rerun");
}

#[test]
fn post_drift_two_fails_after_drift() {
    let r = post_drift_policy(&["drift", "fail", "fail"], false);
    assert_eq!(r.verdict, "fail");
    assert_eq!(r.deciding_round, Some(3));
}

#[test]
fn post_drift_recurring_unstable_after_rerun() {
    let r = post_drift_policy(&["drift", "drift"], true);
    assert_eq!(r.verdict, "unstable");
    let line = format_summary_verdict(&r.verdict, &r.reason);
    assert!(line.contains("inconclusive: unstable runner"));
}

#[test]
fn post_drift_no_drift_path() {
    assert_eq!(post_drift_policy(&["fail"], false).verdict, "fail");
    assert_eq!(post_drift_policy(&["pass"], false).verdict, "pass");
}

#[test]
fn decide_no_drift_fail_and_pass() {
    let fb = vec![1.0; 12];
    let fh = vec![1.2; 12];
    let cb = vec![vec![1.0; 12]];
    let ch = vec![vec![1.20; 12]];
    assert_eq!(
        decide(
            &fb,
            &fh,
            &cb,
            &ch,
            FLAG,
            FAIL,
            DEFAULT_ALPHA,
            0.0,
            CONFIRM_MIN_N,
            None,
            None,
            None,
            None,
            BASE_DRIFT_PCT,
            false
        ),
        "fail"
    );
    let ch_ok = vec![vec![1.05; 12]];
    assert_eq!(
        decide(
            &fb,
            &fh,
            &cb,
            &ch_ok,
            FLAG,
            FAIL,
            DEFAULT_ALPHA,
            0.0,
            CONFIRM_MIN_N,
            None,
            None,
            None,
            None,
            BASE_DRIFT_PCT,
            false
        ),
        "pass"
    );
}

#[test]
fn decide_drift_then_two_fails() {
    let fb = vec![1.0; 12];
    let fh = vec![1.2; 12];
    let cb1 = vec![vec![1.20; 12]];
    let ch1 = vec![vec![1.30; 12]];
    let cb2 = vec![vec![1.02; 12]];
    let ch2 = vec![vec![1.20; 12]];
    let cb3 = vec![vec![1.01; 12]];
    let ch3 = vec![vec![1.18; 12]];
    assert_eq!(
        decide(
            &fb,
            &fh,
            &cb1,
            &ch1,
            FLAG,
            FAIL,
            DEFAULT_ALPHA,
            0.0,
            CONFIRM_MIN_N,
            Some(&cb2),
            Some(&ch2),
            Some(&cb3),
            Some(&ch3),
            BASE_DRIFT_PCT,
            false
        ),
        "fail"
    );
}

#[test]
fn evaluate_first_flags_above_threshold() {
    let base = map_one("s", vec![1.0; 5]);
    let head = map_one("s", vec![1.2; 5]); // +20%
    let rows = evaluate_first(&base, &head, FLAG, &[], 0.0);
    assert_eq!(rows[0].status, "FLAG");
}

#[test]
fn evaluate_first_ok_below_flag() {
    let base = map_one("s", vec![1.0; 5]);
    let head = map_one("s", vec![1.04; 5]); // +4%
    let rows = evaluate_first(&base, &head, FLAG, &[], 0.0);
    assert_eq!(rows[0].status, "ok");
}

#[test]
fn mann_whitney_all_ties_is_one() {
    let a = vec![1.0; 4];
    let b = vec![1.0; 4];
    let p = mann_whitney_greater(&a, &b);
    assert!((p - 1.0).abs() < 1e-9, "p={p}");
}

#[test]
fn mann_whitney_same_multiset_near_half() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![1.0, 2.0, 3.0, 4.0];
    let p = mann_whitney_greater(&a, &b);
    assert!((0.4..=0.7).contains(&p), "p={p}");
}

#[test]
fn mann_whitney_head_clearly_greater() {
    let base = vec![1.0; 12];
    let head = vec![2.0; 12];
    let p = mann_whitney_greater(&base, &head);
    assert!(p < 0.05, "p={p}");
}

#[test]
fn summary_verdict_covers_actions() {
    let cases = [
        ("pass", "confirm clear (no drift)", "gate_action=pass"),
        (
            "fail",
            "confirm-only FAIL (no drift)",
            "gate_action=fail (perf)",
        ),
        ("need_confirm", "one drift-clean FAIL", "need_confirm"),
        ("rerun", "disagreement after drift", "gate_action=rerun"),
        (
            "unstable",
            "recurring base drift after full-gate rerun",
            "inconclusive: unstable runner",
        ),
    ];
    for (action, reason, must_have) in cases {
        let line = format_summary_verdict(action, reason);
        assert!(line.contains(must_have), "{line}");
        assert!(line.contains("verdict_reason="));
        assert!(line.contains(reason));
    }
}

#[test]
fn replay_271_need_confirm() {
    let g1 = json!({
        "confirm_round": 1,
        "rows": [
            {"scenario": "multi_primitive_unlit", "status": "INCONCLUSIVE (base drift)", "base_drift_pct": 79.19, "confirm_round": 1},
            {"scenario": "unlit_sphere", "status": "INCONCLUSIVE (base drift)", "base_drift_pct": 119.04, "confirm_round": 1}
        ]
    });
    let g2 = json!({
        "confirm_round": 2,
        "rows": [
            {"scenario": "multi_primitive_unlit", "status": "FAIL (confirm-only)", "base_drift_pct": 3.10, "confirm_round": 2, "deciding_round": 2},
            {"scenario": "unlit_sphere", "status": "ok", "base_drift_pct": 12.10, "confirm_round": 2, "deciding_round": 2}
        ]
    });
    let out = bench_pr_merge_confirm::merge_confirm_gates(&[g1, g2], false);
    assert_eq!(out["gate_action"], "need_confirm");
    assert_ne!(out["gate_action"], "fail");
    assert_eq!(out["exit_code"], 5);
}

#[test]
fn replay_276_still_fails() {
    let g = json!({
        "confirm_round": 1,
        "rows": [
            {"scenario": "multi_primitive_unlit", "status": "FAIL (confirm-only)", "base_drift_pct": 1.49, "confirm_round": 1, "deciding_round": 1},
            {"scenario": "unlit_sphere", "status": "FAIL (confirm-only)", "base_drift_pct": 1.17, "confirm_round": 1, "deciding_round": 1}
        ]
    });
    let out = bench_pr_merge_confirm::merge_confirm_gates(&[g], false);
    assert_eq!(out["gate_action"], "fail");
    assert_eq!(out["exit_code"], 1);
}

#[test]
fn replay_262_rerun_then_unstable() {
    let g1 = json!({
        "confirm_round": 1,
        "rows": [
            {"scenario": "multi_primitive_unlit", "status": "INCONCLUSIVE (base drift)", "base_drift_pct": 31.59, "confirm_round": 1},
            {"scenario": "unlit_sphere", "status": "INCONCLUSIVE (base drift)", "base_drift_pct": 29.31, "confirm_round": 1}
        ]
    });
    let g2 = json!({
        "confirm_round": 2,
        "rows": [
            {"scenario": "multi_primitive_unlit", "status": "FAIL (inconclusive: base drift)", "base_drift_pct": 33.55, "confirm_round": 2},
            {"scenario": "unlit_sphere", "status": "FAIL (inconclusive: base drift)", "base_drift_pct": 32.48, "confirm_round": 2}
        ]
    });
    let out = bench_pr_merge_confirm::merge_confirm_gates(&[g1.clone(), g2.clone()], false);
    assert_eq!(out["gate_action"], "rerun");
    let out2 = bench_pr_merge_confirm::merge_confirm_gates(&[g1, g2], true);
    assert_eq!(out2["gate_action"], "unstable");
    assert!(
        out2["rows"][0]["status"]
            .as_str()
            .unwrap()
            .contains("inconclusive: unstable runner")
    );
}

#[test]
fn replay_fixtures_new_policy_parity() {
    // #271 / #276 recorded new-policy.json expectations.
    let p271 = include_str!("../../tests/fixtures/replay/271/new-policy.json");
    let expect271: Value = serde_json::from_str(p271).unwrap();
    assert_eq!(expect271["gate_action"], "need_confirm");
    assert_eq!(expect271["exit_code"], 5);

    let p276 = include_str!("../../tests/fixtures/replay/276/new-policy.json");
    let expect276: Value = serde_json::from_str(p276).unwrap();
    assert_eq!(expect276["gate_action"], "fail");
}

#[test]
fn merge_replaces_inconclusive_with_round2() {
    let g1 = json!({
        "schema": "frame-bench-gate/1",
        "mode": "confirm",
        "exit_code": 1,
        "rows": [
            {"scenario": "mpu", "status": "INCONCLUSIVE (base drift)", "base_drift_pct": 20.0, "confirm_round": 1, "deciding_round": null},
            {"scenario": "unlit", "status": "FAIL (confirm-only)", "base_drift_pct": 5.0, "confirm_round": 1, "deciding_round": 1}
        ]
    });
    let g2 = json!({
        "schema": "frame-bench-gate/1",
        "mode": "confirm",
        "exit_code": 1,
        "rows": [
            {"scenario": "mpu", "status": "FAIL (confirm-only)", "base_drift_pct": 3.0, "confirm_round": 2, "deciding_round": 2, "confirm_delta_pct": 14.0}
        ]
    });
    let out = bench_pr_merge_confirm::merge_confirm_gates(&[g1, g2], false);
    let by: BTreeMap<_, _> = out["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["scenario"].as_str().unwrap().to_string(), r.clone()))
        .collect();
    assert_eq!(by["mpu"]["status"], "FAIL (confirm-only)");
    assert_eq!(by["mpu"]["deciding_round"], 2);
    assert_eq!(by["mpu"]["base_drift_pct_round1"], 20.0);
    assert_eq!(out["gate_action"], "fail");
}

#[test]
fn canary_artifacts_pass_action() {
    // Embedded copies of b35 canary gate.json (pass).
    let canary = json!({
        "schema": "frame-bench-gate/1",
        "mode": "confirm",
        "exit_code": 0,
        "gate_action": "pass",
        "rows": [
            {"scenario": "multi_primitive_unlit", "status": "ok"},
            {"scenario": "unlit_sphere", "status": "ok"}
        ]
    });
    let out = bench_pr_merge_confirm::merge_confirm_gates(&[canary], false);
    assert_eq!(out["gate_action"], "pass");
    assert_eq!(out["exit_code"], 0);
}

/// Regression for #302 hold: after a full-gate rerun, a leftover `gate-confirm2.json`
/// from attempt 1 must not be fed to shadow-diff. Python merged only confirm1 (list);
/// existence-based discovery of confirm2 caused a false mismatch on
/// `multi_primitive_unlit` (python ok vs rust FAIL inconclusive: base drift).
#[test]
fn shadow_stale_confirm2_from_prior_attempt() {
    let dir = std::env::temp_dir().join(format!("xtask-stale-rerun-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let confirm1: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/stale-rerun/gate-confirm1.json"
    ))
    .unwrap();
    let stale2: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/stale-rerun/gate-confirm2-stale.json"
    ))
    .unwrap();

    let p1 = dir.join("gate-confirm1.json");
    let p2 = dir.join("gate-confirm2.json"); // name mirrors CI leftover
    fs::write(&p1, serde_json::to_vec_pretty(&confirm1).unwrap()).unwrap();
    fs::write(&p2, serde_json::to_vec_pretty(&stale2).unwrap()).unwrap();

    // Authoritative Python merge used ONLY confirm1 (attempt 2 had no drift → no round 2).
    let py = bench_pr_merge_confirm::merge_confirm_gates(std::slice::from_ref(&confirm1), false);
    assert_eq!(py["gate_action"], "pass", "confirm1-only merge must pass");
    let py_path = dir.join("gate.json");
    fs::write(&py_path, serde_json::to_vec_pretty(&py).unwrap()).unwrap();

    // Bug reproduction: merging confirm1+stale confirm2 keeps gate_action=pass but
    // overwrites row status to FAIL (inconclusive: base drift) — the #302 false mismatch.
    let buggy = bench_pr_merge_confirm::merge_confirm_gates(&[confirm1, stale2], false);
    let mpu = buggy["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["scenario"] == "multi_primitive_unlit")
        .unwrap();
    assert_eq!(
        mpu["status"].as_str().unwrap(),
        "FAIL (inconclusive: base drift)",
        "stale confirm2 overwrites multi_primitive_unlit row status"
    );
    let py_mpu = py["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["scenario"] == "multi_primitive_unlit")
        .unwrap();
    assert_eq!(py_mpu["status"], "ok");
    assert_ne!(
        mpu["status"], py_mpu["status"],
        "row-level mismatch is the false shadow failure mode"
    );

    // Correct shadow: merge-gates.list names only confirm1 even though confirm2 exists on disk.
    let list_path = dir.join("merge-gates.list");
    fs::write(&list_path, format!("{}\n", p1.display())).unwrap();
    let rust_path = dir.join("gate.xtask.json");
    let rc = crate::shadow_diff::run(crate::shadow_diff::Args {
        python_json: py_path.clone(),
        rust_json: rust_path,
        rerun_rust: true,
        base: vec![],
        head: vec![],
        confirm_base: vec![],
        confirm_head: vec![],
        flag_threshold: 5.0,
        fail_threshold: 10.0,
        mode: "first".into(),
        fail_closed_drift: false,
        confirm_round: 1,
        alpha: DEFAULT_ALPHA,
        abs_floor_ms: 0.0,
        head_missing: vec![],
        merge_gate: vec![],
        merge_gates_list: Some(list_path),
        after_rerun: true,
        compare: "merge".into(),
    });
    assert_eq!(
        rc, 0,
        "shadow with merge-gates.list must match Python (ignore stale confirm2)"
    );

    // Existence-based misuse (both files) must mismatch — documents the failure mode.
    let rust_bad = dir.join("gate.xtask.bad.json");
    let rc_bad = crate::shadow_diff::run(crate::shadow_diff::Args {
        python_json: py_path,
        rust_json: rust_bad,
        rerun_rust: true,
        base: vec![],
        head: vec![],
        confirm_base: vec![],
        confirm_head: vec![],
        flag_threshold: 5.0,
        fail_threshold: 10.0,
        mode: "first".into(),
        fail_closed_drift: false,
        confirm_round: 1,
        alpha: DEFAULT_ALPHA,
        abs_floor_ms: 0.0,
        head_missing: vec![],
        merge_gate: vec![p1, p2],
        merge_gates_list: None,
        after_rerun: true,
        compare: "merge".into(),
    });
    assert_eq!(
        rc_bad, 1,
        "feeding stale confirm2 must surface a shadow mismatch"
    );

    let _ = fs::remove_dir_all(&dir);
}
