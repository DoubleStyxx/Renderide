"""Unit tests for bench_pr_report.py missing-artifact paths.

Run via: python3 -m unittest discover -s scripts/ci -p 'test_*.py'
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bench_pr_report as report  # noqa: E402


def _gate_doc(*, exit_code: int = 0) -> dict:
    return {
        "schema": "frame-bench-gate/1",
        "mode": "confirm",
        "flag_threshold": 5.0,
        "fail_threshold": 10.0,
        "abs_floor_ms": 0.0,
        "exit_code": exit_code,
        "rows": [
            {
                "scenario": "unlit_sphere",
                "base": 1.0,
                "head": 1.01,
                "delta_pct": 1.0,
                "status": "ok",
                "p_value": 0.5,
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
                "head": 101.0,
                "delta_pct": 1.0,
                "verdict": "pass",
            }
        ],
        "exit_code": 0,
    }


class ReportMissingArtifactTests(unittest.TestCase):
    def _run(
        self,
        root: Path,
        *,
        write_gate: bool,
        write_cpu: bool,
    ) -> tuple[int, str, Path]:
        gate_art = root / "gate-art"
        cpu_art = root / "cpu-art"
        gate_art.mkdir()
        cpu_art.mkdir()
        if write_gate:
            (gate_art / "frame-summary.md").write_text("# frame\ngate: pass\n", encoding="utf-8")
            (gate_art / "gate.json").write_text(json.dumps(_gate_doc()), encoding="utf-8")
        if write_cpu:
            (cpu_art / "cpu-summary.md").write_text("# cpu\nwall_time_sec: 1\n", encoding="utf-8")
            (cpu_art / "cpu-compare.json").write_text(json.dumps(_cpu_compare_doc()), encoding="utf-8")
        summary_out = root / "bench-pr-summary.md"
        compare_out = root / "bench-compare.json"
        rc = report.run_report(
            gate_art=gate_art,
            cpu_art=cpu_art,
            summary_out=summary_out,
            compare_out=compare_out,
            frame_job_result="success" if write_gate else "failure",
            gate_exit="0" if write_gate else "",
            wall_time_sec="10" if write_gate else "",
            cpu_job_result="success" if write_cpu else "failure",
        )
        summary = summary_out.read_text(encoding="utf-8")
        return rc, summary, compare_out

    def test_gate_artifact_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            rc, summary, compare = self._run(Path(tmp), write_gate=False, write_cpu=True)
            self.assertEqual(rc, 0)
            self.assertIn(report.FRAME_SUMMARY_MISSING, summary)
            self.assertIn(report.GATE_JSON_MISSING, summary)
            self.assertIn("# cpu", summary)
            self.assertNotIn(report.CPU_SUMMARY_MISSING, summary)
            self.assertFalse(compare.exists())

    def test_cpu_artifact_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            rc, summary, compare = self._run(Path(tmp), write_gate=True, write_cpu=False)
            self.assertEqual(rc, 0)
            self.assertIn("# frame", summary)
            self.assertIn(report.CPU_SUMMARY_MISSING, summary)
            self.assertNotIn(report.FRAME_SUMMARY_MISSING, summary)
            self.assertNotIn(report.GATE_JSON_MISSING, summary)
            self.assertTrue(compare.exists())
            payload = json.loads(compare.read_text(encoding="utf-8"))
            self.assertEqual(len(payload["rows"]), 1)
            self.assertEqual(payload["rows"][0]["suite"], "frame")

    def test_both_artifacts_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            rc, summary, compare = self._run(Path(tmp), write_gate=False, write_cpu=False)
            self.assertEqual(rc, 0)
            self.assertIn(report.FRAME_SUMMARY_MISSING, summary)
            self.assertIn(report.CPU_SUMMARY_MISSING, summary)
            self.assertIn(report.GATE_JSON_MISSING, summary)
            self.assertFalse(compare.exists())

    def test_both_present_assembles_compare(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            rc, summary, compare = self._run(Path(tmp), write_gate=True, write_cpu=True)
            self.assertEqual(rc, 0)
            self.assertIn("# frame", summary)
            self.assertIn("# cpu", summary)
            self.assertNotIn(report.FRAME_SUMMARY_MISSING, summary)
            self.assertNotIn(report.CPU_SUMMARY_MISSING, summary)
            self.assertNotIn(report.GATE_JSON_MISSING, summary)
            payload = json.loads(compare.read_text(encoding="utf-8"))
            self.assertEqual([r["suite"] for r in payload["rows"]], ["frame", "cpu"])


if __name__ == "__main__":
    unittest.main()
