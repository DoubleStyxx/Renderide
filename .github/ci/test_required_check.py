"""Unit tests for required_check.py."""

from __future__ import annotations

import json
import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import required_check  # noqa: E402


def _needs(
    *,
    changes: str = "success",
    suites: str | None = "false",
    rust: str = "skipped",
    dotnet: str = "skipped",
    macos: str = "skipped",
    suites_type: str = "str",
) -> dict:
    if suites_type == "missing":
        outputs: dict = {}
    elif suites_type == "bool":
        outputs = {"suites": True}
    elif suites is None:
        outputs = {"suites": None}
    else:
        outputs = {"suites": suites}
    return {
        "changes": {"result": changes, "outputs": outputs},
        "rust": {"result": rust, "outputs": {}},
        "dotnet": {"result": dotnet, "outputs": {}},
        "macos-capacity": {"result": macos, "outputs": {}},
    }


class EvaluateTests(unittest.TestCase):
    def test_docs_only_skip_passes(self) -> None:
        code, lines = required_check.evaluate(_needs())
        self.assertEqual(code, 0)
        self.assertIn("required: success", lines)

    def test_docs_only_allows_a_job_that_still_succeeded(self) -> None:
        code, _lines = required_check.evaluate(_needs(dotnet="success"))
        self.assertEqual(code, 0)

    def test_full_suite_success_passes(self) -> None:
        code, lines = required_check.evaluate(
            _needs(suites="true", rust="success", dotnet="success", macos="success")
        )
        self.assertEqual(code, 0)
        self.assertIn("required: success", lines)

    def test_rust_failure_fails(self) -> None:
        code, lines = required_check.evaluate(
            _needs(suites="true", rust="failure", dotnet="success", macos="success")
        )
        self.assertEqual(code, 1)
        self.assertTrue(any("rust result is 'failure'" in line for line in lines))

    def test_dotnet_failure_fails(self) -> None:
        code, _lines = required_check.evaluate(
            _needs(suites="true", rust="success", dotnet="failure", macos="success")
        )
        self.assertEqual(code, 1)

    def test_cancelled_rust_fails_even_if_note_job_succeeded(self) -> None:
        code, lines = required_check.evaluate(
            _needs(suites="true", rust="cancelled", dotnet="success", macos="success")
        )
        self.assertEqual(code, 1)
        self.assertTrue(any("rust result is 'cancelled'" in line for line in lines))

    def test_cancelled_macos_capacity_fails(self) -> None:
        code, _lines = required_check.evaluate(
            _needs(suites="true", rust="success", dotnet="success", macos="cancelled")
        )
        self.assertEqual(code, 1)

    def test_upstream_skip_is_not_a_path_skip(self) -> None:
        code, lines = required_check.evaluate(
            _needs(suites="true", rust="failure", dotnet="success", macos="skipped")
        )
        self.assertEqual(code, 1)
        self.assertTrue(any("macos-capacity result is 'skipped'" in line for line in lines))
        self.assertTrue(any("upstream failure" in line for line in lines))

    def test_skipped_suite_job_fails_when_suites_apply(self) -> None:
        code, lines = required_check.evaluate(
            _needs(suites="true", rust="skipped", dotnet="success", macos="success")
        )
        self.assertEqual(code, 1)
        self.assertTrue(any("path filter did not" in line for line in lines))

    def test_failure_is_not_ok_on_a_docs_diff(self) -> None:
        code, _lines = required_check.evaluate(_needs(rust="failure"))
        self.assertEqual(code, 1)

    def test_cancellation_is_not_ok_on_a_docs_diff(self) -> None:
        code, _lines = required_check.evaluate(_needs(dotnet="cancelled"))
        self.assertEqual(code, 1)

    def test_changes_failure_fails_closed(self) -> None:
        code, _lines = required_check.evaluate(
            _needs(changes="failure", suites="false", rust="skipped", dotnet="skipped", macos="skipped")
        )
        self.assertEqual(code, 1)

    def test_missing_suites_output_fails_closed(self) -> None:
        code, _lines = required_check.evaluate(_needs(suites_type="missing", rust="skipped"))
        self.assertEqual(code, 1)

    def test_boolean_suites_output_fails_closed(self) -> None:
        code, lines = required_check.evaluate(_needs(suites_type="bool", rust="success", dotnet="success", macos="success"))
        self.assertEqual(code, 1)
        self.assertTrue(any("suites" in line for line in lines))

    def test_changes_cancelled_fails(self) -> None:
        code, _lines = required_check.evaluate(
            _needs(changes="cancelled", suites="true", rust="cancelled", dotnet="cancelled", macos="cancelled")
        )
        self.assertEqual(code, 1)

    def test_missing_payload_fails(self) -> None:
        code, lines = required_check.evaluate(None)
        self.assertEqual(code, 1)
        self.assertIn("needs payload is not an object", lines)

    def test_missing_job_fails_when_suites_apply(self) -> None:
        payload = _needs(suites="true", rust="success", dotnet="success", macos="success")
        del payload["dotnet"]
        code, lines = required_check.evaluate(payload)
        self.assertEqual(code, 1)
        self.assertTrue(any("dotnet result is 'missing'" in line for line in lines))

    def test_always_job_skip_fails_on_a_docs_diff(self) -> None:
        payload = _needs()
        payload["lint"] = {"result": "skipped", "outputs": {}}
        with mock.patch.object(required_check, "ALWAYS_JOBS", ("lint",)):
            code, lines = required_check.evaluate(payload)
        self.assertEqual(code, 1)
        self.assertTrue(any("lint result is 'skipped'" in line for line in lines))

    def test_aggregate_failure_wins_over_cancelled(self) -> None:
        self.assertEqual(required_check.aggregate_conclusions(["success", "cancelled", "failure"]), "failure")

    def test_aggregate_cancelled_with_success(self) -> None:
        self.assertEqual(required_check.aggregate_conclusions(["success", "cancelled"]), "cancelled")

    def test_aggregate_pending_until_complete(self) -> None:
        self.assertEqual(required_check.aggregate_conclusions([]), "pending")
        self.assertEqual(required_check.aggregate_conclusions(["success", "pending"]), "pending")

    def test_group_matrix_and_unexpanded_skip(self) -> None:
        jobs = [
            {"name": "Rust / ubuntu-24.04", "status": "completed", "conclusion": "cancelled"},
            {"name": "Rust / windows-latest", "status": "completed", "conclusion": "success"},
            {"name": ".NET / ${{ matrix.os }}", "status": "completed", "conclusion": "skipped"},
            {"name": "macOS capacity", "status": "completed", "conclusion": "cancelled"},
            {"name": "required", "status": "in_progress", "conclusion": None},
            {"name": "changes", "status": "completed", "conclusion": "success"},
        ]
        groups = required_check.group_conclusions(jobs)
        self.assertEqual(required_check.aggregate_conclusions(groups["rust"]), "cancelled")
        self.assertEqual(required_check.aggregate_conclusions(groups["dotnet"]), "skipped")
        self.assertEqual(required_check.aggregate_conclusions(groups["macos-capacity"]), "cancelled")
        self.assertNotIn("required", groups)

    def test_wait_polls_until_cancelled_matrix_is_terminal(self) -> None:
        snapshots = [
            [
                {"name": "Rust / ubuntu-24.04", "status": "in_progress", "conclusion": None},
                {"name": ".NET / ubuntu-latest", "status": "queued", "conclusion": None},
            ],
            [
                {"name": "Rust / ubuntu-24.04", "status": "completed", "conclusion": "cancelled"},
                {"name": "Rust / windows-latest", "status": "completed", "conclusion": "success"},
                {"name": ".NET / ubuntu-latest", "status": "completed", "conclusion": "cancelled"},
                {"name": "macOS capacity", "status": "completed", "conclusion": "cancelled"},
            ],
        ]

        def list_jobs():
            return snapshots.pop(0)

        sleeps: list[float] = []
        payload = required_check.wait_for_needs(
            changes_result="success",
            suites="true",
            list_jobs=list_jobs,
            sleep=lambda seconds: sleeps.append(seconds),
            now=lambda: 0,
            deadline=10,
        )
        self.assertEqual(sleeps, [20])
        code, lines = required_check.evaluate(payload)
        self.assertEqual(code, 1)
        self.assertTrue(any("rust result is 'cancelled'" in line for line in lines))

    def test_main_reads_needs_json(self) -> None:
        raw = json.dumps(_needs(suites="true", rust="success", dotnet="success", macos="success"))
        previous = os.environ.get("NEEDS_JSON")
        os.environ["NEEDS_JSON"] = raw
        try:
            self.assertEqual(required_check.main(), 0)
        finally:
            if previous is None:
                os.environ.pop("NEEDS_JSON", None)
            else:
                os.environ["NEEDS_JSON"] = previous

    def test_main_rejects_invalid_json(self) -> None:
        previous = os.environ.get("NEEDS_JSON")
        os.environ["NEEDS_JSON"] = "{"
        try:
            self.assertEqual(required_check.main(), 1)
        finally:
            if previous is None:
                os.environ.pop("NEEDS_JSON", None)
            else:
                os.environ["NEEDS_JSON"] = previous


if __name__ == "__main__":
    unittest.main()
