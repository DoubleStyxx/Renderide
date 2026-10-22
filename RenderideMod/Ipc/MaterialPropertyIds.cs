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
    private static readonly Dictionary<int, InflightRequest> Inflight = new();
    private static readonly Dictionary<MessagingManager, RenderCommandHandler> Installed = new();

    /// <summary>Flushes before the first retry of an unanswered request.</summary>
    private const int InitialRetryFlushes = 8;

    /// <summary>Largest gap between retries, in flushes.</summary>
    private const int MaxRetryFlushes = 512;

    /// <summary>Retry count that logs one warning. Later retries stay quiet.</summary>
    private const int WarnAfterRetries = 5;

    private static readonly Dictionary<int, string[]> Retired = new();
    private static readonly Dictionary<string, int> RetryCounts = new();
    private static readonly HashSet<string> ZeroIdWarned = new();

    private static int _nextRequestId = RequestIdBase;
    private static bool _hookMissLogged;

    private sealed class InflightRequest
    {
        public string[] Names = [];
        public int IdleFlushes;
        public int WaitFlushes = InitialRetryFlushes;
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
    /// Hash of <paramref name="names"/> plus each name's settled id.
    /// Unsettled names change the hash when a result arrives. Other names do not.
    /// </summary>
    internal static int StateFor(IReadOnlyList<string> names)
    {
        lock (Gate)
        {
            var hash = new HashCode();
            foreach (var name in names)
            {
                hash.Add(name);
                hash.Add(Ids.TryGetValue(name, out var propertyId) ? propertyId : int.MinValue);
            }

            return hash.ToHashCode();
        }
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

        string[]? names = null;
        var requestId = 0;
        var warnNames = new List<string>();
        var logRequest = false;
        lock (Gate)
        {
            RetireDueRequests(warnNames);
            if (Pending.Count == 0 || _nextRequestId == int.MaxValue)
            {
                names = null;
            }
            else
            {
                names = new string[Pending.Count];
                Pending.CopyTo(names);
                Pending.Clear();
                requestId = _nextRequestId++;
                var wait = InitialRetryFlushes;
                var quiet = true;
                foreach (var name in names)
                {
                    var retries = RetryCounts.TryGetValue(name, out var count) ? count : 0;
                    wait = Math.Max(wait, Backoff(retries));
                    if (retries < WarnAfterRetries)
                        quiet = false;
                }

                Inflight.Add(requestId, new InflightRequest
                {
                    Names = names,
                    WaitFlushes = wait,
                });
                logRequest = !quiet;
            }
        }

        foreach (var name in warnNames)
        {
            ResoniteMod.Warn(
                $"custom shader material property id: name={name} unresolved after {WarnAfterRetries} retries");
        }

        if (names is null)
            return;

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

        if (logRequest)
        {
            ResoniteMod.Msg(
                $"custom shader material property id request: requestId={requestId} count={names.Length}");
        }
    }

    /// <summary>
    /// Moves unanswered requests to <see cref="Retired"/> and queues names that are still unsettled.
    /// The old request id stays mapped so a late result can still fill those names.
    /// </summary>
    private static void RetireDueRequests(List<string> warnNames)
    {
        var due = new List<int>();
        foreach (var pair in Inflight)
        {
            pair.Value.IdleFlushes++;
            if (pair.Value.IdleFlushes >= pair.Value.WaitFlushes)
                due.Add(pair.Key);
        }

        foreach (var requestId in due)
        {
            var tracked = Inflight[requestId];
            Inflight.Remove(requestId);
            Retired[requestId] = tracked.Names;
            foreach (var name in tracked.Names)
            {
                if (Ids.ContainsKey(name))
                    continue;
                var retries = RetryCounts.TryGetValue(name, out var count) ? count + 1 : 1;
                RetryCounts[name] = retries;
                if (retries == WarnAfterRetries)
                    warnNames.Add(name);
                Pending.Add(name);
            }
        }

        DropSettledRetired();
    }

    private static int Backoff(int retries)
    {
        var shift = Math.Min(retries, 6);
        return Math.Min(InitialRetryFlushes << shift, MaxRetryFlushes);
    }

    private static void DropSettledRetired()
    {
        var done = new List<int>();
        foreach (var pair in Retired)
        {
            var settled = true;
            foreach (var name in pair.Value)
            {
                if (!Ids.ContainsKey(name))
                {
                    settled = false;
                    break;
                }
            }

            if (settled)
                done.Add(pair.Key);
        }

        foreach (var requestId in done)
            Retired.Remove(requestId);
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
        List<string>? zeroNames = null;
        lock (Gate)
        {
            string[]? names = null;
            if (Inflight.Remove(result.requestId, out var tracked))
                names = tracked.Names;
            else if (!Retired.Remove(result.requestId, out names))
                return;

            var ids = result.propertyIDs;
            for (var i = 0; i < names.Length; i++)
            {
                var name = names[i];
                if (Ids.ContainsKey(name))
                    continue;

                var propertyId = ids is not null && i < ids.Count ? ids[i] : 0;
                Ids[name] = propertyId;
                RetryCounts.Remove(name);
                if (propertyId > 0)
                {
                    ResoniteMod.Msg(
                        $"custom shader material property id: name={name} id={propertyId}");
                    continue;
                }

                if (ZeroIdWarned.Add(name))
                {
                    zeroNames ??= [];
                    zeroNames.Add(name);
                }
            }

            DropSettledRetired();
        }

        if (zeroNames is null)
            return;
        foreach (var name in zeroNames)
        {
            ResoniteMod.Warn(
                $"custom shader material property id: name={name} id=0");
        }
    }

    private static bool IsInflight(string name)
    {
        foreach (var tracked in Inflight.Values)
        {
            foreach (var pending in tracked.Names)
            {
                if (pending == name)
                    return true;
            }
        }

        return false;
    }
}
