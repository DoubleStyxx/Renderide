#216 WARP black frame: investigation plan

Do not merge the canary. Master `8831f004` contains #186. The quarantine on master is `quarantined:#216` until `2026-10-31T23:59:59Z`. Findings are due Nov 7. This plan is the root-cause path; it is not a request to extend the quarantine.

The three Windows samples below are from the canary **before** that merge (`03e36e51`, `c7194e42`, `4a271512`). The first job after this plan is to repeat the same readback on the merged tree.

## What those samples already show

All three accepted a frame and were labeled `expected failure (WARP, #212)`, not `FlatOrUnstableFrame`. Suite time was about three minutes. CPUs were EPYC 9V74 (run [41084635804](https://github.com/DoubleStyxx/Renderide/actions/runs/41084635804)) and EPYC 7763 (runs [41089310635](https://github.com/DoubleStyxx/Renderide/actions/runs/41089310635), [41093531109](https://github.com/DoubleStyxx/Renderide/actions/runs/41093531109)).

Each encoded the same draw, then read it back as black spheres on the magenta clear:

- `forward_draws=4`, mesh 20, indices `0+3024`, one instance, `pbsmetallic_default` variant `0x0`, `hiz=0`, `depth_draws=0`, `color_load=clear`, `depth_load=clear`, skybox `clear_color`.
- Readback class `black_on_clear`: 17214 pixels `(0,0,0,255)`, 85186 magenta, `other=0`. Unique colors 2. Coverage 0.168 matches the golden non-background fraction.
- `diag-readback-retry`: `poll=QueueEmpty`, `poll_ms=0`, `same_bytes=true`.
- CPU upload: 2 lights, first type 1, color `(1, 0.92, 0.82)`, intensity 1.4, no NaNs. Four materials with non-zero colors. Ambient SH L0 is 0. Camera `(0,0,-4)`, projection finite.
- Other cases in the same jobs read back `class=shaded`. Unlit color is not black.

`direct_metallic_clustered` returns `vec3(0)` when the cluster's light count is 0 (`shaders/modules/pbs/lighting.wgsl`). With no indirect contribution, the fragment is black even though the CPU light list is not.

## Hypotheses

### 1. The frame encodes no draws

**Ruled out** on the pre-merge canary. The experiment was the per-draw log. Evidence: four draws, same mesh and index range, on all three runs. Owner: Bridge. ETA: done Oct 30.

### 2. Draws are encoded but fail the depth test, so the readback is the clear

**Ruled out** for this picture. A depth reject would leave magenta in the sphere coverage. Those pixels are exact black, and the count matches the golden coverage. Depth clear is 0 and compare is `GreaterEqual`. Owner: Bridge. ETA: done Oct 30.

### 3. The readback is stale, and `device.poll(Wait)` or a second submit would show shaded pixels

**Ruled out** for this picture. The retry after `PollType::Wait` matched the first copy on all three runs (`same_bytes=true`, `poll_ms=0`). The published-frame stall is 163–189 ms. Owner: Bridge. ETA: done Oct 30. The older 85–127 s `map_async` stall (#214) is a separate Gauge item and does not explain a black readback that is stable after GPU-done.

### 4. The CPU upload is NaN, zero lights, or zero materials

**Ruled out** on the CPU side. Lights and material colors are finite and non-zero. Ambient SH L0 is 0, which is recorded but is not by itself the direct-light path. Owner: Bridge. ETA: done Oct 30. This does not prove the GPU saw those bytes. The frame, light, and material buffers are not `COPY_SRC`, so a GPU readback was not taken.

### 5. The cluster light list is empty at shade time, so PBS writes black

**Leading hypothesis.** The shader returns black when the cluster range count is 0, and the pixels are exact black rather than a tinted wrong color. CPU `light_count` is 2, so the bug would be the cluster build or the bind the shader reads, not the light authoring.

**Experiment.** On a `pbs_lit` frame, log the clustered-light pass result for that view: light count written, cluster ranges that are non-empty, and whether the pass ran. Compare one Windows WARP log with one Linux llvmpipe log from the same commit. No WGSL change in this step.

- Count is 0 on WARP and non-zero on Linux: the cluster pass or its upload is the bug. Bridge fixes that path.
- Count is non-zero on both, and WARP stays black: hand the bind and the `direct_metallic_clustered` inputs to Shader.

Owner: Bridge for the log and the WARP-vs-Linux compare. Shader only if the count is non-zero. ETA: log on the canary by Nov 3; read the samples by Nov 5; Shader follow-up, if needed, by Nov 6. Findings note Nov 7.

### 6. All four draws bind material offset 0 and share one slot

**Open.** Every draw logged `/e0`, and each material upload also said `offset=0`, while the four colors differed. That is either four buffers at offset 0 or one slot overwritten. Black pixels hide which.

**Experiment.** Log a hash of the bound uniform bytes and whether the four group-1 binds are distinct buffers. If they alias, Bridge fixes the arena offset. If they are distinct and match the uploaded colors, this hypothesis is closed.

Owner: Bridge. ETA: Nov 4, beside the cluster log.

### 7. A flat `(0,0,0,255)` timeout is this same black fragment on the production black clear

**Likely, not yet remeasured on the merged tree.** Black spheres plus a black clear is a flat frame, so the harness waits until `FlatOrUnstableFrame`. The magenta clear makes the same frame non-flat, so these runs were accepted as #212. Three pre-merge samples did not produce a flat timeout. Older canary runs (`92dd8076`) had ~17k non-black, non-magenta pixels, so a tinted #212 picture has existed and was not reproduced here.

**Experiment.** Keep the magenta clear. On the merged tree, classify each Windows `pbs_lit` readback as `black_on_clear`, `shaded`, `magenta_clear`, or `black`. Stop at a non-`black_on_clear` sample or at 8 merged-tree runs, whichever comes first. Do not turn the clear back to black just to recreate the timeout.

Owner: Bridge. ETA: Nov 3 for the first merged-tree samples, Nov 5 for the set.

## Order

1. Bridge: merged-tree samples (hypothesis 7), using the logs already on the canary.
2. Bridge: cluster-count log and the material-bind hash (hypotheses 5 and 6).
3. Shader: only if hypothesis 5's count is non-zero and the frame is still black.
4. Gauge: #214 stall, after the Nov 7 color finding, not instead of it.
