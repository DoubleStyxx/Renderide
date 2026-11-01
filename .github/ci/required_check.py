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

``required`` does not ``needs`` the suite jobs. A concurrency cancellation
marks the workflow run cancelled before a job that is still waiting on
``needs`` is created, so that job never reports. This job starts as soon as
``changes`` finishes and polls the run's jobs until they reach a conclusion,
including ``cancelled``. The step uses ``if: always()`` so a cancellation
that arrives while the poll is running does not skip the conclusion.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time

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


TERMINAL_RESULTS = frozenset({"success", "failure", "cancelled", "skipped"})


def aggregate_conclusions(conclusions: list[str]) -> str:
    """Combine matrix legs the way the gate reads one job id.

    ``failure`` wins over ``cancelled``. An unfinished leg stays ``pending``.
    An empty group is ``pending`` so the waiter keeps polling until the job
    exists.
    """
    if not conclusions or any(item not in TERMINAL_RESULTS for item in conclusions):
        return "pending"
    if any(item == "failure" for item in conclusions):
        return "failure"
    if any(item == "cancelled" for item in conclusions):
        return "cancelled"
    if all(item == "skipped" for item in conclusions):
        return "skipped"
    if all(item == "success" for item in conclusions):
        return "success"
    return "failure"


def covered_job_id(name: str) -> str | None:
    if name == "macOS capacity":
        return "macos-capacity"
    if name.startswith("Rust /"):
        return "rust"
    if name.startswith(".NET /"):
        return "dotnet"
    return None


def job_conclusion(job: dict[str, object]) -> str:
    if job.get("status") != "completed":
        return "pending"
    conclusion = job.get("conclusion")
    if isinstance(conclusion, str) and conclusion in TERMINAL_RESULTS:
        return conclusion
    return "pending"


def group_conclusions(jobs: list[dict[str, object]]) -> dict[str, list[str]]:
    groups: dict[str, list[str]] = {job_id: [] for job_id in COVERED_JOBS}
    for job in jobs:
        name = job.get("name")
        if not isinstance(name, str):
            continue
        job_id = covered_job_id(name)
        if job_id is None:
            continue
        groups[job_id].append(job_conclusion(job))
    return groups


def needs_from_groups(
    groups: dict[str, list[str]],
    *,
    changes_result: str,
    suites: str,
) -> dict[str, object]:
    payload: dict[str, object] = {
        "changes": {"result": changes_result, "outputs": {"suites": suites}},
    }
    for job_id in COVERED_JOBS:
        payload[job_id] = {"result": aggregate_conclusions(groups.get(job_id, []))}
    return payload


def suites_ready(groups: dict[str, list[str]]) -> bool:
    return all(aggregate_conclusions(groups.get(job_id, [])) != "pending" for job_id in COVERED_JOBS)


def list_run_jobs(repository: str, run_id: str) -> list[dict[str, object]]:
    completed = subprocess.run(
        [
            "gh",
            "api",
            f"repos/{repository}/actions/runs/{run_id}/jobs?per_page=100",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        raise RuntimeError(detail or f"gh api exited {completed.returncode}")
    payload = json.loads(completed.stdout)
    jobs = payload.get("jobs", [])
    if not isinstance(jobs, list):
        raise RuntimeError("jobs payload is not a list")
    return [job for job in jobs if isinstance(job, dict)]


def wait_for_needs(
    *,
    changes_result: str,
    suites: str,
    list_jobs,
    sleep,
    now,
    deadline: float,
) -> dict[str, object]:
    """Poll until every covered job has a conclusion, or the deadline passes."""
    groups: dict[str, list[str]] = {job_id: [] for job_id in COVERED_JOBS}
    while True:
        try:
            groups = group_conclusions(list_jobs())
        except (RuntimeError, json.JSONDecodeError, OSError) as exc:
            print(f"waiting for job results: {exc}", file=sys.stderr)
        else:
            summary = ", ".join(
                f"{job_id}={aggregate_conclusions(groups.get(job_id, []))}" for job_id in COVERED_JOBS
            )
            print(f"waiting: {summary}")
            if suites_ready(groups):
                break
        if now() >= deadline:
            print("timed out waiting for suite jobs", file=sys.stderr)
            break
        sleep(20)
    return needs_from_groups(groups, changes_result=changes_result, suites=suites)


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


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if "--wait" in args:
        repository = os.environ["GITHUB_REPOSITORY"]
        run_id = os.environ["GITHUB_RUN_ID"]
        # The suite can sit in the macOS queue for a long time. The job's own
        # timeout is the hard stop; this deadline fails closed shortly before
        # a 6 hour hosted-runner job limit.
        deadline = time.time() + 350 * 60
        payload = wait_for_needs(
            changes_result=os.environ.get("CHANGES_RESULT", ""),
            suites=os.environ.get("SUITES", ""),
            list_jobs=lambda: list_run_jobs(repository, run_id),
            sleep=time.sleep,
            now=time.time,
            deadline=deadline,
        )
    else:
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
