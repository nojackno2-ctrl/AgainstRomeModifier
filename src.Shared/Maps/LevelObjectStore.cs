using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

/// <summary>關卡中一個已放置的世界物件（樹、草、灌木、岩石、腳本標記…）。</summary>
public sealed record LevelWorldObject(int Slot, int TypeId, int Team, uint Uid, float X, float Y, float Z, float Rotation, bool Linked);

/// <summary>可複製到其他槽位的一份完整物件資料（取自原版地圖的同類型物件）。</summary>
public sealed class LevelObjectTemplate
{
    internal LevelObjectTemplate(int typeId, byte[] record, uint[] columns, byte[][] objdata, byte[] position0, byte[] position1)
        : this(typeId, record, columns, objdata, position0, position1, null, 0, 0, null, 0, null) { }

    public LevelObjectTemplate(
        int typeId,
        byte[] record,
        uint[] columns,
        byte[][] objdata,
        byte[] position0,
        byte[] position1,
        byte[]? animRecord = null,
        ushort animTail0 = 0,
        ushort animTail1 = 0,
        byte[]? gfxtypeRecord = null,
        ushort gfxtypeTail = 0,
        byte[]? actionRecord = null,
        byte[]? position2 = null)
    {
        TypeId = typeId;
        Record = record;
        Columns = columns;
        ObjData = objdata;
        Position0 = position0;
        Position1 = position1;
        Position2 = position2;
        AnimRecord = animRecord;
        AnimTail0 = animTail0;
        AnimTail1 = animTail1;
        GfxtypeRecord = gfxtypeRecord;
        GfxtypeTail = gfxtypeTail;
        ActionRecord = actionRecord;
    }
    internal bool InitializeDefinitionVitals { get; set; }
    internal NativeLightSnapshot? DirectLight { get; set; }
    public int TypeId { get; }
    internal byte[] Record { get; }
    internal uint[] Columns { get; }
    internal byte[][] ObjData { get; }
    internal byte[] Position0 { get; }
    internal byte[] Position1 { get; }
    internal byte[]? Position2 { get; }
    internal byte[]? AnimRecord { get; }
    internal ushort AnimTail0 { get; }
    internal ushort AnimTail1 { get; }
    internal byte[]? GfxtypeRecord { get; }
    internal ushort GfxtypeTail { get; }
    internal byte[]? ActionRecord { get; }
}

/// <summary>
/// 地圖 <c>DATA/objects.dat</c>、<c>objdata.dat</c>、<c>position.dat</c> 以及跨池關聯
/// （<c>anim.dat</c>、<c>gfxtype.dat</c>、<c>action.dat</c>、<c>engine.dat</c>）的讀寫與交易同步。
/// </summary>
public sealed partial class LevelObjectStore
{
    public const int RecordSize = 79;
    /// <summary>Detached decoded gfxtype image; independent of mutable save candidates.</summary>
    public byte[]? CopyGfxTypePayload() => _gfxtype?.ToArray();
    public const int PositionSize = 17;
    public static readonly int[] ColumnWidths = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];
    public static readonly int[] ObjDataWidths = [9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 2, 2];
    private const int ObjDataColumn = 1, ThirdPositionColumn = 10;
    private const int ParentObjectColumn = 13;
    private const int DirectLightColumn = 12;
    private readonly HashSet<int> _referencedParents = [];
    private static readonly int[] ProtectedReferenceColumns = [2, 3, 4, 5, 8, 11];

    private readonly byte[] _objects, _objdata, _positions;
    private readonly byte[]? _anim, _gfxtype, _action, _engine;
    private readonly byte[]? _objectsHeader, _objdataHeader, _positionsHeader;
    private readonly byte[]? _animHeader, _gfxtypeHeader, _actionHeader, _engineHeader;
    private readonly int _count, _positionCount;
    private readonly int _columnsOffset;
    private readonly int[] _columnOffsets, _segmentOffsets;
    private readonly byte[] _emptyRecord, _emptyPosition;
    private readonly uint[] _emptyColumns;
    private readonly byte[][] _emptyObjData;
    private readonly byte[]? _emptyAnimRecord;
    private readonly ushort _emptyAnimTail0, _emptyAnimTail1;
    private readonly byte[]? _emptyGfxtypeRecord;
    private readonly ushort _emptyGfxtypeTail;
    private readonly byte[]? _emptyActionRecord;
    private uint _nextUid;
    private readonly IReadOnlyList<string> _unavailablePools;
    private readonly IReadOnlyDictionary<int, NativeObjectDefinition>? _definitions;
    private readonly IReadOnlyDictionary<int, NativeLightTypeDefinition>? _lightDefinitions;
    private readonly NativeLightPool? _lights;
    private readonly NativeParticlePool? _particles;
    private readonly NativeLagerPool? _lagers;
    private readonly NativeBigLagerPool? _bigLagers;
    private readonly HashSet<int> _garrisonMembers = [];
    private readonly HashSet<int> _lightParents = [];
    private readonly Dictionary<int, int> _lightOwners = [];

    private LevelObjectStore(
        byte[] objects,
        byte[] objdata,
        byte[] positions,
        byte[]? objectsHeader,
        byte[]? objdataHeader,
        byte[]? positionsHeader,
        byte[]? anim = null,
        byte[]? animHeader = null,
        byte[]? gfxtype = null,
        byte[]? gfxtypeHeader = null,
        byte[]? action = null,
        byte[]? actionHeader = null,
        byte[]? engine = null,
        byte[]? engineHeader = null,
        IReadOnlyList<string>? unavailablePools = null,
        IReadOnlyDictionary<int, NativeObjectDefinition>? definitions = null,
        NativeLightPool? lights = null,
        IReadOnlyDictionary<int, NativeLightTypeDefinition>? lightDefinitions = null,
        NativeParticlePool? particles = null,
        NativeLagerPool? lagers = null,
        NativeBigLagerPool? bigLagers = null)
    {
        _objects = objects; _objdata = objdata; _positions = positions;
        _objectsHeader = objectsHeader; _objdataHeader = objdataHeader; _positionsHeader = positionsHeader;
        _anim = anim; _animHeader = animHeader;
        _gfxtype = gfxtype; _gfxtypeHeader = gfxtypeHeader;
        _action = action; _actionHeader = actionHeader;
        _engine = engine; _engineHeader = engineHeader;
        _unavailablePools = unavailablePools ?? [];
        _definitions = definitions;
        _lights = lights; _lightDefinitions = lightDefinitions;
        _particles = particles;
        _lagers = lagers; _bigLagers = bigLagers;
        RebuildGarrisonMembers();
        if (lights is not null)
            foreach (var light in lights.ActiveLights())
            {
                if (light.ParentObject >= 0) _lightParents.Add(light.ParentObject);
                if (light.OwnerObject >= 0)
                    _lightOwners[light.OwnerObject] = _lightOwners.ContainsKey(light.OwnerObject) ? -1 : light.Slot;
            }

        if (objects.Length < 16 || BinaryPrimitives.ReadInt32LittleEndian(objects) != 1) throw new InvalidDataException("objects.dat 版本不符。");
        _count = BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(4));
        if (BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(8)) != 30 || BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(12)) != 30)
            throw new InvalidDataException("objects.dat 名稱欄寬不符。");
        _columnsOffset = 16 + _count * RecordSize;
        _columnOffsets = new int[ColumnWidths.Length];
        int offset = _columnsOffset;
        for (int column = 0; column < ColumnWidths.Length; column++) { _columnOffsets[column] = offset; offset += _count * ColumnWidths[column]; }
        if (offset != objects.Length) throw new InvalidDataException($"objects.dat 長度不符（預期 {offset}，實際 {objects.Length}）。");
        // Reader maps column13 to runtime +30; native post-create stores the parent here.
        for (int slot = 0; slot < _count; slot++)
            if (IsActive(slot) && ReadColumn(ParentObjectColumn, slot) is < 0xFFFF and var parent)
                _referencedParents.Add((int)parent);
        if (objdata.Length < 8 || BinaryPrimitives.ReadInt32LittleEndian(objdata) != 1) throw new InvalidDataException("objdata.dat 版本不符。");
        if (BinaryPrimitives.ReadInt32LittleEndian(objdata.AsSpan(4)) != _count) throw new InvalidDataException("objdata.dat 物件數與 objects.dat 不符。");
        _segmentOffsets = new int[ObjDataWidths.Length];
        offset = 8;
        for (int segment = 0; segment < ObjDataWidths.Length; segment++) { _segmentOffsets[segment] = offset; offset += _count * ObjDataWidths[segment]; }
        if (offset != objdata.Length) throw new InvalidDataException($"objdata.dat 長度不符（預期 {offset}，實際 {objdata.Length}）。");
        if (positions.Length < 8 || BinaryPrimitives.ReadInt32LittleEndian(positions) != 1) throw new InvalidDataException("position.dat 版本不符。");
        _positionCount = BinaryPrimitives.ReadInt32LittleEndian(positions.AsSpan(4));
        if (8 + _positionCount * PositionSize != positions.Length) throw new InvalidDataException("position.dat 長度不符。");

        uint activeMaximum = Enumerable.Range(0, _count).Where(IsActive)
            .Select(slot => BinaryPrimitives.ReadUInt32LittleEndian(_objects.AsSpan(RecordOffset(slot) + 3)) & 0x00FFFFFF)
            .DefaultIfEmpty(0u).Max();
        // The native team-0 create-then-set-owner path uses the persisted next counter.
        // Preserve a stale counter's history, but advance past currently occupied low IDs.
        _nextUid = Math.Max(activeMaximum + 1,
            _engine is { Length: >= 38 } ? BinaryPrimitives.ReadUInt32LittleEndian(_engine.AsSpan(34)) : 1u);

        int empty = Enumerable.Range(0, _count).FirstOrDefault(slot => !IsActive(slot), -1);
        if (empty >= 0)
        {
            _emptyRecord = objects.AsSpan(RecordOffset(empty), RecordSize).ToArray();
            _emptyColumns = Enumerable.Range(0, ColumnWidths.Length).Select(column => ReadColumn(column, empty)).ToArray();
        }
        else
        {
            _emptyRecord = CreateInactiveRecordTemplate();
            _emptyColumns = CreateInactiveColumnsTemplate();
        }
        int emptyData = Enumerable.Range(0, _count).FirstOrDefault(index => objdata[SegmentOffset(0, index)] == 0, -1);
        _emptyObjData = Enumerable.Range(0, ObjDataWidths.Length).Select(segment => emptyData >= 0
            ? objdata.AsSpan(SegmentOffset(segment, emptyData), ObjDataWidths[segment]).ToArray()
            : new byte[ObjDataWidths[segment]]).ToArray();
        int freePosition = Enumerable.Range(0, _positionCount).FirstOrDefault(index => positions[8 + index * PositionSize] == 0, -1);
        _emptyPosition = freePosition >= 0 ? positions.AsSpan(8 + freePosition * PositionSize, PositionSize).ToArray() : new byte[PositionSize];

        if (anim is not null)
        {
            if (anim.Length < 8 || BinaryPrimitives.ReadInt32LittleEndian(anim) != 1)
                throw new InvalidDataException("anim.dat 版本不符。");
            int animCount = BinaryPrimitives.ReadInt32LittleEndian(anim.AsSpan(4));
            if (animCount != _count)
                throw new InvalidDataException("anim.dat 槽位數與 objects.dat 不符。");
            if (anim.Length != 8 + _count * 25)
                throw new InvalidDataException("anim.dat 長度不符。");
            int emptySlot = Enumerable.Range(0, _count).FirstOrDefault(s => anim[8 + s * 21] == 0, -1);
            if (emptySlot >= 0)
            {
                _emptyAnimRecord = anim.AsSpan(8 + emptySlot * 21, 21).ToArray();
                _emptyAnimTail0 = BinaryPrimitives.ReadUInt16LittleEndian(anim.AsSpan(8 + _count * 21 + emptySlot * 2));
                _emptyAnimTail1 = BinaryPrimitives.ReadUInt16LittleEndian(anim.AsSpan(8 + _count * 21 + _count * 2 + emptySlot * 2));
            }
            else
            {
                _emptyAnimRecord = new byte[21];
                _emptyAnimTail0 = 0;
                _emptyAnimTail1 = 1;
            }
        }

        if (gfxtype is not null)
        {
            if (gfxtype.Length < 8 || BinaryPrimitives.ReadInt32LittleEndian(gfxtype) != 1)
                throw new InvalidDataException("gfxtype.dat 版本不符。");
            int gfxCount = BinaryPrimitives.ReadInt32LittleEndian(gfxtype.AsSpan(4));
            if (gfxCount != _count)
                throw new InvalidDataException("gfxtype.dat 槽位數與 objects.dat 不符。");
            if (gfxtype.Length != 8 + _count * 17)
                throw new InvalidDataException("gfxtype.dat 長度不符。");
            int emptySlot = Enumerable.Range(0, _count).FirstOrDefault(s => gfxtype[8 + s * 15] == 0, -1);
            if (emptySlot >= 0)
            {
                _emptyGfxtypeRecord = gfxtype.AsSpan(8 + emptySlot * 15, 15).ToArray();
                _emptyGfxtypeTail = BinaryPrimitives.ReadUInt16LittleEndian(gfxtype.AsSpan(8 + _count * 15 + emptySlot * 2));
            }
            else
            {
                _emptyGfxtypeRecord = new byte[15];
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(1), 0);
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(3), 0xFFFF);
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(5), 0);
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(7), 0);
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(9), 0xFFFF);
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(11), 0x0100);
                BinaryPrimitives.WriteUInt16LittleEndian(_emptyGfxtypeRecord.AsSpan(13), 0);
                _emptyGfxtypeTail = 99;
            }
        }

        if (action is not null)
        {
            if (action.Length < 12 || BinaryPrimitives.ReadInt32LittleEndian(action) != 1)
                throw new InvalidDataException("action.dat 版本不符。");
            int actCount = BinaryPrimitives.ReadInt32LittleEndian(action.AsSpan(4));
            if (actCount != _count)
                throw new InvalidDataException("action.dat 槽位數與 objects.dat 不符。");
            int words = BinaryPrimitives.ReadInt32LittleEndian(action.AsSpan(8));
            if (words != 6 || action.Length != 12 + _count * 25)
                throw new InvalidDataException("action.dat 長度或欄位數不符。");
            int emptySlot = Enumerable.Range(0, _count).FirstOrDefault(s => action[12 + s * 25] == 0, -1);
            if (emptySlot >= 0)
            {
                _emptyActionRecord = action.AsSpan(12 + emptySlot * 25, 25).ToArray();
            }
            else
            {
                _emptyActionRecord = new byte[25];
            }
        }

        if (engine is not null)
        {
            if (engine.Length != 94 || BinaryPrimitives.ReadInt32LittleEndian(engine) != 1)
                throw new InvalidDataException("engine.dat 版本或長度不符。");
        }
    }

    public int Capacity => _count;
    public bool HasAnimPool => _anim is not null;
    public bool HasGfxtypePool => _gfxtype is not null;
    public bool HasActionPool => _action is not null;
    public bool HasEngineData => _engine is not null;

    /// <summary>僅供新空白地圖初始化；連結物件與所有位置也清除，呼叫端必須同時初始化原生執行期資料。</summary>
    internal void ResetForBlankMap()
    {
        EnsureAvailablePools();
        for (int slot = 0; slot < _count; slot++)
        {
            _emptyRecord.CopyTo(_objects, RecordOffset(slot));
            for (int column = 0; column < ColumnWidths.Length; column++) WriteColumn(column, slot, _emptyColumns[column]);
            WriteColumn(DirectLightColumn, slot, 0xFFFF);
            for (int segment = 0; segment < ObjDataWidths.Length; segment++) _emptyObjData[segment].CopyTo(_objdata, SegmentOffset(segment, slot));

            if (_anim is not null && _emptyAnimRecord is not null)
            {
                _emptyAnimRecord.CopyTo(_anim, 8 + slot * 21);
                BinaryPrimitives.WriteUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + slot * 2), _emptyAnimTail0);
                BinaryPrimitives.WriteUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + _count * 2 + slot * 2), _emptyAnimTail1);
            }

            if (_gfxtype is not null && _emptyGfxtypeRecord is not null)
            {
                _emptyGfxtypeRecord.CopyTo(_gfxtype, 8 + slot * 15);
                BinaryPrimitives.WriteUInt16LittleEndian(_gfxtype.AsSpan(8 + _count * 15 + slot * 2), _emptyGfxtypeTail);
            }

            if (_action is not null && _emptyActionRecord is not null)
            {
                _emptyActionRecord.CopyTo(_action, 12 + slot * 25);
            }
        }
        _positions.AsSpan(8).Clear();
        _lights?.ReleaseBatch(Enumerable.Range(0, NativeLightPool.Capacity).ToArray());
        _particles?.ReleaseBatch(Enumerable.Range(0, NativeParticlePool.Capacity).ToArray());
        _lagers?.ReleaseBatch(Enumerable.Range(0, NativeLagerPool.Capacity).ToArray());
        _bigLagers?.ReleaseBatch(Enumerable.Range(0, NativeBigLagerPool.Capacity).ToArray());
        _garrisonMembers.Clear();
        _lightOwners.Clear(); _lightParents.Clear();
        _referencedParents.Clear();
        _nextUid = 1;
        if (_engine is not null && _engine.Length >= 38)
        {
            BinaryPrimitives.WriteUInt32LittleEndian(_engine.AsSpan(34), 1);
        }
    }

    public static LevelObjectStore Load(string mapDirectory, IReadOnlyDictionary<int, NativeObjectDefinition>? definitions = null,
        IReadOnlyDictionary<int, NativeLightTypeDefinition>? lightDefinitions = null)
    {
        (byte[] objects, byte[]? h1) = Read(Path.Combine(mapDirectory, "DATA", "objects.dat"));
        (byte[] objdata, byte[]? h2) = Read(Path.Combine(mapDirectory, "DATA", "objdata.dat"));
        (byte[] positions, byte[]? h3) = Read(Path.Combine(mapDirectory, "DATA", "position.dat"));

        int objectCount = objects.Length >= 8 ? BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(4)) : 0;

        byte[]? anim = null, gfxtype = null, action = null, engine = null;
        byte[]? hAnim = null, hGfx = null, hAct = null, hEng = null;
        var unavailable = new List<string>();

        string animPath = Path.Combine(mapDirectory, "DATA", "anim.dat");
        if (File.Exists(animPath))
        {
            try
            {
                (byte[] raw, byte[]? h) = Read(animPath);
                if (raw.Length >= 8 && BinaryPrimitives.ReadInt32LittleEndian(raw) == 1
                    && BinaryPrimitives.ReadInt32LittleEndian(raw.AsSpan(4)) == objectCount
                    && raw.Length == 8 + objectCount * 25)
                {
                    anim = raw; hAnim = h;
                }
            }
            catch (Exception ex) when (ex is InvalidDataException or IOException) { }
            if (anim is null) unavailable.Add("anim.dat");
        }

        string gfxPath = Path.Combine(mapDirectory, "DATA", "gfxtype.dat");
        if (File.Exists(gfxPath))
        {
            try
            {
                (byte[] raw, byte[]? h) = Read(gfxPath);
                if (raw.Length >= 8 && BinaryPrimitives.ReadInt32LittleEndian(raw) == 1
                    && BinaryPrimitives.ReadInt32LittleEndian(raw.AsSpan(4)) == objectCount
                    && raw.Length == 8 + objectCount * 17)
                {
                    gfxtype = raw; hGfx = h;
                }
            }
            catch (Exception ex) when (ex is InvalidDataException or IOException) { }
            if (gfxtype is null) unavailable.Add("gfxtype.dat");
        }

        string actPath = Path.Combine(mapDirectory, "DATA", "action.dat");
        if (File.Exists(actPath))
        {
            try
            {
                (byte[] raw, byte[]? h) = Read(actPath);
                if (raw.Length >= 12 && BinaryPrimitives.ReadInt32LittleEndian(raw) == 1
                    && BinaryPrimitives.ReadInt32LittleEndian(raw.AsSpan(4)) == objectCount
                    && BinaryPrimitives.ReadInt32LittleEndian(raw.AsSpan(8)) == 6
                    && raw.Length == 12 + objectCount * 25)
                {
                    action = raw; hAct = h;
                }
            }
            catch (Exception ex) when (ex is InvalidDataException or IOException) { }
            if (action is null) unavailable.Add("action.dat");
        }

        string engPath = Path.Combine(mapDirectory, "DATA", "engine.dat");
        if (File.Exists(engPath))
        {
            try
            {
                (byte[] raw, byte[]? h) = Read(engPath);
                if (raw.Length == 94 && BinaryPrimitives.ReadInt32LittleEndian(raw) == 1)
                {
                    engine = raw; hEng = h;
                }
            }
            catch (Exception ex) when (ex is InvalidDataException or IOException) { }
            if (engine is null) unavailable.Add("engine.dat");
        }

        NativeLightPool? lights = null;
        string lightPath = Path.Combine(mapDirectory, "DATA", "light.dat");
        if (File.Exists(lightPath))
        {
            try { lights = NativeLightPool.Load(lightPath); }
            catch (Exception ex) when (ex is IOException or InvalidDataException) { unavailable.Add("light.dat"); }
        }
        NativeParticlePool? particles = null;
        string particlePath = Path.Combine(mapDirectory, "DATA", "particle.dat");
        if (File.Exists(particlePath))
        {
            try { particles = NativeParticlePool.Load(particlePath); }
            catch (Exception ex) when (ex is IOException or InvalidDataException) { unavailable.Add("particle.dat"); }
        }
        NativeLagerPool? lagers = null; NativeBigLagerPool? bigLagers = null;
        string lagerPath = Path.Combine(mapDirectory, "DATA", "lager.dat");
        string bigLagerPath = Path.Combine(mapDirectory, "DATA", "biglager.dat");
        if (File.Exists(lagerPath))
        {
            try { lagers = NativeLagerPool.Load(lagerPath); }
            catch (Exception ex) when (ex is IOException or InvalidDataException) { unavailable.Add("lager.dat"); }
        }
        if (File.Exists(bigLagerPath))
        {
            try { bigLagers = NativeBigLagerPool.Load(bigLagerPath); }
            catch (Exception ex) when (ex is IOException or InvalidDataException) { unavailable.Add("biglager.dat"); }
        }
        return new LevelObjectStore(objects, objdata, positions, h1, h2, h3,
            anim, hAnim, gfxtype, hGfx, action, hAct, engine, hEng, unavailable.AsReadOnly(), definitions, lights, lightDefinitions, particles, lagers, bigLagers);
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
                z = BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(p + 8));
                // Native position +12 is degrees; editor/render APIs use radians.
                rotation = (float)(BinaryPrimitives.ReadSingleLittleEndian(_positions.AsSpan(p + 12)) * (Math.PI / 180d));
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
                    if (include(template.TypeId))
                    {
                        template.InitializeDefinitionVitals = true;
                        templates.TryAdd(template.TypeId, template);
                    }
            }
            catch (Exception ex) when (ex is InvalidDataException or IOException) { }
        }
        return templates;
    }

    /// <summary>可安全複製的範本：未與其他物件連結，含可選第三位置的索引均有效。</summary>
    public IEnumerable<LevelObjectTemplate> Templates()
    {
        var seen = new HashSet<int>();
        for (int slot = 0; slot < _count; slot++)
        {
            if (!IsActive(slot) || IsLinked(slot) || !seen.Add(TypeId(slot))) continue;
            int record = RecordOffset(slot);
            int p0 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67)), p1 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69));
            if (p0 >= _positionCount || p1 >= _positionCount) { seen.Remove(TypeId(slot)); continue; }
            int p2 = (int)ReadColumn(ThirdPositionColumn, slot);
            if (!ValidOptionalPosition(p2)) { seen.Remove(TypeId(slot)); continue; }

            byte[]? animRec = null; ushort animT0 = 0, animT1 = 0;
            if (_anim is not null)
            {
                int aSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 71));
                if (aSlot < _count && _anim[8 + aSlot * 21] != 0)
                {
                    animRec = _anim.AsSpan(8 + aSlot * 21, 21).ToArray();
                    animT0 = BinaryPrimitives.ReadUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + aSlot * 2));
                    animT1 = BinaryPrimitives.ReadUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + _count * 2 + aSlot * 2));
                }
            }

            byte[]? gfxRec = null; ushort gfxT = 0;
            if (_gfxtype is not null)
            {
                int gSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 73));
                if (gSlot < _count && _gfxtype[8 + gSlot * 15] != 0)
                {
                    gfxRec = _gfxtype.AsSpan(8 + gSlot * 15, 15).ToArray();
                    gfxT = BinaryPrimitives.ReadUInt16LittleEndian(_gfxtype.AsSpan(8 + _count * 15 + gSlot * 2));
                }
            }

            byte[]? actRec = null;
            if (_action is not null)
            {
                int actSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 77));
                if (actSlot < _count && _action[12 + actSlot * 25] != 0)
                {
                    actRec = _action.AsSpan(12 + actSlot * 25, 25).ToArray();
                }
            }

            yield return new LevelObjectTemplate(TypeId(slot), _objects.AsSpan(record, RecordSize).ToArray(),
                Enumerable.Range(0, ColumnWidths.Length).Select(column => ReadColumn(column, slot)).ToArray(),
                Enumerable.Range(0, ObjDataWidths.Length).Select(segment => _objdata.AsSpan(SegmentOffset(segment, (int)ReadColumn(ObjDataColumn, slot)), ObjDataWidths[segment]).ToArray()).ToArray(),
                _positions.AsSpan(8 + p0 * PositionSize, PositionSize).ToArray(), _positions.AsSpan(8 + p1 * PositionSize, PositionSize).ToArray(),
                animRec, animT0, animT1, gfxRec, gfxT, actRec,
                p2 == 0xFFFF ? null : _positions.AsSpan(8 + p2 * PositionSize, PositionSize).ToArray())
                { DirectLight = CaptureDirectLight(slot) };
        }
    }

    /// <summary>Capture an owned, unlinked object's template only while its runtime UID still matches.</summary>
    public LevelObjectTemplate? OwnedTemplate(int slot, uint uid)
    {
        if (uid == 0 || UidAt(slot) != uid || IsLinked(slot)) return null;
        return CaptureSavedTemplate(slot);
    }

    private LevelObjectTemplate? CaptureSavedTemplate(int slot)
    {
        int record = RecordOffset(slot);
        int p0 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67)), p1 = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69));
        if (p0 >= _positionCount || p1 >= _positionCount) return null;
        int p2 = (int)ReadColumn(ThirdPositionColumn, slot);
        if (!ValidOptionalPosition(p2)) return null;

        byte[]? animRec = null; ushort animT0 = 0, animT1 = 0;
        if (_anim is not null)
        {
            int aSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 71));
            if (aSlot < _count && _anim[8 + aSlot * 21] != 0)
            {
                animRec = _anim.AsSpan(8 + aSlot * 21, 21).ToArray();
                animT0 = BinaryPrimitives.ReadUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + aSlot * 2));
                animT1 = BinaryPrimitives.ReadUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + _count * 2 + aSlot * 2));
            }
        }

        byte[]? gfxRec = null; ushort gfxT = 0;
        if (_gfxtype is not null)
        {
            int gSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 73));
            if (gSlot < _count && _gfxtype[8 + gSlot * 15] != 0)
            {
                gfxRec = _gfxtype.AsSpan(8 + gSlot * 15, 15).ToArray();
                gfxT = BinaryPrimitives.ReadUInt16LittleEndian(_gfxtype.AsSpan(8 + _count * 15 + gSlot * 2));
            }
        }

        byte[]? actRec = null;
        if (_action is not null)
        {
            int actSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 77));
            if (actSlot < _count && _action[12 + actSlot * 25] != 0)
            {
                actRec = _action.AsSpan(12 + actSlot * 25, 25).ToArray();
            }
        }

        return new LevelObjectTemplate(TypeId(slot), _objects.AsSpan(record, RecordSize).ToArray(),
            Enumerable.Range(0, ColumnWidths.Length).Select(column => ReadColumn(column, slot)).ToArray(),
            Enumerable.Range(0, ObjDataWidths.Length).Select(segment => _objdata.AsSpan(SegmentOffset(segment, (int)ReadColumn(ObjDataColumn, slot)), ObjDataWidths[segment]).ToArray()).ToArray(),
            _positions.AsSpan(8 + p0 * PositionSize, PositionSize).ToArray(), _positions.AsSpan(8 + p1 * PositionSize, PositionSize).ToArray(),
            animRec, animT0, animT1, gfxRec, gfxT, actRec,
            p2 == 0xFFFF ? null : _positions.AsSpan(8 + p2 * PositionSize, PositionSize).ToArray())
            { DirectLight = CaptureDirectLight(slot) };
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
        float rotationDegrees = (float)(rotation * (180d / Math.PI));
        if (!float.IsFinite(rotationDegrees)) throw new ArgumentOutOfRangeException(nameof(rotation), "方向超出原生 float 度數範圍。");
        int slot = Enumerable.Range(0, _count).FirstOrDefault(index => !IsActive(index) && !_referencedParents.Contains(index)
            && !_lightParents.Contains(index) && !_lightOwners.ContainsKey(index) && !_garrisonMembers.Contains(index), -1);
        ValidateTemplate(template);
        if (template.Columns[ParentObjectColumn] != 0xFFFF)
            throw new InvalidDataException("物件範本含有附屬物件 parent 關聯，尚未支援完整關聯重建；待存編輯已保留。");
        EnsureAvailablePools();
        if (template.AnimRecord is not null && _anim is null) throw MissingRequiredPool("anim.dat");
        if (template.GfxtypeRecord is not null && _gfxtype is null) throw MissingRequiredPool("gfxtype.dat");
        if (template.ActionRecord is not null && _action is null) throw MissingRequiredPool("action.dat");
        byte[]? thirdPosition = template.Position2;
        if (_definitions is { Count: > 0 })
        {
            if (!_definitions.TryGetValue(template.TypeId, out var definition) || !definition.Active)
                throw new InvalidDataException($"物件類型 {template.TypeId} 沒有有效的 objdef 定義，待存編輯已保留。");
            if (template.InitializeDefinitionVitals && definition.InitialMoves is { } moves && !float.IsFinite(moves))
                throw new InvalidDataException($"物件類型 {template.TypeId} 的 moves 初值無效，待存編輯已保留。");
            if (definition.RequiresThirdPosition) thirdPosition ??= template.Position0;
            else if (thirdPosition is not null)
                throw new InvalidDataException($"物件類型 {template.TypeId} 的第三位置與 objdef afstr 不一致，待存編輯已保留。");
        }
        int[] positions = FreePositions(thirdPosition is null ? 2 : 3);
        int animSlot = template.AnimRecord is null ? -1 : FreePoolSlot(_anim, 8, 21, 71);
        int gfxSlot = template.GfxtypeRecord is null ? -1 : FreePoolSlot(_gfxtype, 8, 15, 73);
        int actionSlot = template.ActionRecord is null ? -1 : FreePoolSlot(_action, 12, 25, 77);
        int dataSlot = Enumerable.Range(0, _count).FirstOrDefault(index => _objdata[SegmentOffset(0, index)] == 0
            && !DataReferenced(index), -1);
        if (slot < 0 || positions.Length == 0 || dataSlot < 0
            || (template.AnimRecord is not null && animSlot < 0) || (template.GfxtypeRecord is not null && gfxSlot < 0)
            || (template.ActionRecord is not null && actionSlot < 0)) return -1;
        if (_nextUid > 0x00FFFFFF)
            throw new InvalidDataException("地圖物件 UID 計數器已達 24 位元邊界，無法安全新增；既有物件與待存編輯已保留。");
        uint uid = _nextUid;

        NativeLightCreation? directCreation = null;
        bool initializeLight = template.InitializeDefinitionVitals && _lightDefinitions is not null && _definitions is { Count: > 0 };
        if (initializeLight && _definitions!.GetValueOrDefault(template.TypeId) is { DirectLightDefinition: >= 0 } lightObject
            && _lightDefinitions!.TryGetValue(lightObject.DirectLightDefinition, out var lightDefinition) && lightDefinition.Active != 0)
            directCreation = lightDefinition.Create(new(x, y, z), ownerObject: slot);
        if (!initializeLight && template.Columns[DirectLightColumn] != 0xFFFF && template.DirectLight is null)
            throw new InvalidDataException("物件範本含有未捕獲的直接光源，無法複製外來索引。");
        int directSlot = -1;
        if (directCreation is not null || (!initializeLight && template.DirectLight is not null))
        {
            if (_lights is null) throw MissingRequiredPool("light.dat");
            var excluded = Enumerable.Range(0, _count).Where(IsActive)
                .Select(index => (int)ReadColumn(DirectLightColumn, index)).Where(index => index < NativeLightPool.Capacity).ToHashSet();
            if (directCreation is not null)
                directSlot = _lights.AddBatch([directCreation], excluded)?.Single() ?? -1;
            else
            {
                var origin = new System.Numerics.Vector3(BinaryPrimitives.ReadSingleLittleEndian(template.Position0.AsSpan(1)),
                    BinaryPrimitives.ReadSingleLittleEndian(template.Position0.AsSpan(5)), BinaryPrimitives.ReadSingleLittleEndian(template.Position0.AsSpan(9)));
                var lightPosition = _definitions?.GetValueOrDefault(template.TypeId) is { Active: true } ownerDefinition
                    ? ownerDefinition.DirectLightPosition(new(x, y, z), rotationDegrees)
                    : template.DirectLight!.Position + new System.Numerics.Vector3(x, y, z) - origin;
                directSlot = _lights.AddCopy(template.DirectLight!, lightPosition, slot, excluded);
            }
            if (directSlot < 0) return -1;
            _lightOwners[slot] = directSlot;
        }

        int record = RecordOffset(slot);
        template.Record.CopyTo(_objects, record);
        BinaryPrimitives.WriteUInt32LittleEndian(_objects.AsSpan(record + 3), uid);
        if (team is { } owner) BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 1), checked((ushort)owner));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 67), checked((ushort)positions[0]));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 69), checked((ushort)positions[1]));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 71), animSlot < 0 ? ushort.MaxValue : checked((ushort)animSlot));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 73), gfxSlot < 0 ? ushort.MaxValue : checked((ushort)gfxSlot));
        BinaryPrimitives.WriteUInt16LittleEndian(_objects.AsSpan(record + 77), actionSlot < 0 ? ushort.MaxValue : checked((ushort)actionSlot));
        for (int column = 0; column < ColumnWidths.Length; column++) WriteColumn(column, slot, column == ObjDataColumn ? checked((ushort)dataSlot) : template.Columns[column]);
        WriteColumn(ThirdPositionColumn, slot, thirdPosition is null ? 0xFFFFu : checked((ushort)positions[2]));
        WriteColumn(DirectLightColumn, slot, directSlot < 0 ? 0xFFFFu : checked((ushort)directSlot));
        for (int segment = 0; segment < ObjDataWidths.Length; segment++) template.ObjData[segment].CopyTo(_objdata, SegmentOffset(segment, dataSlot));
        if (template.InitializeDefinitionVitals && _definitions?.GetValueOrDefault(template.TypeId) is { InitialHp: { } hp, InitialMorale: { } morale })
        {
            // Native 4a9740: signed definition values converted to float; both ratios start at 1.
            int vitals = SegmentOffset(0, dataSlot), moraleOffset = SegmentOffset(1, dataSlot);
            BinaryPrimitives.WriteSingleLittleEndian(_objdata.AsSpan(vitals + 1), hp);
            BinaryPrimitives.WriteSingleLittleEndian(_objdata.AsSpan(vitals + 5), 1f);
            BinaryPrimitives.WriteSingleLittleEndian(_objdata.AsSpan(moraleOffset), morale);
            BinaryPrimitives.WriteSingleLittleEndian(_objdata.AsSpan(moraleOffset + 4), 1f);
        }
        if (template.InitializeDefinitionVitals && _definitions?.GetValueOrDefault(template.TypeId) is { } creationDefinition)
        {
            // Native 4a9a25/4a9af8 copy definition bits to runtime +14/+4c without conversion.
            if (creationDefinition.InitialMoves is { } moves)
                BinaryPrimitives.WriteSingleLittleEndian(_objdata.AsSpan(SegmentOffset(2, dataSlot)), moves);
            if (creationDefinition.InitialMaxRuhm is { } maxRuhm)
                BinaryPrimitives.WriteInt32LittleEndian(_objdata.AsSpan(SegmentOffset(16, dataSlot)), maxRuhm);
            // Native create +1c is a u16 in serialized segment4; its gameplay name is unverified.
            if (creationDefinition.InitialField1c is { } field1c)
                BinaryPrimitives.WriteUInt16LittleEndian(_objdata.AsSpan(SegmentOffset(4, dataSlot)), field1c);
        }
        WritePosition(positions[0], template.Position0, x, y, z, rotationDegrees);
        WritePosition(positions[1], template.Position1, x, y, z, rotationDegrees);
        if (thirdPosition is not null) WritePosition(positions[2], thirdPosition, x, y, z, rotationDegrees);

        if (_anim is not null && template.AnimRecord is not null)
        {
            template.AnimRecord.CopyTo(_anim, 8 + animSlot * 21);
            _anim[8 + animSlot * 21] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + animSlot * 2), template.AnimTail0);
            BinaryPrimitives.WriteUInt16LittleEndian(_anim.AsSpan(8 + _count * 23 + animSlot * 2), template.AnimTail1);
        }
        if (_gfxtype is not null && template.GfxtypeRecord is not null)
        {
            template.GfxtypeRecord.CopyTo(_gfxtype, 8 + gfxSlot * 15);
            _gfxtype[8 + gfxSlot * 15] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(_gfxtype.AsSpan(8 + _count * 15 + gfxSlot * 2), template.GfxtypeTail);
        }
        if (_action is not null && template.ActionRecord is not null)
        {
            template.ActionRecord.CopyTo(_action, 12 + actionSlot * 25);
            _action[12 + actionSlot * 25] = 1;
        }

        _nextUid = uid + 1;
        if (_engine is not null && _engine.Length >= 38)
            BinaryPrimitives.WriteUInt32LittleEndian(_engine.AsSpan(34), _nextUid);

        return slot;
    }

    /// <summary>刪除一個未連結的物件：釋放其所有位置（含可選第三位置）與關聯池槽位。</summary>
    public bool Remove(int slot)
    {
        if (_unavailablePools.Count > 0 || slot < 0 || slot >= _count || !IsActive(slot) || IsLinked(slot)) return false;
        int record = RecordOffset(slot);
        int animSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 71));
        int gfxSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 73));
        int actionSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 77));
        int dataSlot = (int)ReadColumn(ObjDataColumn, slot);
        // Shared targets cannot be released while another active object owns them.
        if (RecordReferenced(71, animSlot, slot) || RecordReferenced(73, gfxSlot, slot)
            || RecordReferenced(77, actionSlot, slot) || DataReferenced(dataSlot, slot)
            || PositionReferenced(BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 67)), slot)
            || PositionReferenced(BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 69)), slot)
            || PositionReferenced((int)ReadColumn(ThirdPositionColumn, slot), slot)) return false;
        RemoveCore(slot);
        return true;
    }

    // Call only after the single-object or complete-family preflight has validated all relationships.
    private void RemoveCore(int slot)
    {
        int record = RecordOffset(slot);
        int animSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 71));
        int gfxSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 73));
        int actionSlot = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + 77));
        int dataSlot = (int)ReadColumn(ObjDataColumn, slot);
        int directSlot = (int)ReadColumn(DirectLightColumn, slot);
        if (directSlot != 0xFFFF)
        {
            _lights!.ReleaseBatch([directSlot]);
            _lightOwners.Remove(slot);
            WriteColumn(DirectLightColumn, slot, 0xFFFF);
        }
        int third = (int)ReadColumn(ThirdPositionColumn, slot);
        if (third < _positionCount) _emptyPosition.CopyTo(_positions, 8 + third * PositionSize);
        foreach (int offset in new[] { 67, 69 })
        {
            int position = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(record + offset));
            if (position < _positionCount) _emptyPosition.CopyTo(_positions, 8 + position * PositionSize);
        }
        _emptyRecord.CopyTo(_objects, record);
        for (int column = 0; column < ColumnWidths.Length; column++) WriteColumn(column, slot, _emptyColumns[column]);
        WriteColumn(DirectLightColumn, slot, 0xFFFF);
        WriteColumn(ParentObjectColumn, slot, 0xFFFF);
        WriteColumn(5, slot, 0xFFFF);
        WriteColumn(11, slot, 0xFFFF);
        for (int segment = 0; segment < ObjDataWidths.Length; segment++) _emptyObjData[segment].CopyTo(_objdata, SegmentOffset(segment, dataSlot));

        if (_anim is not null && _emptyAnimRecord is not null && animSlot < _count)
        {
            _emptyAnimRecord.CopyTo(_anim, 8 + animSlot * 21);
            BinaryPrimitives.WriteUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + animSlot * 2), _emptyAnimTail0);
            BinaryPrimitives.WriteUInt16LittleEndian(_anim.AsSpan(8 + _count * 21 + _count * 2 + animSlot * 2), _emptyAnimTail1);
        }

        if (_gfxtype is not null && _emptyGfxtypeRecord is not null && gfxSlot < _count)
        {
            _emptyGfxtypeRecord.CopyTo(_gfxtype, 8 + gfxSlot * 15);
            BinaryPrimitives.WriteUInt16LittleEndian(_gfxtype.AsSpan(8 + _count * 15 + gfxSlot * 2), _emptyGfxtypeTail);
        }

        if (_action is not null && _emptyActionRecord is not null && actionSlot < _count)
        {
            _emptyActionRecord.CopyTo(_action, 12 + actionSlot * 25);
        }

    }

    /// <summary>槽位目前物件的 uid；空槽或超出範圍時為 null。</summary>
    public uint? UidAt(int slot)
        => slot >= 0 && slot < _count && IsActive(slot) ? BinaryPrimitives.ReadUInt32LittleEndian(_objects.AsSpan(RecordOffset(slot) + 3)) : null;

    /// <summary>只在槽位仍是指定 uid 的物件時移除（避免誤刪後來被其他編輯佔用的槽位）。</summary>
    public bool RemoveIfUid(int slot, uint uid) => UidAt(slot) == uid && Remove(slot);

    /// <summary>在交易內寫回所有已載入的池檔案（沿用原 PFIL 標頭）。</summary>
    public void Save(string mapDirectory, FileRollbackScope rollback)
    {
        Write(Path.Combine(mapDirectory, "DATA", "objects.dat"), _objects, _objectsHeader, rollback);
        Write(Path.Combine(mapDirectory, "DATA", "objdata.dat"), _objdata, _objdataHeader, rollback);
        Write(Path.Combine(mapDirectory, "DATA", "position.dat"), _positions, _positionsHeader, rollback);
        if (_anim is not null) Write(Path.Combine(mapDirectory, "DATA", "anim.dat"), _anim, _animHeader, rollback);
        if (_gfxtype is not null) Write(Path.Combine(mapDirectory, "DATA", "gfxtype.dat"), _gfxtype, _gfxtypeHeader, rollback);
        if (_action is not null) Write(Path.Combine(mapDirectory, "DATA", "action.dat"), _action, _actionHeader, rollback);
        if (_engine is not null) Write(Path.Combine(mapDirectory, "DATA", "engine.dat"), _engine, _engineHeader, rollback);
        _lights?.Save(Path.Combine(mapDirectory, "DATA", "light.dat"), rollback);
        _particles?.Save(Path.Combine(mapDirectory, "DATA", "particle.dat"), rollback);
        _lagers?.Save(Path.Combine(mapDirectory, "DATA", "lager.dat"), rollback);
        _bigLagers?.Save(Path.Combine(mapDirectory, "DATA", "biglager.dat"), rollback);
    }

    private static void Write(string path, byte[] data, byte[]? header, FileRollbackScope rollback)
        => Core.Services.SafeFileWriter.WriteAllBytes(path, header is null ? data : GameLZSS.CompressPfil(data, header), rollback);

    private bool IsActive(int slot) => _objects[RecordOffset(slot)] != 0;
    private int TypeId(int slot) => BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 75));
    private bool IsLinked(int slot, bool includeFamilyLinks = true)
    {
        uint dataSlot = ReadColumn(ObjDataColumn, slot);
        // Native reader/release: these reference fields need cleanup outside the loaded pools.
        // Column 10 is a position link; objdata segment 2 is a float scalar, not a backlink.
        return _unavailablePools.Count > 0 || dataSlot >= _count || ProtectedReferenceColumns.Any(column => (includeFamilyLinks || column is not (5 or 11)) && ReadColumn(column, slot) != 0xFFFF)
            || (includeFamilyLinks && (ReadColumn(ParentObjectColumn, slot) != 0xFFFF || _referencedParents.Contains(slot)
                || _lightParents.Contains(slot) || _garrisonMembers.Contains(slot))) || BrokenDirectLight(slot)
            || BrokenAuxiliaryLink(slot, _anim, 8, 21, 71)
            || BrokenAuxiliaryLink(slot, _gfxtype, 8, 15, 73)
            || BrokenAuxiliaryLink(slot, _action, 12, 25, 77)
            || !ValidOptionalPosition((int)ReadColumn(ThirdPositionColumn, slot));
    }

    private NativeLightSnapshot? CaptureDirectLight(int slot)
        => _lights?.Capture((int)ReadColumn(DirectLightColumn, slot));

    private bool BrokenDirectLight(int slot)
    {
        int target = (int)ReadColumn(DirectLightColumn, slot);
        if (target == 0xFFFF) return _lightOwners.ContainsKey(slot);
        var light = _lights?.Capture(target);
        return light is null || light.OwnerObject != slot || light.ParentObject != -1
            || _lightOwners.GetValueOrDefault(slot, -1) != target
            || Enumerable.Range(0, _count).Any(index => index != slot && IsActive(index) && ReadColumn(DirectLightColumn, index) == target);
    }

    private bool BrokenAuxiliaryLink(int slot, byte[]? pool, int header, int stride, int offset)
    {
        if (pool is null) return false; // Missing pools remain available for partial read-only inspection.
        int target = BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + offset));
        return target != 0xFFFF && (target >= _count || pool[header + target * stride] == 0);
    }

    private static InvalidDataException MissingRequiredPool(string name)
        => new($"物件範本需要 {name}，但此地圖資料池缺失或無效；請從此地圖備份還原，待存編輯已保留。");

    private void EnsureAvailablePools()
    {
        if (_unavailablePools.Count > 0)
            throw new InvalidDataException($"地圖資料池無法安全讀取：{string.Join(", ", _unavailablePools)}。請從此地圖備份還原，待存編輯已保留。");
    }

    private bool RecordReferenced(int offset, int target, int except = -1)
        => target != 0xFFFF && Enumerable.Range(0, _count).Any(slot => slot != except && IsActive(slot)
            && BinaryPrimitives.ReadUInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + offset)) == target);

    private bool DataReferenced(int target, int except = -1)
        => target != 0xFFFF && Enumerable.Range(0, _count).Any(slot => slot != except && IsActive(slot)
            && ReadColumn(ObjDataColumn, slot) == target);

    private bool PositionReferenced(int target, int except = -1)
        => RecordReferenced(67, target, except) || RecordReferenced(69, target, except)
            || (target != 0xFFFF && Enumerable.Range(0, _count).Any(slot => slot != except && IsActive(slot)
                && ReadColumn(ThirdPositionColumn, slot) == target));

    private bool ValidOptionalPosition(int position)
        => position == 0xFFFF || (position < _positionCount && _positions[8 + position * PositionSize] != 0);

    private int FreePoolSlot(byte[]? pool, int header, int stride, int linkOffset)
    {
        if (pool is null) return -1;
        return Enumerable.Range(0, _count).FirstOrDefault(index => pool[header + index * stride] == 0
            && !RecordReferenced(linkOffset, index), -1);
    }

    private static void ValidateTemplate(LevelObjectTemplate template)
    {
        if (template.Record.Length != RecordSize || template.Columns.Length != ColumnWidths.Length
            || template.ObjData.Length != ObjDataWidths.Length || template.Position0.Length != PositionSize
            || template.Position1.Length != PositionSize
            || (template.Position2 is not null && template.Position2.Length != PositionSize)
            || ((template.Columns.Length > ThirdPositionColumn && template.Columns[ThirdPositionColumn] != 0xFFFF)
                != (template.Position2 is not null))
            || template.ObjData.Where((bytes, index) => bytes.Length != ObjDataWidths[index]).Any()
            || (template.AnimRecord is not null && template.AnimRecord.Length != 21)
            || (template.GfxtypeRecord is not null && template.GfxtypeRecord.Length != 15)
            || (template.ActionRecord is not null && template.ActionRecord.Length != 25)
            || template.Columns.Where((value, index) => ColumnWidths[index] == 2 && value > ushort.MaxValue).Any())
            throw new InvalidDataException("物件範本欄位長度或索引無效。");
    }
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

    private int[] FreePositions(int required)
    {
        var slots = new List<int>(required);
        for (int index = 0; index < _positionCount; index++)
            if (_positions[8 + index * PositionSize] == 0 && !PositionReferenced(index))
            {
                slots.Add(index);
                if (slots.Count == required) return slots.ToArray();
            }
        return [];
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

    private static byte[] CreateInactiveRecordTemplate()
    {
        // Editor reset policy: detached inactive record when objects pool has no inactive sample.
        // Active byte at offset 0 is 0; UID at offset 3 is 0; known u16 link fields 67, 69, 71, 73, 75, 77 are 0xFFFF.
        byte[] record = new byte[RecordSize];
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(67), ushort.MaxValue);
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(69), ushort.MaxValue);
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(71), ushort.MaxValue);
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(73), ushort.MaxValue);
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(75), ushort.MaxValue);
        BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(77), ushort.MaxValue);
        return record;
    }

    private static uint[] CreateInactiveColumnsTemplate()
    {
        // Editor reset policy: parallel columns sentinel for inactive slot reset when no inactive sample exists.
        // Parallel 16-bit reference columns use 0xFFFF sentinel; 32-bit columns default to 0.
        var columns = new uint[ColumnWidths.Length];
        for (int i = 0; i < ColumnWidths.Length; i++)
        {
            if (ColumnWidths[i] == 2)
            {
                columns[i] = ushort.MaxValue;
            }
        }
        return columns;
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
