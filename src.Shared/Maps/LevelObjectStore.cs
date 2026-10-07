using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

/// <summary>關卡中一個已放置的世界物件（樹、草、灌木、岩石、腳本標記…）。</summary>
public sealed record LevelWorldObject(int Slot, int TypeId, int Team, uint Uid, float X, float Y, float Z, float Rotation, bool Linked);

/// <summary>可複製到其他槽位的一份完整物件資料（取自原版地圖的同類型物件）。</summary>
public sealed class LevelObjectTemplate
{
    internal LevelObjectTemplate(int typeId, byte[] record, uint[] columns, byte[][] objdata, byte[] position0, byte[] position1)
    {
        TypeId = typeId; Record = record; Columns = columns; ObjData = objdata; Position0 = position0; Position1 = position1;
    }
    public int TypeId { get; }
    internal byte[] Record { get; }
    internal uint[] Columns { get; }
    internal byte[][] ObjData { get; }
    internal byte[] Position0 { get; }
    internal byte[] Position1 { get; }
}

/// <summary>
/// 地圖 <c>DATA/objects.dat</c>、<c>objdata.dat</c>、<c>position.dat</c>（遊戲關卡存檔格式，mp_lsave.c）的讀寫。
///
/// 格式（2026-10-06 由 EXE 載入器 0x48bae0／0x48c0c0／0x48c760 與 ENDL_000 資料交叉驗證）：
/// - objects.dat：16 B 標頭（ver, count=14000, 30, 30）＋ count × 79 B 紀錄（active u8、team u16、uid u32、name[30]、idname[30]、
///   pos0 u16、pos1 u16、self u16、self u16、typeId u16、self u16）＋ 18 個平行欄位陣列（寬度見 <see cref="ColumnWidths"/>；欄 1 為自身索引）。
/// - objdata.dat：8 B 標頭（ver, count）＋ 36 段「count × 寬度」的平行陣列，每槽合計 123 B（見 <see cref="ObjDataWidths"/>）。
/// - position.dat：8 B 標頭（ver, count=33000）＋ count × 17 B（valid u8, x, y, z, rot float32）；每個物件佔一對位置。
/// 空槽：active 0、team 0x7FFF、尾端欄位 0xFFFF、欄 1 = 0xFFFF。新增時複製同類型物件的整個槽位，只改 UID、位置與自身索引。
/// </summary>
public sealed class LevelObjectStore
{
    public const int RecordSize = 79;
    public const int PositionSize = 17;
    public static readonly int[] ColumnWidths = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];
    public static readonly int[] ObjDataWidths = [9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 2, 2];
    private const int SelfColumn = 1, LinkColumn = 10, LinkSegment = 2;

    private readonly byte[] _objects, _objdata, _positions;
    private readonly byte[]? _objectsHeader, _objdataHeader, _positionsHeader;
    private readonly int _count, _positionCount;
    private readonly int _columnsOffset;
    private readonly int[] _columnOffsets, _segmentOffsets;
    private readonly byte[] _emptyRecord, _emptyPosition;
    private readonly uint[] _emptyColumns;
    private readonly byte[][] _emptyObjData;

    private LevelObjectStore(byte[] objects, byte[] objdata, byte[] positions, byte[]? objectsHeader, byte[]? objdataHeader, byte[]? positionsHeader)
    {
        _objects = objects; _objdata = objdata; _positions = positions;
        _objectsHeader = objectsHeader; _objdataHeader = objdataHeader; _positionsHeader = positionsHeader;
        if (objects.Length < 16 || BinaryPrimitives.ReadInt32LittleEndian(objects) != 1) throw new InvalidDataException("objects.dat 版本不符。");
        _count = BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(4));
        if (BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(8)) != 30 || BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(12)) != 30)
            throw new InvalidDataException("objects.dat 名稱欄寬不符。");
        _columnsOffset = 16 + _count * RecordSize;
        _columnOffsets = new int[ColumnWidths.Length];
        int offset = _columnsOffset;
        for (int column = 0; column < ColumnWidths.Length; column++) { _columnOffsets[column] = offset; offset += _count * ColumnWidths[column]; }
        if (offset != objects.Length) throw new InvalidDataException($"objects.dat 長度不符（預期 {offset}，實際 {objects.Length}）。");
        if (objdata.Length < 8 || BinaryPrimitives.ReadInt32LittleEndian(objdata) != 1) throw new InvalidDataException("objdata.dat 版本不符。");
        if (BinaryPrimitives.ReadInt32LittleEndian(objdata.AsSpan(4)) != _count) throw new InvalidDataException("objdata.dat 物件數與 objects.dat 不符。");
        _segmentOffsets = new int[ObjDataWidths.Length];
        offset = 8;
        for (int segment = 0; segment < ObjDataWidths.Length; segment++) { _segmentOffsets[segment] = offset; offset += _count * ObjDataWidths[segment]; }
        if (offset != objdata.Length) throw new InvalidDataException($"objdata.dat 長度不符（預期 {offset}，實際 {objdata.Length}）。");
        if (positions.Length < 8 || BinaryPrimitives.ReadInt32LittleEndian(positions) != 1) throw new InvalidDataException("position.dat 版本不符。");
        _positionCount = BinaryPrimitives.ReadInt32LittleEndian(positions.AsSpan(4));
        if (8 + _positionCount * PositionSize != positions.Length) throw new InvalidDataException("position.dat 長度不符。");

        int empty = Enumerable.Range(0, _count).FirstOrDefault(slot => !IsActive(slot), -1);
        if (empty < 0) throw new InvalidDataException("objects.dat 沒有空槽可作為空白範本。");
        _emptyRecord = objects.AsSpan(RecordOffset(empty), RecordSize).ToArray();
        _emptyColumns = Enumerable.Range(0, ColumnWidths.Length).Select(column => ReadColumn(column, empty)).ToArray();
        _emptyObjData = Enumerable.Range(0, ObjDataWidths.Length).Select(segment => objdata.AsSpan(SegmentOffset(segment, empty), ObjDataWidths[segment]).ToArray()).ToArray();
        int freePosition = Enumerable.Range(0, _positionCount).FirstOrDefault(index => positions[8 + index * PositionSize] == 0, -1);
        _emptyPosition = freePosition >= 0 ? positions.AsSpan(8 + freePosition * PositionSize, PositionSize).ToArray() : new byte[PositionSize];
    }

    public int Capacity => _count;

    public static LevelObjectStore Load(string mapDirectory)
    {
        (byte[] objects, byte[]? h1) = Read(Path.Combine(mapDirectory, "DATA", "objects.dat"));
        (byte[] objdata, byte[]? h2) = Read(Path.Combine(mapDirectory, "DATA", "objdata.dat"));
        (byte[] positions, byte[]? h3) = Read(Path.Combine(mapDirectory, "DATA", "position.dat"));
        return new LevelObjectStore(objects, objdata, positions, h1, h2, h3);
    }

    private static (byte[] Data, byte[]? Header) Read(string path)
    {
        byte[] raw = File.ReadAllBytes(path);
        bool pfil = raw.Length >= 64 && raw[0] == 'P' && raw[1] == 'F' && raw[2] == 'I' && raw[3] == 'L';
        return pfil ? (GameLZSS.DecompressPfil(raw), raw.AsSpan(0, 64).ToArray()) : (raw, null);
    }

    public IReadOnlyList<LevelWorldObject> Objects()
    {
        var list = new List<LevelWorldObject>();
        for (int slot = 0; slot < _count; slot++)
        {
            if (!IsActive(slot)) continue;
            int record = RecordOffset(slot);
            int position = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67));
            float x = 0, y = 0, z = 0, rotation = 0;
            if (position < _positionCount)
            {
                int p = 8 + position * PositionSize + 1;
                x = BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(p)); y = BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(p + 4));
                z = BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(p + 8)); rotation = BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(p + 12));
            }
            list.Add(new LevelWorldObject(slot, TypeId(slot), BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 1)),
                BinaryPrimitives.ReadUInt32LittleEndian(_objects.AsSpan(record + 3)), x, y, z, rotation, IsLinked(slot)));
        }
        return list;
    }

    /// <summary>從所有原版地圖（略過自製地圖）收集可複製範本，每個類型取第一個；<paramref name="include"/> 依類型編號過濾。</summary>
    public static IReadOnlyDictionary<int, LevelObjectTemplate> LoadOfficialTemplates(string gamePath, Func<int, bool> include)
    {
        ArgumentNullException.ThrowIfNull(include);
        var templates = new Dictionary<int, LevelObjectTemplate>();
        string maps = Path.Combine(gamePath, "MAPS");
        if (!Directory.Exists(maps)) return templates;
        foreach (string map in Directory.GetDirectories(maps).OrderBy(path => path, StringComparer.OrdinalIgnoreCase))
        {
            if (CustomMapManifest.IsCustomMapDirectory(map) || !File.Exists(Path.Combine(map, "DATA", "objects.dat"))) continue;
            try
            {
                foreach (LevelObjectTemplate template in Load(map).Templates())
                    if (include(template.TypeId)) templates.TryAdd(template.TypeId, template);
            }
            catch (Exception ex) when (ex is InvalidDataException or IOException) { }
        }
        return templates;
    }

    /// <summary>可安全複製的範本：未與其他物件連結、兩個位置索引都有效的同類型物件。</summary>
    public IEnumerable<LevelObjectTemplate> Templates()
    {
        var seen = new HashSet<int>();
        for (int slot = 0; slot < _count; slot++)
        {
            if (!IsActive(slot) || IsLinked(slot) || !seen.Add(TypeId(slot))) continue;
            int record = RecordOffset(slot);
            int p0 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67)), p1 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69));
            if (p0 >= _positionCount || p1 >= _positionCount) { seen.Remove(TypeId(slot)); continue; }
            yield return new LevelObjectTemplate(TypeId(slot), _objects.AsSpan(record, RecordSize).ToArray(),
                Enumerable.Range(0, ColumnWidths.Length).Select(column => ReadColumn(column, slot)).ToArray(),
                Enumerable.Range(0, ObjDataWidths.Length).Select(segment => _objdata.AsSpan(SegmentOffset(segment, slot), ObjDataWidths[segment]).ToArray()).ToArray(),
                _positions.AsSpan(8 + p0 * PositionSize, PositionSize).ToArray(), _positions.AsSpan(8 + p1 * PositionSize, PositionSize).ToArray());
        }
    }

    /// <summary>Capture an owned, unlinked object's template only while its runtime UID still matches.</summary>
    public LevelObjectTemplate? OwnedTemplate(int slot, uint uid)
    {
        if (uid == 0 || UidAt(slot) != uid || IsLinked(slot)) return null;
        int record = RecordOffset(slot);
        int p0 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67)), p1 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69));
        if (p0 >= _positionCount || p1 >= _positionCount) return null;
        return new LevelObjectTemplate(TypeId(slot), _objects.AsSpan(record, RecordSize).ToArray(),
            Enumerable.Range(0, ColumnWidths.Length).Select(column => ReadColumn(column, slot)).ToArray(),
            Enumerable.Range(0, ObjDataWidths.Length).Select(segment => _objdata.AsSpan(SegmentOffset(segment, slot), ObjDataWidths[segment]).ToArray()).ToArray(),
            _positions.AsSpan(8 + p0 * PositionSize, PositionSize).ToArray(), _positions.AsSpan(8 + p1 * PositionSize, PositionSize).ToArray());
    }

    /// <summary>
    /// 以範本在世界座標新增一個物件；回傳使用的槽位，沒有空間時回傳 -1。
    /// <paramref name="team"/> 改寫記錄的隊伍欄（+1，u16；8＝中立）。官方地圖同型建築在不同隊伍間只有此欄與 uid／位置／自身索引不同。
    /// </summary>
    public int Add(LevelObjectTemplate template, float x, float y, float z, float rotation, int? team = null)
    {
        if (team is < 0 or > 8) throw new ArgumentOutOfRangeException(nameof(team), "隊伍必須介於 0 與 8。");
        ArgumentNullException.ThrowIfNull(template);
        if (!float.IsFinite(x) || !float.IsFinite(y) || !float.IsFinite(z) || !float.IsFinite(rotation)) throw new ArgumentOutOfRangeException(nameof(x), "物件座標必須是有限數值。");
        int slot = Enumerable.Range(0, _count).FirstOrDefault(index => !IsActive(index), -1);
        int position = FreePositionPair();
        if (slot < 0 || position < 0) return -1;
        uint uid = Enumerable.Range(0, _count).Where(IsActive).Select(index => BinaryPrimitives.ReadUInt32LittleEndian(_objects.AsSpan(RecordOffset(index) + 3))).DefaultIfEmpty(0u).Max() + 1;

        int record = RecordOffset(slot);
        template.Record.CopyTo(_objects, record);
        BinaryPrimitives.WriteUInt32LittleEndian(_objects.AsSpan(record + 3), uid);
        if (team is { } owner) BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 1), checked((ushort)owner));
        ushort self = checked((ushort)slot);
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 67), checked((ushort)position));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 69), checked((ushort)(position + 1)));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 71), self);
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 73), self);
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 77), self);
        for (int column = 0; column < ColumnWidths.Length; column++) WriteColumn(column, slot, column == SelfColumn ? self : template.Columns[column]);
        for (int segment = 0; segment < ObjDataWidths.Length; segment++) template.ObjData[segment].CopyTo(_objdata, SegmentOffset(segment, slot));
        WritePosition(position, template.Position0, x, y, z, rotation);
        WritePosition(position + 1, template.Position1, x, y, z, rotation);
        return slot;
    }

    /// <summary>刪除一個未連結的物件：整個槽位改回空槽資料並釋放其兩個位置。</summary>
    public bool Remove(int slot)
    {
        if (slot < 0 || slot >= _count || !IsActive(slot) || IsLinked(slot)) return false;
        int record = RecordOffset(slot);
        foreach (int offset in new[] { 67, 69 })
        {
            int position = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + offset));
            if (position < _positionCount) _emptyPosition.CopyTo(_positions, 8 + position * PositionSize);
        }
        _emptyRecord.CopyTo(_objects, record);
        for (int column = 0; column < ColumnWidths.Length; column++) WriteColumn(column, slot, _emptyColumns[column]);
        for (int segment = 0; segment < ObjDataWidths.Length; segment++) _emptyObjData[segment].CopyTo(_objdata, SegmentOffset(segment, slot));
        return true;
    }

    /// <summary>槽位目前物件的 uid；空槽或超出範圍時為 null。</summary>
    public uint? UidAt(int slot)
        => slot >= 0 && slot < _count && IsActive(slot) ? BinaryPrimitives.ReadUInt32LittleEndian(_objects.AsSpan(RecordOffset(slot) + 3)) : null;

    /// <summary>只在槽位仍是指定 uid 的物件時移除（避免誤刪後來被其他編輯佔用的槽位）。</summary>
    public bool RemoveIfUid(int slot, uint uid) => UidAt(slot) == uid && Remove(slot);

    /// <summary>在交易內寫回三個檔案（沿用原 PFIL 標頭）。</summary>
    public void Save(string mapDirectory, FileRollbackScope rollback)
    {
        Write(Path.Combine(mapDirectory, "DATA", "objects.dat"), _objects, _objectsHeader, rollback);
        Write(Path.Combine(mapDirectory, "DATA", "objdata.dat"), _objdata, _objdataHeader, rollback);
        Write(Path.Combine(mapDirectory, "DATA", "position.dat"), _positions, _positionsHeader, rollback);
    }

    private static void Write(string path, byte[] data, byte[]? header, FileRollbackScope rollback)
        => Core.Services.SafeFileWriter.WriteAllBytes(path, header is null ? data : GameLZSS.CompressPfil(data, header), rollback);

    private bool IsActive(int slot) => _objects[RecordOffset(slot)] != 0;
    private int TypeId(int slot) => BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 75));
    private bool IsLinked(int slot)
        => ReadColumn(LinkColumn, slot) != 0xFFFF || _objdata.AsSpan(SegmentOffset(LinkSegment, slot), ObjDataWidths[LinkSegment]).IndexOfAnyExcept((byte)0) >= 0;
    private int RecordOffset(int slot) => 16 + slot * RecordSize;
    private int SegmentOffset(int segment, int slot) => _segmentOffsets[segment] + slot * ObjDataWidths[segment];

    private uint ReadColumn(int column, int slot)
    {
        int offset = _columnOffsets[column] + slot * ColumnWidths[column];
        return ColumnWidths[column] == 2 ? BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(offset)) : BinaryPrimitives.ReadUInt32LittleEndian(_objects.AsSpan(offset));
    }

    private void WriteColumn(int column, int slot, uint value)
    {
        int offset = _columnOffsets[column] + slot * ColumnWidths[column];
        if (ColumnWidths[column] == 2) BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(offset), checked((ushort)value));
        else BinaryPrimitives.WriteUInt32LittleEndian(_objects.AsSpan(offset), value);
    }

    private int FreePositionPair()
    {
        for (int index = 0; index + 1 < _positionCount; index += 2)
            if (_positions[8 + index * PositionSize] == 0 && _positions[8 + (index + 1) * PositionSize] == 0) return index;
        return -1;
    }

    private void WritePosition(int index, byte[] template, float x, float y, float z, float rotation)
    {
        int offset = 8 + index * PositionSize;
        template.CopyTo(_positions, offset);
        _positions[offset] = 1;
        BinaryPrimitives.WriteSingleLittleEndian(_positions.AsSpan(offset + 1), x);
        BinaryPrimitives.WriteSingleLittleEndian(_positions.AsSpan(offset + 5), y);
        BinaryPrimitives.WriteSingleLittleEndian(_positions.AsSpan(offset + 9), z);
        BinaryPrimitives.WriteSingleLittleEndian(_positions.AsSpan(offset + 13), rotation);
    }
}

/// <summary>objdef.dau 的「編號 → 名稱」對照（第 0 欄 idx、第 52 欄名稱）。</summary>
public static class ObjDefNames
{
    public static IReadOnlyDictionary<int, string> Load(string gamePath)
    {
        string path = Path.Combine(gamePath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
        var names = new Dictionary<int, string>();
        if (!File.Exists(path)) return names;
        string text = MapTextEncoding.Game.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        foreach (string line in text.Split('\n'))
        {
            if (line.Length < 100 || line.TrimStart().StartsWith(';') || line.StartsWith('[')) continue;
            string[] columns = line.Split(',');
            if (columns.Length > 52 && int.TryParse(columns[0].Trim(), out int index)) names[index] = columns[52].Trim();
        }
        return names;
    }

    /// <summary>可由編輯器增刪的地景物件（Lan…）；腳本標記、特效、建築與單位一律不動。</summary>
    public static bool IsLandscape(string? name) => name is not null && name.StartsWith("Lan", StringComparison.OrdinalIgnoreCase);
}
