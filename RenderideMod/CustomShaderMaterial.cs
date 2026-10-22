using System.Reflection;
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
/// <see cref="UpdateKeywords"/>. Every keyword is also a 0/1 float of the same name.
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
    /// Named shader keywords. Each row is uploaded as a 0/1 float. <see cref="UpdateKeywords"/>
    /// also tracks the sync value for packaged Unlit while the override is not installed.
    /// </summary>
    public readonly SyncList<KeywordEntry> Keywords;

    private readonly HashSet<string> _floatKeywords = new(StringComparer.Ordinal);
    private readonly HashSet<string> _clearKeywordFloats = new(StringComparer.Ordinal);

    private static bool _propertiesInitialized;
    private int? _boundMaterialId;
    private string? _boundKey;
    private string? _rejectedKey;
    private int? _nameState;

    /// <summary>Sorted property names for <see cref="NameResolutionState"/>.</summary>
    private readonly List<string> _resolutionNames = new();

    /// <summary>True when <see cref="_resolutionNames"/> must be rebuilt from the sync lists.</summary>
    private bool _resolutionNamesDirty = true;

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
        EnsurePropertiesInitialized();
        HookOwnSlot();
    }

    /// <inheritdoc />
    protected override void OnEnabled()
    {
        base.OnEnabled();
        InvalidateResolutionNames();
        if (EnsurePropertiesInitialized())
            MarkChangeDirty();
        QueuePropertyIds();
        SyncAssignment();
        BindDynVars();
    }

    /// <inheritdoc />
    protected override void OnChanges()
    {
        base.OnChanges();
        InvalidateResolutionNames();
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

        FlushDynVarValues();
        EnsurePropertiesInitialized();
        QueuePropertyIds();
        var nameState = NameResolutionState();
        if (_nameState != nameState)
        {
            _nameState = nameState;
            MarkChangeDirty();
        }

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
    /// Every row is uploaded as a 0/1 float from <see cref="PushKeywordFloats"/>. The override
    /// path ignores host variant bits, so a name <c>SetKeyword</c> accepts must still be a float.
    /// <see cref="ShaderKeywords.UpdateKeyword"/> is the only host call: it tracks the
    /// <see cref="Sync{T}"/> while this material is still drawing packaged Unlit.
    /// <c>SetKeyword</c> is not also called; it would snapshot the bool a second time.
    /// </remarks>
    protected override void UpdateKeywords(ShaderKeywords keywords)
    {
        var live = new HashSet<string>(StringComparer.Ordinal);
        foreach (var entry in Keywords)
        {
            if (!TryPropertyName(entry.PropertyName, out var keyword))
                continue;
            keywords.UpdateKeyword(keyword, entry.Value);
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
        foreach (var entry in Keywords)
            RequestId(entry.PropertyName);
        QueueDynVarPropertyIds();
    }

    /// <summary>
    /// Stock property init only flips <see cref="PropertiesInitialized"/> after it resolves
    /// static <c>MaterialProperty</c> fields. This type has none, so that init never sends a
    /// request and never sets the flag. The host then leaves <see cref="AssetId"/> at -1 and
    /// never calls <see cref="UpdateMaterial"/>. Setting the flag lets the packaged Unlit
    /// upload run; named rows still wait on <see cref="Ipc.MaterialPropertyIds"/>.
    /// </summary>
    /// <returns>True when this call stored the flag.</returns>
    private bool EnsurePropertiesInitialized()
    {
        if (PropertiesInitialized || HasStaticMaterialProperties())
            return false;
        PropertiesInitialized = true;
        return true;
    }

    private static bool HasStaticMaterialProperties()
    {
        const BindingFlags flags = BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
        foreach (var field in typeof(CustomShaderMaterial).GetFields(flags))
        {
            if (field.FieldType.Name == "MaterialProperty")
                return true;
        }

        return false;
    }

    /// <summary>
    /// Hash of this material's property names and their settled ids.
    /// Another component's results do not change it.
    /// The sorted name list is rebuilt after sync-list changes and after dynvar names
    /// change. <see cref="BindDynVars"/> rebuilds <c>_dynVarNames</c> through
    /// <c>RebuildDynVarNames</c>, and <see cref="UnbindDynVars"/> clears it. Both of those
    /// call <see cref="InvalidateResolutionNames"/> because neither runs through
    /// <see cref="OnChanges"/>. A value-only resample does not.
    /// </summary>
    private int NameResolutionState()
    {
        if (_resolutionNamesDirty)
        {
            _resolutionNames.Clear();
            CollectPropertyNames(_resolutionNames);
            _resolutionNames.Sort(StringComparer.Ordinal);
            _resolutionNamesDirty = false;
        }

        return Ipc.MaterialPropertyIds.StateFor(_resolutionNames);
    }

    private void InvalidateResolutionNames()
    {
        _resolutionNamesDirty = true;
    }

    private void CollectPropertyNames(List<string> names)
    {
        foreach (var entry in Floats)
            AddPropertyName(names, entry.PropertyName);
        foreach (var entry in Colors)
            AddPropertyName(names, entry.PropertyName);
        foreach (var entry in Textures)
            AddPropertyName(names, entry.PropertyName);
        foreach (var entry in Keywords)
            AddPropertyName(names, entry.PropertyName);
        CollectDynVarPropertyNames(names);
    }

    private static void AddPropertyName(List<string> names, Sync<string> name)
    {
        if (TryPropertyName(name, out var property))
            names.Add(property);
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
                if (!Ipc.MaterialPropertyIds.IsSettled(keyword))
                    wroteAll = false;
                continue;
            }

            writer.SetFloat(propertyId, entry.Value.Value ? 1f : 0f);
        }

        foreach (var keyword in _clearKeywordFloats)
        {
            if (!Ipc.MaterialPropertyIds.TryGet(keyword, out var propertyId))
            {
                if (!Ipc.MaterialPropertyIds.IsSettled(keyword))
                    wroteAll = false;
                continue;
            }

            writer.SetFloat(propertyId, 0f);
        }

        if (!wroteAll)
            return;

        _clearKeywordFloats.Clear();
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
