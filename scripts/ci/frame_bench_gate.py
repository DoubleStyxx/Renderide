#!/usr/bin/env python3
"""Noise-aware gate for the `Frame bench` workflow (.github/workflows/frame-bench.yml).

Reads schema-1 or schema-2 `renderide-framebench run` reports, in any mix (a master base tree
may still write schema 1 while the head writes schema 2). Schema 2 is a strict superset of
schema 1 that adds a top-level `runner` object (`os`, `cpu_model`, `icd`, `rayon_threads`); the
gate reads only the schema-1 fields, and prints each side's runner line when present. It does
not import renderer code. A `--json` error document (a top-level `error` object, no scenarios)
is an input error.

Gate metric: `cpu_frame_ms.p95`. For each side, every report contributes its
`per_run_cpu_frame_ms_p95` list (one value per measured run). The side's value is the median
of those per-run p95s, and `delta = (head - base) / base * 100`.

Two thresholds, both percent, `--flag-threshold <= --fail-threshold`:

- `--flag-threshold` (default 5): only decides which scenarios get re-measured.
- `--fail-threshold` (default 10): the policy bar. Nothing fails at or below it.
  `--threshold` is a deprecated alias of `--fail-threshold`, like the workflow input: it sets the
  fail threshold and, with a warning, lowers `--flag-threshold` to it when the flag is larger.

Absolute floor (`--abs-floor-ms`, default 0, i.e. off; opt-in): a scenario whose head median
minus base median is at most the floor (in ms) never flags and never fails, whatever its percent.
The floor applies to every data set the rule looks at (first pass, pooled, confirm-only). Such
rows show `ok (abs floor)`. Whether CI should use a nonzero default is open question O8
(plans/test-bench-rewrite.md); it is off by default because at today's lavapipe medians
(~0.2-0.4 ms) a 0.05 ms floor is a 12.5-25% effective bar, above the 10% fail threshold (seeded
simulation at a 0.2 ms median, test_simulation_at_0_2_ms_floor_off_and_on: a true +15% fails
about 90% of the time with the floor off and almost never with a 0.05 ms floor).

Decision rule:

1. First pass (`--mode first`, the default): a scenario is flagged (`FLAG`) when `delta` is
   strictly greater than `--flag-threshold`. No other condition; in particular the base's
   run-to-run spread is not a threshold (a spread of 15-30% is normal on lavapipe, see
   docs/perf-parity-plan.md section 5.1, and one slow base run would hide a real regression).
   Exit 1 when anything is flagged; `--suspects-out` lists the flagged scenarios.
2. Confirm (`--mode confirm`): the workflow re-measures the flagged scenarios for
   `confirm_rounds` interleaved rounds. `--base`/`--head` are the first-pass reports,
   `--confirm-base`/`--confirm-head` the confirm rounds. A scenario FAILS when EITHER holds:
   a. pooled: on all runs (first pass + confirm), `delta > --fail-threshold` AND a one-sided
      Mann-Whitney U test says head is slower, `p < --alpha` (default 0.05);
   b. confirm-only: on the confirm rounds alone, `delta > --fail-threshold` AND the same
      one-sided Mann-Whitney U test, `p < --alpha`.
   Path (b) keeps the confirm strict: confirm rounds that on their own reproduce a regression
   above the bar fail even if odd first-pass runs dilute the pooled data. There is no spread
   term anywhere.
3. No confirm (`confirm_rounds=0`): the workflow runs the first pass with
   `--flag-threshold` set to the fail threshold, so a scenario fails only when its first-pass
   `delta > fail_threshold`. A delta between the two thresholds PASSES: it is below the
   policy bar, and the flag threshold only exists to choose what to re-measure. `decide()`
   implements the same rule.

Measured at the workflow defaults (`runs=9`, flag 5, fail 10, `confirm_rounds=3`; seeded noise
model below, true value 1 ms, no abs floor, 4000 trials per point): a true 0% passes
99.925%; a true +10% fails 47.65% (10% is the bar itself, so about 50% is expected); +12%
fails 69.725%; +15% fails 90.0%. Flag 5 was
chosen over 6 (+15% failed 87.875%) to reach the 90% target at the same 0% pass rate.

Known limit (tested, see test_frame_bench_gate.py): if confirm rounds add no new information
(they repeat the first-pass runs exactly), slow base runs at or above the head median plus heavy
overlap can keep both Mann-Whitney tests above alpha and clear a first-pass delta above the bar
(seeded noise model, true +15-30%: 6 of 3605 such cases at runs=9; 47 of 3511 at runs=5).
Real confirm rounds are fresh runs, where the same outliers do not repeat.
Independence caveat: Mann-Whitney assumes independent runs; lavapipe adds a per-invocation
shift (section 5.1: 5-run medians move 5-10% between invocations). Interleaving base/head
rounds spreads that shift over both sides; the seeded simulation in the tests includes it.

Mann-Whitney U is implemented here (no SciPy): U counts head > base pairs, ties count 1/2.
When `C(n_base + n_head, n_head) <= EXACT_MAX_COMBINATIONS` the p-value is exact, by
enumerating every assignment of the pooled mid-ranks (ties handled exactly). Otherwise it uses
the normal approximation with tie-corrected variance and a 0.5 continuity correction. Both are
deterministic; the gate uses no randomness.

Rows:

- `ok`: no regression.
- `ok (abs floor)`: delta above the threshold in percent, but at most `--abs-floor-ms` in ms.
- `FLAG` (first mode): delta above the flag threshold; re-measure it.
- `FAIL (pooled)`, `FAIL (confirm-only)`, `FAIL (pooled, confirm-only)` (confirm mode): a
  confirmed regression and the path(s) that failed it.
- `base N/A`: scenario measured on head, not defined on base. Never fails.
- `head N/A, skipped`: scenario requested but not defined on head (`--head-missing`). Never fails.

Input errors (exit 2, message on stderr): bad arguments; a report that is unreadable, has a
schema other than 1 or 2, is a schema-2 report without a `runner` object, is a `--json` error
document, or misses a field; a head report with zero scenarios; a NaN or infinite value; a
base median that is 0 or negative (first pass, confirm rounds alone, or pooled).

Exit status: 0 pass, 1 flagged (first) or regression (confirm), 2 usage or input error.
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

SCHEMA = 2  # newest schema renderide-framebench writes
SUPPORTED_SCHEMAS = (1, 2)
DEFAULT_ALPHA = 0.05
# Head-minus-base median deltas at or below this many ms never flag or fail. 0 = off (default);
# the floor is opt-in via --abs-floor-ms until O8 is decided.
DEFAULT_ABS_FLOOR_MS = 0.0
# Float slack at the floor boundary (1.05 - 1.0 is 0.05000000000000004).
ABS_FLOOR_EPSILON_MS = 1e-9
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
    # Confirm mode only: the confirm rounds on their own (path b).
    confirm_delta: float | None = None
    confirm_p: float | None = None
    confirm_n_base: int = 0
    confirm_n_head: int = 0


class InputError(ValueError):
    """Bad report contents; the CLI exits 2."""


def load(paths: list[Path], require_scenarios: bool = False) -> dict[str, list[float]]:
    """Scenario name -> pooled per-run cpu p95 samples across all `paths`.

    Raises `InputError` for a wrong schema, a NaN or infinite value, or (with
    `require_scenarios`, used for head reports) a report with zero scenarios.
    """
    pooled: dict[str, list[float]] = {}
    for path in paths:
        report = read_report(path)
        scenarios = report.get("scenarios") or []
        if require_scenarios and not scenarios:
            raise InputError(f"{path}: head report has zero scenarios")
        for scenario in scenarios:
            samples = scenario.get("per_run_cpu_frame_ms_p95") or [scenario["cpu_frame_ms"]["p95"]]
            values = [float(v) for v in samples]
            bad = [v for v in values if not math.isfinite(v)]
            if bad:
                raise InputError(f"{path}: scenario {scenario['name']!r} has a non-finite value ({bad[0]!r})")
            pooled.setdefault(scenario["name"], []).extend(values)
    return pooled


def read_report(path: Path) -> dict:
    """Parse one report and validate its schema (1 or 2). Raises `InputError`."""
    report = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(report, dict):
        raise InputError(f"{path}: report is not a JSON object")
    if "error" in report:
        error = report.get("error") or {}
        raise InputError(
            f"{path}: framebench error document (exit {error.get('exit_code')!r}, "
            f"{error.get('kind')!r}): {error.get('message')!r}"
        )
    schema = report.get("schema")
    # bool is an int subclass: `True == 1` must not pass as schema 1.
    if isinstance(schema, bool) or not isinstance(schema, int) or schema not in SUPPORTED_SCHEMAS:
        raise InputError(f"{path}: schema {schema!r}, expected one of {list(SUPPORTED_SCHEMAS)}")
    if schema >= 2 and not isinstance(report.get("runner"), dict):
        raise InputError(f"{path}: schema {schema} report has no `runner` object")
    return report


def runner_line(label: str, paths: list[Path]) -> str | None:
    """`label runner: cpu=... icd=... rayon_threads=...` from the first schema-2 report, if any."""
    for path in paths:
        try:
            runner = read_report(path).get("runner")
        except (OSError, ValueError):
            return None
        if isinstance(runner, dict):
            icd = runner.get("icd") or {}
            icd_text = icd.get("path") or "none"
            source = icd.get("var") or icd.get("source") or "?"
            return (
                f"{label} runner: os={runner.get('os', '?')} cpu=\"{runner.get('cpu_model', '?')}\" "
                f"icd={icd_text} ({source}) rayon_threads={runner.get('rayon_threads', '?')} "
                f"({runner.get('rayon_threads_source', '?')})"
            )
    return None


def below_abs_floor(base: list[float], head: list[float], abs_floor_ms: float) -> bool:
    """True when median(head) - median(base) is at most the absolute floor, in ms."""
    return statistics.median(head) - statistics.median(base) <= abs_floor_ms + ABS_FLOOR_EPSILON_MS


def check_base_medians(label: str, base: dict[str, list[float]]) -> None:
    """Raise `InputError` when any base median is 0 or negative (delta is undefined)."""
    for name in sorted(base):
        if base[name] and statistics.median(base[name]) <= 0:
            raise InputError(f"{label} base median for {name!r} is {statistics.median(base[name])!r}; must be > 0")


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


def _missing_rows(
    base: dict[str, list[float]], head: dict[str, list[float]], head_missing: list[str]
) -> tuple[list[str], list[Row]]:
    """Names that have data on both sides, and the N/A rows for the rest."""
    both: list[str] = []
    rows: list[Row] = []
    for name in sorted(set(base) | set(head)):
        head_samples = head.get(name) or []
        base_samples = base.get(name) or []
        if not head_samples:
            rows.append(
                Row(name, statistics.median(base_samples), None, None, None, len(base_samples), 0, "head N/A, skipped")
            )
        elif not base_samples:
            rows.append(Row(name, None, statistics.median(head_samples), None, None, 0, len(head_samples), "base N/A"))
        else:
            both.append(name)
    known = set(base) | set(head)
    for name in sorted(set(head_missing) - known):
        rows.append(Row(name, None, None, None, None, 0, 0, "head N/A, skipped"))
    return both, rows


def _order(rows: list[Row]) -> list[Row]:
    na_head = [r for r in rows if r.status == "head N/A, skipped" and r.base is None and r.n_base == 0]
    rest = sorted((r for r in rows if r not in na_head), key=lambda r: r.scenario)
    return rest + sorted(na_head, key=lambda r: r.scenario)


def evaluate_first(
    base: dict[str, list[float]],
    head: dict[str, list[float]],
    flag_threshold: float,
    head_missing: list[str],
    abs_floor_ms: float = DEFAULT_ABS_FLOOR_MS,
) -> list[Row]:
    """First pass: `FLAG` when delta > flag_threshold AND the ms delta is above the floor."""
    both, rows = _missing_rows(base, head, head_missing)
    for name in both:
        b, h = base[name], head[name]
        delta = delta_percent(b, h)
        if delta is None:
            status = "n/a"
        elif delta <= flag_threshold:
            status = "ok"
        elif below_abs_floor(b, h, abs_floor_ms):
            status = "ok (abs floor)"
        else:
            status = "FLAG"
        rows.append(Row(name, statistics.median(b), statistics.median(h), delta, None, len(b), len(h), status))
    return _order(rows)


def _significant_regression(
    base: list[float], head: list[float], fail_threshold: float, alpha: float,
    abs_floor_ms: float = DEFAULT_ABS_FLOOR_MS,
):
    """(delta, p, regressed, floored) for one data set.

    regressed: delta > fail_threshold AND Mann-Whitney p < alpha AND the ms delta is above the
    absolute floor. floored: the first two held and only the floor cleared it.
    """
    if not base or not head:
        return None, None, False, False
    delta = delta_percent(base, head)
    p_value = mann_whitney_greater(base, head)
    relative = delta is not None and delta > fail_threshold and p_value < alpha
    floored = relative and below_abs_floor(base, head, abs_floor_ms)
    return delta, p_value, relative and not floored, floored


def evaluate_confirm(
    first_base: dict[str, list[float]],
    first_head: dict[str, list[float]],
    confirm_base: dict[str, list[float]],
    confirm_head: dict[str, list[float]],
    fail_threshold: float,
    head_missing: list[str],
    alpha: float = DEFAULT_ALPHA,
    abs_floor_ms: float = DEFAULT_ABS_FLOOR_MS,
) -> list[Row]:
    """Confirm: FAIL when (a) pooled or (b) confirm-only data shows delta > fail AND MW p < alpha,
    each path also needing its ms delta above the absolute floor."""
    pooled_base = {k: list(v) for k, v in first_base.items()}
    pooled_head = {k: list(v) for k, v in first_head.items()}
    for k, v in confirm_base.items():
        pooled_base.setdefault(k, []).extend(v)
    for k, v in confirm_head.items():
        pooled_head.setdefault(k, []).extend(v)
    both, rows = _missing_rows(pooled_base, pooled_head, head_missing)
    for name in both:
        b, h = pooled_base[name], pooled_head[name]
        delta, p_value, pooled_fail, pooled_floor = _significant_regression(b, h, fail_threshold, alpha, abs_floor_ms)
        cb, ch = confirm_base.get(name) or [], confirm_head.get(name) or []
        c_delta, c_p, confirm_fail, confirm_floor = _significant_regression(
            cb, ch, fail_threshold, alpha, abs_floor_ms
        )
        paths = [label for label, hit in (("pooled", pooled_fail), ("confirm-only", confirm_fail)) if hit]
        if delta is None:
            status = "n/a"
        elif paths:
            status = f"FAIL ({', '.join(paths)})"
        elif pooled_floor or confirm_floor:
            status = "ok (abs floor)"
        else:
            status = "ok"
        rows.append(
            Row(name, statistics.median(b), statistics.median(h), delta, p_value, len(b), len(h), status,
                c_delta, c_p, len(cb), len(ch))
        )
    return _order(rows)


def is_failure(status: str) -> bool:
    return status.startswith("FAIL") or status == "FLAG"


def decide(
    first_base: list[float],
    first_head: list[float],
    confirm_base: list[list[float]],
    confirm_head: list[list[float]],
    flag_threshold: float,
    fail_threshold: float,
    alpha: float = DEFAULT_ALPHA,
    abs_floor_ms: float = DEFAULT_ABS_FLOOR_MS,
) -> str:
    """Whole-gate decision for one scenario, as the workflow runs it: `pass` or `fail`.

    `confirm_base` / `confirm_head` are the confirm rounds. Empty means `confirm_rounds=0`:
    then the first pass runs at the fail threshold (a delta between the two thresholds passes).
    """
    if not confirm_base and not confirm_head:
        first = evaluate_first({"s": first_base}, {"s": first_head}, fail_threshold, [], abs_floor_ms)[0]
        return "fail" if first.status == "FLAG" else "pass"
    first = evaluate_first({"s": first_base}, {"s": first_head}, flag_threshold, [], abs_floor_ms)[0]
    if first.status != "FLAG":
        return "pass"
    row = evaluate_confirm(
        {"s": first_base}, {"s": first_head},
        {"s": [v for run in confirm_base for v in run]}, {"s": [v for run in confirm_head for v in run]},
        fail_threshold, [], alpha, abs_floor_ms,
    )[0]
    return "fail" if row.status.startswith("FAIL") else "pass"


def fmt_ms(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.4f}"


def fmt_pct(value: float | None) -> str:
    return "n/a" if value is None else f"{value:+.2f}%"


def fmt_p(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.4f}"


def render(
    rows: list[Row],
    title: str,
    flag_threshold: float,
    fail_threshold: float,
    mode: str = "first",
    alpha: float = DEFAULT_ALPHA,
    abs_floor_ms: float = DEFAULT_ABS_FLOOR_MS,
    runner_lines: list[str] | None = None,
) -> str:
    lines = [f"### {title}", ""]
    floor_text = (
        f" Absolute floor: a head-minus-base median delta <= {abs_floor_ms:g} ms never flags or fails."
        if abs_floor_ms > 0 else ""
    )
    if mode == "confirm":
        lines += [
            "Gate: cpu_frame_ms.p95 (median of per-run p95). Confirm: FAIL when EITHER "
            f"(a) pooled (first pass + confirm) delta > {fail_threshold:g}% AND one-sided Mann-Whitney U p < {alpha:g}, "
            f"OR (b) confirm-rounds-only delta > {fail_threshold:g}% AND Mann-Whitney p < {alpha:g} (head slower)."
            + floor_text,
            "",
            "| scenario | base p95 ms | head p95 ms | pooled delta | pooled runs base/head | pooled MW p "
            "| confirm-only delta | confirm runs base/head | confirm-only MW p | status |",
            "|---|---|---|---|---|---|---|---|---|---|",
        ]
        for r in rows:
            lines.append(
                f"| {r.scenario} | {fmt_ms(r.base)} | {fmt_ms(r.head)} | {fmt_pct(r.delta)} | {r.n_base}/{r.n_head} "
                f"| {fmt_p(r.p_value)} | {fmt_pct(r.confirm_delta)} | {r.confirm_n_base}/{r.confirm_n_head} "
                f"| {fmt_p(r.confirm_p)} | {r.status} |"
            )
    else:
        lines += [
            f"Gate: cpu_frame_ms.p95 (median of per-run p95). First pass: FLAG when delta > {flag_threshold:g}% "
            f"(no spread condition); flags are re-measured and fail only above {fail_threshold:g}%."
            + floor_text,
            "",
            "| scenario | base p95 ms | head p95 ms | delta | runs base/head | status |",
            "|---|---|---|---|---|---|",
        ]
        for r in rows:
            lines.append(
                f"| {r.scenario} | {fmt_ms(r.base)} | {fmt_ms(r.head)} | {fmt_pct(r.delta)} "
                f"| {r.n_base}/{r.n_head} | {r.status} |"
            )
    if runner_lines:
        lines.append("")
        lines.extend(f"- {line}" for line in runner_lines)
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", type=Path, action="append", default=[], help="first-pass base report; repeatable")
    parser.add_argument("--head", type=Path, action="append", default=[], help="first-pass head report; repeatable")
    parser.add_argument("--confirm-base", type=Path, action="append", default=[], help="confirm-round base report")
    parser.add_argument("--confirm-head", type=Path, action="append", default=[], help="confirm-round head report")
    parser.add_argument("--flag-threshold", type=float, default=5.0, help="percent; first pass flags when delta > this")
    parser.add_argument("--fail-threshold", type=float, default=10.0, help="percent; the policy bar")
    parser.add_argument(
        "--threshold",
        type=float,
        dest="legacy_threshold",
        help="deprecated alias of --fail-threshold; lowers --flag-threshold to it if larger",
    )
    parser.add_argument("--mode", choices=("first", "confirm"), default="first", help="first pass or confirm")
    parser.add_argument("--alpha", type=float, default=DEFAULT_ALPHA, help="confirm: Mann-Whitney one-sided level")
    parser.add_argument(
        "--abs-floor-ms",
        type=float,
        default=DEFAULT_ABS_FLOOR_MS,
        help="ms; a head-minus-base median delta at or below this never flags or fails (default 0 = off; opt-in, O8)",
    )
    parser.add_argument("--head-missing", action="append", default=[], help="requested scenario head does not define")
    parser.add_argument("--title", default="Frame bench gate")
    parser.add_argument("--suspects-out", type=Path, help="first mode: write flagged scenario names, one per line")
    args = parser.parse_args(argv)
    if args.legacy_threshold is not None:
        # Same as the workflow's deprecated `threshold` input.
        print(f"frame_bench_gate: warning: --threshold is deprecated; using it as --fail-threshold "
              f"{args.legacy_threshold:g}", file=sys.stderr)
        args.fail_threshold = args.legacy_threshold
        if args.flag_threshold > args.fail_threshold:
            print(f"frame_bench_gate: warning: --flag-threshold {args.flag_threshold:g} > --fail-threshold "
                  f"{args.fail_threshold:g}; lowering --flag-threshold", file=sys.stderr)
            args.flag_threshold = args.fail_threshold
    if not args.head:
        print("frame_bench_gate: at least one --head report is required", file=sys.stderr)
        return 2
    if not 0.0 < args.alpha < 1.0:
        print("frame_bench_gate: --alpha must be in (0, 1)", file=sys.stderr)
        return 2
    if not (math.isfinite(args.abs_floor_ms) and args.abs_floor_ms >= 0.0):
        print("frame_bench_gate: --abs-floor-ms must be a finite number >= 0", file=sys.stderr)
        return 2
    if not 0.0 <= args.flag_threshold <= args.fail_threshold:
        print("frame_bench_gate: need 0 <= --flag-threshold <= --fail-threshold", file=sys.stderr)
        return 2
    if args.mode == "first" and (args.confirm_base or args.confirm_head):
        print("frame_bench_gate: --confirm-base/--confirm-head need --mode confirm", file=sys.stderr)
        return 2
    try:
        base = load(args.base)
        head = load(args.head, require_scenarios=True)
        confirm_base = load(args.confirm_base)
        confirm_head = load(args.confirm_head, require_scenarios=True)
        check_base_medians("first-pass", base)
        if args.mode == "confirm":
            check_base_medians("confirm-round", confirm_base)
            pooled_base = {k: list(v) for k, v in base.items()}
            for k, v in confirm_base.items():
                pooled_base.setdefault(k, []).extend(v)
            check_base_medians("pooled", pooled_base)
    except (OSError, ValueError, KeyError, TypeError) as err:
        print(f"frame_bench_gate: {err}", file=sys.stderr)
        return 2
    if args.mode == "confirm":
        rows = evaluate_confirm(
            base, head, confirm_base, confirm_head, args.fail_threshold, args.head_missing, args.alpha,
            args.abs_floor_ms,
        )
    else:
        rows = evaluate_first(base, head, args.flag_threshold, args.head_missing, args.abs_floor_ms)
    runners = [line for line in (runner_line("base", args.base), runner_line("head", args.head)) if line]
    sys.stdout.write(
        render(rows, args.title, args.flag_threshold, args.fail_threshold, args.mode, args.alpha,
               args.abs_floor_ms, runners)
    )
    hit = [row.scenario for row in rows if is_failure(row.status)]
    if args.suspects_out is not None:
        args.suspects_out.write_text("".join(f"{name}\n" for name in hit), encoding="utf-8")
    word = "fail" if args.mode == "confirm" else "flagged"
    print(f"result: {word if hit else 'pass'}" + (f" ({', '.join(hit)})" if hit else ""))
    return 1 if hit else 0


if __name__ == "__main__":
    sys.exit(main())
