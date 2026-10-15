using System.Reflection;
using Mono.Cecil;
using SharedTypeGenerator.Analysis;
using Xunit;

namespace SharedTypeGenerator.Tests.Unit;

/// <summary>Unit tests for <see cref="CecilLayoutInspector"/> explicit-layout metadata reads.</summary>
public sealed class CecilLayoutInspectorTests
{
    /// <summary>Explicit-layout structs are flagged via the IL <c>ExplicitLayout</c> attribute bit.</summary>
    [Fact]
    public void HasExplicitLayout_returns_true_for_explicit_layout_struct()
    {
        const string source = @"
using System.Runtime.InteropServices;
namespace LayoutAsm {
  [StructLayout(LayoutKind.Explicit, Size = 16)]
  public struct Explicit { [FieldOffset(0)] public int A; }
}";
        (Assembly reflection, AssemblyDefinition cecil) = TestCompilation.Compile(source);
        Type t = reflection.GetType("LayoutAsm.Explicit", throwOnError: true)!;
        Assert.True(CecilLayoutInspector.HasExplicitLayout(cecil, t));
    }

    /// <summary>Default-layout structs do not carry the explicit-layout flag.</summary>
    [Fact]
    public void HasExplicitLayout_returns_false_for_sequential_layout_struct()
    {
        const string source = @"
namespace LayoutAsm {
  public struct Plain { public int A; public int B; }
}";
        (Assembly reflection, AssemblyDefinition cecil) = TestCompilation.Compile(source);
        Type t = reflection.GetType("LayoutAsm.Plain", throwOnError: true)!;
        Assert.False(CecilLayoutInspector.HasExplicitLayout(cecil, t));
    }

    /// <summary>Enums are short-circuited regardless of their underlying layout encoding.</summary>
    [Fact]
    public void HasExplicitLayout_returns_false_for_enum()
    {
        const string source = @"
namespace LayoutAsm {
  public enum E : byte { A }
}";
        (Assembly reflection, AssemblyDefinition cecil) = TestCompilation.Compile(source);
        Type t = reflection.GetType("LayoutAsm.E", throwOnError: true)!;
        Assert.False(CecilLayoutInspector.HasExplicitLayout(cecil, t));
    }

    /// <summary>Reference types are filtered out before the IL probe.</summary>
    [Fact]
    public void HasExplicitLayout_returns_false_for_class_reference_type()
    {
        const string source = @"
namespace LayoutAsm {
  public sealed class Ref { public int A; }
}";
        (Assembly reflection, AssemblyDefinition cecil) = TestCompilation.Compile(source);
        Type t = reflection.GetType("LayoutAsm.Ref", throwOnError: true)!;
        Assert.False(CecilLayoutInspector.HasExplicitLayout(cecil, t));
    }

    /// <summary>The declared <c>Size</c> from <c>StructLayoutAttribute</c> is read out via the ClassLayout table.</summary>
    [Fact]
    public void GetExplicitLayoutSizeOrZero_reads_class_size_from_struct_layout()
    {
        const string source = @"
using System.Runtime.InteropServices;
namespace LayoutAsm {
  [StructLayout(LayoutKind.Explicit, Size = 16)]
  public struct Sized { [FieldOffset(0)] public int A; }
}";
        (Assembly reflection, AssemblyDefinition cecil) = TestCompilation.Compile(source);
        Type t = reflection.GetType("LayoutAsm.Sized", throwOnError: true)!;
        Assert.Equal(16, CecilLayoutInspector.GetExplicitLayoutSizeOrZero(cecil, t));
    }

    /// <summary>Explicit layout without an explicit <c>Size</c> reports zero.</summary>
    [Fact]
    public void GetExplicitLayoutSizeOrZero_returns_zero_when_no_size_specified()
    {
        const string source = @"
using System.Runtime.InteropServices;
namespace LayoutAsm {
  [StructLayout(LayoutKind.Explicit)]
  public struct UnsizedExplicit { [FieldOffset(0)] public int A; }
}";
        (Assembly reflection, AssemblyDefinition cecil) = TestCompilation.Compile(source);
        Type t = reflection.GetType("LayoutAsm.UnsizedExplicit", throwOnError: true)!;
        Assert.Equal(0, CecilLayoutInspector.GetExplicitLayoutSizeOrZero(cecil, t));
    }

    /// <summary>A type whose full name is not in the Cecil module reads back as zero (defensive zero-on-miss).</summary>
    [Fact]
    public void GetExplicitLayoutSizeOrZero_returns_zero_for_unknown_type_full_name()
    {
        const string sourceWithType = @"
namespace LayoutAsm {
  public struct Present { public int A; }
}";
        (Assembly reflection, AssemblyDefinition _) = TestCompilation.Compile(sourceWithType, "PresentAsm");
        Type t = reflection.GetType("LayoutAsm.Present", throwOnError: true)!;

        const string emptySource = @"
namespace OtherAsm {
  public struct Different { public int A; }
}";
        (_, AssemblyDefinition emptyCecil) = TestCompilation.Compile(emptySource, "EmptyAsm");

        Assert.Equal(0, CecilLayoutInspector.GetExplicitLayoutSizeOrZero(emptyCecil, t));
    }
}
