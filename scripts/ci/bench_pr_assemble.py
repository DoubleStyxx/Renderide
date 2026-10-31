#!/usr/bin/env python3
"""Assemble bench-compare.json for bench-pr.yml from frame_bench_gate --json-out and optional cpu compare.

Frame rows come from frame_bench_gate.py (the single decision rule, including Mann-Whitney
p-values after confirm). CPU rows come from `renderide-bench compare --json` when present;
cpu verdicts are informational and never raise this script's exit code.

After a successful write of --out, this process always exits 0. The gate decision lives in
the JSON document's exit_code field (0 pass, 1 fail/flagged); the workflow reads that (or
the Measure step's gate_exit output) in a separate final step. Exit 2 is reserved for
usage / missing / invalid input before a document is written.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


COMPARE_SCHEMA = "renderide-bench-compare/1"
DECISION_RULE = "scripts/ci/frame_bench_gate.py (frame); renderide-bench compare (cpu, report-only)"


def gate_status_to_verdict(status: str) -> str:
    if status.startswith("FAIL") or status.startswith("INCONCLUSIVE"):
        return "fail"
    if status == "FLAG":
        return "flag"
    if status.startswith("ok"):
        return "pass"
    if "N/A" in status or status == "n/a":
        return "missing"
    return "pass"


def rows_from_gate(gate: dict) -> list[dict]:
    rows = []
    for r in gate.get("rows") or []:
        rows.append(
            {
                "suite": "frame",
                "name": r["scenario"],
                "metric": "cpu_frame_ms.p95",
                "base": r.get("base"),
                "head": r.get("head"),
                "delta_pct": r.get("delta_pct"),
                "p_value": r.get("p_value") if r.get("p_value") is not None else r.get("confirm_p"),
                "verdict": gate_status_to_verdict(r.get("status") or "ok"),
                "gate_status": r.get("status"),
                "base_drift_pct": r.get("base_drift_pct"),
                "base_drift_pct_round1": r.get("base_drift_pct_round1"),
                "base_drift_rounds": r.get("base_drift_rounds"),
                "confirm_round": r.get("confirm_round"),
                "deciding_round": r.get("deciding_round"),
            }
        )
    return rows


def rows_from_cpu(compare: dict) -> list[dict]:
    rows = []
    for r in compare.get("rows") or []:
        if r.get("suite") != "cpu":
            continue
        rows.append(
            {
                "suite": "cpu",
                "name": r.get("name"),
                "metric": r.get("metric") or "median_ns",
                "base": r.get("base"),
                "head": r.get("head"),
                "delta_pct": r.get("delta_pct"),
                "p_value": r.get("p_value"),
                "verdict": r.get("verdict") or "pass",
            }
        )
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--gate", type=Path, required=True, help="frame_bench_gate --json-out document")
    parser.add_argument("--cpu-compare", type=Path, help="optional renderide-bench compare --json document")
    parser.add_argument("--out", type=Path, required=True, help="bench-compare.json path")
    args = parser.parse_args(argv)

    if not args.gate.is_file():
        print(f"bench_pr_assemble: missing gate input {args.gate}", file=sys.stderr)
        return 2

    try:
        gate = json.loads(args.gate.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        print(f"bench_pr_assemble: invalid gate JSON: {err}", file=sys.stderr)
        return 2

    if gate.get("schema") != "frame-bench-gate/1":
        print(f"bench_pr_assemble: unexpected gate schema {gate.get('schema')!r}", file=sys.stderr)
        return 2

    rows = rows_from_gate(gate)
    if args.cpu_compare is not None and args.cpu_compare.is_file():
        try:
            cpu = json.loads(args.cpu_compare.read_text(encoding="utf-8"))
        except json.JSONDecodeError as err:
            print(f"bench_pr_assemble: invalid cpu-compare JSON: {err}", file=sys.stderr)
            return 2
        rows.extend(rows_from_cpu(cpu))

    exit_code = int(gate.get("exit_code") or 0)
    payload = {
        "schema": COMPARE_SCHEMA,
        "fail_threshold": gate.get("fail_threshold", 10.0),
        "flag_threshold": gate.get("flag_threshold", 5.0),
        "abs_floor_ms": gate.get("abs_floor_ms", 0.0),
        "decision_rule": DECISION_RULE,
        "gate_mode": gate.get("mode"),
        "rows": rows,
        "exit_code": exit_code,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    # Gate verdict stays in payload["exit_code"]; workflow applies it in a later step.
    return 0


if __name__ == "__main__":
    sys.exit(main())
