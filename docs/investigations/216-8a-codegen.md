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

The runs below demote suspect 2. The scalar-color arm still goes flat black on eight of nine samples, and its one lit frame is a sign change in R rather than a stuck G. Suspect 1 stays, with a sharper mechanism from the FXC log: a gradient inside the dynamic light loop. DXC compiles that loop; FXC will not; WARP's execution of the DXC loop is the thing that sometimes zeros a channel and sometimes zeros the whole sphere. The wiped channel is not stable across the three baseline samples.

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

All three runs finished. `diag-8a png` counts non-background pixels against the black clear at `(0,0)`. A flat black frame therefore reports `non_bg=0`. That is the whole image at linear 0, including the sphere, which is what `actual.scene_color.json` shows (every quadrant mean `(0, 0, 0, 1)`, no NaN). The Windows probe's `lit-production.hlsl` sha256 is `490328d6…` and its `lit-production.dxil` sha256 is `f48fa215…`, the same bytes as the offline `ps_6_7` container. That DXIL is the probe's own `dxc -T ps_6_7` of the naga HLSL. It is not the blob the GPU ran: wgpu compiles again inside the device, and the FXC jobs still wrote this DXC container because `dxc.exe` was on `PATH`.

| Arm | Run | sha | Compiler log |
| --- | --- | --- | --- |
| baseline | [41202573717](https://github.com/DoubleStyxx/Renderide/actions/runs/41202573717) | `da1680ea` | `Using static DXC` |
| fxc | [41202575526](https://github.com/DoubleStyxx/Renderide/actions/runs/41202575526) | `e3e8bb2e` | `Using FXC`, then `error X3511` |
| scalar-color | [41202577341](https://github.com/DoubleStyxx/Renderide/actions/runs/41202577341) | `c7358018` | `Using static DXC` |

PNG `g0` / `r0` / `b0` are counts of non-background pixels whose channel is 0. Flat rows are linear-black scene color, so those counts are 0 because there is no non-background pixel. Settle is the flat-exit `settle_ms`. Non-flat rows were accepted by settle and then quarantined `WARP lit shading untrusted` (the OR image gate can still pass; that is not the result).

### Baseline, static DXC

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | Scene-color means (TL, TR, BL, BR), RGB | Settle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| windows-1 | parity | no | 10374 | 0 | 0 | 0 | all quadrants about `(0.115, 0.005, 0.005)` down to `(0.068, 0.002, 0.002)` | accepted |
| windows-1 | physically based | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 139.2 s |
| windows-1 | pbs_lit | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 143.1 s |
| windows-2 | parity | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 102.0 s |
| windows-2 | physically based | no | 8426 | 0 | 0 | 0 | about `(0.002, 0.002, 0.002)` in every quadrant | accepted |
| windows-2 | pbs_lit | no | 10208 | 0 | 0 | 0 | TL `(0.0003, 0.0004, 0.0007)`, TR `(0, 0, 0)`, BL `(0.002, 0.003, 0.006)`, BR `(0.0005, 0.0009, 0.003)` | accepted |
| windows-3 | parity | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 86.9 s |
| windows-3 | physically based | no | 10374 | 0 | 10374 | 10374 | TL `(0, 0.135, 0)`, TR `(0, 0.162, 0)`, BL `(0, 0.081, 0)`, BR `(0, 0.097, 0)` | accepted |
| windows-3 | pbs_lit | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 90.0 s |

Five of nine are flat black. The `4bd5eef0` ratio (4824/4932 pixels with G=0) did not repeat. The channel that dies moved: windows-3's physically based frame has R and B exactly 0 in the linear means and G alive, and every one of the 10374 non-background PNG pixels has R=0 and B=0. No NaNs in those means. The other non-flat frames keep all three channels, but the physically based and `pbs_lit` ones are about a thousand times dimmer than windows-1 parity.

### FXC

All nine case-samples (three windows jobs, three cases) are flat linear black, settle 27–35 s. The log is `Using FXC for shader compilation`, then:

```text
error X3511: unable to unroll loop, loop does not appear to terminate in a timely manner (41 iterations)
error X3511: forced to unroll loop, but unrolling failed.
```

preceded by `warning X3570: gradient instruction used in a loop with varying iteration`. The pipeline `embedded_raster_material__pbsmetallic_default__forward__…__fs_forward_base` is invalid, so nothing is shaded. The gradient is identified in the next section. FXC tries to unroll the dynamic light loop because of it, and naga's `u32::MAX` countdown does not look like a timely bound. This arm does not compare shading. It shows FXC cannot build this fragment.

### Scalar color, static DXC

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | Scene-color RGB | Settle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| windows-1 | parity | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 103.8 s |
| windows-1 | physically based | no | 10374 | 0 | 4093 | 0 | TL `(-0.028, 0.004, 0.004)`, TR `(0.037, 0.004, 0.004)`, BL `(-0.018, 0.002, 0.002)`, BR `(0.024, 0.002, 0.002)` | accepted |
| windows-1 | pbs_lit | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 125.2 s |
| windows-2 | all three | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 86.9 s, 85.8 s, 87.6 s |
| windows-3 | all three | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 101.6 s, 100.1 s, 106.1 s |

Eight of nine are flat black. The one lit frame is not the green-only wipe. Its linear R changes sign across the sphere (negative on the left, positive on the right) while G and B stay small and positive, and 4093 of 10374 PNG pixels have R=0 because negative linear R encodes as 0. No NaNs. Splitting the color `Load3` did not remove the black frames and did not reproduce a stable channel. It is not the fix.

Rewrite 2 (Smith `max(0)` before `sqrt`) and rewrite 3 (directional `inverseSqrt`) were not run. The FXC failure says the next compile experiment is the gradient inside the dynamic light loop plus the `u32::MAX` bound, not another color-load shape. A WARP A/B that hoists the gradient out of that loop, or that gives the loop a real small bound FXC can unroll, would be the first arm on which FXC and DXC can both shade. `-Gis` is still not selectable in wgpu 30.

## Gradient inside the clustered light loop

`fs_forward_base` calls `shade_metallic_clustered`, which calls `direct_metallic_clustered`. That function's loop is dynamic (`i < count`, `count` loaded from the cluster buffer). Each iteration calls `direct_radiance_metallic_prepared` or `diffuse_only_metallic`, both of which call `eval_light`. `eval_light` always calls `cookies::multiplier` and `shadows::visibility` (`shaders/modules/pbs/light_eval.wgsl`).

Two implicit-derivative samples sit on that path. `dpdx`/`dpdy` in `normal_derivative_variance` (`shaders/modules/pbs/brdf.wgsl`) run once per fragment, before the loop, via `filter_perceptual_roughness`. Material `textureSampleBias` runs in `sample_surface`, also before the loop. Those are not what FXC is unrolling.

The calls inside the loop:

```wgsl
// shaders/modules/lighting/light_cookies.wgsl
fn sample_2d_cookie(light: ft::GpuLight, uv: vec2<f32>) -> f32 {
    return textureSample(
        rg::light_cookie_2d_atlas,
        rg::light_cookie_sampler,
        atlas_cookie_uv(light.cookie_layer, wrap_cookie_uv(uv, light.cookie_reserved)),
    ).r;
}
// and textureSample of light_cookie_point_atlas in point_cookie_multiplier
```

```wgsl
// shaders/modules/lighting/shadows.wgsl
fn sample_shadow_compare(...) -> f32 {
    return textureSampleCompare(
        rg::shadow_atlas,
        rg::shadow_sampler,
        atlas_uv(shadow_view, local_uv),
        layer,
        compare_depth
    );
}
```

`visibility` also has its own `for (i < shadow_view_count)` around `sample_shadow_compare`, and soft shadows add a fixed 3×3 of the same compare. The outer light loop is the one FXC cannot unroll: the HLSL is `while(true)` plus the `uint2(4294967295u, 4294967295u)` countdown from `force_loop_bounding`.

HLSL from the production fragment (naga, before this arm):

```hlsl
float4 _e8_ = light_cookie_2d_atlas.Sample(light_cookie_sampler, _e5);
float _e7_1 = shadow_atlas.SampleCmp(shadow_sampler, float3(_e4, layer), compare_depth);
```

DXIL (`ps_6_7`, `-O3`), implicit derivatives, not `SampleGrad` / `SampleCmpLevel`:

```llvm
%1449 = call %dx.types.ResRet.f32 @dx.op.sample.f32(...)  ; Sample(...)
%2500 = call %dx.types.ResRet.f32 @dx.op.sampleCmp.f32(...) ; SampleCmp(...)
```

`ddx`/`ddy` of the shading normal are separate `DerivCoarseX`/`DerivCoarseY` calls and are not inside this loop.

These three cases never take the sample. `cookie_texture_asset_id` is `-1` and `shadow_type` is `None` (`light_state` in `scene_state.rs`), so `multiplier` returns 1 and `has_shadow_views` returns before `textureSampleCompare`. The opcodes are still in the loop, which is enough for FXC.

### Arm A

`textureSample` on both cookie atlases is now `textureSampleLevel(..., 0.0)`. `textureSampleCompare` is now `textureSampleCompareLevel`. In naga 30 that builtin takes no level argument. The level is fixed at zero. A trailing `0.0` is parsed as an `i32` offset, which does not compile. WGSL has no `textureSampleCompareGrad`.

LOD 0 is not the implicit-derivative LOD when a cookie or a shadow map is actually sampled. On these three cases the sample is not executed, so the fragment result should match the baseline on lavapipe.

### Arm B, not run

A WGSL `for (i < 32u)` does not reach FXC as a 32-iteration loop. wgpu 30 sets `force_loop_bounding: true`, and naga rewrites every loop into `while(true)` with a countdown from `u32::MAX`. That is the loop `X3511` refuses. Bounding the light count in WGSL does not remove it, so arm B was not built.

### Runs

| Arm | Compiler | Run | sha |
| --- | --- | --- | --- |
| baseline resample | static DXC | [41205727909](https://github.com/DoubleStyxx/Renderide/actions/runs/41205727909) | `b660d424` |
| A explicit LOD | static DXC and FXC | [41205954817](https://github.com/DoubleStyxx/Renderide/actions/runs/41205954817) | `360331b1` |

Both runs finished. The earlier push `16dec2f1` ([41205754209](https://github.com/DoubleStyxx/Renderide/actions/runs/41205754209)) still passes a level argument to `textureSampleCompareLevel` and fails shader compose. Ignore it.

Lavapipe PNGs for arm A are byte-identical to the baseline resample (`byte_diff=0` on all three cases, 262144 bytes for the 256² frames and 409600 for the 320² frame). All three Linux reports are `status=pass`. The explicit LOD does not change these scenes, which never execute the cookie or shadow sample.

### Baseline resample, static DXC, run 41205727909

Seven of nine are flat linear black. Settle on those is 104–193 s.

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | Linear RGB means | Settle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| windows-1 | all three | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 131.1 s, 110.1 s, 130.9 s |
| windows-2 | parity, physically based | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 104.9 s, 108.3 s |
| windows-2 | pbs_lit | no | 8622 | 8622 | 8622 | 0 | TL `(0, 0, 0.109)`, TR `(0, 0, 0.168)`, BL and BR `(0, 0, 0)`. No NaN, no negative. | accepted |
| windows-3 | parity, physically based | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 160.5 s, 193.3 s |
| windows-3 | pbs_lit | no | 12273 | 4242 | 3584 | 3561 | mixed, including negatives (16909 negative channel-samples). Not one stuck channel. | accepted |

Together with the first baseline (5/9 flat, one green-only frame), the static-DXC failure is still a coin flip between a black sphere and a frame that keeps only some channels. No NaNs in the quadrant means.

### Arm A, static DXC, run 41205954817

Six of nine are flat linear black.

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | Linear RGB | NaN / neg | Settle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dxc-1 | all three | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 0 / 0 | 85.4 s, 86.0 s, 86.8 s |
| dxc-2 | parity | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 0 / 0 | 63.4 s |
| dxc-2 | physically based | no | 3354 | 0 | 0 | 0 | about `(-0.045, -0.042, -0.036)` and `(0.009, 0.009, 0.007)` | 0 / 15216 | accepted |
| dxc-2 | pbs_lit | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 0 / 0 | 63.6 s |
| dxc-3 | parity | yes | 0 | 0 | 0 | 0 | `(0, 0, 0)` | 0 / 0 | 70.3 s |
| dxc-3 | physically based | no | 108 | 0 | 0 | 0 | top half about `(-0.057, -0.052, -0.045)`, bottom `(0, 0, 0)` | 0 / 14472 | accepted |
| dxc-3 | pbs_lit | no | 8661 | 1 | 7 | 0 | includes `(-0.018, 0.026, 0.196)` and `(0.001, 0.002, 0.007)` | 0 / 17523 | accepted |

The PNG `g0=0` on the non-flat rows is not a healthy frame. The linear values are negative, and sRGB encode turns those into 0. Explicit LOD did not stop the black frames or the nonsense signs.

### Arm A, FXC, same run

All three Windows jobs failed (`windows-fxc-1`, `windows-fxc-2`, `windows-fxc-3`). `windows-fxc-1` logs `Using FXC for shader compilation`, then a 10 s main-thread watchdog hang while naga's HLSL is still being logged, then `FATAL: Windows exception (code=0xC0000374)` (heap corruption) about 59 s in. There is no `X3511` in that log. The PNGs are linear black because the renderer died during compile (`asset-ack-timeout`, renderer exited). This is not a shaded FXC frame. Removing the implicit samples did not produce a usable FXC pipeline on WARP in these three samples.

### Production fix

Not indicated. Arm A is behaviour-preserving on lavapipe and is not a WARP fix. Arm B cannot reach FXC as a bounded loop while wgpu injects the `u32::MAX` countdown. The diff that was run, and should not be treated as the fix, is:

```diff
--- a/crates/renderide/shaders/modules/lighting/light_cookies.wgsl
+++ b/crates/renderide/shaders/modules/lighting/light_cookies.wgsl
@@
-    return textureSample(
+    return textureSampleLevel(
         rg::light_cookie_2d_atlas,
         rg::light_cookie_sampler,
         atlas_cookie_uv(light.cookie_layer, wrap_cookie_uv(uv, light.cookie_reserved)),
+        0.0,
     ).r;
```

The point-cookie `textureSample` becomes `textureSampleLevel(..., 0.0)` the same way. The shadow compare becomes `textureSampleCompareLevel` with no extra level argument (naga 30 forces LOD 0).

```diff
--- a/crates/renderide/shaders/modules/lighting/shadows.wgsl
+++ b/crates/renderide/shaders/modules/lighting/shadows.wgsl
@@
-    return textureSampleCompare(
+    return textureSampleCompareLevel(
         rg::shadow_atlas,
         rg::shadow_sampler,
         atlas_uv(shadow_view, local_uv),
         layer,
-        compare_depth
+        compare_depth,
     );
```

The pipeline DXIL is the `IDxcBlob` from `GetOutput(DXC_OUT_OBJECT)` in `wgpu-hal` `compile_dxc`, stored as `CompiledShader::Dxc` and never written to disk. Capturing it means a debug write of that blob next to the existing HLSL dump inside wgpu-hal, or a D3D12 shader-tracing tool. There is no wgpu 30 device flag that dumps it. Not added here.

## FXC's X3570 is `SampleCmp`, and the debug shader has the same call

Run [41202575526](https://github.com/DoubleStyxx/Renderide/actions/runs/41202575526) (`e3e8bb2e`, `WGPU_DX12_COMPILER=fxc`) logs the production fragment `fs_forward_base`. The HLSL FXC compiled is the naga dump in that job's renderer log (`Naga generated shader for "fs_forward_base"`), not the probe's default-options `lit-production.hlsl` (different line numbers). The diagnostic is only this line, repeated for each unroll attempt:

```text
raster_material_shader(1278,19-205): warning X3570: gradient instruction used in a loop with varying iteration, attempting to unroll the loop
raster_material_shader(1605,5-15): error X3511: unable to unroll loop, loop does not appear to terminate in a timely manner (41 iterations) or unrolled loop is too large
raster_material_shader(2757,5-15): error X3511: forced to unroll loop, but unrolling failed.
```

Line 1278, columns 19–205, is the `SampleCmp` expression inside `sample_shadow_compare`:

```hlsl
float _e7_1 = shadow_atlas.SampleCmp(shadow_sampler, float3(_e4, layer), compare_depth);
```

That is `textureSampleCompare` in `shaders/modules/lighting/shadows.wgsl` (`sample_shadow_compare`). The cookie `Sample` calls are earlier (lines 1019 and 1090) and FXC does not name them. `ddx`/`ddy` of the shading normal are lines 1699–1700, outside these loops.

Line 1605 is `while(true)` in `visibility`, the shadow-view loop, after naga's `uint2(4294967295u, 4294967295u)` countdown. Line 2757 is `while(true)` in `direct_metallic_clustered`, the clustered light loop. FXC tries to unroll both because the inlined `SampleCmp` is a gradient op. The "(41 iterations)" figure is FXC's unroll limit, not a source trip count.

`warp_216_term_debug.wgsl` with `_TERM_DEBUG` calls `shade_metallic_term_debug`. The composed id returns `shade_metallic_clustered`, which calls the same `eval_light` → `cookies::multiplier` / `shadows::visibility`. `dpdx`/`dpdy` stay in `filter_perceptual_roughness`, before the loop. There is no `fwidth`. So the debug composed path contains the same `SampleCmp` in the same dynamic loop. On run [41174993308](https://github.com/DoubleStyxx/Renderide/actions/runs/41174993308) that shader matches llvmpipe within 6.32e-4 on static DXC / WARP, with no channel drop (`216-plan.md`). A gradient in the loop does not by itself break WARP under DXC. Arm A already removed the implicit samples and the black frames stayed (6/9 flat). No further gradient arm.

Arm B stays unbuilt. `force_loop_bounding: true` in wgpu 30 rewrites every loop to `while(true)` plus the `u32::MAX` countdown before FXC sees it, so a WGSL `for (i < 32u)` is not a 32-iteration loop.

Whether FXC also rejects the debug shader is a separate compile. The 8d run used static DXC, not FXC. The rerun branch runs `warp216_negparity_composed` once under `WGPU_DX12_COMPILER=fxc` to record that.

## What production has that the debug composed path does not

Both call `shade_metallic_clustered`. Ranked by what is in the production fragment and not on the debug path:

1. A second copy of the light loop. `fs_forward_base` also calls `shade_metallic_transparent_clustered` when `ALPHAPREMULTIPLY` is set. These cases use variant bits `0`, so the branch is not taken, but the function is still compiled. The debug composed id returns before its own term-split loop; production compiles both lighting entries.
2. Material `textureSampleBias` (`ts::sample_tex_2d`), including unconditional `_MainTex` and `_OcclusionMap` samples, plus parallax, detail, normal, and emission samples under keyword tests, and `discard` in `clip_standard_alpha`. The debug shader builds `MetallicSurface` from `_Color`, `_Metallic`, `_Glossiness`, and the geometric normal. No material texture, no discard. These three cases have no texture fixtures and bits `0`, so a white texel and occlusion `1` match the sampled values.
3. Nine extra `@group(1)` textures and the `a2c=cutout` output `vec4(color, s.alpha)`. Debug writes `retain_globals_additive(vec4(color, 1.0))` and one uniform. Alpha of the white default times `_Color.a` is 1 on these cases, so A2C is a weaker suspect than (1) and (2). Not split into its own arm.

Fast-math is independent of that diff. The probe DXIL for this fragment (`ps_6_7`, no `-Gis`) contains 8334 `fast` ops (`fmul fast` 3217, `fadd fast` 1415, `fcmp fast` 1240). `-Od` alone leaves them. wgpu 30's `compile_dxc` passes `-HV 2018`, `-Ges` (`DXC_ARG_ENABLE_STRICTNESS`), and `-no-warnings`. It does not pass `-Gis`.

## Arms after the counter-evidence

| Arm | Branch | Shader / compiler | Why |
| --- | --- | --- | --- |
| baseline rerun | `cursor/warp-216-8a-dxc-rerun-4858` | production fragment, static DXC, 3 samples | Flat rate against the 5/9 baseline. One FXC job compiles the debug composed case. |
| no-premul | `cursor/warp-216-8a-no-premul-4858` | drop `shade_metallic_transparent_clustered` | Bisect (1). Lavapipe should be byte-identical at bits 0. |
| untextured | `cursor/warp-216-8a-untextured-4858` | `sample_surface` uses uniform color, gloss, geometric normal; no `textureSampleBias`, no `discard` | Bisect (2). Lavapipe should match these three cases. |
| gis | `cursor/warp-216-8a-gis-4858` | production fragment, patched `wgpu-hal` `compile_dxc` adds `-Gis` and `-Od` | Fast-math off. Sub-branch only. |

Gradient hoist and the bounded loop are not in this set. FXC is not re-run on the bisection or `-Gis` arms: the production pipeline stays invalid there for the same `SampleCmp` (`-Gis` is a DXC flag). SSIM is not used.

## Results

No job failed for a missing runner or a cache miss. The FXC job and the three `-Gis` Windows jobs failed because the shader or the frame failed. They were not rerun.

Lavapipe `actual.scene_color.rgba16f` is byte-identical across the baseline rerun, no-premul, untextured, and `-Gis` (`byte_diff=0` on all three cases). The 8-bit PNGs differ by at most 1 code on a few hundred pixels between the rerun and the other three; the linear buffers do not. no-premul and untextured are behaviour-preserving on these cases. WARP linear buffers are not byte-identical to lavapipe. On the two arms that shade, the quadrant means match lavapipe to about 1e-4, and parity's negative count is 1787 on both.

`g0`/`r0`/`b0` on a healthy parity frame are the lavapipe counts (24 / 0 / 68). Those zeros are dark pixels, not a channel wipe. A wipe is a non-flat frame whose linear mean is 0 in a channel that lavapipe keeps, or a flat frame.

### Baseline rerun, static DXC, run 41208228557 (`06a88b09`)

Seven of nine are flat linear black. No NaNs. This rerun did not draw a G-only wipe. The two non-flat frames are not the lavapipe image: one is about a thousand times too dim, and one has a large negative B.

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | NaN / neg | Settle | Linear RGB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dxc-1 | parity | yes | 0 | 0 | 0 | 0 | 0 / 0 | 105.3 s | `(0,0,0)` |
| dxc-1 | physically based | yes | 0 | 0 | 0 | 0 | 0 / 0 | 106.2 s | `(0,0,0)` |
| dxc-1 | pbs_lit | no | 12884 | 1 | 0 | 12807 | 0 / 12804 | accepted | TL `(0.002, 0.002, -0.068)`, TR `(0,0,0)`, BL `(0.005, 0.005, -0.253)`, BR `(0.009, 0.024, -1.098)` |
| dxc-2 | parity | yes | 0 | 0 | 0 | 0 | 0 / 0 | 84.8 s | `(0,0,0)` |
| dxc-2 | physically based | yes | 0 | 0 | 0 | 0 | 0 / 0 | 83.6 s | `(0,0,0)` |
| dxc-2 | pbs_lit | no | 10208 | 0 | 0 | 0 | 0 / 0 | accepted | TL `(0.0003, 0.0004, 0.0007)`, TR `(0,0,0)`, BL `(0.002, 0.003, 0.006)`, BR `(0.0005, 0.0009, 0.003)` |
| dxc-3 | parity | yes | 0 | 0 | 0 | 0 | 0 / 0 | 66.6 s | `(0,0,0)` |
| dxc-3 | physically based | no | 10374 | 0 | 0 | 0 | 0 / 0 | accepted | about `(0.002, 0.002, 0.002)` in every quadrant |
| dxc-3 | pbs_lit | yes | 0 | 0 | 0 | 0 | 0 / 0 | 66.0 s | `(0,0,0)` |

### no-premul, static DXC, run 41208243248 (`d6ea1163`)

Zero of nine are flat. Every sample matches the lavapipe means below. Parity negatives are 1787, the same count as lavapipe. `pbs_lit_material_matrix` is an unexpected pass on all three Windows jobs (the image gate passed). The other two cases stay quarantined `WARP lit shading untrusted` by policy, with these floats. Stall on the accepted captures is 149–196 ms. There is no flat-exit settle.

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | NaN / neg | Linear RGB (TL, TR, BL, BR) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dxc-1, dxc-2, dxc-3 | parity | no | 9791 | 24 | 0 | 68 | 0 / 1787 | `(0.119, 0.097, 0.069)`, `(0.114, 0.092, 0.064)`, `(0.026, 0.021, 0.014)`, `(0.024, 0.018, 0.012)` |
| dxc-1, dxc-2, dxc-3 | physically based | no | 9868 | 0 | 0 | 0 | 0 / 0 | `(0.124, 0.102, 0.074)` top, `(0.030, 0.025, 0.018)` bottom, left equals right |
| dxc-1, dxc-2, dxc-3 | pbs_lit | no | 16949 | 0 | 2 | 5 | 0 / 0 | `(0.106, 0.077, 0.012)`, `(0.106, 0.140, 0.371)`, `(0.224, 0.017, 0.017)`, `(0.027, 0.182, 1.056)` |

The three samples agree. `pbs_lit` PNG `g0` is 0–5 against lavapipe's 37 because near-zero G encodes to 0. The linear G means are the lavapipe means.

### untextured, static DXC, run 41208259340 (`d72bddb9`)

Same outcome as no-premul. Zero of nine are flat. The nine linear means, `non_bg`, channel-zero counts, and the parity negative count 1787 match the no-premul table and lavapipe. Stall 143–288 ms. No flat-exit settle. `pbs_lit` unexpected-passes on all three Windows jobs.

### `-Gis -Od`, static DXC, run 41208415616 (`b9a32b8b`)

The probe log on each Windows job is `diag-8a dxc status=0 flags=-Gis -Od dxil-fast-count=0`. Fast-math flags are gone from that DXIL. The frames are still wrong. Five of nine are flat black. The non-flat frames are grey (R=G=B) or, on dxc-3 `pbs_lit`, a G-only wipe (`r0=b0=17214`, linear R and B are 0, G is about 0.168). No NaNs, no negatives. The Windows jobs exit non-zero because those frames fail the gate. That is not an infrastructure failure.

| Sample | Case | Flat | non_bg | g0 | r0 | b0 | NaN / neg | Settle | Linear RGB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dxc-1 | parity | no | 10374 | 0 | 0 | 0 | 0 / 0 | accepted, stall 381 ms | grey `(0.135, 0.135, 0.135)` top, `(0.081, 0.081, 0.081)` bottom |
| dxc-1 | physically based | yes | 0 | 0 | 0 | 0 | 0 / 0 | no early exit, stall 379 ms | `(0,0,0)` |
| dxc-1 | pbs_lit | yes | 0 | 0 | 0 | 0 | 0 / 0 | no early exit, stall 393 ms | `(0,0,0)` |
| dxc-2 | parity | no | 10374 | 0 | 0 | 0 | 0 / 0 | accepted, stall 396 ms | same grey as dxc-1 parity |
| dxc-2 | physically based | yes | 0 | 0 | 0 | 0 | 0 / 0 | no early exit, stall 382 ms | `(0,0,0)` |
| dxc-2 | pbs_lit | no | 17214 | 0 | 0 | 0 | 0 / 0 | accepted, stall 385 ms | grey `(0.093)`, `(0.017)`, `(0.126)`, `(0.042)` |
| dxc-3 | parity | yes | 0 | 0 | 0 | 0 | 0 / 0 | no early exit, stall 273 ms | `(0,0,0)` |
| dxc-3 | physically based | yes | 0 | 0 | 0 | 0 | 0 / 0 | no early exit, stall 253 ms | `(0,0,0)` |
| dxc-3 | pbs_lit | no | 17214 | 0 | 17214 | 17214 | 0 / 0 | accepted, stall 255 ms | G-only, about `(0, 0.168, 0)` in every quadrant |

### FXC on the debug composed shader

`warp216_negparity_composed` under `WGPU_DX12_COMPILER=fxc` (the `windows-fxc-debug` job of run 41208228557) does not shade. The pipeline fails with the same pair of diagnostics as production, on the debug shader's own HLSL:

```text
raster_material_shader(956,18-204): warning X3570: gradient instruction used in a loop with varying iteration, attempting to unroll the loop
raster_material_shader(1283,5-15): error X3511: unable to unroll loop ... (42 iterations)
raster_material_shader(2448,5-15): error X3511: forced to unroll loop, but unrolling failed.
```

Line 956 is `shadow_atlas.SampleCmp(...)`, the same `textureSampleCompare` as production line 1278. The saved scene color is linear black, NaN 0, because FXC never produced a fragment (`flat-or-unstable-frame` after 300 s). The 8d run that matched llvmpipe was static DXC. FXC rejects the debug shader, and DXC still shades it. The gradient is a red herring for the DXC failure.

## Verdict

no-premul and untextured both eliminate the flat-black frames and the channel wipes. Against this rerun's 7/9 flat, both arms are 0/9 flat, and all 18 of those frames carry the lavapipe means, including parity's 1787 negatives. That is not a 4/9-versus-6/9 split. No further samples.

`-Gis -Od` does not. Fast flags are confirmed absent (`dxil-fast-count=0`) and dxc-3 still draws the G-only wipe. The cost of shipping it would be `-Od` (optimizations off) on every DX12 shader, without fixing WARP.

The two successful arms cut different things. no-premul drops the second copy of the light loop (`shade_metallic_transparent_clustered`) and keeps material `textureSampleBias`. untextured drops material sampling and `discard`, and keeps the second light loop. Either cut is enough for these cases. The shared light-loop math is not the defect. The production fragment is large enough that DXC's compile of it is unstable on WARP.

The smaller cut that still samples textures is no-premul. It is not safe to delete the premultiply call in every build: `ALPHAPREMULTIPLY` materials would lose that entry. These cases never take it (variant bits 0), which is why lavapipe stayed byte-identical. The production-shaped change is to compile that call only into the premultiply variant, so the opaque pipeline is the shader this arm ran.

```diff
--- a/crates/renderide/shaders/materials/pbsmetallic.wgsl
+++ b/crates/renderide/shaders/materials/pbsmetallic.wgsl
@@
-    if (alpha_premultiply_enabled()) {
-        return plight::shade_metallic_transparent_clustered(
-            frag_pos.xy,
-            world_pos,
-            view_layer,
-            surface,
-            options,
-        );
-    }
     let color = plight::shade_metallic_clustered(
         frag_pos.xy,
         world_pos,
         view_layer,
         surface,
         options,
     );
     return vec4<f32>(color, s.alpha);
```

Do not land that deletion by itself. Gate it so a premultiply variant still calls `shade_metallic_transparent_clustered`, and the default opaque variant does not. Cost of the split: one extra pipeline for premultiplied metallic materials, and the opaque pipeline loses the second light loop that this arm showed WARP cannot compile reliably. Cost of the untextured alternative, if someone took that instead: albedo, metalness, normal, occlusion, detail, and emission maps would no longer affect `pbsmetallic`. That is the wrong cut. `-Gis` is not a fix. No production PR.
