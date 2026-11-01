# Grok audit, master 2026-10-31

Verdict: **NOT APPROVED**

Master SHA: `a557b624c489d3b87afe832a1023841ddeeb1159`
(`bench: GPU-driven judge scenes`, #295). The tree before that commit was
`6046ffd5975480edb268947527ae7258b5fb583c`. #295 does not touch the files
cited below.

MUST count: **2**

This is a read-only review. No product code was changed. Open pull requests
were reviewed at the head SHAs below and were not pushed, merged, closed, or
force-pushed.

Approval means no MUST finding. A MUST is a defect that can corrupt GPU work,
deadlock or invoke undefined behavior on a crash, or break a safety claim the
unsafe block depends on. SHOULD items are real and should be fixed. They do
not, by themselves, flip the verdict. NIT items are cleanup.

PR verdicts below are about the **diff against that PR's base**. They do not
clear this master verdict. Every open head still contains M1 and M2 unless a
later change edits those sites. None of these PRs do.

## Scope

Workspace crates on master:

- `crates/renderide`
- `crates/renderide-shared`
- `crates/renderide-test`
- `crates/renderide-harness-env`
- `crates/renderide-bench`
- `crates/renderide-control-client`
- `crates/renderide-framebench`
- `crates/interprocess`
- `crates/bootstrapper`
- `crates/logger`
- `crates/renderide-wayland-tablet`

Also reviewed: `.github/workflows`, `.github/actions`, `scripts/`, `.github/ci`,
`generators/` (C# contract generator), and `RenderideMod/` (host mod).

Checked: correctness of the cited paths, `unsafe` justification, GPU submit
ownership, non-test panics, IPC bounds, secrets, dependency advisory gates,
CI token scope and action pinning, `pull_request_target`, test quarantines,
and non-Rust scripts.

About 307 `unsafe {` blocks and 15 `unsafe fn` items sit under `crates/`.
The workspace denies `clippy::undocumented_unsafe_blocks`, so a missing
`// SAFETY:` comment fails CI. The crash-handler comments below are present
and wrong. No `std::mem::transmute` in production crate code. No
`pull_request_target`. No committed API keys or private keys. Release and
nightly workflows pass `RENDERIDE_RELEASE_PRIVATE_KEY_HEX` through the
environment and log presence, not the value. Job tokens stay `contents: read`
except where the job publishes, attests, uploads Pages, writes a bench
comment, or uploads CodeQL (`security-events: write`). Those elevations match
the job.

`renderide-wayland-tablet` forbids `unsafe`. Ring and MemoryPack decode paths
reject oversized lengths, and shared-memory slices go through `byte_subrange`
before `from_raw_parts`. Production renderer and IPC paths do not use
`unwrap` / `expect` / `panic!` on the host decode hot path; Clippy denies
those outside tests. `todo!` and `unimplemented!` are absent under `crates/`.
Quarantine #216 (`crates/renderide-test/src/scene_dsl/quarantine_216.rs:26`)
expires `2026-11-14T23:59:59Z` and its case lists are empty.

## MUST

### M1. Main-thread `Queue::submit` breaks driver-thread queue ownership

`crates/renderide/src/gpu/driver_thread/worker.rs:3`
`crates/renderide/src/gpu/sync/queue_access_gate.rs:5`
`crates/renderide/src/gpu/context/submission.rs:290`
`crates/renderide/src/runtime/offscreen_tasks/camera.rs:378`
`crates/renderide/src/runtime/offscreen_tasks/camera/alpha_coverage.rs:104`
`crates/renderide/src/runtime/offscreen_tasks/camera/camera360.rs:346`
`crates/renderide/src/gpu/driver_thread.rs:261`

The driver loop is documented as the only `wgpu::Queue::submit` for render-graph
work, and it holds `GpuQueueAccessGate` around that submit. The gate exists
because `Queue::write_texture` and `Queue::submit` take wgpu-core locks in
opposite orders, and because OpenXR requires external synchronization of the
`VkQueue` shared with wgpu.

`submit_frame_batch` only pushes a batch onto the driver ring and returns.
`render_camera_task` then calls `apply_camera_task_alpha_coverage`, and
`render_camera360_task` calls `project_camera360_to_equirect`. Both finish a
command encoder and call `gpu.queue().submit` on the caller thread. They do
not call `flush_driver`, and they do not take the queue gate.

Two failures follow. The alpha or equirect pass can be queued on the GPU
before the driver has submitted the capture it is supposed to read, so the
repair runs against a cleared or stale target. The same call can overlap the
driver thread inside `Queue::submit` or a gated `write_texture` (video and
texture upload). That is the ABBA hang the gate was added to stop, and it is
unsynchronized `vkQueueSubmit` on the OpenXR queue.

`DriverThread::flush` makes the second hole worse. `flush_within` returns
false on timeout and its comment says the caller must not overlap that
submit. `flush` discards the bool. These sites then `queue().submit` anyway:

- `crates/renderide/src/runtime/offscreen_tasks/camera.rs:576` then `:608`
- `crates/renderide/src/runtime/offscreen_tasks/reflection_probe/readback.rs:136` then `:166`
- `crates/renderide/src/app/headless/readback.rs:164` then `:205`
- `crates/renderide/src/app/headless/scene_color_dump.rs:83` then `:111`
- `crates/renderide/src/gpu/frame_bench_queries.rs:177` then `:225`

Fix: submit alpha repair, Camera360 projection, and readback copies through
`submit_frame_batch` / `DriverSubmitter` so they stay ordered behind the
capture on the driver thread. If any `Queue::submit` remains on the caller
thread, take `GpuQueueAccessGate` for the whole submit. Change `flush` to
return the timeout result and skip the copy when the driver did not signal.

### M2. Fatal-crash `unsafe` blocks claim signal-safety they do not have

`crates/renderide/src/process_io/fatal_crash_log/unix.rs:74`
`crates/renderide/src/process_io/fatal_crash_log/unix.rs:94`
`crates/renderide/src/process_io/fatal_crash_log/stack_trace.rs:83`
`crates/renderide/src/process_io/fatal_crash_log/windows.rs:48`
`crates/renderide/src/process_io/fatal_crash_log/windows.rs:62`

The Unix `CrashHandler::attach` SAFETY comment says the closure only calls
async-signal-safe operations (`libc::write`, `__errno_location`) and a
`OnceLock`. The closure calls `write_stack_trace`. Phase 1 (hex instruction
pointers) is allocation-free. Phase 2 calls `symbolicate_frames`, which
allocates and runs `backtrace::resolve`, inside `catch_unwind`.

`catch_unwind` does not make `malloc` async-signal-safe. Phase 2 is skipped
only for `SIGABRT` on Linux and Android (`stack_trace.rs:73`). `SIGSEGV` and
the other fatal signals still enter the allocator. If the fault happened
while the malloc lock was held, the handler deadlocks and the crash report
never finishes. The module documents that deadlock for `SIGABRT` and then
does the same work for every other signal.

The Windows handler's SAFETY comment says the closure only does synchronous
file writes under mutexes. It also calls `write_stack_trace` (heap, DWARF or
dbghelp) and takes `std::sync::Mutex` in the vectored exception handler. A
crash on the thread that already holds the log mutex deadlocks the handler.

Fix: from the signal or vectored-exception handler, write only the Phase 1
hex IPs with `write` / `WriteFile`. Symbolicate on another thread, or leave
the addresses for `addr2line`. Update both SAFETY comments so they describe
the operations the closure actually performs. Do not treat `catch_unwind` as
a signal-safety fence.

## SHOULD

### S1. Light-row copy ceiling ignores the descriptor

`crates/renderide/src/scene/lights/apply.rs:117`

`packable_row_copy_max_bytes` does `.min(MAX_LIGHT_ROW_COPY_BYTES).max(MAX_ACCESS_COPY_BYTES)`.
Both constants are 64 MiB, so the function always returns 64 MiB. A small
`buffer_capacity` does not lower the ceiling. The mapped slice is still
bounds-checked, so this is not a proven out-of-bounds read. The cap does not
do what the function comment says.

Fix: use `.min` for the accessor ceiling. Add a test where a small
`buffer_capacity` lowers `max_bytes`.

### S2. Readback timeout does not bound `Device::poll`

`crates/renderide/src/runtime/offscreen_tasks/readback.rs:181`

`await_buffer_map` calls `device.poll(PollType::wait_indefinitely())` and only
then `recv_timeout(timeout)`. `CAMERA_READBACK_TIMEOUT` and
`PROBE_READBACK_TIMEOUT` do not apply while the device waits for every queued
submission. A stalled queue blocks the host completion path with no deadline.

Fix: poll with `PollType::Wait { timeout: Some(...) }` in a loop, or submit
the copy on the driver thread and wait on that submission index.

### S3. Bootstrapper turns a panic into success

`crates/bootstrapper/src/lib.rs:92`

`run` catches a panic from `orchestration::run`, logs it, and returns `Ok(())`.
The comment says this mirrors ResoBoot. The process exit code then says the
launch succeeded.

Fix: return `Err` or exit non-zero after the log line. Keep the log.

### S4. Debug UDP attach accepts the first local datagram

`crates/renderide-shared/src/ipc/connection.rs:166`

`-AttachRenderer` binds `127.0.0.1:42512` and parses the first datagram as
the queue name and capacity. Any local process can attach the renderer to
its queue. The packet itself is length-checked and capacity is capped at
`QueueOptions::MAX_CAPACITY`.

Fix: require a shared secret in the datagram, or `SO_PASSCRED` / an equivalent
same-user check, and document the debug-only threat model.

### S5. Video sources are not path-checked

`crates/renderide/src/assets/video/source.rs:15`

A source containing `://` is passed to GStreamer unchanged (`http`, `rtsp`,
`file`, and anything else). A relative path is joined to the process current
directory with no `..` rejection. Shader probe paths already reject `..`.

Fix: reuse the shader path rule for local files, and allow only an explicit
scheme list for network URIs.

### S6. Vive tracker enumeration trusts the runtime count

`crates/renderide/src/xr/input/tracker.rs:321`

The second `xrEnumerateViveTrackerPathsHTCX` call writes into a buffer of
`count` elements and the code then `truncate`s to `written`. There is no
upper bound on `count` before `Vec::with_capacity`, and no check that
`written <= count`. A hostile or buggy runtime can force a large allocation
or write past the buffer during the call. `truncate` itself does not panic
when `written` is larger than the length.

Fix: cap `count`, and treat `written > count` as an error before using the
buffer.

### S7. Windows views use the host-claimed size

`crates/renderide-shared/src/ipc/shared_memory/windows.rs:48`
`crates/renderide-shared/src/ipc/shared_memory/bounds.rs:12`

`MapViewOfFile` is asked for `required_view_capacity`, which is
`max(buffer_capacity, offset + length)` from the host descriptor. Unix maps
the real file and ignores that capacity. A lie on Windows fails the map when
the section is smaller, which is safe, but the renderer still trusts the
descriptor for the view length it records.

Fix: map with a size derived from the section, or cap the requested view
(for example at `MAX_ACCESS_COPY_BYTES`) before calling `MapViewOfFile`.

### S8. No dependency advisory gate

No `deny.toml`, `audit.toml`, or CI step runs `cargo-deny` or `cargo-audit`.
`.github/dependabot.yml` only opens version updates. `cargo-audit` was not
run in this pass, so this finding is the missing gate, not a named advisory.

Fix: add `cargo deny` (advisories, licenses, sources) or `cargo audit` on
the PR and nightly workflows, and fail the job on a known vulnerability.

### S9. CI actions are pinned on release paths and floating elsewhere

`release.yml`, `nightly.yml`, and `pr-body.yml` pin third-party actions to
commit SHAs. These still float on a tag:

- `.github/workflows/ci.yml:55` `actions/checkout@v7` (also lines 103, 356, 401, 422)
- `.github/workflows/ci.yml:114` `actions/cache@v4`
- `.github/workflows/ci.yml:213` `mozilla-actions/sccache-action@v0.0.11`
- `.github/workflows/ci.yml:222` `Swatinem/rust-cache@v2`
- `.github/workflows/ci.yml:248` `uncenter/setup-taplo@v2`
- `.github/workflows/ci.yml:323` `actions/upload-artifact@v7`
- `.github/workflows/ci.yml:358` `actions/setup-dotnet@v6`
- `.github/workflows/bench-pr.yml:66` `actions/checkout@v7` (also 105, 112, 531, 538, 708)
- `.github/workflows/bench-pr.yml:480` `actions/upload-artifact@v7` (also 686, 749)
- `.github/workflows/bench-pr.yml:717` `actions/download-artifact@v7` (also 724)
- `.github/workflows/bench-pr.yml:758` `actions/github-script@v7` (job has `pull-requests: write`)
- `.github/workflows/codeql.yml:62` `actions/checkout@v7`, plus `github/codeql-action/*@v4` and `actions/upload-artifact@v7` / `download-artifact@v8`
- `.github/workflows/static.yml:38` `actions/checkout@v7`, `configure-pages@v6`, `upload-pages-artifact@v5`, `deploy-pages@v5` (Pages write token)
- `.github/workflows/frame-bench.yml:104` `actions/checkout@v7` (also 128, 135, and `upload-artifact@v7` at 495)
- `.github/workflows/frame-bench-gate-tests.yml:42` `actions/checkout@v7` and `:47` `actions/setup-python@v7`

No workflow interpolates a pull request title or body into a `run:` script.
`workflow_dispatch` inputs that reach the shell go through environment
variables. `github-script` on `bench-pr.yml` reads a file the job wrote. The
risk is a moved tag, highest on `github-script` and `deploy-pages`.

Fix: pin every third-party action to a full SHA, matching `release.yml`.
Start with `bench-pr.yml` `github-script` and `static.yml` `deploy-pages`.

### S10. CodeQL does not build the Rust workspace

`.github/workflows/codeql.yml:51` sets `build-mode: none` for Rust. The
uploaded SARIF does not come from a compiled analysis of `crates/renderide`.

Fix: use `build-mode: manual` (or `autobuild` if it succeeds) and compile
the workspace before `analyze`.

### S11. Rust-everywhere: policy scripts are still Python and shell

C# under `generators/` and `RenderideMod/` is the host and contract surface.
It should stay. These scripts encode CI policy and should move to a Rust
`xtask` or a crate test when that policy is touched:

- `.github/ci/ci_paths.py`
- `.github/ci/required_check.py`
- `scripts/ci/check_pr_body.py`
- `scripts/ci/check_rust_toolchain_pin.py`
- `scripts/ci/frame_bench_gate.py`
- `scripts/ci/bench_pr_assemble.py`
- `scripts/ci/bench_pr_report.py`
- `scripts/ci/bench_pr_merge_confirm.py`
- `scripts/ci/bench_pr_echo_verdict.py`
- `scripts/ci/bench_pr_force_unstable.py`

`scripts/ci/pre-push-check.sh` and `scripts/bench.sh` are local wrappers
(NIT unless the policy move happens). `.github/ci/macos_capacity_note.py`
and `mach_dxcompiler_curl.py` are small helpers. `index.html` is the Pages
site, not a policy script.

Fix: move the gate scripts first (`check_pr_body`, `frame_bench_gate`,
`ci_paths`). Do not rewrite the C# generator or the mod as part of that.

### S12. Error-path present can run off the driver thread

`crates/renderide/src/render_graph/swapchain_scope.rs:167`

`Drop` presents a surface texture that `take_surface_texture` did not take.
It locks `queue_gate` when the scope has one. The present still runs on
whatever thread drops the scope, which is not the driver thread.

Fix: enqueue the present on the driver, or `debug_assert` that drop never
observes a live texture outside tests.

### S13. Double-buffer swap is not tied to GPU retire

`crates/renderide/src/gpu_pools/pools/render_texture.rs:464`
`crates/renderide/src/gpu/driver_thread.rs:68`

`begin_double_buffered_frame` swaps published and history at plan time.
`DRIVER_VISIBLE_FRAMES_IN_FLIGHT` is 2. Same-queue FIFO ordering hides the
hazard only while every writer and sampler of those images is submitted on
the driver thread. M1 violates that.

Fix: after M1, keep the swap, or delay it until the previous writer's submit
token is done if a third frame can still sample the image.

## NIT

### N1. Security link uses a different GitHub login

`SECURITY.md:3` points at `https://github.com/DoubleStyx/Renderide/security`
(one `x`). The repository is `DoubleStyxx/Renderide`.

Fix: point the security tab URL at this repository.

### N2. Ignored HUD test has a ticket and no date

`crates/renderide/src/runtime/debug_hud_frame.rs` ignores a test with reason
`H7` and no expiry. #216's quarantine does have an expiry. This one does not.

Fix: add a review date or an issue URL and an expiry to the `ignore` reason.

### N3. Error-type equality tests compare a value to itself

`crates/renderide-shared/src/packing/memory_pack_error.rs` (and the sibling
error modules) use `assert_eq!(a, b)` after `let b = a`. That checks `Copy`
and `Eq` plumbing, not decode behavior.

Fix: leave them, or compare two separately constructed values.

### N4. Material coverage allowlist has no review date

`crates/renderide/material_coverage_allowlist.toml` is shrink-only and each
entry has a reason. It is not a failing-test quarantine. A review date would
still make stale rows obvious.

## Open pull requests

Verdict here means the diff against that PR's base introduces no MUST.
Master stays **NOT APPROVED** at every head that still contains M1 and M2.

| PR | Head SHA | Base | Verdict | MUST |
| --- | --- | --- | --- | --- |
| 298 | `f4e49c0212b45cd7a45ed7d4e198f58a6afba937` | master | APPROVED | 0 |
| 297 | `1fe0b2ffb6d824030d1f53f35ff8a7da1a24e15b` | master | APPROVED | 0 |
| 296 | `cfcbd43f47e6a4b708a2c1788c2b6c8c10db5a55` | master | APPROVED | 0 |
| 294 | `205d5ee604510b0f943787c0406662e2f23f558c` | master | APPROVED | 0 |
| 284 | `abcd43d73baa0e6e3b044e1c0d58324753196f78` | master | APPROVED | 0 |
| 282 | `657464dc7fba84537fa04b8bdcf75cf4c9780fb9` | `cursor/ipc-roundtrip-judge-74df` | APPROVED | 0 |
| 275 | `27edb8b38d73dce06ada040e512c233539be1008` | master | APPROVED | 0 |
| 274 | `0eb772e90e3f28979a582f7382adf9da1d659a5c` | master | APPROVED | 0 |
| 272 | `19bcef91518367e9dcbfbe3aae88c14db2d5d5a4` | master | APPROVED | 0 |
| 270 | `b89bd5d289cc8a5fefc3e0e24cb52d4e1c804b43` | master | APPROVED | 0 |
| 226 | `d44ca169626bb25587e22a916e82ba9051aa62b0` | master | APPROVED | 0 |
| 38 | `878e2cd5b018ada47f14406498598419827f60fe` | `mod/custom-shader-material` | APPROVED | 0 |
| 32 | `64291dd49c48cb8f29497b89bfdc60d9cd4d8d2d` | master | APPROVED | 0 |

### PR notes (not MUST)

- **298.** Channel-zero and flat-timeout excuses now require a clean frame
  (no foreign failure, and a recorded count only at 0). `None` still counts
  as "not recorded" and is allowed. `clean_listed_channel_zero_frame_is_excused`
  asserts `excuse(true) == WARP_EXPECTED_FAILURE_CASES.contains(LISTED)`, so
  removing the case from the list does not fail that one test. The negative
  tests in the same file do fail closed. SHOULD: assert `excuse(true)` is
  true for the listed name.
- **297.** Manual smoke checklist only. No code. The roundtrip sentence
  matches current master (DLL absent, tests compiled out). PR 294 changes
  that fact; this doc would need a line update if 294 lands first.
- **296.** Report-only `cargo-mutants` workflow. Checkout and the upload
  action are SHA-pinned. `Swatinem/rust-cache@v2` and `actions/cache@v4` are
  not. `continue-on-error: true` on the mutants step means a crashed run is
  still green. That matches the "not required" comment. SHOULD: pin the two
  actions, and do not mark this check required while it cannot fail.
  `scripts/ci/mutants_summary.py` is another Python policy script (S11).
- **294.** Builds pinned Renderite `7ccfd2ee7c28648cb2e46dd4f52f51613a848071`,
  checks the SHA and the MIT license file, restores with a lock file, and
  fails the job if `CrossLanguageRoundtripTests` has zero passes or any skip.
  SHOULD: `build.sh` does `rm -rf` on `$1` before the path is constrained.
  CI calls it with no argument (`RUNNER_TEMP`). Reject paths outside that
  temp dir. New steps use `Swatinem/rust-cache@v2` (S9). The build script is
  bash (S11). This audit did not execute the DLL roundtrip.
- **284.** `_ALPHAPREMULTIPLY_ON` becomes a compile-time keyword for
  `pbsspecular`, matching metallic, bit 1. Tests check that the opaque and
  premultiply targets each contain one light loop, and that unlit stems are
  not rewritten. No MUST.
- **282.** Wire types for `RenderTextureReadbackTask`,
  `RenderTextureReadbackResult`, `FrameSubmitData.readback_tasks`, and
  `FrameControllerState` match the direction of the pinned contract. Dispatch
  does not implement readback: `command_dispatch.rs` falls through to
  `Unhandled`. `FrameControllerState` is grouped with controllers that do not
  synthesize curls (`hand_synth.rs`). SHOULD: either implement readback and
  Frame controller curls, or log a single explicit "decoded, not implemented"
  and test that a host waiting on `RenderTextureReadbackResult` cannot lock
  the session. This audit did not run the cross-language roundtrip.
- **275, 274, 272.** One comment line in `crates/renderide/src/lib.rs`
  (`parity-noop-N canary (DO NOT MERGE)`). No behavior change. Diff verdict
  is APPROVED. Do not merge the canaries. This review did not approve them
  on GitHub.
- **270.** Frame-bench gate parity. Checkout under `path: tools` makes
  `tools/scripts/ci/...` a real runner path; that is not a missing-file bug.
  Post-drift policy fails only after two consecutive drift-clean FAIL rounds,
  and disagreement triggers one full rerun then an unstable result. That is
  looser than fail-closed on the second drift. SHOULD: keep it that way only
  with the unstable result distinct from pass, which the workflow attempts.
  Actions stay on floating tags (S9). Still Python (S11).
- **226.** First-party change is a `[patch.crates-io]` of `profiling` and
  `wgpu-profiler` plus a Tracy 0.14.1 comment. Vendored trees carry their
  upstream licenses. No new `unsafe` in `crates/renderide/src/profiling.rs`.
  SHOULD: when the crates.io releases depend on `tracy-client` 0.19, drop
  the patches. Until then, clippy the vendored crates with the workspace
  `undocumented_unsafe_blocks` deny or document why they are exempt.
- **32.** Per-material custom shader key. Host variant bits are dropped when
  the stem came from the material override, so Unlit `_ALPHATEST` cannot
  force custom keyword 0. `TryPropertyName` only requires a non-empty trimmed
  string. SHOULD: reject property names that are not WGSL identifiers before
  they hit the uniform packer. `AssignMaterial` already rejects an unknown
  embedded shader name.
- **38.** Dynamic-variable bindings on top of 32. Selection is separated
  from the engine walk, and unhook paths guard destroyed slots. SHOULD:
  `SameBindings` treats two rows with the same property as equal once the
  first row matches, so a duplicate property can hide a value change.
  `ColorProfile` is stored as a raw byte and cast back. Confirm the byte
  values match FrooxEngine before relying on a non-default profile.

## What this pass did not do

- No `cargo audit` or `cargo deny` database refresh (the tools are not
  configured, and this pass did not install them).
- No GPU run of Camera360, alpha repair, or a crash-handler signal.
- No execution of the Renderite.Shared roundtrip on PR 294 or 282.
