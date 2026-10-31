#!/usr/bin/env python3
"""Fail the ``required`` job unless every applicable suite job succeeded.

The job always runs (``if: always()``). GitHub still reports a job skipped by
``if:`` as a successful check for branch protection, and ``needs.<id>.result``
is ``skipped`` both for a path-filter skip and for a skip caused by an upstream
failure. Those two skips are not the same.

``changes`` publishes ``suites=true`` or ``suites=false``:

- ``false`` and ``changes`` succeeded: each covered job may be ``success`` or
  ``skipped``. The skip is the path filter. ``failure`` and ``cancelled`` still
  fail this job.
- anything else: each covered job must be ``success``. A ``skipped`` result is
  an upstream failure or cancellation, not a path skip.

``changes`` itself must be ``success``. A missing or non-boolean ``suites``
output fails closed, so an unknown diff cannot turn the gate green.
"""

from __future__ import annotations

import json
import os
import sys

# Job ids in ci.yml. The Rust matrix (ubuntu-24.04 lavapipe goldens,
# windows-latest WARP, macos-latest) is one id. The .NET matrix is the other.
# macos-capacity runs only after an applicable Rust matrix; it is covered so
# its own failure or cancellation cannot hide behind the aggregate, and so a
# skip caused by Rust failing is not treated as a path skip.
COVERED_JOBS: tuple[str, ...] = ("rust", "dotnet", "macos-capacity")

# Jobs that are not path-filtered. A skip is never acceptable for these.
# `changes` is handled on its own because the gate also reads its output.
# Add a job id here when it must run on every pull request and every master
# push, including docs-only diffs.
ALWAYS_JOBS: tuple[str, ...] = ()


def evaluate(needs: object) -> tuple[int, list[str]]:
    """Return ``(exit_code, report_lines)`` for a ``toJson(needs)`` object."""
    lines: list[str] = []
    problems: list[str] = []
    if not isinstance(needs, dict):
        return 1, ["needs payload is not an object"]

    changes = needs.get("changes")
    changes_result = "missing"
    suites = ""
    if not isinstance(changes, dict):
        problems.append("changes job is missing from needs")
    else:
        raw_result = changes.get("result", "")
        changes_result = raw_result if isinstance(raw_result, str) and raw_result else "missing"
        outputs = changes.get("outputs")
        raw_suites: object = ""
        if isinstance(outputs, dict):
            raw_suites = outputs.get("suites", "")
        suites = raw_suites if isinstance(raw_suites, str) else ""
        if changes_result != "success":
            problems.append(f"changes result is {changes_result!r}, expected 'success'")
        if suites not in {"true", "false"}:
            shown = raw_suites if isinstance(raw_suites, str) else type(raw_suites).__name__
            problems.append(f"changes output suites is {shown!r}, expected 'true' or 'false'")

    # A path skip is the only skipped result that may pass, and only when the
    # detector actually succeeded and said the suites do not apply.
    allow_skip = changes_result == "success" and suites == "false"
    if allow_skip:
        lines.append("path filter: suites do not apply; skipped suite jobs are ok")
    elif suites == "true" and changes_result == "success":
        lines.append("path filter: suites apply; every covered job must succeed")
    else:
        lines.append("path filter: unknown; failing closed (skipped is not ok)")

    lines.append(f"changes: {changes_result} suites={suites or 'missing'}")
    for job_id in ALWAYS_JOBS:
        result = _job_result(needs, job_id)
        lines.append(f"{job_id}: {result}")
        if result != "success":
            problems.append(
                f"{job_id} result is {result!r}, expected 'success'. "
                "This job is not path-filtered, so skipped is not ok."
            )
    for job_id in COVERED_JOBS:
        result = _job_result(needs, job_id)
        lines.append(f"{job_id}: {result}")
        if result not in {"success", "failure", "cancelled", "skipped"}:
            problems.append(f"{job_id} result is {result!r}")
            continue
        if allow_skip:
            if result not in {"success", "skipped"}:
                problems.append(
                    f"{job_id} result is {result!r}; failure and cancellation "
                    "fail the gate even when the path filter skips the suites"
                )
        elif result != "success":
            if result == "skipped":
                problems.append(
                    f"{job_id} result is 'skipped' but the path filter did not "
                    "skip the suites. A skip here came from an upstream failure "
                    "or cancellation, not from path detection."
                )
            else:
                problems.append(f"{job_id} result is {result!r}, expected 'success'")

    if problems:
        lines.append("required: failure")
        lines.extend(f"error: {problem}" for problem in problems)
        return 1, lines
    lines.append("required: success")
    return 0, lines


def _job_result(needs: dict[str, object], job_id: str) -> str:
    job = needs.get(job_id)
    if not isinstance(job, dict):
        return "missing"
    result = job.get("result", "")
    if isinstance(result, str) and result:
        return result
    return "missing"


def format_report(lines: list[str]) -> str:
    return "\n".join(lines) + "\n"


def write_summary(report: str) -> None:
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not path:
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write("## required\n\n```\n")
        handle.write(report)
        handle.write("```\n")


def main() -> int:
    raw = os.environ.get("NEEDS_JSON", "")
    try:
        payload = json.loads(raw) if raw else None
    except json.JSONDecodeError as exc:
        print(f"NEEDS_JSON is not valid JSON: {exc}", file=sys.stderr)
        return 1
    code, lines = evaluate(payload)
    report = format_report(lines)
    sys.stdout.write(report)
    write_summary(report)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
