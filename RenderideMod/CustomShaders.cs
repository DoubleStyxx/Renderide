namespace RenderideMod;

/// <summary>
/// Loads authored WGSL into the Renderide renderer.
/// </summary>
/// <remarks>
/// <para>
/// FrooxEngine already uploads Unity shaders with <c>ShaderUpload</c>. Renderite's command
/// table is closed, so this mod reuses that command. The <c>file</c> field carries a
/// <c>renderide-custom-wgsl:v1</c> payload instead of an AssetBundle path. The renderer
/// compiles the WGSL and routes the shader asset onto it.
/// </para>
/// <para>
/// Call <see cref="Install(string, string)"/> with the Unity shader filename (for example
/// <c>Unlit</c>) to replace that shader for assets the host has uploaded or uploads later.
/// Pass an explicit asset id when the material should follow one shader asset only.
/// </para>
/// <para>
/// Floats, float4 colors, and textures already travel on the host material update batch,
/// keyed by property name (<c>_Color</c>, <c>_MainTex</c>, <c>_Speed</c>). Property blocks
/// use that same batch. WGSL <c>@group(1)</c> members with those names read the values.
/// </para>
/// </remarks>
public static class CustomShaders
{
    /// <summary>
    /// Text of <c>Shaders/example_solid.wgsl</c>, embedded in the mod assembly.
    /// </summary>
    /// <remarks>
    /// <c>CustomShaders.Install("Unlit", CustomShaders.ExampleSolidWgsl)</c> turns every Unlit
    /// material into a flat <c>_Color</c> fill.
    /// </remarks>
    public static string ExampleSolidWgsl => ReadEmbeddedShader("example_solid.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_rainbow.wgsl</c>, embedded in the mod assembly.
    /// </summary>
    /// <remarks>
    /// Samples <c>_MainTex</c> with <c>_MainTex_ST</c>, multiplies by <c>_Color</c>, and cycles
    /// hue with <c>_Speed</c> (default 0.2) when <c>//#keyword RAINBOW</c> is on. Off writes
    /// <c>_MainTex * _Color</c>. FrooxEngine already uploads those values on the material batch.
    /// </remarks>
    public static string ExampleRainbowWgsl => ReadEmbeddedShader("example_rainbow.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_poincare.wgsl</c>. Poincare-disk {p,q} tiling.
    /// </summary>
    /// <remarks>
    /// <c>_Schlafli.xy</c> is {p,q} (default 7,3). <c>_Speed</c> rotates the disk.
    /// <c>//#keyword TILE</c> off writes <c>_MainTex * _Color</c>.
    /// </remarks>
    public static string ExamplePoincareWgsl => ReadEmbeddedShader("example_poincare.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_gyroid.wgsl</c>. Object-space raymarched gyroid SDF.
    /// </summary>
    /// <remarks>
    /// <c>_Frequency</c> scales the field. <c>_Speed</c> scrolls the phase.
    /// <c>//#keyword GYROID</c> off writes <c>_MainTex * _Color</c>.
    /// </remarks>
    public static string ExampleGyroidWgsl => ReadEmbeddedShader("example_gyroid.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_hologram.wgsl</c>. Fresnel tint plus scanlines.
    /// </summary>
    /// <remarks>
    /// <c>_Scan.x</c> is line density, <c>_Scan.y</c> is contrast, <c>_Fresnel</c> is the power.
    /// <c>_Speed</c> scrolls the lines. <c>//#keyword HOLO</c> off writes <c>_MainTex * _Color</c>.
    /// </remarks>
    public static string ExampleHologramWgsl => ReadEmbeddedShader("example_hologram.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_hieroglyph.wgsl</c>. Procedural gold glyph grid.
    /// </summary>
    /// <remarks>
    /// <c>_Grid.xy</c> is the cell count. <c>_Emission</c> scales the pulse. <c>_Speed</c> drives it.
    /// <c>//#keyword EMIT</c> off writes <c>_MainTex * _Color</c>.
    /// </remarks>
    public static string ExampleHieroglyphWgsl => ReadEmbeddedShader("example_hieroglyph.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_nebula.wgsl</c>. Domain-warped fbm nebula.
    /// </summary>
    /// <remarks>
    /// <c>_Warp.x</c> is the warp amount and <c>_Warp.y</c> is the pattern scale.
    /// <c>_Speed</c> scrolls the domain. <c>//#keyword NEBULA</c> off writes <c>_MainTex * _Color</c>.
    /// </remarks>
    public static string ExampleNebulaWgsl => ReadEmbeddedShader("example_nebula.wgsl");

    /// <summary>
    /// Text of <c>Shaders/example_dispersion.wgsl</c>. Per-channel refraction offset.
    /// </summary>
    /// <remarks>
    /// <c>_Spread</c> is the red/blue UV offset. <c>_Speed</c> rotates the offset axis.
    /// <c>//#keyword SPLIT</c> off writes one <c>_MainTex * _Color</c> sample.
    /// </remarks>
    public static string ExampleDispersionWgsl => ReadEmbeddedShader("example_dispersion.wgsl");

    /// <summary>Reads an embedded WGSL file shipped under <c>Shaders/</c>.</summary>
    /// <param name="fileName">File name, for example <c>example_solid.wgsl</c>.</param>
    public static string ReadEmbeddedShader(string fileName)
    {
        var name = "RenderideMod.Shaders." + fileName;
        using var stream = typeof(CustomShaders).Assembly.GetManifestResourceStream(name)
            ?? throw new ArgumentException($"No embedded shader {name}.", nameof(fileName));
        using var reader = new StreamReader(stream);
        return reader.ReadToEnd();
    }

    /// <summary>
    /// Compiles <paramref name="wgsl"/> and binds it to every shader asset whose name matches
    /// <paramref name="shaderKey"/>.
    /// </summary>
    /// <param name="shaderKey">Unity shader filename, with or without <c>.shader</c>.</param>
    /// <param name="wgsl">Authoring WGSL. See <c>Shaders/example_solid.wgsl</c>.</param>
    public static void Install(string shaderKey, string wgsl)
    {
        Ipc.CustomShaderQueue.EnqueueInstall(null, shaderKey, wgsl);
    }

    /// <summary>
    /// Compiles <paramref name="wgsl"/> and binds it to <paramref name="assetId"/>, and to
    /// other shader assets with the same name.
    /// </summary>
    /// <param name="assetId">Host shader asset id from the material's shader reference.</param>
    /// <param name="shaderKey">Unity shader filename, with or without <c>.shader</c>.</param>
    /// <param name="wgsl">Authoring WGSL. See <c>Shaders/example_solid.wgsl</c>.</param>
    public static void Install(int assetId, string shaderKey, string wgsl)
    {
        Ipc.CustomShaderQueue.EnqueueInstall(assetId, shaderKey, wgsl);
    }

    /// <summary>
    /// Drops the custom program for <paramref name="shaderKey"/> and restores the packaged
    /// shader for matching assets.
    /// </summary>
    /// <param name="shaderKey">Key previously passed to <see cref="Install(string, string)"/>.</param>
    public static void Remove(string shaderKey)
    {
        Ipc.CustomShaderQueue.EnqueueRemove(null, shaderKey);
    }

    /// <summary>
    /// Drops the custom program and restores <paramref name="assetId"/> along with other
    /// assets that share <paramref name="shaderKey"/>.
    /// </summary>
    /// <param name="assetId">Host shader asset id to rebind.</param>
    /// <param name="shaderKey">Key previously passed to <see cref="Install(string, string)"/>.</param>
    public static void Remove(int assetId, string shaderKey)
    {
        Ipc.CustomShaderQueue.EnqueueRemove(assetId, shaderKey);
    }

    /// <summary>
    /// Routes <paramref name="materialAssetId"/> onto the installed program
    /// <paramref name="shaderKey"/> without changing stock shader routes.
    /// </summary>
    /// <param name="materialAssetId">Host material asset id.</param>
    /// <param name="shaderKey">Key previously passed to <see cref="Install(string, string)"/>.</param>
    /// <remarks>
    /// The upload uses a negative carrier asset id. The material id travels in the
    /// <c>material=</c> header. See <c>CUSTOM_SHADERS.md</c> for the exact bytes.
    /// </remarks>
    public static void AssignMaterial(int materialAssetId, string shaderKey)
    {
        Ipc.CustomShaderQueue.EnqueueAssign(materialAssetId, shaderKey);
    }

    /// <summary>
    /// Drops the per-material program override for <paramref name="materialAssetId"/>.
    /// </summary>
    /// <param name="materialAssetId">Host material asset id passed to <see cref="AssignMaterial"/>.</param>
    public static void ClearMaterial(int materialAssetId)
    {
        Ipc.CustomShaderQueue.EnqueueClear(materialAssetId);
    }
}
