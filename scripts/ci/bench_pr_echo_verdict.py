#!/usr/bin/env python3
"""Print a one-line GITHUB_STEP_SUMMARY verdict for a gate.json (gate_action + reason)."""
from __future__ import annotations

import json
import sys
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

import frame_bench_gate as gate  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv[1:]
    if len(args) != 1:
        print("usage: bench_pr_echo_verdict.py GATE.json", file=sys.stderr)
        return 2
    path = Path(args[0])
    g = json.loads(path.read_text(encoding="utf-8"))
    action = str(g.get("gate_action") or "").strip()
    reason = str(g.get("verdict_reason") or "").strip()
    # First-pass-only gate.json has no gate_action; derive from exit_code/mode.
    if not action:
        raw_exit = g.get("exit_code")
        exit_code = 1 if raw_exit is None else int(raw_exit)
        mode = str(g.get("mode") or "")
        if exit_code == 0:
            action = "pass"
            reason = reason or (
                "first pass clear (no confirm)" if mode == "first" else "gate clear"
            )
        else:
            action = "fail"
            reason = reason or (
                "first pass FLAG with confirm_rounds=0"
                if mode == "first"
                else "gate fail"
            )
        g["gate_action"] = action
        g["verdict_reason"] = reason
        path.write_text(json.dumps(g, indent=2) + "\n", encoding="utf-8")
    print(gate.format_summary_verdict(action, reason))
    return 0


if __name__ == "__main__":
    sys.exit(main())
