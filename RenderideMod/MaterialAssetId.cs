using System.Reflection;
using HarmonyLib;
using ResoniteModLoader;

namespace RenderideMod;

/// <summary>
/// Reads the host render material asset id that <c>MaterialsUpdateBatch</c> sends as
/// <c>SelectTarget</c>. FrooxEngine does not document that id on <c>Material</c>, so this
/// walks the same private-member path as <see cref="Ipc.MessagingHostAccessor"/>.
/// </summary>
internal static class MaterialAssetId
{
    private static readonly string[] IdNames =
    [
        "AssetId",
        "RenderAssetId",
        "_assetId",
        "_renderAssetId",
    ];

    private static MemberInfo? _materialMember;
    private static bool _materialMemberSearched;
    private static MemberInfo? _connectorMember;
    private static bool _connectorMemberSearched;
    private static PropertyInfo? _connectorProperty;
    private static FieldInfo? _connectorField;
    private static bool _connectorHolderSearched;
    private static bool _hasConnectorHolder;
    private static bool _loggedMember;
    private static bool _warned;

    /// <summary>
    /// Writes the render material asset id when it is known and non-negative.
    /// </summary>
    /// <param name="material">Live <c>FrooxEngine.Material</c> instance.</param>
    /// <param name="assetId">SelectTarget id, when found.</param>
    internal static bool TryRead(object material, out int assetId)
    {
        assetId = -1;
        if (Read(material, ref _materialMember, ref _materialMemberSearched) is int materialId && materialId >= 0)
        {
            assetId = materialId;
            return true;
        }

        var connector = ConnectorOf(material);
        if (connector is not null
            && Read(connector, ref _connectorMember, ref _connectorMemberSearched) is int connectorId
            && connectorId >= 0)
        {
            assetId = connectorId;
            return true;
        }

        WarnIfMissing(connector);
        return false;
    }

    private static void WarnIfMissing(object? connector)
    {
        if (_warned || _materialMember is not null || _connectorMember is not null)
            return;
        if (!_materialMemberSearched || !_connectorHolderSearched)
            return;
        // A null connector instance can mean the render asset is not integrated yet.
        if (_hasConnectorHolder && connector is null && !_connectorMemberSearched)
            return;

        _warned = true;
        ResoniteMod.Warn(
            "CustomShaderMaterial: no int AssetId/RenderAssetId on the material or its connector. AssignMaterial was not called.");
    }

    private static int? Read(object target, ref MemberInfo? cache, ref bool searched)
    {
        if (!searched)
        {
            searched = true;
            cache = FindIntMember(target.GetType());
        }

        if (cache is null)
            return null;

        var value = cache switch
        {
            PropertyInfo property => property.GetValue(target),
            FieldInfo field => field.GetValue(target),
            _ => null,
        };
        if (value is not int id)
            return null;
        if (!_loggedMember && id >= 0)
        {
            _loggedMember = true;
            ResoniteMod.Msg(
                $"custom shader material asset id via {cache.DeclaringType?.Name}.{cache.Name}");
        }

        return id;
    }

    private static MemberInfo? FindIntMember(Type type)
    {
        foreach (var name in IdNames)
        {
            var property = AccessTools.Property(type, name);
            if (property is not null
                && property.PropertyType == typeof(int)
                && property.GetIndexParameters().Length == 0
                && property.GetGetMethod(nonPublic: true) is not null)
                return property;

            var field = AccessTools.Field(type, name);
            if (field is not null && field.FieldType == typeof(int))
                return field;
        }

        return null;
    }

    private static object? ConnectorOf(object material)
    {
        if (!_connectorHolderSearched)
        {
            _connectorHolderSearched = true;
            var type = material.GetType();
            var property = AccessTools.Property(type, "Connector");
            if (property?.GetGetMethod(nonPublic: true) is not null)
                _connectorProperty = property;
            _connectorField = AccessTools.Field(type, "_connector") ?? AccessTools.Field(type, "connector");
            _hasConnectorHolder = _connectorProperty is not null || _connectorField is not null;
        }

        if (_connectorProperty is not null)
            return _connectorProperty.GetValue(material);
        return _connectorField?.GetValue(material);
    }
}
