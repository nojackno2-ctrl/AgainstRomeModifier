using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

public sealed record NativeSavedLager(int Slot, byte Active, short Occupancy, IReadOnlyList<int> SmallMembers, short BigLager);

/// <summary>Complete detached small-camp record and its separately serialized biglager link.</summary>
public sealed class NativeLagerSnapshot
{
    private readonly byte[] _record;
    internal NativeLagerSnapshot(byte[] record, short bigLager) { _record = record; BigLager = bigLager; }
    public short BigLager { get; }
    public byte[] RecordBytes() => _record.ToArray();
}

/// <summary>Complete detached large-camp record, preserving all 800 member positions and sentinels.</summary>
public sealed class NativeBigLagerSnapshot
{
    private readonly byte[] _record;
    internal NativeBigLagerSnapshot(byte[] record) { _record = record; }
    public byte[] RecordBytes() => _record.ToArray();
}
public sealed record NativeBigLagerCopy(NativeBigLagerSnapshot Snapshot, IReadOnlyDictionary<int, int> ObjectMap);
public sealed record NativeLagerCopy(NativeLagerSnapshot Snapshot, IReadOnlyDictionary<int, int> ObjectMap,
    IReadOnlyDictionary<int, int> BigLagerMap);

/// <summary>Serialized native lager records and independent biglager index array. Does not simulate garrison lifecycle.</summary>
public sealed class NativeLagerPool
{
    public const int Capacity = 3200, RecordSize = 43, PayloadLength = 16 + Capacity * (RecordSize + 2);
    private readonly byte[] _payload;
    private readonly byte[]? _header;
    private NativeLagerPool(byte[] payload, byte[]? header)
    {
        if (payload.Length != PayloadLength || BinaryPrimitives.ReadInt32LittleEndian(payload) != 1
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(4)) != Capacity
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(8)) != 6
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(12)) != 10)
            throw new InvalidDataException("lager.dat 必須是 version1／3200 槽／6 輔助欄／10 成員的完整布局。");
        _payload = payload.ToArray(); _header = header;
    }
    public static NativeLagerPool ParsePayload(ReadOnlySpan<byte> payload) => new(payload.ToArray(), null);
    public static NativeLagerPool Load(string path)
    {
        byte[] bytes = File.ReadAllBytes(path); bool wrapped = bytes.AsSpan().StartsWith("PFIL"u8);
        if (wrapped && bytes.Length < 64) throw new InvalidDataException("lager.dat PFIL 標頭不完整。");
        return new(GameLZSS.DecompressPfil(bytes), wrapped ? bytes[..64] : null);
    }
    public byte[] ToPayload() => _payload.ToArray();
    internal NativeLagerPool CloneForEditing() => new(_payload, _header?.ToArray());
    public NativeLagerSnapshot? CaptureSnapshot(int slot)
    {
        var camp = Capture(slot);
        return camp is null ? null : new(_payload.AsSpan(16 + slot * RecordSize, RecordSize).ToArray(), camp.BigLager);
    }
    /// <summary>Copies saved small camps after complete preflight; caller supplies independently allocated
    /// object/biglager mappings and reserves dangling references in the full family transaction.</summary>
    public int[]? AddCopies(IReadOnlyList<NativeLagerCopy> copies, IReadOnlySet<int>? excludedSlots = null)
    {
        ArgumentNullException.ThrowIfNull(copies);
        var records = new List<(byte[] Record, short Big)>();
        var members = new HashSet<int>(); var bigTargets = new HashSet<int>();
        foreach (var copy in copies)
        {
            ArgumentNullException.ThrowIfNull(copy);
            ArgumentNullException.ThrowIfNull(copy.Snapshot);
            ArgumentNullException.ThrowIfNull(copy.ObjectMap);
            ArgumentNullException.ThrowIfNull(copy.BigLagerMap);
            byte[] record = copy.Snapshot.RecordBytes();
            int occupancy = BinaryPrimitives.ReadInt16LittleEndian(record.AsSpan(9));
            int capacity = copy.Snapshot.BigLager == -1 ? 10 : NativeBigLagerPool.MembersPerSlot;
            if (occupancy < 0 || occupancy > capacity) throw new InvalidDataException("營地的保存人數無效。");
            for (int member = 0; member < 10; member++)
            {
                int offset = 23 + member * 2;
                int source = BinaryPrimitives.ReadInt16LittleEndian(record.AsSpan(offset));
                if (source == -1) continue;
                if (source is < 0 or >= 14000 || !copy.ObjectMap.TryGetValue(source, out int target)
                    || target is < 0 or >= 14000 || !members.Add(target))
                    throw new InvalidDataException("小型營地成員必須完整映射至互不共用的新物件。");
                BinaryPrimitives.WriteInt16LittleEndian(record.AsSpan(offset), (short)target);
            }
            int big = copy.Snapshot.BigLager;
            if (big != -1)
            {
                if (big is < 0 or >= NativeBigLagerPool.Capacity || !copy.BigLagerMap.TryGetValue(big, out int mapped)
                    || mapped is < 0 or >= NativeBigLagerPool.Capacity || !bigTargets.Add(mapped))
                    throw new InvalidDataException("小型營地的大型營地連結必須有獨立的新索引。");
                big = mapped;
            }
            records.Add((record, (short)big));
        }
        int[] slots = Enumerable.Range(0, Capacity).Where(slot => Capture(slot) is null
            && excludedSlots?.Contains(slot) != true).Take(records.Count).ToArray();
        if (slots.Length != records.Count) return null;
        for (int index = 0; index < slots.Length; index++)
        {
            records[index].Record.CopyTo(_payload, 16 + slots[index] * RecordSize);
            BinaryPrimitives.WriteInt16LittleEndian(_payload.AsSpan(16 + Capacity * RecordSize + slots[index] * 2), records[index].Big);
        }
        return slots;
    }
    public NativeSavedLager? Capture(int slot)
    {
        if (slot is < 0 or >= Capacity) return null;
        int record = 16 + slot * RecordSize; byte active = _payload[record];
        if (active == 0) return null;
        var members = new List<int>();
        for (int index = 0; index < 10; index++)
        {
            short member = BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(record + 23 + index * 2));
            if (member != -1) members.Add(member);
        }
        return new(slot, active, BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(record + 9)), members.AsReadOnly(),
            BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(16 + Capacity * RecordSize + slot * 2)));
    }
    public IReadOnlyList<NativeSavedLager> ActiveLagers()
        => Enumerable.Range(0, Capacity).Select(Capture).OfType<NativeSavedLager>().ToArray();
    /// <summary>Caller must preflight member objects and biglager ownership before releasing related pools.</summary>
    public IReadOnlyList<NativeSavedLager> ReleaseBatch(IReadOnlyList<int> slots)
    {
        ArgumentNullException.ThrowIfNull(slots);
        foreach (int slot in slots) if (slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(slots));
        var unique = slots.Distinct().ToArray();
        var snapshots = unique.Select(Capture).OfType<NativeSavedLager>().ToArray();
        foreach (int slot in unique)
        {
            int record = 16 + slot * RecordSize;
            _payload.AsSpan(record, RecordSize).Clear();
            _payload.AsSpan(record + 23, 20).Fill(255);
            BinaryPrimitives.WriteInt16LittleEndian(_payload.AsSpan(16 + Capacity * RecordSize + slot * 2), -1);
        }
        return snapshots;
    }
    public void Save(string path, FileRollbackScope rollback)
        => Core.Services.SafeFileWriter.WriteAllBytes(path, _header is null ? _payload : GameLZSS.CompressPfil(_payload, _header), rollback);
}

public sealed record NativeSavedBigLager(int Slot, byte Active, IReadOnlyList<int> Members);

/// <summary>Native version1 32-slot biglager table: one byte plus 800 signed16 object indices per record.</summary>
public sealed class NativeBigLagerPool
{
    public const int Capacity = 32, MembersPerSlot = 800, RecordSize = 1601, PayloadLength = 12 + Capacity * RecordSize;
    private readonly byte[] _payload;
    private readonly byte[]? _header;
    private NativeBigLagerPool(byte[] payload, byte[]? header)
    {
        if (payload.Length != PayloadLength || BinaryPrimitives.ReadInt32LittleEndian(payload) != 1
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(4)) != Capacity
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(8)) != MembersPerSlot)
            throw new InvalidDataException("biglager.dat 必須是 version1／32 槽／800 成員的完整布局。");
        _payload = payload.ToArray(); _header = header;
    }
    public static NativeBigLagerPool ParsePayload(ReadOnlySpan<byte> payload) => new(payload.ToArray(), null);
    public static NativeBigLagerPool Load(string path)
    {
        byte[] bytes = File.ReadAllBytes(path); bool wrapped = bytes.AsSpan().StartsWith("PFIL"u8);
        if (wrapped && bytes.Length < 64) throw new InvalidDataException("biglager.dat PFIL 標頭不完整。");
        return new(GameLZSS.DecompressPfil(bytes), wrapped ? bytes[..64] : null);
    }
    public byte[] ToPayload() => _payload.ToArray();
    internal NativeBigLagerPool CloneForEditing() => new(_payload, _header?.ToArray());
    public NativeBigLagerSnapshot? CaptureSnapshot(int slot)
        => Capture(slot) is null ? null : new(_payload.AsSpan(12 + slot * RecordSize, RecordSize).ToArray());
    /// <summary>Copies complete member arrays after whole-batch preflight. Caller validates new objects
    /// and reserves externally referenced inactive slots in the enclosing family transaction.</summary>
    public int[]? AddCopies(IReadOnlyList<NativeBigLagerCopy> copies, IReadOnlySet<int>? excludedSlots = null)
    {
        ArgumentNullException.ThrowIfNull(copies);
        var records = new List<byte[]>();
        var batchMembers = new HashSet<int>();
        foreach (var copy in copies)
        {
            ArgumentNullException.ThrowIfNull(copy);
            ArgumentNullException.ThrowIfNull(copy.Snapshot);
            ArgumentNullException.ThrowIfNull(copy.ObjectMap);
            byte[] record = copy.Snapshot.RecordBytes();
            var localMembers = new HashSet<int>();
            for (int index = 0; index < MembersPerSlot; index++)
            {
                int offset = 1 + index * 2;
                int source = BinaryPrimitives.ReadInt16LittleEndian(record.AsSpan(offset));
                if (source == -1) continue;
                if (source is < 0 or >= 14000 || !copy.ObjectMap.TryGetValue(source, out int target)
                    || target is < 0 or >= 14000 || !localMembers.Add(target) || !batchMembers.Add(target))
                    throw new InvalidDataException("大型營地成員必須完整映射至互不共用的新物件。");
                BinaryPrimitives.WriteInt16LittleEndian(record.AsSpan(offset), (short)target);
            }
            records.Add(record);
        }
        int[] slots = Enumerable.Range(0, Capacity).Where(slot => Capture(slot) is null
            && excludedSlots?.Contains(slot) != true).Take(records.Count).ToArray();
        if (slots.Length != records.Count) return null;
        for (int index = 0; index < slots.Length; index++)
            records[index].CopyTo(_payload, 12 + slots[index] * RecordSize);
        return slots;
    }
    public NativeSavedBigLager? Capture(int slot)
    {
        if (slot is < 0 or >= Capacity) return null;
        int record = 12 + slot * RecordSize; byte active = _payload[record];
        if (active == 0) return null;
        var members = new List<int>();
        for (int index = 0; index < MembersPerSlot; index++)
        {
            short member = BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(record + 1 + index * 2));
            if (member != -1) members.Add(member);
        }
        return new(slot, active, members.AsReadOnly());
    }
    public IReadOnlyList<NativeSavedBigLager> ActiveLagers()
        => Enumerable.Range(0, Capacity).Select(Capture).OfType<NativeSavedBigLager>().ToArray();
    public IReadOnlyList<NativeSavedBigLager> ReleaseBatch(IReadOnlyList<int> slots)
    {
        ArgumentNullException.ThrowIfNull(slots);
        foreach (int slot in slots) if (slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(slots));
        var unique = slots.Distinct().ToArray();
        var snapshots = unique.Select(Capture).OfType<NativeSavedBigLager>().ToArray();
        foreach (int slot in unique)
        {
            int record = 12 + slot * RecordSize; _payload[record] = 0;
            _payload.AsSpan(record + 1, MembersPerSlot * 2).Fill(255);
        }
        return snapshots;
    }
    public void Save(string path, FileRollbackScope rollback)
        => Core.Services.SafeFileWriter.WriteAllBytes(path, _header is null ? _payload : GameLZSS.CompressPfil(_payload, _header), rollback);
}
