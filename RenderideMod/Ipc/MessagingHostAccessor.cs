using System.Reflection;
using FrooxEngine;
using HarmonyLib;

namespace RenderideMod.Ipc;

/// <summary>
/// Isolates the reflection needed to reach the private
/// <c>RenderSystem._messagingHost</c> field. Centralising the access here
/// means a future FrooxEngine change that exposes the field publicly only
/// requires editing this one file.
/// </summary>
internal static class MessagingHostAccessor
{
    /// <summary>Cached <see cref="FieldInfo"/> for <c>RenderSystem._messagingHost</c>; resolved once at type-init.</summary>
    private static readonly FieldInfo? Field =
        AccessTools.Field(typeof(RenderSystem), "_messagingHost");

    /// <summary>
    /// Returns the live <see cref="RenderiteMessagingHost"/> owned by
    /// <paramref name="renderSystem"/>, or <c>null</c> if reflection failed
    /// (field renamed, removed, or assembly mismatch).
    /// </summary>
    /// <param name="renderSystem">The engine's render system instance.</param>
    internal static RenderiteMessagingHost? Get(RenderSystem renderSystem)
        => Field?.GetValue(renderSystem) as RenderiteMessagingHost;
}
