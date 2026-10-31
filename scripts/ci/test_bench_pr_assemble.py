"""Unit tests for bench_pr_assemble.py. Run via: python3 -m unittest discover -s scripts/ci -p 'test_*.py'"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bench_pr_assemble as assemble  # noqa: E402


def _gate_doc(*, exit_code: int = 0, status: str = "ok", delta: float = 1.0) -> dict:
    return {
        "schema": "frame-bench-gate/1",
        "mode": "first",
        "flag_threshold": 5.0,
        "fail_threshold": 10.0,
        "abs_floor_ms": 0.0,
        "exit_code": exit_code,
        "rows": [
            {
                "scenario": "unlit_sphere",
                "base": 1.0,
                "head": 1.0 + delta / 100.0,
                "delta_pct": delta,
                "status": status,
                "p_value": None,
            }
        ],
    }


def _cpu_compare_doc() -> dict:
    return {
        "schema": "renderide-bench-compare/1",
        "rows": [
            {
                "suite": "cpu",
                "name": "cpu_hot/encode",
                "metric": "median_ns",
                "base": 100.0,
                "head": 110.0,
                "delta_pct": 10.0,
                "p_value": 0.01,
                "verdict": "flag",
            },
            {
                "suite": "frame",
                "name": "should_skip",
                "metric": "cpu_frame_ms.p95",
                "base": 1.0,
                "head": 2.0,
                "delta_pct": 100.0,
                "verdict": "fail",
            },
        ],
    }


class AssemblePassFailTests(unittest.TestCase):
    def test_pass_writes_compare_and_exits_zero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            gate_path = root / "gate.json"
            out_path = root / "bench-compare.json"
            gate_path.write_text(json.dumps(_gate_doc(exit_code=0, status="ok", delta=1.0)), encoding="utf-8")
            rc = assemble.main(["--gate", str(gate_path), "--out", str(out_path)])
            self.assertEqual(rc, 0)
            payload = json.loads(out_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["schema"], assemble.COMPARE_SCHEMA)
            self.assertEqual(payload["exit_code"], 0)
            self.assertEqual(payload["gate_mode"], "first")
            self.assertEqual(len(payload["rows"]), 1)
            row = payload["rows"][0]
            self.assertEqual(row["suite"], "frame")
            self.assertEqual(row["name"], "unlit_sphere")
            self.assertEqual(row["metric"], "cpu_frame_ms.p95")
            self.assertEqual(row["verdict"], "pass")
            self.assertEqual(row["gate_status"], "ok")
            self.assertAlmostEqual(row["delta_pct"], 1.0)

    def test_fail_gate_still_exits_zero_with_exit_code_in_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            gate_path = root / "gate.json"
            out_path = root / "bench-compare.json"
            gate_path.write_text(
                json.dumps(_gate_doc(exit_code=1, status="FAIL (confirm)", delta=25.0)),
                encoding="utf-8",
            )
            rc = assemble.main(["--gate", str(gate_path), "--out", str(out_path)])
            self.assertEqual(rc, 0, "assemble must exit 0 after writing so CI can upload on FAIL")
            payload = json.loads(out_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["exit_code"], 1)
            self.assertEqual(payload["rows"][0]["verdict"], "fail")
            self.assertEqual(payload["rows"][0]["gate_status"], "FAIL (confirm)")


class AssembleMissingInputTests(unittest.TestCase):
    def test_missing_gate_file_exits_2(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            out_path = root / "bench-compare.json"
            rc = assemble.main(["--gate", str(root / "nope.json"), "--out", str(out_path)])
            self.assertEqual(rc, 2)
            self.assertFalse(out_path.exists())

    def test_bad_schema_exits_2(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            gate_path = root / "gate.json"
            out_path = root / "bench-compare.json"
            gate_path.write_text(json.dumps({"schema": "wrong/1", "exit_code": 0, "rows": []}), encoding="utf-8")
            rc = assemble.main(["--gate", str(gate_path), "--out", str(out_path)])
            self.assertEqual(rc, 2)
            self.assertFalse(out_path.exists())

    def test_invalid_json_exits_2(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            gate_path = root / "gate.json"
            out_path = root / "bench-compare.json"
            gate_path.write_text("{not-json", encoding="utf-8")
            rc = assemble.main(["--gate", str(gate_path), "--out", str(out_path)])
            self.assertEqual(rc, 2)
            self.assertFalse(out_path.exists())


class AssembleJsonOutShapeTests(unittest.TestCase):
    def test_json_out_shape_includes_cpu_rows_and_required_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            gate_path = root / "gate.json"
            cpu_path = root / "cpu.json"
            out_path = root / "bench-compare.json"
            gate_path.write_text(json.dumps(_gate_doc(exit_code=0, status="FLAG", delta=7.0)), encoding="utf-8")
            cpu_path.write_text(json.dumps(_cpu_compare_doc()), encoding="utf-8")
            rc = assemble.main(
                ["--gate", str(gate_path), "--cpu-compare", str(cpu_path), "--out", str(out_path)]
            )
            self.assertEqual(rc, 0)
            payload = json.loads(out_path.read_text(encoding="utf-8"))
            for key in (
                "schema",
                "fail_threshold",
                "flag_threshold",
                "abs_floor_ms",
                "decision_rule",
                "gate_mode",
                "rows",
                "exit_code",
            ):
                self.assertIn(key, payload)
            self.assertEqual(payload["schema"], "renderide-bench-compare/1")
            self.assertEqual(payload["fail_threshold"], 10.0)
            self.assertEqual(payload["flag_threshold"], 5.0)
            # Frame row + one cpu row (frame rows from cpu compare are filtered out).
            self.assertEqual(len(payload["rows"]), 2)
            frame, cpu = payload["rows"]
            self.assertEqual(frame["suite"], "frame")
            self.assertEqual(frame["verdict"], "flag")
            self.assertEqual(cpu["suite"], "cpu")
            self.assertEqual(cpu["name"], "cpu_hot/encode")
            self.assertEqual(cpu["metric"], "median_ns")
            self.assertAlmostEqual(cpu["delta_pct"], 10.0)
            self.assertEqual(cpu["verdict"], "flag")


if __name__ == "__main__":
    unittest.main()
