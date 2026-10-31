#216 WARP black frame: investigation plan

Do not merge the canary. Findings are due Nov 7. This plan is the root-cause path; it is not a request to extend the quarantine.

#232 has merged to master as `6f0c7aa6d1cb0fc2964cb3444da40369a073e42b`. This canary does not contain that commit. After that merge, #216 covers four WARP lit cases through `2026-11-14T23:59:59Z` (`quarantine_216.rs` on that commit):

- `pbs_lit_material_matrix` matches only a flat frame, reason `quarantined:#216`.
- `negative_light_parity`, `negative_light_physically_based`, and `negative_color_physically_based` match a flat frame or any finished comparison, including a tolerance pass, reason `WARP lit shading untrusted`.

The three Windows samples in the first rows of the table are from the canary **before** the #186 merge (`03e36e51`, `c7194e42`, `4a271512`). The #232 measurements are in the evidence section.

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

That split is the canary's view of master `8831f004`. On master `6f0c7aa6` the same flat-frame rule still applies to `pbs_lit_material_matrix`. The three negative-light cases are wider: `quarantine_matches` is true on WARP when the flat-frame signature matches or `image_compared` is set, and the stamped reason is `WARP lit shading untrusted`. A tolerance pass is quarantined. A harness error other than that flat frame stays a failure. The expiry in that file is `2026-11-14T23:59:59Z`.

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
| [41145831835](https://github.com/DoubleStyxx/Renderide/actions/runs/41145831835) `4bd5eef0` (#232) | EPYC 7763 | production black | see evidence | see evidence | see evidence |
| [41150822333](https://github.com/DoubleStyxx/Renderide/actions/runs/41150822333) `6ae283f1` (#232) | EPYC 7763 | production black | see evidence | see evidence | see evidence |

No canary row is `quarantined:#216`. On #232 run 41145831835 the flat timeouts for `negative_light_parity` and `negative_color_physically_based` are stamped `quarantined:#216`. The pure-black canary rows (`other=0`, 17214 black, 85186 magenta) are exact `(0,0,0,255)` spheres. The production clear is linear black, stored as `(0,0,0,255)`. `black_spheres_on_the_production_clear_are_flat` runs that through `is_flat_image`: the same pixels with the background replaced by black are flat; one magenta pixel is not. A pure-black sphere frame on the production clear is therefore rejected by settle and times out as `FlatOrUnstableFrame`, which is the #216 path. #219 is the other exit: 8703 non-black pixels, 1435 unique colors, luma range 255, so it was accepted and took #212. Same shading bug, two harness exits. The exit depends on whether the accepted frame stays within one color of the clear.

## Keeping the #216 path in CI

Do not wait on a WARP sample to land flat. The classifier is already what CI should run:

- `warp_flat_frame_is_quarantined_before_expiry_and_fails_after` builds a real `FlatOrUnstableFrame` string and a PNG with pending `0` and gpu_done `1`, then calls `apply_quarantine`.
- `other_failures_stay_hard_failures` shows `flat single color` and a pipeline timeout are not #216.
- `expected_failure_label_is_left_unchanged` shows a #212 label is not reclassified.
- `warp_pending_timeout_flat_and_dropped_draws_stay_failures` shows a flat timeout is not #212.
- `black_spheres_on_the_production_clear_are_flat` locks the pixel rule that sends a pure-black sphere frame to that timeout.

That set is the Nov 14 check. A forced-flat GPU frame on WARP is not required, and it would still depend on which way the shade falls. Owner: Bridge. ETA: the tests are on the canary now; they stay in the #216 fix PR. No Shader or Gauge work for this routing.

## Evidence: `negative_light_physically_based` on WARP

Both captures are Windows, CPU AMD EPYC 7763, adapter Microsoft Basic Render Driver, Dx12, vendor `0x1414`, device `0x008c`. The case description says physically based upload clamps each channel of color times intensity to be non-negative, and the capture must match the scene without that light. The loose gate is `visual_tolerance(0.78, 56, 0.22, 18, 24, 0.06)` with combine `or`. The main frame is `actual.png`, 256×256. These two SHAs still spawned `positive_only.png`. Master `6f0c7aa6` drops that second spawn.

### Run [41145831835](https://github.com/DoubleStyxx/Renderide/actions/runs/41145831835), `4bd5eef003154befb4deb6aa1becacc0b67dc25d`

`report.json` for the main frame: SSIM `0.8344220382053291`, failing-pixel fraction `0.150054931640625`, `max_abs_diff_observed` 250. Those round to the cited 0.834, 0.150, and 250. `evaluation.passed` is true and `combine` is `or` (SSIM threshold 0.78, max failing fraction 0.22). The case `passed` field is false because `positive_only.png` timed out at 300s on a flat frame (`ready=780 pending=455 frames=1235`).

The cited colors are individual pixels, and one of each pair sits on the cited golden:

- `(150, 72)`, upper-right of the sphere (non-background bbox x 70–185, y 59–161): actual `(177, 0, 159)`, golden `(223, 204, 177)`. G is 0.
- `(86, 144)`, lower-left of the sphere: actual `(0, 102, 0)`, golden `(129, 118, 102)`.

`(177, 0, 159)` occurs 17 times, all in x 147–176, y 63–102. `(0, 102, 0)` occurs 19 times, all in x 74–86, y 133–152. Every non-background pixel in the sphere's upper-right half has G exactly 0 (2272 pixels).

**Flag.** Those triples are not the region means. The upper-right half's non-background mean is `(144.84, 0.00, 129.97)` against golden `(213.04, 195.22, 169.59)`. The lower-left half's mean is `(0.03, 69.05, 0.02)` against golden `(155.50, 142.14, 122.87)`.

Ubuntu llvmpipe on the same run (adapter `llvmpipe (LLVM 20.1.2, 256 bits)`, Vulkan CPU): SSIM `0.9996882096165792`, failing fraction `0.0`, `max_abs_diff_observed` 1. **Flag:** that is short of an exact match.

### Run [41150822333](https://github.com/DoubleStyxx/Renderide/actions/runs/41150822333), `6ae283f1ecf72e19a662fe764994a5bbd428c202`

`report.json` for the main frame: SSIM `0.8681488598786302`, failing-pixel fraction `0.158294677734375`, `max_abs_diff_observed` 206. Those round to the cited 0.868, 0.158, and 206. `evaluation.passed` is true under the same OR gate. The case `passed` field is false because `positive_only.png` timed out at 300s (`ready=791 pending=442 frames=1233`) and that PNG is 256×256 black.

**Flag.** None of `(177, 0, 159)`, `(0, 102, 0)`, `(223, 204, 177)`, or `(129, 118, 102)` occurs in this `actual.png`. G is not 0 on the sphere. The upper-right non-background mean is `(208.07, 49.38, 45.65)` against golden `(213.04, 195.22, 169.59)`. The lower-left mean is `(206.58, 41.06, 37.82)` against golden `(155.50, 142.14, 122.87)`. The golden file matches the earlier run byte for byte. This frame is a warm tint, with all three channels positive, and it still clears the OR gate.

Ubuntu llvmpipe on this run: SSIM `1.0`, failing fraction `0.0`, `max_abs_diff_observed` 0. That match is exact.

### Pack values

The Windows logs for both physically based captures contain `negative direct lights active: signed scene-color HDR will be used while negative lights are packed`. That line is `signed_scene_color_required`, set from `light_has_negative_contribution` on the resolved scene lights before `packed_light_radiance`. The artifacts do not print the packed color or intensity. Kairo's statement that the pack values were finite and positive is recorded as reported. These logs neither confirm nor contradict the stored floats.

### The other three cases on the same jobs

| Case | `4bd5eef0` / 41145831835 | `6ae283f1` / 41150822333 |
| --- | --- | --- |
| `pbs_lit_material_matrix` | Accepted, then `expected failure (WARP, #212)`. SSIM 0.808, fail fraction 0.137, max abs diff 255. Evaluation failed on the region gates. Upper-right band (x 160, y 77, 160×73) mean is `(22.91, 0, 0)` against golden `(40.67, 46.32, 63.48)`. G and B of that mean are 0. | 300s flat timeout, pending 0, `ready=785 pending=445 frames=1230`. `actual.png` is 320×320 black. No image metrics. |
| `negative_light_parity` | 300s flat timeout, pending 0, `ready=804 pending=428 frames=1232`. Stamped `quarantined:#216`. | Case passed. SSIM 0.826, fail fraction 0.148, max abs diff 228. OR gate passed. 4824 of 4932 non-background pixels have G = 0. The upper-half non-background mean is `(31.52, 0.42, 0.37)`. |
| `negative_color_physically_based` | 300s flat timeout, pending 0, `ready=800 pending=430 frames=1230`. Stamped `quarantined:#216`. | Case passed (`error` null). SSIM 0.855, fail fraction 0.149, max abs diff 217. OR gate passed. Upper-half non-background mean `(50.36, 47.99, 44.32)`, lower-left `(16.33, 15.24, 13.61)`. The saved `positive_only.png` is 256×256 black. |

`6ae283f1` also passed the unlit cases in that job at SSIM about 0.999. The broken captures are the lit ones. Both physically based WARP logs say `Using static DXC for shader compilation`, scene color `Rgba16Float`, surface `Rgba8UnormSrgb`, post-processing off.

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

**Open for the all-black frames.** The shader returns black when the cluster range count is 0. `pbs_lit` on run 41150822333 is 320×320 black, and both physically based `positive_only.png` captures are 256×256 black. CPU `light_count` on the canary was 2, so the bug would be the cluster build or the bind the shader reads, not the light authoring. The channel-zero and warm-tint frames in the evidence section are a different picture: those pixels are not `vec3(0)`. Hypothesis 8 covers them.

**Experiment.** On a `pbs_lit` frame, log the clustered-light pass result for that view: light count written, cluster ranges that are non-empty, and whether the pass ran. Compare one Windows WARP log with one Linux llvmpipe log from the same commit. No WGSL change in this step.

- Count is 0 on WARP and non-zero on Linux: the cluster pass or its upload is the bug. Bridge fixes that path.
- Count is non-zero on both, and WARP stays black: hand the bind and the `direct_metallic_clustered` inputs to Shader.

Owner: Bridge for the log and the WARP-vs-Linux compare. Shader only if the count is non-zero. ETA: log on the canary by Nov 3; read the samples by Nov 5; Shader follow-up, if needed, by Nov 6. Findings note Nov 7.

### 6. All four draws bind material offset 0 and share one slot

**Open.** Every draw logged `/e0`, and each material upload also said `offset=0`, while the four colors differed. That is either four buffers at offset 0 or one slot overwritten. Black pixels hide which.

**Experiment.** Log a hash of the bound uniform bytes and whether the four group-1 binds are distinct buffers. If they alias, Bridge fixes the arena offset. If they are distinct and match the uploaded colors, this hypothesis is closed.

Owner: Bridge. ETA: Nov 4, beside the cluster log.

### 7. #216 and #212 are one shading bug with two harness exits

**Supported for the flat-versus-accepted split.** A pure-black sphere frame on the production clear is flat (`black_spheres_on_the_production_clear_are_flat`), so settle rejects it and the timeout is #216. The same pixels on the magenta clear are not flat, so the frame is accepted and the image check is #212. That is runs 41084635804, 41089310635, 41093531109, and the merged-tree run 41103575428. #219 (`41099685993`, production clear, 8703 non-black pixels) was not flat and took #212. The `4bd5eef0` and `6ae283f1` main frames are the third exit: the OR gate passes and the pixels are still wrong. `6f0c7aa6` quarantines that finished comparison as `WARP lit shading untrusted`. Those two runs predate that stamp, so their reports are not marked quarantined. Owner: Bridge. ETA: routing is done Oct 30. The shade itself is hypotheses 5 and 8.

### 8. WARP lit shading zeroes whole channels

**Lead for the colored wrong frames.** On `4bd5eef0` the physically based sphere's upper-right half has G exactly 0, and one pixel is `(177, 0, 159)` against golden `(223, 204, 177)`. The lower-left sample `(0, 102, 0)` is green only. The same run's `pbs_lit` upper-right band mean is `(22.91, 0, 0)`. On `6ae283f1` that zeroing is gone and the physically based sphere is a warm tint around `(208, 49, 46)` that still passes the OR gate (SSIM 0.868, fail fraction 0.158, max abs diff 206). Lavapipe on `6ae283f1` matches the golden exactly (max abs diff 0). Both Windows captures compiled with static DXC into an `Rgba16Float` scene color and an `Rgba8UnormSrgb` surface.

Candidate causes, each with its own experiment. Findings note Nov 7.

**8a. DXC codegen.** The WARP logs already say `Using static DXC for shader compilation`.

**Experiment.** Rebuild the same lit shader with FXC if the Windows toolchain can select it, and capture `negative_light_physically_based` once under each compiler. A G=0 frame only under DXC confirms this cause. If FXC cannot be selected, dump the DXIL for the lit fragment from a channel-zero run and compare it with the SPIR-V from the llvmpipe job of the same commit.

Owner: Bridge. ETA: Nov 5 for the two captures, or the DXIL dump if FXC is unavailable. Shader reads the dump if the bytes disagree.

**8b. Swizzle or packing.** `packed_light_radiance` is the CPU pack. The artifacts do not print its floats.

**Experiment.** Log the packed `GpuLight` color and intensity for this case on one WARP run and one llvmpipe run. Finite, positive, and equal on both closes the CPU pack. A zero or negative channel on WARP only is the bug, and Bridge fixes the pack. A following round-trip writes those floats into a storage buffer and reads them back on WARP, so a GPU swizzle shows up as a different value than the CPU log.

Owner: Bridge. ETA: Nov 4.

**8c. sRGB conversion.** The measured frames are the 8-bit sRGB surface. Scene color is `Rgba16Float`, and post-processing is off.

**Experiment.** Read the scene-color texture back before the sRGB surface copy, on WARP, for the same frame whose surface has G = 0. Channels present in the float target and missing on the surface put the bug in the copy. Channels already zero in the float target leave this hypothesis closed and point back at 8a or 8b.

Owner: Bridge. ETA: Nov 6.

**8d. The zero is in one lighting term.** This needs a new scene, so it is Shader's.

**Experiment.** A debug scene writes one lighting term at a time into R, then G, then B, and Bridge captures it on WARP and llvmpipe. A term that is already missing G is handed back to Shader as a WGSL fix. A term that is intact, with the composed frame still missing G, closes this and stays with 8a–8c.

Owner: Shader for the scene, Bridge for the two captures. ETA: Nov 6, after 8b's pack log is in.

## Order

1. Bridge: packed-light float log (8b) and the cluster-count log (hypothesis 5), plus the material-bind hash (hypothesis 6). The #212/#216 split is the routing above.
2. Bridge: FXC-versus-DXC or the DXIL dump (8a), then the pre-sRGB scene-color readback (8c).
3. Shader: the per-term debug scene (8d) when the pack log is finite and the cluster count is non-zero on a frame that is still wrong.
4. Gauge: #214 stall, after the Nov 7 color finding.
