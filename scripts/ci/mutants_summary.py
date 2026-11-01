#!/usr/bin/env python3
"""Summarize a cargo-mutants run for the job summary.

Reads ``outcomes.json`` and ``missed.txt`` from a ``mutants.out`` directory.
Prints caught, missed, timeout, and unviable counts plus the mutation score,
then lists every missed mutant. Appends the same markdown to
``$GITHUB_STEP_SUMMARY`` when that variable is set.

The mutation score is ``caught / (caught + missed + timeout)``. Unviable
mutants did not build, so they are not in the denominator. A run with no
viable mutant has no score.

Exit 0 when ``outcomes.json`` is readable. Exit 1 when it is missing or the
counts are not integers. Missed mutants do not fail this script: the workflow
is report-only.

Run: ``python3 scripts/ci/mutants_summary.py --mutants-out mutants.out``
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path


class SummaryError(Exception):
    """The mutants output cannot be summarized."""


def _require_int(data: dict, key: str) -> int:
    value = data.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        raise SummaryError(f"outcomes.json field {key!r} is not an integer")
    if value < 0:
        raise SummaryError(f"outcomes.json field {key!r} is negative")
    return value


def load_outcomes(path: Path) -> dict:
    """Return the parsed ``outcomes.json`` object."""
    if not path.is_file():
        raise SummaryError(f"missing outcomes file: {path}")
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SummaryError(f"outcomes.json is not JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise SummaryError("outcomes.json must be an object")
    return data


def counts_from_outcomes(data: dict) -> dict[str, int]:
    """Return the four outcome counts plus the total from ``outcomes.json``."""
    counts = {
        "caught": _require_int(data, "caught"),
        "missed": _require_int(data, "missed"),
        "timeout": _require_int(data, "timeout"),
        "unviable": _require_int(data, "unviable"),
        "total": _require_int(data, "total_mutants"),
    }
    summed = counts["caught"] + counts["missed"] + counts["timeout"] + counts["unviable"]
    if summed != counts["total"]:
        raise SummaryError(
            "outcomes.json counts do not add up: "
            f"caught+missed+timeout+unviable={summed} total_mutants={counts['total']}"
        )
    return counts


def mutation_score(counts: dict[str, int]) -> float | None:
    """Return caught / (caught + missed + timeout), or None when that is zero."""
    denominator = counts["caught"] + counts["missed"] + counts["timeout"]
    if denominator == 0:
        return None
    return counts["caught"] / denominator


def load_missed(path: Path) -> list[str]:
    """Return missed-mutant lines. A missing file means there were no misses."""
    if not path.is_file():
        return []
    lines = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line:
            lines.append(line)
    return lines


def format_summary(
    counts: dict[str, int],
    missed: list[str],
    *,
    version: str,
    start_time: str,
    end_time: str,
) -> str:
    """Return the markdown job summary."""
    score = mutation_score(counts)
    denominator = counts["caught"] + counts["missed"] + counts["timeout"]
    if score is None:
        score_text = "n/a (no viable mutants)"
    else:
        score_text = f"{score * 100:.2f}% ({counts['caught']}/{denominator})"
    lines = [
        "## cargo-mutants",
        "",
        f"- cargo-mutants: {version or 'unknown'}",
        f"- start: {start_time or 'unknown'}",
        f"- end: {end_time or 'unknown'}",
        f"- caught: {counts['caught']}",
        f"- missed: {counts['missed']}",
        f"- timeout: {counts['timeout']}",
        f"- unviable: {counts['unviable']}",
        f"- total: {counts['total']}",
        f"- mutation score: {score_text}",
        "",
        "Score is caught / (caught + missed + timeout). Unviable mutants are excluded.",
        "",
        "### Missed mutants",
        "",
    ]
    if not missed:
        lines.append("None.")
    else:
        lines.append("```")
        lines.extend(missed)
        lines.append("```")
    if len(missed) != counts["missed"]:
        lines.extend(
            [
                "",
                f"missed.txt has {len(missed)} lines; outcomes.json says {counts['missed']}.",
            ]
        )
    return "\n".join(lines) + "\n"


def write_summary(text: str) -> None:
    """Print the summary and append it to the GitHub step summary when present."""
    sys.stdout.write(text)
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    with Path(summary_path).open("a", encoding="utf-8") as handle:
        handle.write(text)


def summarize(mutants_out: Path) -> str:
    """Load a mutants.out directory and return the summary markdown."""
    data = load_outcomes(mutants_out / "outcomes.json")
    counts = counts_from_outcomes(data)
    missed = load_missed(mutants_out / "missed.txt")
    version = data.get("cargo_mutants_version")
    start_time = data.get("start_time")
    end_time = data.get("end_time")
    return format_summary(
        counts,
        missed,
        version=version if isinstance(version, str) else "",
        start_time=start_time if isinstance(start_time, str) else "",
        end_time=end_time if isinstance(end_time, str) else "",
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mutants-out",
        type=Path,
        default=Path("mutants.out"),
        help="directory written by cargo-mutants (default: mutants.out)",
    )
    args = parser.parse_args(argv)
    try:
        text = summarize(args.mutants_out)
    except SummaryError as exc:
        print(f"mutants summary FAILED: {exc}", file=sys.stderr)
        return 1
    write_summary(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
