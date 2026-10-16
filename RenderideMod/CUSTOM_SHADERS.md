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

`Unlit` is the Unity shader filename without `.shader`. The renderer normalizes it the same way it normalizes AssetBundle names (`Unlit.shader` and `unlit` are the same key; spaces become dashes). Materials whose shader asset resolves to that name start drawing with the custom program, including shaders the host uploads after the install. `CustomShaders.Remove("Unlit")` restores the packaged Unlit stem.

`Shaders/example_solid.wgsl` is a working program. It writes the host `_Color` property, so an existing Unlit material picks the color up with no extra properties. Copy that file and edit the fragment entry.

To bind one shader asset id and leave other assets on the packaged shader, pass the id from the material's shader reference:

```csharp
CustomShaders.Install(shaderAssetId, "MySolid", wgsl);
```

Negative carrier ids are used when you omit the asset id. Those ids only carry the upload; rebinding is by shader name.

## Authoring rules

The source uses the same directives as `crates/renderide/shaders/materials`:

- `#import renderide::frame::globals`, `renderide::draw::per_draw`, and `renderide::mesh::vertex`.
- Call `rg::retain_globals_additive` from the fragment shader so the fixed frame bind group stays intact.
- Declare `@group(1) @binding(0)` for material uniforms. Property names such as `_Color` match the host material. Add `//#mat_default _Color vec4 1 1 1 1` for the fallback value.
- Put `//#pass type=forward` immediately above the `@fragment` entry. The vertex entry defaults to `vs_main`.
- Wrap `@builtin(view_index)` in `#ifdef MULTIVIEW`. The renderer composes a desktop target and a stereo target.

A failed compile is logged (`custom shader install failed`) and the shader asset keeps its packaged route. The next successful `Install` for that key replaces it.

## Smoke test

1. Build RenderideMod (`dotnet build RenderideMod -c Release`, with `ResonitePath` set) and run Resonite with Renderide as the renderer.
2. From a mod `OnEngineInit`, call `CustomShaders.Install("Unlit", CustomShaders.ExampleSolidWgsl);`.
3. Mod log: `custom shader upload sent: assetId=-1 bytes=...` once the renderer reaches the `Rendering` state.
4. Renderer log: a `shader_upload: asset_id=-1` line with a `renderide_custom_unlit` embedded stem, and no `custom shader install failed` or `payload rejected` warning.
5. Spawn a box with an UnlitMaterial and set its Color. It draws as a flat fill of that color, untextured.
6. `CustomShaders.Remove("Unlit");` restores the textured packaged Unlit.
