using System.Reflection;
using SharedTypeGenerator.Analysis;
using Xunit;

namespace SharedTypeGenerator.Tests.Unit;

/// <summary>Unit tests for <see cref="PodAnalyzer"/> blittability classification.</summary>
public sealed class PodAnalyzerTests
{
    private static readonly Assembly TestAssembly = typeof(PodAnalyzerTests).Assembly;

    private enum SampleEnum
    {
        A,
    }

    private struct PodFields
    {
        internal int A = 0;
        internal float B = 0f;

        public PodFields() { }
    }

    private struct WithString
    {
        internal string Text = string.Empty;

        public WithString() { }
    }

    private struct GlamWrapper
    {
        internal RenderQuaternion Q = default;

        public GlamWrapper() { }
    }

    private struct GlamComposite
    {
        internal RenderQuaternion Q = default;
        internal int Tag = 0;

        public GlamComposite() { }
    }

    private struct VecComposite
    {
        internal RenderVector2 V = default;
        internal int Tag = 0;

        public VecComposite() { }
    }

    private struct RenderQuaternion
    {
    }

    private struct RenderVector2
    {
    }

    /// <summary>Primitives, <see cref="Guid"/>, and <see cref="bool"/> are all blittable under managed-layout pod rules.</summary>
    [Theory]
    [InlineData(typeof(byte))]
    [InlineData(typeof(int))]
    [InlineData(typeof(uint))]
    [InlineData(typeof(long))]
    [InlineData(typeof(float))]
    [InlineData(typeof(double))]
    [InlineData(typeof(bool))]
    [InlineData(typeof(Guid))]
    public void IsFieldTypePod_primitives_are_pod(Type type)
    {
        Assert.True(PodAnalyzer.IsFieldTypePod(type, []));
    }

    /// <summary>Enums are pod regardless of underlying type.</summary>
    [Fact]
    public void IsFieldTypePod_enum_is_pod()
    {
        Assert.True(PodAnalyzer.IsFieldTypePod(typeof(SampleEnum), []));
    }

    /// <summary>Reference types like <see cref="string"/> are not pod.</summary>
    [Fact]
    public void IsFieldTypePod_string_is_not_pod()
    {
        Assert.False(PodAnalyzer.IsFieldTypePod(typeof(string), []));
    }

    /// <summary>Value structs whose fields are all pod are themselves pod.</summary>
    [Fact]
    public void IsFieldTypePod_value_struct_with_pod_fields_is_pod()
    {
        Assert.True(PodAnalyzer.IsFieldTypePod(typeof(PodFields), []));
    }

    /// <summary>A single non-blittable field disqualifies the enclosing struct.</summary>
    [Fact]
    public void IsFieldTypePod_value_struct_with_string_field_is_not_pod()
    {
        Assert.False(PodAnalyzer.IsFieldTypePod(typeof(WithString), []));
    }

    /// <summary>Already-visited value types short-circuit through the non-pod fall-through to break cycles.</summary>
    [Fact]
    public void IsFieldTypePod_recursive_visited_short_circuits()
    {
        var visited = new HashSet<Type> { typeof(PodFields) };
        Assert.False(PodAnalyzer.IsFieldTypePod(typeof(PodFields), visited));
    }

    /// <summary>Restricted-variant enums are not eligible for whole-struct Rust Pod (validated via MemoryPackable instead).</summary>
    [Fact]
    public void IsRustLayoutPodField_enum_is_not_rust_pod()
    {
        Assert.False(PodAnalyzer.IsRustLayoutPodField(typeof(SampleEnum), [], TestAssembly));
    }

    /// <summary>Single-field glam wrappers stay whole-struct Pod (the <c>fields.Length == 1</c> branch).</summary>
    [Fact]
    public void IsRustLayoutPodField_glam_single_field_wrapper_is_pod()
    {
        Assert.True(PodAnalyzer.IsRustLayoutPodField(typeof(GlamWrapper), [], TestAssembly));
    }

    /// <summary>Composites mixing a SIMD-aligned glam type with other fields are not whole-struct Pod.</summary>
    [Fact]
    public void IsRustLayoutPodField_glam_quat_in_composite_is_not_pod()
    {
        Assert.False(PodAnalyzer.IsRustLayoutPodField(typeof(GlamComposite), [], TestAssembly));
    }

    /// <summary>Glam types not in the composite-non-pod set (e.g., <c>Vec2</c>) compose normally.</summary>
    [Fact]
    public void IsRustLayoutPodField_glam_vec2_composite_is_pod()
    {
        Assert.True(PodAnalyzer.IsRustLayoutPodField(typeof(VecComposite), [], TestAssembly));
    }

    /// <summary>Plain primitive fields propagate through to the underlying pod check.</summary>
    [Fact]
    public void IsRustLayoutPodField_primitive_passes_through()
    {
        Assert.True(PodAnalyzer.IsRustLayoutPodField(typeof(int), [], TestAssembly));
    }
}
