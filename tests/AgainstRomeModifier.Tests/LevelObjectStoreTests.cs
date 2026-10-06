using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class LevelObjectStoreTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Load_returns_only_active_objects_with_fields_and_both_link_flags(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        Assert.Equal(8, store.Capacity);
        Assert.Equal(new[]
        {
            new LevelWorldObject(0, 100, 2, 10, 1, 2, 3, .25f, false),
            new LevelWorldObject(2, 100, 4, 30, 21, 22, 23, 2.25f, false),
            new LevelWorldObject(3, 200, 5, 40, 31, 32, 33, 3.25f, true),
            new LevelWorldObject(4, 300, 6, 50, 41, 42, 43, 4.25f, true),
            new LevelWorldObject(5, 400, 7, 60, 0, 0, 0, 0, false),
            new LevelWorldObject(6, 500, 8, 70, 61, 62, 63, 6.25f, false),
        }, store.Objects());
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Templates_deduplicate_types_and_skip_links_and_either_invalid_position(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        Assert.Equal(100, Assert.Single(level.Load().Templates()).TypeId);
        // An invalid earlier instance must not suppress a later valid instance of its type.
        U16(level.Objects, Record(0) + 69, LevelFixture.PositionCount);
        level.Write();
        Assert.Equal(100, Assert.Single(level.Load().Templates()).TypeId);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Add_uses_first_free_slot_and_even_pair_and_copies_template_bytes(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 101.5f, -2, 303, .75f));
        Assert.Equal(new LevelWorldObject(1, 100, 2, 71, 101.5f, -2, 303, .75f, false),
            Assert.Single(store.Objects(), item => item.Slot == 1));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        byte[] objects = level.ReadPayload("objects.dat");
        byte[] data = level.ReadPayload("objdata.dat");
        byte[] positions = level.ReadPayload("position.dat");
        byte[] expectedRecord = Slice(level.Objects, Record(0), LevelObjectStore.RecordSize);
        U32(expectedRecord, 3, 71);
        U16(expectedRecord, 67, 14); U16(expectedRecord, 69, 15);
        foreach (int offset in new[] { 71, 73, 77 }) U16(expectedRecord, offset, 1);
        Assert.Equal(expectedRecord, Slice(objects, Record(1), LevelObjectStore.RecordSize));
        for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
        {
            int width = LevelObjectStore.ColumnWidths[column];
            byte[] expected = Slice(level.Objects, Column(column, 0), width);
            if (column == 1) U16(expected, 0, 1);
            Assert.Equal(expected, Slice(objects, Column(column, 1), width));
        }
        for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
            Assert.Equal(Slice(level.ObjData, Segment(segment, 0), LevelObjectStore.ObjDataWidths[segment]),
                Slice(data, Segment(segment, 1), LevelObjectStore.ObjDataWidths[segment]));
        foreach (int index in new[] { 14, 15 })
        {
            int offset = Position(index);
            Assert.Equal(1, positions[offset]);
            Assert.Equal(101.5f, F32(positions, offset + 1));
            Assert.Equal(-2f, F32(positions, offset + 5));
            Assert.Equal(303f, F32(positions, offset + 9));
            Assert.Equal(.75f, F32(positions, offset + 13));
        }
        Assert.Equal(Slice(level.Positions, Position(12), 2 * LevelObjectStore.PositionSize),
            Slice(positions, Position(12), 2 * LevelObjectStore.PositionSize));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Add_returns_minus_one_when_slots_or_even_position_pairs_are_exhausted(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 1, 2, 3, 4));
        Assert.Equal(7, store.Add(template, 1, 2, 3, 4));
        var before = store.Objects().ToArray();
        Assert.Equal(-1, store.Add(template, 1, 2, 3, 4));
        Assert.Equal(before, store.Objects());
        // There are free individual positions, but no completely free even pair.
        for (int index = 0; index < LevelFixture.PositionCount; index++)
            level.Positions[Position(index)] = (byte)(index % 2 == 0 ? 0 : 1);
        level.Write();
        store = level.Load();
        before = store.Objects().ToArray();
        Assert.Equal(-1, store.Add(Assert.Single(store.Templates()), 1, 2, 3, 4));
        Assert.Equal(before, store.Objects());
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Add_rejects_non_finite_coordinates_and_rotation_without_mutation(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        LevelObjectTemplate template = Assert.Single(store.Templates());
        foreach (float value in new[] { float.NaN, float.PositiveInfinity, float.NegativeInfinity })
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, value, 2, 3, 4));
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, value, 3, 4));
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, 2, value, 4));
            Assert.Throws<ArgumentOutOfRangeException>(() => store.Add(template, 1, 2, 3, value));
        }
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        Assert.Equal(level.Objects, level.ReadPayload("objects.dat"));
        Assert.Equal(level.ObjData, level.ReadPayload("objdata.dat"));
        Assert.Equal(level.Positions, level.ReadPayload("position.dat"));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Remove_restores_captured_empty_bytes_and_frees_both_positions(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        Assert.True(store.Remove(0));
        Assert.DoesNotContain(store.Objects(), item => item.Slot == 0);
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        byte[] expectedObjects = (byte[])level.Objects.Clone();
        byte[] expectedData = (byte[])level.ObjData.Clone();
        byte[] expectedPositions = (byte[])level.Positions.Clone();
        Array.Copy(level.Objects, Record(1), expectedObjects, Record(0), LevelObjectStore.RecordSize);
        for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
            Array.Copy(level.Objects, Column(column, 1), expectedObjects, Column(column, 0), LevelObjectStore.ColumnWidths[column]);
        for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
            Array.Copy(level.ObjData, Segment(segment, 1), expectedData, Segment(segment, 0), LevelObjectStore.ObjDataWidths[segment]);
        foreach (int index in new[] { 0, 1 })
            Array.Copy(level.Positions, Position(12), expectedPositions, Position(index), LevelObjectStore.PositionSize);
        Assert.Equal(expectedObjects, level.ReadPayload("objects.dat"));
        Assert.Equal(expectedData, level.ReadPayload("objdata.dat"));
        Assert.Equal(expectedPositions, level.ReadPayload("position.dat"));
        Assert.Equal(0, store.Add(Assert.Single(store.Templates()), 9, 8, 7, 6));
        store.Save(level.Map, rollback);
        Assert.Equal(0, BinaryPrimitives.ReadUInt16LittleEndian(level.ReadPayload("objects.dat").AsSpan(Record(0) + 67)));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Remove_refuses_linked_inactive_and_out_of_range_slots_without_mutation(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        LevelObjectStore store = level.Load();
        foreach (int slot in new[] { -1, 8, int.MaxValue, 1, 7, 3, 4 }) Assert.False(store.Remove(slot));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(level.Map, rollback);
        Assert.Equal(level.Objects, level.ReadPayload("objects.dat"));
        Assert.Equal(level.ObjData, level.ReadPayload("objdata.dat"));
        Assert.Equal(level.Positions, level.ReadPayload("position.dat"));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Save_reloads_objects_preserves_encoding_and_headers_and_rolls_back(bool pfil)
    {
        using var level = new LevelFixture(pfil);
        byte[][] originals = LevelFixture.Files.Select(level.ReadRaw).ToArray();
        LevelObjectStore store = level.Load();
        Assert.Equal(1, store.Add(Assert.Single(store.Templates()), 123, -45, 678, 1.25f));
        Assert.True(store.Remove(2));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        {
            store.Save(level.Map, rollback);
            Assert.Equal(store.Objects(), level.Load().Objects());
            for (int file = 0; file < LevelFixture.Files.Length; file++)
            {
                byte[] saved = level.ReadRaw(LevelFixture.Files[file]);
                Assert.False(originals[file].SequenceEqual(saved));
                if (pfil)
                {
                    Assert.Equal("PFIL", System.Text.Encoding.ASCII.GetString(saved, 0, 4));
                    for (int i = 0; i < 64; i++)
                        if (i < 16 || i >= 20) Assert.Equal(originals[file][i], saved[i]);
                    Assert.Equal(level.ReadPayload(LevelFixture.Files[file]).Length,
                        BinaryPrimitives.ReadInt32LittleEndian(saved.AsSpan(16)));
                }
                else Assert.Equal(level.ReadPayload(LevelFixture.Files[file]), saved);
            }
        }
        for (int file = 0; file < LevelFixture.Files.Length; file++)
            Assert.Equal(originals[file], level.ReadRaw(LevelFixture.Files[file]));
    }

    [Theory]
    [InlineData("objects-version")]
    [InlineData("name-width")]
    [InlineData("idname-width")]
    [InlineData("objects-length")]
    [InlineData("objdata-length")]
    [InlineData("position-length")]
    [InlineData("objdata-count")]
    [InlineData("no-empty-slot")]
    public void Constructor_rejects_invalid_layout(string corruption)
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var level = new LevelFixture(pfil);
            switch (corruption)
            {
                case "objects-version": U32(level.Objects, 0, 2); break;
                case "name-width": U32(level.Objects, 8, 29); break;
                case "idname-width": U32(level.Objects, 12, 31); break;
                case "objects-length": level.Objects = level.Objects[..^1]; break;
                case "objdata-length": level.ObjData = level.ObjData[..^1]; break;
                case "position-length": level.Positions = level.Positions[..^1]; break;
                case "objdata-count": U32(level.ObjData, 4, 7); break;
                case "no-empty-slot":
                    for (int slot = 0; slot < LevelFixture.Count; slot++) level.Objects[Record(slot)] = 1;
                    break;
            }
            level.Write();
            Assert.Throws<InvalidDataException>(() => level.Load());
        }
    }

    [Fact]
    public void Constructor_rejects_wrong_objdata_version()
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var level = new LevelFixture(pfil);
            U32(level.ObjData, 0, 2); level.Write();
            Assert.Throws<InvalidDataException>(() => level.Load());
        }
    }

    [Fact]
    public void Constructor_rejects_wrong_position_version()
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var level = new LevelFixture(pfil);
            U32(level.Positions, 0, 2); level.Write();
            Assert.Throws<InvalidDataException>(() => level.Load());
        }
    }

    [Theory]
    [InlineData(null, false)]
    [InlineData("", false)]
    [InlineData("Lan", true)]
    [InlineData("lanTree", true)]
    [InlineData("LAN_Rock", true)]
    [InlineData("La", false)]
    [InlineData("GerLan", false)]
    [InlineData(" LanTree", false)]
    [InlineData("Ger_House", false)]
    public void IsLandscape_matches_only_case_insensitive_Lan_prefix(string? name, bool expected)
        => Assert.Equal(expected, ObjDefNames.IsLandscape(name));

    private static int Record(int slot) => 16 + slot * LevelObjectStore.RecordSize;
    private static int Position(int index) => 8 + index * LevelObjectStore.PositionSize;
    private static int Column(int column, int slot) => 16 + LevelFixture.Count * LevelObjectStore.RecordSize
        + LevelFixture.Count * LevelObjectStore.ColumnWidths.Take(column).Sum() + slot * LevelObjectStore.ColumnWidths[column];
    private static int Segment(int segment, int slot) => 8
        + LevelFixture.Count * LevelObjectStore.ObjDataWidths.Take(segment).Sum() + slot * LevelObjectStore.ObjDataWidths[segment];
    private static byte[] Slice(byte[] bytes, int offset, int length) => bytes.AsSpan(offset, length).ToArray();
    private static void U16(byte[] bytes, int offset, int value) => BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset), checked((ushort)value));
    private static void U32(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
    private static float F32(byte[] bytes, int offset) => BinaryPrimitives.ReadSingleLittleEndian(bytes.AsSpan(offset));

    private sealed class LevelFixture : IDisposable
    {
        internal const int Count = 8, PositionCount = 20;
        internal static readonly string[] Files = ["objects.dat", "objdata.dat", "position.dat"];
        private readonly string _root = Path.Combine(Path.GetTempPath(), "LevelObjectStoreTests_" + Guid.NewGuid().ToString("N"));
        private readonly bool _pfil;
        internal string Map => Path.Combine(_root, "MAP");
        internal byte[] Objects = new byte[16 + Count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
        internal byte[] ObjData = new byte[8 + Count * LevelObjectStore.ObjDataWidths.Sum()];
        internal byte[] Positions = new byte[8 + PositionCount * LevelObjectStore.PositionSize];

        internal LevelFixture(bool pfil)
        {
            _pfil = pfil;
            Directory.CreateDirectory(Path.Combine(Map, "DATA"));
            U32(Objects, 0, 1); U32(Objects, 4, Count); U32(Objects, 8, 30); U32(Objects, 12, 30);
            U32(ObjData, 0, 1); U32(ObjData, 4, Count);
            U32(Positions, 0, 1); U32(Positions, 4, PositionCount);
            int[] types = [100, 0, 100, 200, 300, 400, 500, 0];
            for (int slot = 0; slot < Count; slot++)
            {
                bool active = slot != 1 && slot != 7;
                int record = Record(slot);
                Objects[record] = active ? (byte)1 : (byte)0;
                U16(Objects, record + 1, active ? slot + 2 : 0x7FFF);
                U32(Objects, record + 3, active ? (uint)(slot + 1) * 10 : 0);
                if (active)
                {
                    SyntheticFixture.GameEncoding.GetBytes($"Name{slot}").CopyTo(Objects, record + 7);
                    SyntheticFixture.GameEncoding.GetBytes($"Id{slot}").CopyTo(Objects, record + 37);
                }
                foreach (int offset in new[] { 67, 69, 71, 73, 75, 77 }) U16(Objects, record + offset, 0xFFFF);
                if (active)
                {
                    int pair = slot == 0 ? 0 : (slot - 1) * 2;
                    U16(Objects, record + 67, slot == 5 ? PositionCount : pair);
                    U16(Objects, record + 69, slot == 6 ? PositionCount : pair + 1);
                    foreach (int offset in new[] { 71, 73, 77 }) U16(Objects, record + offset, slot);
                    U16(Objects, record + 75, types[slot]);
                    foreach (int index in new[] { pair, pair + 1 })
                    {
                        Positions[Position(index)] = 1;
                        float[] values = [slot * 10 + 1, slot * 10 + 2, slot * 10 + 3, slot + .25f];
                        for (int i = 0; i < values.Length; i++)
                            BinaryPrimitives.WriteSingleLittleEndian(Positions.AsSpan(Position(index) + 1 + i * 4), values[i]);
                    }
                }
                for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
                {
                    uint value = column == 1 ? (uint)(active ? slot : 0xFFFF)
                        : column == 10 ? (uint)(slot == 3 ? 4 : 0xFFFF) : (uint)(100 + column * 11 + slot);
                    if (LevelObjectStore.ColumnWidths[column] == 2) U16(Objects, Column(column, slot), (int)value);
                    else U32(Objects, Column(column, slot), 0x12340000 + value);
                }
                for (int segment = 0; segment < LevelObjectStore.ObjDataWidths.Length; segment++)
                    for (int i = 0; i < LevelObjectStore.ObjDataWidths[segment]; i++)
                        ObjData[Segment(segment, slot) + i] = segment == 2
                            ? (byte)(slot == 4 && i == 3 ? 1 : 0) : (byte)(20 + segment + slot + i);
            }
            // Position 12 is free but 13 is occupied; 14,15 is the first free even pair.
            Positions[Position(13)] = 1;
            BinaryPrimitives.WriteSingleLittleEndian(Positions.AsSpan(Position(12) + 1), -999);
            Write();
        }

        internal LevelObjectStore Load() => LevelObjectStore.Load(Map);
        internal byte[] ReadRaw(string file) => File.ReadAllBytes(Path.Combine(Map, "DATA", file));
        internal byte[] ReadPayload(string file) => GameLZSS.DecompressPfil(ReadRaw(file));
        internal void Write()
        {
            byte[][] payloads = [Objects, ObjData, Positions];
            for (int file = 0; file < Files.Length; file++)
            {
                byte[] header = Enumerable.Range(0, 64).Select(i => (byte)(i + 30 + file)).ToArray();
                "PFIL"u8.CopyTo(header);
                File.WriteAllBytes(Path.Combine(Map, "DATA", Files[file]),
                    _pfil ? GameLZSS.CompressPfil(payloads[file], header) : payloads[file]);
            }
        }

        public void Dispose() => Directory.Delete(_root, recursive: true);
    }
}
