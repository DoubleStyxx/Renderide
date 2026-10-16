# Loading a custom shader

RenderideMod sends authored WGSL through the existing `ShaderUpload` command (`RendererCommand` tag 24). FrooxEngine uses that command for Unity shader AssetBundles, and Renderite's polymorphic command table is fixed, so the mod does not add a new command type. A `file` value that starts with `renderide-custom-wgsl:v1` is WGSL instead of a bundle path. AssetBundle uploads are unchanged.

The upload is flushed from the `RenderSystem.SubmitFrame` postfix on the background queue, the same queue FrooxEngine uses for shader assets. The renderer compiles the WGSL with the shipped `renderide::` modules, reflects `@group(1)` material properties, and routes matching shader assets onto the new program.

## Replace a stock shader

Call this from your own mod after RenderideMod has loaded, or from `OnEngineInit` in a mod that references RenderideMod:

```csharp
using RenderideMod;

CustomShaders.Install("Unlit", CustomShaders.ExampleSolidWgsl);
```

`ExampleSolidWgsl` is `Shaders/example_solid.wgsl`, embedded in RenderideMod.dll. Your own file works the same way: `CustomShaders.Install("Unlit", File.ReadAllText(path))`.

`ExampleRainbowWgsl` is `Shaders/example_rainbow.wgsl`. It tints with `_Color`, samples `_MainTex`, and cycles hue with `_Speed` (default 0.2).

`Unlit` is the Unity shader filename without `.shader`. The renderer normalizes it the same way it normalizes AssetBundle names (`Unlit.shader` and `unlit` are the same key; spaces become dashes). Materials whose shader asset resolves to that name start drawing with the custom program, including shaders the host uploads after the install. `CustomShaders.Remove("Unlit")` restores the packaged Unlit stem.

`Shaders/example_solid.wgsl` is a working program. It writes the host `_Color` property, so an existing Unlit material picks the color up with no extra properties. Copy that file and edit the fragment entry.

To bind one shader asset id and leave other assets on the packaged shader, pass the id from the material's shader reference:

```csharp
CustomShaders.Install(shaderAssetId, "MySolid", wgsl);
```

Negative carrier ids are used when you omit the asset id. Those ids only carry the upload; rebinding is by shader name.

## Lifecycle

```csharp
using RenderideMod;

// Install by name: every shader asset named Unlit, now and later.
CustomShaders.Install("Unlit", CustomShaders.ExampleSolidWgsl);

// Re-Install with the same key replaces the program. Identical source is a no-op.
CustomShaders.Install("Unlit", File.ReadAllText(path));

// Remove by name: drops the program, matching assets return to the packaged stem.
CustomShaders.Remove("Unlit");

// Explicit asset id: bind this shader asset (and same-name assets) to the program.
CustomShaders.Install(shaderAssetId, "MySolid", wgsl);
CustomShaders.Remove(shaderAssetId, "MySolid");

// Route one material onto an installed key. Stock shader routes stay put.
CustomShaders.AssignMaterial(materialAssetId, "MySolid");
CustomShaders.ClearMaterial(materialAssetId);
```

- All calls queue; nothing is sent until the next `RenderSystem.SubmitFrame` postfix with a live messaging host.
- Calls without an asset id consume a negative carrier id (`-1`, `-2`, ...). Remove included.
- `Remove` key must match the `Install` key after normalization (`Unlit`, `Unlit.shader`, `unlit` are equal).
- Remove of an unknown key is a silent no-op on the renderer.
- Empty/whitespace key or WGSL, or a key containing a newline, throws `ArgumentException` at the call site.
- `AssignMaterial` and `ClearMaterial` also queue on a negative carrier id. The material asset id is the `material=` header, not `ShaderUpload.assetId`.

## Wire format

`ShaderUpload.file` is ASCII. The first line is exactly `renderide-custom-wgsl:v1`. Header lines follow until a separator. The separator is a line whose text is `---`. Bytes after that line are the WGSL body. Header lines are `name=value`. Blank header lines are ignored. Names and values are trimmed. Duplicate names are rejected. Unknown names are rejected.

`ShaderUpload.assetId` is the host shader asset id for an install that names one asset, or a negative carrier (`-1`, `-2`, ...) otherwise. Assign and clear always use a carrier. A carrier id gets no `ShaderUploadResult`.

Install, bytes unchanged:

```
renderide-custom-wgsl:v1
key=<shader key>
---
<wgsl>
```

Remove, bytes unchanged:

```
renderide-custom-wgsl:v1
key=<shader key>
remove=1
---
```

Assign:

```
renderide-custom-wgsl:v1
op=assign
material=<material asset id>
key=<shader key>
---
```

Clear:

```
renderide-custom-wgsl:v1
op=clear
material=<material asset id>
---
```

Header rules:

- `key` is required for install, remove, and assign. It is forbidden on clear. It is one line, non-empty after trim.
- `material` is a base-10 `i32` (optional leading `-`). It is required for assign and clear, and forbidden on install and remove.
- `op` is `install`, `remove`, `assign`, or `clear`. Omitted `op` means install, unless `remove` is `1`, `true`, or `yes`, which means remove. `op=remove` is the same as `remove=1`. `op=install` together with `remove=1` is rejected. `remove=1` on assign or clear is rejected.
- Install body is the WGSL and must be non-empty after trim. Remove, assign, and clear bodies must be empty. The formatter writes a trailing newline after `---`.
- `key` is normalized by the renderer the same way as a Unity shader filename (`Unlit.shader` and `unlit` match). The header itself keeps the key text the caller passed.

`CustomShaders.AssignMaterial` writes the assign bytes above. `CustomShaders.ClearMaterial` writes the clear bytes. A key that does not match a stock shader name only registers the program; stock routes change only when the key equals a stock shader name.

The renderer keeps a map from material asset id to that custom key. Material resolution uses the key's stem when the program is installed, and the stock route until then. Pipeline and bind-group caches for the stem stay keyed by `shader_source_generation`. `Remove` drops overrides for that key. Unloading the material drops its override.

## Authoring rules

The source uses the same directives as `crates/renderide/shaders/materials`:

- `#import renderide::frame::globals`, `renderide::draw::per_draw`, and `renderide::mesh::vertex`.
- Call `rg::retain_globals_additive` from the fragment shader so the fixed frame bind group stays intact.
- Declare `@group(1) @binding(0)` for material uniforms. Property names such as `_Color`, `_MainTex`, and `_Speed` match the host material. Add `//#mat_default _Color vec4 1 1 1 1` for the fallback value.
- Put each texture at `@group(1)`. `texture_2d`, `texture_cube`, and `texture_2d_array` map to the host texture property of the same name. `texture_2d` reads 2D, render, and video assets. `texture_cube` reads a cubemap. `texture_2d_array` reads that cubemap's face array.
- Pair a sampler with its texture by name (`_MainTex` and `_MainTex_sampler`). A sampler whose name does not match a texture uses the texture on the previous binding, then the default sampler.
- A declared `*_ST` `vec4<f32>` is that texture's tiling and offset. Unset, it is `(1, 1, 0, 0)`.
- `//#keyword NAME` defines a shader variant. Names are bits in source order: the first keyword is bit 0. That order is not Unity's keyword bit layout. A host shader-variant bit, or a nonzero float property of the same name, enables the `#ifdef`. The renderer keeps at most 16 composed variants per stem. A float whose name matches `_[A-Z0-9_]+` and is not given a `//#keyword` is packed as 0 or 1.
- Supported uniform members are `f32`, `vec2<f32>`, `vec3<f32>`, `vec4<f32>`, `u32`, and `array<vec4<f32>, N>`. A host `float4` array writes into that member at a 16-byte stride, dropping elements past `N` and zeroing slots the host did not set. `array<f32, N>` is not a uniform type: its stride is 4, and uniform arrays require a stride of 16. `//#mat_default` is `float` for `f32` and `vec4` for `vec4<f32>`.
- Put `//#pass type=forward` immediately above the `@fragment` entry. The vertex entry defaults to `vs_main`.
- Wrap `@builtin(view_index)` in `#ifdef MULTIVIEW`. The renderer composes a desktop target and a stereo target.

## Material properties

FrooxEngine already uploads the material property block on `MaterialsUpdateBatch`: `SetFloat`, `SetFloat4` (colors and vectors), `SetFloatArray`, `SetFloat4Array`, and `SetTexture`, keyed by property name. Material property blocks ride the same batch and override the material values. RenderideMod does not add a command for that block.

The renderer reflects `@group(1)` with naga, maps each uniform member and texture name to those property names, and packs a per-material uniform buffer in WGSL uniform (std140) layout. A later property write updates that buffer, or the texture bind group, without rebuilding the pipeline. The bind-group cache and the stem layout stay keyed by `shader_source_generation`, so a new shader source still misses the old layout.

An unset float or vector uses its `//#mat_default`, then zero. An unset texture binds white (`texture_2d_array` uses a one-layer white array). A reflected type the host cannot supply (`i32`, `mat4x4`, a default whose kind does not match the field) fails the install. Asset ids `>= 0` get `ShaderUploadResult`. Carrier ids below zero still get no ack. A rejected replace leaves the previous program in place.

A failed compile is logged (`custom shader install failed`) and the shader asset keeps its packaged route. The next successful `Install` for that key replaces it.

## Smoke test

Path: install `Unlit` with `ExampleSolidWgsl`, confirm a negative carrier asset id, remove, confirm the packaged Unlit is back.

Setup:

1. `dotnet build RenderideMod -c Release`. `ResonitePath` is auto-detected for default Steam installs; pass `-p:ResonitePath=...` otherwise. The build copies `RenderideMod.dll` to `rml_mods`.
2. Run Resonite with Renderide as the renderer. Spawn a box with an UnlitMaterial, a texture, and a non-white Color. Baseline: textured.

Install:

3. From a mod `OnEngineInit` (or any point after RenderideMod loads), call `CustomShaders.Install("Unlit", CustomShaders.ExampleSolidWgsl);`. The upload is queued and flushed from the `RenderSystem.SubmitFrame` postfix.
4. Mod log: `custom shader upload sent: assetId=-1 bytes=<n>`. The first carrier id is `-1`; each later `Install`/`Remove` without an asset id takes the next lower id (`-2`, `-3`, ...). A non-negative id here means an explicit-id overload was called.
5. Renderer log, in order:
   - `custom shader installed key=unlit stem=renderide_custom_unlit_default`
   - `shader_upload: asset_id=-1 shader_asset_name=Some("unlit") ... raster_pipeline=EmbeddedStem("renderide_custom_unlit_default")`
   - no `custom shader payload rejected` or `custom shader install failed for unlit` warning.
6. Box draws as a flat fill of its Color, texture ignored. Materials with no Color use the `//#mat_default` value from `example_solid.wgsl`.

Remove:

7. Call `CustomShaders.Remove("Unlit");`.
8. Mod log: `custom shader upload sent: assetId=-2 bytes=<n>` (next carrier id).
9. Renderer log: `custom shader removed key=unlit stem=renderide_custom_unlit`, then a `shader_upload: asset_id=-2` line. Remove of a key that was never installed logs no `removed` line.
10. Box is textured again: matching shader assets are retargeted to the packaged Unlit stem. No restart.

Fail triage:

- No mod `upload sent` line: renderer never reached a state where the messaging host exists; queue holds until it does.
- `payload rejected`: header malformed; payload must start with `renderide-custom-wgsl:v1`.
- `install failed`: WGSL compose/reflect error; asset stays on the packaged route.
