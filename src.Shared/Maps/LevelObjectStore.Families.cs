using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

/// <summary>Current, prevalidated root and descendants; serialized parent links, not reconstructed APT creation.</summary>
public sealed record LevelObjectFamily(int RootSlot, uint RootUid, IReadOnlyList<int> ObjectSlots, IReadOnlyList<int> LightSlots)
{
    public IReadOnlyList<int> ParticleSlots { get; init; } = Array.Empty<int>();
    public IReadOnlyList<int> LagerSlots { get; init; } = Array.Empty<int>();
    public IReadOnlyList<int> BigLagerSlots { get; init; } = Array.Empty<int>();
}

/// <summary>Detached supported-family state keyed by original slot. Indices are not reallocated yet.</summary>
public sealed record LevelObjectFamilySnapshot(
    int RootSlot, uint RootUid,
    IReadOnlyDictionary<int, LevelObjectTemplate> Objects,
    IReadOnlyDictionary<int, NativeLightSnapshot> Lights,
    IReadOnlyDictionary<int, NativeParticleSnapshot> Particles,
    IReadOnlyDictionary<int, NativeLagerSnapshot> Lagers,
    IReadOnlyDictionary<int, NativeBigLagerSnapshot> BigLagers);

/// <summary>Non-mutating allocation proposal for all currently supported family pools; not a reservation.</summary>
public sealed record LevelObjectFamilyCoreAllocation(
    IReadOnlyDictionary<int, int> Objects, IReadOnlyDictionary<int, uint> Uids,
    IReadOnlyDictionary<int, int> Positions, IReadOnlyDictionary<int, int> ObjData)
{
    public IReadOnlyDictionary<int, int> Anim { get; init; } = new Dictionary<int, int>();
    public IReadOnlyDictionary<int, int> GfxType { get; init; } = new Dictionary<int, int>();
    public IReadOnlyDictionary<int, int> Action { get; init; } = new Dictionary<int, int>();
    public IReadOnlyDictionary<int, int> Lights { get; init; } = new Dictionary<int, int>();
    public IReadOnlyDictionary<int, int> Particles { get; init; } = new Dictionary<int, int>();
    public IReadOnlyDictionary<int, int> Lagers { get; init; } = new Dictionary<int, int>();
    public IReadOnlyDictionary<int, int> BigLagers { get; init; } = new Dictionary<int, int>();
}
public sealed record LevelObjectFamilyCopyCandidate(LevelObjectStore Store, int RootSlot, uint RootUid,
    LevelObjectFamilyCoreAllocation Allocation);

public sealed partial class LevelObjectStore
{
    /// <summary>Creates a detached saved-family copy with exact fixed displacement. Source remains untouched;
    /// candidate validation/Save and UI acceptance are separate from native create or simulation.</summary>
    public LevelObjectFamilyCopyCandidate? CreateFamilyCopyCandidate(int rootSlot, uint rootUid, int dx, int dy, int dz)
        => CreateFamilyImportCandidate(this, rootSlot, rootUid, dx, dy, dz);

    /// <summary>Imports a complete validated saved family into a detached target. Neither input is mutated.
    /// Source and target capacities may differ. Team override applies to every member.</summary>
    public LevelObjectFamilyCopyCandidate? CreateFamilyImportCandidate(LevelObjectStore source,
        int rootSlot, uint rootUid, int dx, int dy, int dz, int? team = null)
    {
        ArgumentNullException.ThrowIfNull(source);
        if (team is < 0 or > 8) throw new ArgumentOutOfRangeException(nameof(team));
        source = source.CloneForEditing(); // Private read snapshot for the whole transaction.
        var snapshot = source.CaptureFamily(rootSlot, rootUid);
        var plan = PlanFamilyCoreAllocation(source, rootSlot, rootUid);
        if (snapshot is null || plan is null) return null;
        var candidate = CloneForEditing();
        static int Remap(int source, IReadOnlyDictionary<int, int> map)
            => source == 0xFFFF || source == -1 ? source : map.TryGetValue(source, out int target) ? target
                : throw new InvalidDataException("群組副本缺少必要的索引映射。");
        foreach (var pair in plan.Positions)
            source._positions.AsSpan(8 + pair.Key * PositionSize, PositionSize).CopyTo(
                candidate._positions.AsSpan(8 + pair.Value * PositionSize));
        foreach (var pair in plan.ObjData)
            for (int segment = 0; segment < ObjDataWidths.Length; segment++)
                source._objdata.AsSpan(source.SegmentOffset(segment, pair.Key), ObjDataWidths[segment]).CopyTo(
                    candidate._objdata.AsSpan(candidate.SegmentOffset(segment, pair.Value)));
        var sourceStore = source;
        void Auxiliary(byte[]? source, byte[]? target, IReadOnlyDictionary<int, int> map, int header, int stride, int tails)
        {
            foreach (var pair in map)
            {
                source!.AsSpan(header + pair.Key * stride, stride).CopyTo(target!.AsSpan(header + pair.Value * stride));
                for (int tail = 0; tail < tails; tail++)
                    source.AsSpan(header + sourceStore._count * stride + tail * sourceStore._count * 2 + pair.Key * 2, 2).CopyTo(
                        target.AsSpan(header + candidate._count * stride + tail * candidate._count * 2 + pair.Value * 2));
            }
        }
        Auxiliary(source._anim, candidate._anim, plan.Anim, 8, 21, 2);
        Auxiliary(source._gfxtype, candidate._gfxtype, plan.GfxType, 8, 15, 1);
        Auxiliary(source._action, candidate._action, plan.Action, 12, 25, 0);
        foreach (var pair in plan.Objects)
        {
            int sourceRecord = source.RecordOffset(pair.Key), targetRecord = candidate.RecordOffset(pair.Value);
            source._objects.AsSpan(sourceRecord, RecordSize).CopyTo(candidate._objects.AsSpan(targetRecord));
            BinaryPrimitives.WriteUInt32LittleEndian(candidate._objects.AsSpan(targetRecord + 3), plan.Uids[pair.Key]);
            if (team is { } owner) BinaryPrimitives.WriteUInt16LittleEndian(candidate._objects.AsSpan(targetRecord + 1), checked((ushort)owner));
            foreach (var link in new[] { (67, plan.Positions), (69, plan.Positions), (71, plan.Anim), (73, plan.GfxType), (77, plan.Action) })
                BinaryPrimitives.WriteUInt16LittleEndian(candidate._objects.AsSpan(targetRecord + link.Item1),
                    (ushort)Remap(BinaryPrimitives.ReadUInt16LittleEndian(source._objects.AsSpan(sourceRecord + link.Item1)), link.Item2));
            for (int column = 0; column < ColumnWidths.Length; column++)
            {
                var map = column switch { ObjDataColumn => plan.ObjData, 5 => plan.Lagers, ThirdPositionColumn => plan.Positions,
                    11 => plan.Particles, DirectLightColumn => plan.Lights, ParentObjectColumn => plan.Objects, _ => null };
                uint value = source.ReadColumn(column, pair.Key);
                candidate.WriteColumn(column, pair.Value, map is null ? value : (uint)Remap((int)value, map));
            }
        }
        foreach (var pair in plan.Lights)
        {
            var light = snapshot.Lights[pair.Key];
            candidate._lights!.InstallFamilyCopy(pair.Value, light, Remap(light.OwnerObject, plan.Objects), Remap(light.ParentObject, plan.Objects));
        }
        static IReadOnlySet<int> Exclude(int capacity, IReadOnlyDictionary<int, int> map)
            => Enumerable.Range(0, capacity).Except(map.Values).ToHashSet();
        static void Check(int[]? actual, IReadOnlyDictionary<int, int> map)
        {
            if (actual is null || !actual.SequenceEqual(map.Values))
                throw new InvalidDataException("群組副本的池配置與預檢計畫不一致。");
        }
        if (plan.Particles.Count > 0) Check(candidate._particles!.AddCopies(plan.Particles.Keys.Select(slot =>
            new NativeParticleCopy(snapshot.Particles[slot], Remap(snapshot.Particles[slot].OwnerObject, plan.Objects), plan.Lights)).ToArray(),
            Exclude(NativeParticlePool.Capacity, plan.Particles)), plan.Particles);
        if (plan.BigLagers.Count > 0) Check(candidate._bigLagers!.AddCopies(plan.BigLagers.Keys.Select(slot =>
            new NativeBigLagerCopy(snapshot.BigLagers[slot], plan.Objects)).ToArray(),
            Exclude(NativeBigLagerPool.Capacity, plan.BigLagers)), plan.BigLagers);
        if (plan.Lagers.Count > 0) Check(candidate._lagers!.AddCopies(plan.Lagers.Keys.Select(slot =>
            new NativeLagerCopy(snapshot.Lagers[slot], plan.Objects, plan.BigLagers)).ToArray(),
            Exclude(NativeLagerPool.Capacity, plan.Lagers)), plan.Lagers);
        candidate._nextUid = _nextUid + (uint)plan.Objects.Count;
        if (candidate._engine is { Length: >= 38 })
            BinaryPrimitives.WriteUInt32LittleEndian(candidate._engine.AsSpan(34), candidate._nextUid);
        candidate = candidate.CloneForEditing(); // Rebuild all ownership caches from the completed copied state.
        int newRoot = plan.Objects[rootSlot]; uint newUid = plan.Uids[rootSlot];
        if (candidate.FamilyAt(newRoot, newUid) is null)
            throw new InvalidDataException("群組副本未形成獨立有效的閉包。");
        if ((dx != 0 || dy != 0 || dz != 0) && !candidate.TranslateFamilyIfUidFixed(newRoot, newUid, dx, dy, dz))
            throw new InvalidDataException("群組副本無法套用位移。");
        return new(candidate, newRoot, newUid, plan);
    }
    public LevelObjectFamilyCoreAllocation? PlanFamilyCoreAllocation(int rootSlot, uint rootUid)
        => PlanFamilyCoreAllocation(this, rootSlot, rootUid);

    private LevelObjectFamilyCoreAllocation? PlanFamilyCoreAllocation(LevelObjectStore sourceStore, int rootSlot, uint rootUid)
    {
        var family = sourceStore.FamilyAt(rootSlot, rootUid);
        if (_count is < 1 or > 14000 || sourceStore._count is < 1 or > 14000 || _unavailablePools.Count > 0) return null;
        if (family is null) return null;
        int count = family.ObjectSlots.Count;
        if ((long)Objects().Count + count > 14000) return null;
        if (family.LightSlots.Count > 0 && _lights is null || family.ParticleSlots.Count > 0 && _particles is null
            || family.LagerSlots.Count > 0 && _lagers is null || family.BigLagerSlots.Count > 0 && _bigLagers is null) return null;
        if (_nextUid == 0 || _nextUid > 0x00FFFFFF || (ulong)_nextUid + (uint)count - 1 > 0x00FFFFFF)
            throw new InvalidDataException("群組複製所需的 UID 超出原生 24-bit 範圍。");
        var particleOwners = _particles?.ActiveEmitters().Select(emitter => (int)emitter.OwnerObject).ToHashSet()
            ?? new HashSet<int>();
        int[] objects = Enumerable.Range(0, _count).Where(slot => !IsActive(slot)
            && !_referencedParents.Contains(slot) && !_lightParents.Contains(slot)
            && !_lightOwners.ContainsKey(slot) && !_garrisonMembers.Contains(slot)
            && !particleOwners.Contains(slot)).Take(count).ToArray();
        int[] sourcePositions = family.ObjectSlots.SelectMany(slot => new[] {
            (int)BinaryPrimitives.ReadUInt16LittleEndian(sourceStore._objects.AsSpan(sourceStore.RecordOffset(slot) + 67)),
            (int)BinaryPrimitives.ReadUInt16LittleEndian(sourceStore._objects.AsSpan(sourceStore.RecordOffset(slot) + 69)),
            (int)sourceStore.ReadColumn(ThirdPositionColumn, slot) }).Where(slot => slot != 0xFFFF).Distinct().ToArray();
        int[] positions = FreePositions(sourcePositions.Length);
        int[] sourceData = family.ObjectSlots.Select(slot => (int)sourceStore.ReadColumn(ObjDataColumn, slot)).Distinct().ToArray();
        int[] data = Enumerable.Range(0, _count).Where(slot => _objdata[SegmentOffset(0, slot)] == 0
            && !DataReferenced(slot)).Take(sourceData.Length).ToArray();
        if (objects.Length != count || positions.Length != sourcePositions.Length || data.Length != sourceData.Length)
            return null;
        static IReadOnlyDictionary<int, T> Map<T>(IReadOnlyList<int> source, IReadOnlyList<T> target)
            => new System.Collections.ObjectModel.ReadOnlyDictionary<int, T>(
                source.Select((slot, index) => (slot, index)).ToDictionary(pair => pair.slot, pair => target[pair.index]));
        IReadOnlyDictionary<int, int>? Auxiliary(byte[]? sourcePool, byte[]? pool, int header, int stride, int link)
        {
            int[] source = family.ObjectSlots.Select(slot =>
                (int)BinaryPrimitives.ReadUInt16LittleEndian(sourceStore._objects.AsSpan(sourceStore.RecordOffset(slot) + link)))
                .Where(slot => slot != 0xFFFF).Distinct().ToArray();
            if (source.Length == 0) return Map(source, Array.Empty<int>());
            if (sourcePool is null || source.Any(slot => slot >= sourceStore._count || sourcePool[header + slot * stride] == 0))
                throw new InvalidDataException("群組輔助池連結無效，不能配置複製計畫。");
            if (pool is null) return null;
            int[] target = Enumerable.Range(0, _count).Where(slot => pool[header + slot * stride] == 0
                && !RecordReferenced(link, slot)).Take(source.Length).ToArray();
            return target.Length == source.Length ? Map(source, target) : null;
        }
        var anim = Auxiliary(sourceStore._anim, _anim, 8, 21, 71);
        var gfx = Auxiliary(sourceStore._gfxtype, _gfxtype, 8, 15, 73);
        var action = Auxiliary(sourceStore._action, _action, 12, 25, 77);
        if (anim is null || gfx is null || action is null) return null;
        HashSet<int> ObjectReferences(int column) => Enumerable.Range(0, _count).Where(IsActive)
            .Select(slot => (int)ReadColumn(column, slot)).Where(slot => slot != 0xFFFF).ToHashSet();
        IReadOnlyDictionary<int, int>? Dedicated(IReadOnlyList<int> source, int capacity,
            Func<int, bool> active, IReadOnlySet<int> reserved)
        {
            int[] target = Enumerable.Range(0, capacity).Where(slot => !active(slot) && !reserved.Contains(slot))
                .Take(source.Count).ToArray();
            return target.Length == source.Count ? Map(source, target) : null;
        }
        var lightRefs = ObjectReferences(DirectLightColumn);
        if (_particles is not null)
            lightRefs.UnionWith(_particles.ActiveEmitters().SelectMany(emitter => emitter.LightSlots));
        var lights = Dedicated(family.LightSlots, NativeLightPool.Capacity,
            slot => _lights?.Capture(slot) is not null, lightRefs);
        var particles = Dedicated(family.ParticleSlots, NativeParticlePool.Capacity,
            slot => _particles?.Capture(slot) is not null, ObjectReferences(11));
        var lagers = Dedicated(family.LagerSlots, NativeLagerPool.Capacity,
            slot => _lagers?.Capture(slot) is not null, ObjectReferences(5));
        var bigRefs = _lagers?.ActiveLagers().Select(camp => (int)camp.BigLager).ToHashSet() ?? new HashSet<int>();
        var bigLagers = Dedicated(family.BigLagerSlots, NativeBigLagerPool.Capacity,
            slot => _bigLagers?.Capture(slot) is not null, bigRefs);
        if (lights is null || particles is null || lagers is null || bigLagers is null) return null;
        return new(Map(family.ObjectSlots, objects),
            Map(family.ObjectSlots, Enumerable.Range(0, count).Select(index => _nextUid + (uint)index).ToArray()),
            Map(sourcePositions, positions), Map(sourceData, data)) { Anim = anim, GfxType = gfx, Action = action,
                Lights = lights, Particles = particles, Lagers = lagers, BigLagers = bigLagers };
    }
    /// <summary>Independent candidate state for cross-pool edits. Preserves native headers and UID cursor;
    /// accepting or saving this candidate is a separate operation.</summary>
    public LevelObjectStore CloneForEditing()
    {
        var clone = new LevelObjectStore(_objects.ToArray(), _objdata.ToArray(), _positions.ToArray(),
            _objectsHeader?.ToArray(), _objdataHeader?.ToArray(), _positionsHeader?.ToArray(),
            _anim?.ToArray(), _animHeader?.ToArray(), _gfxtype?.ToArray(), _gfxtypeHeader?.ToArray(),
            _action?.ToArray(), _actionHeader?.ToArray(), _engine?.ToArray(), _engineHeader?.ToArray(),
            _unavailablePools.ToArray(), _definitions, _lights?.CloneForEditing(), _lightDefinitions,
            _particles?.CloneForEditing(), _lagers?.CloneForEditing(), _bigLagers?.CloneForEditing());
        clone._nextUid = _nextUid;
        return clone;
    }
    /// <summary>Captures only a prevalidated independent family; no mutation or index/UID allocation.</summary>
    public LevelObjectFamilySnapshot? CaptureFamily(int rootSlot, uint rootUid)
    {
        var family = FamilyAt(rootSlot, rootUid);
        if (family is null) return null;
        static IReadOnlyDictionary<int, T> Capture<T>(IReadOnlyList<int> slots, Func<int, T?> read) where T : class
        {
            var result = new Dictionary<int, T>();
            foreach (int slot in slots)
                result.Add(slot, read(slot) ?? throw new InvalidDataException("群組快照的關聯資料已失效。"));
            return new System.Collections.ObjectModel.ReadOnlyDictionary<int, T>(result);
        }
        return new(rootSlot, rootUid, Capture(family.ObjectSlots, CaptureSavedTemplate),
            Capture(family.LightSlots, slot => _lights?.Capture(slot)),
            Capture(family.ParticleSlots, slot => _particles?.CaptureSnapshot(slot)),
            Capture(family.LagerSlots, slot => _lagers?.CaptureSnapshot(slot)),
            Capture(family.BigLagerSlots, slot => _bigLagers?.CaptureSnapshot(slot)));
    }
    /// <summary>
    /// Translates an independent saved family by exact signed16.16 deltas, retaining UIDs, indices,
    /// directions and relative offsets. All candidate positions/effects are validated before mutation.
    /// Offline saved-state editing; no native terrain clamping, simulation or attachment reconstruction.
    /// </summary>
    public bool TranslateFamilyIfUidFixed(int rootSlot, uint rootUid, int dx, int dy, int dz)
    {
        var family = FamilyAt(rootSlot, rootUid);
        if (family is null) return false;
        double[] delta = [dx / 65536d, dy / 65536d, dz / 65536d];
        float Translate(float value, int axis)
        {
            float result = (float)(value + delta[axis]);
            if (!float.IsFinite(value) || !float.IsFinite(result))
                throw new InvalidDataException("整組物件的座標必須是有限 float32 值。");
            return result;
        }
        var positionSlots = family.ObjectSlots.SelectMany(slot => new[] {
            (int)BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 67)),
            (int)BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 69)),
            (int)ReadColumn(ThirdPositionColumn, slot) }).Where(slot => slot != ushort.MaxValue).Distinct().ToArray();
        byte[] positions = _positions.ToArray();
        foreach (int slot in positionSlots)
            for (int axis = 0; axis < 3; axis++)
            {
                int offset = 8 + slot * PositionSize + 1 + axis * 4;
                float value = BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(offset));
                BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(offset), Translate(value, axis));
            }
        var particles = family.ParticleSlots.Count == 0 ? null : NativeParticlePool.ParsePayload(_particles!.ToPayload());
        var childLights = particles?.TranslateBatchFixed(family.ParticleSlots, dx, dy, dz)
            ?? Array.Empty<NativeParticleLightPosition>();
        var childTargets = childLights.Select(item => item.LightSlot).ToHashSet();
        var targets = childLights.Select(item => new NativeLightPosition(item.LightSlot, item.Position)).ToList();
        foreach (int slot in family.LightSlots)
        {
            if (childTargets.Contains(slot)) continue;
            var light = _lights!.Capture(slot)!;
            targets.Add(new(slot, new(Translate(light.Position.X, 0), Translate(light.Position.Y, 1), Translate(light.Position.Z, 2))));
        }
        var lights = family.LightSlots.Count == 0 ? null : NativeLightPool.ParsePayload(_lights!.ToPayload());
        lights?.SetPositionsBatch(targets);
        // Fixed-size copies cannot fail after complete preflight; preserve each original PFIL header.
        positions.CopyTo(_positions, 0);
        if (particles is not null) _particles!.CopyPayloadFrom(particles);
        if (lights is not null) _lights!.CopyPayloadFrom(lights);
        return true;
    }

    /// <summary>Returns a complete removable family only for a current root UID and supported independent pool links.</summary>
    public LevelObjectFamily? FamilyAt(int rootSlot, uint rootUid)
    {
        if (rootUid == 0 || UidAt(rootSlot) != rootUid || _unavailablePools.Count > 0
            || ReadColumn(ParentObjectColumn, rootSlot) != 0xFFFF) return null;
        var members = new HashSet<int> { rootSlot };
        var order = new List<int> { rootSlot };
        var children = new Dictionary<int, List<int>>();
        for (int slot = 0; slot < _count; slot++)
        {
            if (!IsActive(slot)) continue;
            int parent = (int)ReadColumn(ParentObjectColumn, slot);
            if (parent == 0xFFFF) continue;
            if (!children.TryGetValue(parent, out var list)) children[parent] = list = [];
            list.Add(slot);
        }
        var lagerSlots = new HashSet<int>(); var bigSlots = new HashSet<int>();
        var garrisonOwners = new Dictionary<int, int>();
        var edges = new Dictionary<int, HashSet<int>>();
        for (int cursor = 0; cursor < order.Count; cursor++)
        {
            int owner = order[cursor]; var related = new HashSet<int>();
            if (children.TryGetValue(owner, out var descendants)) related.UnionWith(descendants);
            int campSlot = (int)ReadColumn(5, owner);
            if (campSlot != ushort.MaxValue)
            {
                var camp = _lagers?.Capture(campSlot);
                if (camp is null || !lagerSlots.Add(campSlot)) return null;
                var occupants = new HashSet<int>();
                foreach (int member in camp.SmallMembers)
                    if (!occupants.Add(member)) return null;
                int capacity = 10, occupiedCount = camp.SmallMembers.Count;
                if (camp.BigLager != -1)
                {
                    var big = _bigLagers?.Capture(camp.BigLager);
                    if (big is null || !bigSlots.Add(big.Slot)) return null;
                    if (big.Members.Distinct().Count() != big.Members.Count) return null;
                    occupants.UnionWith(big.Members); capacity = 800; occupiedCount = big.Members.Count;
                }
                if (camp.Occupancy < 0 || camp.Occupancy > capacity || camp.Occupancy != occupiedCount) return null;
                foreach (int member in occupants)
                {
                    if (member < 0 || member >= _count || !IsActive(member) || member == owner
                        || garrisonOwners.TryGetValue(member, out int previous) && previous != campSlot) return null;
                    garrisonOwners[member] = campSlot;
                    related.Add(member);
                }
            }
            edges[owner] = related;
            foreach (int child in related.Order()) if (members.Add(child)) order.Add(child);
        }
        // A member may also be a descendant. Shared paths are valid; actual dependency cycles are not.
        var incoming = members.ToDictionary(slot => slot, _ => 0);
        foreach (var outgoing in edges.Values) foreach (int child in outgoing) incoming[child]++;
        var ready = new Queue<int>(order.Where(slot => incoming[slot] == 0)); var sorted = new List<int>();
        while (ready.TryDequeue(out int parent))
        {
            sorted.Add(parent);
            foreach (int child in edges[parent].Order()) if (--incoming[child] == 0) ready.Enqueue(child);
        }
        if (sorted.Count != members.Count) return null;
        order = sorted;
        foreach (int slot in order)
        {
            int parent = (int)ReadColumn(ParentObjectColumn, slot);
            if (parent != ushort.MaxValue && !members.Contains(parent)) return null;
        }
        // Every external user must survive; an unreferenced active biglager still owns its saved members.
        if (Enumerable.Range(0, _count).Any(slot => IsActive(slot) && !members.Contains(slot)
            && lagerSlots.Contains((int)ReadColumn(5, slot)))) return null;
        if (_lagers is not null && _lagers.ActiveLagers().Any(camp => !lagerSlots.Contains(camp.Slot)
            && (camp.SmallMembers.Any(members.Contains) || bigSlots.Contains(camp.BigLager)))) return null;
        if (_bigLagers is not null && _bigLagers.ActiveLagers().Any(big => !bigSlots.Contains(big.Slot)
            && big.Members.Any(members.Contains))) return null;
        if (order.Count > 1 && _lights is null) return null; // Missing effect data cannot prove a complete attached family.
        foreach (int slot in order)
        {
            if (IsLinked(slot, includeFamilyLinks: false)) return null;
            int record = RecordOffset(slot);
            int p0 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67));
            int p1 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69));
            if (p0 == 0xFFFF || p1 == 0xFFFF || !ValidOptionalPosition(p0) || !ValidOptionalPosition(p1)) return null;
        }
        // Any pool slot owned outside the family must survive; reject before the first mutation.
        var data = order.Select(slot => (int)ReadColumn(ObjDataColumn, slot)).ToHashSet();
        var positions = order.SelectMany(slot => new[] {
            (int)BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 67)),
            (int)BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 69)),
            (int)ReadColumn(ThirdPositionColumn, slot) }).Where(slot => slot != 0xFFFF).ToHashSet();
        var auxiliary = new[] { 71, 73, 77 }.ToDictionary(offset => offset,
            offset => order.Select(slot => (int)BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + offset)))
                .Where(slot => slot != 0xFFFF).ToHashSet());
        for (int slot = 0; slot < _count; slot++)
        {
            if (!IsActive(slot) || members.Contains(slot)) continue;
            int record = RecordOffset(slot);
            if (data.Contains((int)ReadColumn(ObjDataColumn, slot))
                || positions.Contains(BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67)))
                || positions.Contains(BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69)))
                || positions.Contains((int)ReadColumn(ThirdPositionColumn, slot))
                || auxiliary.Any(pair => pair.Value.Contains(BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + pair.Key)))))
                return null;
        }
        var particleSlots = new HashSet<int>();
        var particleLights = new HashSet<int>();
        foreach (int slot in order)
        {
            int target = (int)ReadColumn(11, slot);
            if (target == 0xFFFF) continue;
            var emitter = _particles?.Capture(target);
            if (emitter is null || emitter.OwnerObject != slot || !particleSlots.Add(target)) return null;
            foreach (int light in emitter.LightSlots)
            {
                if (_lights?.Capture(light) is not { } saved || saved.OwnerObject >= 0 && !members.Contains(saved.OwnerObject)
                    || saved.ParentObject >= 0 && !members.Contains(saved.ParentObject)) return null;
                particleLights.Add(light);
            }
        }
        if (_particles is not null)
            foreach (var emitter in _particles.ActiveEmitters())
            {
                if (members.Contains(emitter.OwnerObject) && !particleSlots.Contains(emitter.Slot)) return null;
            }
        // Reject an outside object's dangling backlink even if its emitter reports an inside owner.
        if (Enumerable.Range(0, _count).Any(slot => IsActive(slot) && !members.Contains(slot) && particleSlots.Contains((int)ReadColumn(11, slot)))) return null;
        var lightSlots = new List<int>(particleLights);
        if (_lights is not null)
            foreach (var light in _lights.ActiveLights())
            {
                bool ownerInside = members.Contains(light.OwnerObject), parentInside = members.Contains(light.ParentObject);
                if (!ownerInside && !parentInside) continue;
                // Direct and APT parent light are separate ownership forms. Mixed links are unsupported.
                if (ownerInside && light.ParentObject != -1 || parentInside && light.OwnerObject != -1) return null;
                if (parentInside && light.Definition is < 0 or >= 256) return null;
                if (!lightSlots.Contains(light.Slot)) lightSlots.Add(light.Slot);
            }
        var lightTargets = lightSlots.ToHashSet();
        if (_particles is not null && _particles.ActiveEmitters().Any(emitter => !particleSlots.Contains(emitter.Slot)
            && emitter.LightSlots.Any(lightTargets.Contains))) return null;
        if (Enumerable.Range(0, _count).Any(slot => IsActive(slot) && !members.Contains(slot)
            && lightTargets.Contains((int)ReadColumn(DirectLightColumn, slot)))) return null;
        return new(rootSlot, rootUid, order.AsReadOnly(), lightSlots.AsReadOnly())
        {
            ParticleSlots = particleSlots.Order().ToArray(), LagerSlots = lagerSlots.Order().ToArray(),
            BigLagerSlots = bigSlots.Order().ToArray()
        };
    }

    /// <summary>Removes the entire current root family after a complete preflight; Save persists all pools in one rollback scope.</summary>
    public bool RemoveFamilyIfUid(int rootSlot, uint rootUid)
    {
        var family = FamilyAt(rootSlot, rootUid);
        if (family is null) return false;
        // Releasing parent-only APT lights requires no object direct-light backlink changes.
        int[] attached = family.LightSlots.Where(slot => _lights!.Capture(slot)!.OwnerObject < 0).ToArray();
        _particles?.ReleaseBatch(family.ParticleSlots);
        _lagers?.ReleaseBatch(family.LagerSlots);
        _bigLagers?.ReleaseBatch(family.BigLagerSlots);
        _lights?.ReleaseBatch(attached);
        for (int index = family.ObjectSlots.Count - 1; index >= 0; index--) RemoveCore(family.ObjectSlots[index]);
        _referencedParents.Clear(); _lightParents.Clear(); _lightOwners.Clear();
        RebuildGarrisonMembers();
        for (int slot = 0; slot < _count; slot++)
            if (IsActive(slot) && ReadColumn(ParentObjectColumn, slot) is < 0xFFFF and var parent) _referencedParents.Add((int)parent);
        if (_lights is not null)
            foreach (var light in _lights.ActiveLights())
            {
                if (light.ParentObject >= 0) _lightParents.Add(light.ParentObject);
                if (light.OwnerObject >= 0) _lightOwners[light.OwnerObject] = _lightOwners.ContainsKey(light.OwnerObject) ? -1 : light.Slot;
            }
        return true;
    }

    private void RebuildGarrisonMembers()
    {
        _garrisonMembers.Clear();
        if (_lagers is not null) foreach (var camp in _lagers.ActiveLagers()) _garrisonMembers.UnionWith(camp.SmallMembers);
        if (_bigLagers is not null) foreach (var big in _bigLagers.ActiveLagers()) _garrisonMembers.UnionWith(big.Members);
    }
}
