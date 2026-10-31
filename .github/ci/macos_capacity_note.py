#!/usr/bin/env python3
"""Write a run summary when Rust / macos-latest is cancelled before it runs.

GitHub still marks that required check cancelled. This note is a separate
check so the run summary says the cancellation is runner capacity, not a
test failure. It does not change the macOS check's conclusion.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

JOB_NAME = "Rust / macos-latest"


def gh_json(path: str) -> object:
    completed = subprocess.run(
        ["gh", "api", path],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def jobs() -> list[dict[str, object]]:
    repository = os.environ["GITHUB_REPOSITORY"]
    run_id = os.environ["GITHUB_RUN_ID"]
    payload = gh_json(f"repos/{repository}/actions/runs/{run_id}/jobs?per_page=100")
    if not isinstance(payload, dict):
        return []
    found = payload.get("jobs", [])
    if not isinstance(found, list):
        return []
    return [job for job in found if isinstance(job, dict)]


def annotations(job: dict[str, object]) -> str:
    repository = os.environ["GITHUB_REPOSITORY"]
    job_id = job.get("id")
    if not isinstance(job_id, int):
        return ""
    try:
        detail = gh_json(f"repos/{repository}/actions/jobs/{job_id}")
    except subprocess.CalledProcessError:
        return ""
    if not isinstance(detail, dict):
        return ""
    lines: list[str] = []
    steps = detail.get("steps")
    if isinstance(steps, list):
        for step in steps:
            if not isinstance(step, dict):
                continue
            name = step.get("name")
            conclusion = step.get("conclusion")
            if name and conclusion:
                lines.append(f"{name}: {conclusion}")
    return "\n".join(lines)


def never_started(job: dict[str, object]) -> bool:
    if job.get("started_at") in (None, ""):
        return True
    if not job.get("runner_name"):
        return True
    steps = job.get("steps")
    if not isinstance(steps, list) or len(steps) == 0:
        return True
    return all(
        isinstance(step, dict) and step.get("conclusion") in (None, "cancelled", "skipped")
        for step in steps
    )


def write_summary(text: str) -> None:
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not path:
        print(text)
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(text)
        if not text.endswith("\n"):
            handle.write("\n")


def main() -> int:
    matches = [job for job in jobs() if job.get("name") == JOB_NAME]
    if not matches:
        print(f"{JOB_NAME} was not part of this run")
        return 0
    job = matches[-1]
    conclusion = job.get("conclusion")
    print(f"{JOB_NAME} conclusion: {conclusion}")
    if conclusion != "cancelled":
        return 0
    queued = never_started(job)
    if queued:
        title = "Runner capacity"
        body = (
            f"`{JOB_NAME}` was cancelled before a GitHub-hosted runner picked it up. "
            "This is a runner-capacity cancellation, not a compile or test failure. "
            "The macOS check stays required, so the run stays red until that job is rerun "
            "on a runner."
        )
    else:
        title = "macOS job cancelled"
        body = (
            f"`{JOB_NAME}` was cancelled after a runner had started it. "
            "That is not a test assertion by itself. The log above the cancellation "
            "is the failure to read."
        )
    detail = annotations(job)
    summary = f"## {title}\n\n{body}\n"
    if detail:
        summary += f"\n```\n{detail}\n```\n"
    write_summary(summary)
    print(f"::warning title={title}::{body}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        print(exc.stderr or exc, file=sys.stderr)
        print("::warning::Could not read the macOS job status to explain a cancellation.")
        raise SystemExit(0) from exc
