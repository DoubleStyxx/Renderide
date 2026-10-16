using Elements.Core;
using FrooxEngine;
using ResoniteModLoader;

namespace RenderideMod;

/// <summary>
/// Unlit material that routes one render material onto an installed custom shader key.
/// </summary>
/// <remarks>
/// <para>
/// The host still uploads this material as Unlit. <see cref="ShaderKey"/> calls
/// <see cref="CustomShaders.AssignMaterial"/> for this material's render asset id, so a second
/// material can stay on the packaged Unlit program. Property lists are written with
/// <c>Material.SetFloat</c>, <c>SetFloat4</c>, and <c>SetTexture</c> and ride
/// <c>MaterialsUpdateBatch</c>. Keywords are 0/1 floats whose names match <c>//#keyword</c>.
/// </para>
/// <para>
/// Enable and a non-empty <see cref="ShaderKey"/> assign. Disable, destroy, and a cleared key
/// call <see cref="CustomShaders.ClearMaterial"/>.
/// </para>
/// </remarks>
[Category(new string[] { "Assets/Materials" })]
public class CustomShaderMaterial : SingleShaderMaterialProvider
{
    /// <summary>Installed custom program key. Empty clears the per-material override.</summary>
    public readonly Sync<string> ShaderKey;

    /// <summary>Named floats. <see cref="FloatEntry.Name"/> is the WGSL uniform name.</summary>
    public readonly SyncList<FloatEntry> Floats;

    /// <summary>Named colors. Uploaded with <c>SetFloat4</c> as the <c>colorX</c> rgba components.</summary>
    public readonly SyncList<ColorEntry> Colors;

    /// <summary>Named textures. <see cref="TextureEntry.Texture"/> is the texture asset provider.</summary>
    public readonly SyncList<TextureEntry> Textures;

    /// <summary>
    /// Named keywords. Uploaded with <c>SetFloat</c> as 1 or 0. The name matches a <c>//#keyword</c> line.
    /// </summary>
    public readonly SyncList<KeywordEntry> Keywords;

    private int? _boundMaterialId;
    private string? _boundKey;
    private string? _rejectedKey;

    /// <summary>Packaged Unlit shader. The custom program replaces it per material, not by name.</summary>
    protected override Uri ShaderURL => OfficialAssets.Shaders.Unlit;

    /// <inheritdoc />
    protected override void OnEnabled()
    {
        base.OnEnabled();
        SyncAssignment();
    }

    /// <inheritdoc />
    protected override void OnChanges()
    {
        base.OnChanges();
        if (!Enabled.Value)
            return;
        SyncAssignment();
    }

    /// <inheritdoc />
    protected override void OnCommonUpdate()
    {
        base.OnCommonUpdate();
        if (!Enabled.Value)
            return;
        SyncAssignment();
    }

    /// <inheritdoc />
    protected override void OnDisabled()
    {
        ClearBound();
        base.OnDisabled();
    }

    /// <inheritdoc />
    protected override void OnDestroy()
    {
        ClearBound();
        base.OnDestroy();
    }

    /// <summary>Writes the named lists onto <paramref name="asset"/> and refreshes the shader key.</summary>
    /// <param name="asset">Material asset the host uploads.</param>
    protected override void UpdateMaterial(Material asset)
    {
        if (asset is null)
            return;

        PushFloats(asset);
        PushColors(asset);
        PushTextures(asset);
        PushKeywords(asset);
        // The provider may not have published Asset yet. Use the instance being uploaded.
        SyncAssignment(asset);
    }

    private void PushFloats(Material asset)
    {
        foreach (var entry in Floats)
        {
            if (!TryPropertyName(entry.Name, out var property))
                continue;
            asset.SetFloat(property, entry.Value.Value);
        }
    }

    private void PushColors(Material asset)
    {
        foreach (var entry in Colors)
        {
            if (!TryPropertyName(entry.Name, out var property))
                continue;
            var color = entry.Value.Value;
            var rgba = new float4(color.r, color.g, color.b, color.a);
            asset.SetFloat4(property, rgba);
        }
    }

    private void PushTextures(Material asset)
    {
        foreach (var entry in Textures)
        {
            if (!TryPropertyName(entry.Name, out var property))
                continue;
            asset.SetTexture(property, entry.Texture.Target?.Asset);
        }
    }

    private void PushKeywords(Material asset)
    {
        foreach (var entry in Keywords)
        {
            if (!TryPropertyName(entry.Name, out var property))
                continue;
            asset.SetFloat(property, entry.Value.Value ? 1f : 0f);
        }
    }

    private void SyncAssignment()
    {
        SyncAssignment(Asset);
    }

    private void SyncAssignment(Material? material)
    {
        if (!Enabled.Value)
            return;
        if (material is null)
            return;
        if (!MaterialAssetId.TryRead(material, out var materialId))
            return;

        var key = ShaderKey.Value?.Trim();
        if (string.IsNullOrEmpty(key))
        {
            _rejectedKey = null;
            ClearBound();
            return;
        }

        if (_boundMaterialId == materialId && (_boundKey == key || _rejectedKey == key))
            return;

        if (_boundMaterialId is int previous && previous != materialId)
            ClearBound();

        try
        {
            CustomShaders.AssignMaterial(materialId, key);
        }
        catch (ArgumentException ex)
        {
            _boundMaterialId = materialId;
            _boundKey = null;
            _rejectedKey = key;
            ResoniteMod.Warn($"custom shader material assign rejected: {ex.Message}");
            return;
        }

        _rejectedKey = null;
        _boundMaterialId = materialId;
        _boundKey = key;
        ResoniteMod.Msg($"custom shader material assign: material={materialId} key={key}");
    }

    private void ClearBound()
    {
        if (_boundMaterialId is not int materialId)
            return;

        CustomShaders.ClearMaterial(materialId);
        _boundMaterialId = null;
        _boundKey = null;
        _rejectedKey = null;
        ResoniteMod.Msg($"custom shader material clear: material={materialId}");
    }

    private static bool TryPropertyName(Sync<string> name, out string property)
    {
        property = name.Value?.Trim() ?? "";
        return property.Length > 0;
    }

    /// <summary>One named float in <see cref="Floats"/>.</summary>
    public class FloatEntry : SyncObject
    {
        /// <summary>WGSL uniform name, for example <c>_Speed</c>.</summary>
        public readonly Sync<string> Name;

        /// <summary>Value passed to <c>SetFloat</c>.</summary>
        public readonly Sync<float> Value;
    }

    /// <summary>One named color in <see cref="Colors"/>.</summary>
    public class ColorEntry : SyncObject
    {
        /// <summary>WGSL uniform name, for example <c>_Color</c>.</summary>
        public readonly Sync<string> Name;

        /// <summary>Color whose rgba components are packed into <c>SetFloat4</c>.</summary>
        public readonly Sync<colorX> Value;
    }

    /// <summary>One named texture in <see cref="Textures"/>.</summary>
    public class TextureEntry : SyncObject
    {
        /// <summary>WGSL texture name, for example <c>_MainTex</c>.</summary>
        public readonly Sync<string> Name;

        /// <summary>Texture asset provider passed to <c>SetTexture</c>.</summary>
        public readonly AssetRef<ITexture2D> Texture;
    }

    /// <summary>One named keyword in <see cref="Keywords"/>.</summary>
    public class KeywordEntry : SyncObject
    {
        /// <summary><c>//#keyword</c> name.</summary>
        public readonly Sync<string> Name;

        /// <summary>True uploads 1, false uploads 0.</summary>
        public readonly Sync<bool> Value;
    }
}
