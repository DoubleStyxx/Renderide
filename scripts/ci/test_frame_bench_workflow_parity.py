"""frame-bench.yml stays in parity with bench-pr.yml gate defaults and drift wiring."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FRAME = (ROOT / ".github/workflows/frame-bench.yml").read_text(encoding="utf-8")
BENCH = (ROOT / ".github/workflows/bench-pr.yml").read_text(encoding="utf-8")


def _input_default(text: str, name: str) -> str | None:
    pattern = (
        r"^\s*"
        + re.escape(name)
        + r":\s*\n(?:^\s+[^\n]*\n)*?^\s+default:\s*\"([^\"]+)\""
    )
    m = re.search(pattern, text, re.MULTILINE)
    return m.group(1) if m else None


class FrameBenchParity(unittest.TestCase):
    def test_runs_default_is_12(self) -> None:
        self.assertEqual(_input_default(FRAME, "runs"), "12")
        self.assertIn('RUNS: "12"', BENCH)

    def test_confirm_rounds_default_is_1(self) -> None:
        self.assertEqual(_input_default(FRAME, "confirm_rounds"), "1")
        self.assertIn('CONFIRM_ROUNDS: "1"', BENCH)

    def test_frame_bench_wires_drift_guard(self) -> None:
        for needle in (
            "--drift-out",
            "--fail-closed-drift",
            "--confirm-round 1",
            "--confirm-round 2",
            "bench_pr_merge_confirm.py",
            "base-drift guard",
        ):
            self.assertIn(needle, FRAME, needle)

    def test_frame_bench_interleaved_chunks(self) -> None:
        self.assertIn("half=$((runs / 2))", FRAME)
        self.assertIn("rest=$((runs - half))", FRAME)
        self.assertIn("interleaved chunks", FRAME)


if __name__ == "__main__":
    unittest.main()
