namespace SharedTypeGenerator.Options;

/// <summary>Resolves <c>Renderite.Shared.dll</c> for <see cref="GeneratorOptions"/> when the CLI does not pass <c>-i</c> / <c>--assembly-path</c>, and validates explicit paths.</summary>
public static class AssemblyPathResolution
{
    /// <summary>
    /// Fills <see cref="GeneratorOptions.AssemblyPath"/> from discovery when empty, or normalizes and checks an explicit path.
    /// </summary>
    /// <param name="options">Options mutated on success when assembly path was empty.</param>
    /// <param name="error">Sink for user-facing errors (e.g. <see cref="Console.Error"/>).</param>
    /// <returns><see langword="true"/> when <paramref name="options"/>.AssemblyPath is set to an existing file; otherwise <see langword="false"/>.</returns>
    public static bool TryResolveOrValidate(GeneratorOptions options, TextWriter error)
    {
        if (string.IsNullOrWhiteSpace(options.AssemblyPath))
        {
            string? discovered = ResoniteAssemblyDiscovery.TryFindRenderiteSharedDll();
            if (discovered == null)
            {
                error.WriteLine(
                    "Could not find Renderite.Shared.dll. Set RENDERITE_SHARED_DLL or RESONITE_DIR, install Resonite via Steam, or pass -i / --assembly-path.");
                return false;
            }

            options.AssemblyPath = discovered;
            return true;
        }

        options.AssemblyPath = Path.GetFullPath(options.AssemblyPath.Trim());
        if (!File.Exists(options.AssemblyPath))
        {
            error.WriteLine($"Assembly path does not exist: {options.AssemblyPath}");
            return false;
        }

        return true;
    }
}
