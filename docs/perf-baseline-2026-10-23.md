# Perf baseline: fork master @ aafcb720 (2026-10-23)

Measured by Gauge Drone (independent perf/quality reviewer). Docs-only change; no code touched.

- Commit: `aafcb72035fa2bc24b860cb2c6c3d4ece5aa49c3` (`DoubleStyxx/Renderide` master, "Bump wgpu to 30 and compatible workspace dependencies (#47)"; includes #33 release pipeline, #43 clippy::all deny)
- Machine: shared cloud Linux box (Debian 13), Intel Xeon (virtualized) 8 vCPU, 15 GB RAM, **no physical GPU**
- GPU backend: wgpu 30 / Vulkan on **lavapipe** (llvmpipe LLVM 19.1.7, Mesa 25.0.7-2+deb13u1, `device_type: Cpu`), `TIMESTAMP_QUERY` available
- Toolchain: rustc/cargo 1.99.1 stable, `--locked`, release/bench profile (`optimized + debuginfo`)
- Caveat: absolute ms are not comparable across machines/adapters (see README). The box is shared, so noise is real: per-run p95 spread is up to ~1.5x on single runs; compare only against a re-run on the same box with the same flags. Two identical CI-parameter runs are included to show run-to-run variance.

## 1. Headless frame benchmark (`renderide-framebench`, lavapipe)

### 1a. CI parameters (mirrors `.github/workflows/frame-bench.yml`: 256x256, warmup 10, frames 20, runs 5, 2 threads pinned to CPUs 0-1)

| scenario | run | cpu_frame_ms p50 | p95 | p99 | draws | upload bytes |
|---|---|---|---|---|---|---|
| unlit_sphere | A | 0.242 | 0.275 | 0.293 | 1 | 1632 |
| unlit_sphere | B | 0.232 | 0.273 | 0.284 | 1 | 1632 |
| multi_primitive_unlit | A | 0.264 | 0.313 | 0.325 | 4 | 3168 |
| multi_primitive_unlit | B | 0.268 | 0.318 | 0.341 | 4 | 3168 |

Gate metric is `cpu_frame_ms.p95` (median of per-run p95). A vs B differ by <1.7%, well inside the 10% gate.

GPU pass p50 (ms, lavapipe, run A, 256x256): unlit_sphere: ClusteredLight 0.008, HiZBuild 0.006, SceneColorCompose 0.430, WorldMeshForwardDepthPrepass 0.247, WorldMeshForwardOpaque 0.640. multi_primitive_unlit: ClusteredLight 0.008, HiZBuild 0.005, SceneColorCompose 0.413, DepthPrepass 0.115, Opaque 0.351, TransparentSequence 0.857.

### 1b. Desktop-style parameters (1920x1080, warmup 30, frames 120, runs 3, 8 threads, unpinned; the `scripts/bench.sh` defaults, but on lavapipe)

| scenario | cpu_frame_ms p50 | p95 | p99 |
|---|---|---|---|
| unlit_sphere | 0.273 | 0.328 | 0.542 |
| multi_primitive_unlit | 0.327 | 0.407 | 0.441 |

GPU pass p50 (ms, software rasterizer, not representative of a real GPU): unlit_sphere: SceneColorCompose 3.565, DepthPrepass 1.324, Opaque 4.019, ClusteredLight 0.032, HiZBuild 0.018. multi_primitive_unlit: SceneColorCompose 3.353, DepthPrepass 0.892, Opaque 2.991, TransparentSequence 1.291, ClusteredLight 0.031, HiZBuild 0.016.

Note: the two built-in scenarios are tiny (1 and 4 draws), so CPU frame time is sub-millisecond and sensitive to noise. They are good for catching gross regressions, not for fine perf work. Raw reports: `docs/perf-baseline-2026-10-23/*.json`.

## 2. Criterion CPU microbenchmarks (`cargo bench -p renderide-bench`)

Point estimates [low mid high], default sampling, 8 vCPU shared box:

| bench | time |
|---|---|
| transforms/trs_and_degenerate_scale | 68.43 / 69.24 / 70.06 us |
| culling/homogeneous_clip_aabb | 50.05 / 50.74 / 51.43 us |
| world_matrices/forest_recompute | 167.7 / 174.8 / 186.6 us |
| world_matrices/reverse_chain_recompute | 228.8 / 231.2 / 233.7 us |
| buffer_pool/free_list_reuse | 11.98 / 12.14 / 12.31 us |
| material_uniforms/property_change_pack | 278.9 / 285.6 / 293.6 us |
| material_uniforms/one_material_repack | 180.0 / 182.5 / 185.1 ns |
| bind_groups/cache_hit | 117.9 / 120.3 / 122.7 us |
| bind_groups/miss_rebuild | 186.6 / 188.8 / 191.1 us |
| keyword_variants/cache_hit | 34.88 / 35.39 / 35.92 ns |
| keyword_variants/cache_miss | 43.48 / 44.02 / 44.58 ns |
| keyword_variants/compose | 34.37 / 35.04 / 35.80 ms |

`keyword_variants/compose` is ~1000x larger than every other bench (ms vs us/ns). It likely includes shader compose/naga work per iteration; worth confirming it is intentional. Total bench wall time 4m22s after build. `--features gpu` benches are not defined yet (feature is empty).

## 3. Lint, format, unsafe on master

- `cargo fmt --all -- --check`: clean (exit 0)
- `taplo fmt --check --diff`: clean (exit 0)
- `cargo clippy --all-targets --locked --all-features -- -W clippy::all -D warnings` (the Linux CI/pre-push command): clean, 0 warnings, 1m39s
- Policy: `[workspace.lints.clippy] undocumented_unsafe_blocks = "deny"`, `unsafe_op_in_unsafe_fn`/`missing_unsafe_on_extern` warn, `clippy.toml` relaxes unwrap/expect/panic/print/indexing in tests only. `renderide-shared` is `#![deny(unsafe_code)]` with scoped `expect(unsafe_code)` exceptions; `renderide-test` and several test crates are `forbid(unsafe_code)`. Policy docs: `CONTRIBUTING.md` (Code style), `SI_POLICY.md` (no new `unsafe` without `// SAFETY:` and maintainer review).
- `unsafe` count in `crates/**/*.rs` (word `unsafe`, line comments stripped; tests included):

| metric | count |
|---|---|
| `unsafe` keyword occurrences | 335 (351 raw `rg -w unsafe` lines incl. comments) |
| `unsafe { }` blocks | 305 |
| `unsafe fn` (incl. `unsafe extern "C" fn`) | 9 |
| `unsafe impl` | 8 |
| `unsafe extern` blocks | 6 |
| `// SAFETY:` comments | 313 |

Per crate (keyword occurrences): renderide 119, bootstrapper 90, interprocess 75, renderide-shared 24, logger 24, renderide-test 3; others 0. Hotspots: interprocess/src/ring.rs 37, bootstrapper/src/vr_prompt.rs 27, renderide/src/config/persist/load/tests.rs 22.

## 4. Exact repeatable commands

System prerequisites (Debian/Ubuntu): `sudo apt-get install -y mesa-vulkan-drivers vulkan-tools build-essential pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev` (a C++ compiler is required by `imgui-sys`); `rustup component add clippy`.

```bash
git checkout aafcb72035fa2bc24b860cb2c6c3d4ece5aa49c3
export CARGO_TARGET_DIR=/workspace/target-gauge   # any path
# CPU microbenchmarks (no GPU)
cargo bench --locked -p renderide-bench
# Headless frame bench on lavapipe
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json VK_DRIVER_FILES=$VK_ICD_FILENAMES \
       WGPU_BACKEND=vulkan RENDERIDE_LOG_TEE_TERMINAL=0 RAYON_NUM_THREADS=2
cargo build --locked --release -p renderide -p renderide-framebench
taskset -c 0-1 $CARGO_TARGET_DIR/release/renderide-framebench run \
  --renderer $CARGO_TARGET_DIR/release/renderide-renderer \
  --resolution 256x256 --warmup 10 --frames 20 --runs 5 --timeout-seconds 600 \
  --output results/head.json
# 1080p variant (bench.sh defaults, lavapipe, 8 threads)
RAYON_NUM_THREADS=8 $CARGO_TARGET_DIR/release/renderide-framebench run \
  --renderer $CARGO_TARGET_DIR/release/renderide-renderer \
  --resolution 1920x1080 --warmup 30 --frames 120 --runs 3 --timeout-seconds 900 --output results/fb-1080p.json
# Compare candidate vs baseline (gate: cpu_frame_ms.p95, default 10%)
$CARGO_TARGET_DIR/release/renderide-framebench compare --baseline results/master.json --candidate results/head.json --threshold 10
# Hosted alternative: GitHub Actions "Frame bench" (workflow_dispatch, base_ref/head_ref inputs) on the fork
# Lint/fmt
cargo fmt --all -- --check && taplo fmt --check --diff
cargo clippy --all-targets --locked --all-features -- -W clippy::all -D warnings
```

For PR comparison, always build and run base and head back to back on the same machine with identical flags and compare with `framebench compare`; run the CI-parameter config at least twice to gauge noise first.
