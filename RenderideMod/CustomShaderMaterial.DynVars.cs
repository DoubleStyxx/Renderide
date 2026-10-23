using Elements.Core;
using FrooxEngine;
using RenderideMod.DynVars;
using ResoniteModLoader;

namespace RenderideMod;

public partial class CustomShaderMaterial
{
    private const string DefaultVariablePrefix = ShaderDynVarSelect.DefaultPrefix;

    private readonly List<SampledDynVar> _samples = [];
    private readonly List<ShaderDynBinding> _dynBindings = [];
    private readonly HashSet<string> _dynVarNames = new(StringComparer.Ordinal);
    private readonly List<Action> _dynUnsub = [];
    private readonly HashSet<Slot> _dynComponentSlots = [];
    private readonly HashSet<Slot> _dynChildSlots = [];
    private readonly HashSet<Slot> _dynLifetimeSlots = [];
    private readonly HashSet<IDynamicVariable> _dynHookedVars = [];
    private readonly HashSet<DynamicVariableSpace> _dynHookedSpaces = [];
    private readonly HashSet<IChangeable> _dynHookedMembers = [];
    private readonly Dictionary<IAssetProvider<ITexture2D>, int> _textureTokens = new(ReferenceEqualityComparer.Instance);

    private string? _dynPrefix;
    private string _dynLoggedNames = "";
    private bool _dynRefreshing;
    private bool _dynValueDirty;
    private bool _ownSlotHooked;
    private int _nextTextureToken = 1;

    /// <summary>Blank <see cref="VariablePrefix"/> uses <see cref="DefaultVariablePrefix"/>.</summary>
    private string ActivePrefix() => ShaderDynVarSelect.ActivePrefix(VariablePrefix.Value);

    private void HookOwnSlot()
    {
        if (_ownSlotHooked)
            return;
        Slot.ParentChanged += OnDynHierarchyChanged;
        _ownSlotHooked = true;
    }

    private void UnhookOwnSlot()
    {
        if (!_ownSlotHooked)
            return;
        _ownSlotHooked = false;
        // OnDestroy can run after Slot is gone. ParentChanged throws on a destroyed slot,
        // and the subscription dies with the slot, so the unsubscribe is skipped.
        try
        {
            if (Slot.IsRemoved || Slot.IsDestroyed)
                return;
            Slot.ParentChanged -= OnDynHierarchyChanged;
        }
        catch (Exception ex)
        {
            ResoniteMod.Warn($"custom shader material dynvar parent unhook skipped: {ex.Message}");
        }
    }

    private void OnDynHierarchyChanged(Slot _)
    {
        if (!_dynRefreshing && Enabled.Value)
            BindDynVars();
    }

    private void BindDynVars()
    {
        if (_dynRefreshing)
            return;

        _dynRefreshing = true;
        _dynValueDirty = false;
        try
        {
            UnbindDynVarHooks();
            var prefix = ActivePrefix();
            var snapshot = Capture(prefix);
            var next = ShaderDynVarSelect.Select(
                snapshot.ReaderSlotId,
                snapshot.Slots,
                snapshot.Spaces,
                snapshot.Variables,
                prefix);
            var changed = !SameBindings(next);
            _samples.Clear();
            _samples.AddRange(snapshot.Samples);
            _dynBindings.Clear();
            _dynBindings.AddRange(next);
            // RebuildDynVarNames marks the cached property-name list dirty.
            // OnChanges does not run for this path.
            RebuildDynVarNames();
            PruneTextureTokens();
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
        _samples.Clear();
        _dynBindings.Clear();
        _dynVarNames.Clear();
        _dynValueDirty = false;
        // OnChanges rebuilds the cached name list only for sync-list edits.
        // Disable and destroy clear dynvar names without OnChanges, so the next
        // NameResolutionState would keep hashing names that are no longer bound.
        InvalidateResolutionNames();
        _textureTokens.Clear();
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
        _dynLifetimeSlots.Clear();
        _dynHookedVars.Clear();
        _dynHookedSpaces.Clear();
        _dynHookedMembers.Clear();
    }

    /// <summary>
    /// Copies the slot chain and the variables under spaces that can match <paramref name="prefix"/>.
    /// Selection itself is <see cref="ShaderDynVarSelect"/>.
    /// </summary>
    private DynSnapshot Capture(string prefix)
    {
        var snapshot = new DynSnapshot(prefix);
        var chain = new List<Slot>();
        var slot = Slot;
        while (slot != null)
        {
            chain.Add(slot);
            slot = slot.Parent;
        }

        for (var i = chain.Count - 1; i >= 0; i--)
        {
            WatchAncestorSlot(chain[i]);
            snapshot.AddSlot(chain[i]);
        }

        snapshot.ReaderSlotId = snapshot.SlotId(Slot);
        for (var i = 0; i < chain.Count; i++)
        {
            foreach (var space in chain[i].GetComponents<DynamicVariableSpace>())
            {
                var walk = ShaderDynVarSelect.SpaceCanMatch(SpaceKey(space), prefix);
                snapshot.AddSpace(this, space, walk);
            }
        }

        // The chain loop above walks a nearer space before a farther one is in Spaces.
        // VariableCanMatch on that partial list misses an unslashed name under a nearer
        // OnlyDirectBinding space that actually binds to the farther space.
        snapshot.HookCapturedVariables(this);
        return snapshot;
    }

    private void WatchAncestorSlot(Slot slot)
    {
        WatchComponentEvents(slot);
        WatchSlotLifetime(slot);
    }

    private void WatchSpaceSlot(Slot slot)
    {
        WatchComponentEvents(slot);
        WatchSlotLifetime(slot);
        if (!_dynChildSlots.Add(slot))
            return;
        // ChildAdded and ChildRemoved are SlotChildEvent(Slot parent, Slot child).
        slot.ChildAdded += OnDynChildChanged;
        slot.ChildRemoved += OnDynChildChanged;
        _dynUnsub.Add(() =>
        {
            slot.ChildAdded -= OnDynChildChanged;
            slot.ChildRemoved -= OnDynChildChanged;
        });
    }

    private void OnDynChildChanged(Slot parent, Slot _)
    {
        OnDynHierarchyChanged(parent);
    }

    private void WatchSlotLifetime(Slot slot)
    {
        if (!_dynLifetimeSlots.Add(slot))
            return;
        // ParentChanged and ActiveChanged are SlotEvent(Slot). HookOwnSlot already
        // subscribes ParentChanged on this component's slot for the component lifetime.
        var ownSlot = ReferenceEquals(slot, Slot);
        if (!ownSlot)
            slot.ParentChanged += OnDynHierarchyChanged;
        slot.ActiveChanged += OnDynHierarchyChanged;
        _dynUnsub.Add(() =>
        {
            if (!ownSlot)
                slot.ParentChanged -= OnDynHierarchyChanged;
            slot.ActiveChanged -= OnDynHierarchyChanged;
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

        Action<IChangeable> handler = _ => OnDynHierarchyChanged(space.Slot);
        HookSyncMembers(space, handler);
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

    private void HookVariable(IDynamicVariable variable, bool canMatch)
    {
        if (!_dynHookedVars.Add(variable) || variable is not Component component)
            return;

        Action<IChangeable> rebind = _ => OnDynHierarchyChanged(component.Slot);
        HookMember(component.Enabled, rebind);
        HookNamedMember(component, "VariableName", rebind);
        if (canMatch)
        {
            Action<IChangeable> value = _ => OnDynValueChanged();
            var hookedValue = HookNamedMember(component, "Value", value);
            var hookedReference = HookNamedMember(component, "Reference", value);
            if (!hookedValue && !hookedReference)
            {
                ResoniteMod.Warn(
                    "custom shader material dynvar: no Value or Reference sync member on "
                    + component.GetType().Name
                    + " name="
                    + variable.VariableName);
            }
        }

        if (component is IDestroyable destroyable)
        {
            Action<IDestroyable> destroyed = _ => OnDynHierarchyChanged(component.Slot);
            destroyable.Destroyed += destroyed;
            _dynUnsub.Add(() => destroyable.Destroyed -= destroyed);
        }
    }

    private void OnDynValueChanged()
    {
        if (_dynRefreshing || !Enabled.Value)
            return;
        _dynValueDirty = true;
    }

    /// <summary>
    /// One resample for every value event since the previous update.
    /// Reads bound variables only. Does not walk slots or replace hooks.
    /// </summary>
    private void FlushDynVarValues()
    {
        if (!_dynValueDirty || _dynRefreshing)
            return;
        _dynValueDirty = false;
        switch (ResampleBoundValues())
        {
            case ShaderDynValueApply.Changed:
                PruneTextureTokens();
                MarkChangeDirty();
                break;
            case ShaderDynValueApply.NeedsRebind:
                BindDynVars();
                break;
        }
    }

    /// <summary>
    /// Copies fresh lanes onto <see cref="_dynBindings"/> with <see cref="ShaderDynVarSelect.ApplySample"/>.
    /// Unbound captured variables are not read.
    /// </summary>
    private ShaderDynValueApply ResampleBoundValues()
    {
        var changed = false;
        for (var i = 0; i < _dynBindings.Count; i++)
        {
            var binding = _dynBindings[i];
            if ((uint)binding.VariableId >= (uint)_samples.Count)
                return ShaderDynValueApply.NeedsRebind;
            var source = _samples[binding.VariableId];
            if (source.Source is not Component component || !TrySample(source.Source, out var sampled))
                return ShaderDynValueApply.NeedsRebind;

            var token = sampled.Kind == ShaderDynValueKind.Texture
                ? TokenFor(sampled.Texture)
                : 0;
            var sample = new ShaderDynVariable(
                binding.VariableId,
                0,
                component is IDynamicVariable variable ? variable.VariableName : null,
                IsLive(component),
                sampled.Kind,
                sampled.X,
                sampled.Y,
                sampled.Z,
                sampled.W,
                token,
                0,
                sampled.Profile);
            var apply = ShaderDynVarSelect.ApplySample(binding, sample, out var updated);
            if (apply == ShaderDynValueApply.NeedsRebind)
                return apply;
            source.Texture = sampled.Texture;
            if (apply == ShaderDynValueApply.Unchanged)
                continue;
            _dynBindings[i] = updated;
            changed = true;
        }

        return changed ? ShaderDynValueApply.Changed : ShaderDynValueApply.Unchanged;
    }

    private bool HookNamedMember(Component component, string name, Action<IChangeable> handler)
    {
        var count = component.SyncMemberCount;
        for (var i = 0; i < count; i++)
        {
            if (component.GetSyncMember(i) is not SyncElement element)
                continue;
            if (!string.Equals(element.Name, name, StringComparison.Ordinal))
                continue;
            HookMember(element, handler);
            return true;
        }

        return false;
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

    private static string? SpaceKey(DynamicVariableSpace space)
    {
        var raw = space.SpaceName.Value;
        var processed = DynamicVariableHelper.ProcessName(raw);
        if (!string.IsNullOrEmpty(processed))
            return processed;
        return ShaderDynVarSelect.Normalize(raw);
    }

    private static bool TrySample(IDynamicVariable variable, out SampledValue sampled)
    {
        if (variable is DynamicVariableBase<float> floatVar)
        {
            var value = floatVar.DynamicValue;
            sampled = new SampledValue(ShaderDynValueKind.Scalar, value, 0f, 0f, 0f, null, ShaderDynColorProfile.Linear);
            return true;
        }

        if (variable is DynamicVariableBase<float2> float2Var)
        {
            var value = float2Var.DynamicValue;
            sampled = new SampledValue(ShaderDynValueKind.Float2, value.x, value.y, 0f, 0f, null, ShaderDynColorProfile.Linear);
            return true;
        }

        if (variable is DynamicVariableBase<float3> float3Var)
        {
            var value = float3Var.DynamicValue;
            sampled = new SampledValue(ShaderDynValueKind.Float3, value.x, value.y, value.z, 0f, null, ShaderDynColorProfile.Linear);
            return true;
        }

        if (variable is DynamicVariableBase<float4> float4Var)
        {
            var value = float4Var.DynamicValue;
            sampled = new SampledValue(ShaderDynValueKind.Float4, value.x, value.y, value.z, value.w, null, ShaderDynColorProfile.Linear);
            return true;
        }

        if (variable is DynamicVariableBase<colorX> colorVar)
        {
            var value = colorVar.DynamicValue;
            sampled = new SampledValue(
                ShaderDynValueKind.Color,
                value.r,
                value.g,
                value.b,
                value.a,
                null,
                ColorProfileOf(value.profile));
            return true;
        }

        if (variable is DynamicVariableBase<IAssetProvider<ITexture2D>> textureVar)
        {
            sampled = new SampledValue(
                ShaderDynValueKind.Texture,
                0f,
                0f,
                0f,
                0f,
                textureVar.DynamicValue,
                ShaderDynColorProfile.Linear);
            return true;
        }

        sampled = default;
        return false;
    }

    /// <summary>
    /// <c>Elements.Core.ColorProfile</c> is a byte enum: Linear 0, sRGB 1, sRGBAlpha 2.
    /// Values outside that set stay as the raw byte so a later profile still round-trips.
    /// </summary>
    private static ShaderDynColorProfile ColorProfileOf(ColorProfile profile)
    {
        return (ShaderDynColorProfile)(byte)profile;
    }

    private int TokenFor(IAssetProvider<ITexture2D>? texture)
    {
        if (texture is null)
            return 0;
        if (_textureTokens.TryGetValue(texture, out var token))
            return token;
        token = _nextTextureToken++;
        _textureTokens[texture] = token;
        return token;
    }

    private void PruneTextureTokens()
    {
        if (_textureTokens.Count == 0)
            return;
        var live = new HashSet<int>();
        foreach (var binding in _dynBindings)
        {
            if (binding.TextureToken != 0)
                live.Add(binding.TextureToken);
        }

        if (live.Count == _textureTokens.Count)
            return;

        List<IAssetProvider<ITexture2D>>? stale = null;
        foreach (var pair in _textureTokens)
        {
            if (live.Contains(pair.Value))
                continue;
            stale ??= [];
            stale.Add(pair.Key);
        }

        if (stale is null)
            return;
        foreach (var key in stale)
            _textureTokens.Remove(key);
    }

    private static bool IsLive(Component component)
    {
        return !component.IsRemoved && component.Enabled.Value && component.Slot.IsActive;
    }

    private bool SameBindings(IReadOnlyList<ShaderDynBinding> next)
    {
        if (next.Count != _dynBindings.Count)
            return false;
        foreach (var binding in next)
        {
            var found = false;
            foreach (var current in _dynBindings)
            {
                if (!string.Equals(current.Property, binding.Property, StringComparison.Ordinal))
                    continue;
                if (!ShaderDynVarSelect.SameSample(current, binding))
                    return false;
                found = true;
                break;
            }

            if (!found)
                return false;
        }

        return true;
    }

    private void RebuildDynVarNames()
    {
        _dynVarNames.Clear();
        foreach (var binding in _dynBindings)
            _dynVarNames.Add(binding.Property);
        // BindDynVars is not an OnChanges. The cached sorted name list
        // (_resolutionNames) is rebuilt only when _resolutionNamesDirty is set,
        // which OnEnabled and OnChanges do for sync lists. A rebind that adds or
        // drops a dynvar property has to set that flag here, or the settled-id
        // hash keeps the previous names and a new id never dirties this material.
        // A value-only resample does not call this: it keeps the property names.
        InvalidateResolutionNames();
    }

    private void LogDynVarNames()
    {
        var names = new string[_dynBindings.Count];
        for (var i = 0; i < _dynBindings.Count; i++)
            names[i] = _dynBindings[i].Property;
        var key = string.Join(",", names);
        if (key == _dynLoggedNames)
            return;
        _dynLoggedNames = key;
        ResoniteMod.Msg(
            $"custom shader material dynvars: count={_dynBindings.Count} prefix={_dynPrefix}");
    }

    private void QueueDynVarPropertyIds()
    {
        foreach (var binding in _dynBindings)
            Ipc.MaterialPropertyIds.Request(binding.Property);
    }

    /// <summary>
    /// Adds dynvar property names to this material's settled-id hash.
    /// A result for one of these names dirties this material. Another material's result does not.
    /// </summary>
    private void CollectDynVarPropertyNames(List<string> names)
    {
        foreach (var name in _dynVarNames)
        {
            if (name.Length > 0)
                names.Add(name);
        }
    }

    private bool DynVarOverrides(string property) => _dynVarNames.Contains(property);

    private void PushDynVars(ref MaterialUpdateWriter writer)
    {
        foreach (var binding in _dynBindings)
        {
            if ((uint)binding.VariableId >= (uint)_samples.Count)
                continue;
            var sample = _samples[binding.VariableId];
            if (sample.Source is IWorldElement element && element.IsRemoved)
                continue;
            if (!Ipc.MaterialPropertyIds.TryGet(binding.Property, out var propertyId))
                continue;

            switch (binding.WriterKind)
            {
                case ShaderDynWriterKind.Scalar:
                    writer.SetFloat(propertyId, binding.X);
                    break;
                case ShaderDynWriterKind.Float4:
                    var vector = new float4(binding.X, binding.Y, binding.Z, binding.W);
                    writer.SetFloat4(propertyId, in vector);
                    break;
                case ShaderDynWriterKind.Color:
                    // colorX(r, g, b, a) defaults the profile. SetColor then runs
                    // ToProfile(RenderSystem.MaterialColorProfile), so the stored profile has to be passed.
                    var color = new colorX(
                        binding.X,
                        binding.Y,
                        binding.Z,
                        binding.W,
                        (ColorProfile)(byte)binding.ColorProfile);
                    writer.SetColor(propertyId, in color);
                    break;
                case ShaderDynWriterKind.Texture:
                    writer.SetTexture(propertyId, sample.Texture);
                    break;
            }
        }
    }

    private readonly struct SampledValue(
        ShaderDynValueKind kind,
        float x,
        float y,
        float z,
        float w,
        IAssetProvider<ITexture2D>? texture,
        ShaderDynColorProfile profile)
    {
        public ShaderDynValueKind Kind { get; } = kind;
        public float X { get; } = x;
        public float Y { get; } = y;
        public float Z { get; } = z;
        public float W { get; } = w;
        public IAssetProvider<ITexture2D>? Texture { get; } = texture;
        public ShaderDynColorProfile Profile { get; } = profile;
    }

    private sealed class SampledDynVar
    {
        public IDynamicVariable Source = null!;
        public IAssetProvider<ITexture2D>? Texture;
    }

    private sealed class DynSnapshot
    {
        private readonly Dictionary<Slot, int> _slotIds = [];
        private readonly HashSet<DynamicVariableSpace> _spacesSeen = [];
        private readonly HashSet<Slot> _walked = [];
        private readonly List<PendingVar> _pending = [];

        private struct PendingVar
        {
            public IDynamicVariable Variable;
            public int SlotId;
        }

        public DynSnapshot(string prefix)
        {
            Prefix = prefix;
        }

        public string Prefix { get; }

        public List<ShaderDynSlot> Slots { get; } = [];
        public List<ShaderDynSpace> Spaces { get; } = [];
        public List<ShaderDynVariable> Variables { get; } = [];
        public List<SampledDynVar> Samples { get; } = [];
        public int ReaderSlotId { get; set; }

        public int SlotId(Slot slot) => _slotIds[slot];

        public int AddSlot(Slot slot)
        {
            if (_slotIds.TryGetValue(slot, out var existing))
                return existing;

            int? parentId = null;
            var sibling = 0;
            if (slot.Parent is Slot parent && _slotIds.TryGetValue(parent, out var parentSlotId))
            {
                parentId = parentSlotId;
                sibling = SiblingIndex(slot);
            }

            var id = Slots.Count;
            _slotIds[slot] = id;
            Slots.Add(new ShaderDynSlot(id, parentId, sibling));
            return id;
        }

        public void AddSpace(CustomShaderMaterial owner, DynamicVariableSpace space, bool walkSubtree)
        {
            if (!_spacesSeen.Add(space))
            {
                if (walkSubtree)
                    Walk(owner, space.Slot);
                return;
            }

            owner.HookSpace(space);
            var slotId = AddSlot(space.Slot);
            Spaces.Add(new ShaderDynSpace(
                slotId,
                SpaceKey(space),
                space.OnlyDirectBinding.Value,
                IsLive(space),
                Spaces.Count));
            if (walkSubtree)
                Walk(owner, space.Slot);
        }

        private void Walk(CustomShaderMaterial owner, Slot slot)
        {
            if (!_walked.Add(slot))
                return;
            owner.WatchSpaceSlot(slot);
            var slotId = AddSlot(slot);
            foreach (var space in slot.GetComponents<DynamicVariableSpace>())
                AddSpace(owner, space, walkSubtree: false);
            foreach (var variable in slot.GetComponents<IDynamicVariable>())
                AddVariable(owner, variable, slotId);
            foreach (var child in slot.Children)
                Walk(owner, child);
        }

        /// <summary>
        /// Hooks each captured variable against the finished space list.
        /// A nearer <c>OnlyDirectBinding</c> space is walked before its ancestor is added.
        /// </summary>
        public void HookCapturedVariables(CustomShaderMaterial owner)
        {
            foreach (var pending in _pending)
            {
                var canMatch = ShaderDynVarSelect.VariableCanMatch(
                    pending.Variable.VariableName,
                    pending.SlotId,
                    Slots,
                    Spaces,
                    Prefix);
                owner.HookVariable(pending.Variable, canMatch);
            }
        }

        private void AddVariable(CustomShaderMaterial owner, IDynamicVariable variable, int slotId)
        {
            _pending.Add(new PendingVar { Variable = variable, SlotId = slotId });
            if (variable is not Component component || !TrySample(variable, out var sampled))
                return;

            var token = sampled.Kind == ShaderDynValueKind.Texture
                ? owner.TokenFor(sampled.Texture)
                : 0;
            var id = Samples.Count;
            Samples.Add(new SampledDynVar { Source = variable, Texture = sampled.Texture });
            Variables.Add(new ShaderDynVariable(
                id,
                slotId,
                variable.VariableName,
                IsLive(component),
                sampled.Kind,
                sampled.X,
                sampled.Y,
                sampled.Z,
                sampled.W,
                token,
                Variables.Count,
                sampled.Profile));
        }

        private static int SiblingIndex(Slot slot)
        {
            var parent = slot.Parent;
            if (parent is null)
                return 0;
            var index = 0;
            foreach (var child in parent.Children)
            {
                if (ReferenceEquals(child, slot))
                    return index;
                index++;
            }

            return index;
        }
    }
}
