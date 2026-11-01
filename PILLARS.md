# Renderide pillars

Six pillars are the center of gravity for this fork: performance, fidelity, parity, experimentation, customizability, and reliability. A pull request says which of them it advances. The headings live in [`.github/pull_request_template.md`](.github/pull_request_template.md). [`.github/workflows/pr-body.yml`](.github/workflows/pr-body.yml) runs the `pr-body` job on `pull_request` (`opened`, `edited`, `synchronize`, `reopened`) and fails when `## Verification evidence`, `## Pillars advanced`, or `## Ratchet metrics moved` is missing or still template placeholder text. The check is [`scripts/ci/check_pr_body.py`](scripts/ci/check_pr_body.py). It reads the pull request body from the Actions event payload.

That job is not yet part of the `required` aggregator. The aggregator is open pull request [#277](https://github.com/DoubleStyxx/Renderide/pull/277) and is not on `master`. Add the `pr-body` job name there. This document does not edit that branch.

Three labels are used below.

- **Enforced now** means a check on current `master` fails the change when the rule is broken.
- **In progress** means an open pull request adds the check. The files it adds are not on `master`, so this document does not link them as if they were.
- **Planned** means the check is not in the tree.

`master` for this description is `87072abc` (`ci: pin exact rustc 1.99.1 (P0-4)`).

## Verification evidence

Every pull request names the golden or test that fails without the change, and links the CI run that shows it. A golden is not blessed without a before/after diff in the pull request. Planned: a separate PR (the ratchet PR) will replace CONTRIBUTING §2.5's same-PR bless with a judge-then-code two-PR rule, so goldens, tolerances and baselines never land in the same PR as the code they judge.

**Enforced now.** [`.github/pull_request_template.md`](.github/pull_request_template.md) carries the `## Verification evidence` section. [`scripts/ci/check_pr_body.py`](scripts/ci/check_pr_body.py) fails the `pr-body` job when that heading is absent, empty, or still contains the template markers `pr-body-lint:placeholder` or `TODO(pr-body):`. The same rule applies to `## Pillars advanced` and `## Ratchet metrics moved`. The ratchet section records before and after values for each ratchet metric the pull request moves. `None` plus a reason passes. A bare `None` fails. [`CONTRIBUTING.md`](CONTRIBUTING.md) (section 2.5) already requires a headless case under `crates/renderide-test` for a drawing, shading, lighting, culling, skinning, or Resonite-parity change, and requires that the capture fail on the parent commit and pass with the change. The suite writes `diff.png` and `side_by_side.png` next to the capture ([`crates/renderide-test/src/scene_dsl/side_by_side.rs`](crates/renderide-test/src/scene_dsl/side_by_side.rs)). `ci.yml` uploads `target/renderide-test-out/**` from the headless suite. `run --bless` refuses to overwrite an existing golden unless `--force` is passed ([`crates/renderide-test/tests/cli_contract.rs`](crates/renderide-test/tests/cli_contract.rs)). Cases on the #216 quarantine list refuse bless even with `--force` ([`crates/renderide-test/src/scene_dsl/quarantine_216.rs`](crates/renderide-test/src/scene_dsl/quarantine_216.rs)).

**Gap.** The lint checks that the section is present and filled in. Which golden was named, and whether a CI URL is present, is a review rule on top of that. A committed golden PNG is not diffed against the parent by a check that fails when the body lacks the before/after image. Reviewers apply that rule from the section the author writes.

## Performance

Frame time on the default path stays inside the noise-aware regression bar. CPU hot-path numbers are reported beside that bar and do not decide it.

**Enforced now, on pull requests that touch the measured trees.** [`.github/workflows/bench-pr.yml`](.github/workflows/bench-pr.yml) is the Gauge frame-gate workflow. It runs when a pull request changes `crates/renderide/src/**`, `crates/renderide-framebench/**`, `crates/renderide-bench/**`, the gate scripts, or the workflow itself. The header of that file states the methodology: at least 12 runs, an interleaved chunked first pass, one reversed-order confirm round, `RAYON_NUM_THREADS=2`. A scenario fails only on the confirm round.

The decision rule is [`scripts/ci/frame_bench_gate.py`](scripts/ci/frame_bench_gate.py) (`BASE_DRIFT_PCT = 15.0`). Confirm-only delta must be above the fail threshold (10 percent in the workflow) and the one-sided Mann-Whitney U p-value must be below alpha (default 0.05), with at least 12 samples on each side. Pooled delta and p-value are reported and do not fail the row. If the confirm-round base median drifts more than 15 percent from the first-pass base median, the row is `INCONCLUSIVE (base drift)` and the workflow runs one extra reversed confirm ([`bench-pr.yml`](.github/workflows/bench-pr.yml) writes `--drift-out` and calls [`scripts/ci/bench_pr_merge_confirm.py`](scripts/ci/bench_pr_merge_confirm.py)). A second drifting round fails closed as `FAIL (inconclusive: base drift)`. The sole verdict job is `bench-pr-lavapipe`. Unit tests live in [`scripts/ci/test_frame_bench_gate.py`](scripts/ci/test_frame_bench_gate.py) and run from [`.github/workflows/frame-bench-gate-tests.yml`](.github/workflows/frame-bench-gate-tests.yml) and from the self-test step in `bench-pr.yml`.

[`crates/renderide-bench/benches/cpu_hot.rs`](crates/renderide-bench/benches/cpu_hot.rs) is the Criterion file behind the `cpu_hot` bench (`[[bench]] name = "cpu_hot"` in [`crates/renderide-bench/Cargo.toml`](crates/renderide-bench/Cargo.toml)). The workflow job `bench-pr-cpuhot` runs `renderide-bench run --suite cpu` with `--bench-arg=--quick`. The job is `continue-on-error: true` and always exits 0. `cpu_hot` never gates. The catalog of those ids is [`crates/renderide-bench/src/catalog.rs`](crates/renderide-bench/src/catalog.rs).

[`.github/workflows/frame-bench.yml`](.github/workflows/frame-bench.yml) is the manual lavapipe frame bench (`workflow_dispatch` only). It calls the same gate script. Its default is 9 runs and 3 confirm rounds; the gate concatenates the confirm rounds, so that default clears the 12-sample floor. `bench-pr.yml` uses one confirm round of 12 runs.

**Gaps.**

- `bench-pr.yml` calls itself non-required. A red `bench-pr-lavapipe` stays outside the `required` aggregator in #277.
- `cpu_hot` is report-only. The workflow records that no-op canary #242 produced false FAIL/FLAG rows under the same 5 percent / 10 percent thresholds, and it says to leave `cpu_hot` non-gating until a noise study exists.
- Open pull request [#289](https://github.com/DoubleStyxx/Renderide/pull/289) proposes a post-drift two-clean-FAIL policy. `master` still uses the one-extra-round, then fail-closed, rule above.

## Fidelity

A captured frame matches its committed golden inside the tolerance that case declares. Tolerances are not widened to hide a mismatch.

**Enforced now.** The headless suite is `renderide-test check-suite`, run from the Rust job in [`.github/workflows/ci.yml`](.github/workflows/ci.yml). Linux uses the pinned lavapipe composite action [`.github/actions/lavapipe/action.yml`](.github/actions/lavapipe/action.yml). The workflow comment states that lavapipe is the behavior gate; Windows (DX12 WARP) and macOS (Metal) captures are adapter comparisons. Cases, resolutions, and tolerances are registered in [`crates/renderide-test/src/scene_dsl/cases.rs`](crates/renderide-test/src/scene_dsl/cases.rs). Comparison criteria live in [`crates/renderide-test/src/scene_dsl/tolerance.rs`](crates/renderide-test/src/scene_dsl/tolerance.rs): SSIM-Y (`ssim_min`), per-channel absolute difference (`max_abs_diff`, `max_failing_pixel_fraction`), luma and unique-color coverage, region floors (`PBS_BAND_FLOOR_FRACTION` is 0.75), and region color gates. Committed PNGs are under [`crates/renderide-test/goldens/`](crates/renderide-test/goldens/).

The pull request template and [`CONTRIBUTING.md`](CONTRIBUTING.md) section 2.5 both say not to widen an SSIM, per-pixel, or coverage tolerance to hide a mismatch. The template's Rendering and parity checklist is the same rule.

**Gap.** No script diffs `ssim_min` or the other tolerance fields and fails the pull request when a number moves. Widening is a review rule. A golden file can be replaced in the same commit as a tolerance edit; the author has to show the before/after diff in `## Verification evidence`, and reviewers have to reject a silent widening.

## Parity

Observable behaviour matches Yellow Dog Man Studios' MIT reference implementations: the Renderite.Shared IPC contract, and Renderite.Unity shader behaviour. Port behaviour, not source. Keep the upstream MIT copyright notice on anything that is derived from those sources. This repository's own [`LICENSE`](LICENSE) is MIT, copyright DoubleStyx, and is not a substitute for that attribution.

**Enforced now, for pixels and for the generator's own units.** Scene goldens in the fidelity section are the pixel check against the behaviour the renderer ships. [`generators/SharedTypeGenerator`](generators/SharedTypeGenerator) reads `Renderite.Shared.dll` and emits [`crates/renderide-shared/src/shared.rs`](crates/renderide-shared/src/shared.rs). [`generators/SharedTypeGenerator.Tests/Roundtrip/CrossLanguageRoundtripTests.cs`](generators/SharedTypeGenerator.Tests/Roundtrip/CrossLanguageRoundtripTests.cs) packs with C# and unpacks with the Rust `roundtrip` binary. [`generators/SharedTypeGenerator.Tests/SharedTypeGenerator.Tests.csproj`](generators/SharedTypeGenerator.Tests/SharedTypeGenerator.Tests.csproj) removes `Roundtrip/**/*.cs` from the compile when `Renderite.Shared.dll` is not discoverable. Hosted `ci.yml` does not have that DLL, so the .NET job runs the generator tests with the roundtrip sources excluded. [`crates/renderide/src/materials/shader_specialization.rs`](crates/renderide/src/materials/shader_specialization.rs) records that shader specialization is renderer-local and must not change the Renderite.Unity / Renderite.Shared wire contract. Shader sources in the tree are the WGSL under [`crates/renderide/shaders/`](crates/renderide/shaders/). The tree has no Renderite.Unity checkout. [`docs/parity-audit.md`](docs/parity-audit.md) is a read-only audit against the public candidate write-up.

**In progress (P0-5).** [Pull request #282](https://github.com/DoubleStyxx/Renderide/pull/282) runs the C#/Rust IPC roundtrip on hosted CI against a pinned Yellow-Dog-Man/Renderite commit, with the MIT license of that upstream called out in the pull request. Until it merges, hosted CI still compiles the roundtrip out.

**Planned, after 7 November 2026.** Numeric per-shader behaviour assertions ported from the MIT sources. No file holds them yet. Port behaviour, not code, and keep the MIT attribution.

## Experimentation

A new or experimental path is behind a Cargo feature, an `[experimental]` setting that defaults off, or an opt-in suite group. The default path stays on the golden suite and, when renderer source changes, on the frame gate.

**Enforced now, for the defaults that already have a gate.** [`crates/renderide/Cargo.toml`](crates/renderide/Cargo.toml) features `tracy`, `video-textures`, `bench-api`, `control-api`, and `hud-counters` are off unless requested. `ci.yml` builds the default renderer with `cargo build --locked -p renderide` and runs `cargo test --workspace --locked` on that same default feature set. Linux Clippy adds `--all-features`, and one Linux step runs the `video-textures` MPEG-TS test. Those two steps cover the optional features. The default build line stays feature-free. [`crates/renderide/src/config/types/experimental.rs`](crates/renderide/src/config/types/experimental.rs) defaults `reflection_probe_sh2_enabled` and `material_shader_hot_reload_enabled` to false. The opt-in `warp_216_debug` group is not part of `registry()` ([`crates/renderide-test/src/scene_dsl/cases.rs`](crates/renderide-test/src/scene_dsl/cases.rs), [`crates/renderide-test/src/scene_dsl/suite.rs`](crates/renderide-test/src/scene_dsl/suite.rs)); `check-suite` therefore does not run it. A pull request that edits `crates/renderide/src/**` still goes through `bench-pr-lavapipe` and, on the CI path filter, through `check-suite`.

**Gap.** No check walks a diff and fails when a new code path is compiled into the default binary without a flag. The default goldens and the frame gate catch a default-path pixel or frame-time regression. An experimental path that stays on the default and has no golden of its own can still land.

## Customizability

Config knobs and material options that the renderer already models have tests next to them. A shipped material root is supposed to have a scene-DSL golden.

**Enforced now, where tests exist.** Renderer settings are [`crates/renderide/src/config/types.rs`](crates/renderide/src/config/types.rs) (`display`, `rendering`, `debug`, `post_processing`, `experimental`, `watchdog`). Round-trip tests include [`crates/renderide/src/config/persist/load/tests.rs`](crates/renderide/src/config/persist/load/tests.rs), [`crates/renderide/src/config/persist/save.rs`](crates/renderide/src/config/persist/save.rs), and the `types` modules beside them (MSAA, graphics API, scene color, post-processing, HUD). Material render-state mapping is tested under [`crates/renderide/src/materials/render_state/`](crates/renderide/src/materials/render_state/) (including [`unity_mapping.rs`](crates/renderide/src/materials/render_state/unity_mapping.rs)). `cargo test --workspace` in `ci.yml` runs these.

**In progress (P0-2).** [Pull request #285](https://github.com/DoubleStyxx/Renderide/pull/285) fails `cargo test` when a shipped material root has no scene-DSL golden and is not on a shrink-only allowlist. The matrix it adds is not on `master`. [Pull request #286](https://github.com/DoubleStyxx/Renderide/pull/286) is the follow-up that makes a change to that coverage doc select the CI suite. On `master`, a material shader root can ship with no golden and `cargo test` still passes.

**Gap.** Config fields are covered where someone wrote a round-trip. There is no check that every key in `RendererSettings` has a test.

## Reliability

The tree builds on a pinned toolchain, formatting and Clippy are clean, `unsafe` is documented, and the suites that apply to a pull request report a conclusion.

**Enforced now.**

- Toolchain. [`rust-toolchain.toml`](rust-toolchain.toml) pins channel `1.99.1` with components `rustfmt` and `clippy`. [`scripts/ci/check_rust_toolchain_pin.py`](scripts/ci/check_rust_toolchain_pin.py) fails when a workflow floats `stable` / `beta` / `nightly` or names a different `toolchain:`. `ci.yml` installs `dtolnay/rust-toolchain` at SHA `683b588c487a69d01f6c6ddf40f036ff682cc2c8` with `toolchain: "1.99.1"` and runs the pin check. [`.github/workflows/frame-bench-gate-tests.yml`](.github/workflows/frame-bench-gate-tests.yml) runs the pin check and `python -m unittest discover -s scripts/ci -p 'test_*.py'`.
- Format and lints. `ci.yml` runs `cargo fmt --all -- --check` and `cargo clippy --all-targets --locked` with `-D warnings -D clippy::all`. Linux passes `--all-features`. The workspace denies `undocumented_unsafe_blocks` ([`Cargo.toml`](Cargo.toml)), so each `unsafe` block needs a `// SAFETY:` comment. [`clippy.toml`](clippy.toml) allows unwrap, expect, panic, dbg, print, and indexing in tests only. The .NET job builds with `-warnaserror` and, on Linux, runs `dotnet format Generators.sln --verify-no-changes`. Local hooks in [`.pre-commit-config.yaml`](.pre-commit-config.yaml) and [`scripts/ci/pre-push-check.sh`](scripts/ci/pre-push-check.sh) mirror the same-machine part of that gate. [`CONTRIBUTING.md`](CONTRIBUTING.md) section 2.7 and [`SI_POLICY.md`](SI_POLICY.md) state the `// SAFETY:` rule.
- Pull request sections. The `pr-body` job described at the top of this file fails closed when any of the three required sections is missing or left as a placeholder, and when `## Ratchet metrics moved` is a bare `None`.
- Suites that the path filter selects. `ci.yml` runs Rust tests, the headless golden suite, and the .NET generator tests on the operating-system matrix. The Rust job name is `Rust / ${{ matrix.os }}`, which is `Rust / ubuntu-24.04`, `Rust / windows-latest`, and `Rust / macos-latest`.

**In progress.** [#277](https://github.com/DoubleStyxx/Renderide/pull/277) adds a `required` job that fails unless every applicable suite job succeeded, and a `changes` job so a docs-only pull request still gets a conclusion instead of a pending check. The ruleset file in that pull request is not applied. On `master`, `ci.yml` is path-filtered, so a pull request that does not touch those paths never starts the Rust or .NET jobs. `bench-pr` and CodeQL ([`.github/workflows/codeql.yml`](.github/workflows/codeql.yml)) are outside that aggregator. [`CONTRIBUTING.md`](CONTRIBUTING.md) section 2.9 still lists `Rust / ubuntu-latest` as the branch-protection context; the workflow reports `Rust / ubuntu-24.04`.

**Planned (P0-3).** A mutation and negative suite. No workflow, crate, or script for it is on `master`.

**Planned.** `docs/ci/ratchet.toml` is the acyclic RSI monotonic quality ratchet (docs/RSI.md, separate PR). Another agent is adding that file in a separate pull request. The file is not on `master`. This document names the path and does not link it. Until it lands, `## Ratchet metrics moved` can say `None` plus a reason.

**Gap.** `pr-body` fails its own workflow when the sections are wrong. Merge stays open until branch protection requires that job name. The place to add it is the `required` aggregator in #277, which is still open. Hooks can be skipped with `--no-verify`.
