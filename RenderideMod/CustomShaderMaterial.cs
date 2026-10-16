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
/// <see cref="Ipc.MaterialPropertyIds"/>. Keywords in the packaged Unlit set go through
/// <see cref="UpdateKeywords"/>. Any other keyword is a 0/1 float of the same name.
/// </para>
/// <para>
/// Enable and a non-empty <see cref="ShaderKey"/> assign. Disable, destroy, and a cleared key
/// call <see cref="CustomShaders.ClearMaterial"/>.
/// </para>
/// <para>
/// <see cref="VariablePrefix"/> reads dynamic variables from spaces on this slot and its parents.
/// Names that start with the prefix override <see cref="Floats"/>, <see cref="Colors"/>, and
/// <see cref="Textures"/> for the same property. Values travel on the same writer path.
/// </para>
/// </remarks>
[Category(new string[] { "Assets/Materials" })]
public partial class CustomShaderMaterial : SingleShaderMaterialProvider
{
    /// <summary>Installed custom program key. Empty clears the per-material override.</summary>
    public readonly Sync<string> ShaderKey;

    /// <summary>
    /// Name prefix for dynamic variables. Blank is <c>Shader/</c>.
    /// The property name is the canonical variable name with this prefix removed.
    /// </summary>
    public readonly Sync<string> VariablePrefix;

    /// <summary>Named floats. <see cref="FloatEntry.PropertyName"/> is the WGSL uniform name.</summary>
    public readonly SyncList<FloatEntry> Floats;

    /// <summary>Named colors. Uploaded with <c>SetColor</c>.</summary>
    public readonly SyncList<ColorEntry> Colors;

    /// <summary>Named textures. <see cref="TextureEntry.Texture"/> is the texture provider.</summary>
    public readonly SyncList<TextureEntry> Textures;

    /// <summary>
    /// Named shader keywords. Unlit metadata keywords use <see cref="UpdateKeywords"/>.
    /// Other names are uploaded as 0/1 floats.
    /// </summary>
    public readonly SyncList<KeywordEntry> Keywords;

    private readonly HashSet<string> _floatKeywords = new(StringComparer.Ordinal);
    private readonly HashSet<string> _clearKeywordFloats = new(StringComparer.Ordinal);
    private readonly HashSet<string> _writtenKeywordFloats = new(StringComparer.Ordinal);

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
    protected override void OnAttach()
    {
        base.OnAttach();
        VariablePrefix.Value = DefaultVariablePrefix;
    }

    /// <inheritdoc />
    protected override void OnAwake()
    {
        base.OnAwake();
        HookOwnSlot();
    }

    /// <inheritdoc />
    protected override void OnEnabled()
    {
        base.OnEnabled();
        QueuePropertyIds();
        SyncAssignment();
        BindDynVars();
    }

    /// <inheritdoc />
    protected override void OnChanges()
    {
        base.OnChanges();
        if (!Enabled.Value)
            return;
        QueuePropertyIds();
        SyncAssignment();
        if (ActivePrefix() != _dynPrefix)
            BindDynVars();
    }

    /// <inheritdoc />
    protected override void OnCommonUpdate()
    {
        base.OnCommonUpdate();
        if (!Enabled.Value)
            return;

        var generation = Ipc.MaterialPropertyIds.Generation;
        if (generation != _propertyGeneration || KeywordFloatsPending())
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
        UnbindDynVars();
        ClearBound();
        base.OnDisabled();
    }

    /// <inheritdoc />
    protected override void OnDestroy()
    {
        UnbindDynVars();
        UnhookOwnSlot();
        ClearBound();
        base.OnDestroy();
    }

    /// <summary>Sets keyword state from <see cref="Keywords"/>.</summary>
    /// <param name="keywords">Host keyword set for this material.</param>
    /// <remarks>
    /// <c>SetKeyword</c> only accepts names in the packaged Unlit metadata. A miss returns -1
    /// and the keyword is dropped. Those names are recorded for <see cref="PushKeywordFloats"/>.
    /// </remarks>
    protected override void UpdateKeywords(ShaderKeywords keywords)
    {
        var live = new HashSet<string>(StringComparer.Ordinal);
        foreach (var entry in Keywords)
        {
            if (!TryPropertyName(entry.PropertyName, out var keyword))
                continue;
            keywords.UpdateKeyword(keyword, entry.Value);
            if (keywords.SetKeyword(keyword, entry.Value.Value) >= 0)
                continue;
            live.Add(keyword);
            Ipc.MaterialPropertyIds.Request(keyword);
        }

        _floatKeywords.RemoveWhere(name =>
        {
            if (live.Contains(name))
                return false;
            _clearKeywordFloats.Add(name);
            return true;
        });
        foreach (var name in live)
            _floatKeywords.Add(name);
    }

    /// <summary>Writes named floats, colors, and textures by resolved property id.</summary>
    /// <param name="writer">Host material update writer.</param>
    protected override void UpdateMaterial(ref MaterialUpdateWriter writer)
    {
        QueuePropertyIds();
        PushFloats(ref writer);
        PushColors(ref writer);
        PushTextures(ref writer);
        PushKeywordFloats(ref writer);
        PushDynVars(ref writer);
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
        QueueDynVarPropertyIds();
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
            if (DynVarOverrides(property))
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
            if (DynVarOverrides(property))
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
            if (DynVarOverrides(property))
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(property, out var propertyId))
                continue;
            writer.SetTexture(propertyId, entry.Texture.Target);
        }
    }

    private void PushKeywordFloats(ref MaterialUpdateWriter writer)
    {
        var wroteAll = true;
        foreach (var entry in Keywords)
        {
            if (!TryPropertyName(entry.PropertyName, out var keyword))
                continue;
            if (!_floatKeywords.Contains(keyword))
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(keyword, out var propertyId))
            {
                wroteAll = false;
                continue;
            }

            writer.SetFloat(propertyId, entry.Value.Value ? 1f : 0f);
        }

        foreach (var keyword in _clearKeywordFloats)
        {
            if (!Ipc.MaterialPropertyIds.TryGet(keyword, out var propertyId))
            {
                wroteAll = false;
                continue;
            }

            writer.SetFloat(propertyId, 0f);
        }

        if (!wroteAll)
            return;

        _clearKeywordFloats.Clear();
        _writtenKeywordFloats.Clear();
        foreach (var keyword in _floatKeywords)
            _writtenKeywordFloats.Add(keyword);
    }

    private bool KeywordFloatsPending()
    {
        if (_clearKeywordFloats.Count > 0 || _writtenKeywordFloats.Count != _floatKeywords.Count)
            return true;
        foreach (var keyword in _floatKeywords)
        {
            if (!_writtenKeywordFloats.Contains(keyword))
                return true;
        }

        return false;
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
