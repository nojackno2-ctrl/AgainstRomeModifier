using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

public sealed record NativeSavedEmitter(int Slot, ushort Active, short OwnerObject, IReadOnlyList<int> LightSlots);
public sealed record NativeParticleRelease(IReadOnlyList<int> OwnerObjects, IReadOnlyList<int> LightSlots);
public sealed record NativeParticleLightPosition(int EmitterSlot, int Child, int LightSlot, System.Numerics.Vector3 Position);
public sealed record NativeParticleCopy(NativeParticleSnapshot Snapshot, int OwnerObject,
    IReadOnlyDictionary<int, int> LightMap);

/// <summary>Detached full saved emitter, including inactive children and four parallel state words.</summary>
public sealed class NativeParticleSnapshot
{
    private readonly byte[] _record;
    private readonly int[] _state;
    internal NativeParticleSnapshot(byte[] record, int[] state) { _record = record; _state = state; }
    public byte[] RecordBytes() => _record.ToArray();
    public int[] StateWords() => _state.ToArray();
    public short OwnerObject => BinaryPrimitives.ReadInt16LittleEndian(_record.AsSpan(6));
    public IReadOnlyList<int> LightSlots => Enumerable.Range(0, NativeParticlePool.Children)
        .Select(child => (int)BinaryPrimitives.ReadInt16LittleEndian(_record.AsSpan(12 + child * 35 + 33)))
        .Where(slot => slot != -1).ToArray();
}

/// <summary>Native version1 particle records and four parallel arrays; no runtime simulation.</summary>
public sealed class NativeParticlePool
{
    public const int Capacity = 1024, Children = 64, RecordSize = 2252;
    public const int PayloadLength = 12 + Capacity * (RecordSize + 16);
    private readonly byte[] _payload;
    private readonly byte[]? _header;
    private NativeParticlePool(byte[] payload, byte[]? header)
    {
        if (payload.Length != PayloadLength || BinaryPrimitives.ReadInt32LittleEndian(payload) != 1
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(4)) != Capacity
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(8)) != Children)
            throw new InvalidDataException("particle.dat 必須是 version1／1024 槽／64 子項的完整布局。");
        _payload = payload.ToArray(); _header = header;
    }
    public static NativeParticlePool ParsePayload(ReadOnlySpan<byte> payload) => new(payload.ToArray(), null);
    public static NativeParticlePool Load(string path)
    {
        byte[] packed = File.ReadAllBytes(path); bool wrapped = packed.AsSpan().StartsWith("PFIL"u8);
        if (wrapped && packed.Length < 64) throw new InvalidDataException("particle.dat PFIL 標頭不完整。");
        return new(GameLZSS.DecompressPfil(packed), wrapped ? packed[..64] : null);
    }
    public byte[] ToPayload() => _payload.ToArray();
    internal NativeParticlePool CloneForEditing() => new(_payload, _header?.ToArray());
    internal void CopyPayloadFrom(NativeParticlePool source) => source._payload.CopyTo(_payload, 0);
    public NativeParticleSnapshot? CaptureSnapshot(int slot)
    {
        if (Capture(slot) is null) return null;
        byte[] record = _payload.AsSpan(12 + slot * RecordSize, RecordSize).ToArray();
        int[] state = Enumerable.Range(0, 4).Select(column => BinaryPrimitives.ReadInt32LittleEndian(
            _payload.AsSpan(12 + Capacity * RecordSize + column * Capacity * 4 + slot * 4))).ToArray();
        return new(record, state);
    }
    /// <summary>Copies saved emitters after whole-batch preflight. Caller reserves dangling object references
    /// and allocates/validates mapped lights and owners in the same family transaction.</summary>
    public int[]? AddCopies(IReadOnlyList<NativeParticleCopy> copies, IReadOnlySet<int>? excludedSlots = null)
    {
        ArgumentNullException.ThrowIfNull(copies);
        var candidates = new List<(byte[] Record, int[] State)>();
        var owners = new HashSet<int>();
        var batchLightTargets = new HashSet<int>();
        foreach (var copy in copies)
        {
            ArgumentNullException.ThrowIfNull(copy);
            ArgumentNullException.ThrowIfNull(copy.Snapshot);
            ArgumentNullException.ThrowIfNull(copy.LightMap);
            if (copy.OwnerObject is < 0 or >= 14000) throw new ArgumentOutOfRangeException(nameof(copies));
            if (!owners.Add(copy.OwnerObject))
                throw new InvalidDataException("同一物件不能擁有多個新粒子發射器。");
            byte[] record = copy.Snapshot.RecordBytes();
            BinaryPrimitives.WriteInt16LittleEndian(record.AsSpan(6), (short)copy.OwnerObject);
            var targets = new Dictionary<int, int>();
            for (int child = 0; child < Children; child++)
            {
                int offset = 12 + child * 35 + 33;
                int source = BinaryPrimitives.ReadInt16LittleEndian(record.AsSpan(offset));
                if (source == -1) continue;
                if (source is < 0 or >= NativeLightPool.Capacity
                    || !copy.LightMap.TryGetValue(source, out int target)
                    || target is < 0 or >= NativeLightPool.Capacity)
                    throw new InvalidDataException("粒子快照的每個光源連結都必須有有效的新索引。");
                if (targets.TryGetValue(target, out int previous) && previous != source)
                    throw new InvalidDataException("不同粒子光源不能合併至同一新槽位。");
                targets[target] = source;
                BinaryPrimitives.WriteInt16LittleEndian(record.AsSpan(offset), (short)target);
            }
            foreach (int target in targets.Keys)
                if (!batchLightTargets.Add(target))
                    throw new InvalidDataException("不同新粒子發射器不能共用光源槽位。");
            candidates.Add((record, copy.Snapshot.StateWords()));
        }
        int[] slots = Enumerable.Range(0, Capacity).Where(slot => Capture(slot) is null
            && excludedSlots?.Contains(slot) != true).Take(candidates.Count).ToArray();
        if (slots.Length != candidates.Count) return null;
        for (int item = 0; item < slots.Length; item++)
        {
            int slot = slots[item];
            candidates[item].Record.CopyTo(_payload, 12 + slot * RecordSize);
            for (int column = 0; column < 4; column++)
                BinaryPrimitives.WriteInt32LittleEndian(_payload.AsSpan(
                    12 + Capacity * RecordSize + column * Capacity * 4 + slot * 4), candidates[item].State[column]);
        }
        return slots;
    }
    public NativeSavedEmitter? Capture(int slot)
    {
        if (slot is < 0 or >= Capacity) return null;
        int record = 12 + slot * RecordSize;
        ushort active = BinaryPrimitives.ReadUInt16LittleEndian(_payload.AsSpan(record));
        if (active == 0) return null;
        var lights = new List<int>();
        for (int child = 0; child < Children; child++)
        {
            short light = BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(record + 12 + child * 35 + 33));
            if (light != -1) lights.Add(light);
        }
        return new(slot, active, BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(record + 6)), lights.AsReadOnly());
    }
    public IReadOnlyList<NativeSavedEmitter> ActiveEmitters()
        => Enumerable.Range(0, Capacity).Select(Capture).OfType<NativeSavedEmitter>().ToArray();
    /// <summary>
    /// Atomically updates saved particles and their active child lights. The family caller must still
    /// validate external owners/sharing; this operation does not move object positions or APT attachments.
    /// </summary>
    public IReadOnlyList<NativeParticleLightPosition> TranslateBatchWithLightsFixed(
        IReadOnlyList<int> slots, int dx, int dy, int dz, NativeLightPool lights)
    {
        ArgumentNullException.ThrowIfNull(lights);
        var candidate = ParsePayload(_payload);
        var positions = candidate.TranslateBatchFixed(slots, dx, dy, dz);
        lights.SetPositionsBatch(positions.Select(item => new NativeLightPosition(item.LightSlot, item.Position)).ToArray());
        candidate._payload.CopyTo(_payload, 0);
        return positions;
    }
    /// <summary>
    /// Translates saved active children using exact signed 16.16 deltas. Preflights the entire batch
    /// before writing; the caller must update returned lights in the same transaction.
    /// This does not advance native simulation or choose a float-to-fixed rounding policy.
    /// </summary>
    public IReadOnlyList<NativeParticleLightPosition> TranslateBatchFixed(IReadOnlyList<int> slots, int dx, int dy, int dz)
    {
        ArgumentNullException.ThrowIfNull(slots);
        var writes = new List<(int Offset, int Value)>();
        var lights = new List<NativeParticleLightPosition>();
        foreach (int slot in slots.Distinct())
        {
            if (slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(slots));
            if (Capture(slot) is null) throw new InvalidDataException("無法移動未啟用的粒子發射器。");
            for (int child = 0; child < Children; child++)
            {
                int offset = 12 + slot * RecordSize + 12 + child * 35;
                if (_payload[offset] == 0) continue;
                int x = checked(BinaryPrimitives.ReadInt32LittleEndian(_payload.AsSpan(offset + 1)) + dx);
                int y = checked(BinaryPrimitives.ReadInt32LittleEndian(_payload.AsSpan(offset + 5)) + dy);
                int z = checked(BinaryPrimitives.ReadInt32LittleEndian(_payload.AsSpan(offset + 9)) + dz);
                int light = BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(offset + 33));
                if (light is < -1 or >= NativeLightPool.Capacity) throw new InvalidDataException("粒子子項的光源索引無效。");
                writes.Add((offset + 1, x)); writes.Add((offset + 5, y)); writes.Add((offset + 9, z));
                if (light >= 0) lights.Add(new(slot, child, light, new(x >> 16, y >> 16, z >> 16)));
            }
        }
        foreach (var write in writes) BinaryPrimitives.WriteInt32LittleEndian(_payload.AsSpan(write.Offset), write.Value);
        return lights.AsReadOnly();
    }
    /// <summary>Prevalidates all slots; caller must include returned owners/lights in the same family transaction.</summary>
    public NativeParticleRelease ReleaseBatch(IReadOnlyList<int> slots)
    {
        ArgumentNullException.ThrowIfNull(slots);
        foreach (int slot in slots) if (slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(slots));
        var distinct = slots.Distinct().ToArray(); var emitters = distinct.Select(Capture).OfType<NativeSavedEmitter>().ToArray();
        var result = new NativeParticleRelease(emitters.Select(e => (int)e.OwnerObject).Where(o => o >= 0).Distinct().ToArray(),
            emitters.SelectMany(e => e.LightSlots).Distinct().ToArray());
        foreach (int slot in distinct)
        {
            int record = 12 + slot * RecordSize; _payload.AsSpan(record, RecordSize).Clear();
            BinaryPrimitives.WriteInt16LittleEndian(_payload.AsSpan(record + 6), -1);
            for (int child = 0; child < Children; child++)
                BinaryPrimitives.WriteInt16LittleEndian(_payload.AsSpan(record + 12 + child * 35 + 33), -1);
            for (int column = 0; column < 4; column++)
                BinaryPrimitives.WriteInt32LittleEndian(_payload.AsSpan(12 + Capacity * RecordSize + column * Capacity * 4 + slot * 4), column == 3 ? 30 : 0);
        }
        return result;
    }
    public void Save(string path, FileRollbackScope rollback)
        => Core.Services.SafeFileWriter.WriteAllBytes(path, _header is null ? _payload : GameLZSS.CompressPfil(_payload, _header), rollback);
}
