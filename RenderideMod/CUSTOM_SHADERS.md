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

`ExampleRainbowWgsl` is `Shaders/example_rainbow.wgsl`. It tints with `_Color`, samples `_MainTex` with `_MainTex_ST`, and cycles hue from `rg::frame.frame_time.x` scaled by `_Speed` (default 0.2) when `//#keyword RAINBOW` is on. Off, the fragment is `_MainTex * _Color`.

`ExamplePoincareWgsl` is `Shaders/example_poincare.wgsl`. It draws a hyperbolic Poincare-disk {p,q} tiling. `_Schlafli` is a `vec4<f32>` whose `xy` are p and q (default 7, 3). `_Speed` rotates the disk. `//#keyword TILE` off writes `_MainTex * _Color`.

`ExampleGyroidWgsl` is `Shaders/example_gyroid.wgsl`. It raymarches a gyroid SDF in object space. `_Frequency` is an `f32` scale. `_Speed` scrolls the phase. `//#keyword GYROID` off writes `_MainTex * _Color`.

`ExampleHologramWgsl` is `Shaders/example_hologram.wgsl`. It mixes a Fresnel tint with scanlines. `_Scan` is a `vec4<f32>` (`x` density, `y` contrast). `_Fresnel` is an `f32` power. `_Speed` scrolls the lines. `//#keyword HOLO` off writes `_MainTex * _Color`.

`ExampleHieroglyphWgsl` is `Shaders/example_hieroglyph.wgsl`. It draws a procedural glyph grid with a gold tint, a view-aligned specular, and an emissive pulse. `_Grid` is a `vec4<f32>` (`xy` cell count). `_Emission` is an `f32` scale. `_Speed` drives the pulse. `//#keyword EMIT` off writes `_MainTex * _Color`.

`ExampleNebulaWgsl` is `Shaders/example_nebula.wgsl`. It is a domain-warped fbm. `_Warp` is a `vec4<f32>` (`x` warp amount, `y` pattern scale). `_Speed` scrolls the domain. `//#keyword NEBULA` off writes `_MainTex * _Color`.

`ExampleDispersionWgsl` is `Shaders/example_dispersion.wgsl`. It offsets the red and blue `_MainTex` samples along a refraction axis and leaves green on the geometric UV. `_Spread` is an `f32` offset. `_Speed` rotates the axis. `//#keyword SPLIT` off writes one `_MainTex * _Color` sample.

Each of those samples `_MainTex` with `_MainTex_ST`, tints with `_Color`, and reads time from `rg::frame.frame_time.x * _Speed`.

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
- Scene use of assign/clear is the `CustomShaderMaterial` component below.

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

## Dynamic variable uniforms

Resonite `DynamicVariableSpace` values feed custom-shader uniforms through the material property batch. The mapping is mod-side. This tree does not add a dynvar component, and `renderide-custom-wgsl:v1` is unchanged. No new IPC command is added. The renderer already applies `SetFloat`, `SetFloat4`, and `SetTexture` from `MaterialsUpdateBatch`.

Naming:

- The default prefix is `Shader/`.
- The uniform name is the dynvar name with that prefix removed once. `Shader/_Speed` writes `_Speed`.
- A helper may be configured with another prefix. A dynvar that does not start with the active prefix is not a shader uniform.

Type mapping, written with the existing batch ops:

- `float` -> `f32`, via `SetFloat`.
- `float2` -> `vec4<f32>`, `xy` from the value, `zw` = 0, via `SetFloat4`.
- `float3` -> `vec4<f32>`, `xyz` from the value, `w` = 0, via `SetFloat4`.
- `float4` -> `vec4<f32>`, via `SetFloat4`.
- `colorX` -> `vec4<f32>`, rgba, via `SetFloat4`.
- texture -> `texture_2d` of the same uniform name, via `SetTexture`. An optional `float4` dynvar named `<uniform>_ST` (for example `Shader/_MainTex_ST`) is that texture's tiling and offset. Unset, `_ST` stays `(1, 1, 0, 0)`.

Update timing:

- The helper writes mapped values onto the host material with the same property ops FrooxEngine already uses. The renderer applies them when it applies that `MaterialsUpdateBatch`.
- A dynvar change does not recompile the shader and does not allocate a new stem. It updates the uniform buffer or the texture bind the same way any other property write does.

Limits:

- Only a reflected `@group(1)` member is stored. A name with no matching uniform or texture does not change the buffer.
- `float` fills an `f32` member. `float2`, `float3`, `float4`, and `colorX` fill a `vec4<f32>` member. A texture fills `texture_2d` only. Cubemaps and texture arrays are outside this contract.
- Keyword toggles stay floats of the keyword name (`TILE`, `GYROID`, and the other example keywords). A nonzero value enables the `#ifdef`, same as any other float property of that name.
- The wire payload is still the install, remove, assign, and clear bytes above.

## CustomShaderMaterial

`CustomShaderMaterial` is a FrooxEngine material under Assets/Materials. It extends `SingleShaderMaterialProvider` and keeps the packaged Unlit `ShaderURL`, so the host uploads it on `MaterialsUpdateBatch` the same way it uploads `UnlitMaterial`. `ShaderKey` is an installed program key for this material only. A second material with no component, or with an empty key, stays on its stock route.

| Field | Contents |
| --- | --- |
| `ShaderKey` | string. Empty clears the override. |
| `VariablePrefix` | string. Default `Shader/`. Blank is the same default. |
| `Floats` | list of `PropertyName` (string) and `Value` (float). `MaterialUpdateWriter.SetFloat(propertyId, value)`. |
| `Colors` | list of `PropertyName` (string) and `Value` (`colorX`). `SetColor(propertyId, in colorX)`. |
| `Textures` | list of `PropertyName` (string) and `Texture` (`ITexture2D` asset ref). `SetTexture(propertyId, provider)`. |
| `Keywords` | list of `PropertyName` (string) and `Value` (bool). Every row is `SetFloat` 0 or 1. `UpdateKeyword` tracks the `Sync<bool>` for packaged Unlit. |

## Dynamic variables

`VariablePrefix` reads dynamic variables from `DynamicVariableSpace` components on the material slot and its parents. That is the same parent walk a variable uses to bind: a name `Space/Local` binds to the space named `Space`, and a name with no slash binds to the first ancestor space whose `OnlyDirectBinding` is false.

The canonical name is the component `VariableName` when it contains a slash, otherwise `SpaceName/VariableName`. Names that start with the prefix are sampled. The property name is that canonical name with the prefix removed (`Shader/_Speed` and a space named `Shader` with a variable `_Speed` both become `_Speed`).

| Dynvar type | Writer call |
| --- | --- |
| `float` | `SetFloat` |
| `float2`, `float3`, `float4` | `SetFloat4`, missing lanes zero |
| `colorX` | `SetColor`, same path as static `Colors` and stock `UnlitMaterial` |
| `IAssetProvider<ITexture2D>` | `SetTexture` |

`SetColor` (`MaterialUpdateWriter.cs:223`) converts with `ToProfile(RenderSystem.MaterialColorProfile)` and then calls `SetFloat4` (`MaterialUpdateWriter.cs:217`). Stock `UnlitMaterial.cs:510` uses `writer.UpdateColor`, which calls `SetColor`. Static `Colors` already use `SetColor`. A color dynvar uses that same call so the profile matches a stock material.

Other types are ignored. A dynvar property replaces a `Floats`, `Colors`, or `Textures` row of the same name. The value still uses `MaterialPropertyIdRequest` and `MaterialUpdateWriter`. `ShaderUpload` is unchanged: it only installs or removes WGSL, and only when `ShaderKey` or the source changes.

The component subscribes to space, slot, and variable change events and calls `MarkChangeDirty` only when the mapped value or the mapped set changes. It does not poll variable values from `OnCommonUpdate`.

Nearest ancestor space wins when two variables map to one property. Inside one space, the first live variable in slot hierarchy order wins. A disabled variable or an inactive slot does not override.

## Smoke checklist: dynamic variables

Build and run the rainbow-beside-unlit setup first (`CustomShaders.Install("Rainbow", CustomShaders.ExampleRainbowWgsl)`).

1. On box A, `CustomShaderMaterial` has `ShaderKey` `Rainbow` and `VariablePrefix` `Shader/` (the attach default).
2. On that slot, or a parent, add `DynamicVariableSpace`. Set `SpaceName` to `Shader`. Leave `OnlyDirectBinding` false.
3. Under that space, add `DynamicValueVariable<float>` named `_Speed` (or `Shader/_Speed`) set to `0.5`.
4. Add `DynamicValueVariable<colorX>` named `_Color`, `DynamicReferenceVariable<IAssetProvider<ITexture2D>>` named `_MainTex` with a texture, and optionally `DynamicValueVariable<float4>` named `_MainTex_ST`.
5. Also add a `Floats` row `_Speed` = `0.2` and a `Colors` row `_Color`. These are the static values the dynvars must hide.
6. Mod log includes `custom shader material dynvars: count=<n> prefix=Shader/` with `_Speed`, `_Color`, and `_MainTex` in the property-id lines. Box A uses the dynvar speed, color, and texture. The static `0.2` and static color are not the values on the material.
7. Change `_Speed` once. The material updates on that change. Leaving the value alone does not keep dirtying the material.
8. `float2` / `float3` dynvars, if added under names such as `_Pad`, upload as `SetFloat4` with the unused lanes at 0.
9. Delete the `_Speed` variable, or disable it. The static `Floats` value `0.2` is written again. Box B is unchanged.
10. Clear `ShaderKey` or disable the component. Mod log: `custom shader material clear: material=<id>`. Box A returns to packaged Unlit.
11. Move the material slot out from under the `Shader` space. The dynvar overrides drop and the static rows apply. Move it back and the overrides return.
12. Set `VariablePrefix` to `Other/`. The `Shader/` variables stop overriding. Set it back to `Shader/` and they return. Blank prefix is `Shader/`.

`PropertyName` is the WGSL name (`_Color`, `_MainTex`, `_Speed`, `RAINBOW`). Blank names are skipped. The entry field is not named `Name` because that hides `SyncObject.Name`.

Property ids are not names. Stock materials declare static `MaterialProperty` fields; `MaterialProviderBase` reflects them once per type and `RenderMaterialManager` resolves them with `MaterialPropertyIdRequest`, guarded by static `PropertiesInitialized`. `CustomShaderMaterial` has no static `MaterialProperty` fields. That init sends no request and never sets the flag, so the host would leave `AssetId` at `-1` and skip `UpdateMaterial`. The component sets `PropertiesInitialized` itself in that case. Each list name, including keyword rows, is requested on first use and on rename through the same `MaterialPropertyIdRequest` command (request ids start at `1<<30`). The renderer answers on the background queue. The mod reads that result from `RenderiteMessagingHost._background` (and `_primary` if that field is present). Results whose request id is `>= 1<<30` are not forwarded to `RenderMaterialManager`. `SetFloat`, `SetColor`, and `SetTexture` use the returned int. Writes wait until the id is positive. Each material keeps its sorted property names and marks itself change-dirty when one of its own names gains a stored id, not when some other material's request completes. An unanswered request is kept, and only the names that are still unsettled are sent again. The gap doubles from 8 flushes through 16, 32, and so on, and stops growing at 512. The fifth retry logs one warning; later retries do not. A stored id of `0` is not retried and logs one warning. Retired request ids are capped at 64. Older entries are dropped, and a late result for a dropped id is ignored; the name stays queued on a newer request until it settles. `_MainTex_ST` is not a row on this component; an unset `*_ST` stays `(1, 1, 0, 0)`.

Enable, and any change of `ShaderKey` to a non-empty value, calls `CustomShaders.AssignMaterial(AssetId, ShaderKey)`. Disable, destroy, and a cleared or whitespace key call `CustomShaders.ClearMaterial`. `AssetId` is `DynamicRendererAsset.AssetId` (`public int`, default `-1`). The assign waits while it is negative. It is not a `ShaderUpload` carrier id.

`UpdateKeywords` passes each row's `Sync<bool>` to `ShaderKeywords.UpdateKeyword` and does not also call `SetKeyword`. Every row, including names packaged Unlit accepts, is `SetFloat` 1 or 0. On a per-material override the renderer ignores host variant bits, so Unlit `_ALPHATEST` (bit 0) cannot force `//#keyword RAINBOW` and a zero float turns that keyword off. A custom program reached by shader name still ORs those host bits. Removing a keyword row sends 0 once.

Install the rainbow program under its own key. Do not install it as `Unlit` when a stock Unlit material should keep the packaged shader:

```csharp
using RenderideMod;

CustomShaders.Install("Rainbow", CustomShaders.ExampleRainbowWgsl);
```

On one mesh, attach `CustomShaderMaterial` and set `ShaderKey` to `Rainbow`. Add a Colors row `_Color`, a Textures row `_MainTex`, a Floats row `_Speed`, and a Keywords row `RAINBOW` set true. On another mesh, attach `UnlitMaterial` and leave it alone. The two materials coexist: Rainbow is one material's override, Unlit stays the packaged route. `RAINBOW` off takes the `_MainTex * _Color` branch of the custom program when the renderer sees that keyword as disabled.

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

## Smoke test: rainbow beside unlit

Two materials in one world. One draws `ExampleRainbowWgsl` through `CustomShaderMaterial`. The other is stock `UnlitMaterial`. Install rainbow under its own key so the Unlit shader asset stays on the packaged stem.

Headless coverage of that split is the `custom_shader_beside_unlit` case in `renderide-test`. The left material is `ExampleRainbowWgsl` with `RAINBOW` off (`_MainTex * _Color`). The right material stays packaged Unlit on `_Tex`. The second frame clears the left material. `custom_shader_property_update` is the frame that turns `RAINBOW` on.

1. `dotnet build RenderideMod -c Release` and run Resonite with Renderide, same as the smoke test above.
2. After RenderideMod loads, call `CustomShaders.Install("Rainbow", CustomShaders.ExampleRainbowWgsl);`.
3. Mod log: `custom shader upload sent: assetId=-1 bytes=<n>` (or the next carrier id if an earlier install ran). Renderer log: `custom shader installed key=rainbow stem=renderide_custom_rainbow_default`. No `custom shader payload rejected` and no `custom shader install failed for rainbow`.
4. Spawn two boxes.
5. Box A: attach `CustomShaderMaterial`. Set `ShaderKey` to `Rainbow`. Add a Colors row `PropertyName` `_Color`, a Textures row `PropertyName` `_MainTex` with a texture, a Floats row `PropertyName` `_Speed` set to `0.2`, and a Keywords row `PropertyName` `RAINBOW` set true.
6. Box B: attach `UnlitMaterial` with a texture and a non-white TintColor. Do not attach `CustomShaderMaterial`.
7. Mod log, once `AssetId` is non-negative: `custom shader material property id request: requestId=<n> count=<n>`, then `custom shader material property id: name=_Color id=<id>` (and the same for `_MainTex`, `_Speed`, and `RAINBOW`), then `custom shader material assign: material=<id> key=Rainbow`, then `custom shader upload sent` with the next negative carrier id. Renderer log: `shader_upload: asset_id=<carrier> shader_asset_name=None ... raster_pipeline=Null`. That carrier does not register a shader route. The override is the material id to key map. No `custom shader payload rejected` line.
8. Box A cycles hue from world x and `_Speed`, multiplied by `_Color` and `_MainTex` (sampled with `_MainTex_ST` at the default `(1, 1, 0, 0)`). `RAINBOW` is on because `SetFloat` sent 1. Box B stays textured unlit. Rainbow and Unlit coexist because the override is per material asset id.
9. Clear `ShaderKey`, or disable or destroy the component on box A. Mod log: `custom shader material clear: material=<id>`, then another carrier `custom shader upload sent` line. Renderer log is the same carrier `shader_upload` line with `raster_pipeline=Null`. Box A draws packaged Unlit. Box B is unchanged.
10. Set the `RAINBOW` keyword false. `SetFloat` sends 0. Box A stays on the Rainbow program. The fragment is `_MainTex * _Color` with no hue cycle, including when the Unlit material has alpha clipping on. Box B is unchanged.

Fail triage:

- No `custom shader material assign` line: `AssetId` is still `-1` (the render asset is not integrated yet). Property rows can still upload once their ids exist. The assign waits until `AssetId >= 0`.
- `_background MessagingManager was not found`: property id requests are waiting, so `SetFloat`, `SetColor`, and `SetTexture` do not run yet. Assign can still run.
- `custom shader material assign rejected`: `ShaderKey` was empty after trim or contained a newline.
- Property id log is missing for a row: the name was blank, the request is still in flight, or the result id was `0`.
- Box A has no hue cycle and the assign log is present: `RAINBOW` is off, `_Speed` is 0, the material id in the assign line is not the id of the drawn material, or the `RAINBOW` property id was 0 so `SetFloat` did not run. Host Unlit variant bits do not enable `RAINBOW` on an assigned material.
- Both boxes share the packaged Unlit program: the Rainbow install did not land, or assign/clear targeted a different material id. The stock Unlit route is unchanged by an install of key `Rainbow`.
