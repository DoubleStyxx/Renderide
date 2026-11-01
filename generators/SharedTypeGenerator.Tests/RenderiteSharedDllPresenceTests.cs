using SharedTypeGenerator.Tests.Support;
using Xunit;

namespace SharedTypeGenerator.Tests;

/// <summary>
/// Guards the C#/Rust IPC roundtrip so a missing <c>Renderite.Shared.dll</c> fails on CI and skips only outside CI.
/// The roundtrip sources themselves reference Renderite.Shared types, so they are compiled only when the DLL is present.
/// </summary>
public sealed class RenderiteSharedDllPresenceTests
{
    /// <summary>
    /// Passes when the shared assembly is available. Fails when <c>CI</c> is set and the DLL is missing.
    /// Skips with an explanation only when the DLL is missing and <c>CI</c> is not set.
    /// </summary>
    [SkippableFact]
    public void Shared_dll_is_required_on_ci()
    {
        if (RenderiteSharedDllLocator.TryFind() != null)
            return;

        if (RenderiteSharedDllLocator.IsCiEnvironment())
        {
            throw new InvalidOperationException(
                "Renderite.Shared.dll was not found and CI is set. The C#/Rust IPC roundtrip must fail instead of skipping. "
                + "Set RENDERITE_SHARED_DLL to the Release build of Yellow-Dog-Man/Renderite Renderite.Shared "
                + "at commit 7ccfd2ee7c28648cb2e46dd4f52f51613a848071.");
        }

        Skip.If(
            true,
            "Renderite.Shared.dll was not found, so the C#/Rust IPC roundtrip tests were not run. "
            + "This skip happens only when CI is not set. Set RENDERITE_SHARED_DLL to a Release build of "
            + "Yellow-Dog-Man/Renderite Renderite.Shared at commit 7ccfd2ee7c28648cb2e46dd4f52f51613a848071, "
            + "or point RESONITE_DIR at an install that contains the DLL.");
    }
}
