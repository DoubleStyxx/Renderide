namespace RenderideMod.DynVars;

/// <summary>
/// Chooses which dynamic variables become custom-shader properties.
/// </summary>
/// <remarks>
/// <para>
/// FrooxEngine does not expose <c>DynamicVariableSpace.SpaceLookup</c>. The private handler on a
/// variable is not public either. Binding follows the same parent walk the engine documents:
/// a name that contains <c>/</c> binds to the first live ancestor space with that name, and any
/// other name binds to the first live ancestor space whose <c>OnlyDirectBinding</c> is false.
/// <c>DynamicVariableHelper.ParsePath</c> is that slash split. This type trims whitespace only.
/// </para>
/// <para>
/// The material reads spaces on its slot and parents. The nearest such space wins when two
/// variables map to one property. Inside one space, the first live variable in slot preorder
/// wins. <see cref="CustomShaderMaterial"/> samples FrooxEngine values and passes them here.
/// </para>
/// </remarks>
public static class ShaderDynVarSelect
{
    /// <summary>Prefix used when <see cref="ActivePrefix"/> is blank.</summary>
    public const string DefaultPrefix = "Shader/";

    /// <summary>Blank and whitespace prefixes are <see cref="DefaultPrefix"/>.</summary>
    /// <param name="prefix">Raw <c>VariablePrefix</c> value.</param>
    /// <returns>The prefix used for property names.</returns>
    public static string ActivePrefix(string? prefix)
    {
        var trimmed = prefix?.Trim();
        if (string.IsNullOrEmpty(trimmed))
            return DefaultPrefix;
        return trimmed;
    }

    /// <summary>Trims <paramref name="name"/>. Empty and whitespace become null.</summary>
    /// <param name="name">Space name or variable name.</param>
    /// <returns>The trimmed name, or null.</returns>
    public static string? Normalize(string? name)
    {
        var trimmed = name?.Trim();
        if (string.IsNullOrEmpty(trimmed))
            return null;
        return trimmed;
    }

    /// <summary>Ordinal compare after <see cref="Normalize"/>.</summary>
    /// <param name="left">First space name.</param>
    /// <param name="right">Second space name.</param>
    /// <returns>True when both names trim to the same non-empty text.</returns>
    public static bool NamesEqual(string? left, string? right)
    {
        var a = Normalize(left);
        var b = Normalize(right);
        return a is not null && string.Equals(a, b, StringComparison.Ordinal);
    }

    /// <summary>
    /// Splits a variable name on the first <c>/</c>.
    /// </summary>
    /// <param name="variableName">Raw variable name.</param>
    /// <param name="spaceName">Text before the slash, or null when there is no slash.</param>
    /// <param name="localName">Text after the slash, or the whole name when there is no slash.</param>
    /// <returns>False when the name is empty.</returns>
    public static bool TrySplitPath(string? variableName, out string? spaceName, out string localName)
    {
        spaceName = null;
        localName = "";
        var trimmed = Normalize(variableName);
        if (trimmed is null)
            return false;

        var slash = trimmed.IndexOf('/');
        if (slash < 0)
        {
            localName = trimmed;
            return true;
        }

        spaceName = Normalize(trimmed[..slash]);
        localName = Normalize(trimmed[(slash + 1)..]) ?? "";
        return true;
    }

    /// <summary>
    /// Canonical name. A name that contains a slash is kept. Any other name is
    /// <c>space/local</c>.
    /// </summary>
    /// <param name="variableName">Raw variable name.</param>
    /// <param name="spaceName">Bound space name. Ignored when <paramref name="variableName"/> has a slash.</param>
    /// <param name="canonical">Name compared with the prefix.</param>
    /// <returns>False when the name or the required space name is empty.</returns>
    public static bool TryCanonical(string? variableName, string? spaceName, out string canonical)
    {
        canonical = "";
        if (!TrySplitPath(variableName, out var explicitSpace, out var localName))
            return false;
        if (explicitSpace is not null || variableName?.Trim().Contains('/') == true)
        {
            canonical = Normalize(variableName) ?? "";
            return canonical.Length > 0;
        }

        var space = Normalize(spaceName);
        if (space is null)
            return false;
        canonical = space + "/" + localName;
        return true;
    }

    /// <summary>Property name is the canonical name with <paramref name="prefix"/> removed once.</summary>
    /// <param name="canonical">Canonical variable name.</param>
    /// <param name="prefix">Active prefix, including its slash.</param>
    /// <param name="property">WGSL property name. A leading underscore is preserved.</param>
    /// <returns>False when the name does not start with the prefix or the remainder is empty.</returns>
    public static bool TryProperty(string canonical, string prefix, out string property)
    {
        property = "";
        if (string.IsNullOrEmpty(prefix) || !canonical.StartsWith(prefix, StringComparison.Ordinal))
            return false;
        property = canonical[prefix.Length..].Trim();
        return property.Length > 0;
    }

    /// <summary>
    /// True when a variable bound to <paramref name="spaceName"/> can start with <paramref name="prefix"/>.
    /// </summary>
    /// <param name="spaceName">Space name.</param>
    /// <param name="prefix">Active prefix.</param>
    /// <returns>False for a blank space or prefix.</returns>
    public static bool SpaceCanMatch(string? spaceName, string prefix)
    {
        var name = Normalize(spaceName);
        if (name is null || string.IsNullOrEmpty(prefix))
            return false;
        var rooted = name + "/";
        return rooted.StartsWith(prefix, StringComparison.Ordinal)
            || prefix.StartsWith(rooted, StringComparison.Ordinal);
    }

    /// <summary>
    /// Selects the variables that override material rows for <paramref name="readerSlotId"/>.
    /// </summary>
    /// <param name="readerSlotId">Slot of the material.</param>
    /// <param name="slots">Slots reachable from the reader or from a space on that chain.</param>
    /// <param name="spaces">Spaces in component order. <see cref="ShaderDynSpace.Order"/> is that order.</param>
    /// <param name="variables">Variables in hierarchy order. <see cref="ShaderDynVariable.Order"/> is that order.</param>
    /// <param name="prefix">Raw prefix. Blank becomes <see cref="DefaultPrefix"/>.</param>
    /// <returns>One binding per property. Nearest space, then preorder, wins.</returns>
    public static ShaderDynBinding[] Select(
        int readerSlotId,
        IReadOnlyList<ShaderDynSlot> slots,
        IReadOnlyList<ShaderDynSpace> spaces,
        IReadOnlyList<ShaderDynVariable> variables,
        string? prefix)
    {
        var activePrefix = ActivePrefix(prefix);
        var slotById = new Dictionary<int, ShaderDynSlot>(slots.Count);
        foreach (var slot in slots)
            slotById[slot.Id] = slot;
        if (!slotById.ContainsKey(readerSlotId))
            return [];

        var children = Children(slots, slotById);
        var spacesBySlot = GroupSpaces(spaces);
        var variablesBySlot = GroupVariables(variables);
        var bindings = new List<ShaderDynBinding>();
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var visited = new HashSet<int>();
        var slotId = readerSlotId;
        while (slotById.TryGetValue(slotId, out var slot))
        {
            if (!visited.Add(slotId))
                break;
            if (spacesBySlot.TryGetValue(slotId, out var spaceIndexes))
            {
                foreach (var spaceIndex in spaceIndexes)
                {
                    var space = spaces[spaceIndex];
                    if (!space.Live || !SpaceCanMatch(space.Name, activePrefix))
                        continue;
                    Absorb(
                        spaceIndex,
                        spaces,
                        variables,
                        children,
                        spacesBySlot,
                        variablesBySlot,
                        slotById,
                        activePrefix,
                        seen,
                        bindings);
                }
            }

            if (slot.ParentId is not int parentId)
                break;
            slotId = parentId;
        }

        return [.. bindings];
    }

    /// <summary>Bitwise float compare so a NaN sample does not dirty the material on every edit.</summary>
    /// <param name="left">Current binding.</param>
    /// <param name="right">Next binding.</param>
    /// <returns>True when the property, writer, lanes, texture token, and color profile match.</returns>
    public static bool SameSample(ShaderDynBinding left, ShaderDynBinding right)
    {
        return string.Equals(left.Property, right.Property, StringComparison.Ordinal)
            && left.WriterKind == right.WriterKind
            && left.TextureToken == right.TextureToken
            && left.ColorProfile == right.ColorProfile
            && Bits(left.X) == Bits(right.X)
            && Bits(left.Y) == Bits(right.Y)
            && Bits(left.Z) == Bits(right.Z)
            && Bits(left.W) == Bits(right.W);
    }

    /// <summary>
    /// True when <paramref name="variableName"/> can produce a property under <paramref name="prefix"/>.
    /// A slashed name ignores <paramref name="spaceName"/>. A local name uses that space.
    /// </summary>
    /// <param name="variableName">Raw variable name.</param>
    /// <param name="spaceName">Space that would bind a name with no slash.</param>
    /// <param name="prefix">Raw prefix. Blank becomes <see cref="DefaultPrefix"/>.</param>
    /// <returns>False when the canonical name does not start with the prefix.</returns>
    public static bool VariableCanMatch(string? variableName, string? spaceName, string? prefix)
    {
        if (!TryCanonical(variableName, spaceName, out var canonical))
            return false;
        return TryProperty(canonical, ActivePrefix(prefix), out _);
    }

    /// <summary>
    /// True when <paramref name="variableName"/> on <paramref name="slotId"/> binds to a space
    /// whose canonical name starts with <paramref name="prefix"/>. Uses the same space walk as <see cref="Select"/>.
    /// </summary>
    /// <param name="variableName">Raw variable name.</param>
    /// <param name="slotId">Slot of the variable.</param>
    /// <param name="slots">Slots already copied, including parents of <paramref name="slotId"/>.</param>
    /// <param name="spaces">Every space on the chain. A nearer <c>OnlyDirectBinding</c> space hides the ancestor when that ancestor is missing from this list.</param>
    /// <param name="prefix">Raw prefix. Blank becomes <see cref="DefaultPrefix"/>.</param>
    /// <returns>False when no live space binds the name, or the property would be empty.</returns>
    public static bool VariableCanMatch(
        string? variableName,
        int slotId,
        IReadOnlyList<ShaderDynSlot> slots,
        IReadOnlyList<ShaderDynSpace> spaces,
        string? prefix)
    {
        var slotById = new Dictionary<int, ShaderDynSlot>(slots.Count);
        foreach (var slot in slots)
            slotById[slot.Id] = slot;
        if (!slotById.ContainsKey(slotId))
            return false;

        var variable = new ShaderDynVariable(
            0,
            slotId,
            variableName,
            true,
            ShaderDynValueKind.Scalar,
            0f,
            0f,
            0f,
            0f,
            0,
            0);
        var spaceIndex = FindBoundSpace(variable, spaces, GroupSpaces(spaces), slotById);
        var spaceName = spaceIndex is int index ? spaces[index].Name : null;
        return VariableCanMatch(variableName, spaceName, prefix);
    }

    /// <summary>
    /// Copies sampled lanes onto bindings with the same <see cref="ShaderDynBinding.VariableId"/>.
    /// The property name stays. A sample whose type does not match the writer asks for a full rebind.
    /// </summary>
    /// <param name="bindings">Bindings from the last structure walk.</param>
    /// <param name="samples">Fresh samples. <see cref="ShaderDynVariable.Id"/> matches <see cref="ShaderDynBinding.VariableId"/>.</param>
    /// <param name="destination">Replaced with the updated bindings. Empty when the result is <see cref="ShaderDynValueApply.NeedsRebind"/>.</param>
    /// <returns>Whether any <see cref="SameSample"/> comparison failed, or the writer no longer matches.</returns>
    public static ShaderDynValueApply ApplyValueSamples(
        IReadOnlyList<ShaderDynBinding> bindings,
        IReadOnlyList<ShaderDynVariable> samples,
        List<ShaderDynBinding> destination)
    {
        destination.Clear();
        var byId = new Dictionary<int, int>(samples.Count);
        for (var i = 0; i < samples.Count; i++)
            byId[samples[i].Id] = i;

        var changed = false;
        foreach (var binding in bindings)
        {
            if (!byId.TryGetValue(binding.VariableId, out var index))
            {
                destination.Add(binding);
                continue;
            }

            var sample = samples[index];
            var apply = ApplySample(binding, sample, out var next);
            if (apply == ShaderDynValueApply.NeedsRebind)
            {
                destination.Clear();
                return apply;
            }

            if (apply == ShaderDynValueApply.Changed)
                changed = true;
            destination.Add(next);
        }

        return changed ? ShaderDynValueApply.Changed : ShaderDynValueApply.Unchanged;
    }

    /// <summary>
    /// Writes one fresh sample onto one binding. The property name stays.
    /// </summary>
    /// <param name="binding">Binding from the last structure walk.</param>
    /// <param name="sample">Fresh sample. <see cref="ShaderDynVariable.Id"/> must equal <see cref="ShaderDynBinding.VariableId"/>.</param>
    /// <param name="updated">The binding with the sample's lanes. Default when the result is <see cref="ShaderDynValueApply.NeedsRebind"/>.</param>
    /// <returns>Whether this pair's lanes changed, or the writer no longer matches.</returns>
    public static ShaderDynValueApply ApplySample(
        in ShaderDynBinding binding,
        in ShaderDynVariable sample,
        out ShaderDynBinding updated)
    {
        if (sample.Id != binding.VariableId || !WriterMatches(binding.WriterKind, sample.ValueKind))
        {
            updated = default;
            return ShaderDynValueApply.NeedsRebind;
        }

        updated = ToBinding(sample, binding.Property);
        return SameSample(binding, updated)
            ? ShaderDynValueApply.Unchanged
            : ShaderDynValueApply.Changed;
    }

    private static bool WriterMatches(ShaderDynWriterKind writer, ShaderDynValueKind kind)
    {
        return writer switch
        {
            ShaderDynWriterKind.Scalar => kind == ShaderDynValueKind.Scalar,
            ShaderDynWriterKind.Float4 => kind is ShaderDynValueKind.Float2
                or ShaderDynValueKind.Float3
                or ShaderDynValueKind.Float4,
            ShaderDynWriterKind.Color => kind == ShaderDynValueKind.Color,
            ShaderDynWriterKind.Texture => kind == ShaderDynValueKind.Texture,
            _ => false,
        };
    }

    private static int Bits(float value) => BitConverter.SingleToInt32Bits(value);

    private static Dictionary<int, List<int>> Children(
        IReadOnlyList<ShaderDynSlot> slots,
        Dictionary<int, ShaderDynSlot> slotById)
    {
        var children = new Dictionary<int, List<int>>();
        foreach (var slot in slots)
        {
            if (slot.ParentId is not int parentId)
                continue;
            if (!children.TryGetValue(parentId, out var list))
            {
                list = [];
                children[parentId] = list;
            }

            list.Add(slot.Id);
        }

        foreach (var list in children.Values)
        {
            list.Sort((left, right) =>
            {
                var order = slotById[left].SiblingIndex.CompareTo(slotById[right].SiblingIndex);
                return order != 0 ? order : left.CompareTo(right);
            });
        }

        return children;
    }

    private static Dictionary<int, List<int>> GroupSpaces(IReadOnlyList<ShaderDynSpace> spaces)
    {
        var grouped = new Dictionary<int, List<int>>();
        for (var i = 0; i < spaces.Count; i++)
        {
            var slotId = spaces[i].SlotId;
            if (!grouped.TryGetValue(slotId, out var list))
            {
                list = [];
                grouped[slotId] = list;
            }

            list.Add(i);
        }

        foreach (var list in grouped.Values)
        {
            list.Sort((left, right) =>
            {
                var order = spaces[left].Order.CompareTo(spaces[right].Order);
                return order != 0 ? order : left.CompareTo(right);
            });
        }

        return grouped;
    }

    private static Dictionary<int, List<int>> GroupVariables(IReadOnlyList<ShaderDynVariable> variables)
    {
        var grouped = new Dictionary<int, List<int>>();
        for (var i = 0; i < variables.Count; i++)
        {
            var slotId = variables[i].SlotId;
            if (!grouped.TryGetValue(slotId, out var list))
            {
                list = [];
                grouped[slotId] = list;
            }

            list.Add(i);
        }

        foreach (var list in grouped.Values)
        {
            list.Sort((left, right) =>
            {
                var order = variables[left].Order.CompareTo(variables[right].Order);
                return order != 0 ? order : left.CompareTo(right);
            });
        }

        return grouped;
    }

    private static void Absorb(
        int spaceIndex,
        IReadOnlyList<ShaderDynSpace> spaces,
        IReadOnlyList<ShaderDynVariable> variables,
        Dictionary<int, List<int>> children,
        Dictionary<int, List<int>> spacesBySlot,
        Dictionary<int, List<int>> variablesBySlot,
        Dictionary<int, ShaderDynSlot> slotById,
        string prefix,
        HashSet<string> seen,
        List<ShaderDynBinding> bindings)
    {
        var space = spaces[spaceIndex];
        foreach (var slotId in Preorder(space.SlotId, children))
        {
            if (!variablesBySlot.TryGetValue(slotId, out var variableIndexes))
                continue;
            foreach (var variableIndex in variableIndexes)
            {
                var variable = variables[variableIndex];
                if (!variable.Live)
                    continue;
                if (FindBoundSpace(variable, spaces, spacesBySlot, slotById) != spaceIndex)
                    continue;
                if (!TryCanonical(variable.Name, space.Name, out var canonical))
                    continue;
                if (!TryProperty(canonical, prefix, out var property))
                    continue;
                if (!seen.Add(property))
                    continue;
                bindings.Add(ToBinding(variable, property));
            }
        }
    }

    private static int? FindBoundSpace(
        ShaderDynVariable variable,
        IReadOnlyList<ShaderDynSpace> spaces,
        Dictionary<int, List<int>> spacesBySlot,
        Dictionary<int, ShaderDynSlot> slotById)
    {
        _ = TrySplitPath(variable.Name, out var explicitSpace, out _);
        var direct = explicitSpace is not null;
        var seen = new HashSet<int>();
        var slotId = variable.SlotId;
        while (slotById.ContainsKey(slotId))
        {
            if (!seen.Add(slotId))
                return null;
            if (spacesBySlot.TryGetValue(slotId, out var spaceIndexes))
            {
                foreach (var spaceIndex in spaceIndexes)
                {
                    var space = spaces[spaceIndex];
                    if (!space.Live)
                        continue;
                    if (direct)
                    {
                        if (NamesEqual(space.Name, explicitSpace))
                            return spaceIndex;
                    }
                    else if (!space.OnlyDirectBinding)
                    {
                        return spaceIndex;
                    }
                }
            }

            if (slotById[slotId].ParentId is not int parentId)
                return null;
            slotId = parentId;
        }

        return null;
    }

    private static List<int> Preorder(int root, Dictionary<int, List<int>> children)
    {
        var output = new List<int>();
        var seen = new HashSet<int>();
        AppendPreorder(root, children, output, seen);
        return output;
    }

    private static void AppendPreorder(
        int slotId,
        Dictionary<int, List<int>> children,
        List<int> output,
        HashSet<int> seen)
    {
        if (!seen.Add(slotId))
            return;
        output.Add(slotId);
        if (!children.TryGetValue(slotId, out var kids))
            return;
        foreach (var child in kids)
            AppendPreorder(child, children, output, seen);
    }

    private static ShaderDynBinding ToBinding(ShaderDynVariable variable, string property)
    {
        var x = variable.X;
        var y = variable.Y;
        var z = variable.Z;
        var w = variable.W;
        var texture = 0;
        ShaderDynWriterKind writer;
        switch (variable.ValueKind)
        {
            case ShaderDynValueKind.Scalar:
                writer = ShaderDynWriterKind.Scalar;
                y = 0f;
                z = 0f;
                w = 0f;
                break;
            case ShaderDynValueKind.Float2:
                writer = ShaderDynWriterKind.Float4;
                z = 0f;
                w = 0f;
                break;
            case ShaderDynValueKind.Float3:
                writer = ShaderDynWriterKind.Float4;
                w = 0f;
                break;
            case ShaderDynValueKind.Float4:
                writer = ShaderDynWriterKind.Float4;
                break;
            case ShaderDynValueKind.Color:
                writer = ShaderDynWriterKind.Color;
                break;
            case ShaderDynValueKind.Texture:
                writer = ShaderDynWriterKind.Texture;
                texture = variable.TextureToken;
                x = 0f;
                y = 0f;
                z = 0f;
                w = 0f;
                break;
            default:
                writer = ShaderDynWriterKind.Scalar;
                break;
        }

        var profile = writer == ShaderDynWriterKind.Color
            ? variable.ColorProfile
            : ShaderDynColorProfile.Linear;
        return new ShaderDynBinding(variable.Id, property, writer, x, y, z, w, texture, profile);
    }
}

/// <summary>One slot in the parent walk.</summary>
/// <param name="Id">Caller-assigned id.</param>
/// <param name="ParentId">Parent slot id. Null for a root.</param>
/// <param name="SiblingIndex">Index in the parent's child list.</param>
public readonly record struct ShaderDynSlot(int Id, int? ParentId, int SiblingIndex);

/// <summary>One <c>DynamicVariableSpace</c> in component order on its slot.</summary>
/// <param name="SlotId">Slot that owns the component.</param>
/// <param name="Name">Space name. The adapter passes <c>ProcessName</c> output when that call returns text.</param>
/// <param name="OnlyDirectBinding">Indirect names skip this space when true.</param>
/// <param name="Live">False when the component is disabled, removed, or on an inactive slot.</param>
/// <param name="Order">Component order on <paramref name="SlotId"/>.</param>
public readonly record struct ShaderDynSpace(
    int SlotId,
    string? Name,
    bool OnlyDirectBinding,
    bool Live,
    int Order);

/// <summary>Host value kind before padding.</summary>
public enum ShaderDynValueKind : byte
{
    /// <summary><c>float</c>. Written with <c>SetFloat</c>.</summary>
    Scalar,

    /// <summary><c>float2</c>. Written with <c>SetFloat4</c> and zero in <c>zw</c>.</summary>
    Float2,

    /// <summary><c>float3</c>. Written with <c>SetFloat4</c> and zero in <c>w</c>.</summary>
    Float3,

    /// <summary><c>float4</c>. Written with <c>SetFloat4</c>.</summary>
    Float4,

    /// <summary><c>colorX</c>. Written with <c>SetColor</c>. <see cref="ShaderDynVariable.ColorProfile"/> is kept.</summary>
    Color,

    /// <summary><c>IAssetProvider&lt;ITexture2D&gt;</c>. Written with <c>SetTexture</c>.</summary>
    Texture,
}

/// <summary>Writer call for a selected property.</summary>
public enum ShaderDynWriterKind : byte
{
    /// <summary><c>MaterialUpdateWriter.SetFloat</c>.</summary>
    Scalar,

    /// <summary><c>MaterialUpdateWriter.SetFloat4</c>.</summary>
    Float4,

    /// <summary><c>MaterialUpdateWriter.SetColor</c>.</summary>
    Color,

    /// <summary><c>MaterialUpdateWriter.SetTexture</c>.</summary>
    Texture,
}

/// <summary>
/// <c>Elements.Core.ColorProfile</c> stored by its underlying value.
/// Linear is 0, sRGB is 1, sRGBAlpha is 2.
/// </summary>
public enum ShaderDynColorProfile : byte
{
    /// <summary><c>ColorProfile.Linear</c>.</summary>
    Linear = 0,

    /// <summary><c>ColorProfile.sRGB</c>.</summary>
    Srgb = 1,

    /// <summary><c>ColorProfile.sRGBAlpha</c>.</summary>
    SrgbAlpha = 2,
}

/// <summary>Result of copying new lanes onto an existing binding set.</summary>
public enum ShaderDynValueApply : byte
{
    /// <summary>Every <see cref="ShaderDynVarSelect.SameSample"/> comparison matched.</summary>
    Unchanged,

    /// <summary>At least one lane, texture token, or color profile changed.</summary>
    Changed,

    /// <summary>A sample type no longer matches the writer. The caller rebinds.</summary>
    NeedsRebind,
}

/// <summary>One sampled variable.</summary>
/// <param name="Id">Index the caller uses to recover the source component.</param>
/// <param name="SlotId">Slot of the component.</param>
/// <param name="Name">Raw variable name.</param>
/// <param name="Live">False when the component is disabled, removed, or on an inactive slot.</param>
/// <param name="ValueKind">Host type.</param>
/// <param name="X">Lane 0. Unused for textures.</param>
/// <param name="Y">Lane 1.</param>
/// <param name="Z">Lane 2.</param>
/// <param name="W">Lane 3.</param>
/// <param name="TextureToken">Stable identity for a texture reference. Zero is null.</param>
/// <param name="Order">Order among variables on <paramref name="SlotId"/>.</param>
/// <param name="ColorProfile"><c>colorX.profile</c>. Ignored unless <paramref name="ValueKind"/> is <see cref="ShaderDynValueKind.Color"/>.</param>
public readonly record struct ShaderDynVariable(
    int Id,
    int SlotId,
    string? Name,
    bool Live,
    ShaderDynValueKind ValueKind,
    float X,
    float Y,
    float Z,
    float W,
    int TextureToken,
    int Order,
    ShaderDynColorProfile ColorProfile = ShaderDynColorProfile.Linear);

/// <summary>One property the material writer should emit.</summary>
/// <param name="VariableId"><see cref="ShaderDynVariable.Id"/> of the winning variable.</param>
/// <param name="Property">WGSL name. <c>Shader/_Speed</c> becomes <c>_Speed</c>.</param>
/// <param name="WriterKind">Which <c>MaterialUpdateWriter</c> call to use.</param>
/// <param name="X">Lane 0. Padded lanes are zero.</param>
/// <param name="Y">Lane 1.</param>
/// <param name="Z">Lane 2.</param>
/// <param name="W">Lane 3.</param>
/// <param name="TextureToken">Texture identity. Zero is a null texture.</param>
/// <param name="ColorProfile"><c>colorX.profile</c> for <see cref="ShaderDynWriterKind.Color"/>. Otherwise <see cref="ShaderDynColorProfile.Linear"/>.</param>
public readonly record struct ShaderDynBinding(
    int VariableId,
    string Property,
    ShaderDynWriterKind WriterKind,
    float X,
    float Y,
    float Z,
    float W,
    int TextureToken,
    ShaderDynColorProfile ColorProfile = ShaderDynColorProfile.Linear);
