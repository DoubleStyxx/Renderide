using System.Globalization;
using System.Text;
using SharedTypeGenerator.Emission;
using Xunit;

namespace SharedTypeGenerator.Tests.Unit;

/// <summary>Unit tests for <see cref="RustWriter"/> indentation and emission helpers.</summary>
public sealed class RustWriterTests
{
    /// <summary>Struct emission round-trips through a <see cref="MemoryStream"/>.</summary>
    [Fact]
    public void BeginStruct_writes_members_and_closes_block()
    {
        using var ms = new MemoryStream();
        using (var writer = new RustWriter(new StreamWriter(ms, Encoding.UTF8, leaveOpen: true) { NewLine = "\n" },
                   disposeWriter: true))
        {
            using (writer.BeginStruct("Demo", "Clone, Copy"))
            {
                writer.StructMember("alpha", "i32");
                writer.StructMember("_padding", "[u8; 4]");
            }
        }

        string text = Encoding.UTF8.GetString(ms.ToArray());
        Assert.Contains("pub struct Demo", text, StringComparison.Ordinal);
        Assert.Contains("pub alpha: i32,", text, StringComparison.Ordinal);
        Assert.Contains("pub _padding: [u8; 4],", text, StringComparison.Ordinal);
        Assert.Contains('}', text);
    }

    /// <summary>Synthetic <c>_padding</c> must not be passed through field humanization.</summary>
    [Fact]
    public void StructMember_synthetic_padding_not_humanized()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginStruct("S", "Clone"))
        {
            w.StructMember("_padding", "[u8; 4]");
        }

        Assert.Contains("pub _padding:", sw.ToString(), StringComparison.Ordinal);
    }

    /// <summary>Nested <see cref="RustWriter.BeginIf"/> blocks increase indentation by four spaces per level.</summary>
    [Fact]
    public void Nested_if_blocks_indent()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        {
            using (w.BeginIf("self.a"))
            {
                using (w.BeginIf("self.b"))
                {
                    w.Line("inner();");
                }
            }
        }

        string s = sw.ToString();
        Assert.Contains("if self.a {", s, StringComparison.Ordinal);
        Assert.Contains("    if self.b {", s, StringComparison.Ordinal);
        Assert.Contains("        inner();", s, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.BeginIf"/> nests correctly.</summary>
    [Fact]
    public void BeginIf_emits_condition_brace()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginIf("self.ok"))
        {
            w.Line("do_work();");
        }

        Assert.Contains("if self.ok {", sw.ToString(), StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.BeginExternStruct"/> emits <c>#[repr(C)]</c> after derives.</summary>
    [Fact]
    public void BeginExternStruct_repr_and_derive_order()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginExternStruct("PodThing", "Pod", "Zeroable"))
        {
            w.StructMember("x", "u32");
        }

        string text = sw.ToString();
        int derive = text.IndexOf("#[derive", StringComparison.Ordinal);
        int repr = text.IndexOf("#[repr(C)]", StringComparison.Ordinal);
        Assert.True(repr >= 0 && derive >= 0);
        Assert.True(derive < repr);
    }

    /// <summary>Comment, doc, and fixme prefixes are applied.</summary>
    [Fact]
    public void Comment_DocLine_Fixme_prefixes()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        {
            w.Comment("note");
            w.DocLine("doc text");
            w.Fixme("later");
        }

        string text = sw.ToString();
        Assert.Contains("// note", text, StringComparison.Ordinal);
        Assert.Contains("/// doc text", text, StringComparison.Ordinal);
        Assert.Contains("// FIXME: later", text, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.EnumMember"/> can mark the default variant.</summary>
    [Fact]
    public void EnumMember_default_attribute()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginEnum("E", "u8"))
        {
            w.EnumMember("A", isDefault: true);
            w.EnumMember("B");
        }

        string text = sw.ToString();
        Assert.Contains("#[default]", text, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.BeginUnion"/> emits a tagged enum without <c>#[repr(...)]</c>.</summary>
    [Fact]
    public void BeginUnion_emits_pub_enum_without_repr()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginUnion("U"))
        {
            w.EnumVariantWithPayload("First", "i32");
        }

        string text = sw.ToString();
        Assert.Contains("pub enum U {", text, StringComparison.Ordinal);
        Assert.Contains("#[derive(Debug)]", text, StringComparison.Ordinal);
        Assert.DoesNotContain("#[repr(", text, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.TransparentStruct"/> emits a Pod/Zeroable single-field newtype with <c>#[repr(transparent)]</c>.</summary>
    [Fact]
    public void TransparentStruct_emits_repr_transparent_with_pod_zeroable_derives()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        {
            w.TransparentStruct("Wrapper", "u32");
        }

        string text = sw.ToString();
        Assert.Contains("#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]", text, StringComparison.Ordinal);
        Assert.Contains("#[repr(transparent)]", text, StringComparison.Ordinal);
        Assert.Contains("pub struct Wrapper(pub u32);", text, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.BeginImpl"/> emits an inherent impl block.</summary>
    [Fact]
    public void BeginImpl_emits_impl_block()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginImpl("Foo"))
        {
            w.Line("// body");
        }

        Assert.Contains("impl Foo {", sw.ToString(), StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.BeginTraitImpl"/> emits <c>impl Trait for Type</c>.</summary>
    [Fact]
    public void BeginTraitImpl_emits_impl_trait_for_type()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginTraitImpl("MyTrait", "Foo"))
        {
            w.Line("// body");
        }

        Assert.Contains("impl MyTrait for Foo {", sw.ToString(), StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.BeginMethod"/> emits <c>pub fn</c> with generics, parameters, and a return type.</summary>
    [Fact]
    public void BeginMethod_public_with_return_and_generics()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginMethod("name", "bool", ["T", "U"], ["a: i32"]))
        {
            w.Line("true");
        }

        Assert.Contains("pub fn name<T, U>(a: i32) -> bool {", sw.ToString(), StringComparison.Ordinal);
    }

    /// <summary>Private methods omit <c>pub</c>; missing return types omit the arrow.</summary>
    [Fact]
    public void BeginMethod_private_no_return_and_no_generics()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginMethod("name", string.Empty, generics: null, ["a: i32"], isPublic: false))
        {
            w.Line("// body");
        }

        string text = sw.ToString();
        Assert.Contains("fn name(a: i32) {", text, StringComparison.Ordinal);
        Assert.DoesNotContain("pub fn name", text, StringComparison.Ordinal);
        Assert.DoesNotContain("->", text, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.EnumMemberWithValue"/> emits an explicit discriminant; <c>isDefault</c> precedes it.</summary>
    [Fact]
    public void EnumMemberWithValue_emits_explicit_discriminant()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginEnum("E", "u8"))
        {
            w.EnumMemberWithValue("Seven", "7", isDefault: true);
            w.EnumMemberWithValue("Nine", "9");
        }

        string text = sw.ToString();
        Assert.Contains("Seven = 7,", text, StringComparison.Ordinal);
        Assert.Contains("Nine = 9,", text, StringComparison.Ordinal);
        Assert.Contains("#[default]", text, StringComparison.Ordinal);
    }

    /// <summary><see cref="RustWriter.EnumVariantWithPayload"/> emits a tuple-style variant.</summary>
    [Fact]
    public void EnumVariantWithPayload_emits_tuple_variant()
    {
        using var sw = new StringWriter(CultureInfo.InvariantCulture);
        using (var w = new RustWriter(sw))
        using (w.BeginUnion("U"))
        {
            w.EnumVariantWithPayload("Wrap", "Payload");
        }

        Assert.Contains("Wrap(Payload),", sw.ToString(), StringComparison.Ordinal);
    }
}
