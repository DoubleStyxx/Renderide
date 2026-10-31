#!/usr/bin/env python3
"""Merge confirm round-2 (fail-closed drift) rows into round-1 gate.json.

Round-2 rows replace INCONCLUSIVE (base drift) scenes from round 1. Other round-1
rows are kept. Records per-round base_drift_pct under base_drift_rounds.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def merge_confirm_gates(gate1: dict, gate2: dict) -> dict:
    by2 = {r["scenario"]: r for r in gate2.get("rows") or []}
    merged_rows = []
    for r in gate1.get("rows") or []:
        rounds = []
        if r.get("base_drift_pct") is not None:
            rounds.append({"round": int(r.get("confirm_round") or 1), "base_drift_pct": r["base_drift_pct"]})
        status = r.get("status") or ""
        if status.startswith("INCONCLUSIVE (base drift)") and r["scenario"] in by2:
            r2 = dict(by2[r["scenario"]])
            if r2.get("base_drift_pct") is not None:
                rounds.append(
                    {"round": int(r2.get("confirm_round") or 2), "base_drift_pct": r2["base_drift_pct"]}
                )
            r2["base_drift_rounds"] = rounds
            r2["base_drift_pct_round1"] = r.get("base_drift_pct")
            merged_rows.append(r2)
        else:
            r = dict(r)
            if rounds:
                r["base_drift_rounds"] = rounds
            merged_rows.append(r)
    out = dict(gate1)
    out["rows"] = merged_rows
    out["confirm_round"] = 2
    hit = [
        r["scenario"]
        for r in merged_rows
        if str(r.get("status") or "").startswith(("FAIL", "INCONCLUSIVE", "FLAG"))
    ]
    out["exit_code"] = 1 if hit else 0
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--gate1", type=Path, required=True)
    parser.add_argument("--gate2", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        g1 = json.loads(args.gate1.read_text(encoding="utf-8"))
        g2 = json.loads(args.gate2.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as err:
        print(f"bench_pr_merge_confirm: {err}", file=sys.stderr)
        return 2
    merged = merge_confirm_gates(g1, g2)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(merged, indent=2) + "\n", encoding="utf-8")
    print(
        f"merged confirm round 2 into {args.out}; exit_code={merged['exit_code']} "
        f"hit={[r['scenario'] for r in merged['rows'] if str(r.get('status','')).startswith(('FAIL','INCONCLUSIVE'))]}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
