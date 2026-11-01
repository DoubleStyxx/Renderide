using Xunit;

namespace SharedTypeGenerator.Tests;

/// <summary>
/// Explicit expected-fail list for the C#/Rust IPC roundtrip.
/// </summary>
/// <remarks>
/// The C# reference is Yellow-Dog-Man/Renderite commit 7ccfd2ee7c28648cb2e46dd4f52f51613a848071
/// (MIT). <c>crates/renderide-shared/src/shared.rs</c> does not match that commit yet for the
/// types below: <c>VideoTextureLoad.additionalSource</c>, <c>FrameSubmitData.readbackTasks</c>,
/// the render-texture readback commands, and <c>FrameControllerState</c>.
/// Each listed case must fail the roundtrip. A listed case that passes is a stale marker and fails CI.
/// Remove an entry only in the contract change that updates <c>shared.rs</c>, after this judge list has been reviewed on its own.
/// </remarks>
public static class ExpectedContractDrift
{
    /// <summary>Type names that must disagree with the current Rust packer.</summary>
    public static readonly string[] TypeNames =
    [
        "FrameControllerState",
        "FrameSubmitData",
        "RenderTextureReadbackResult",
        "RenderTextureReadbackTask",
        "VideoTextureLoad",
    ];

    /// <summary>Returns <see langword="true"/> when <paramref name="typeName"/> is on the expected-fail list.</summary>
    public static bool IsMarked(string typeName)
    {
        foreach (string name in TypeNames)
        {
            if (string.Equals(name, typeName, StringComparison.Ordinal))
                return true;
        }

        return false;
    }

    /// <summary>
    /// Returns an error when a marked case passed. A real mismatch on a marked case returns <see langword="null"/>.
    /// </summary>
    public static string? Rejection(string typeName, bool roundtripPassed)
    {
        if (IsMarked(typeName) && roundtripPassed)
        {
            return typeName
                + " is on ExpectedContractDrift but the roundtrip passed. The marker is stale. "
                + "Remove it only in the contract change that updates shared.rs.";
        }

        return null;
    }
}

/// <summary>Locks the expected-fail rule so a stale marker fails even without the shared DLL.</summary>
public sealed class ExpectedContractDriftTests
{
    /// <summary>The tracked drift set is exactly the five Renderite.Shared mismatches.</summary>
    [Fact]
    public void Marker_list_is_the_known_contract_drift()
    {
        Assert.Equal(
            [
                "FrameControllerState",
                "FrameSubmitData",
                "RenderTextureReadbackResult",
                "RenderTextureReadbackTask",
                "VideoTextureLoad",
            ],
            ExpectedContractDrift.TypeNames);
    }

    /// <summary>A marked case that passes is rejected.</summary>
    [Fact]
    public void Stale_expected_fail_marker_is_rejected()
    {
        string? message = ExpectedContractDrift.Rejection("VideoTextureLoad", roundtripPassed: true);
        Assert.NotNull(message);
        Assert.Contains("stale", message, StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>A marked case that fails the roundtrip is the intended judge outcome.</summary>
    [Fact]
    public void Expected_fail_accepts_a_real_mismatch()
    {
        Assert.Null(ExpectedContractDrift.Rejection("FrameSubmitData", roundtripPassed: false));
    }

    /// <summary>Unmarked types are not treated as expected failures.</summary>
    [Fact]
    public void Unmarked_pass_is_not_a_stale_marker()
    {
        Assert.False(ExpectedContractDrift.IsMarked("KeepAlive"));
        Assert.Null(ExpectedContractDrift.Rejection("KeepAlive", roundtripPassed: true));
    }
}
