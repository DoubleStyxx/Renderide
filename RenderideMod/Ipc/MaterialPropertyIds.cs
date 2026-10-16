using System.Reflection;
using FrooxEngine;
using Renderite.Shared;
using ResoniteModLoader;

namespace RenderideMod.Ipc;

/// <summary>
/// Resolves runtime material property names to renderer property ids.
/// </summary>
/// <remarks>
/// Stock materials declare static <c>MaterialProperty</c> fields. <c>MaterialProviderBase</c>
/// reflects those once per type and <c>RenderMaterialManager</c> fills them with
/// <see cref="MaterialPropertyIdRequest"/>. Named list rows are not static fields, so this
/// cache sends the same command for names it has not seen and applies
/// <see cref="MaterialPropertyIdResult"/> for its own request ids.
/// </remarks>
internal static class MaterialPropertyIds
{
    /// <summary>First request id. Stays above the small ids <c>RenderMaterialManager</c> allocates.</summary>
    private const int RequestIdBase = 1 << 30;

    private static readonly object Gate = new();
    private static readonly Dictionary<string, int> Ids = new();
    private static readonly HashSet<string> Pending = new();
    private static readonly Dictionary<int, string[]> Inflight = new();
    private static readonly RenderCommandHandler ObserveHandler = Observe;

    private static int _nextRequestId = RequestIdBase;
    private static int _generation;
    private static bool _hookMissLogged;

    /// <summary>Bumps when a result is stored. Components dirty themselves when it changes.</summary>
    internal static int Generation
    {
        get
        {
            lock (Gate)
                return _generation;
        }
    }

    /// <summary>Queues <paramref name="name"/> until a positive property id is known.</summary>
    internal static void Request(string name)
    {
        if (string.IsNullOrEmpty(name))
            return;

        lock (Gate)
        {
            if (Ids.ContainsKey(name) || Pending.Contains(name) || IsInflight(name))
                return;
            Pending.Add(name);
        }
    }

    /// <summary>Reads a resolved id. Zero and missing names return false.</summary>
    internal static bool TryGet(string name, out int propertyId)
    {
        lock (Gate)
        {
            if (Ids.TryGetValue(name, out propertyId) && propertyId > 0)
                return true;
        }

        propertyId = 0;
        return false;
    }

    /// <summary>
    /// Hooks the host receive handler, then sends one <see cref="MaterialPropertyIdRequest"/>
    /// for every name still pending.
    /// </summary>
    internal static void Flush(RenderiteMessagingHost messagingHost)
    {
        if (!TryHook(messagingHost))
            return;

        string[] names;
        int requestId;
        lock (Gate)
        {
            if (Pending.Count == 0)
                return;
            if (_nextRequestId == int.MaxValue)
                return;

            names = new string[Pending.Count];
            Pending.CopyTo(names);
            Pending.Clear();
            requestId = _nextRequestId++;
            Inflight.Add(requestId, names);
        }

        var request = new MaterialPropertyIdRequest { requestId = requestId };
        request.propertyNames.AddRange(names);
        try
        {
            messagingHost.SendCommand(request, isBackground: true);
        }
        catch (Exception ex)
        {
            lock (Gate)
            {
                Inflight.Remove(requestId);
                foreach (var name in names)
                    Pending.Add(name);
            }

            ResoniteMod.Warn($"custom shader material property id request failed: {ex.Message}");
            return;
        }

        ResoniteMod.Msg(
            $"custom shader material property id request: requestId={requestId} count={names.Length}");
    }

    private static bool TryHook(RenderiteMessagingHost messagingHost)
    {
        var manager = FindManager(messagingHost);
        if (manager is null)
        {
            if (!_hookMissLogged)
            {
                _hookMissLogged = true;
                ResoniteMod.Warn(
                    "CustomShaderMaterial: MessagingManager was not found on the render host. Property id requests are waiting.");
            }

            return false;
        }

        var current = manager.CommandHandler;
        if (current is null)
            return false;
        if (ContainsObserve(current))
            return true;

        manager.CommandHandler = ObserveHandler + current;
        return true;
    }

    private static bool ContainsObserve(RenderCommandHandler handler)
    {
        foreach (var entry in handler.GetInvocationList())
        {
            if (entry.Method == ObserveHandler.Method)
                return true;
        }

        return false;
    }

    private static MessagingManager? FindManager(object root)
    {
        const BindingFlags flags = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
        foreach (var field in root.GetType().GetFields(flags))
        {
            object? value;
            try
            {
                value = field.GetValue(root);
            }
            catch (TargetException)
            {
                continue;
            }

            if (value is MessagingManager manager)
                return manager;
        }

        foreach (var property in root.GetType().GetProperties(flags))
        {
            if (!property.CanRead || property.GetIndexParameters().Length != 0)
                continue;
            try
            {
                if (property.GetValue(root) is MessagingManager manager)
                    return manager;
            }
            catch (TargetInvocationException)
            {
            }
        }

        return null;
    }

    private static void Observe(RendererCommand command, int messageSize)
    {
        _ = messageSize;
        if (command is not MaterialPropertyIdResult result)
            return;

        lock (Gate)
        {
            if (!Inflight.Remove(result.requestId, out var names))
                return;

            var ids = result.propertyIDs;
            for (var i = 0; i < names.Length; i++)
            {
                var propertyId = ids is not null && i < ids.Count ? ids[i] : 0;
                Ids[names[i]] = propertyId;
                ResoniteMod.Msg(
                    $"custom shader material property id: name={names[i]} id={propertyId}");
            }

            _generation++;
        }
    }

    private static bool IsInflight(string name)
    {
        foreach (var names in Inflight.Values)
        {
            foreach (var pending in names)
            {
                if (pending == name)
                    return true;
            }
        }

        return false;
    }
}
