"""frame-bench.yml stays in parity with bench-pr.yml gate defaults and drift wiring."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FRAME_PATH = ROOT / ".github/workflows/frame-bench.yml"
BENCH_PATH = ROOT / ".github/workflows/bench-pr.yml"
# bench-pr.yml sparse-checkouts tools/ to scripts/ci only; skip when the
# workflow files are absent. frame-bench-gate-tests (full checkout) still runs them.
_HAVE_WORKFLOWS = FRAME_PATH.is_file() and BENCH_PATH.is_file()
FRAME = FRAME_PATH.read_text(encoding="utf-8") if _HAVE_WORKFLOWS else ""
BENCH = BENCH_PATH.read_text(encoding="utf-8") if _HAVE_WORKFLOWS else ""


def _input_default(text: str, name: str) -> str | None:
    pattern = (
        r"^\s*"
        + re.escape(name)
        + r":\s*\n(?:^\s+[^\n]*\n)*?^\s+default:\s*\"([^\"]+)\""
    )
    m = re.search(pattern, text, re.MULTILINE)
    return m.group(1) if m else None


class FrameBenchParity(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not _HAVE_WORKFLOWS:
            raise unittest.SkipTest(
                "workflow yml absent (sparse tools checkout); covered by frame-bench-gate-tests"
            )

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
