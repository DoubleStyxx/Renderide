"""Unit tests for mutants_summary.py."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import mutants_summary as summary  # noqa: E402


def write_run(root: Path, outcomes: dict, missed: list[str] | None) -> Path:
    out = root / "mutants.out"
    out.mkdir()
    (out / "outcomes.json").write_text(json.dumps(outcomes), encoding="utf-8")
    if missed is not None:
        (out / "missed.txt").write_text("\n".join(missed) + ("\n" if missed else ""), encoding="utf-8")
    return out


class MutationScore(unittest.TestCase):
    def test_score_excludes_unviable(self) -> None:
        counts = {"caught": 8, "missed": 2, "timeout": 0, "unviable": 5, "total": 15}
        self.assertEqual(summary.mutation_score(counts), 0.8)

    def test_timeout_lowers_score(self) -> None:
        counts = {"caught": 1, "missed": 1, "timeout": 2, "unviable": 0, "total": 4}
        self.assertEqual(summary.mutation_score(counts), 0.25)

    def test_no_viable_mutants(self) -> None:
        counts = {"caught": 0, "missed": 0, "timeout": 0, "unviable": 3, "total": 3}
        self.assertIsNone(summary.mutation_score(counts))


class Summarize(unittest.TestCase):
    def test_lists_every_missed_mutant(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = write_run(
                Path(tmp),
                {
                    "caught": 3,
                    "missed": 2,
                    "timeout": 1,
                    "unviable": 0,
                    "total_mutants": 6,
                    "cargo_mutants_version": "27.1.0",
                    "start_time": "2026-11-01T00:00:00Z",
                    "end_time": "2026-11-01T01:00:00Z",
                },
                [
                    "crates/interprocess/src/layout.rs:110:43: replace * with +",
                    "crates/renderide-shared/src/packing/bit_span.rs:33:9: replace BitSpanMut::len -> usize with 0",
                ],
            )
            text = summary.summarize(out)
        self.assertIn("- caught: 3", text)
        self.assertIn("- missed: 2", text)
        self.assertIn("- timeout: 1", text)
        self.assertIn("- unviable: 0", text)
        self.assertIn("50.00% (3/6)", text)
        self.assertIn("replace * with +", text)
        self.assertIn("replace BitSpanMut::len -> usize with 0", text)
        self.assertIn("cargo-mutants: 27.1.0", text)

    def test_empty_missed_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = write_run(
                Path(tmp),
                {
                    "caught": 4,
                    "missed": 0,
                    "timeout": 0,
                    "unviable": 1,
                    "total_mutants": 5,
                },
                [],
            )
            text = summary.summarize(out)
        self.assertIn("100.00% (4/4)", text)
        self.assertIn("None.", text)

    def test_missing_outcomes_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(summary.SummaryError):
                summary.summarize(Path(tmp))

    def test_inconsistent_counts_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = write_run(
                Path(tmp),
                {"caught": 1, "missed": 1, "timeout": 0, "unviable": 0, "total_mutants": 9},
                ["one"],
            )
            with self.assertRaises(summary.SummaryError):
                summary.summarize(out)

    def test_main_writes_step_summary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            out = write_run(
                root,
                {"caught": 1, "missed": 0, "timeout": 0, "unviable": 0, "total_mutants": 1},
                None,
            )
            step = root / "step.md"
            old = summary.os.environ.get("GITHUB_STEP_SUMMARY")
            summary.os.environ["GITHUB_STEP_SUMMARY"] = str(step)
            try:
                code = summary.main(["--mutants-out", str(out)])
            finally:
                if old is None:
                    summary.os.environ.pop("GITHUB_STEP_SUMMARY", None)
                else:
                    summary.os.environ["GITHUB_STEP_SUMMARY"] = old
            self.assertEqual(code, 0)
            self.assertIn("- caught: 1", step.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
