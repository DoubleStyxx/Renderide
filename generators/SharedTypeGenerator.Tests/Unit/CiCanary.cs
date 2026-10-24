using Xunit;

namespace SharedTypeGenerator.Tests.Unit;

/// <summary>CI canary. This file does not compile.</summary>
public sealed class CiCanary
{
    /// <summary>References a type that does not exist.</summary>
    [Fact]
    public void Broken()
    {
        CiCanaryMissingType value = null;
    }
}
