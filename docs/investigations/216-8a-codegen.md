# 216 experiment 8a: production `pbsmetallic` lit-fragment codegen

Offline comparison of the production forward fragment, plus the WARP arms that consume it. This file lives on `cursor/warp-216-8a-baseline-4858`, cut from canary `cursor/warp-black-diag-bb54` at `22647eae` (that commit merged master `fbf2cd21`). Nothing here is pushed to the canary.

The composed shader bytes are from canary run [41198150721](https://github.com/DoubleStyxx/Renderide/actions/runs/41198150721) at `a364ae7f`. Linux `warp216-debug-linux` and Windows `warp216-debug-windows-2` wrote the same WGSL, SPIR-V, and HLSL (`sha256` below). `fbf2cd21` only adds `shade_metallic_term_debug` behind `#ifdef _TERM_DEBUG`. Production `pbsmetallic` does not define that keyword, so the lit fragment those cases compile is unchanged.

Full files: `docs/investigations/216-8a-artifacts/`. Tool versions and hashes: `versions.txt` in that directory.

## What the failing cases actually compile

`negative_light_parity`, `negative_light_physically_based`, `negative_color_physically_based`, and `pbs_lit_material_matrix` draw `pbsmetallic_default`, variant bits `0x0`. The only naga-oil shader def on that target is the absence of `MULTIVIEW`. Keyword bits (`METALLICGLOSSMAP`, `NORMALMAP`, `EMISSION`, and the rest) are runtime tests of `_RenderideVariantBits`, not preprocessor defines, so the compiled fragment still contains every keyword branch. The fragment entry is `fs_forward_base`. It calls `shade_metallic_clustered`, which calls `direct_metallic_clustered`.

`physically_based` changes the packed light, not this shader. Parity leaves intensity `-1.45` (`0xbfb9999a`) in the storage buffer. The physically-based cases store `0` for that intensity or for the negative color. The same DXIL runs for all four.

The debug group that matched on WARP (`warp_216_debug`, PR #237) does not compile this fragment. It uses `warp_216_term_debug.wgsl` with `_TERM_DEBUG`, a constant albedo, and no `_MainTex` sample.

## Compiler path wgpu 30 uses on DX12

Crates: `wgpu 30.0.1`, `wgpu-hal 30.0.1`, `naga 30.0.1`. `crates/renderide/Cargo.toml` enables `wgpu` feature `static-dxc`, which links `mach-dxcompiler-rs 0.1.6+2026.09.16-48d5a66.1`.

`Dx12Compiler` in wgpu-types 30.0.1 is `Fxc`, `DynamicDxc { dxc_path }`, `StaticDxc`, and `Auto` (the default). `InstanceDescriptor::with_env` is what `build_wgpu_instance` calls. That applies `WGPU_DX12_COMPILER`.

| `WGPU_DX12_COMPILER` | Compiler |
| --- | --- |
| unset, or `auto` | `Auto`. With `static-dxc` linked, that is static DXC. The WARP log line is `Using static DXC for shader compilation`. |
| `fxc` | FXC (`d3dcompiler_47.dll`, `D3DCompile`). Log line: `Using FXC for shader compilation`. |
| `staticdxc` | Static DXC, even if the feature probe would have fallen through. |
| `dxc` or `dynamicdxc` | `dxcompiler.dll` on the DLL search path. |

Renderide does not set `backend_options.dx12.shader_compiler` itself. The CI runs are static DXC because the feature is on and the env var is unset. FXC is selectable with no code change: export `WGPU_DX12_COMPILER=fxc` on the renderer process. The harness inherits the job environment.

Static DXC's advertised maximum model is `DxcShaderModel::V6_7` (`CompilerContainer::new_static_dxc`). The device model is `min(adapter CheckFeatureSupport, that cap)`. FXC's `max_shader_model()` is `None`, and the adapter path then forces naga shader model **5.1**. An adapter that cannot do DXIL is not exposed when a DXC compiler is selected. WARP driver `10.0.26100.33648` (Windows 11 24H2) supports shader model 6.7, so the expected DXC profile is **`ps_6_7`**. This offline pass did not call `CheckFeatureSupport`. The Windows dump's `lit-production.dxil` sha256 is the check: it matches `fs_forward_base.ps_6_7.dxil` below if the device model is 6.7.

`instance_flags=InstanceFlags(0x0)` on the WARP log, so `InstanceFlags::DEBUG` is off.

### Flags DXC actually gets

From `wgpu-hal-30.0.1/src/dx12/shader_compilation.rs` `compile_dxc`:

- `-E <entry>`
- `-T <stage>_<model>` (`ps_6_7` for this fragment when the device model is 6.7)
- `-HV 2018` (comment in wgpu: naga does not support HLSL 2021)
- `-no-warnings`
- `DXC_ARG_ENABLE_STRICTNESS`, which is `-Ges` (legacy strict syntax, not IEEE strictness)
- `-enable-16bit-types` only if `SHADER_F16` or `SHADER_I16` is enabled. Renderide does not request those for this device.
- `-Zi` and `DXC_ARG_SKIP_OPTIMIZATIONS` (`-Od`) only when `InstanceFlags::DEBUG` is set and the Intel debug-info workaround is off. They are off on these runs.
- No `-O0`/`-O1`/`-O2`/`-O3`. No `-Gis` (force IEEE strictness). No `-Gfa`. No denorm mode (`-denorm`; DXC's default is `any`).

Compiling the dumped HLSL with the matching Linux `dxc` (`--version` prints `1.7`; the binary's strings identify `3.7-v1.4.0.2274-1812-g84da60c6c-dirty`, release tag `2026.09.16+48d5a66.1`):

| Flags | sha256 of the DXIL container |
| --- | --- |
| `-T ps_6_7 -E fs_forward_base -HV 2018 -Ges -no-warnings` (wgpu's set) | `f48fa215d4733547988b30f7ee6fdf80cc21e66648e01a468dae2290d906c1dc` |
| same plus `-O3` | identical to the row above |
| same plus `-Od` | `1fccf27a51ec04dc2e0a29fe47b815567f9957a1e98004f48ee6582d57fa7b57` |
| same plus `-Gis` | `a992a860e7b164b6e75055f39e0b708c5c0d4d527fa7feba0811ede85e34b418` |
| `-T ps_6_6` otherwise the same | `28c68493201760da7e6c134f32445af39b45b453f65685bff2823a8ed6319644` |
| `-T ps_6_0`, `ps_5_1`, or `ps_5_0` | `ps_5_*` is promoted to 6.0. Hash `7b1fff5a4c63ce2df3aa954704586b401ae1fa402c14974cb55f8d6829a40738` |

wgpu's omitted `-O` flag **is** `-O3` for this compiler. `-Od` still emits `fast` math (11065 `fast` instructions versus 8334 at `-O3`). `-Gis` is what clears `fast` (0 remaining) while keeping the same opcode mix.

### Flags FXC gets, when it is selected

`compile_fxc` calls `D3DCompile` with `D3DCOMPILE_ENABLE_STRICTNESS`. `DEBUG` adds `D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION`. The profile string is `ps_5_1` because the shader model is 5.1. There is no `-HV` and no DXIL. This Linux `dxc` cannot emit that DXBC: `-T ps_5_1` is rewritten to shader model 6.0. The FXC arm has to run on Windows.

### Naga options versus the canary probe

The canary probe (`RENDERIDE_LIT_SHADER_PROBE`) writes HLSL with `naga::back::hlsl::Options::default()`:

- shader model **5.1** (wgpu uses the device model, expected 6.7)
- `fake_missing_bindings: true` (wgpu sets `false` and fills `binding_map`)
- `force_loop_bounding: true`, `restrict_indexing: true`, `zero_initialize_workgroup_memory: true` (same as wgpu's pipeline-layout defaults)
- `PipelineOptions::default()`, so every entry point is written. wgpu sets `entry_point` to the one stage it is compiling and runs `process_overrides` on that stage.

Naga's HLSL math emission does not change between 5.1 and 6.7 for this module. The only shader-model branches in the writer are subgroup builtins, per-vertex fragment inputs, mesh shaders, and `dot4add_*8packed`. None of those appear in `fs_forward_base`. Storage loads of `f32` vectors stay `Load` / `Load2` / `Load3` / `Load4` on `ByteAddressBuffer` (`naga-30.0.1/src/back/hlsl/storage.rs`).

Vulkan SPIR-V from wgpu-hal uses `force_loop_bounding: true`, no `FPFastMathMode` decoration, and `GLSL.std.450` `FMin` / `FMax` / `FClamp` / `Pow` / `Sqrt` / `InverseSqrt`. The probe's SPIR-V uses naga's default SPIR-V options, which also bound loops and do not emit fast-math decorations. Bounds-check policy can differ from lavapipe's robust-access settings. The light-index path is an address calculation plus a compare against `light_count`, not a clamped array index, on both backends.

## Opcode diff that matters

Counts are for `fs_forward_base` compiled at `ps_6_7` with wgpu's flags.

| Op | DXIL (`-O3`, which is the default) | SPIR-V (naga) |
| --- | --- | --- |
| `FMax` | 647 | `GLSL.std.450 FMax` |
| `FMin` | 465 | `FMin` |
| `Saturate` | 6 | no saturate op; `saturate` in naga becomes `FClamp` |
| `Sqrt` / `Rsqrt` | 43 / 450 | `Sqrt` / `InverseSqrt` |
| `Exp` / `Log` | 28 / 30 | `Pow` appears **3** times, not as `Exp`/`Log` |
| `fast` float instructions | 8334 | 0 `FPFastMathMode` decorations |
| `rawBufferLoad` mask 7 (`Load3`) | 6 | structured `OpAccessChain` + `OpLoad` of the `vec3` member |
| `rawBufferLoad` mask 15 / 1 | 182 / 45 | same structured loads |

### Clustered loop

HLSL after naga, including the loop bound wgpu injects (`force_loop_bounding`, countdown from `u32::MAX`, [wgpu#7319](https://github.com/gfx-rs/wgpu/issues/7319)):

```hlsl
uint count_3_ = _e29.y;
if ((count_3_ == 0u)) {
    return (0.0).xxx;
}
uint2 loop_bound_7 = uint2(4294967295u, 4294967295u);
bool loop_init_7 = true;
while(true) {
    if (all(loop_bound_7 == uint2(0u, 0u))) { break; }
    loop_bound_7 -= uint2(loop_bound_7.y == 0u, 1u);
    // i = 0 on the first iteration, then i + 1
    if ((_e30_3 < count_3_)) {
    } else {
        break;
    }
    const uint _e43 = cluster_light_index_from_rangeX(/* range, i */);
    if ((_e43 >= frame.light_count)) {
        continue;
    }
    // full GpuLight load, then direct_radiance_metallic_prepared
}
```

Index loads:

```hlsl
uint2 cluster_light_range_at(uint cluster_id) {
    uint2 _e3_2 = asuint(cluster_light_ranges.Load2(cluster_id*8));
    return _e3_2;
}
uint cluster_light_index_from_range(uint2 range, uint slot) {
    uint _e6_1 = asuint(cluster_light_indices.Load((range.x + slot)*4));
    return _e6_1;
}
```

There is no clamp on `cluster_id` or `range.x + slot`. SPIR-V is the same address math with `OpAccessChain` into the runtime arrays. The empty-cluster early-out is present on both. Hypothesis 5 already showed the center tile is non-empty, so this early-out is not the black frame. The `u32::MAX` literal is in the HLSL and in the SPIR-V (`OpConstant %uint 4294967295`, composite `v2uint`). It does not appear as that decimal in the `-O3` DXIL listing, so DXC rewrote or deleted the counter. A 2-light loop cannot hit the bound. The HLSL form `uint2(<bool>, 1u)` is the one place the two backends are not the same instruction: SPIR-V uses `OpIEqual` / `OpAll` and an integer subtract (`naga-30.0.1/src/back/spv/block.rs`).

### `N·L`

WGSL is `max(dot(brdf_n, light_dir), 0.0)`, then `if n_dot_l <= 0.0 { return 0; }`. HLSL is `max(dot(...), 0.0)`. DXIL lowers that to `dx.op.binary` `FMax`, not to `saturate`. The six `Saturate` instructions are elsewhere. `n_dot_l` is a scalar multiplied into all three radiance channels, so a bad `FMax` here blacks the whole light, not one channel.

SPIR-V for the sRGB predicate is ordered:

```spirv
%1884 = OpFOrdLessThanEqual %bool %value_1 %SRGB_LINEAR_THRESHOLD
OpSelectionMerge %1885 None
OpBranchConditional %1884 %1886 %1885
%1886 = OpLabel
%1887 = OpFMul %float %value_1 %SRGB_LINEAR_INV_SLOPE
OpReturnValue %1887
; else if value < 1: Pow(normalized, 2.4)
; else: Pow(value, 2.2)
```

DXIL for the same function, after `color * intensity`, inverts the test and uses an unordered compare under `fast`:

```llvm
%3016 = fmul fast float %color_r, %intensity
%3019 = fcmp fast ugt float %3016, 0x3FA4B5DCC0000000   ; > 0.04045, unordered
br i1 %3019, label %nonlinear, label %linear
; linear: fmul fast by 1/12.92
; 0.04045 < x < 1: Log(normalized), fmul by 2.4, Exp
; x >= 1:          Log(x),          fmul by 2.2, Exp
```

`fcmp ugt` is true when either operand is NaN. `OpFOrdLessThanEqual` is false for NaN, and the SPIR-V then also fails `OpFOrdLessThan` against 1 and executes `Pow`. Finite values agree. Negative values (`-1.45 * color`) are `<= 0.04045` on both, so they take the linear multiply and do not enter `pow`. That matches the source. The parity light does not hit `pow` of a negative base unless DXC speculates into the `Log` anyway. The `Log` calls are on the taken-or-not gamma blocks; they are not on the linear-return block.

### Signed intensity

`light_radiance` is `srgb_light_to_linear(light.color * light.intensity)` with no `abs`, `sign`, or `max(..., 0)`. HLSL scalarizes it:

```hlsl
float3 srgb_light_to_linear(float3 color) {
    const float _e2 = srgb_light_channel_to_linear(color.x);
    const float _e4 = srgb_light_channel_to_linear(color.y);
    const float _e6 = srgb_light_channel_to_linear(color.z);
    return float3(_e2, _e4, _e6);
}
```

`signed_light_radiance` multiplies that `float3` by the scalar `attenuation * n_dot_l`. A negative intensity scales all three channels. It does not, by itself, zero one channel. `pbs_lit_material_matrix` has no negative radiance and still lost channels on `4bd5eef0`, so a negative-only `pow` is not the whole bug.

### `Load3` of `GpuLight.color`

The light struct stride in the HLSL is 144. Color is a `float3` at byte 32, intensity at byte 44:

```llvm
%1303 = add i32 %base, 32
%1304 = call %dx.types.ResRet.i32 @dx.op.rawBufferLoad.i32(
    i32 139, %dx.types.Handle %lights, i32 %1303, i32 undef, i8 7, i32 4)
%1308 = bitcast i32 %extract0 to float   ; color.r
%1309 = bitcast i32 %extract1 to float   ; color.g
%1310 = bitcast i32 %extract2 to float   ; color.b
```

Mask `7` is the three-component load. There are six of them in this entry: position, direction, and color, inlined twice (the opaque `shade_metallic_clustered` path and the premultiplied transparent path). SPIR-V loads the `vec3` member through a storage-buffer pointer. No swizzle drops `.y` in the HLSL constructor: `ret.color = arg4` where `arg4` is the `Load3` result.

A WARP bug that returns 0 in element 1 of a mask-7 load would zero G everywhere that pixel is lit by that light. Pixel `(0, 102, 0)` on `4bd5eef0` has G nonzero and R and B zero, so mask-7 is not stuck at "Y is always 0" on that frame. It can still be wrong for some invocations. The debug shader loads the same struct, and its floats matched lavapipe, which is evidence against a unconditional `Load3` failure and for something the extra production code (textures, keyword branches, register pressure, fast-math reassociation) provokes.

### Per-channel texture multiply the debug shader does not do

```hlsl
const float4 _e42 = sample_tex_2d(_MainTex, wrap_bits, _MainTex_sampler, uv, lod_bias);
base_color = mat._Color.xyz * _e42.xyz;
```

`sample_tex_2d` is `SampleBias` after the mirror-once UV fix. Occlusion then uses `_OcclusionMap.y` only, as a scalar. A white default albedo preserves lighting. A sample that returned 0 in one channel would zero that channel of the lit color. The green-only pixel means the albedo G was not globally 0. This multiply is still the largest production-only per-channel op the debug shader never executes.

### NaN, `min`/`max`, denorms, half

- WGSL `min`/`max`: if one operand is NaN, the other is returned. `clamp(NaN, 0, 1)` is therefore 0.
- SPIR-V `FMin`/`FMax`/`FClamp` follow GLSL.std.450 (NaN loses to a number). No fast-math decoration.
- DXIL `FMin`/`FMax` have the same stated NaN rule, but every one of them in this container is fed by `fast` arithmetic (`nnan`, `ninf`, `reassoc`, contract). A NaN that fast-math assumed could not exist is undefined on WARP. `saturate` (6 sites) flushes NaN to 0 in DXIL. Most clamps in this shader are `FMax`+`FMin`, not `saturate`.
- `pow` is `exp(y * log(x))` in DXIL and `OpExtInst Pow` in SPIR-V. `log` of a non-positive base is NaN. The sRGB linear branch avoids that for negatives. Other `pow` sites (`indirect_specular_visibility`'s `pow(no_v + ao, exponent)`, 30 `Log`s total) still lower this way.
- Denorm mode is DXC's default `any`. wgpu does not pass `-denorm preserve`. SPIR-V has no denorm decoration here either.
- No half-precision promotion: `-enable-16bit-types` is absent, and the loads are `f32`.
- `select` is a scalar-or-vector ternary. The directional light is `(dir_len_sq > 1e-16) ? -light_dir / sqrt(dir_len_sq) : float3(0,0,1)`. HLSL evaluates both sides. `sqrt(0)` then a divide is Inf on the untaken side. WGSL `select` also evaluates both operands, and SPIR-V `OpSelect` does too. The scalar condition is not a mis-sized component mask. The variant-bit tests (`and` with `1u << n`) are integer masks on `_RenderideVariantBits`, and variant `0` skips those branches.

## Ranked suspects

1. **DXC fast-math versus precise SPIR-V.** 8334 `fast` ops at the optimization level wgpu actually uses, `pow` lowered to `exp(log)`, and `fcmp fast ugt` where SPIR-V has `OpFOrdLessThanEqual`. `-Gis` removes every `fast` flag and changes the container hash. `-Od` does not. This is the best fit for channel-selective zeros that are not the same channel in every pixel (`(177, 0, 159)` and `(0, 102, 0)` on the same `4bd5eef0` frame) and for a bug the smaller debug shader does not hit. wgpu cannot pass `-Gis`. FXC is the selectable compiler that does not emit this DXIL.

2. **`rawBufferLoad` mask 7 of `GpuLight` `float3`s.** Six sites, color at offset 32, element 1 is G. Real semantic difference from the structured SPIR-V load. Weakened by the green-only pixel and by the debug shader using the same load and matching lavapipe. Still the right A/B if WARP's mask-7 path is only wrong inside this large DXIL.

3. **`_MainTex.xyz` multiplied into albedo.** Production-only. Would zero a channel that the sample returns as 0. Not a global G=0, because some lit pixels are green-only. Worth keeping in mind if the scalar-color arm and the FXC arm both still drop channels.

Not the lead: the cluster-loop bounds and the `light_count` continue (same on both backends, and the tile is non-empty); scalar `max(dot, 0)` for `N·L`; a negative `pow` on the parity intensity (the linear sRGB branch is taken); half-precision; a mis-sized `select` mask on the directional light.

## Proposed A/B rewrites

Not applied on the baseline branch. The scalar-color rewrite is the one the scalar arm compiles.

### 1. Split `GpuLight.color` into three `f32`s

Layout stays byte-identical: `vec3` at offset 32 is 12 bytes, `intensity` stays at 44. Naga emits three mask-1 `Load`s instead of mask-7 `Load3` for color. Position and direction stay `Load3`.

```diff
--- a/crates/renderide/shaders/modules/frame/types.wgsl
+++ b/crates/renderide/shaders/modules/frame/types.wgsl
@@
-    color: vec3<f32>,
+    color_r: f32,
+    color_g: f32,
+    color_b: f32,
     intensity: f32,
```

```diff
--- a/crates/renderide/shaders/modules/lighting/birp.wgsl
+++ b/crates/renderide/shaders/modules/lighting/birp.wgsl
@@
 fn light_radiance(light: ft::GpuLight) -> vec3<f32> {
-    return srgb_light_to_linear(light.color * light.intensity);
+    let color = vec3<f32>(light.color_r, light.color_g, light.color_b);
+    return srgb_light_to_linear(color * light.intensity);
 }
```

The audit in `crates/renderide/tests/shader_module_audit/pbs/lighting.rs` locks the old return line and has to move with it. No Rust struct change: the GPU struct is still three floats then intensity.

### 2. `max(0)` before the Smith GGX square roots

Precise math already keeps the radicand non-negative. Fast reassociation can make it negative, and `sqrt` of that is NaN, per channel, because the terms are scalarized.

```diff
--- a/crates/renderide/shaders/modules/pbs/brdf.wgsl
+++ b/crates/renderide/shaders/modules/pbs/brdf.wgsl
@@
-    let lv = n_dot_l * sqrt((n_dot_v - a2 * n_dot_v) * n_dot_v + a2);
-    let ll = n_dot_v * sqrt((n_dot_l - a2 * n_dot_l) * n_dot_l + a2);
+    let lv = n_dot_l * sqrt(max((n_dot_v - a2 * n_dot_v) * n_dot_v + a2, 0.0));
+    let ll = n_dot_v * sqrt(max((n_dot_l - a2 * n_dot_l) * n_dot_l + a2, 0.0));
```

### 3. Directional `select` without dividing by `sqrt(0)`

```diff
--- a/crates/renderide/shaders/modules/pbs/light_eval.wgsl
+++ b/crates/renderide/shaders/modules/pbs/light_eval.wgsl
@@
-        out.l = select(vec3<f32>(0.0, 0.0, 1.0), -light_dir / sqrt(dir_len_sq), dir_len_sq > 1e-16);
+        let len_sq = max(dir_len_sq, 1e-16);
+        let facing = -light_dir * inverseSqrt(len_sq);
+        out.l = select(vec3<f32>(0.0, 0.0, 1.0), facing, dir_len_sq > 1e-16);
```

`select` still evaluates both sides. The untaken side is now `inverseSqrt(1e-16)` instead of a division by zero. This is the weakest of the three. It does not explain a green-only pixel.

### Alternate compiler, not a WGSL edit

`WGPU_DX12_COMPILER=fxc` on the renderer. Confirmed by the log line `Using FXC for shader compilation`. Profile becomes `ps_5_1`, `D3DCOMPILE_ENABLE_STRICTNESS`, optimizations left on. That is the arm that leaves DXIL fast-math entirely.

## WARP arms

Judge pixels, not SSIM. The harness logs `diag-8a png case=… flat=… non_bg=… g0=… r0=… b0=…` when `RENDERIDE_LIT_SHADER_PROBE=1`. Non-background pixels are those that differ from pixel `(0,0)` (the clear). Baseline on `4bd5eef0` parity was 4824 of 4932 non-background pixels with G=0. Also record `actual.scene_color.json` (enabled for these three cases while the probe is on), report-v2 `region_stats`, the flat-versus-accepted split, and settle time from `capture-stats.json`. Three windows jobs are three samples. Do not compare SSIM: the debug group sat at 0.952–0.958 on WARP even when the float means matched.

| Arm | Branch | Compiler / shader | What it isolates |
| --- | --- | --- | --- |
| baseline | `cursor/warp-216-8a-baseline-4858` | static DXC, production fragment | G=0 counts on this tree |
| fxc | `cursor/warp-216-8a-fxc-4858` | `WGPU_DX12_COMPILER=fxc` | DXIL fast-math versus FXC `ps_5_1` |
| scalar-color | `cursor/warp-216-8a-scalar-color-4858` | static DXC, rewrite 1 | mask-7 color load |

Each branch's copy of `.github/workflows/warp-216-debug.yml` lists only that branch under `on.push.branches`, and also `workflow_dispatch`. The job runs:

```text
renderide-test check-suite --jobs 1 \
  --case negative_light_parity \
  --case negative_light_physically_based \
  --case pbs_lit_material_matrix
```

with `RAYON_NUM_THREADS=1` and `RENDERIDE_LIT_SHADER_PROBE=1`. Windows also puts the matching `dxc.exe` (`2026.09.16+48d5a66.1`) on `PATH` so the probe writes `lit-production.dxil` and `lit-production.dxil.ll`. Artifacts: `warp216-debug-<label>`.

Run ids, per-arm G=0 counts, float means, and the flat split are filled in after those runs finish. Rewrite 2 (Smith `max(0)` before `sqrt`) and rewrite 3 (directional `inverseSqrt`) are not compiled on a WARP branch yet. They are the follow-ups if FXC still drops channels (then the bug is not DXIL-specific) or if scalar-color does not move G=0 (then mask-7 is not the load that matters).

## What the WARP capture still needs

- The three samples per arm above, including flat versus non-flat and settle time. `4bd5eef0` / run 41145831835 is the G=0 baseline. `6ae283f1` / run 41150822333 was a different commit and a warm tint, not a second sample of that binary.
- `diag-8a dxc status=0` and the sha256 of `lit-production.dxil` next to `f48fa215…` (ps_6_7) or `28c68493…` (ps_6_6). A mismatch means the device shader model was not the one assumed here.
- The FXC log line, so a green FXC frame is not actually static DXC.
- `actual.scene_color.json` quadrant means (NaN / negative / mean per channel) beside the PNG G=0 count. The PNG count is after the sRGB encode. The float sidecar is the scene color.
- Do not treat SSIM as the result.
