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
/// cache sends the same command for names it has not seen. Results come back on the
/// host <c>_background</c> queue. Request ids at or above <c>1&lt;&lt;30</c> are consumed
/// here and are not forwarded to <c>RenderMaterialManager</c>.
/// </remarks>
internal static class MaterialPropertyIds
{
    /// <summary>First request id. Stays above the small ids <c>RenderMaterialManager</c> allocates.</summary>
    private const int RequestIdBase = 1 << 30;

    private static readonly object Gate = new();
    private static readonly Dictionary<string, int> Ids = new();
    private static readonly HashSet<string> Pending = new();
    private static readonly Dictionary<int, string[]> Inflight = new();
    private static readonly Dictionary<MessagingManager, RenderCommandHandler> Installed = new();

    /// <summary>
    /// Flushes with an unanswered request before those names are sent again.
    /// The background result normally lands within a frame. A missed handler
    /// install must not leave the name inflight forever.
    /// </summary>
    private const int RetryAfterIdleFlushes = 8;

    private static int _nextRequestId = RequestIdBase;
    private static int _generation;
    private static int _idleFlushes;
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
    /// True once a result has stored <paramref name="name"/>, including a non-positive id.
    /// Callers stop waiting on names that will never become writable.
    /// </summary>
    internal static bool IsSettled(string name)
    {
        lock (Gate)
            return Ids.ContainsKey(name);
    }

    /// <summary>
    /// Hooks <c>_background</c> (and <c>_primary</c> when present), then sends one
    /// <see cref="MaterialPropertyIdRequest"/> for every name still pending.
    /// Property id results arrive on the background queue.
    /// </summary>
    internal static void Flush(RenderiteMessagingHost messagingHost)
    {
        if (!TryHook(messagingHost))
            return;

        string[] names;
        int requestId;
        lock (Gate)
        {
            if (Pending.Count == 0 && Inflight.Count > 0)
            {
                _idleFlushes++;
                if (_idleFlushes >= RetryAfterIdleFlushes)
                {
                    foreach (var inflight in Inflight.Values)
                    {
                        foreach (var name in inflight)
                            Pending.Add(name);
                    }

                    Inflight.Clear();
                    _idleFlushes = 0;
                }
            }

            if (Pending.Count == 0)
                return;
            if (_nextRequestId == int.MaxValue)
                return;

            _idleFlushes = 0;
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
        var background = ManagerField(messagingHost, "_background");
        if (background is null)
        {
            if (!_hookMissLogged)
            {
                _hookMissLogged = true;
                ResoniteMod.Warn(
                    "CustomShaderMaterial: _background MessagingManager was not found on the render host. Property id requests are waiting.");
            }

            return false;
        }

        if (!Install(background))
            return false;

        var primary = ManagerField(messagingHost, "_primary");
        if (primary is not null)
            Install(primary);
        return true;
    }

    /// <summary>
    /// Replaces <see cref="MessagingManager.CommandHandler"/> with a wrapper that keeps
    /// mod property-id results (request ids at or above <see cref="RequestIdBase"/>) and
    /// forwards every other command. The stock handler indexes only ids it allocated.
    /// </summary>
    private static bool Install(MessagingManager manager)
    {
        var current = manager.CommandHandler;
        if (current is null)
            return false;
        if (Installed.TryGetValue(manager, out var installed) && ReferenceEquals(current, installed))
            return true;

        RenderCommandHandler forward = current;
        RenderCommandHandler wrapped = (command, messageSize) =>
        {
            if (command is MaterialPropertyIdResult result && result.requestId >= RequestIdBase)
            {
                Apply(result);
                return;
            }

            forward(command, messageSize);
        };
        manager.CommandHandler = wrapped;
        Installed[manager] = wrapped;
        return true;
    }

    private static MessagingManager? ManagerField(RenderiteMessagingHost host, string fieldName)
    {
        var field = host.GetType().GetField(
            fieldName,
            BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic);
        if (field is null)
            return null;

        try
        {
            return field.GetValue(host) as MessagingManager;
        }
        catch (TargetException)
        {
            return null;
        }
    }

    private static void Apply(MaterialPropertyIdResult result)
    {
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
