using System.Collections.Generic;
using System.Globalization;
using FrooxEngine;
using Renderite.Shared;

namespace RenderideMod.Ipc;

/// <summary>
/// Queues <see cref="ShaderUpload"/> payloads until the render system's messaging
/// host exists, then flushes them on the background command queue.
/// </summary>
internal static class CustomShaderQueue
{
    private const string PayloadPrefix = "renderide-custom-wgsl:v1\n";

    private static readonly object Gate = new();
    private static readonly Queue<ShaderUpload> Pending = new();
    private static int nextCarrierAssetId = -1;

    /// <summary>Queues an install for <paramref name="shaderKey"/>.</summary>
    internal static void EnqueueInstall(int? assetId, string shaderKey, string wgsl)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(shaderKey);
        ArgumentException.ThrowIfNullOrWhiteSpace(wgsl);
        Enqueue(assetId, FormatInstall(shaderKey, wgsl));
    }

    /// <summary>Queues removal of the custom program for <paramref name="shaderKey"/>.</summary>
    internal static void EnqueueRemove(int? assetId, string shaderKey)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(shaderKey);
        Enqueue(assetId, FormatRemoval(shaderKey));
    }

    /// <summary>
    /// Queues a per-material assign. <paramref name="shaderKey"/> is the custom program key.
    /// The <c>ShaderUpload</c> asset id is a negative carrier; the material id is the payload header.
    /// </summary>
    internal static void EnqueueAssign(int materialAssetId, string shaderKey)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(shaderKey);
        Enqueue(null, FormatAssign(materialAssetId, shaderKey));
    }

    /// <summary>
    /// Queues a per-material clear. The <c>ShaderUpload</c> asset id is a negative carrier.
    /// </summary>
    internal static void EnqueueClear(int materialAssetId)
    {
        Enqueue(null, FormatClear(materialAssetId));
    }

    /// <summary>Sends every queued upload. Later uploads stay queued when the host is missing.</summary>
    internal static void Flush(RenderiteMessagingHost? messagingHost)
    {
        if (messagingHost is null)
            return;

        List<ShaderUpload> batch;
        lock (Gate)
        {
            if (Pending.Count == 0)
                return;
            batch = new List<ShaderUpload>(Pending.Count);
            while (Pending.Count > 0)
                batch.Add(Pending.Dequeue());
        }

        foreach (var upload in batch)
        {
            messagingHost.SendCommand(upload, isBackground: true);
            ResoniteModLoader.ResoniteMod.Msg(
                $"custom shader upload sent: assetId={upload.assetId} bytes={upload.file?.Length ?? 0}");
        }
    }

    private static void Enqueue(int? assetId, string file)
    {
        var upload = new ShaderUpload
        {
            assetId = assetId ?? NextCarrierAssetId(),
            file = file,
        };
        lock (Gate)
        {
            Pending.Enqueue(upload);
        }
    }

    private static int NextCarrierAssetId()
    {
        lock (Gate)
        {
            var assetId = nextCarrierAssetId;
            nextCarrierAssetId--;
            return assetId;
        }
    }

    private static string FormatInstall(string shaderKey, string wgsl)
    {
        var key = HeaderKey(shaderKey);
        return $"{PayloadPrefix}key={key}\n---\n{wgsl}";
    }

    private static string FormatRemoval(string shaderKey)
    {
        var key = HeaderKey(shaderKey);
        return $"{PayloadPrefix}key={key}\nremove=1\n---\n";
    }

    private static string FormatAssign(int materialAssetId, string shaderKey)
    {
        var key = HeaderKey(shaderKey);
        var material = materialAssetId.ToString(CultureInfo.InvariantCulture);
        return $"{PayloadPrefix}op=assign\nmaterial={material}\nkey={key}\n---\n";
    }

    private static string FormatClear(int materialAssetId)
    {
        var material = materialAssetId.ToString(CultureInfo.InvariantCulture);
        return $"{PayloadPrefix}op=clear\nmaterial={material}\n---\n";
    }

    private static string HeaderKey(string shaderKey)
    {
        var key = shaderKey.Trim();
        if (key.Contains('\n') || key.Contains('\r'))
            throw new ArgumentException("Shader key cannot contain a newline.", nameof(shaderKey));
        return key;
    }
}
