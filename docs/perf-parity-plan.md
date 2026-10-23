# Performance parity plan (issue #148): Renderide vs the Unity renderer

Owner: Gauge Drone (benchmarks and perf gate). Feature owners attach numbers using the block in section 6.

Status: draft. Everything marked **lavapipe** is measured and repeatable on any Linux box with Mesa lavapipe. Everything marked **real GPU** needs a machine with a physical GPU and (for the Unity side) a Resonite install; nothing in this document claims a Unity number that was not measured.

## 1. What "parity" means here

Three separable questions, each with its own tool:

| Question | Metric | Tool | Where it can run |
|---|---|---|---|
| Does the renderer burn more **CPU** per frame than the reference for the same scene? | `cpu_frame_ms` p50/p95, draw calls, upload bytes | `renderide-framebench`, Criterion (`renderide-bench`) | lavapipe is enough for **relative** (PR vs master) numbers; absolute Unity comparison needs the same real machine |
| Does it burn more **GPU** time? | per-pass GPU ms (`gpu_passes_ms`), total GPU ms | `renderide-framebench` with a real adapter (timestamp queries) | **real GPU only** (lavapipe pass times are a software rasteriser) |
| Does it use more **memory** or produce different pixels? | VRAM, RSS; golden diffs | OS tools, `renderide-test check-suite` | goldens: lavapipe; VRAM/RSS: real machine |

Rule: lavapipe numbers answer "did this PR make Renderide slower than master". They never answer "is Renderide faster than Unity".

## 2. Comparison scenes

Scenes are host-driven (the framebench host replays a fixed scene over IPC; see `crates/renderide-test/src/host/scene_session/`), so the same scene runs against any renderer binary. Scenes marked *implemented* exist on this branch; the rest are specified so owners can add them.

| ID | Scenario (`--scenario`) | What it stresses | Draws / lights | Status |
|---|---|---|---|---|
| S0 | `unlit_sphere`, `multi_primitive_unlit` | smoke, startup, per-frame floor | 1 / 4 | existing (CI default) |
| S1 | `many_static_meshes` | draw-call count, instancing, CPU frustum/Hi-Z cull, draw prep | 1,600 unlit spheres, one mesh and material, 0 lights | implemented |
| S2 | `lights_shadows` | clustered lights, shadow atlas, PBS forward | floor + 8x8 PBS spheres, 1 shadowed directional + 4 shadowed point lights | implemented |
| S3 | `pbs_lit_material_matrix` | PBS shading, material bind groups | existing golden scene, now selectable in framebench | implemented (selector only) |
| S4 | `many_shadowed_point_lights` | shadow-heavy lighting (30 shadowed points) | defined by PR #46 (merged); opt-in, raises `per_pixel_lights` to 32 | implemented (master) |
| S5 | `offscreen_skinned_avatars` (+ control `onscreen_skinned_avatars`) | skinned deform, CPU cull of skinned renderers (#166/#177), shadow casters that stay when off-screen | floor + 16 visible and 256 behind-the-camera one-bone skinned spheres (host-sent posed bounds), all shadow casters of one soft-shadowed sun; control puts all 272 in view | implemented (host now uploads one-bone skinned meshes, bone assignments and posed bounds) |
| S6 | `ui_canvas_stack` | UI draw ordering, transparent sort (#50) | hundreds of coplanar canvas quads | to add; CPU part already covered by `transparent_sort` Criterion bench on #50 |
| S7 | `vr_mirror` | multiview + secondary view | stereo + mirror camera | **real GPU/OpenXR**; not representable on lavapipe |
| S8 | `particles` | particle-system CPU update + draw | emitters with N particles | to add once a particle upload path is scriptable |

Camera, resolution, warmup and frame counts are part of the scene definition: S1/S2 use the CI config (256x256, warmup 10, frames 20, runs 5) for the gate and 1920x1080 (warmup 30, frames 120, runs 3) for the reference table.

## 3. What is measurable where

### Lavapipe + Criterion (today, no GPU)

- `cargo bench -p renderide-bench` (`benches/cpu_hot.rs`): transforms, culling (`homogeneous_clip_aabb`), world matrices, material uniforms, bind groups, keyword variants, buffer pool. PR-owned groups (e.g. `shadow_churn`, `hysteresis_*`, `transparent_sort`) appear on their branches.
- `renderide-framebench` with S0-S3: CPU frame time, draw count, upload count/bytes, per-pass GPU ms *relative* between two builds on one machine.
- `renderide-test check-suite`: golden images (correctness, not perf).
- Allocation checks (per-frame heap) via the PR's own Criterion or test, as required by the issue templates.

Known limits on lavapipe: absolute ms are not comparable across machines; GPU pass times measure a CPU rasteriser; the shared VM adds roughly 5-10% run-to-run noise on sub-millisecond frames (measured, see the baseline doc `docs/perf-baseline-2026-10-23.md`, which lives on PR #141 / branch `gauge/perf-baseline` (not yet on master) at https://github.com/DoubleStyxx/Renderide/blob/gauge/perf-baseline/docs/perf-baseline-2026-10-23.md, and section 5), so the gate uses repeats and a median.

### Needs a real GPU (and the Unity reference)

| Need | Why | Who |
|---|---|---|
| Per-pass GPU ms, total GPU ms, VRAM | lavapipe is not a GPU | Styxx (or any machine with a discrete/integrated GPU) |
| Unity/Resonite reference numbers for S1-S3, S5-S8 | needs Resonite (Unity renderer) on the same machine and a world that recreates the scene | Styxx |
| Shadow quality/perf (#46), Hi-Z popping (#52), UI z-fighting (#50) | visual and driver-dependent | Styxx or a GPU runner |
| VR mirror (S7), multiview, OpenXR | needs headset or runtime | Styxx |

## 4. Real-GPU run book (what Styxx would run)

Machine notes to record in every table: CPU model, GPU model + driver version, OS, resolution, display refresh, power mode, `rustc --version`, git SHA.

```bash
# 0. build
cargo build --locked --release -p renderide -p renderide-framebench

# 1. Renderide, real adapter (do NOT set VK_ICD_FILENAMES to lavapipe; WGPU_BACKEND=vulkan|dx12|metal as appropriate)
export RENDERIDE_LOG_TEE_TERMINAL=0
for s in unlit_sphere multi_primitive_unlit many_static_meshes lights_shadows pbs_lit_material_matrix; do
  target/release/renderide-framebench run --renderer target/release/renderide-renderer \
    --scenario "$s" --resolution 1920x1080 --warmup 60 --frames 300 --runs 5 \
    --timeout-seconds 900 --output "results/gpu-$s.json"
done
# the report has adapter name, backend, timestamp_query, cpu_frame_ms{p50,p95,p99}, draw_calls,
# upload_bytes and gpu_passes_ms per pass. Check adapter.device_type != Cpu before trusting GPU ms.

# 2. Criterion on the same machine (CPU only)
cargo bench --locked -p renderide-bench

# 3. Unity/Resonite reference, same machine, same resolution
#    Build the equivalent scene in a Resonite world (S1: 40x40 grid of spheres at 1.1 spacing, one shared
#    material; S2: floor + 8x8 PBS spheres, 1 directional + 4 point lights, soft shadows).
#    Record: Resonite's frametime graph / Unity Profiler (CPU ms, GPU ms), draw calls (RenderDoc or
#    the Unity Frame Debugger), and VRAM (nvidia-smi / the OS GPU monitor). Use at least 300 frames after 60
#    warmup frames, vsync off, window size 1920x1080.

# 4. Fill the parity table (section 7) and file a follow-up issue per scene where Renderide is slower.
```

## 5. Noise floor (measured, lavapipe, 8 vCPU shared VM)

- Criterion: the same binary run twice, 9 benches apart, differs by 1-3% on the 50-250 us benches and by up to 7-20% on the 35 ns and 190-220 us (`bind_groups/miss_rebuild`) benches. Use the median of at least 3 interleaved runs and only call a regression above max(5%, observed spread).
- framebench CI config on the 1-4 draw scenes: ~0.23-0.31 ms, repeat spread of about 5-10% on p50 and p95. The first run after a quiet period is consistently slower; discard run 1 or warm up with a throwaway run.
- Therefore the ~2% floor quoted informally applies only to the largest scenes (S1/S2) and to Criterion benches over ~50 us. The gate below uses S1/S2, where frame time is milliseconds and relative noise is smaller.

## 6. How to attach numbers (copy-paste block for PR descriptions)

```
### Perf evidence (Gauge format v1)
- Base: <master SHA>   Head: <PR head SHA>   (PR head merged with master? yes/no)
- Machine: <cpu>, <vCPU>, adapter=<llvmpipe|real GPU name>, rustc <ver>, run on same box back to back
- Scenes/benches (from the table in docs/perf-parity-plan.md section 8): <list>
- Commands:
  cargo bench --locked -p renderide-bench              # base, then head, then base again (noise)
  taskset -c 0-1 renderide-framebench run --renderer <bin> --scenario <s> --resolution 256x256 \
     --warmup 10 --frames 20 --runs 5 --output <out>.json   # repeat 3x, interleave base/head
  renderide-framebench compare --baseline base.json --candidate head.json --threshold 10 \
     --gate cpu_frame_ms.p50 --gate cpu_frame_ms.p95
- Results (median of 3 interleaved repeats):
  | scene/bench | metric | base | head | delta | noise (base vs base) | verdict |
- Draw calls / upload bytes: <base -> head>
- Allocation check: <how verified, per-frame allocs 0>
- unsafe delta: +<n>/-<n> (each with // SAFETY:), clippy CI command clean, fmt/taplo clean
- Needs real GPU to judge: <yes/no, what>
```

CI shortcut: Actions -> "Frame bench" -> Run workflow with `base_ref=master`, `head_ref=<branch>`, `scenarios=many_static_meshes,lights_shadows` (input added on this branch), `runs=5`.

## 7. Parity table (to be filled; one row per scene and machine)

| Scene | Machine / adapter | Renderide CPU ms p50/p95 | Renderide GPU ms | draws | VRAM | Unity/Resonite CPU ms | Unity GPU ms | draws | VRAM | delta | follow-up issue |
|---|---|---|---|---|---|---|---|---|---|---|---|
| S1 many_static_meshes | | | | | | | | | | | |
| S2 lights_shadows | | | | | | | | | | | |
| S3 pbs_lit_material_matrix | | | | | | | | | | | |
| S5 offscreen_skinned_avatars | | | | | | | | | | | |
| S6 ui_canvas_stack | | | | | | | | | | | |
| S7 vr_mirror | | | | | | | | | | | |

Lavapipe reference numbers for the implemented scenes are in section 10.

## 8. Upcoming feature PRs: scenes, benches, metric, threshold

Thresholds are relative to master measured on the same box in the same session. "Fail" means hold the PR until explained; an intended cost must be stated with its number (per the issue template). Note that `renderide-framebench compare` currently has a single `--threshold` per invocation, so per-metric thresholds in this table need one `compare` invocation per distinct threshold (see section 9, item 3).

| PR / issue | Owner | Scenes (framebench) | Criterion benches | Metric | Threshold | Notes |
|---|---|---|---|---|---|---|
| #166 skinned culling (fork PR #177) | Shader | S5 `offscreen_skinned_avatars` + control `onscreen_skinned_avatars` (regression guard), S1/S2 unchanged | `culling/skinned_homogeneous_clip_aabb` vs `culling/homogeneous_clip_aabb` (both exist on #177) | S5 draw calls: offscreen must equal the visible share (17) and onscreen ~271-273 on base AND head; no allocation in the clip loop (`skinned_clip_loop_allocates_nothing`); S1/S2 must not regress | S1/S2 p50,p95 <= +5% (noise-adjusted); rigid row <= +5%; skinned row <= +10% over the rigid row | **Measured 2026-10-23 (section 10): production master already culls skinned draws via the prepared-renderables path; #177 changes only the `#[cfg(test)]` scene-walk collector plus a no-op `mesh_bounds` refactor, so no end-to-end frame-time or draw-count delta is expected or observed.** Real GPU only for skinned deform cost |
| #144 clear modes | Shader | S2 plus a two-camera stacked-render-texture scene (to add) | none new (load-op change) | `cpu_frame_ms` p50/p95; GPU pass ms for Opaque/DepthPrepass (real GPU) | S1/S2 <= +5%; no new attachments (upload bytes/count unchanged) | golden for `Nothing`/`Depth`; load-op savings only visible on a real GPU (tile GPUs) |
| #145 LOD cross-fade | Shader | S1 with cross-fade off must be bit-identical (draw count equal); a fading-LOD scene (to add) | allocation check on LOD collect; add `lod_select` bench if CPU cost is claimed | draw count off=base; extra draws only in transition; `cpu_frame_ms` | fade off: draws identical, p50 <= +2% (S1); fade on: report the extra-draw cost | golden mid-fade frame |
| #147 camera double buffering | Shader | render-texture camera scene (to add), S0 | none | VRAM (second image created once), `cpu_frame_ms` | S0/S1 <= +5%; VRAM +1 color image per double-buffered texture only | VRAM check needs a real GPU or wgpu allocation report |
| #146 velocity buffer | Shader | S1/S2 with motion blur on and off (to add `motion_blur` flag) | allocation check: no per-frame previous-matrix `Vec` | `cpu_frame_ms`, GPU ms Opaque/MotionBlur | motion blur off: S1/S2 <= +3% (extra MRT must not tax the off path); on: report cost | GPU cost of the extra MRT is a **real GPU** question |
| #153 IME preedit | Bridge | none (input path) | none; unit tests on the input snapshot | idle allocations per frame | no per-frame alloc when idle; S0 p50 <= +2% | framebench is the wrong tool |
| #154 texture sub-regions | Bridge | S0 + upload-heavy scene (to add) | upload bytes bench in the asset worker (add `texture_subregion_upload` to `cpu_hot.rs`) | bytes uploaded per sub-rect <= rect bytes; asset-worker CPU ms | bytes == rect size; CPU ms <= +5% vs full upload path | |
| #151 dynamic VR/desktop switch | Bridge | none | none | session test | no per-frame alloc difference between modes; S0/S1 p50 <= +2% in desktop | framebench is the wrong tool; **real GPU/OpenXR** for the switch |
| #155 ASTC native upload | Bridge | texture-heavy scene (to add) | asset-worker upload CPU time and bytes (before/after decode) | upload CPU ms, upload bytes, VRAM | CPU ms and VRAM must drop vs decode path; non-ASTC devices unchanged (<= +2%) | native ASTC only on devices with `TEXTURE_COMPRESSION_ASTC`; lavapipe exercises the fallback path only |

## 9. Proposed CI perf gate

Use the existing manual workflow (`.github/workflows/frame-bench.yml`, `workflow_dispatch` with `base_ref` / `head_ref`); it builds both refs on one runner, pins to 2 CPUs and compares medians of per-run percentiles. Proposed changes (this branch includes the workflow change, which is additive):

1. **`scenarios` input** (done on this branch): comma-separated list of `[a-z0-9_]` names, passed through `env:` (not interpolated into the script) and regex-validated like `threads`, then forwarded as repeated `--scenario`. **Each tree is measured with its own `renderide-framebench` binary** (base on base's, head on head's), as before. A scenario that the base tree does not define (checked against its `scenario.rs`) is run on head only; `compare` then shows it as `missing` (base N/A) and it cannot fail the gate. If none of the requested scenarios exist on base, the relative gate is skipped and the summary says so.
2. **Default gate set**: `unlit_sphere,multi_primitive_unlit,many_static_meshes,lights_shadows`. The tiny scenes catch gross regressions; S1/S2 (milliseconds) are the ones with a usable signal.
3. **Thresholds**: `--gate cpu_frame_ms.p50 --gate cpu_frame_ms.p95 --threshold 10` for scenes under 1 ms (noise 5-10%), `--threshold 5` for S1/S2. Also gate on `draw_calls.p50` and `upload_bytes.p50` with threshold 0 (these are deterministic; any change must be explained). **Limitation:** `renderide-framebench compare` takes ONE `--threshold` that applies to every `--gate` metric of that invocation (and to all scenarios in the report); it has no per-metric or per-scenario thresholds. Until per-metric support exists (follow-up: accept `--gate metric=percent` in `compare.rs`), run `compare` more than once with different `--gate`/`--threshold` pairs (timing metrics at 5-10, `draw_calls.p50`/`upload_bytes.p50` at 0) and fail if any invocation fails; the table in section 8 gives the intended threshold per metric, not what a single invocation can express.
4. **Repeat policy**: `runs=5`; re-dispatch once before treating a failure as real (the first run after idle is slow). Add `workflow_dispatch` input `repeat_on_fail` later if needed.
5. **Criterion** is a separate, non-gating job: `cargo bench -p renderide-bench` for base and head back to back, base twice, report only (Criterion noise on shared runners is 5-20%). Promote to a gate only for benches with a stable spread below 3%.
6. **When to run**: manually on PRs labelled `perf` and on every PR touching `crates/renderide/src/{world_mesh,render_graph,passes,gpu,materials,occlusion}`; the repo's `pull_request` trigger stays off until runner noise is characterised (open question for Prism).
7. **Real-GPU nightly** (future, needs a self-hosted GPU runner): same commands as section 4 with a stored artifact per commit and a trend chart; gate on GPU ms p95 at 10%.

## 10. Measured lavapipe reference (this branch's scenes)

Measured 2026-10-23 12:0x-12:3x MDT by Gauge Drone. Machine: shared 8-vCPU Linux VM, **llvmpipe/lavapipe (no real GPU)**, rustc stable, release+debuginfo, fork master **2bb2af47** (before that 3c084f4f for the PR matrix) vs #177 head 90f7c05c. Medians of 4 interleaved rounds (CI config: 256x256, 2 cores, warmup 10, frames 30, runs 5) and 3 rounds (1280x720, warmup 20, frames 60, runs 3, 8 threads). Expect +/-5-10% round-to-round noise.

| Scene | CI cfg: draws, cpu_frame p50 / p95 ms | 1280x720: draws, p50 / p95 ms |
|---|---|---|
| S0 unlit_sphere | 1, 0.24 / 0.29 | 1, 0.25 / 0.29 |
| S0 multi_primitive_unlit | 4, 0.27 / 0.33 | 4, 0.28 / 0.34 |
| S1 many_static_meshes | 676, 1.9 / 2.3 | 1,040, 2.1 / 2.5 |
| S2 lights_shadows (casters On) | 65, 0.73 / 1.3 | 65, 0.50 / 0.58 |
| S5 offscreen_skinned_avatars | 17, 0.67 / 0.84 | 17, 0.98 / 1.35 |
| S5 control onscreen_skinned_avatars | 271, 1.0 / 1.2 | 273, 1.7 / 2.3 |

Reading: the S5 pair is the intended regression guard: 256 behind-the-camera avatars cost nothing in draw calls on either tree (17 vs 271-273 draws), so any change that makes the off-screen share draw again will show in `draw_calls` immediately (deterministic, gate at 0%). S1 draw counts differ between resolutions because of frustum/Hi-Z culling. Criterion reference (same box): `culling/homogeneous_clip_aabb` ~49-51 us, `culling/skinned_homogeneous_clip_aabb` ~52 us, `shadow_churn/static_repaint` ~120-130 us, `shadow_churn/cold_plan` ~4.2-4.5 ms, `keyword_variants/compose` ~34 ms (real per-variant naga-oil compose cost on the warmup worker, not a frame hitch), `transparent_sort/precomputed_10k` ~1.73 ms.

Multi-thread lock behaviour is not visible in framebench or Criterion; the `gauge/lock-contention-bench` branch adds `renderide-lockbench` (`cargo run --release --manifest-path crates/renderide-lockbench/Cargo.toml -- --threads 2,4,8`, a standalone package outside the workspace) for A/B-ing lock crates; use it whenever a dependency bump touches `parking_lot` or the GPU queue gate.

### Exact commands for Styxx on a real GPU
```
git clone https://github.com/DoubleStyxx/Renderide && cd Renderide
cargo build --release --locked -p renderide -p renderide-framebench
unset VK_ICD_FILENAMES VK_DRIVER_FILES        # use the real driver
for s in many_static_meshes lights_shadows pbs_lit_material_matrix offscreen_skinned_avatars onscreen_skinned_avatars; do
  RAYON_NUM_THREADS=8 target/release/renderide-framebench run --renderer target/release/renderide-renderer \
     --scenario $s --resolution 1920x1080 --warmup 30 --frames 120 --runs 3 --output perf-$s.json
done   # check adapter.device_type != Cpu in the JSON; keep gpu_passes_ms
```
