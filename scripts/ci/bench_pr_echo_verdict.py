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
    print(
        gate.format_summary_verdict(
            str(g.get("gate_action") or ""),
            str(g.get("verdict_reason") or ""),
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
