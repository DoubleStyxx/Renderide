#!/usr/bin/env python3
"""Stamp gate.json as inconclusive: unstable runner after a full-gate rerun failed to settle."""
from __future__ import annotations

import json
import sys
from pathlib import Path


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv[1:]
    if len(args) != 1:
        print("usage: bench_pr_force_unstable.py GATE.json", file=sys.stderr)
        return 2
    path = Path(args[0])
    g = json.loads(path.read_text(encoding="utf-8"))
    g["gate_action"] = "unstable"
    g["verdict_reason"] = (g.get("verdict_reason") or "") + " (forced unstable after full-gate rerun)"
    for r in g.get("rows") or []:
        st = str(r.get("status") or "")
        if st.startswith("INCONCLUSIVE") or st.startswith("FAIL (inconclusive: base drift)"):
            r["status"] = "FAIL (inconclusive: unstable runner)"
    g["exit_code"] = 1
    path.write_text(json.dumps(g, indent=2) + "\n", encoding="utf-8")
    print(f"forced unstable on {path}: {g.get('verdict_reason')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
