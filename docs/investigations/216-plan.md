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

## How #212 and #216 are chosen

On master `8831f004` the capture gate runs first, then the two labels, in this order (`suite.rs` `run_suite_with`):

1. `png_readback` either accepts a frame or times out.
2. `warp_pbs::apply_expected_failure` (#212).
3. `quarantine_216::apply_quarantine_now` (#216).

A frame is a settle candidate only when `gpu_done`, pending=0, no dropped draws, and `golden::is_flat_image` is false (`gpu_ready.rs`). Flat means every channel's min/max span is at most 1. Two consecutive candidates are required. A flat frame is `SettleDecision::Reject` and never accepted.

Timeout with `last_pending == Some(0)` becomes `HarnessError::FlatOrUnstableFrame`, whose text contains `flat or unstable`. That path does not run the image check. `FlatImage` (`flat single color`) is a different error: the golden check, used when the only frames were flat and pending was not 0.

#212 matches only an accepted frame whose error contains `case failed tolerance`, on WARP, for `pbs_lit_material_matrix`, with pending=0 and no dropped draws. It then rewrites `error` and `result` to `expected failure (WARP, #212)`. `fault_from_failure` checks `flat or unstable` and `flat single color` before `case failed tolerance`, so those errors are not #212.

#216 matches only while `now < 2026-10-31T23:59:59Z`, on WARP, for that same case, when the error contains `flat or unstable` and does not contain `pipeline-pending` or `PipelineNotReady`, and the PNG text is pending `0` and gpu_done `1`. It sets `status=quarantined` and `reason=quarantined:#216`. It does not read pixels. `flat single color` is not a #216 match (`other_failures_stay_hard_failures`).

They do not overlap. #212 runs first but refuses a `flat or unstable` error, so it cannot erase a #216 case. #216 leaves a #212 label alone (`expected_failure_label_is_left_unchanged`). A settled non-flat frame cannot reach #216, because that frame is accepted and the error is the image check, not `flat or unstable`. An unsettled capture can still be #216 without the last PNG being flat: the timeout only requires last pending=0.

## Windows samples

| Run | CPU | Clear | Label | black | other |
| --- | --- | --- | --- | --- | --- |
| [40088875619](https://github.com/DoubleStyxx/Renderide/actions/runs/40088875619) `c721bbbe` | Xeon Platinum 8370C | magenta | #212 | 8547 | 8667 |
| [40096421641](https://github.com/DoubleStyxx/Renderide/actions/runs/40096421641) `92dd8076` | EPYC 9V74 | magenta | #212 | 27 | 17187 |
| [40097968491](https://github.com/DoubleStyxx/Renderide/actions/runs/40097968491) `234d01e7` | not recorded | magenta | #212 | not recorded | not recorded |
| [41084635804](https://github.com/DoubleStyxx/Renderide/actions/runs/41084635804) `03e36e51` | EPYC 9V74 | magenta | #212 | 17214 | 0 |
| [41089310635](https://github.com/DoubleStyxx/Renderide/actions/runs/41089310635) `c7194e42` | EPYC 7763 | magenta | #212 | 17214 | 0 |
| [41093531109](https://github.com/DoubleStyxx/Renderide/actions/runs/41093531109) `4a271512` | EPYC 7763 | magenta | #212 | 17214 | 0 |
| [41103575428](https://github.com/DoubleStyxx/Renderide/actions/runs/41103575428) `48f67568` | EPYC 9V45 | magenta | #212 | 17214 | 0 |
| [41099685993](https://github.com/DoubleStyxx/Renderide/actions/runs/41099685993) `8c514272` (#219) | EPYC 9V45 | production black | #212 | 93697 | 8703 |

No row is `quarantined:#216`. The pure-black canary rows (`other=0`, 17214 black, 85186 magenta) are exact `(0,0,0,255)` spheres. The production clear is linear black, stored as `(0,0,0,255)`. `black_spheres_on_the_production_clear_are_flat` runs that through `is_flat_image`: the same pixels with the background replaced by black are flat; one magenta pixel is not. A pure-black sphere frame on the production clear is therefore rejected by settle and times out as `FlatOrUnstableFrame`, which is the #216 path. #219 is the other exit: 8703 non-black pixels, 1435 unique colors, luma range 255, so it was accepted and took #212. Same shading bug, two harness exits. The exit depends on whether the accepted frame stays within one color of the clear.

## Keeping the #216 path in CI

Do not wait on a WARP sample to land flat. The classifier is already what CI should run:

- `warp_flat_frame_is_quarantined_before_expiry_and_fails_after` builds a real `FlatOrUnstableFrame` string and a PNG with pending `0` and gpu_done `1`, then calls `apply_quarantine`.
- `other_failures_stay_hard_failures` shows `flat single color` and a pipeline timeout are not #216.
- `expected_failure_label_is_left_unchanged` shows a #212 label is not reclassified.
- `warp_pending_timeout_flat_and_dropped_draws_stay_failures` shows a flat timeout is not #212.
- `black_spheres_on_the_production_clear_are_flat` locks the pixel rule that sends a pure-black sphere frame to that timeout.

That set is the Nov 14 check. A forced-flat GPU frame on WARP is not required, and it would still depend on which way the shade falls. Owner: Bridge. ETA: the tests are on the canary now; they stay in the #216 fix PR. No Shader or Gauge work for this routing.

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

### 7. #216 and #212 are one shading bug with two harness exits

**Supported.** A pure-black sphere frame on the production clear is flat (`black_spheres_on_the_production_clear_are_flat`), so settle rejects it and the timeout is #216. The same pixels on the magenta clear are not flat, so the frame is accepted and the image check is #212. That is runs 41084635804, 41089310635, 41093531109, and the merged-tree run 41103575428. #219 (`41099685993`, production clear, 8703 non-black pixels) was not flat and took #212. Older magenta runs with thousands of `other` pixels are the same #212 exit. Owner: Bridge. ETA: routing is done Oct 30. The shade itself is hypothesis 5.

## Order

1. Bridge: cluster-count log and the material-bind hash (hypotheses 5 and 6). The #212/#216 split is the routing above, not another sample hunt.
2. Shader: only if hypothesis 5's count is non-zero and the frame is still black.
3. Gauge: #214 stall, after the Nov 7 color finding, not instead of it.
