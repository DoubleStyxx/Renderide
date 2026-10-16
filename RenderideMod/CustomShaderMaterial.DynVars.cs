using Elements.Core;
using FrooxEngine;
using ResoniteModLoader;

namespace RenderideMod;

public partial class CustomShaderMaterial
{
    private const string DefaultVariablePrefix = "Shader/";

    private readonly List<BoundDynVar> _dynVars = new();
    private readonly HashSet<string> _dynVarNames = new(StringComparer.Ordinal);
    private readonly List<Action> _dynUnsub = new();
    private readonly HashSet<Slot> _dynComponentSlots = new();
    private readonly HashSet<Slot> _dynChildSlots = new();
    private readonly HashSet<IDynamicVariable> _dynHookedVars = new();
    private readonly HashSet<DynamicVariableSpace> _dynHookedSpaces = new();
    private readonly HashSet<IChangeable> _dynHookedMembers = new();

    private string? _dynPrefix;
    private string _dynLoggedNames = "";
    private bool _dynRefreshing;
    private bool _ownSlotHooked;

    /// <summary>Blank <see cref="VariablePrefix"/> uses <see cref="DefaultVariablePrefix"/>.</summary>
    private string ActivePrefix()
    {
        var prefix = VariablePrefix.Value?.Trim();
        if (string.IsNullOrEmpty(prefix))
            return DefaultVariablePrefix;
        return prefix;
    }

    private void HookOwnSlot()
    {
        if (_ownSlotHooked)
            return;
        Slot.ParentChanged += OnOwnParentChanged;
        _ownSlotHooked = true;
    }

    private void UnhookOwnSlot()
    {
        if (!_ownSlotHooked)
            return;
        Slot.ParentChanged -= OnOwnParentChanged;
        _ownSlotHooked = false;
    }

    private void OnOwnParentChanged(Slot _)
    {
        if (Enabled.Value)
            BindDynVars();
    }

    private void BindDynVars()
    {
        if (_dynRefreshing)
            return;

        _dynRefreshing = true;
        try
        {
            UnbindDynVarHooks();
            var prefix = ActivePrefix();
            var seen = new HashSet<string>(StringComparer.Ordinal);
            var next = new List<BoundDynVar>();
            CollectDynVars(prefix, seen, next);

            var changed = !SameDynVars(next);
            _dynVars.Clear();
            _dynVars.AddRange(next);
            RebuildDynVarNames();
            _dynPrefix = prefix;
            QueueDynVarPropertyIds();
            LogDynVarNames();
            if (changed && Enabled.Value)
                MarkChangeDirty();
        }
        catch (Exception ex)
        {
            ResoniteMod.Warn($"custom shader material dynvar bind failed: {ex.Message}");
        }
        finally
        {
            _dynRefreshing = false;
        }
    }

    private void UnbindDynVars()
    {
        UnbindDynVarHooks();
        _dynVars.Clear();
        _dynVarNames.Clear();
        _dynPrefix = null;
        _dynLoggedNames = "";
    }

    private void UnbindDynVarHooks()
    {
        for (var i = _dynUnsub.Count - 1; i >= 0; i--)
            _dynUnsub[i]();
        _dynUnsub.Clear();
        _dynComponentSlots.Clear();
        _dynChildSlots.Clear();
        _dynHookedVars.Clear();
        _dynHookedSpaces.Clear();
        _dynHookedMembers.Clear();
    }

    /// <summary>
    /// Spaces on this slot and its parents. A space is walked only when a variable bound to it
    /// can produce a canonical name that starts with <paramref name="prefix"/>.
    /// </summary>
    private void CollectDynVars(string prefix, HashSet<string> seen, List<BoundDynVar> next)
    {
        var slot = Slot;
        while (slot != null)
        {
            WatchAncestorSlot(slot);
            foreach (var space in slot.GetComponents<DynamicVariableSpace>())
            {
                if (!IsLive(space))
                    continue;
                HookSpace(space);
                if (!SpaceCanMatch(SpaceKey(space), prefix))
                    continue;
                AbsorbSpace(space, prefix, seen, next);
            }

            slot = slot.Parent;
        }
    }

    private void AbsorbSpace(
        DynamicVariableSpace space,
        string prefix,
        HashSet<string> seen,
        List<BoundDynVar> next)
    {
        VisitSlots(space.Slot, slot =>
        {
            WatchSpaceSlot(slot);
            foreach (var variable in slot.GetComponents<IDynamicVariable>())
            {
                HookVariable(variable);
                if (variable is not Component component || !IsLive(component))
                    continue;
                if (!TryMap(variable, space, prefix, out var mapped))
                    continue;
                if (!seen.Add(mapped.Property))
                    continue;
                next.Add(mapped);
            }
        });
    }

    private static void VisitSlots(Slot slot, Action<Slot> visit)
    {
        visit(slot);
        foreach (var child in slot.Children)
            VisitSlots(child, visit);
    }

    private void WatchAncestorSlot(Slot slot)
    {
        WatchComponentEvents(slot);
    }

    private void WatchSpaceSlot(Slot slot)
    {
        WatchComponentEvents(slot);
        if (!_dynChildSlots.Add(slot))
            return;
        slot.ChildAdded += OnSpaceChildAdded;
        slot.ChildRemoved += OnSpaceChildRemoved;
        _dynUnsub.Add(() =>
        {
            slot.ChildAdded -= OnSpaceChildAdded;
            slot.ChildRemoved -= OnSpaceChildRemoved;
        });
    }

    private void WatchComponentEvents(Slot slot)
    {
        if (!_dynComponentSlots.Add(slot))
            return;
        slot.ComponentAdded += OnSlotComponentAdded;
        slot.ComponentRemoved += OnSlotComponentRemoved;
        _dynUnsub.Add(() =>
        {
            slot.ComponentAdded -= OnSlotComponentAdded;
            slot.ComponentRemoved -= OnSlotComponentRemoved;
        });
    }

    private void OnSpaceChildAdded(Slot _)
    {
        if (!_dynRefreshing && Enabled.Value)
            BindDynVars();
    }

    private void OnSpaceChildRemoved(Slot _)
    {
        if (!_dynRefreshing && Enabled.Value)
            BindDynVars();
    }

    private void OnSlotComponentAdded(Component component)
    {
        if (_dynRefreshing || !Enabled.Value)
            return;
        if (component is DynamicVariableSpace || component is IDynamicVariable)
            BindDynVars();
    }

    private void OnSlotComponentRemoved(Component component)
    {
        if (_dynRefreshing || !Enabled.Value)
            return;
        if (component is DynamicVariableSpace || component is IDynamicVariable)
            BindDynVars();
    }

    private void HookSpace(DynamicVariableSpace space)
    {
        if (!_dynHookedSpaces.Add(space))
            return;

        Action<IChangeable> handler = _ =>
        {
            if (!_dynRefreshing && Enabled.Value)
                BindDynVars();
        };
        HookMember(space, handler);
        HookSyncMembers(space, handler);
    }

    private void HookVariable(IDynamicVariable variable)
    {
        if (!_dynHookedVars.Add(variable) || variable is not Component component)
            return;

        Action<IChangeable> handler = _ => OnVariableChanged(variable);
        HookMember(component, handler);
        HookSyncMembers(component, handler);
        if (component is IDestroyable destroyable)
        {
            Action<IDestroyable> destroyed = _ =>
            {
                if (!_dynRefreshing && Enabled.Value)
                    BindDynVars();
            };
            destroyable.Destroyed += destroyed;
            _dynUnsub.Add(() => destroyable.Destroyed -= destroyed);
        }
    }

    private void HookSyncMembers(Component component, Action<IChangeable> handler)
    {
        var count = component.SyncMemberCount;
        for (var i = 0; i < count; i++)
        {
            if (component.GetSyncMember(i) is IChangeable member)
                HookMember(member, handler);
        }
    }

    private void HookMember(IChangeable member, Action<IChangeable> handler)
    {
        if (!_dynHookedMembers.Add(member))
            return;
        member.Changed += handler;
        _dynUnsub.Add(() =>
        {
            member.Changed -= handler;
            _dynHookedMembers.Remove(member);
        });
    }

    private void OnVariableChanged(IDynamicVariable variable)
    {
        if (_dynRefreshing || !Enabled.Value)
            return;

        try
        {
            var index = IndexOfDynVar(variable);
            if (index < 0)
            {
                if (CouldMap(variable))
                    BindDynVars();
                return;
            }

            var current = _dynVars[index];
            if (!TryMapLive(variable, ActivePrefix(), out var mapped)
                || mapped.Property != current.Property
                || mapped.Kind != current.Kind)
            {
                BindDynVars();
                return;
            }

            if (ValuesEqual(current, mapped))
                return;

            current.FloatValue = mapped.FloatValue;
            current.Vector = mapped.Vector;
            current.Texture = mapped.Texture;
            MarkChangeDirty();
        }
        catch (Exception ex)
        {
            ResoniteMod.Warn($"custom shader material dynvar read failed: {ex.Message}");
        }
    }

    private int IndexOfDynVar(IDynamicVariable variable)
    {
        for (var i = 0; i < _dynVars.Count; i++)
        {
            if (ReferenceEquals(_dynVars[i].Source, variable))
                return i;
        }

        return -1;
    }

    private bool CouldMap(IDynamicVariable variable)
    {
        if (variable is not Component component || !IsLive(component))
            return false;
        var space = FindBoundSpace(component, variable.VariableName);
        return space != null
            && IsAncestorSpace(space)
            && TryMap(variable, space, ActivePrefix(), out _);
    }

    private bool TryMapLive(IDynamicVariable variable, string prefix, out BoundDynVar mapped)
    {
        mapped = null!;
        if (variable is not Component component || !IsLive(component))
            return false;
        var space = FindBoundSpace(component, variable.VariableName);
        if (space == null || !IsAncestorSpace(space))
            return false;
        return TryMap(variable, space, prefix, out mapped);
    }

    private bool IsAncestorSpace(DynamicVariableSpace space)
    {
        var slot = Slot;
        var spaceSlot = space.Slot;
        while (slot != null)
        {
            if (ReferenceEquals(slot, spaceSlot))
                return true;
            slot = slot.Parent;
        }

        return false;
    }

    /// <summary>
    /// Standard lookup: direct names bind to the named space, other names bind to the first
    /// ancestor space that allows indirect binding. <see cref="DynamicVariableHandler{T}.CurrentSpace"/>
    /// is the same link; the handler field on the variable is not public.
    /// </summary>
    private static DynamicVariableSpace? FindBoundSpace(Component component, string? variableName)
    {
        DynamicVariableHelper.ParsePath(variableName ?? "", out var explicitSpace, out _);
        var direct = !string.IsNullOrEmpty(explicitSpace);
        var slot = component.Slot;
        while (slot != null)
        {
            foreach (var space in slot.GetComponents<DynamicVariableSpace>())
            {
                if (!IsLive(space))
                    continue;
                if (direct)
                {
                    if (SpaceNamesEqual(space.SpaceName.Value, explicitSpace))
                        return space;
                }
                else if (!space.OnlyDirectBinding.Value)
                {
                    return space;
                }
            }

            slot = slot.Parent;
        }

        return null;
    }

    private static bool TryMap(
        IDynamicVariable variable,
        DynamicVariableSpace space,
        string prefix,
        out BoundDynVar mapped)
    {
        mapped = null!;
        if (FindBoundSpace((Component)variable, variable.VariableName) != space)
            return false;
        if (!TryCanonical(variable, space, out var canonical))
            return false;
        if (!canonical.StartsWith(prefix, StringComparison.Ordinal))
            return false;

        var property = canonical.Substring(prefix.Length).Trim();
        if (property.Length == 0)
            return false;
        if (!TrySample(variable, out var kind, out var scalar, out var vector, out var texture))
            return false;

        mapped = new BoundDynVar
        {
            Source = variable,
            Property = property,
            Kind = kind,
            FloatValue = scalar,
            Vector = vector,
            Texture = texture,
        };
        return true;
    }

    private static bool TryCanonical(IDynamicVariable variable, DynamicVariableSpace space, out string canonical)
    {
        canonical = "";
        var raw = variable.VariableName;
        if (string.IsNullOrEmpty(raw))
            return false;

        DynamicVariableHelper.ParsePath(raw, out var explicitSpace, out _);
        if (!string.IsNullOrEmpty(explicitSpace))
        {
            canonical = raw;
            return true;
        }

        var spaceName = SpaceKey(space);
        if (string.IsNullOrEmpty(spaceName))
            return false;
        canonical = spaceName + "/" + raw;
        return true;
    }

    /// <summary>
    /// True when some name bound to <paramref name="spaceName"/> can start with <paramref name="prefix"/>.
    /// </summary>
    private static bool SpaceCanMatch(string? spaceName, string prefix)
    {
        if (string.IsNullOrEmpty(spaceName) || string.IsNullOrEmpty(prefix))
            return false;
        var rooted = spaceName + "/";
        return rooted.StartsWith(prefix, StringComparison.Ordinal)
            || prefix.StartsWith(rooted, StringComparison.Ordinal);
    }

    private static string? SpaceKey(DynamicVariableSpace space)
    {
        var raw = space.SpaceName.Value;
        return DynamicVariableHelper.ProcessName(raw) ?? raw;
    }

    private static bool SpaceNamesEqual(string? left, string? right)
    {
        var a = DynamicVariableHelper.ProcessName(left) ?? left;
        var b = DynamicVariableHelper.ProcessName(right) ?? right;
        return !string.IsNullOrEmpty(a) && string.Equals(a, b, StringComparison.Ordinal);
    }

    private static bool TrySample(
        IDynamicVariable variable,
        out DynKind kind,
        out float scalar,
        out float4 vector,
        out IAssetProvider<ITexture2D>? texture)
    {
        scalar = 0f;
        vector = default;
        texture = null;
        if (variable is DynamicVariableBase<float> floatVar)
        {
            kind = DynKind.Float;
            scalar = floatVar.DynamicValue;
            return true;
        }

        if (variable is DynamicVariableBase<float2> float2Var)
        {
            kind = DynKind.Float4;
            var value = float2Var.DynamicValue;
            vector = new float4(value.x, value.y, 0f, 0f);
            return true;
        }

        if (variable is DynamicVariableBase<float3> float3Var)
        {
            kind = DynKind.Float4;
            var value = float3Var.DynamicValue;
            vector = new float4(value.x, value.y, value.z, 0f);
            return true;
        }

        if (variable is DynamicVariableBase<float4> float4Var)
        {
            kind = DynKind.Float4;
            vector = float4Var.DynamicValue;
            return true;
        }

        if (variable is DynamicVariableBase<colorX> colorVar)
        {
            kind = DynKind.Float4;
            vector = LinearColor(colorVar.DynamicValue);
            return true;
        }

        if (variable is DynamicVariableBase<IAssetProvider<ITexture2D>> textureVar)
        {
            kind = DynKind.Texture;
            texture = textureVar.DynamicValue;
            return true;
        }

        kind = default;
        return false;
    }

    private static float4 LinearColor(colorX color)
    {
        var linear = color.ToProfile(ColorProfile.Linear);
        return new float4(linear.r, linear.g, linear.b, linear.a);
    }

    private static bool IsLive(Component component)
    {
        return !component.IsRemoved && component.Enabled.Value && component.Slot.IsActive;
    }

    private bool SameDynVars(List<BoundDynVar> next)
    {
        if (next.Count != _dynVars.Count)
            return false;
        for (var i = 0; i < next.Count; i++)
        {
            var a = _dynVars[i];
            var b = next[i];
            if (!ReferenceEquals(a.Source, b.Source) || a.Property != b.Property || a.Kind != b.Kind)
                return false;
            if (!ValuesEqual(a, b))
                return false;
        }

        return true;
    }

    private static bool ValuesEqual(BoundDynVar left, BoundDynVar right)
    {
        if (left.Kind != right.Kind)
            return false;
        switch (left.Kind)
        {
            case DynKind.Float:
                return left.FloatValue == right.FloatValue;
            case DynKind.Float4:
                return left.Vector.x == right.Vector.x
                    && left.Vector.y == right.Vector.y
                    && left.Vector.z == right.Vector.z
                    && left.Vector.w == right.Vector.w;
            case DynKind.Texture:
                return ReferenceEquals(left.Texture, right.Texture);
            default:
                return false;
        }
    }

    private void RebuildDynVarNames()
    {
        _dynVarNames.Clear();
        foreach (var binding in _dynVars)
            _dynVarNames.Add(binding.Property);
    }

    private void LogDynVarNames()
    {
        var names = new string[_dynVars.Count];
        for (var i = 0; i < _dynVars.Count; i++)
            names[i] = _dynVars[i].Property;
        var key = string.Join(",", names);
        if (key == _dynLoggedNames)
            return;
        _dynLoggedNames = key;
        ResoniteMod.Msg(
            $"custom shader material dynvars: count={_dynVars.Count} prefix={_dynPrefix}");
    }

    private void QueueDynVarPropertyIds()
    {
        foreach (var binding in _dynVars)
            Ipc.MaterialPropertyIds.Request(binding.Property);
    }

    private bool DynVarOverrides(string property)
    {
        return _dynVarNames.Contains(property);
    }

    private void PushDynVars(ref MaterialUpdateWriter writer)
    {
        foreach (var binding in _dynVars)
        {
            if (binding.Source is IWorldElement element && element.IsRemoved)
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(binding.Property, out var propertyId))
                continue;

            switch (binding.Kind)
            {
                case DynKind.Float:
                    writer.SetFloat(propertyId, binding.FloatValue);
                    break;
                case DynKind.Float4:
                    var vector = binding.Vector;
                    writer.SetFloat4(propertyId, in vector);
                    break;
                case DynKind.Texture:
                    writer.SetTexture(propertyId, binding.Texture);
                    break;
            }
        }
    }

    private enum DynKind : byte
    {
        Float,
        Float4,
        Texture,
    }

    private sealed class BoundDynVar
    {
        public IDynamicVariable Source = null!;
        public string Property = "";
        public DynKind Kind;
        public float FloatValue;
        public float4 Vector;
        public IAssetProvider<ITexture2D>? Texture;
    }
}
