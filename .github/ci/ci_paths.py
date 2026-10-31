#!/usr/bin/env python3
"""Decide whether the Rust and .NET suites apply to a workflow diff.

The patterns are the historical ``ci.yml`` trigger filters, plus
``.github/ci/**`` and the exact path ``docs/testing/material-coverage.md``.
``.github/ci/**`` holds the scripts the suite jobs run and this gate. The
material-coverage document is the one docs path that must run the suites: a
path skip is a ``required`` success, so leaving that file out would let a
docs-only drift go green. The workflow trigger itself no longer uses a path
filter: a trigger filter never creates check runs, so a required check would
stay pending. This script runs inside the always-on ``changes`` job and
publishes ``suites=true`` or ``suites=false``.

``workflow_dispatch``, an unrecognized event, a missing parent SHA, or a diff
that cannot be computed all publish ``suites=true``. Skipping the suites is
allowed only when the diff was read and none of it matches.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

# Exact repository-root paths, or ``dir/**`` (that directory and everything
# under it). This is the subset of GitHub path-filter syntax the old trigger
# used. ``?`` and ``*`` are intentionally unsupported so a new pattern cannot
# silently match less than the workflow author meant.
SUITE_PATTERNS: tuple[str, ...] = (
    ".github/workflows/ci.yml",
    ".github/actions/lavapipe/**",
    ".github/ci/**",
    ".taplo.toml",
    "Cargo.lock",
    "Cargo.toml",
    "Generators.sln",
    "clippy.toml",
    "crates/**",
    "generators/**",
    "third_party/**",
    "RenderideMod/Shaders/**",
    # Exact file, not docs/**. Other docs stay skipped. A path skip is a
    # required success, so this file has to select the suite on its own.
    "docs/testing/material-coverage.md",
)

SHA_RE = re.compile(r"^[0-9a-fA-F]{40}$")
ZERO_SHA = "0" * 40


class DiffUnavailable(Exception):
    """The changed-file list could not be trusted. Callers run every suite."""


def match_pattern(path: str, pattern: str) -> bool:
    """Return whether ``path`` matches one historical GitHub path filter."""
    if path.startswith("/") or "\\" in path or "\n" in path or "\0" in path:
        return False
    if pattern.endswith("/**"):
        root = pattern[: -len("/**")]
        if not root or any(token in root for token in ("*", "?", "[")):
            raise ValueError(f"unsupported path pattern: {pattern}")
        return path.startswith(root + "/")
    if any(token in pattern for token in ("*", "?", "[")):
        raise ValueError(f"unsupported path pattern: {pattern}")
    return path == pattern


def path_selects_suite(path: str) -> bool:
    return any(match_pattern(path, pattern) for pattern in SUITE_PATTERNS)


def diff_selects_suites(paths: list[str]) -> bool:
    return any(path_selects_suite(path) for path in paths)


def suites_output(event_name: str, changed: list[str] | None) -> str:
    """Return ``true`` or ``false`` for the ``suites`` job output.

    ``changed is None`` means the diff is unknown. The suites run.
    """
    if changed is None:
        return "true"
    if event_name not in {"pull_request", "push"}:
        return "true"
    return "true" if diff_selects_suites(changed) else "false"


def paths_from_name_status_z(blob: bytes) -> list[str]:
    """Parse ``git diff --name-status -z`` and return every path it names.

    Rename and copy records contribute both the old and the new path, so a
    move out of ``crates/`` still selects the suites.
    """
    if not blob:
        return []
    parts = blob.split(b"\0")
    if parts and parts[-1] == b"":
        parts.pop()
    paths: list[str] = []
    index = 0
    while index < len(parts):
        try:
            status = parts[index].decode("utf-8", "strict")
        except UnicodeDecodeError as exc:
            raise DiffUnavailable("git status record is not utf-8") from exc
        index += 1
        width = 2 if _is_rename_or_copy(status) else 1
        if index + width > len(parts):
            raise DiffUnavailable("truncated git name-status record")
        for offset in range(width):
            try:
                path = parts[index + offset].decode("utf-8", "strict")
            except UnicodeDecodeError as exc:
                raise DiffUnavailable("git path is not utf-8") from exc
            paths.append(path)
        index += width
    return paths


def _is_rename_or_copy(status: str) -> bool:
    if len(status) < 2 or status[0] not in {"R", "C"}:
        return False
    score = status[1:]
    return score.isdigit()


def validate_sha(value: str) -> str:
    if not SHA_RE.fullmatch(value):
        raise DiffUnavailable("refusing to pass a non-SHA to git")
    if value.lower() == ZERO_SHA:
        raise DiffUnavailable("SHA is all zeros")
    return value.lower()


def git_diff(left: str, right: str, *, cwd: str | None = None) -> list[str]:
    """List paths that differ between two commits. Both arguments are SHAs."""
    left_sha = validate_sha(left)
    right_sha = validate_sha(right)
    completed = subprocess.run(
        ["git", "diff", "--name-status", "-z", left_sha, right_sha],
        cwd=cwd,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", "replace").strip()
        raise DiffUnavailable(detail or f"git diff exited {completed.returncode}")
    return paths_from_name_status_z(completed.stdout)


def changed_files(
    event_name: str,
    *,
    head: str,
    base: str,
    before: str,
    cwd: str | None = None,
) -> list[str] | None:
    """Return changed paths, or None when the suites must run unconditionally."""
    if event_name == "workflow_dispatch":
        return None
    if event_name == "pull_request":
        return git_diff(base, head, cwd=cwd)
    if event_name == "push":
        if not before or before.lower() == ZERO_SHA:
            return None
        return git_diff(before, head, cwd=cwd)
    return None


def write_output(name: str, value: str) -> None:
    if value not in {"true", "false"}:
        raise RuntimeError("refusing to publish a non-boolean suites output")
    line = f"{name}={value}\n"
    path = os.environ.get("GITHUB_OUTPUT")
    if not path:
        print(line, end="")
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(line)


def decide(
    event_name: str,
    *,
    head: str,
    base: str,
    before: str,
    cwd: str | None = None,
) -> tuple[str, list[str] | None, str]:
    """Return ``(suites, paths, reason)``.

    ``paths`` is None when the diff was not used. ``reason`` is a short log line.
    """
    try:
        changed = changed_files(event_name, head=head, base=base, before=before, cwd=cwd)
    except DiffUnavailable as exc:
        return "true", None, f"diff unavailable ({exc}); running suites"
    suites = suites_output(event_name, changed)
    if changed is None:
        return suites, None, f"event {event_name or 'empty'} runs every suite"
    if suites == "true":
        return suites, changed, f"{len(changed)} path(s); suites apply"
    return suites, changed, f"{len(changed)} path(s); no suite path"


def main() -> int:
    event_name = os.environ.get("EVENT_NAME", "")
    suites, changed, reason = decide(
        event_name,
        head=os.environ.get("HEAD_SHA", ""),
        base=os.environ.get("BASE_SHA", ""),
        before=os.environ.get("BEFORE_SHA", ""),
    )
    print(f"suites={suites}")
    print(reason)
    if changed is not None:
        for path in changed:
            print(f"path: {path}")
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as handle:
            handle.write(f"## Path filter\n\n- suites: `{suites}`\n- {reason}\n")
    write_output("suites", suites)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"ci_paths failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
