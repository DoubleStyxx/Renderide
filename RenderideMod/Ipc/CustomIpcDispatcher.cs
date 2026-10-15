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
    /// Today this is a boilerplate no-op — future custom renderer commands
    /// are sent here.
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

        // Future custom RendererCommand sends go here, e.g.:
        //   messagingHost.SendCommand(new MyCustomCommand(...), isBackground: false);
        //
        // Custom command types must derive from Renderite.Shared.RendererCommand
        // and be registered with the renderer's type table before the renderer
        // can decode them.
        _ = messagingHost;
    }
}
