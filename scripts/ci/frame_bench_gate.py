#!/usr/bin/env python3
"""Noise-aware gate for the `Frame bench` workflow (.github/workflows/frame-bench.yml).

Reads schema-1 `renderide-framebench run` reports. It does not import renderer code, so the
same script gates any base/head pair whose reports share schema 1.

Gate metric: `cpu_frame_ms.p95`. For each side, every report contributes its
`per_run_cpu_frame_ms_p95` list (one value per measured run). The side's value is the median
of those per-run p95s, and `delta = (head - base) / base * 100`.

Decision rule:

1. First pass (`--mode first`, the default): a scenario is flagged when `delta` is strictly
   greater than `--threshold`. No other condition. In particular the base's run-to-run spread
   is not a threshold: a spread of 15-30% (normal on lavapipe, see docs/perf-parity-plan.md
   section 5.1) would otherwise hide real regressions of the same size.
2. Confirm (`--mode confirm`): the workflow re-measures the flagged scenarios for
   `confirm_rounds` interleaved rounds and passes ALL runs, first pass plus confirm, for each
   side. A scenario fails only when both hold on that pooled data:
   a. the pooled `delta` is strictly greater than `--threshold` (the same test as the first
      pass, on more runs, so pooling never widens the threshold), and
   b. a one-sided Mann-Whitney U test says head runs are slower than base runs,
      `p < --alpha` (default 0.05).
   There is no spread term anywhere: a slow base run can widen nothing. (a) is the first-pass
   test itself on a superset of the runs, so the confirm threshold is never looser than the
   first pass. (b) can only clear a flag whose pooled runs overlap so much that "head slower"
   is not distinguishable from noise; with 4x the runs it has far more power than any 5-run
   comparison. `confirm_rounds=0` skips the confirm: every first-pass flag fails.
   Known limit (tested, see test_frame_bench_gate.py): if the confirm rounds add no new
   information (they repeat the first-pass runs exactly), slow base runs sit at or above the
   head median and the two sides overlap heavily, (b) can stay above alpha and clear the flag
   (1.3% of seeded noise-model flags with a true regression >= 15%). Real confirm rounds are
   fresh runs, where the same outliers do not repeat.
   Independence caveat: Mann-Whitney assumes independent runs; lavapipe adds a per-invocation
   shift (section 5.1: round medians move 5-10%). Interleaving base/head rounds spreads that
   shift over both sides; the seeded simulation in the tests includes it.

Mann-Whitney U is implemented here (no SciPy): U counts head > base pairs, ties count 1/2.
When `C(n_base + n_head, n_head) <= EXACT_MAX_COMBINATIONS` the p-value is exact, by
enumerating every assignment of the pooled mid-ranks (ties handled exactly). Otherwise it uses
the normal approximation with tie-corrected variance and a 0.5 continuity correction. Both are
deterministic; the gate uses no randomness.

Rows:

- `ok`: no regression.
- `FAIL`: in `first` mode, a suspect to confirm (see `--suspects-out`); in `confirm` mode, a
  confirmed regression.
- `base N/A`: scenario measured on head, not defined on base. Never fails.
- `head N/A, skipped`: scenario requested but not defined on head (`--head-missing`). Never fails.

Exit status: 0 pass, 1 regression, 2 usage or input error.
"""

from __future__ import annotations

import argparse
import itertools
import json
import math
import statistics
import sys
from dataclasses import dataclass
from pathlib import Path

SCHEMA = 1
DEFAULT_ALPHA = 0.05
# C(14, 7) = 3432 and C(16, 8) = 12870 are exact; C(18, 9) = 48620 uses the normal approximation.
EXACT_MAX_COMBINATIONS = 20_000


@dataclass
class Row:
    scenario: str
    base: float | None
    head: float | None
    delta: float | None
    p_value: float | None
    n_base: int
    n_head: int
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


def delta_percent(base: list[float], head: list[float]) -> float | None:
    base_value = statistics.median(base)
    if base_value <= 0:
        return None
    return (statistics.median(head) - base_value) / base_value * 100.0


def _mid_ranks(values: list[float]) -> tuple[list[float], list[int]]:
    """Mid-ranks (1-based) for `values` in input order, and the size of every tie group."""
    order = sorted(range(len(values)), key=lambda i: values[i])
    ranks = [0.0] * len(values)
    ties: list[int] = []
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j + 1]] == values[order[i]]:
            j += 1
        rank = (i + j) / 2.0 + 1.0
        for k in range(i, j + 1):
            ranks[order[k]] = rank
        ties.append(j - i + 1)
        i = j + 1
    return ranks, ties


def mann_whitney_greater(base: list[float], head: list[float]) -> float:
    """One-sided Mann-Whitney U p-value for H1: head is stochastically greater (slower) than base."""
    n_base, n_head = len(base), len(head)
    if n_base == 0 or n_head == 0:
        return 1.0
    pooled = list(base) + list(head)
    ranks, ties = _mid_ranks(pooled)
    offset = n_head * (n_head + 1) / 2.0
    u_obs = sum(ranks[n_base:]) - offset
    n = n_base + n_head
    if math.comb(n, n_head) <= EXACT_MAX_COMBINATIONS:
        hits = 0
        total = 0
        tolerance = 1e-9
        for chosen in itertools.combinations(ranks, n_head):
            total += 1
            if sum(chosen) - offset >= u_obs - tolerance:
                hits += 1
        return hits / total
    mean = n_base * n_head / 2.0
    tie_term = sum(t * t * t - t for t in ties) / (n * (n - 1))
    variance = n_base * n_head / 12.0 * ((n + 1) - tie_term)
    if variance <= 0:
        return 1.0
    z = (u_obs - mean - 0.5) / math.sqrt(variance)
    return 0.5 * math.erfc(z / math.sqrt(2.0))


def evaluate(
    base: dict[str, list[float]],
    head: dict[str, list[float]],
    threshold: float,
    head_missing: list[str],
    mode: str = "first",
    alpha: float = DEFAULT_ALPHA,
) -> list[Row]:
    """Rows for every scenario. `mode` is `first` (delta only) or `confirm` (delta and U test)."""
    if mode not in ("first", "confirm"):
        raise ValueError(f"unknown mode {mode!r}")
    rows: list[Row] = []
    for name in sorted(set(base) | set(head)):
        head_samples = head.get(name) or []
        base_samples = base.get(name) or []
        if not head_samples:
            # Only reachable when the base report has a scenario the head report lacks.
            rows.append(
                Row(name, statistics.median(base_samples), None, None, None, len(base_samples), 0, "head N/A, skipped")
            )
            continue
        head_value = statistics.median(head_samples)
        if not base_samples:
            rows.append(Row(name, None, head_value, None, None, 0, len(head_samples), "base N/A"))
            continue
        base_value = statistics.median(base_samples)
        delta = delta_percent(base_samples, head_samples)
        if delta is None:
            rows.append(Row(name, base_value, head_value, None, None, len(base_samples), len(head_samples), "n/a"))
            continue
        p_value = mann_whitney_greater(base_samples, head_samples) if mode == "confirm" else None
        regressed = delta > threshold
        if mode == "confirm":
            regressed = regressed and p_value is not None and p_value < alpha
        status = "FAIL" if regressed else "ok"
        rows.append(Row(name, base_value, head_value, delta, p_value, len(base_samples), len(head_samples), status))
    known = {row.scenario for row in rows}
    for name in sorted(set(head_missing) - known):
        rows.append(Row(name, None, None, None, None, 0, 0, "head N/A, skipped"))
    return rows


def decide(
    first_base: list[float],
    first_head: list[float],
    confirm_base: list[list[float]],
    confirm_head: list[list[float]],
    threshold: float,
    alpha: float = DEFAULT_ALPHA,
) -> str:
    """Whole-gate decision for one scenario, as the workflow runs it: `pass` or `fail`.

    `confirm_base` / `confirm_head` are the confirm rounds (may be empty: `confirm_rounds=0`
    fails on the first pass).
    """
    first = evaluate({"s": first_base}, {"s": first_head}, threshold, [])[0]
    if first.status != "FAIL":
        return "pass"
    if not confirm_base and not confirm_head:
        return "fail"
    pooled_base = list(first_base) + [v for run in confirm_base for v in run]
    pooled_head = list(first_head) + [v for run in confirm_head for v in run]
    row = evaluate({"s": pooled_base}, {"s": pooled_head}, threshold, [], mode="confirm", alpha=alpha)[0]
    return "fail" if row.status == "FAIL" else "pass"


def fmt_ms(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.4f}"


def fmt_pct(value: float | None) -> str:
    return "n/a" if value is None else f"{value:+.2f}%"


def fmt_p(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.4f}"


def render(rows: list[Row], title: str, threshold: float, mode: str = "first", alpha: float = DEFAULT_ALPHA) -> str:
    if mode == "confirm":
        rule = (
            f"Confirm (all runs pooled, first pass + confirm rounds): FAIL needs pooled delta > {threshold:g}% "
            f"AND one-sided Mann-Whitney U p < {alpha:g} (head slower than base)."
        )
    else:
        rule = f"First pass: flag when delta > {threshold:g}% (no spread condition). Flags are confirmed before failing."
    lines = [
        f"### {title}",
        "",
        f"Gate: cpu_frame_ms.p95 (median of per-run p95). {rule}",
        "",
        "| scenario | base p95 ms | head p95 ms | delta | runs base/head | Mann-Whitney p | status |",
        "|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        lines.append(
            f"| {row.scenario} | {fmt_ms(row.base)} | {fmt_ms(row.head)} | {fmt_pct(row.delta)} "
            f"| {row.n_base}/{row.n_head} | {fmt_p(row.p_value)} | {row.status} |"
        )
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", type=Path, action="append", default=[], help="base report; repeatable (pooled)")
    parser.add_argument("--head", type=Path, action="append", default=[], help="head report; repeatable (pooled)")
    parser.add_argument("--threshold", type=float, default=10.0, help="percent; flag only when strictly greater")
    parser.add_argument("--mode", choices=("first", "confirm"), default="first", help="first pass or pooled confirm")
    parser.add_argument("--alpha", type=float, default=DEFAULT_ALPHA, help="confirm: Mann-Whitney one-sided level")
    parser.add_argument("--head-missing", action="append", default=[], help="requested scenario head does not define")
    parser.add_argument("--title", default="Frame bench gate")
    parser.add_argument("--suspects-out", type=Path, help="write failing scenario names, one per line")
    args = parser.parse_args(argv)
    if not args.head:
        print("frame_bench_gate: at least one --head report is required", file=sys.stderr)
        return 2
    if not 0.0 < args.alpha < 1.0:
        print("frame_bench_gate: --alpha must be in (0, 1)", file=sys.stderr)
        return 2
    try:
        base = load(args.base)
        head = load(args.head)
    except (OSError, ValueError, KeyError, TypeError) as err:
        print(f"frame_bench_gate: {err}", file=sys.stderr)
        return 2
    rows = evaluate(base, head, args.threshold, args.head_missing, mode=args.mode, alpha=args.alpha)
    sys.stdout.write(render(rows, args.title, args.threshold, mode=args.mode, alpha=args.alpha))
    failed = [row.scenario for row in rows if row.status == "FAIL"]
    if args.suspects_out is not None:
        args.suspects_out.write_text("".join(f"{name}\n" for name in failed), encoding="utf-8")
    print(f"result: {'fail' if failed else 'pass'}" + (f" ({', '.join(failed)})" if failed else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
