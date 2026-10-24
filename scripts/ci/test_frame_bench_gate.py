"""Unit tests for frame_bench_gate.py. Run: python3 -m unittest discover -s scripts/ci -p 'test_*.py'"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import frame_bench_gate as gate  # noqa: E402


def report(scenarios: dict[str, list[float]]) -> dict:
    return {
        "schema": 1,
        "scenarios": [
            {"name": name, "cpu_frame_ms": {"p50": 0.0, "p95": runs[0], "p99": 0.0}, "per_run_cpu_frame_ms_p95": runs}
            for name, runs in scenarios.items()
        ],
    }


class GateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write(self, name: str, scenarios: dict[str, list[float]]) -> Path:
        path = self.dir / name
        path.write_text(json.dumps(report(scenarios)), encoding="utf-8")
        return path

    def run_gate(self, *args: str) -> int:
        return gate.main(list(args))

    def test_delta_inside_base_spread_passes(self) -> None:
        # Shape of #178 run 40050133923: +~11% on p95 while base runs spread ~20%.
        base = self.write("b.json", {"unlit_sphere": [0.19, 0.2145, 0.23, 0.21, 0.22]})
        head = self.write("h.json", {"unlit_sphere": [0.2391, 0.23, 0.24, 0.25, 0.235]})
        rows = gate.evaluate(gate.load([base]), gate.load([head]), 10.0, [])
        self.assertEqual(rows[0].status, "ok")
        self.assertGreater(rows[0].delta, 10.0)

    def test_delta_beyond_threshold_and_spread_fails(self) -> None:
        base = self.write("b.json", {"s": [1.0, 1.01, 0.99, 1.0, 1.0]})
        head = self.write("h.json", {"s": [1.2, 1.21, 1.19, 1.2, 1.2]})
        suspects = self.dir / "suspects.txt"
        code = self.run_gate("--base", str(base), "--head", str(head), "--suspects-out", str(suspects))
        self.assertEqual(code, 1)
        self.assertEqual(suspects.read_text(encoding="utf-8"), "s\n")

    def test_threshold_equality_passes(self) -> None:
        base = self.write("b.json", {"s": [1.0, 1.0, 1.0]})
        head = self.write("h.json", {"s": [1.1, 1.1, 1.1]})
        rows = gate.evaluate(gate.load([base]), gate.load([head]), 10.000001, [])
        self.assertEqual(rows[0].status, "ok")

    def test_reports_are_pooled(self) -> None:
        b1 = self.write("b1.json", {"s": [1.0]})
        b2 = self.write("b2.json", {"s": [2.0, 3.0]})
        self.assertEqual(gate.load([b1, b2]), {"s": [1.0, 2.0, 3.0]})

    def test_base_missing_and_head_missing_never_fail(self) -> None:
        base = self.write("b.json", {"a": [1.0, 1.0]})
        head = self.write("h.json", {"a": [1.0, 1.0], "new_scene": [5.0, 5.0]})
        rows = gate.evaluate(gate.load([base]), gate.load([head]), 10.0, ["gone_scene"])
        status = {row.scenario: row.status for row in rows}
        self.assertEqual(status, {"a": "ok", "new_scene": "base N/A", "gone_scene": "head N/A, skipped"})
        md = gate.render(rows, "t", 10.0)
        self.assertIn("| gone_scene | n/a | n/a | n/a | n/a | head N/A, skipped |", md)

    def test_wrong_schema_is_an_input_error(self) -> None:
        path = self.dir / "bad.json"
        path.write_text(json.dumps({"schema": 2, "scenarios": []}), encoding="utf-8")
        self.assertEqual(self.run_gate("--base", str(path), "--head", str(path)), 2)


if __name__ == "__main__":
    unittest.main()
