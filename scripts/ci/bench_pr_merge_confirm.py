#!/usr/bin/env python3
"""Merge multi-round confirm gate.json files under the post-drift two-clean-FAIL policy.

Round N rows replace prior INCONCLUSIVE (base drift) scenes. After any drift event,
FAIL only if two consecutive drift-clean confirms both FAIL. Disagreement or recurring
drift → gate_action `rerun` (or `unstable` when --after-rerun). A single drift-clean
ok clears. No-drift confirm-only FAIL/pass is unchanged (#253).

Records per-round base_drift_pct under base_drift_rounds and writes gate_action /
verdict_reason for the workflow.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Allow `python3 scripts/ci/bench_pr_merge_confirm.py` from repo root and unittest discover.
_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

import frame_bench_gate as gate  # noqa: E402


def _kind(status: str) -> str:
    return gate.confirm_status_kind(status or "")


def merge_confirm_gates(
    gates,
    gate2: dict | None = None,
    *,
    after_rerun: bool = False,
) -> dict:
    """Merge ordered confirm-round gate dicts (round 1..N) into one gate.json body.

    Back-compat: ``merge_confirm_gates(g1, g2)`` still works (two dict args).
    """
    if gate2 is not None:
        gates = [gates, gate2]
    if not isinstance(gates, list):
        raise TypeError("gates must be a list of gate dicts")
    if not gates:
        raise ValueError("merge_confirm_gates requires at least one gate")
    if len(gates) == 1:
        out = dict(gates[0])
        # Still annotate gate_action from row statuses (no-drift or incomplete).
        return _annotate_action(out, after_rerun=after_rerun)

    # Start from round 1 rows; overlay later rounds onto scenes that were drifting
    # (or that the later round re-measured). Keep non-drift round-1 decisions.
    merged_by: dict[str, dict] = {}
    round_kinds: dict[str, list[str]] = {}
    drift_rounds: dict[str, list[dict]] = {}

    for gi, g in enumerate(gates):
        round_no = int(g.get("confirm_round") or (gi + 1))
        for r in g.get("rows") or []:
            name = r["scenario"]
            status = r.get("status") or ""
            kind = _kind(status)
            rounds = drift_rounds.setdefault(name, [])
            if r.get("base_drift_pct") is not None:
                rounds.append({"round": round_no, "base_drift_pct": r["base_drift_pct"]})
            kinds = round_kinds.setdefault(name, [])
            # Only record a kind slot when this round actually measured the scene,
            # or when we are still filling from round 1.
            if gi == 0:
                kinds.append(kind)
                row = dict(r)
                if rounds:
                    row["base_drift_rounds"] = list(rounds)
                merged_by[name] = row
            else:
                prev = merged_by.get(name)
                prev_status = (prev or {}).get("status") or ""
                # Later rounds replace scenes that were inconclusive/drifting, or
                # any scene re-measured in this gate (present in g.rows).
                if prev is None or gate.is_inconclusive_drift(prev_status) or name in {
                    x["scenario"] for x in (g.get("rows") or [])
                }:
                    # Extend kinds: if we skipped intermediate rounds for this scene, pad.
                    while len(kinds) < gi:
                        kinds.append("pass")  # non-drift scenes not re-measured
                    if len(kinds) == gi:
                        kinds.append(kind)
                    else:
                        kinds[gi] = kind
                    row = dict(r)
                    if rounds:
                        row["base_drift_rounds"] = list(rounds)
                    if prev is not None and prev.get("base_drift_pct") is not None:
                        row["base_drift_pct_round1"] = prev.get("base_drift_pct_round1", prev.get("base_drift_pct"))
                    merged_by[name] = row

    out = dict(gates[0])
    out["rows"] = list(merged_by.values())
    out["confirm_round"] = int(gates[-1].get("confirm_round") or len(gates))
    return _annotate_action(out, round_kinds=round_kinds, after_rerun=after_rerun)


def _annotate_action(
    out: dict,
    *,
    round_kinds: dict[str, list[str]] | None = None,
    after_rerun: bool = False,
) -> dict:
    """Set gate_action / verdict_reason / exit_code from per-scenario policy results."""
    if round_kinds is None:
        round_kinds = {}
        for r in out.get("rows") or []:
            round_kinds[r["scenario"]] = [_kind(r.get("status") or "")]

    scenario_results: dict[str, gate.DecideResult] = {}
    for name, kinds in round_kinds.items():
        scenario_results[name] = gate.post_drift_policy(kinds, after_rerun=after_rerun)

    # Priority: unstable > fail > rerun > need_confirm > pass
    priority = {"unstable": 5, "fail": 4, "rerun": 3, "need_confirm": 2, "pass": 1}
    best = gate.DecideResult("pass", "all scenarios clear")
    for name, res in scenario_results.items():
        if priority.get(res.verdict, 0) > priority.get(best.verdict, 0):
            best = gate.DecideResult(res.verdict, f"{name}: {res.reason}", res.deciding_round)

    # Write final row statuses for unstable.
    if best.verdict == "unstable":
        new_rows = []
        for r in out.get("rows") or []:
            r = dict(r)
            name = r["scenario"]
            res = scenario_results.get(name)
            if res and res.verdict == "unstable":
                r["status"] = "FAIL (inconclusive: unstable runner)"
                r["deciding_round"] = None
            new_rows.append(r)
        out["rows"] = new_rows

    out["gate_action"] = best.verdict
    out["verdict_reason"] = best.reason
    if best.deciding_round is not None:
        out["deciding_round"] = best.deciding_round

    # exit_code: 0 pass; 1 fail/unstable; workflow handles need_confirm/rerun before Apply.
    if best.verdict in ("pass",):
        out["exit_code"] = 0
    elif best.verdict in ("fail", "unstable"):
        out["exit_code"] = 1
    elif best.verdict == "rerun":
        out["exit_code"] = 3
    elif best.verdict == "need_confirm":
        out["exit_code"] = 5
    else:
        out["exit_code"] = 1

    # hit list for logs: anything that is not a clean pass row under final action.
    hit = []
    for r in out.get("rows") or []:
        st = str(r.get("status") or "")
        if st.startswith(("FAIL", "INCONCLUSIVE", "FLAG")):
            hit.append(r["scenario"])
    out["hit"] = hit
    return out


# Back-compat wrapper used by older tests / call sites.
def merge_confirm_gates_pair(gate1: dict, gate2: dict, *, after_rerun: bool = False) -> dict:
    g1 = dict(gate1)
    g1.setdefault("confirm_round", 1)
    g2 = dict(gate2)
    g2.setdefault("confirm_round", 2)
    return merge_confirm_gates([g1, g2], after_rerun=after_rerun)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--gate",
        action="append",
        type=Path,
        dest="gates",
        required=True,
        help="Confirm-round gate.json in order (repeatable; round 1 first)",
    )
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--after-rerun",
        action="store_true",
        help="This attempt follows a full-gate rerun; recurring drift/disagreement → unstable",
    )
    # Legacy pair flags
    parser.add_argument("--gate1", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--gate2", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    paths: list[Path] = list(args.gates or [])
    if args.gate1 and args.gate2:
        paths = [args.gate1, args.gate2]
    if len(paths) < 1:
        print("bench_pr_merge_confirm: need at least one --gate", file=sys.stderr)
        return 2
    try:
        gates = [json.loads(p.read_text(encoding="utf-8")) for p in paths]
    except (OSError, json.JSONDecodeError) as err:
        print(f"bench_pr_merge_confirm: {err}", file=sys.stderr)
        return 2

    merged = merge_confirm_gates(gates, after_rerun=args.after_rerun)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(merged, indent=2) + "\n", encoding="utf-8")
    print(
        f"merged {len(gates)} confirm round(s) into {args.out}; "
        f"gate_action={merged.get('gate_action')} exit_code={merged['exit_code']} "
        f"reason={merged.get('verdict_reason')!r} hit={merged.get('hit')}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
