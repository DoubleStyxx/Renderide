using System;
using FrooxEngine;
using HarmonyLib;
using RenderideMod.Ipc;
using ResoniteModLoader;

namespace RenderideMod.Patches;

/// <summary>
/// Harmony postfix on <see cref="RenderSystem.SubmitFrame"/>. Runs immediately
/// after FrooxEngine finishes submitting its own per-frame IPC traffic, which
/// is the correct moment to enqueue piggyback custom renderer commands on the
/// same primary/background queues the engine just used.
/// </summary>
/// <remarks>
/// This patch is intentionally a thin shim: it owns the Harmony attribute and
/// the rendering-state guard, and delegates all sending logic to
/// <see cref="CustomIpcDispatcher"/>. Adding a second hook (e.g. on engine
/// shutdown) should be a new file, not an edit to this one.
/// </remarks>
[HarmonyPatch(typeof(RenderSystem), nameof(RenderSystem.SubmitFrame))]
internal static class RenderSystemSubmitFramePatch
{
    /// <summary>
    /// Runs after the original <see cref="RenderSystem.SubmitFrame"/>. The
    /// state guard mirrors the original method's own early-out so the postfix
    /// only fires on real, in-flight frames.
    /// </summary>
    /// <param name="__instance">The active <see cref="RenderSystem"/> for this engine.</param>
    [HarmonyPostfix]
    private static void Postfix(RenderSystem __instance)
    {
        if (__instance.State != RendererState.Rendering)
            return;

        try
        {
            CustomIpcDispatcher.DispatchAfterFrameSubmit(__instance);
        }
        catch (Exception ex)
        {
            ResoniteMod.Error($"RenderideMod postfix failed: {ex}");
        }
    }
}
