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
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import frame_bench_gate as gate  # noqa: E402

# Workflow defaults (.github/workflows/frame-bench.yml).
RUNS = 5
CONFIRM_ROUNDS = 3
THRESHOLD = 10.0

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


def simulate(true_pct: float, runs: int, rounds: int, threshold: float, trials: int, seed: int) -> float:
    """Fraction of trials where the whole gate (first pass + pooled confirm) fails."""
    rng = random.Random(seed)
    fails = 0
    for _ in range(trials):
        first_base = invocation(rng, 1.0, runs)
        first_head = invocation(rng, 1.0 + true_pct / 100.0, runs)
        confirm_base, confirm_head = [], []
        for _ in range(rounds):  # interleaved: base round, head round
            confirm_base.append(invocation(rng, 1.0, runs))
            confirm_head.append(invocation(rng, 1.0 + true_pct / 100.0, runs))
        fails += gate.decide(first_base, first_head, confirm_base, confirm_head, threshold) == "fail"
    return fails / trials


def old_rule_fails(base: list[float], head: list[float], threshold: float) -> bool:
    """The rule Prism rejected on #192 (b56748f6): fail when delta > max(threshold, base spread)."""
    base_median = statistics.median(base)
    delta = (statistics.median(head) - base_median) / base_median * 100.0
    spread = (max(base) - min(base)) / base_median * 100.0
    return delta > threshold and delta > spread


def report(scenarios: dict[str, list[float]]) -> dict:
    return {
        "schema": 1,
        "scenarios": [
            {"name": name, "cpu_frame_ms": {"p50": 0.0, "p95": runs[0], "p99": 0.0}, "per_run_cpu_frame_ms_p95": runs}
            for name, runs in scenarios.items()
        ],
    }


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

    def run_gate(self, *args: str) -> int:
        with redirect_stdout(io.StringIO()):
            return gate.main(list(args))

    # --- existing cases, updated for the new semantics -------------------------------------

    def test_delta_inside_base_spread_is_flagged(self) -> None:
        # Shape of #178 run 40050133923: +~11% on p95 while base runs spread ~19%. The old rule
        # passed this because delta < spread; the first pass must now flag it for the confirm.
        base = {"unlit_sphere": [0.19, 0.2145, 0.23, 0.21, 0.22]}
        head = {"unlit_sphere": [0.2391, 0.23, 0.24, 0.25, 0.235]}
        row = gate.evaluate(base, head, THRESHOLD, [])[0]
        self.assertGreater(row.delta, THRESHOLD)
        self.assertEqual(row.status, "FAIL")
        # A confirm whose fresh runs show no regression clears it (pooled delta drops below 10%).
        calm = [[0.21, 0.215, 0.22, 0.212, 0.218]] * CONFIRM_ROUNDS
        self.assertEqual(gate.decide(base["unlit_sphere"], head["unlit_sphere"], calm, calm, THRESHOLD), "pass")

    def test_delta_beyond_threshold_fails(self) -> None:
        base = self.write("b.json", {"s": [1.0, 1.01, 0.99, 1.0, 1.0]})
        head = self.write("h.json", {"s": [1.2, 1.21, 1.19, 1.2, 1.2]})
        suspects = self.dir / "suspects.txt"
        code = self.run_gate("--base", str(base), "--head", str(head), "--suspects-out", str(suspects))
        self.assertEqual(code, 1)
        self.assertEqual(suspects.read_text(encoding="utf-8"), "s\n")

    def test_threshold_equality_passes(self) -> None:
        base = self.write("b.json", {"s": [1.0, 1.0, 1.0]})
        head = self.write("h.json", {"s": [1.1, 1.1, 1.1]})
        for mode in ("first", "confirm"):
            rows = gate.evaluate(gate.load([base]), gate.load([head]), 10.000001, [], mode=mode)
            self.assertEqual(rows[0].status, "ok", mode)

    def test_reports_are_pooled(self) -> None:
        b1 = self.write("b1.json", {"s": [1.0]})
        b2 = self.write("b2.json", {"s": [2.0, 3.0]})
        self.assertEqual(gate.load([b1, b2]), {"s": [1.0, 2.0, 3.0]})

    def test_base_missing_and_head_missing_never_fail(self) -> None:
        base = self.write("b.json", {"a": [1.0, 1.0]})
        head = self.write("h.json", {"a": [1.0, 1.0], "new_scene": [5.0, 5.0]})
        for mode in ("first", "confirm"):
            rows = gate.evaluate(gate.load([base]), gate.load([head]), 10.0, ["gone_scene"], mode=mode)
            status = {row.scenario: row.status for row in rows}
            self.assertEqual(status, {"a": "ok", "new_scene": "base N/A", "gone_scene": "head N/A, skipped"})
            md = gate.render(rows, "t", 10.0, mode=mode)
            self.assertIn("| gone_scene | n/a | n/a | n/a | 0/0 | n/a | head N/A, skipped |", md)

    def test_wrong_schema_is_an_input_error(self) -> None:
        path = self.dir / "bad.json"
        path.write_text(json.dumps({"schema": 2, "scenarios": []}), encoding="utf-8")
        self.assertEqual(self.run_gate("--base", str(path), "--head", str(path)), 2)

    # --- (a) Prism's cases: one slow base run must not hide a regression -------------------

    def test_one_slow_base_run_does_not_hide_17_percent(self) -> None:
        base, head = [1.0, 1.0, 1.0, 1.0, 1.5], [1.17] * 5
        self.assertFalse(old_rule_fails(base, head, THRESHOLD))  # the bug: spread 50% hid it
        self.assertEqual(gate.evaluate({"s": base}, {"s": head}, THRESHOLD, [])[0].status, "FAIL")
        # Confirm rounds of the same shape (one slow base run each) still fail.
        self.assertEqual(gate.decide(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS, THRESHOLD), "fail")
        self.assertEqual(gate.decide(base, head, [], [], THRESHOLD), "fail")  # confirm_rounds=0

    def test_one_slow_base_run_does_not_hide_40_percent(self) -> None:
        base, head = [1.0, 1.0, 1.0, 1.0, 1.5], [1.4] * 5
        self.assertFalse(old_rule_fails(base, head, THRESHOLD))
        self.assertEqual(gate.decide(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS, THRESHOLD), "fail")
        self.assertEqual(gate.decide(base, head, [], [], THRESHOLD), "fail")

    def test_one_slow_base_run_via_cli_confirm(self) -> None:
        base = self.write("base.json", {"s": [1.0, 1.0, 1.0, 1.0, 1.5]})
        head = self.write("head.json", {"s": [1.17] * 5})
        suspects = self.dir / "suspects.txt"
        self.assertEqual(self.run_gate("--base", str(base), "--head", str(head), "--suspects-out", str(suspects)), 1)
        self.assertEqual(suspects.read_text(encoding="utf-8"), "s\n")
        args = ["--mode", "confirm", "--base", str(base), "--head", str(head)]
        for i in range(CONFIRM_ROUNDS):
            args += ["--base", str(self.write(f"cb{i}.json", {"s": [1.0, 1.0, 1.0, 1.0, 1.5]}))]
            args += ["--head", str(self.write(f"ch{i}.json", {"s": [1.17] * 5}))]
        self.assertEqual(self.run_gate(*args), 1)

    # --- (b) the confirm is no easier than the first pass ----------------------------------

    def test_old_pooled_spread_passed_new_rule_fails(self) -> None:
        # First pass: tight base, +20%: both rules flag. Confirm rounds each carry one slow base
        # run, so the old pooled-spread rule saw spread 50% > delta 20% and passed. New rule fails.
        first_base, first_head = [1.0, 1.02, 0.98, 1.0, 1.01], [1.2] * 5
        confirm_base = [[1.0, 1.0, 1.0, 1.0, 1.5]] * CONFIRM_ROUNDS
        confirm_head = [[1.2] * 5] * CONFIRM_ROUNDS
        self.assertTrue(old_rule_fails(first_base, first_head, THRESHOLD))
        pooled_base = first_base + [v for r in confirm_base for v in r]
        pooled_head = first_head + [v for r in confirm_head for v in r]
        self.assertFalse(old_rule_fails(pooled_base, pooled_head, THRESHOLD))
        self.assertEqual(gate.decide(first_base, first_head, confirm_base, confirm_head, THRESHOLD), "fail")

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
                self.assertEqual(gate.evaluate({"s": base}, {"s": head}, THRESHOLD, [])[0].status, "FAIL")
                for rounds in (1, 2, CONFIRM_ROUNDS, 5):
                    self.assertEqual(gate.decide(base, head, [base] * rounds, [head] * rounds, THRESHOLD), "fail")

    def test_duplicated_confirm_on_simulated_flags(self) -> None:
        # Pooled delta of duplicated data equals the first-pass delta, so (a) always holds; (b)
        # is the only escape. Measure how often it escapes on noise-model flags with a true
        # regression >= 15% (known limit, see the gate docstring: slow base runs at or above the
        # head median plus heavy overlap). Seeded: 47 of 3511 flags escape (1.3%).
        rng = random.Random(3)
        flagged = escaped = 0
        for _ in range(4000):
            base = invocation(rng, 1.0, RUNS)
            head = invocation(rng, 1.0 + rng.choice([15, 20, 30]) / 100.0, RUNS)
            if gate.evaluate({"s": base}, {"s": head}, THRESHOLD, [])[0].status != "FAIL":
                continue
            flagged += 1
            if gate.decide(base, head, [base] * CONFIRM_ROUNDS, [head] * CONFIRM_ROUNDS, THRESHOLD) != "fail":
                escaped += 1
                self.assertGreaterEqual(sum(v >= statistics.median(head) for v in base), 1)
        self.assertGreater(flagged, 2000)
        self.assertLess(escaped / flagged, 0.03)

    # --- Mann-Whitney implementation ------------------------------------------------------

    def test_mann_whitney_exact_known_values(self) -> None:
        self.assertAlmostEqual(gate.mann_whitney_greater([1, 2, 3, 4, 5], [6, 7, 8, 9, 10]), 1 / 252)
        self.assertAlmostEqual(gate.mann_whitney_greater([6, 7, 8, 9, 10], [1, 2, 3, 4, 5]), 1.0)
        self.assertAlmostEqual(gate.mann_whitney_greater([1, 2], [3]), 1 / 3)
        # Prism's case: one slow base run, every head run above the four fast ones.
        self.assertAlmostEqual(gate.mann_whitney_greater([1, 1, 1, 1, 1.5], [1.17] * 5), 6 / 252)

    def test_mann_whitney_ties(self) -> None:
        self.assertEqual(gate.mann_whitney_greater([1.0] * 5, [1.0] * 5), 1.0)
        self.assertEqual(gate.mann_whitney_greater([1.0] * 30, [1.0] * 30), 1.0)  # zero variance
        # Ties between sides count 1/2; result must sit between the strict cases.
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
            self.assertEqual(self.run_gate("--mode", "confirm", "--alpha", alpha, "--base", str(base), "--head", str(base)), 2)

    def test_confirm_mode_passes_overlapping_runs(self) -> None:
        # Pooled delta > 10% but the runs interleave: Mann-Whitney cannot tell head is slower.
        base = self.write("b.json", {"s": [1.0, 1.0, 1.0, 1.25, 1.3, 1.3, 1.35, 1.0]})
        head = self.write("h.json", {"s": [0.95, 0.98, 1.3, 1.3, 1.32, 1.35, 1.36, 0.97]})
        rows = gate.evaluate(gate.load([base]), gate.load([head]), THRESHOLD, [], mode="confirm")
        self.assertGreater(rows[0].delta, THRESHOLD)
        self.assertGreaterEqual(rows[0].p_value, gate.DEFAULT_ALPHA)
        self.assertEqual(rows[0].status, "ok")

    # --- (c) seeded simulation, noise shaped like section 5.1 -----------------------------

    def test_noise_model_matches_section_5_1_spreads(self) -> None:
        rng = random.Random(7)
        spreads = sorted(
            (max(x) - min(x)) / statistics.median(x) * 100 for x in (invocation(rng, 1.0, RUNS) for _ in range(4000))
        )
        median_spread = spreads[len(spreads) // 2]
        self.assertTrue(15.0 <= median_spread <= 30.0, median_spread)

    def test_simulation_at_workflow_defaults(self) -> None:
        # Target: +15% fails >= 90%, 0% passes >= 95%. Measured (seed 1/2, 4000 trials):
        # 0% passes ~99.9%; +15% fails only ~71%: the first pass (one 5-run invocation per side,
        # with a per-invocation shift) misses a true +15% ~29% of the time. The bar is NOT met
        # at these defaults; see test_simulation_at_proposed_defaults. Assert the measured
        # numbers so a change to the rule or the defaults shows up here.
        fail_15 = simulate(15.0, RUNS, CONFIRM_ROUNDS, THRESHOLD, trials=4000, seed=1)
        pass_0 = 1.0 - simulate(0.0, RUNS, CONFIRM_ROUNDS, THRESHOLD, trials=4000, seed=2)
        print(f"\n[sim defaults runs={RUNS} rounds={CONFIRM_ROUNDS} thr={THRESHOLD:g}] "
              f"fail@+15%={fail_15:.3f} pass@0%={pass_0:.3f}", file=sys.stderr)
        self.assertGreaterEqual(pass_0, 0.95)
        self.assertGreater(fail_15, 0.65)
        self.assertLess(fail_15, 0.90)  # documents the shortfall; flip when defaults change

    def test_simulation_at_proposed_defaults(self) -> None:
        # Proposed: runs=9, threshold=6, confirm_rounds=3 (needs Prism's sign-off).
        fail_15 = simulate(15.0, 9, CONFIRM_ROUNDS, 6.0, trials=4000, seed=1)
        pass_0 = 1.0 - simulate(0.0, 9, CONFIRM_ROUNDS, 6.0, trials=4000, seed=2)
        print(f"\n[sim proposed runs=9 rounds={CONFIRM_ROUNDS} thr=6] "
              f"fail@+15%={fail_15:.3f} pass@0%={pass_0:.3f}", file=sys.stderr)
        self.assertGreaterEqual(fail_15, 0.90)
        self.assertGreaterEqual(pass_0, 0.95)


if __name__ == "__main__":
    unittest.main()
