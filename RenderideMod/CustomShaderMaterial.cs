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
/// <see cref="CustomShaders.AssignMaterial"/> with <see cref="DynamicRendererAsset{A}.AssetId"/>.
/// A second material can stay on the packaged Unlit program. Floats, colors, and textures
/// are written with <see cref="MaterialUpdateWriter"/> using property ids from
/// <see cref="Ipc.MaterialPropertyIds"/>. Keywords go through <see cref="UpdateKeywords"/>.
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

    /// <summary>Named floats. <see cref="FloatEntry.PropertyName"/> is the WGSL uniform name.</summary>
    public readonly SyncList<FloatEntry> Floats;

    /// <summary>Named colors. Uploaded with <c>SetColor</c>.</summary>
    public readonly SyncList<ColorEntry> Colors;

    /// <summary>Named textures. <see cref="TextureEntry.Texture"/> is the texture provider.</summary>
    public readonly SyncList<TextureEntry> Textures;

    /// <summary>Named shader keywords. Passed to <see cref="UpdateKeywords"/>.</summary>
    public readonly SyncList<KeywordEntry> Keywords;

    private static bool _propertiesInitialized;
    private int? _boundMaterialId;
    private string? _boundKey;
    private string? _rejectedKey;
    private int _propertyGeneration;

    /// <summary>Packaged Unlit shader. The custom program replaces it per material, not by name.</summary>
    protected override Uri ShaderURL => OfficialAssets.Shaders.Unlit;

    /// <summary>
    /// Static flag the engine sets after it resolves this type's static <c>MaterialProperty</c> fields.
    /// This type has none; named list rows use <see cref="Ipc.MaterialPropertyIds"/> instead.
    /// </summary>
    public override bool PropertiesInitialized
    {
        get => _propertiesInitialized;
        protected set => _propertiesInitialized = value;
    }

    /// <inheritdoc />
    protected override void OnEnabled()
    {
        base.OnEnabled();
        QueuePropertyIds();
        SyncAssignment();
    }

    /// <inheritdoc />
    protected override void OnChanges()
    {
        base.OnChanges();
        if (!Enabled.Value)
            return;
        QueuePropertyIds();
        SyncAssignment();
    }

    /// <inheritdoc />
    protected override void OnCommonUpdate()
    {
        base.OnCommonUpdate();
        if (!Enabled.Value)
            return;

        var generation = Ipc.MaterialPropertyIds.Generation;
        if (generation != _propertyGeneration)
        {
            _propertyGeneration = generation;
            MarkChangeDirty();
        }

        QueuePropertyIds();
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

    /// <summary>Sets keyword state from <see cref="Keywords"/>.</summary>
    /// <param name="keywords">Host keyword set for this material.</param>
    protected override void UpdateKeywords(ShaderKeywords keywords)
    {
        foreach (var entry in Keywords)
        {
            if (!TryPropertyName(entry.PropertyName, out var keyword))
                continue;
            keywords.UpdateKeyword(keyword, entry.Value.Value);
        }
    }

    /// <summary>Writes named floats, colors, and textures by resolved property id.</summary>
    /// <param name="writer">Host material update writer.</param>
    protected override void UpdateMaterial(ref MaterialUpdateWriter writer)
    {
        QueuePropertyIds();
        PushFloats(ref writer);
        PushColors(ref writer);
        PushTextures(ref writer);
        SyncAssignment();
    }

    private void QueuePropertyIds()
    {
        foreach (var entry in Floats)
            RequestId(entry.PropertyName);
        foreach (var entry in Colors)
            RequestId(entry.PropertyName);
        foreach (var entry in Textures)
            RequestId(entry.PropertyName);
    }

    private static void RequestId(Sync<string> name)
    {
        if (TryPropertyName(name, out var property))
            Ipc.MaterialPropertyIds.Request(property);
    }

    private void PushFloats(ref MaterialUpdateWriter writer)
    {
        foreach (var entry in Floats)
        {
            if (!TryPropertyName(entry.PropertyName, out var property))
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(property, out var propertyId))
                continue;
            writer.SetFloat(propertyId, entry.Value.Value);
        }
    }

    private void PushColors(ref MaterialUpdateWriter writer)
    {
        foreach (var entry in Colors)
        {
            if (!TryPropertyName(entry.PropertyName, out var property))
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(property, out var propertyId))
                continue;
            colorX color = entry.Value.Value;
            writer.SetColor(propertyId, in color);
        }
    }

    private void PushTextures(ref MaterialUpdateWriter writer)
    {
        foreach (var entry in Textures)
        {
            if (!TryPropertyName(entry.PropertyName, out var property))
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(property, out var propertyId))
                continue;
            writer.SetTexture(propertyId, entry.Texture.Target);
        }
    }

    private void SyncAssignment()
    {
        if (!Enabled.Value)
            return;

        var materialId = AssetId;
        if (materialId < 0)
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
        public readonly Sync<string> PropertyName;

        /// <summary>Value passed to <c>SetFloat</c>.</summary>
        public readonly Sync<float> Value;
    }

    /// <summary>One named color in <see cref="Colors"/>.</summary>
    public class ColorEntry : SyncObject
    {
        /// <summary>WGSL uniform name, for example <c>_Color</c>.</summary>
        public readonly Sync<string> PropertyName;

        /// <summary>Color passed to <c>SetColor</c>.</summary>
        public readonly Sync<colorX> Value;
    }

    /// <summary>One named texture in <see cref="Textures"/>.</summary>
    public class TextureEntry : SyncObject
    {
        /// <summary>WGSL texture name, for example <c>_MainTex</c>.</summary>
        public readonly Sync<string> PropertyName;

        /// <summary>Texture provider passed to <c>SetTexture</c>.</summary>
        public readonly AssetRef<ITexture2D> Texture;
    }

    /// <summary>One named keyword in <see cref="Keywords"/>.</summary>
    public class KeywordEntry : SyncObject
    {
        /// <summary><c>//#keyword</c> name.</summary>
        public readonly Sync<string> PropertyName;

        /// <summary>True enables the keyword.</summary>
        public readonly Sync<bool> Value;
    }
}
