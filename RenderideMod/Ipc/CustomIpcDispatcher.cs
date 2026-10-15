using FrooxEngine;
using ResoniteModLoader;

namespace RenderideMod.Ipc;

/// <summary>
/// Single sending surface for any custom IPC traffic this mod produces.
/// Patches never call <c>SendCommand</c> directly; they go through this type
/// so that command construction, queue selection, and error handling stay in
/// one place.
/// </summary>
internal static class CustomIpcDispatcher
{
    /// <summary>
    /// Invoked from the <see cref="Patches.RenderSystemSubmitFramePatch"/>
    /// postfix, after FrooxEngine has finished sending its own frame data.
    /// Flushes custom shader uploads queued by <see cref="RenderideMod.CustomShaders"/>.
    /// </summary>
    /// <param name="renderSystem">The engine's active render system.</param>
    internal static void DispatchAfterFrameSubmit(RenderSystem renderSystem)
    {
        var messagingHost = MessagingHostAccessor.Get(renderSystem);
        if (messagingHost is null)
        {
            ResoniteMod.Warn("RenderideMod: _messagingHost was unavailable; skipping custom IPC.");
            return;
        }

        CustomShaderQueue.Flush(messagingHost);
    }
}
