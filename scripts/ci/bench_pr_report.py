#!/usr/bin/env python3
"""Build the bench-pr combined summary and optional bench-compare.json.

Extracted from `.github/workflows/bench-pr.yml` report job so missing-artifact
paths are unit-tested: gate artifact missing, cpu artifact missing, both missing.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import bench_pr_assemble as assemble


FRAME_SUMMARY_MISSING = "_frame gate artifact missing_"
CPU_SUMMARY_MISSING = (
    "_cpu_hot artifact missing (job may have been skipped or failed before upload)_"
)
GATE_JSON_MISSING = "gate.json missing; cannot assemble bench-compare.json"


def build_combined_summary(
    *,
    frame_job_result: str,
    gate_exit: str,
    wall_time_sec: str,
    cpu_job_result: str,
    frame_summary: str | None,
    cpu_summary: str | None,
    gate_json_present: bool,
) -> str:
    """Return the markdown written to bench-pr-summary.md / step summary."""
    lines = [
        "# bench-pr (combined)",
        "",
        f"- frame job: `{frame_job_result}` (gate_exit={gate_exit}, wall_time_sec={wall_time_sec})",
        f"- cpu_hot job: `{cpu_job_result}` (report-only; never gates)",
        "- decision rule: `scripts/ci/frame_bench_gate.py` only",
        "",
    ]
    if frame_summary is not None:
        lines.append(frame_summary.rstrip("\n"))
        lines.append("")
    else:
        lines.append(FRAME_SUMMARY_MISSING)
        lines.append("")

    if cpu_summary is not None:
        lines.append(cpu_summary.rstrip("\n"))
        lines.append("")
    else:
        lines.append(CPU_SUMMARY_MISSING)
        lines.append("")

    if not gate_json_present:
        lines.append(GATE_JSON_MISSING)
        lines.append("")

    return "\n".join(lines)


def read_optional_text(path: Path) -> str | None:
    if path.is_file():
        return path.read_text(encoding="utf-8")
    return None


def run_report(
    *,
    gate_art: Path,
    cpu_art: Path,
    summary_out: Path,
    compare_out: Path,
    frame_job_result: str,
    gate_exit: str,
    wall_time_sec: str,
    cpu_job_result: str,
) -> int:
    """Write summary_out; assemble compare_out when gate.json exists.

    Always returns 0 after writing the summary (report job must not fail the
    workflow on missing optional artifacts). Returns 2 only for usage errors
    before any write (caller / CLI).
    """
    frame_summary = read_optional_text(gate_art / "frame-summary.md")
    cpu_summary = read_optional_text(cpu_art / "cpu-summary.md")
    gate_json = gate_art / "gate.json"
    cpu_compare = cpu_art / "cpu-compare.json"
    gate_present = gate_json.is_file()

    summary = build_combined_summary(
        frame_job_result=frame_job_result,
        gate_exit=gate_exit,
        wall_time_sec=wall_time_sec,
        cpu_job_result=cpu_job_result,
        frame_summary=frame_summary,
        cpu_summary=cpu_summary,
        gate_json_present=gate_present,
    )
    summary_out.parent.mkdir(parents=True, exist_ok=True)
    summary_out.write_text(summary, encoding="utf-8")

    if gate_present:
        argv = ["--gate", str(gate_json), "--out", str(compare_out)]
        if cpu_compare.is_file():
            argv.extend(["--cpu-compare", str(cpu_compare)])
        rc = assemble.main(argv)
        if rc != 0:
            # Assemble refused (bad JSON/schema). Keep summary; still exit 0 so
            # the comment/step summary land. Note already in summary if we add one.
            print(f"bench_pr_report: assemble exited {rc}", file=sys.stderr)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--gate-art", type=Path, required=True)
    parser.add_argument("--cpu-art", type=Path, required=True)
    parser.add_argument("--summary-out", type=Path, required=True)
    parser.add_argument("--compare-out", type=Path, required=True)
    parser.add_argument("--frame-job-result", required=True)
    parser.add_argument("--gate-exit", default="")
    parser.add_argument("--wall-time-sec", default="")
    parser.add_argument("--cpu-job-result", required=True)
    args = parser.parse_args(argv)
    return run_report(
        gate_art=args.gate_art,
        cpu_art=args.cpu_art,
        summary_out=args.summary_out,
        compare_out=args.compare_out,
        frame_job_result=args.frame_job_result,
        gate_exit=args.gate_exit,
        wall_time_sec=args.wall_time_sec,
        cpu_job_result=args.cpu_job_result,
    )


if __name__ == "__main__":
    sys.exit(main())
