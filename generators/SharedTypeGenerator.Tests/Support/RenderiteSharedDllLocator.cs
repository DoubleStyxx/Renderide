using SharedTypeGenerator.Options;

namespace SharedTypeGenerator.Tests.Support;

/// <summary>
/// Resolves <c>Renderite.Shared.dll</c> for the IPC roundtrip suite.
/// Discovery matches the test project probes: <c>RENDERITE_SHARED_DLL</c>, Resonite/Steam search, then
/// <c>generators/SharedTypeGenerator.Tests/lib/Renderite.Shared.dll</c>.
/// </summary>
public static class RenderiteSharedDllLocator
{
    /// <summary>Returns <see langword="true"/> when <c>CI</c> is <c>true</c> or <c>1</c>.</summary>
    public static bool IsCiEnvironment()
    {
        string? ci = Environment.GetEnvironmentVariable("CI");
        return string.Equals(ci, "true", StringComparison.OrdinalIgnoreCase)
            || string.Equals(ci, "1", StringComparison.Ordinal);
    }

    /// <summary>Returns the absolute path of <c>Renderite.Shared.dll</c>, or <see langword="null"/> when it is not available.</summary>
    public static string? TryFind()
    {
        string? fromEnv = Environment.GetEnvironmentVariable(ResoniteAssemblyDiscovery.RenderiteSharedDllEnvVar);
        if (!string.IsNullOrEmpty(fromEnv) && File.Exists(fromEnv))
            return Path.GetFullPath(fromEnv);

        string? discovered = ResoniteAssemblyDiscovery.TryFindRenderiteSharedDll();
        if (discovered != null)
            return discovered;

        string libPath = Path.Combine(
            AppContext.BaseDirectory,
            "..",
            "..",
            "..",
            "lib",
            ResoniteAssemblyDiscovery.SharedDllFileName);
        string fullLib = Path.GetFullPath(libPath);
        if (File.Exists(fullLib))
            return fullLib;

        return null;
    }
}
