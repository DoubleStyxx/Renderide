using RenderideMod.DynVars;
using Xunit;

namespace RenderideMod.DynVarCore.Tests;

/// <summary>
/// Binding and prefix rules for shader properties read from dynamic variables.
/// The scene tests build the smoke-checklist hierarchy without FrooxEngine.
/// </summary>
public sealed class ShaderDynVarSelectTests
{
    /// <summary>A blank prefix is the documented default.</summary>
    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void Blank_prefix_is_shader_slash(string? prefix)
    {
        Assert.Equal("Shader/", ShaderDynVarSelect.ActivePrefix(prefix));
    }

    /// <summary>Example uniform names keep the underscore after the default prefix.</summary>
    [Theory]
    [InlineData("Shader/_Color", "_Color")]
    [InlineData("Shader/_MainTex", "_MainTex")]
    [InlineData("Shader/_MainTex_ST", "_MainTex_ST")]
    [InlineData("Shader/_Speed", "_Speed")]
    [InlineData("Shader/_Schlafli", "_Schlafli")]
    [InlineData("Shader/_Frequency", "_Frequency")]
    [InlineData("Shader/_Scan", "_Scan")]
    [InlineData("Shader/_Fresnel", "_Fresnel")]
    [InlineData("Shader/_Grid", "_Grid")]
    [InlineData("Shader/_Emission", "_Emission")]
    [InlineData("Shader/_Warp", "_Warp")]
    [InlineData("Shader/_Spread", "_Spread")]
    public void Example_dynvar_names_map_to_uniform_properties(string variableName, string property)
    {
        Assert.True(ShaderDynVarSelect.TryCanonical(variableName, "Shader", out var canonical));
        Assert.True(ShaderDynVarSelect.TryProperty(canonical, "Shader/", out var mapped));
        Assert.Equal(property, mapped);
    }

    /// <summary>A space named Shader and a local name are the same property as the slashed name.</summary>
    [Fact]
    public void Indirect_name_uses_the_bound_space()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, "Shader");
        var reader = world.Slot(space);
        world.Float(reader, "_Speed", 0.5f);

        var binding = Assert.Single(world.Select(reader));
        Assert.Equal("_Speed", binding.Property);
        Assert.Equal(ShaderDynWriterKind.Scalar, binding.WriterKind);
        Assert.Equal(0.5f, binding.X);
    }

    /// <summary>Whitespace around the name and the space does not become part of the property.</summary>
    [Fact]
    public void Trim_drops_whitespace_and_keeps_the_underscore()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, " Shader ");
        world.Float(space, " Shader/_Speed ", 1f);

        var binding = Assert.Single(world.Select(space, " Shader/ "));
        Assert.Equal("_Speed", binding.Property);
    }

    /// <summary>float2 and float3 pad the unused lanes with zeros and use SetFloat4.</summary>
    [Fact]
    public void Float2_and_float3_pad_to_float4()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, "Shader");
        world.Add(space, "Shader/_Pad2", ShaderDynValueKind.Float2, 1f, 2f, 9f, 9f);
        world.Add(space, "Shader/_Pad3", ShaderDynValueKind.Float3, 3f, 4f, 5f, 9f);

        var bindings = world.Select(space);
        var pad2 = Assert.Single(bindings, binding => binding.Property == "_Pad2");
        var pad3 = Assert.Single(bindings, binding => binding.Property == "_Pad3");
        Assert.Equal(ShaderDynWriterKind.Float4, pad2.WriterKind);
        Assert.Equal((1f, 2f, 0f, 0f), (pad2.X, pad2.Y, pad2.Z, pad2.W));
        Assert.Equal(ShaderDynWriterKind.Float4, pad3.WriterKind);
        Assert.Equal((3f, 4f, 5f, 0f), (pad3.X, pad3.Y, pad3.Z, pad3.W));
    }

    /// <summary>colorX stays on SetColor. Textures keep their identity token.</summary>
    [Fact]
    public void Color_and_texture_keep_their_writer()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, "Shader");
        world.Add(space, "Shader/_Color", ShaderDynValueKind.Color, 0.1f, 0.2f, 0.3f, 1f);
        world.Add(space, "Shader/_MainTex", ShaderDynValueKind.Texture, texture: 7);

        var bindings = world.Select(space);
        var color = Assert.Single(bindings, binding => binding.Property == "_Color");
        var texture = Assert.Single(bindings, binding => binding.Property == "_MainTex");
        Assert.Equal(ShaderDynWriterKind.Color, color.WriterKind);
        Assert.Equal((0.1f, 0.2f, 0.3f, 1f), (color.X, color.Y, color.Z, color.W));
        Assert.Equal(ShaderDynColorProfile.Linear, color.ColorProfile);
        Assert.Equal(ShaderDynWriterKind.Texture, texture.WriterKind);
        Assert.Equal(7, texture.TextureToken);
    }

    /// <summary>A sampler name is not reserved. Users do not create that dynvar; the selector does not block it.</summary>
    [Fact]
    public void Sampler_names_are_not_filtered()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, "Shader");
        world.Float(space, "Shader/_MainTex_sampler", 1f);

        var binding = Assert.Single(world.Select(space));
        Assert.Equal("_MainTex_sampler", binding.Property);
    }

    /// <summary>The smoke scene: dynvars hide same-named static rows until they leave or the prefix changes.</summary>
    [Fact]
    public void Smoke_scene_overrides_until_the_variable_or_prefix_changes()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, "Shader");
        var reader = world.Slot(space);
        world.Float(reader, "Shader/_Speed", 0.5f);
        world.Add(reader, "Shader/_Color", ShaderDynValueKind.Color, 0.2f, 0.4f, 0.6f, 1f);
        world.Add(reader, "Shader/_MainTex_ST", ShaderDynValueKind.Float4, 2f, 2f, 0.1f, 0f);

        var bound = world.Select(reader);
        Assert.Equal(3, bound.Length);
        Assert.Contains(bound, binding => binding.Property == "_Speed" && binding.X == 0.5f);
        Assert.Contains(bound, binding => binding.Property == "_Color" && binding.WriterKind == ShaderDynWriterKind.Color);
        Assert.Contains(bound, binding => binding.Property == "_MainTex_ST" && binding.WriterKind == ShaderDynWriterKind.Float4);

        var withoutSpeed = new DynFixture();
        var spaceOnly = withoutSpeed.Slot();
        withoutSpeed.Space(spaceOnly, "Shader");
        var readerOnly = withoutSpeed.Slot(spaceOnly);
        withoutSpeed.Add(readerOnly, "Shader/_Color", ShaderDynValueKind.Color, 0.2f, 0.4f, 0.6f, 1f);
        var remaining = withoutSpeed.Select(readerOnly);
        Assert.DoesNotContain(remaining, binding => binding.Property == "_Speed");
        Assert.Single(remaining);

        var disabled = new DynFixture();
        var disabledSpace = disabled.Slot();
        disabled.Space(disabledSpace, "Shader");
        disabled.Float(disabledSpace, "Shader/_Speed", 0.5f, live: false);
        disabled.Float(disabledSpace, "Shader/_Speed", 0.2f);
        Assert.Equal(0.2f, Assert.Single(disabled.Select(disabledSpace)).X);

        Assert.Empty(world.Select(reader, "Other/"));
        Assert.Equal(3, world.Select(reader, "  ").Length);
    }

    /// <summary>The nearer space wins. Inside one space the first live preorder variable wins.</summary>
    [Fact]
    public void Nearest_space_and_preorder_pick_the_winner()
    {
        var nested = new DynFixture();
        var parent = nested.Slot();
        nested.Space(parent, "Shader");
        var child = nested.Slot(parent);
        nested.Space(child, "Shader");
        nested.Float(parent, "_Speed", 1f);
        nested.Float(child, "_Speed", 9f);
        Assert.Equal(9f, Assert.Single(nested.Select(child)).X);

        var order = new DynFixture();
        var root = order.Slot();
        order.Space(root, "Shader");
        var deeper = order.Slot(root);
        order.Float(root, "_Speed", 1f);
        order.Float(deeper, "_Speed", 2f);
        Assert.Equal(1f, Assert.Single(order.Select(root)).X);

        var skipDisabled = new DynFixture();
        var slot = skipDisabled.Slot();
        skipDisabled.Space(slot, "Shader");
        skipDisabled.Float(slot, "_Speed", 1f, live: false);
        var childSlot = skipDisabled.Slot(slot);
        skipDisabled.Float(childSlot, "_Speed", 4f);
        Assert.Equal(4f, Assert.Single(skipDisabled.Select(slot)).X);
    }

    /// <summary>Indirect names skip OnlyDirectBinding. Direct names bind to that space by name.</summary>
    [Fact]
    public void Direct_and_indirect_binding_follow_only_direct()
    {
        var indirect = new DynFixture();
        var parent = indirect.Slot();
        indirect.Space(parent, "Shader");
        var child = indirect.Slot(parent);
        indirect.Space(child, "Other", onlyDirect: true);
        indirect.Float(child, "_Speed", 3f);
        Assert.Equal("_Speed", Assert.Single(indirect.Select(child)).Property);

        var blocked = new DynFixture();
        var only = blocked.Slot();
        blocked.Space(only, "Shader", onlyDirect: true);
        blocked.Float(only, "_Speed", 1f);
        Assert.Empty(blocked.Select(only));

        var direct = new DynFixture();
        var directSlot = direct.Slot();
        direct.Space(directSlot, "Shader", onlyDirect: true);
        direct.Float(directSlot, "Shader/_Speed", 6f);
        Assert.Equal(6f, Assert.Single(direct.Select(directSlot)).X);

        var named = new DynFixture();
        var outer = named.Slot();
        named.Space(outer, "Shader");
        var inner = named.Slot(outer);
        named.Space(inner, "Other");
        named.Float(inner, "Shader/_Color", 1f);
        var binding = Assert.Single(named.Select(inner));
        Assert.Equal("_Color", binding.Property);
        Assert.Equal(ShaderDynWriterKind.Scalar, binding.WriterKind);
    }

    /// <summary>A reader that is not under the space does not see its variables.</summary>
    [Fact]
    public void Reader_outside_the_space_drops_the_override()
    {
        var world = new DynFixture();
        var root = world.Slot();
        var space = world.Slot(root);
        world.Space(space, "Shader");
        world.Float(space, "Shader/_Speed", 0.5f);
        var reader = world.Slot(root);

        Assert.Empty(world.Select(reader));
        Assert.Single(world.Select(space));
    }

    /// <summary>A disabled nearer space does not hide the parent space.</summary>
    [Fact]
    public void Disabled_nearer_space_does_not_block_the_parent()
    {
        var world = new DynFixture();
        var parent = world.Slot();
        world.Space(parent, "Shader");
        var child = world.Slot(parent);
        world.Space(child, "Shader", live: false);
        world.Float(child, "_Speed", 8f);

        Assert.Equal(8f, Assert.Single(world.Select(child)).X);
    }

    /// <summary>
    /// <c>colorX.profile</c> is part of the sample. Linear and sRGB with the same lanes are different.
    /// </summary>
    [Fact]
    public void Color_profile_survives_selection_and_value_edits()
    {
        var world = new DynFixture();
        var space = world.Slot();
        world.Space(space, "Shader");
        world.Add(
            space,
            "Shader/_Color",
            ShaderDynValueKind.Color,
            0.2f,
            0.4f,
            0.6f,
            1f,
            profile: ShaderDynColorProfile.Srgb);

        var selected = Assert.Single(world.Select(space));
        Assert.Equal(ShaderDynWriterKind.Color, selected.WriterKind);
        Assert.Equal(ShaderDynColorProfile.Srgb, selected.ColorProfile);

        var edited = new ShaderDynVariable(
            selected.VariableId,
            space,
            "Shader/_Color",
            true,
            ShaderDynValueKind.Color,
            0.2f,
            0.4f,
            0.6f,
            1f,
            0,
            0,
            ShaderDynColorProfile.Linear);
        var updated = new List<ShaderDynBinding>();
        Assert.Equal(
            ShaderDynValueApply.Changed,
            ShaderDynVarSelect.ApplyValueSamples([selected], [edited], updated));
        var next = Assert.Single(updated);
        Assert.Equal("_Color", next.Property);
        Assert.Equal(ShaderDynColorProfile.Linear, next.ColorProfile);
        Assert.False(ShaderDynVarSelect.SameSample(selected, next));

        updated.Clear();
        Assert.Equal(
            ShaderDynValueApply.Unchanged,
            ShaderDynVarSelect.ApplyValueSamples([next], [edited], updated));
    }

    /// <summary>A value edit keeps the property and does not treat an unchanged sample as new.</summary>
    [Fact]
    public void Value_sample_updates_lanes_and_leaves_the_property()
    {
        var binding = new ShaderDynBinding(3, "_Speed", ShaderDynWriterKind.Scalar, 0.2f, 0f, 0f, 0f, 0);
        var sample = new ShaderDynVariable(
            3,
            1,
            "Shader/_Speed",
            true,
            ShaderDynValueKind.Scalar,
            0.5f,
            9f,
            9f,
            9f,
            0,
            0);
        var updated = new List<ShaderDynBinding>();
        Assert.Equal(
            ShaderDynValueApply.Changed,
            ShaderDynVarSelect.ApplyValueSamples([binding], [sample], updated));
        var next = Assert.Single(updated);
        Assert.Equal("_Speed", next.Property);
        Assert.Equal(ShaderDynWriterKind.Scalar, next.WriterKind);
        Assert.Equal(0.5f, next.X);
        Assert.Equal(0f, next.Y);
    }

    /// <summary>Names that cannot produce a property under the prefix are not value-hooked.</summary>
    [Theory]
    [InlineData("Shader/_Speed", "Shader", "Shader/", true)]
    [InlineData("_Speed", "Shader", "Shader/", true)]
    [InlineData("World/Position", "Shader", "Shader/", false)]
    [InlineData("_Speed", "World", "Shader/", false)]
    [InlineData("Shader/_Color", "World", "Shader/", true)]
    public void Variable_can_match_only_when_the_prefix_covers_the_name(
        string variableName,
        string spaceName,
        string prefix,
        bool expected)
    {
        Assert.Equal(expected, ShaderDynVarSelect.VariableCanMatch(variableName, spaceName, prefix));
    }

    /// <summary>The hook filter uses the same space walk as selection, including OnlyDirectBinding.</summary>
    [Fact]
    public void Variable_can_match_follows_the_bound_space()
    {
        var slots = new List<ShaderDynSlot>
        {
            new(0, null, 0),
            new(1, 0, 0),
        };
        var directChild = new List<ShaderDynSpace>
        {
            new(0, "Shader", false, true, 0),
            new(1, "World", false, true, 1),
        };
        Assert.False(ShaderDynVarSelect.VariableCanMatch("_Speed", 1, slots, directChild, "Shader/"));

        var indirectChild = new List<ShaderDynSpace>
        {
            new(0, "Shader", false, true, 0),
            new(1, "World", true, true, 1),
        };
        Assert.True(ShaderDynVarSelect.VariableCanMatch("_Speed", 1, slots, indirectChild, "Shader/"));
    }

    /// <summary>
    /// An unslashed name under a nearer OnlyDirectBinding Shader space binds to the farther Shader space.
    /// The nearer space alone does not: that is the mid-walk list, and it must not decide the value hook.
    /// </summary>
    [Fact]
    public void Unslashed_name_under_direct_only_space_uses_the_ancestor_shader_space()
    {
        var slots = new List<ShaderDynSlot>
        {
            new(0, null, 0),
            new(1, 0, 0),
        };
        var nearerOnly = new List<ShaderDynSpace>
        {
            new(1, "Shader", true, true, 0),
        };
        Assert.False(ShaderDynVarSelect.VariableCanMatch("_Speed", 1, slots, nearerOnly, "Shader/"));

        var withAncestor = new List<ShaderDynSpace>
        {
            new(1, "Shader", true, true, 0),
            new(0, "Shader", false, true, 0),
        };
        Assert.True(ShaderDynVarSelect.VariableCanMatch("_Speed", 1, slots, withAncestor, "Shader/"));

        var variables = new List<ShaderDynVariable>
        {
            new(0, 1, "_Speed", true, ShaderDynValueKind.Scalar, 0.5f, 0f, 0f, 0f, 0, 0),
        };
        var selected = ShaderDynVarSelect.Select(1, slots, withAncestor, variables, "Shader/");
        var binding = Assert.Single(selected);
        Assert.Equal("_Speed", binding.Property);
        Assert.Equal(0.5f, binding.X);
        Assert.Empty(ShaderDynVarSelect.Select(1, slots, nearerOnly, variables, "Shader/"));
    }

    /// <summary>One bound sample updates in place. A writer mismatch asks for a full rebind.</summary>
    [Fact]
    public void Apply_sample_updates_one_binding_without_the_other_variables()
    {
        var speed = new ShaderDynBinding(3, "_Speed", ShaderDynWriterKind.Scalar, 0.2f, 0f, 0f, 0f, 0);
        var sample = new ShaderDynVariable(
            3,
            1,
            "Shader/_Speed",
            true,
            ShaderDynValueKind.Scalar,
            0.5f,
            0f,
            0f,
            0f,
            0,
            0);
        Assert.Equal(ShaderDynValueApply.Changed, ShaderDynVarSelect.ApplySample(speed, sample, out var updated));
        Assert.Equal("_Speed", updated.Property);
        Assert.Equal(0.5f, updated.X);

        Assert.Equal(ShaderDynValueApply.Unchanged, ShaderDynVarSelect.ApplySample(updated, sample, out _));

        var color = new ShaderDynVariable(
            3,
            1,
            "Shader/_Speed",
            true,
            ShaderDynValueKind.Color,
            0.5f,
            0f,
            0f,
            1f,
            0,
            0,
            ShaderDynColorProfile.Srgb);
        Assert.Equal(ShaderDynValueApply.NeedsRebind, ShaderDynVarSelect.ApplySample(updated, color, out _));
    }

    /// <summary>NaN lanes compare equal by bits so an unchanged sample is not a new value.</summary>
    [Fact]
    public void Same_sample_treats_nan_as_unchanged()
    {
        var value = new ShaderDynBinding(0, "_Speed", ShaderDynWriterKind.Scalar, float.NaN, 0f, 0f, 0f, 0);
        Assert.True(ShaderDynVarSelect.SameSample(value, value));
        Assert.False(value.X == value.X);
    }

    /// <summary>Space walk skipping matches the prefix, not a shared substring without the slash boundary.</summary>
    [Theory]
    [InlineData("Shader", "Shader/", true)]
    [InlineData("ShaderExtra", "Shader/", false)]
    [InlineData("World", "Shader/", false)]
    [InlineData("", "Shader/", false)]
    [InlineData("Shader", "Shader", true)]
    public void Space_can_match_only_when_a_name_could_use_the_prefix(string space, string prefix, bool expected)
    {
        Assert.Equal(expected, ShaderDynVarSelect.SpaceCanMatch(space, prefix));
    }

    private sealed class DynFixture
    {
        private readonly List<ShaderDynSlot> _slots = [];
        private readonly List<ShaderDynSpace> _spaces = [];
        private readonly List<ShaderDynVariable> _variables = [];
        private int _nextSlot;

        public int Slot(int? parent = null)
        {
            var id = _nextSlot++;
            var sibling = 0;
            if (parent is int parentId)
            {
                foreach (var slot in _slots)
                {
                    if (slot.ParentId == parentId)
                        sibling++;
                }
            }

            _slots.Add(new ShaderDynSlot(id, parent, sibling));
            return id;
        }

        public void Space(int slot, string? name, bool onlyDirect = false, bool live = true)
        {
            _spaces.Add(new ShaderDynSpace(slot, name, onlyDirect, live, _spaces.Count));
        }

        public void Float(int slot, string? name, float value, bool live = true)
        {
            Add(slot, name, ShaderDynValueKind.Scalar, value, live: live);
        }

        public void Add(
            int slot,
            string? name,
            ShaderDynValueKind kind,
            float x = 0f,
            float y = 0f,
            float z = 0f,
            float w = 0f,
            int texture = 0,
            bool live = true,
            ShaderDynColorProfile profile = ShaderDynColorProfile.Linear)
        {
            var id = _variables.Count;
            _variables.Add(new ShaderDynVariable(
                id,
                slot,
                name,
                live,
                kind,
                x,
                y,
                z,
                w,
                texture,
                id,
                profile));
        }

        public ShaderDynBinding[] Select(int reader, string? prefix = null)
        {
            return ShaderDynVarSelect.Select(reader, _slots, _spaces, _variables, prefix);
        }
    }
}
