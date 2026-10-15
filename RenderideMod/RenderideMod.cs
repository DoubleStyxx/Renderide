using HarmonyLib;
using ResoniteModLoader;

namespace RenderideMod;

/// <summary>
/// Mod entry point. Owns ResoniteModLoader metadata and the Harmony bootstrap;
/// all patch logic lives under <c>Patches/</c> and all IPC dispatch under <c>Ipc/</c>.
/// </summary>
public class RenderideMod : ResoniteMod
{
    /// <summary>Single source of truth for the mod version, mirrored into <see cref="System.Reflection.AssemblyVersionAttribute"/>.</summary>
    internal const string VERSION_CONSTANT = "0.1.0";

    /// <summary>Stable Harmony id used for <see cref="Harmony.PatchAll()"/> and any future selective unpatch.</summary>
    private const string HarmonyId = "dev.doublestyx.renderidemod";

    /// <inheritdoc />
    public override string Name => "RenderideMod";

    /// <inheritdoc />
    public override string Author => "DoubleStyx";

    /// <inheritdoc />
    public override string Version => VERSION_CONSTANT;

    /// <inheritdoc />
    public override string Link => "https://github.com/DoubleStyx/Renderide";

    /// <summary>
    /// Called by ResoniteModLoader once FrooxEngine has finished initialising.
    /// Applies every <c>[HarmonyPatch]</c> declared in this assembly.
    /// </summary>
    public override void OnEngineInit()
    {
        var harmony = new Harmony(HarmonyId);
        harmony.PatchAll();
        Msg($"RenderideMod {VERSION_CONSTANT} initialised.");
    }
}
