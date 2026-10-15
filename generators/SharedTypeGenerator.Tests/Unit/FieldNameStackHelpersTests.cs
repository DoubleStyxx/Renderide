using SharedTypeGenerator.Analysis;
using Xunit;

namespace SharedTypeGenerator.Tests.Unit;

/// <summary>Unit tests for <see cref="FieldNameStackHelpers"/>.</summary>
public sealed class FieldNameStackHelpersTests
{
    /// <summary>An empty stack returns the documented sentinel without throwing.</summary>
    [Fact]
    public void PopLastFieldAndClear_returns_unknown_for_empty_stack()
    {
        var stack = new Stack<string>();

        string result = FieldNameStackHelpers.PopLastFieldAndClear(stack);

        Assert.Equal("_unknown", result);
        Assert.Empty(stack);
    }

    /// <summary>Returns the top entry and clears any remaining entries (one field per pack op).</summary>
    [Fact]
    public void PopLastFieldAndClear_returns_top_and_clears()
    {
        var stack = new Stack<string>();
        stack.Push("a");
        stack.Push("b");
        stack.Push("c");

        string result = FieldNameStackHelpers.PopLastFieldAndClear(stack);

        Assert.Equal("c", result);
        Assert.Empty(stack);
    }
}
