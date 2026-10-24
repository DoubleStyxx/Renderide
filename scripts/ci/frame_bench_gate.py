#!/usr/bin/env python3
"""Noise-aware gate for the `Frame bench` workflow (.github/workflows/frame-bench.yml).

Reads schema-1 `renderide-framebench run` reports. It does not import renderer code, so the
same script gates any base/head pair whose reports share schema 1.

Gate metric: `cpu_frame_ms.p95`. For each side, every report contributes its
`per_run_cpu_frame_ms_p95` list. The side's value is the median of all those per-run p95s.

A scenario regresses only when both hold:

1. `delta = (head - base) / base * 100` is strictly greater than `--threshold`.
2. `delta` is strictly greater than the base's own run-to-run spread,
   `(max - min) / median * 100` over the base per-run p95s.

Rows:

- `ok`: no regression.
- `FAIL`: regression (in the first pass this means "suspect"; see `--suspects-out`).
- `base N/A`: scenario measured on head, not defined on base. Never fails.
- `head N/A, skipped`: scenario requested but not defined on head (`--head-missing`). Never fails.

Exit status: 0 pass, 1 regression, 2 usage or input error.
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from dataclasses import dataclass
from pathlib import Path

SCHEMA = 1


@dataclass
class Row:
    scenario: str
    base: float | None
    head: float | None
    delta: float | None
    spread: float | None
    status: str


def load(paths: list[Path]) -> dict[str, list[float]]:
    """Scenario name -> pooled per-run cpu p95 samples across all `paths`."""
    pooled: dict[str, list[float]] = {}
    for path in paths:
        report = json.loads(path.read_text(encoding="utf-8"))
        if report.get("schema") != SCHEMA:
            raise ValueError(f"{path}: schema {report.get('schema')!r}, expected {SCHEMA}")
        for scenario in report.get("scenarios", []):
            samples = scenario.get("per_run_cpu_frame_ms_p95") or [scenario["cpu_frame_ms"]["p95"]]
            pooled.setdefault(scenario["name"], []).extend(float(v) for v in samples)
    return pooled


def spread_percent(samples: list[float]) -> float | None:
    mid = statistics.median(samples)
    if mid <= 0:
        return None
    return (max(samples) - min(samples)) / mid * 100.0


def evaluate(
    base: dict[str, list[float]],
    head: dict[str, list[float]],
    threshold: float,
    head_missing: list[str],
) -> list[Row]:
    rows: list[Row] = []
    for name in sorted(set(base) | set(head)):
        head_samples = head.get(name)
        base_samples = base.get(name)
        if not head_samples:
            # Only reachable when the base report has a scenario the head report lacks.
            rows.append(Row(name, statistics.median(base_samples), None, None, None, "head N/A, skipped"))
            continue
        head_value = statistics.median(head_samples)
        if not base_samples:
            rows.append(Row(name, None, head_value, None, None, "base N/A"))
            continue
        base_value = statistics.median(base_samples)
        spread = spread_percent(base_samples)
        if base_value <= 0 or spread is None:
            rows.append(Row(name, base_value, head_value, None, spread, "n/a"))
            continue
        delta = (head_value - base_value) / base_value * 100.0
        status = "FAIL" if delta > threshold and delta > spread else "ok"
        rows.append(Row(name, base_value, head_value, delta, spread, status))
    known = {row.scenario for row in rows}
    for name in sorted(set(head_missing) - known):
        rows.append(Row(name, None, None, None, None, "head N/A, skipped"))
    return rows


def fmt_ms(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.4f}"


def fmt_pct(value: float | None, signed: bool = True) -> str:
    if value is None:
        return "n/a"
    return f"{value:+.2f}%" if signed else f"{value:.2f}%"


def render(rows: list[Row], title: str, threshold: float) -> str:
    lines = [
        f"### {title}",
        "",
        f"Gate: cpu_frame_ms.p95 (median of per-run p95). FAIL needs delta > {threshold:g}% "
        "and delta > base run-to-run spread ((max - min) / median of base per-run p95).",
        "",
        "| scenario | base p95 ms | head p95 ms | delta | base spread | status |",
        "|---|---|---|---|---|---|",
    ]
    for row in rows:
        lines.append(
            f"| {row.scenario} | {fmt_ms(row.base)} | {fmt_ms(row.head)} | {fmt_pct(row.delta)} "
            f"| {fmt_pct(row.spread, signed=False)} | {row.status} |"
        )
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", type=Path, action="append", default=[], help="base report; repeatable (pooled)")
    parser.add_argument("--head", type=Path, action="append", default=[], help="head report; repeatable (pooled)")
    parser.add_argument("--threshold", type=float, default=10.0, help="percent; fail only when strictly greater")
    parser.add_argument("--head-missing", action="append", default=[], help="requested scenario head does not define")
    parser.add_argument("--title", default="Frame bench gate")
    parser.add_argument("--suspects-out", type=Path, help="write failing scenario names, one per line")
    args = parser.parse_args(argv)
    if not args.head:
        print("frame_bench_gate: at least one --head report is required", file=sys.stderr)
        return 2
    try:
        base = load(args.base)
        head = load(args.head)
    except (OSError, ValueError, KeyError, TypeError) as err:
        print(f"frame_bench_gate: {err}", file=sys.stderr)
        return 2
    rows = evaluate(base, head, args.threshold, args.head_missing)
    sys.stdout.write(render(rows, args.title, args.threshold))
    failed = [row.scenario for row in rows if row.status == "FAIL"]
    if args.suspects_out is not None:
        args.suspects_out.write_text("".join(f"{name}\n" for name in failed), encoding="utf-8")
    print(f"result: {'fail' if failed else 'pass'}" + (f" ({', '.join(failed)})" if failed else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
