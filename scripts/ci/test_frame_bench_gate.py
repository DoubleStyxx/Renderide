"""Unit tests for frame_bench_gate.py. Run: python3 -m unittest discover -s scripts/ci -p 'test_*.py'"""

from __future__ import annotations

import io
import json
import math
import random
import statistics
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import frame_bench_gate as gate  # noqa: E402

# Historical frame-bench.yml defaults (pre-parity); locked simulation rates below.
RUNS = 9
CONFIRM_ROUNDS = 3
# Current bench-pr.yml / frame-bench.yml parity defaults.
PARITY_RUNS = 12
PARITY_CONFIRM_ROUNDS = 1
FLAG = 5.0
FAIL = 10.0
# Opt-in absolute floor used by the explicit floor tests (the gate default is 0, off; O8 open).
FLOOR = 0.05

# Noise model shaped like docs/perf-parity-plan.md section 5.1 (lavapipe, 5 runs x 300 frames):
# per-invocation shift (round medians move ~5-10%), lognormal per-run noise, and an occasional
# slow run 1.15-1.5x. Gives a median 5-run p95 spread of ~18% with IQR ~12-34% (asserted below).
SIGMA_INVOCATION = 0.03
SIGMA_RUN = 0.06
P_SLOW = 0.10
SLOW_RANGE = (1.15, 1.5)


def invocation(rng: random.Random, true_ms: float, runs: int) -> list[float]:
    """Per-run p95s for one `renderide-framebench run --runs <runs>` invocation."""
    shift = math.exp(rng.gauss(0.0, SIGMA_INVOCATION))
    out = []
    for _ in range(runs):
        value = true_ms * shift * math.exp(rng.gauss(0.0, SIGMA_RUN))
        if rng.random() < P_SLOW:
            value *= rng.uniform(*SLOW_RANGE)
        out.append(value)
    return out


def simulate(
    true_pct: float,
    runs: int,
    rounds: int,
    flag: float,
    fail: float,
    trials: int,
    seed: int,
    true_ms: float = 1.0,
    abs_floor_ms: float = gate.DEFAULT_ABS_FLOOR_MS,
) -> float:
    """Fraction of trials where the whole gate (first pass + confirm-only FAIL rule) fails."""
    rng = random.Random(seed)
    head_ms = true_ms * (1.0 + true_pct / 100.0)
    fails = 0
    for _ in range(trials):
        first_base = invocation(rng, true_ms, runs)
        first_head = invocation(rng, head_ms, runs)
        confirm_base, confirm_head = [], []
        for _ in range(rounds):  # interleaved: base round, head round
            confirm_base.append(invocation(rng, true_ms, runs))
            confirm_head.append(invocation(rng, head_ms, runs))
        verdict = gate.decide(first_base, first_head, confirm_base, confirm_head, flag, fail, abs_floor_ms=abs_floor_ms, drift_threshold=1e9)
        fails += verdict == "fail"
    return fails / trials


def old_rule_fails(base: list[float], head: list[float], threshold: float) -> bool:
    """The rule Prism rejected on #192 (b56748f6): fail when delta > max(threshold, base spread)."""
    base_median = statistics.median(base)
    delta = (statistics.median(head) - base_median) / base_median * 100.0
    spread = (max(base) - min(base)) / base_median * 100.0
    return delta > threshold and delta > spread


# Schema-2 `runner` block (renderide-framebench --json / --output), as on a 7763 CI runner.
RUNNER = {
    "os": "linux",
    "cpu_model": "AMD EPYC 7763 64-Core Processor",
    "icd": {"path": "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json", "source": "env", "var": "VK_DRIVER_FILES"},
    "rayon_threads": 2,
    "rayon_threads_source": "env",
}


def report2(scenarios: dict[str, list[float]]) -> dict:
    """Schema-2 report: schema 1 plus `runner` (and fields the gate ignores)."""
    doc = report(scenarios)
    doc["schema"] = 2
    doc["runner"] = dict(RUNNER)
    for scenario in doc["scenarios"]:
        scenario["draw_calls"] = {"p50": 676.0, "p95": 676.0, "p99": 676.0}
    return doc


def report(scenarios: dict[str, list[float]]) -> dict:
    return {
        "schema": 1,
        "scenarios": [
            {"name": name, "cpu_frame_ms": {"p50": 0.0, "p95": runs[0], "p99": 0.0}, "per_run_cpu_frame_ms_p95": runs}
            for name, runs in scenarios.items()
        ],
    }


def flat(value: float, n: int = RUNS) -> list[float]:
    return [value] * n


class GateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write(self, name: str, scenarios: dict[str, list[float]]) -> Path:
        path = self.dir / name
        path.write_text(json.dumps(report(scenarios)), encoding="utf-8")
        return path

    def write2(self, name: str, scenarios: dict[str, list[float]]) -> Path:
        path = self.dir / name
        path.write_text(json.dumps(report2(scenarios)), encoding="utf-8")
        return path

    def run_gate(self, *args: str) -> int:
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            return gate.main(list(args))

    def decide(self, fb, fh, cb, ch, confirm_min_n: int = 1) -> str:
        # Default confirm_min_n=1 so property tests with small n still exercise MW paths;
        # production CONFIRM_MIN_N=12 is covered by dedicated n>=12 tests below.
        return gate.decide(fb, fh, cb, ch, FLAG, FAIL, confirm_min_n=confirm_min_n)

    def confirm_row(
        self, fb, fh, cb, ch, abs_floor_ms: float = gate.DEFAULT_ABS_FLOOR_MS, confirm_min_n: int = 1
    ) -> gate.Row:
        flat_cb = [v for r in cb for v in r]
        flat_ch = [v for r in ch for v in r]
        return gate.evaluate_confirm(
            {"s": fb}, {"s": fh}, {"s": flat_cb}, {"s": flat_ch}, FAIL, [],
            abs_floor_ms=abs_floor_ms, confirm_min_n=confirm_min_n,
        )[0]

    # --- first pass and plumbing ----------------------------------------------------------

    def test_delta_inside_base_spread_is_flagged_then_cleared(self) -> None:
        # Shape of #178 run 40050133923: +~11% on p95 while base runs spread ~19%. The old
        # spread rule passed this; the first pass now flags it and a calm confirm clears it.
        # Default rule (abs floor off). At ~0.21 ms this delta is 0.025 ms, which an opted-in
        # 0.05 ms floor clears outright (test_abs_floor_clears_issue_178_shape).
        base = [0.19, 0.2145, 0.23, 0.21, 0.22]
        head = [0.2391, 0.23, 0.24, 0.25, 0.235]
        row = gate.evaluate_first({"s": base}, {"s": head}, FLAG, [])[0]
        self.assertGreater(row.delta, FAIL)
        self.assertEqual(row.status, "FLAG")
        calm = [[0.21, 0.215, 0.22, 0.212, 0.218]] * CONFIRM_ROUNDS
        self.assertEqual(self.decide(base, head, calm, calm), "pass")

    def test_flag_threshold_decides_what_is_flagged(self) -> None:
        base, head = flat(1.0), flat(1.08)
        self.assertEqual(gate.evaluate_first({"s": base}, {"s": head}, FLAG, [])[0].status, "FLAG")
        self.assertEqual(gate.evaluate_first({"s": base}, {"s": flat(1.04)}, FLAG, [])[0].status, "ok")

    def test_delta_beyond_threshold_is_flagged_via_cli(self) -> None:
        base = self.write("b.json", {"s": [1.0, 1.01, 0.99, 1.0, 1.0], "t": flat(1.0)})
        head = self.write("h.json", {"s": [1.2, 1.21, 1.19, 1.2, 1.2], "t": flat(1.0)})
        suspects = self.dir / "suspects.txt"
        code = self.run_gate("--base", str(base), "--head", str(head), "--suspects-out", str(suspects))
        self.assertEqual(code, 1)
        self.assertEqual(suspects.read_text(encoding="utf-8"), "s\n")

    def test_threshold_equality_passes(self) -> None:
        base, head = flat(1.0, 3), flat(1.1, 3)
        self.assertEqual(gate.evaluate_first({"s": base}, {"s": head}, 10.000001, [])[0].status, "ok")
        row = gate.evaluate_confirm({"s": base}, {"s": head}, {"s": base * 3}, {"s": head * 3}, 10.000001, [])[0]
        self.assertEqual(row.status, "ok")

    def test_reports_are_pooled(self) -> None:
        b1 = self.write("b1.json", {"s": [1.0]})
        b2 = self.write("b2.json", {"s": [2.0, 3.0]})
        self.assertEqual(gate.load([b1, b2]), {"s": [1.0, 2.0, 3.0]})

    def test_base_missing_and_head_missing_never_fail(self) -> None:
        base = {"a": [1.0, 1.0]}
        head = {"a": [1.0, 1.0], "new_scene": [5.0, 5.0]}
        expected = {"a": "ok", "new_scene": "base N/A", "gone_scene": "head N/A, skipped"}
        rows = gate.evaluate_first(base, head, FLAG, ["gone_scene"])
        self.assertEqual({r.scenario: r.status for r in rows}, expected)
        self.assertIn("| gone_scene | n/a | n/a | n/a | 0/0 | head N/A, skipped |", gate.render(rows, "t", FLAG, FAIL))
        rows = gate.evaluate_confirm(base, head, {}, {}, FAIL, ["gone_scene"])
        self.assertEqual({r.scenario: r.status for r in rows}, expected)
        md = gate.render(rows, "t", FLAG, FAIL, mode="confirm")
        self.assertIn("| gone_scene | n/a | n/a | n/a | 0/0 | n/a | n/a | 0/0 | n/a | n/a | 1 | n/a | head N/A, skipped |", md)

    def test_wrong_schema_is_an_input_error(self) -> None:
        path = self.dir / "bad.json"
        path.write_text(json.dumps({"schema": 3, "scenarios": []}), encoding="utf-8")
        self.assertEqual(self.run_gate("--base", str(path), "--head", str(path)), 2)

    def test_threshold_arguments(self) -> None:
        base = self.write("b.json", {"s": flat(1.0)})
        head = self.write("h.json", {"s": flat(1.08)})
        common = ["--base", str(base), "--head", str(head)]
        self.assertEqual(self.run_gate(*common), 1)  # default flag 5
        self.assertEqual(self.run_gate(*common, "--flag-threshold", "9"), 0)
        self.assertEqual(self.run_gate(*common, "--flag-threshold", "11", "--fail-threshold", "10"), 2)
        self.assertEqual(self.run_gate(*common, "--flag-threshold", "-1"), 2)
        # Deprecated --threshold maps to --fail-threshold; 4 < default flag 5 lowers the flag to 4
        # (test_legacy_threshold_alias_lowers_flag) instead of erroring.
        self.assertEqual(self.run_gate(*common, "--threshold", "4"), 1)
        self.assertEqual(self.run_gate(*common, "--threshold", "7", "--flag-threshold", "7"), 1)
        self.assertEqual(self.run_gate(*common, "--threshold", "9", "--flag-threshold", "9"), 0)
        self.assertEqual(self.run_gate(*common, "--confirm-base", str(base)), 2)  # confirm data needs --mode confirm

    def run_gate_capture(self, *args: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = gate.main(list(args))
        return code, out.getvalue(), err.getvalue()

    def test_legacy_threshold_alias_lowers_flag(self) -> None:
        # Like the workflow's deprecated `threshold` input: sets fail, lowers flag with a warning.
        base = self.write("b.json", {"s": flat(1.0)})
        head = self.write("h.json", {"s": flat(1.045)})  # +4.5%: under default flag 5
        # Relative thresholds only: +4.5% of 1 ms is 0.045 ms, under the default abs floor.
        common = ["--base", str(base), "--head", str(head), "--abs-floor-ms", "0"]
        self.assertEqual(self.run_gate(*common), 0)
        code, out, err = self.run_gate_capture(*common, "--threshold", "4")
        self.assertEqual(code, 1)  # flag lowered 5 -> 4, so +4.5% is flagged
        self.assertIn("--threshold is deprecated", err)
        self.assertIn("lowering --flag-threshold", err)
        self.assertIn("FLAG when delta > 4%", out)
        self.assertIn("fail only above 4%", out)
        # Not larger: the flag is kept and no lowering warning is printed.
        code, out, err = self.run_gate_capture(*common, "--threshold", "8", "--flag-threshold", "3")
        self.assertEqual(code, 1)
        self.assertNotIn("lowering", err)
        self.assertIn("FLAG when delta > 3%", out)
        self.assertIn("fail only above 8%", out)
        # Confirm mode: the alias is the fail bar there too (+4.5% > 4, all runs slower).
        # Use n=12 confirm samples so the production CONFIRM_MIN_N rule can FAIL.
        cb = self.write("cb.json", {"s": [1.0] * 12})
        ch = self.write("ch.json", {"s": [1.045] * 12})
        code, out, _ = self.run_gate_capture(
            "--mode", "confirm", *common, "--confirm-base", str(cb), "--confirm-head", str(ch), "--threshold", "4"
        )
        self.assertEqual(code, 1)
        self.assertIn("FAIL (confirm-only)", out)

    # --- input errors (exit 2) ---------------------------------------------------------------

    def write_raw(self, name: str, report_obj: dict) -> Path:
        path = self.dir / name
        path.write_text(json.dumps(report_obj), encoding="utf-8")  # NaN is written as `NaN`
        return path

    def test_head_report_with_zero_scenarios_is_an_input_error(self) -> None:
        base = self.write("b.json", {"s": flat(1.0)})
        empty = self.write_raw("h.json", {"schema": 1, "scenarios": []})
        code, _, err = self.run_gate_capture("--base", str(base), "--head", str(empty))
        self.assertEqual(code, 2)
        self.assertIn("head report has zero scenarios", err)
        missing_key = self.write_raw("h2.json", {"schema": 1})
        self.assertEqual(self.run_gate("--base", str(base), "--head", str(missing_key)), 2)
        # A confirm-round head report with zero scenarios is an error too.
        head = self.write("h3.json", {"s": flat(1.2)})
        code, _, err = self.run_gate_capture(
            "--mode", "confirm", "--base", str(base), "--head", str(head),
            "--confirm-base", str(base), "--confirm-head", str(empty),
        )
        self.assertEqual(code, 2)
        self.assertIn("zero scenarios", err)

    def test_zero_base_median_is_an_input_error(self) -> None:
        head = self.write("h.json", {"s": flat(1.0), "t": flat(1.0)})
        zero = self.write("b.json", {"s": [0.0] * 5 + [1.0] * 4, "t": flat(1.0)})  # median 0
        code, _, err = self.run_gate_capture("--base", str(zero), "--head", str(head))
        self.assertEqual(code, 2)
        self.assertIn("base median for 's' is 0.0", err)
        # Confirm rounds alone with a zero base median (first pass fine).
        good = self.write("b2.json", {"s": flat(1.0), "t": flat(1.0)})
        cb = self.write("cb.json", {"s": flat(0.0)})
        ch = self.write("ch.json", {"s": flat(1.2)})
        code, _, err = self.run_gate_capture(
            "--mode", "confirm", "--base", str(good), "--head", str(head),
            "--confirm-base", str(cb), "--confirm-head", str(ch),
        )
        self.assertEqual(code, 2)
        self.assertIn("confirm-round base median", err)
        # A non-zero median with a single 0 run is fine.
        some_zero = self.write("b3.json", {"s": [0.0] + [1.0] * 8, "t": flat(1.0)})
        self.assertEqual(self.run_gate("--base", str(some_zero), "--head", str(head)), 0)

    def test_nan_value_is_an_input_error(self) -> None:
        base = self.write("b.json", {"s": flat(1.0)})
        head = self.write("h.json", {"s": flat(1.0)})
        nan_head = self.write("hn.json", {"s": [1.0] * 8 + [float("nan")]})
        nan_base = self.write("bn.json", {"s": [float("nan")] + [1.0] * 8})
        for args in (
            ("--base", str(base), "--head", str(nan_head)),
            ("--base", str(nan_base), "--head", str(head)),
            ("--mode", "confirm", "--base", str(base), "--head", str(head),
             "--confirm-base", str(nan_base), "--confirm-head", str(head)),
        ):
            code, _, err = self.run_gate_capture(*args)
            self.assertEqual(code, 2, args)
            self.assertIn("non-finite value (nan)", err)
        # The single-p95 fallback (no per-run list) is checked as well.
        fallback = self.write_raw("hf.json", {"schema": 1, "scenarios": [
            {"name": "s", "cpu_frame_ms": {"p50": 0.0, "p95": float("nan"), "p99": 0.0}}]})
        self.assertEqual(self.run_gate("--base", str(base), "--head", str(fallback)), 2)

    def test_infinite_value_is_an_input_error(self) -> None:
        # json writes float("inf") as `Infinity`, which json.loads reads back as inf.
        base = self.write("b.json", {"s": flat(1.0)})
        head = self.write("h.json", {"s": flat(1.0)})
        inf_head = self.write("hi.json", {"s": [1.0] * 8 + [float("inf")]})
        neg_inf_base = self.write("bi.json", {"s": [float("-inf")] + [1.0] * 8})
        for args, shown in (
            (("--base", str(base), "--head", str(inf_head)), "inf"),
            (("--base", str(neg_inf_base), "--head", str(head)), "-inf"),
            (("--mode", "confirm", "--base", str(base), "--head", str(head),
              "--confirm-base", str(base), "--confirm-head", str(inf_head)), "inf"),
        ):
            code, _, err = self.run_gate_capture(*args)
            self.assertEqual(code, 2, args)
            self.assertIn(f"non-finite value ({shown})", err)
        fallback = self.write_raw("hf.json", {"schema": 1, "scenarios": [
            {"name": "s", "cpu_frame_ms": {"p50": 0.0, "p95": float("inf"), "p99": 0.0}}]})
        self.assertEqual(self.run_gate("--base", str(base), "--head", str(fallback)), 2)

    def test_negative_base_median_is_an_input_error(self) -> None:
        head = self.write("h.json", {"s": flat(1.0)})
        negative = self.write("b.json", {"s": [-1.0] * 5 + [1.0] * 4})  # median -1.0
        code, _, err = self.run_gate_capture("--base", str(negative), "--head", str(head))
        self.assertEqual(code, 2)
        self.assertIn("first-pass base median for 's' is -1.0; must be > 0", err)
        # Confirm rounds alone with a negative base median (first pass fine).
        good = self.write("b2.json", {"s": flat(1.0)})
        cb = self.write("cb.json", {"s": flat(-0.5)})
        code, _, err = self.run_gate_capture(
            "--mode", "confirm", "--base", str(good), "--head", str(head),
            "--confirm-base", str(cb), "--confirm-head", str(head),
        )
        self.assertEqual(code, 2)
        self.assertIn("confirm-round base median for 's' is -0.5", err)
        # A single negative run under a positive median is not an input error.
        some_negative = self.write("b3.json", {"s": [-1.0] + [1.0] * 8})
        self.assertEqual(self.run_gate("--base", str(some_negative), "--head", str(head)), 0)

    def test_pooled_base_median_check(self) -> None:
        # The pooled check runs after the first-pass and confirm-round checks. A pooled median sits
        # between the two parts' medians, so real reports cannot reach it with both parts > 0
        # (asserted on a seeded search below). The CLI wiring is still tested: with the two earlier
        # checks stubbed out, a pooled base median <= 0 exits 2 with the "pooled" message.
        rng = random.Random(7)
        choices = [-100.0, -10.0, -1.0, 0.0, 0.5, 1.0, 2.0, 50.0, 101.0]
        for _ in range(20000):
            a = [rng.choice(choices) for _ in range(rng.randint(1, 6))]
            b = [rng.choice(choices) for _ in range(rng.randint(1, 6))]
            if statistics.median(a) > 0 and statistics.median(b) > 0:
                self.assertGreater(statistics.median(a + b), 0, (a, b))

        head = self.write("h.json", {"s": flat(1.0)})
        first_base = self.write("b.json", {"s": [-1.0] * 5 + [1.0] * 4})  # median -1
        confirm_base = self.write("cb.json", {"s": [-1.0] * 5 + [1.0] * 4})  # median -1
        real_check = gate.check_base_medians
        seen: list[str] = []

        def only_pooled(label: str, base: dict[str, list[float]]) -> None:
            seen.append(label)
            if label == "pooled":
                real_check(label, base)

        args = ("--mode", "confirm", "--base", str(first_base), "--head", str(head),
                "--confirm-base", str(confirm_base), "--confirm-head", str(head))
        with mock.patch.object(gate, "check_base_medians", side_effect=only_pooled):
            code, _, err = self.run_gate_capture(*args)
        self.assertEqual(seen, ["first-pass", "confirm-round", "pooled"])
        self.assertEqual(code, 2)
        self.assertIn("pooled base median for 's' is -1.0; must be > 0", err)
        # The pooled check itself, for 0 and for a negative median.
        for values, shown in (([0.0] * 10 + [1.0] * 8, "0.0"), ([-2.0] * 10 + [1.0] * 8, "-2.0")):
            with self.assertRaises(gate.InputError) as caught:
                gate.check_base_medians("pooled", {"s": values})
            self.assertIn(f"pooled base median for 's' is {shown}", str(caught.exception))
        gate.check_base_medians("pooled", {"s": [0.0] * 8 + [1.0] * 10})  # median 1.0: fine

    # --- confirm paths --------------------------------------------------------------------

    def test_pooled_alone_does_not_fail(self) -> None:
        # Pooled (first + 1 round): +24.5% and significant. Confirm-only: +9%, under the bar.
        # NEW rule: pooled is report-only — must not FAIL when confirm-only is not significant.
        fb, fh = flat(1.0), flat(1.4)
        cb, ch = [flat(1.0)], [flat(1.09)]
        row = self.confirm_row(fb, fh, cb, ch)
        self.assertGreater(row.delta, FAIL)
        self.assertLessEqual(row.confirm_delta, FAIL)
        self.assertEqual(row.status, "ok")
        self.assertEqual(self.decide(fb, fh, cb, ch), "pass")

    def test_path_b_alone_fails(self) -> None:
        # First pass +7% (flagged). Confirm rounds alone: median +14%, every head run slower.
        # Pooled: the first-pass head runs pull the pooled head median down to +8%: (a) passes.
        fb, fh = flat(1.0), flat(1.07)
        confirm_head = [1.08] * 13 + [1.14] * 14
        cb = [flat(1.0)] * CONFIRM_ROUNDS
        ch = [confirm_head[i * RUNS:(i + 1) * RUNS] for i in range(CONFIRM_ROUNDS)]
        row = self.confirm_row(fb, fh, cb, ch)
        self.assertLessEqual(row.delta, FAIL)
        self.assertGreater(row.confirm_delta, FAIL)
        self.assertLess(row.confirm_p, gate.DEFAULT_ALPHA)
        self.assertEqual(row.status, "FAIL (confirm-only)")
        self.assertEqual(self.decide(fb, fh, cb, ch), "fail")

    def test_both_paths_fail(self) -> None:
        fb, fh = flat(1.0), flat(1.3)
        cb, ch = [flat(1.0)] * CONFIRM_ROUNDS, [flat(1.3)] * CONFIRM_ROUNDS
        self.assertEqual(self.confirm_row(fb, fh, cb, ch).status, "FAIL (confirm-only)")

    def test_neither_path_fails(self) -> None:
        # A +20% first-pass spike that the confirm rounds do not reproduce.
        fb, fh = flat(1.0), flat(1.2)
        cb, ch = [flat(1.0)] * CONFIRM_ROUNDS, [flat(1.0)] * CONFIRM_ROUNDS
        row = self.confirm_row(fb, fh, cb, ch)
        self.assertLessEqual(row.delta, FAIL)
        self.assertLessEqual(row.confirm_delta, FAIL)
        self.assertEqual(row.status, "ok")
        self.assertEqual(self.decide(fb, fh, cb, ch), "pass")

    def test_confirm_only_significant_fails_with_n_ge_12(self) -> None:
        # Confirm-only +14% significant with 12 samples/side; pooled diluted below the bar.
        fb, fh = flat(1.0), flat(1.07)
        cb = [1.0] * 12
        ch = [1.14] * 12
        row = gate.evaluate_confirm({"s": fb}, {"s": fh}, {"s": cb}, {"s": ch}, FAIL, [])[0]
        self.assertEqual(row.confirm_n_base, 12)
        self.assertEqual(row.confirm_n_head, 12)
        self.assertGreater(row.confirm_delta, FAIL)
        self.assertLess(row.confirm_p, gate.DEFAULT_ALPHA)
        self.assertEqual(row.status, "FAIL (confirm-only)")
        self.assertEqual(gate.decide(fb, fh, [cb], [ch], FLAG, FAIL), "fail")

    def test_confirm_only_significant_but_n_lt_12_does_not_fail(self) -> None:
        fb, fh = flat(1.0), flat(1.07)
        cb = [1.0] * 11
        ch = [1.14] * 11
        row = gate.evaluate_confirm({"s": fb}, {"s": fh}, {"s": cb}, {"s": ch}, FAIL, [])[0]
        self.assertEqual(row.status, "ok (confirm n<12)")
        self.assertEqual(gate.decide(fb, fh, [cb], [ch], FLAG, FAIL), "pass")

    def test_confirm_only_not_significant_passes_even_if_pooled_is(self) -> None:
        # Same shape as pooled-alone: confirm under the bar → PASS despite pooled FAIL under old OR rule.
        fb, fh = [1.0] * 12, [1.4] * 12
        cb, ch = [1.0] * 12, [1.09] * 12
        row = gate.evaluate_confirm({"s": fb}, {"s": fh}, {"s": cb}, {"s": ch}, FAIL, [])[0]
        self.assertGreater(row.delta, FAIL)
        self.assertLess(row.p_value, gate.DEFAULT_ALPHA)
        self.assertLessEqual(row.confirm_delta, FAIL)
        self.assertEqual(row.status, "ok")

    def test_large_delta_without_significance_passes(self) -> None:
        # Delta > bar on both paths, but the runs interleave: neither Mann-Whitney test fires.
        base = [1.0, 1.0, 1.0, 1.25, 1.3, 1.3, 1.35, 1.0]
        head = [0.95, 0.98, 1.3, 1.3, 1.32, 1.35, 1.36, 0.97]
        row = gate.evaluate_confirm({"s": base}, {"s": head}, {"s": base}, {"s": head}, FAIL, [])[0]
        self.assertGreater(row.delta, FAIL)
        self.assertGreater(row.confirm_delta, FAIL)
        self.assertGreaterEqual(row.p_value, gate.DEFAULT_ALPHA)
        self.assertEqual(row.status, "ok")

    def test_delta_between_flag_and_fail_is_flagged_then_cleared(self) -> None:
        fb, fh = flat(1.0), flat(1.08)  # +8%: above flag 5, below fail 10
        self.assertEqual(gate.evaluate_first({"s": fb}, {"s": fh}, FLAG, [])[0].status, "FLAG")
        # Even when the confirm reproduces +8% exactly, nothing is above the bar.
        self.assertEqual(self.decide(fb, fh, [fb] * CONFIRM_ROUNDS, [fh] * CONFIRM_ROUNDS), "pass")
        self.assertEqual(self.decide(fb, fh, [flat(1.0)] * 3, [flat(1.0)] * 3), "pass")

    def test_confirm_rounds_zero_uses_fail_threshold(self) -> None:
        self.assertEqual(self.decide(flat(1.0), flat(1.08), [], []), "pass")  # flagged-only band passes
        self.assertEqual(self.decide(flat(1.0), flat(1.12), [], []), "fail")
        self.assertEqual(self.decide(flat(1.0), flat(1.0999), [], []), "pass")  # just under the bar

    def test_cli_confirm_mode_reports_both_paths(self) -> None:
        fb = self.write("base.json", {"s": flat(1.0), "calm": flat(1.0)})
        fh = self.write("head.json", {"s": flat(1.07), "calm": flat(1.0)})
        confirm_head = [1.08] * 13 + [1.14] * 14
        args = ["--mode", "confirm", "--base", str(fb), "--head", str(fh)]
        for i in range(CONFIRM_ROUNDS):
            args += ["--confirm-base", str(self.write(f"cb{i}.json", {"s": flat(1.0)}))]
            args += ["--confirm-head", str(self.write(f"ch{i}.json", {"s": confirm_head[i * RUNS:(i + 1) * RUNS]}))]
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(gate.main(args), 1)
        self.assertIn("FAIL (confirm-only)", out.getvalue())
        self.assertIn("| calm |", out.getvalue())
        self.assertIn("result: fail (s)", out.getvalue())

    # --- earlier cases: one slow base run must not hide a regression ----------------------

    def test_one_slow_base_run_does_not_hide_17_percent(self) -> None:
        base, head = [1.0, 1.0, 1.0, 1.0, 1.5], [1.17] * 5
        self.assertFalse(old_rule_fails(base, head, FAIL))  # the original bug: spread 50% hid it
        self.assertEqual(gate.evaluate_first({"s": base}, {"s": head}, FLAG, [])[0].status, "FLAG")
        self.assertEqual(self.decide(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS), "fail")
        self.assertEqual(self.decide(base, head, [], []), "fail")  # confirm_rounds=0

    def test_one_slow_base_run_does_not_hide_40_percent(self) -> None:
        base, head = [1.0, 1.0, 1.0, 1.0, 1.5], [1.4] * 5
        self.assertFalse(old_rule_fails(base, head, FAIL))
        self.assertEqual(self.decide(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS), "fail")
        self.assertEqual(self.decide(base, head, [], []), "fail")

    def test_one_slow_base_run_via_cli_confirm(self) -> None:
        base = self.write("base.json", {"s": [1.0, 1.0, 1.0, 1.0, 1.5]})
        head = self.write("head.json", {"s": [1.17] * 5})
        suspects = self.dir / "suspects.txt"
        self.assertEqual(self.run_gate("--base", str(base), "--head", str(head), "--suspects-out", str(suspects)), 1)
        self.assertEqual(suspects.read_text(encoding="utf-8"), "s\n")
        args = ["--mode", "confirm", "--base", str(base), "--head", str(head)]
        for i in range(CONFIRM_ROUNDS):
            args += ["--confirm-base", str(self.write(f"cb{i}.json", {"s": [1.0, 1.0, 1.0, 1.0, 1.5]}))]
            args += ["--confirm-head", str(self.write(f"ch{i}.json", {"s": [1.17] * 5}))]
        self.assertEqual(self.run_gate(*args), 1)

    def test_old_pooled_spread_passed_new_rule_fails(self) -> None:
        # Tight first-pass base, +20%: the old rule flagged it. Each confirm round carries one
        # slow base run, so the old pooled-spread confirm saw spread 50% > delta 20% and passed.
        first_base, first_head = [1.0, 1.02, 0.98, 1.0, 1.01], [1.2] * 5
        confirm_base = [[1.0, 1.0, 1.0, 1.0, 1.5]] * CONFIRM_ROUNDS
        confirm_head = [[1.2] * 5] * CONFIRM_ROUNDS
        self.assertTrue(old_rule_fails(first_base, first_head, FAIL))
        pooled_base = first_base + [v for r in confirm_base for v in r]
        pooled_head = first_head + [v for r in confirm_head for v in r]
        self.assertFalse(old_rule_fails(pooled_base, pooled_head, FAIL))
        self.assertEqual(self.decide(first_base, first_head, confirm_base, confirm_head), "fail")

    # --- duplicated-confirm property ------------------------------------------------------

    def test_flag_with_duplicated_confirm_data_fails(self) -> None:
        cases = [
            ([1.0, 1.0, 1.0, 1.0, 1.5], [1.17] * 5),
            ([1.0, 1.0, 1.0, 1.0, 1.5], [1.4] * 5),
            ([1.0, 1.01, 0.99, 1.0, 1.0], [1.2, 1.21, 1.19, 1.2, 1.2]),
            ([0.19, 0.2145, 0.23, 0.21, 0.22], [0.2391, 0.23, 0.24, 0.25, 0.235]),
            ([1.0, 1.1, 0.95, 1.3, 1.05], [1.2, 1.15, 1.25, 1.18, 1.4]),
        ]
        for base, head in cases:
            with self.subTest(base=base, head=head):
                self.assertGreater(gate.delta_percent(base, head), FAIL)
                for rounds in (1, 2, CONFIRM_ROUNDS, 5):
                    # Mann-Whitney property, default rule (abs floor off).
                    verdict = self.decide(base, head, [base] * rounds, [head] * rounds)
                    self.assertEqual(verdict, "fail")

    def duplicated_slips(self, runs: int, confirm_rounds: int | None = None) -> tuple[int, int]:
        # Cases whose first-pass delta is above the fail bar, re-"confirmed" with copies of the
        # first-pass runs. Path (b) then sees the same runs again, so a slip means both
        # Mann-Whitney tests stayed above alpha. Seeded, so the counts are fixed.
        rounds = CONFIRM_ROUNDS if confirm_rounds is None else confirm_rounds
        rng = random.Random(3)
        eligible = slipped = 0
        for _ in range(4000):
            base = invocation(rng, 1.0, runs)
            head = invocation(rng, 1.0 + rng.choice([15, 20, 30]) / 100.0, runs)
            if gate.delta_percent(base, head) <= FAIL:
                continue
            eligible += 1
            if self.decide(base, head, [base] * rounds, [head] * rounds) != "fail":
                slipped += 1
                # For small n the slip always coincides with at least one base >= head median;
                # at n=12 that diagnostic is not always true, so only assert it for n<=9.
                if runs <= 9:
                    self.assertGreaterEqual(sum(v >= statistics.median(head) for v in base), 1)
        return eligible, slipped

    def test_duplicated_confirm_on_simulated_cases(self) -> None:
        # runs=5 (old default): 47 of 3511 slip, as before; path (b) adds nothing on copied data.
        # runs=9 (new default): 6 of 3605.
        self.assertEqual(self.duplicated_slips(5), (3511, 47))
        self.assertEqual(self.duplicated_slips(RUNS), (3605, 6))

    def test_duplicated_confirm_parity_n12(self) -> None:
        # Parity default n=12, confirm_rounds=1: path (b) on copied data; count is locked.
        self.assertEqual(
            self.duplicated_slips(PARITY_RUNS, PARITY_CONFIRM_ROUNDS), (3638, 28)
        )

    # --- Mann-Whitney implementation ------------------------------------------------------

    def test_mann_whitney_exact_known_values(self) -> None:
        self.assertAlmostEqual(gate.mann_whitney_greater([1, 2, 3, 4, 5], [6, 7, 8, 9, 10]), 1 / 252)
        self.assertAlmostEqual(gate.mann_whitney_greater([6, 7, 8, 9, 10], [1, 2, 3, 4, 5]), 1.0)
        self.assertAlmostEqual(gate.mann_whitney_greater([1, 2], [3]), 1 / 3)
        self.assertAlmostEqual(gate.mann_whitney_greater([1, 1, 1, 1, 1.5], [1.17] * 5), 6 / 252)

    def test_mann_whitney_ties(self) -> None:
        self.assertEqual(gate.mann_whitney_greater([1.0] * 5, [1.0] * 5), 1.0)
        self.assertEqual(gate.mann_whitney_greater([1.0] * 30, [1.0] * 30), 1.0)  # zero variance
        p_tie = gate.mann_whitney_greater([1, 2, 3], [3, 4, 5])
        self.assertLess(gate.mann_whitney_greater([1, 2, 3], [4, 5, 6]), p_tie)
        self.assertLess(p_tie, gate.mann_whitney_greater([1, 2, 4], [3, 5, 6]) + 1e-12)

    def test_mann_whitney_normal_close_to_exact(self) -> None:
        rng = random.Random(11)
        base = [rng.uniform(1.0, 1.3) for _ in range(8)]
        head = [rng.uniform(1.05, 1.35) for _ in range(8)]
        exact = gate.mann_whitney_greater(base, head)
        saved = gate.EXACT_MAX_COMBINATIONS
        try:
            gate.EXACT_MAX_COMBINATIONS = 0
            approx = gate.mann_whitney_greater(base, head)
        finally:
            gate.EXACT_MAX_COMBINATIONS = saved
        self.assertAlmostEqual(exact, approx, delta=0.02)

    def test_mann_whitney_is_deterministic_for_pooled_defaults(self) -> None:
        rng = random.Random(5)
        base = [rng.uniform(1.0, 1.3) for _ in range(RUNS * (CONFIRM_ROUNDS + 1))]
        head = [v * 1.12 for v in base]
        self.assertEqual(gate.mann_whitney_greater(base, head), gate.mann_whitney_greater(base, head))

    def test_alpha_must_be_a_probability(self) -> None:
        base = self.write("b.json", {"s": [1.0]})
        for alpha in ("0", "1", "-0.1"):
            with redirect_stderr(io.StringIO()):
                self.assertEqual(
                    self.run_gate("--mode", "confirm", "--alpha", alpha, "--base", str(base), "--head", str(base)), 2
                )

    # --- seeded simulation, noise shaped like section 5.1 ---------------------------------

    def test_noise_model_matches_section_5_1_spreads(self) -> None:
        rng = random.Random(7)
        spreads = sorted(
            (max(x) - min(x)) / statistics.median(x) * 100 for x in (invocation(rng, 1.0, 5) for _ in range(4000))
        )
        median_spread = spreads[len(spreads) // 2]
        self.assertTrue(15.0 <= median_spread <= 30.0, median_spread)

    def test_simulation_at_workflow_defaults(self) -> None:
        # Historical frame-bench defaults (runs=9, confirm_rounds=3). Parity n=12/1 is
        # test_simulation_at_parity_defaults. Confirm-only FAIL; 4000 trials; seeds 2 then 1.
        # Measured: 0% passes 99.925%; +10% fails 38.675%; +12% fails 60.375%; +15% fails 85.3%.
        rates = {pct: simulate(pct, RUNS, CONFIRM_ROUNDS, FLAG, FAIL, trials=4000, seed=1) for pct in (10, 12, 15)}
        pass_0 = 1.0 - simulate(0.0, RUNS, CONFIRM_ROUNDS, FLAG, FAIL, trials=4000, seed=2)
        print(f"\n[sim runs={RUNS} rounds={CONFIRM_ROUNDS} flag={FLAG:g} fail={FAIL:g}] pass@0%={pass_0:.4f} "
              + " ".join(f"fail@+{k}%={v:.4f}" for k, v in rates.items()), file=sys.stderr)
        self.assertGreaterEqual(pass_0, 0.95)
        self.assertAlmostEqual(pass_0, 0.99925, places=5)
        self.assertAlmostEqual(rates[10], 0.38675, places=5)
        self.assertAlmostEqual(rates[12], 0.60375, places=5)
        self.assertAlmostEqual(rates[15], 0.853, places=5)
        self.assertTrue(0.30 <= rates[10] <= 0.55)

    def test_simulation_at_parity_defaults(self) -> None:
        # bench-pr / frame-bench parity: runs=12, confirm_rounds=1, flag=5, fail=10.
        rates = {
            pct: simulate(pct, PARITY_RUNS, PARITY_CONFIRM_ROUNDS, FLAG, FAIL, trials=4000, seed=1)
            for pct in (10, 12, 15)
        }
        pass_0 = 1.0 - simulate(
            0.0, PARITY_RUNS, PARITY_CONFIRM_ROUNDS, FLAG, FAIL, trials=4000, seed=2
        )
        print(
            f"\n[sim parity runs={PARITY_RUNS} rounds={PARITY_CONFIRM_ROUNDS} "
            f"flag={FLAG:g} fail={FAIL:g}] pass@0%={pass_0:.4f} "
            + " ".join(f"fail@+{k}%={v:.4f}" for k, v in rates.items()),
            file=sys.stderr,
        )
        self.assertGreaterEqual(pass_0, 0.95)
        self.assertAlmostEqual(pass_0, 0.99325, places=5)
        self.assertAlmostEqual(rates[10], 0.3985, places=5)
        self.assertAlmostEqual(rates[12], 0.55075, places=5)
        self.assertAlmostEqual(rates[15], 0.74825, places=5)

    def test_simulation_at_0_2_ms_floor_off_and_on(self) -> None:
        # Why the abs floor is opt-in (O8 open): today's lavapipe scenarios run at ~0.2-0.4 ms.
        # Same noise model and seeds as above at a true 0.2 ms median, 4000 trials each.
        # Confirm-only rule. Floor off: 0% passes 99.925%, +15% fails 85.3%, +20% fails
        # 98.525%. 0.05 ms floor: 0% passes 100%, +15% fails 0.0%, +20% fails 2.65%.
        def rate(pct: float, floor: float, seed: int = 1) -> float:
            return simulate(pct, RUNS, CONFIRM_ROUNDS, FLAG, FAIL, trials=4000, seed=seed, true_ms=0.2,
                            abs_floor_ms=floor)

        off = {pct: rate(pct, gate.DEFAULT_ABS_FLOOR_MS) for pct in (15, 20)}
        on = {pct: rate(pct, FLOOR) for pct in (15, 20)}
        pass_0_off = 1.0 - rate(0.0, gate.DEFAULT_ABS_FLOOR_MS, seed=2)
        pass_0_on = 1.0 - rate(0.0, FLOOR, seed=2)
        print(f"\n[sim 0.2 ms] floor off: pass@0%={pass_0_off:.4f} fail@+15%={off[15]:.4f} fail@+20%={off[20]:.4f}; "
              f"floor {FLOOR:g} ms: pass@0%={pass_0_on:.4f} fail@+15%={on[15]:.4f} fail@+20%={on[20]:.4f}",
              file=sys.stderr)
        self.assertEqual(gate.DEFAULT_ABS_FLOOR_MS, 0.0)
        self.assertAlmostEqual(off[15], 0.853, places=5)
        self.assertAlmostEqual(off[20], 0.98525, places=5)
        self.assertAlmostEqual(pass_0_off, 0.99925, places=5)
        self.assertAlmostEqual(on[15], 0.0, places=5)
        self.assertAlmostEqual(on[20], 0.0265, places=5)
        self.assertAlmostEqual(pass_0_on, 1.0, places=5)
        self.assertGreaterEqual(off[15], 0.85)  # floor off keeps the +15% detection target
        self.assertLessEqual(on[15], 0.05)  # a 0.05 ms floor suppresses it at 0.2 ms

    # --- schema 2 ---------------------------------------------------------------------------

    def test_schema_2_report_is_accepted(self) -> None:
        path = self.write2("h2.json", {"s": [1.0, 2.0]})
        self.assertEqual(gate.load([path]), {"s": [1.0, 2.0]})

    def test_schema_1_base_and_schema_2_head_via_cli(self) -> None:
        # The frame-bench.yml case for B1: base = master (schema 1), head = PR (schema 2).
        base = self.write("b.json", {"s": flat(2.0), "t": flat(1.0)})
        head = self.write2("h.json", {"s": flat(2.0), "t": flat(1.0)})
        code, out, _ = self.run_gate_capture("--base", str(base), "--head", str(head))
        self.assertEqual(code, 0)
        self.assertIn("| s | 2.0000 | 2.0000 | +0.00% | 9/9 | ok |", out)
        slow = self.write2("hs.json", {"s": flat(2.4), "t": flat(1.0)})
        suspects = self.dir / "suspects.txt"
        code = self.run_gate("--base", str(base), "--head", str(slow), "--suspects-out", str(suspects))
        self.assertEqual(code, 1)
        self.assertEqual(suspects.read_text(encoding="utf-8"), "s\n")

    def test_schema_1_and_2_pool_the_same(self) -> None:
        one = self.write("one.json", {"s": [1.0, 1.5]})
        two = self.write2("two.json", {"s": [1.0, 1.5]})
        self.assertEqual(gate.load([one]), gate.load([two]))
        self.assertEqual(gate.load([one, two]), {"s": [1.0, 1.5, 1.0, 1.5]})

    def test_confirm_mode_accepts_mixed_schemas(self) -> None:
        base = self.write("b.json", {"s": flat(2.0)})
        head = self.write2("h.json", {"s": flat(2.4)})
        # n=12 so CONFIRM_MIN_N allows FAIL (confirm-only) on the +20% confirm delta.
        cb = self.write("cb.json", {"s": [2.0] * 12})
        ch = self.write2("ch.json", {"s": [2.4] * 12})
        code = self.run_gate("--mode", "confirm", "--base", str(base), "--head", str(head),
                             "--confirm-base", str(cb), "--confirm-head", str(ch))
        self.assertEqual(code, 1)

    def test_schema_2_without_runner_is_an_input_error(self) -> None:
        doc = report2({"s": flat(1.0)})
        del doc["runner"]
        path = self.write_raw("h.json", doc)
        base = self.write("b.json", {"s": flat(1.0)})
        self.assertEqual(self.run_gate("--base", str(base), "--head", str(path)), 2)

    def test_unsupported_schema_values_are_input_errors(self) -> None:
        base = self.write("b.json", {"s": flat(1.0)})
        for value in (0, 3, "2", True, None, 2.0):
            with self.subTest(schema=value):
                doc = report2({"s": flat(1.0)})
                doc["schema"] = value
                head = self.write_raw("h.json", doc)
                self.assertEqual(self.run_gate("--base", str(base), "--head", str(head)), 2)

    def test_framebench_error_document_is_an_input_error(self) -> None:
        # `renderide-framebench run --json` failure output: schema, error, runner; no scenarios.
        doc = {"schema": 2, "error": {"exit_code": 3, "kind": "run_failure", "message": "handshake timed out"},
               "runner": dict(RUNNER)}
        bad = self.write_raw("err.json", doc)
        good = self.write2("ok.json", {"s": flat(1.0)})
        code, _, err = self.run_gate_capture("--base", str(good), "--head", str(bad))
        self.assertEqual(code, 2)
        self.assertIn("error document", err)
        # On the base side too: an error document must not silently become "base N/A" rows.
        self.assertEqual(self.run_gate("--base", str(bad), "--head", str(good)), 2)

    def test_runner_lines_are_rendered_for_schema_2(self) -> None:
        base = self.write("b.json", {"s": flat(1.0)})
        head = self.write2("h.json", {"s": flat(1.0)})
        _, out, _ = self.run_gate_capture("--base", str(base), "--head", str(head))
        self.assertIn('- head runner: os=linux cpu="AMD EPYC 7763 64-Core Processor" '
                      "icd=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json (VK_DRIVER_FILES) rayon_threads=2 (env)", out)
        self.assertNotIn("base runner:", out)  # schema-1 base has no runner block

    # --- absolute floor ---------------------------------------------------------------------

    def test_abs_floor_clears_issue_178_shape(self) -> None:
        # The #178 shape: +11.5% at ~0.21 ms is a 0.025 ms delta, so it never flags or fails.
        base = [0.19, 0.2145, 0.23, 0.21, 0.22]
        head = [0.2391, 0.23, 0.24, 0.25, 0.235]
        row = gate.evaluate_first({"s": base}, {"s": head}, FLAG, [], FLOOR)[0]
        self.assertGreater(row.delta, FAIL)
        self.assertEqual(row.status, "ok (abs floor)")
        self.assertFalse(gate.is_failure(row.status))
        cb, ch = [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS
        self.assertEqual(gate.decide(base, head, cb, ch, FLAG, FAIL, abs_floor_ms=FLOOR), "pass")
        self.assertEqual(gate.decide(base, head, [], [], FLAG, FAIL, abs_floor_ms=FLOOR), "pass")
        self.assertEqual(self.decide(base, head, cb, ch), "fail")  # default: floor off

    def test_abs_floor_boundary_is_inclusive(self) -> None:
        # Exactly 0.05 ms (+50% of 0.1 ms) does not flag; 0.0501 ms does.
        def status(base: float, head: float, flag: float) -> str:
            return gate.evaluate_first({"s": flat(base)}, {"s": flat(head)}, flag, [], FLOOR)[0].status

        self.assertEqual(status(0.1, 0.15, FLAG), "ok (abs floor)")
        self.assertEqual(status(1.0, 1.05, 4.0), "ok (abs floor)")
        self.assertEqual(status(0.1, 0.1501, FLAG), "FLAG")

    def test_abs_floor_applies_to_both_confirm_paths(self) -> None:
        # Large, significant relative regressions under the floor never fail; above it they do.
        base, head = [0.1, 0.101, 0.099, 0.1, 0.1], [0.14, 0.141, 0.139, 0.14, 0.14]  # +40%, 0.04 ms
        row = self.confirm_row(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS, FLOOR)
        self.assertEqual(row.status, "ok (abs floor)")
        self.assertLess(row.p_value, 0.05)
        self.assertLess(row.confirm_p, 0.05)
        big_base = [v * 10 for v in base]
        big_head = [v * 10 for v in head]  # +40%, 0.4 ms
        row = self.confirm_row(big_base, big_head, [big_base] * CONFIRM_ROUNDS, [big_head] * CONFIRM_ROUNDS, FLOOR)
        self.assertEqual(row.status, "FAIL (confirm-only)")
        row = self.confirm_row(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS)
        self.assertEqual(row.status, "FAIL (confirm-only)")  # default: floor off

    def test_abs_floor_never_hides_a_regression_above_it(self) -> None:
        # A +20% regression at 2 ms (0.4 ms) still flags and fails, with or without the floor.
        base, head = [2.0, 2.02, 1.98, 2.0, 2.0], [2.4, 2.42, 2.38, 2.4, 2.4]
        for floor in (0.0, FLOOR):
            with self.subTest(floor=floor):
                verdict = gate.decide(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS, FLAG, FAIL,
                                      abs_floor_ms=floor)
                self.assertEqual(verdict, "fail")

    def test_abs_floor_cli(self) -> None:
        base = self.write("b.json", {"s": flat(0.2)})
        head = self.write2("h.json", {"s": flat(0.24)})  # +20%, 0.04 ms
        common = ["--base", str(base), "--head", str(head)]
        code, out, _ = self.run_gate_capture(*common)
        self.assertEqual(code, 1)  # default: floor off, +20% flags
        self.assertNotIn("abs floor", out.lower())
        code, out, _ = self.run_gate_capture(*common, "--abs-floor-ms", "0.05")
        self.assertEqual(code, 0)
        self.assertIn("| ok (abs floor) |", out)
        self.assertIn("<= 0.05 ms never flags or fails", out)
        self.assertEqual(self.run_gate(*common, "--abs-floor-ms", "0"), 1)
        self.assertEqual(self.run_gate(*common, "--abs-floor-ms", "0.01"), 1)
        self.assertEqual(self.run_gate(*common, "--abs-floor-ms", "-0.01"), 2)
        self.assertEqual(self.run_gate(*common, "--abs-floor-ms", "nan"), 2)
        self.assertEqual(self.run_gate(*common, "--abs-floor-ms", "inf"), 2)



class BaseDriftGuardTests(unittest.TestCase):
    """Confirm-base median drift >15% vs first-pass base → inconclusive / fail-closed."""

    def test_drift_triggers_inconclusive(self) -> None:
        fb = [1.0] * 12
        fh = [1.2] * 12
        # confirm base median 1.20 → 20% drift
        cb = [1.20] * 12
        ch = [1.25] * 12
        row = gate.evaluate_confirm({"s": fb}, {"s": fh}, {"s": cb}, {"s": ch}, FAIL, [])[0]
        self.assertGreater(row.base_drift_pct, gate.BASE_DRIFT_PCT)
        self.assertEqual(row.status, "INCONCLUSIVE (base drift)")
        self.assertIsNone(row.deciding_round)
        self.assertTrue(gate.is_failure(row.status))
        self.assertTrue(gate.is_inconclusive_drift(row.status))

    def test_drift_then_clean_extra_round_pass(self) -> None:
        fb = [1.0] * 12
        fh = [1.2] * 12  # first FLAG
        cb1 = [1.20] * 12  # 20% drift
        ch1 = [1.30] * 12
        cb2 = [1.01] * 12  # clean (~1% drift)
        ch2 = [1.05] * 12  # confirm-only +4% under fail bar → PASS
        self.assertEqual(
            gate.decide(fb, fh, [cb1], [ch1], FLAG, FAIL, confirm_base_2=[cb2], confirm_head_2=[ch2]),
            "pass",
        )
        row2 = gate.evaluate_confirm(
            {"s": fb}, {"s": fh}, {"s": cb2}, {"s": ch2}, FAIL, [],
            fail_closed_drift=True, confirm_round=2,
        )[0]
        self.assertLessEqual(row2.base_drift_pct, gate.BASE_DRIFT_PCT)
        self.assertEqual(row2.status, "ok")
        self.assertEqual(row2.deciding_round, 2)

    def test_drift_then_clean_extra_round_fail(self) -> None:
        fb = [1.0] * 12
        fh = [1.2] * 12
        cb1 = [1.20] * 12
        ch1 = [1.30] * 12
        cb2 = [1.02] * 12  # clean
        ch2 = [1.20] * 12  # confirm-only +~18% significant → FAIL
        self.assertEqual(
            gate.decide(fb, fh, [cb1], [ch1], FLAG, FAIL, confirm_base_2=[cb2], confirm_head_2=[ch2]),
            "fail",
        )
        row2 = gate.evaluate_confirm(
            {"s": fb}, {"s": fh}, {"s": cb2}, {"s": ch2}, FAIL, [],
            fail_closed_drift=True, confirm_round=2,
        )[0]
        self.assertEqual(row2.status, "FAIL (confirm-only)")
        self.assertEqual(row2.deciding_round, 2)

    def test_drift_twice_fails_closed_with_inconclusive_reason(self) -> None:
        fb = [1.0] * 12
        fh = [1.2] * 12
        cb1 = [1.20] * 12
        ch1 = [1.25] * 12
        cb2 = [1.25] * 12  # 25% drift again
        ch2 = [1.40] * 12
        self.assertEqual(
            gate.decide(fb, fh, [cb1], [ch1], FLAG, FAIL, confirm_base_2=[cb2], confirm_head_2=[ch2]),
            "fail",
        )
        row2 = gate.evaluate_confirm(
            {"s": fb}, {"s": fh}, {"s": cb2}, {"s": ch2}, FAIL, [],
            fail_closed_drift=True, confirm_round=2,
        )[0]
        self.assertEqual(row2.status, "FAIL (inconclusive: base drift)")
        self.assertIsNone(row2.deciding_round)
        # Render must state the inconclusive reason.
        out = gate.render([row2], "t", FLAG, FAIL, mode="confirm")
        self.assertIn("inconclusive: base drift", out)
        self.assertIn("base drift %", out)
        self.assertIn(f"{row2.base_drift_pct:.2f}%", out)

    def test_drift_without_extra_round_fails_closed_in_decide(self) -> None:
        fb = [1.0] * 12
        fh = [1.2] * 12
        cb1 = [1.20] * 12
        ch1 = [1.25] * 12
        self.assertEqual(gate.decide(fb, fh, [cb1], [ch1], FLAG, FAIL), "fail")

    def test_render_includes_drift_columns_and_inconclusive(self) -> None:
        fb, fh = [1.0] * 12, [1.2] * 12
        cb, ch = [1.20] * 12, [1.25] * 12
        row = gate.evaluate_confirm({"s": fb}, {"s": fh}, {"s": cb}, {"s": ch}, FAIL, [])[0]
        out = gate.render([row], "Confirm drift", FLAG, FAIL, mode="confirm")
        self.assertIn("INCONCLUSIVE (base drift)", out)
        self.assertIn("base drift %", out)
        self.assertIn("deciding round", out)
        self.assertIn("confirm round", out)


class MergeConfirmTests(unittest.TestCase):
    def test_merge_replaces_inconclusive_with_round2(self) -> None:
        import bench_pr_merge_confirm as merge

        g1 = {
            "schema": "frame-bench-gate/1",
            "mode": "confirm",
            "exit_code": 1,
            "rows": [
                {
                    "scenario": "mpu",
                    "status": "INCONCLUSIVE (base drift)",
                    "base_drift_pct": 20.0,
                    "confirm_round": 1,
                    "deciding_round": None,
                },
                {
                    "scenario": "unlit",
                    "status": "FAIL (confirm-only)",
                    "base_drift_pct": 5.0,
                    "confirm_round": 1,
                    "deciding_round": 1,
                },
            ],
        }
        g2 = {
            "schema": "frame-bench-gate/1",
            "mode": "confirm",
            "exit_code": 1,
            "rows": [
                {
                    "scenario": "mpu",
                    "status": "FAIL (confirm-only)",
                    "base_drift_pct": 3.0,
                    "confirm_round": 2,
                    "deciding_round": 2,
                    "confirm_delta_pct": 14.0,
                }
            ],
        }
        out = merge.merge_confirm_gates(g1, g2)
        by = {r["scenario"]: r for r in out["rows"]}
        self.assertEqual(by["mpu"]["status"], "FAIL (confirm-only)")
        self.assertEqual(by["mpu"]["deciding_round"], 2)
        self.assertEqual(by["mpu"]["base_drift_pct_round1"], 20.0)
        self.assertEqual(by["mpu"]["base_drift_rounds"][0]["round"], 1)
        self.assertEqual(by["mpu"]["base_drift_rounds"][1]["base_drift_pct"], 3.0)
        self.assertEqual(by["unlit"]["status"], "FAIL (confirm-only)")
        self.assertEqual(out["exit_code"], 1)


if __name__ == "__main__":
    unittest.main()


class JsonOutTests(unittest.TestCase):
    def test_json_out_writes_rows(self):
        base = {
            "schema": 1,
            "scenarios": [{
                "name": "unlit_sphere",
                "cpu_frame_ms": {"p95": 1.0},
                "per_run_cpu_frame_ms_p95": [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            }],
        }
        head = {
            "schema": 1,
            "scenarios": [{
                "name": "unlit_sphere",
                "cpu_frame_ms": {"p95": 1.2},
                "per_run_cpu_frame_ms_p95": [1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2],
            }],
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "base.json").write_text(json.dumps(base), encoding="utf-8")
            (root / "head.json").write_text(json.dumps(head), encoding="utf-8")
            out = root / "gate.json"
            code = gate.main([
                "--base", str(root / "base.json"),
                "--head", str(root / "head.json"),
                "--flag-threshold", "5",
                "--fail-threshold", "10",
                "--json-out", str(out),
            ])
            self.assertEqual(code, 1)  # flagged at +20%
            payload = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(payload["schema"], "frame-bench-gate/1")
            self.assertEqual(payload["mode"], "first")
            self.assertEqual(payload["exit_code"], 1)
            self.assertEqual(len(payload["rows"]), 1)
            row = payload["rows"][0]
            self.assertEqual(row["scenario"], "unlit_sphere")
            self.assertAlmostEqual(row["delta_pct"], 20.0, places=6)
            self.assertEqual(row["status"], "FLAG")
