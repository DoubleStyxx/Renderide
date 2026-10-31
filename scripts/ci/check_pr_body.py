#!/usr/bin/env python3
"""Fail a pull request whose body drops the required sections or leaves the template.

Required headings, each followed by text the author wrote:

- ``## Verification evidence``
- ``## Pillars advanced``

The template markers ``pr-body-lint:placeholder`` and ``TODO(pr-body):`` are failures
anywhere outside a fenced code block. An empty section is a failure too. Fenced code
is ignored so a pull request can quote the template.

The workflow passes the Actions event payload (``GITHUB_EVENT_PATH``) via ``--event``.
The body is ``pull_request.body`` in that JSON. This script does not call the GitHub API.

Run: ``python3 scripts/ci/check_pr_body.py --event "$GITHUB_EVENT_PATH"``
Unit tests: ``python3 -m unittest discover -s scripts/ci -p 'test_check_pr_body.py'``
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REQUIRED_TITLES: tuple[str, ...] = (
    "Verification evidence",
    "Pillars advanced",
)

# Strings the pull request template leaves for the author to delete.
PLACEHOLDER_MARKERS: tuple[str, ...] = (
    "pr-body-lint:placeholder",
    "TODO(pr-body):",
)

_ATX = re.compile(r"^[ ]{0,3}(#{1,6})[ \t]+(.*?)[ \t]*$")
_HTML_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)
_TRAILING_HASHES = re.compile(r"\s+#+\s*$")


def normalize(text: str) -> str:
    """Normalize newlines and drop a leading BOM."""
    if text.startswith("\ufeff"):
        text = text[1:]
    return text.replace("\r\n", "\n").replace("\r", "\n")


def _fence_marker(line: str) -> str | None:
    stripped = line.lstrip(" ")
    if len(line) - len(stripped) > 3:
        return None
    if stripped.startswith("```"):
        return "```"
    if stripped.startswith("~~~"):
        return "~~~"
    return None


def strip_fenced_code(text: str) -> str:
    """Drop fenced code blocks. An unclosed fence drops the rest of the body."""
    kept: list[str] = []
    fence: str | None = None
    for line in text.split("\n"):
        marker = _fence_marker(line)
        if fence is None:
            if marker is not None:
                fence = marker
                continue
            kept.append(line)
        elif marker == fence:
            fence = None
    return "\n".join(kept)


def classify_heading(line: str) -> tuple[int, str] | None:
    """Return ``(level, title)`` for an ATX heading, else ``None``."""
    match = _ATX.match(line)
    if match is None:
        return None
    level = len(match.group(1))
    title = _TRAILING_HASHES.sub("", match.group(2)).strip()
    if not title:
        return None
    return level, title


def section_bodies(text: str) -> tuple[dict[str, str], list[str]]:
    """Map each required title to the body under its first heading.

    A second copy of a required heading is an error. The first body is kept.
    """
    bodies: dict[str, str] = {}
    errors: list[str] = []
    current: str | None = None
    buf: list[str] = []

    def flush() -> None:
        nonlocal current, buf
        if current is None:
            buf = []
            return
        bodies[current] = "\n".join(buf).strip("\n")
        buf = []
        current = None

    for line in text.split("\n"):
        classified = classify_heading(line)
        if classified is None:
            if current is not None:
                buf.append(line)
            continue
        level, title = classified
        flush()
        if level == 2 and title in REQUIRED_TITLES:
            if title in bodies:
                errors.append(f"duplicate heading '## {title}'")
                current = None
            else:
                current = title
    flush()
    return bodies, errors


def has_substantive_text(body: str) -> bool:
    """True when the section has a non-whitespace line outside HTML comments."""
    visible = _HTML_COMMENT.sub("", body)
    return any(line.strip() for line in visible.split("\n"))


def lint_pr_body(body: str) -> list[str]:
    """Return human-readable errors. An empty list means the body passes."""
    unfenced = strip_fenced_code(normalize(body))
    errors: list[str] = []
    for marker in PLACEHOLDER_MARKERS:
        if marker in unfenced:
            errors.append(
                f"template placeholder {marker!r} is still in the pull request body"
            )
    bodies, dup_errors = section_bodies(unfenced)
    errors.extend(dup_errors)
    for title in REQUIRED_TITLES:
        heading = f"## {title}"
        if title not in bodies:
            errors.append(f"missing heading '{heading}'")
            continue
        if not has_substantive_text(bodies[title]):
            errors.append(f"heading '{heading}' has no text under it")
    return errors


def body_from_event(payload: object) -> str:
    """Return ``pull_request.body``. Raise ``ValueError`` when the payload is unusable."""
    if not isinstance(payload, dict):
        raise ValueError("event payload is not a JSON object")
    pull_request = payload.get("pull_request")
    if not isinstance(pull_request, dict):
        raise ValueError("event payload has no pull_request object")
    body = pull_request.get("body")
    if body is None:
        return ""
    if not isinstance(body, str):
        raise ValueError("pull_request.body is not a string")
    return body


def load_event_body(path: Path) -> str:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ValueError(f"cannot read event file: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"event file is not JSON: {exc}") from exc
    return body_from_event(payload)


def report(errors: list[str]) -> int:
    if errors:
        print("pr-body lint FAILED:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1
    print("pr-body lint OK")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument(
        "--event",
        type=Path,
        help="GitHub Actions event JSON (GITHUB_EVENT_PATH)",
    )
    source.add_argument(
        "--body-file",
        type=Path,
        help="UTF-8 file containing the pull request body",
    )
    args = parser.parse_args(argv)
    try:
        if args.event is not None:
            body = load_event_body(args.event)
        else:
            body = args.body_file.read_text(encoding="utf-8")
    except (OSError, ValueError) as exc:
        print(f"pr-body lint usage error: {exc}", file=sys.stderr)
        return 2
    return report(lint_pr_body(body))


if __name__ == "__main__":
    sys.exit(main())
