# Required CI check

`ci.yml` reports one status check named `required` on every pull request and every push to `master`. That is the check to require before merging. It passes only when every applicable suite job succeeded, or when the path detector skipped those jobs because the diff does not touch them. It fails when any covered job failed or was cancelled, and it fails when a covered job was skipped for any reason other than that path detector.

CodeQL (`codeql.yml`) and the bench-pr workflow (`bench-pr.yml`) are not part of `required`.

## Why the workflow trigger has no path filter

A `paths` filter on `pull_request` or `push` skips the whole workflow. GitHub then creates no check run. A required check with that name stays pending, and the pull request cannot merge. A job skipped by its own `if` does create a check, and branch protection treats that skipped check as success. That includes a skip caused by an upstream failure. Requiring `Rust / ubuntu-24.04` (or any other suite job by name) would go green on a docs-only change, and it would also go green when the job was skipped because something it depends on failed.

`required` is the job that tells those cases apart. The ruleset in [`master-ruleset.json`](master-ruleset.json) requires that context and no other.

## What runs

The trigger is `pull_request` (every branch), `push` to `master`, and `workflow_dispatch`. There is no `paths` key.

| Job | Check name | When it runs |
| --- | --- | --- |
| `changes` | `changes` | Always. Decides `suites=true` or `suites=false`. |
| `rust` | `Rust / ubuntu-24.04`, `Rust / windows-latest`, `Rust / macos-latest` | `suites=true` |
| `dotnet` | `.NET / ubuntu-latest`, `.NET / windows-latest`, `.NET / macos-latest` | `suites=true` |
| `macos-capacity` | `macOS capacity` | `suites=true`, including after the Rust matrix fails or is cancelled |
| `required` | `required` | As soon as `changes` finishes. It polls the suite jobs until they have a conclusion. |

No existing check name was renamed. `changes` and `required` are new.

The Rust job is still the suite that runs lavapipe goldens on `ubuntu-24.04`, WARP / DX12 renderer tests on `windows-latest`, and the Metal headless suite on `macos-latest`. The `.NET` job is still the generator build and tests. `required` does not replace those jobs. It waits for them and reads `needs.*.result`.

`macos-capacity` used to declare `if: always()` with `needs: rust`. The workflow only started when the old path filter matched, so the note never ran for a docs-only diff. It now also requires `suites=true`, so a docs-only pull request does not start it. When the suites do run, `always()` still lets it explain a Rust cancellation.

On a docs-only diff, `changes` and `required` are the only jobs that take a runner. `required` finishes as soon as the skipped jobs are skipped. It does not stay pending.

While a suite applies, `required` stays in progress until that suite finishes. That wait is the suite, not a missing check. A diff that cannot be read sets `suites=true`, so the suites run instead of being skipped.

`required` does not list the suite jobs in `needs`. If it did, a concurrency cancellation would complete the workflow run before `required` was created, and the check would be missing. The job starts after `changes` and polls the run. Its step uses `if: always()`, so a cancellation that arrives while the poll is running does not skip the conclusion. The poll exits 1 when a covered job is `cancelled`. The job timeout is 360 minutes; a suite that is still unfinished then fails the gate closed.

## How a result is judged

`changes` must be `success` and its `suites` output must be the string `true` or `false`. Anything else fails closed.

| `suites` | Covered job result | `required` |
| --- | --- | --- |
| `false` | `success` or `skipped` | success, if every covered job is one of those |
| `false` | `failure` or `cancelled` | failure |
| `true` | `success` for every covered job | success |
| `true` | `failure`, `cancelled`, or `skipped` | failure |

`skipped` is a path skip only when `changes` succeeded and published `suites=false`. When `suites=true`, `skipped` means an upstream job failed or was cancelled and GitHub did not run the dependent job. That fails the gate.

`workflow_dispatch`, a push whose `before` SHA is missing or all zeros, an unrecognized event, and a `git diff` that fails all publish `suites=true`.

Cancelling the workflow run before `required` starts can leave that check `cancelled` instead of `failure`. A cancelled check is not success, so the ruleset still blocks the merge. Cancelling a dependency job while the run stays open makes `required` execute and exit 1.

The decision logic is `.github/ci/required_check.py` and `.github/ci/ci_paths.py`. The `changes` job runs their unit tests on every pull request.

## Path patterns

Applicability lives in `SUITE_PATTERNS` in `.github/ci/ci_paths.py`. The list is the historical `ci.yml` trigger, plus `.github/ci/**`.

The historical patterns are `.github/workflows/ci.yml`, `.github/actions/lavapipe/**`, `.taplo.toml`, `Cargo.lock`, `Cargo.toml`, `Generators.sln`, `clippy.toml`, `crates/**`, `generators/**`, `third_party/**`, and `RenderideMod/Shaders/**`.

`.github/ci/**` is the addition. That directory is the curl shim the Windows Rust job runs, the macOS capacity note, and this gate. A change there runs the suites the gate judges. `docs/**`, CodeQL, and bench-pr still do not.

Patterns are either an exact root path or `dir/**` (that directory and its files). A rename counts both the old and the new path, so moving a file out of `crates/` still selects the suites. Do not put a `paths` filter back on the workflow trigger.

## Adding a job

1. Add the job to `.github/workflows/ci.yml`. Keep `required`'s `if: always()`.
2. Path-gated job: `needs: changes` (plus any real upstream) and `if: needs.changes.outputs.suites == 'true'`. If it must still run when an upstream suite job fails or is cancelled, follow `macos-capacity`: `always()` and the same `suites` comparison.
3. Add the job id to `COVERED_JOBS` (next step) so the poll waits for it. Do not add it to `required`'s `needs`.
4. Path-gated job: append the id to `COVERED_JOBS` in `.github/ci/required_check.py`. `skipped` is then accepted only for a real path skip.
5. Job that must run on every pull request, including docs-only diffs: append the id to `ALWAYS_JOBS` instead. `skipped` fails the gate. `changes` is already required to be `success`; it is not in that tuple because the gate also reads its output.
6. Add unit tests for success, failure, cancelled, and skipped. The `changes` job runs `python3 -m unittest discover -s .github/ci -p 'test_*.py'`.
7. Leave the ruleset's required context as `required`. Do not add the new job name there. Teach `covered_job_id()` how to recognize the job's check name if it is not one of the existing prefixes.

The poll reads `CHANGES_RESULT` and `SUITES` from the step environment and job conclusions from the Actions API. Do not interpolate those values inside the `run` script.

## Applying the ruleset

This repository does not apply [`master-ruleset.json`](master-ruleset.json) from CI, and opening the pull request does not apply it. Apply it once, after a commit on the default branch has actually reported `required`. Applying it earlier blocks merges on a check the default branch does not report yet.

`bypass_actors` is empty. The ruleset has no bypass, including for repository admins. Nothing in this file grants one.

The apply command creates a ruleset. It does not update an existing one. List them first so it is not created twice:

```bash
gh api repos/DoubleStyxx/Renderide/rulesets --jq '.[] | {id, name, enforcement}'
```

```bash
gh api --method POST repos/DoubleStyxx/Renderide/rulesets --input docs/ci/master-ruleset.json
```

What the file sets:

- Enforcement is `active`, on `~DEFAULT_BRANCH`.
- `deletion` and `non_fast_forward` block deleting the branch and force-pushing to it.
- `pull_request` requires a pull request. `required_approving_review_count` is 0 because a separate reviewer process already exists; this ruleset does not add a second approval count. Direct pushes are still rejected. `dismiss_stale_reviews_on_push` is true. `require_code_owner_review`, `require_last_push_approval`, and `required_review_thread_resolution` are false because the API requires those fields and this ruleset does not turn those policies on. `allowed_merge_methods` is omitted so the repository's existing merge methods stay as they are.
- `required_status_checks` has `strict_required_status_checks_policy` true (the branch must be up to date) and one context, `required`.
- `integration_id` is `15368`, the GitHub Actions GitHub App on github.com. Check runs on this repository report `app.slug` `github-actions` and `app.id` `15368`. The id is set so a status with the same context from another app does not satisfy the rule. GitHub Enterprise Server uses a different app id; do not copy this file there unchanged.
- `do_not_enforce_on_create` is false.
