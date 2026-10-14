using System.Buffers.Binary;
using System.Numerics;

namespace AgainstRomeModifier.Maps;

public sealed record NativeLightCreation(Vector3 Position, Vector3 Color, float Radius,
    short FlickerType, float FlickerParameter, uint SpecialFx, short Definition,
    int OwnerObject = -1, int ParentObject = -1);

public sealed record NativeSavedLight(int Slot, uint Active, Vector3 Position,
    short OwnerObject, short ParentObject, short Definition);
public sealed record NativeLightPosition(int Slot, Vector3 Position);

/// <summary>Detached saved fields for editor-owned copies; not native definition initialization.</summary>
public sealed class NativeLightSnapshot
{
    internal NativeLightSnapshot(byte[] fields) => Fields = fields;
    internal byte[] Fields { get; }
    public Vector3 Position => new(BinaryPrimitives.ReadSingleLittleEndian(Fields.AsSpan(4)),
        BinaryPrimitives.ReadSingleLittleEndian(Fields.AsSpan(8)), BinaryPrimitives.ReadSingleLittleEndian(Fields.AsSpan(12)));
    public short OwnerObject => BinaryPrimitives.ReadInt16LittleEndian(Fields.AsSpan(50));
    public short ParentObject => BinaryPrimitives.ReadInt16LittleEndian(Fields.AsSpan(52));
}

/// <summary>Verified version1 parallel-column light.dat pool. All mutations are in memory.</summary>
public sealed class NativeLightPool
{
    public const int Capacity = 1024;
    public const int PayloadLength = 8 + Capacity * 56;
    private static readonly int[] Widths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
    private readonly byte[] _payload;
    private readonly byte[]? _header;
    private readonly int[] _offsets;

    private NativeLightPool(byte[] payload, byte[]? header)
    {
        if (payload.Length != PayloadLength || BinaryPrimitives.ReadInt32LittleEndian(payload) != 1
            || BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(4)) != Capacity)
            throw new InvalidDataException("light.dat 必須是完整的 version1／1024 槽平行欄位布局。");
        _payload = payload.ToArray(); _header = header;
        _offsets = new int[Widths.Length];
        int offset = 8;
        for (int i = 0; i < Widths.Length; i++) { _offsets[i] = offset; offset += Capacity * Widths[i]; }
    }

    public static NativeLightPool ParsePayload(ReadOnlySpan<byte> payload) => new(payload.ToArray(), null);

    public static NativeLightPool Load(string path)
    {
        byte[] source = File.ReadAllBytes(path);
        bool wrapped = source.AsSpan().StartsWith("PFIL"u8);
        if (wrapped && source.Length < 64) throw new InvalidDataException("light.dat PFIL 標頭不完整。");
        return new(GameLZSS.DecompressPfil(source), wrapped ? source.AsSpan(0, 64).ToArray() : null);
    }

    public byte[] ToPayload() => _payload.ToArray();
    internal NativeLightPool CloneForEditing() => new(_payload, _header?.ToArray());
    internal void InstallFamilyCopy(int slot, NativeLightSnapshot snapshot, int owner, int parent)
    {
        if (slot is < 0 or >= Capacity || ReadUInt(0, slot) != 0 || owner is < -1 or >= 14000
            || parent is < -1 or >= 14000 || !float.IsFinite(snapshot.Position.X)
            || !float.IsFinite(snapshot.Position.Y) || !float.IsFinite(snapshot.Position.Z))
            throw new InvalidDataException("群組光源副本的槽位／回連／座標無效。");
        int offset = 0;
        for (int column = 0; column < Widths.Length; column++)
        {
            snapshot.Fields.AsSpan(offset, Widths[column]).CopyTo(_payload.AsSpan(Offset(column, slot)));
            offset += Widths[column];
        }
        WriteShort(13, slot, (short)owner); WriteShort(14, slot, (short)parent);
    }
    internal void CopyPayloadFrom(NativeLightPool source) => source._payload.CopyTo(_payload, 0);

    /// <summary>Updates only XYZ after validating every active target and rejecting conflicting shared targets.</summary>
    public void SetPositionsBatch(IReadOnlyList<NativeLightPosition> positions)
    {
        ArgumentNullException.ThrowIfNull(positions);
        var targets = new Dictionary<int, Vector3>();
        foreach (var item in positions)
        {
            ArgumentNullException.ThrowIfNull(item);
            if (item.Slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(positions));
            if (ReadUInt(0, item.Slot) == 0) throw new InvalidDataException("無法移動未啟用的光源。");
            if (!float.IsFinite(item.Position.X) || !float.IsFinite(item.Position.Y) || !float.IsFinite(item.Position.Z))
                throw new InvalidDataException("光源座標必須是有限 float32 值。");
            if (targets.TryGetValue(item.Slot, out var previous) && previous != item.Position)
                throw new InvalidDataException("共用光源收到不同的目標座標。");
            targets[item.Slot] = item.Position;
        }
        foreach (var pair in targets)
        {
            WriteFloat(1, pair.Key, pair.Value.X); WriteFloat(2, pair.Key, pair.Value.Y); WriteFloat(3, pair.Key, pair.Value.Z);
        }
    }

    public NativeLightSnapshot? Capture(int slot)
    {
        if (slot is < 0 or >= Capacity || ReadUInt(0, slot) == 0) return null;
        byte[] fields = new byte[56]; int offset = 0;
        for (int column = 0; column < Widths.Length; column++)
        {
            _payload.AsSpan(Offset(column, slot), Widths[column]).CopyTo(fields.AsSpan(offset));
            offset += Widths[column];
        }
        return new(fields);
    }

    public int AddCopy(NativeLightSnapshot snapshot, Vector3 position, int ownerObject,
        IReadOnlySet<int>? excludedSlots = null)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        if (!float.IsFinite(position.X) || !float.IsFinite(position.Y) || !float.IsFinite(position.Z)
            || ownerObject is < 0 or >= 14000 || snapshot.ParentObject != -1)
            throw new InvalidDataException("直接光源副本的座標／owner／parent 無效。");
        int slot = Enumerable.Range(0, Capacity).FirstOrDefault(s => ReadUInt(0, s) == 0 && excludedSlots?.Contains(s) != true, -1);
        if (slot < 0) return -1;
        int offset = 0;
        for (int column = 0; column < Widths.Length; column++)
        {
            snapshot.Fields.AsSpan(offset, Widths[column]).CopyTo(_payload.AsSpan(Offset(column, slot)));
            offset += Widths[column];
        }
        WriteUInt(0, slot, 1); WriteShort(13, slot, checked((short)ownerObject)); WriteShort(14, slot, -1);
        WriteFloat(1, slot, position.X); WriteFloat(2, slot, position.Y); WriteFloat(3, slot, position.Z);
        return slot;
    }

    public void Save(string path, FileRollbackScope rollback)
        => Core.Services.SafeFileWriter.WriteAllBytes(path,
            _header is null ? _payload : GameLZSS.CompressPfil(_payload, _header), rollback);

    public IReadOnlyList<NativeSavedLight> ActiveLights()
    {
        var result = new List<NativeSavedLight>();
        for (int slot = 0; slot < Capacity; slot++)
        {
            uint active = ReadUInt(0, slot);
            if (active != 0) result.Add(new(slot, active, new(ReadFloat(1, slot), ReadFloat(2, slot), ReadFloat(3, slot)),
                ReadShort(13, slot), ReadShort(14, slot), ReadShort(15, slot)));
        }
        return result;
    }

    /// <summary>Prevalidates and reserves the complete batch. No capacity returns null without mutation.</summary>
    public int[]? AddBatch(IReadOnlyList<NativeLightCreation> lights, IReadOnlySet<int>? excludedSlots = null)
    {
        ArgumentNullException.ThrowIfNull(lights);
        foreach (var light in lights) Validate(light);
        int[] slots = Enumerable.Range(0, Capacity).Where(slot => ReadUInt(0, slot) == 0 && excludedSlots?.Contains(slot) != true).Take(lights.Count).ToArray();
        if (slots.Length != lights.Count) return null;
        for (int i = 0; i < slots.Length; i++)
        {
            int slot = slots[i]; var light = lights[i];
            Reset(slot);
            WriteUInt(0, slot, 1);
            WriteFloat(1, slot, light.Position.X); WriteFloat(2, slot, light.Position.Y); WriteFloat(3, slot, light.Position.Z);
            WriteFloat(4, slot, light.Color.X); WriteFloat(5, slot, light.Color.Y); WriteFloat(6, slot, light.Color.Z);
            // 49f500 initializes both radius fields from the same parameter.
            WriteFloat(7, slot, light.Radius); WriteFloat(8, slot, light.Radius);
            WriteShort(9, slot, light.FlickerType); WriteFloat(10, slot, light.FlickerParameter);
            WriteUInt(12, slot, light.SpecialFx); WriteShort(15, slot, light.Definition);
            WriteShort(13, slot, checked((short)light.OwnerObject)); WriteShort(14, slot, checked((short)light.ParentObject));
        }
        return slots;
    }

    /// <summary>
    /// Releases a complete light batch and returns direct object owners whose column12 backlinks need clearing.
    /// The caller must perform those object writes in the same family transaction.
    /// </summary>
    public IReadOnlyList<int> ReleaseBatch(IReadOnlyList<int> slots)
    {
        ArgumentNullException.ThrowIfNull(slots);
        foreach (int slot in slots)
            if (slot is < 0 or >= Capacity) throw new ArgumentOutOfRangeException(nameof(slots));
        int[] distinct = slots.Distinct().ToArray();
        int[] owners = distinct.Select(slot => (int)ReadShort(13, slot)).Where(owner => owner >= 0).Distinct().ToArray();
        foreach (int slot in distinct) Reset(slot);
        return owners;
    }

    private static void Validate(NativeLightCreation light)
    {
        ArgumentNullException.ThrowIfNull(light);
        if (!float.IsFinite(light.Position.X) || !float.IsFinite(light.Position.Y) || !float.IsFinite(light.Position.Z)
            || !float.IsFinite(light.Color.X) || !float.IsFinite(light.Color.Y) || !float.IsFinite(light.Color.Z)
            || !float.IsFinite(light.Radius) || !float.IsFinite(light.FlickerParameter))
            throw new InvalidDataException("光源位置與參數必須是有限 float32 值。");
        if (light.Definition is < 0 or >= 256 || light.OwnerObject is < -1 or >= 14000 || light.ParentObject is < -1 or >= 14000)
            throw new InvalidDataException("光源定義或物件關聯超出原生索引範圍。");
    }

    private void Reset(int slot)
    {
        for (int i = 0; i < Widths.Length; i++) _payload.AsSpan(Offset(i, slot), Widths[i]).Clear();
        for (int i = 13; i < 16; i++) WriteShort(i, slot, -1);
    }

    private int Offset(int column, int slot) => _offsets[column] + slot * Widths[column];
    private uint ReadUInt(int column, int slot) => BinaryPrimitives.ReadUInt32LittleEndian(_payload.AsSpan(Offset(column, slot)));
    private short ReadShort(int column, int slot) => BinaryPrimitives.ReadInt16LittleEndian(_payload.AsSpan(Offset(column, slot)));
    private float ReadFloat(int column, int slot) => BinaryPrimitives.ReadSingleLittleEndian(_payload.AsSpan(Offset(column, slot)));
    private void WriteUInt(int column, int slot, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(_payload.AsSpan(Offset(column, slot)), value);
    private void WriteShort(int column, int slot, short value) => BinaryPrimitives.WriteInt16LittleEndian(_payload.AsSpan(Offset(column, slot)), value);
    private void WriteFloat(int column, int slot, float value) => BinaryPrimitives.WriteSingleLittleEndian(_payload.AsSpan(Offset(column, slot)), value);
}
