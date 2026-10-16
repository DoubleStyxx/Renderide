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
    /// Samples <c>_MainTex</c>, multiplies by <c>_Color</c>, and cycles hue with <c>_Speed</c>
    /// (default 0.2). FrooxEngine already uploads those values on the material batch; this mod
    /// does not open a second property channel.
    /// </remarks>
    public static string ExampleRainbowWgsl => ReadEmbeddedShader("example_rainbow.wgsl");

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
}
