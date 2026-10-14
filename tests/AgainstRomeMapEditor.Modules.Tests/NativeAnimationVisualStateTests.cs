using System.Buffers.Binary;
using System.Security.Cryptography;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeAnimationVisualStateTests
{
    [Fact]
    public void Parser_preserves_signed_frame_and_native_axis_permutation()
    {
        byte[] record = Record(-123, -234, 345, short.MinValue, short.MaxValue);
        byte[] hash = SHA256.HashData(record);
        var state = Assert.IsType<NativeAnimationVisualState>(NativeAnimationVisualState.ParseRecord(record));
        Assert.Equal((short)-123, state.AlrFrame);
        Assert.Equal((short)-234, state.AptAxis0);
        Assert.Equal((short)345, state.AptAxis1);
        Assert.Equal(short.MinValue, state.AptAxis2);
        Assert.Equal(short.MaxValue, state.AptAxis3);
        Assert.Equal(hash, SHA256.HashData(record));
        Assert.NotEqual(BinaryPrimitives.ReadInt16LittleEndian(record.AsSpan(11)), state.AptAxis0);
    }

    [Fact]
    public void Parser_rejects_every_truncation_extra_bytes_and_inactive_record()
    {
        byte[] record = Record(0, 0, 0, 0, 0);
        for (int length = 0; length < NativeAnimationVisualState.RecordSize; length++)
            Assert.Null(NativeAnimationVisualState.ParseRecord(record.AsSpan(0, length)));
        Assert.Null(NativeAnimationVisualState.ParseRecord(record.Concat(new byte[] { 0 }).ToArray()));
        record[0] = 0;
        Assert.Null(NativeAnimationVisualState.ParseRecord(record));
    }

    [Theory]
    [InlineData(short.MinValue)]
    [InlineData(-1)]
    [InlineData(0)]
    [InlineData(short.MaxValue)]
    public void Frame_retains_entire_signed_word_domain(short frame)
    {
        Assert.Equal(frame, NativeAnimationVisualState.ParseRecord(Record(frame, 0, 0, 0, 0))!.AlrFrame);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Child_without_animation_does_not_inherit_root_state(bool pfil)
    {
        using var fixture = new Fixture(pfil);
        U16(fixture.Objects, 16 + 2 * 79 + 71, 0xFFFF);
        fixture.WriteObjects();
        var hashes = fixture.Hashes();
        var store = LevelObjectStore.Load(fixture.Map);
        Assert.NotNull(store.AnimationVisualStateAt(0, 0xAB000064));
        Assert.Null(store.AnimationVisualStateAt(2, 200));
        AssertHashes(hashes, fixture.Hashes());
    }

    [Fact]
    public void Detached_state_has_no_setters_or_source_buffer_reference()
    {
        byte[] record = Record(-1, 2, -3, 4, -5);
        var state = Assert.IsType<NativeAnimationVisualState>(NativeAnimationVisualState.ParseRecord(record));
        record.AsSpan().Fill(0xFF);
        Assert.Equal(new NativeAnimationVisualState(-1, 2, -3, 4, -5), state);
        foreach (string name in new[] { "AlrFrame", "AptAxis0", "AptAxis1", "AptAxis2", "AptAxis3" })
            Assert.Null(typeof(NativeAnimationVisualState).GetProperty(name)!.SetMethod);
        Assert.Equal(state, state with { });
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Store_reads_nonidentity_links_and_child_own_state_without_changing_source(bool pfil)
    {
        using var fixture = new Fixture(pfil);
        var hashes = fixture.Hashes();
        var store = LevelObjectStore.Load(fixture.Map);
        var root = Assert.IsType<NativeAnimationVisualState>(store.AnimationVisualStateAt(0, 0xAB000064));
        var child = Assert.IsType<NativeAnimationVisualState>(store.AnimationVisualStateAt(2, 200));
        Assert.Equal(new NativeAnimationVisualState(-123, -234, 345, short.MinValue, short.MaxValue), root);
        Assert.Equal(new NativeAnimationVisualState(0, 11, -22, 33, -44), child);
        Assert.NotEqual(root, child);
        for (int i = 0; i < 3; i++)
            Assert.Equal(root, store.AnimationVisualStateAt(0, 0xAB000064));
        AssertHashes(hashes, fixture.Hashes());

        // A separate file edit/reload cannot change an already detached snapshot.
        Record(99, 98, 97, 96, 95).CopyTo(fixture.Anim, 8 + 3 * 21);
        fixture.WriteAnim();
        var reloaded = LevelObjectStore.Load(fixture.Map);
        Assert.Equal((short)99, reloaded.AnimationVisualStateAt(0, 0xAB000064)!.AlrFrame);
        Assert.Equal((short)-123, root.AlrFrame);
        Assert.Equal(root, store.AnimationVisualStateAt(0, 0xAB000064));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Store_rejects_stale_uid_inactive_object_and_out_of_range_object(bool pfil)
    {
        using var fixture = new Fixture(pfil);
        var hashes = fixture.Hashes();
        var store = LevelObjectStore.Load(fixture.Map);
        Assert.Null(store.AnimationVisualStateAt(0, 100)); // Full UID, not just its low 24 bits.
        Assert.Null(store.AnimationVisualStateAt(0, 0xAB000065));
        Assert.Null(store.AnimationVisualStateAt(1, 0));
        Assert.Null(store.AnimationVisualStateAt(-1, 200));
        Assert.Null(store.AnimationVisualStateAt(Fixture.Count, 200));
        AssertHashes(hashes, fixture.Hashes());
    }

    [Theory]
    [InlineData("missing")]
    [InlineData("sentinel")]
    [InlineData("negative")]
    [InlineData("minimum")]
    [InlineData("range")]
    [InlineData("inactive")]
    [InlineData("shared")]
    [InlineData("truncated")]
    [InlineData("version")]
    [InlineData("count")]
    public void Broken_animation_is_absent_never_fabricated_frame_zero(string fault)
    {
        foreach (bool pfil in new[] { false, true })
        {
            using var fixture = new Fixture(pfil);
            switch (fault)
            {
                case "sentinel": U16(fixture.Objects, 16 + 71, 0xFFFF); break;
                case "negative": U16(fixture.Objects, 16 + 71, 0xFFFE); break;
                case "minimum": U16(fixture.Objects, 16 + 71, 0x8000); break;
                case "range": U16(fixture.Objects, 16 + 71, Fixture.Count); break;
                case "inactive": fixture.Anim[8 + 3 * 21] = 0; break;
                case "shared": U16(fixture.Objects, 16 + 2 * 79 + 71, 3); break;
                case "version": U32(fixture.Anim, 0, 2); break;
                case "count": U32(fixture.Anim, 4, Fixture.Count - 1); break;
            }
            fixture.WriteObjects();
            fixture.WriteAnim(fault == "truncated" ? fixture.Anim[..^1] : fixture.Anim);
            if (fault == "missing") File.Delete(Path.Combine(fixture.Data, "anim.dat"));
            var hashes = fixture.Hashes();
            var store = LevelObjectStore.Load(fixture.Map);
            Assert.Null(store.AnimationVisualStateAt(0, 0xAB000064));
            AssertHashes(hashes, fixture.Hashes());
        }
    }

    private static void AssertHashes(Dictionary<string, byte[]> expected, Dictionary<string, byte[]> actual)
    {
        Assert.Equal(expected.Keys.Order(), actual.Keys.Order());
        foreach (var (name, hash) in expected) Assert.Equal(hash, actual[name]);
    }

    private static byte[] Record(short frame, short axis0, short axis1, short axis2, short axis3)
    {
        byte[] record = new byte[21];
        record[0] = 1;
        // Opaque fields deliberately contain non-finite float bit patterns: they
        // are not visual coordinates and must not invalidate signed word extraction.
        U32(record, 1, 0x7FC00000);
        U32(record, 7, 0xFF800000);
        I16(record, 5, frame);
        I16(record, 11, 777);
        I16(record, 13, axis0);
        I16(record, 15, axis2);
        I16(record, 17, axis1);
        I16(record, 19, axis3);
        return record;
    }

    private static void U16(byte[] data, int offset, ushort value) => BinaryPrimitives.WriteUInt16LittleEndian(data.AsSpan(offset), value);
    private static void I16(byte[] data, int offset, short value) => BinaryPrimitives.WriteInt16LittleEndian(data.AsSpan(offset), value);
    private static void U32(byte[] data, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(data.AsSpan(offset), value);

    private sealed class Fixture : IDisposable
    {
        public const ushort Count = 6;
        private readonly bool _pfil;
        public string Map { get; } = Path.Combine(Path.GetTempPath(), "ArmAnimationState_" + Guid.NewGuid().ToString("N"));
        public string Data => Path.Combine(Map, "DATA");
        public byte[] Objects { get; } = new byte[16 + Count * (79 + LevelObjectStore.ColumnWidths.Sum())];
        public byte[] Anim { get; } = new byte[8 + Count * 25];

        public Fixture(bool pfil)
        {
            _pfil = pfil;
            Directory.CreateDirectory(Data);
            U32(Objects, 0, 1); U32(Objects, 4, Count); U32(Objects, 8, 30); U32(Objects, 12, 30);
            for (int slot = 0; slot < Count; slot++)
            {
                foreach (int field in new[] { 67, 69, 71, 73, 75, 77 }) U16(Objects, 16 + slot * 79 + field, 0xFFFF);
                for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
                    if (LevelObjectStore.ColumnWidths[column] == 2) U16(Objects, Column(column, slot), 0xFFFF);
            }
            byte[] objdata = new byte[8 + Count * LevelObjectStore.ObjDataWidths.Sum()];
            U32(objdata, 0, 1); U32(objdata, 4, Count);
            byte[] position = new byte[8 + Count * 17];
            U32(position, 0, 1); U32(position, 4, Count);
            foreach (int slot in new[] { 0, 2 })
            {
                int core = 16 + slot * 79;
                Objects[core] = 1;
                U32(Objects, core + 3, slot == 0 ? 0xAB000064u : 200u);
                U16(Objects, core + 67, (ushort)slot); U16(Objects, core + 69, (ushort)slot);
                U16(Objects, core + 75, 555); U16(Objects, Column(1, slot), (ushort)slot);
                objdata[8 + slot * 9] = 1;
                position[8 + slot * 17] = 1;
            }
            U16(Objects, Column(13, 2), 0); // Legitimate child link; no root-only restriction.
            U16(Objects, 16 + 71, 3); // Object 0 -> anim 3.
            U16(Objects, 16 + 2 * 79 + 71, 1); // Child 2 -> anim 1.
            U32(Anim, 0, 1); U32(Anim, 4, Count);
            Record(-123, -234, 345, short.MinValue, short.MaxValue).CopyTo(Anim, 8 + 3 * 21);
            Record(0, 11, -22, 33, -44).CopyTo(Anim, 8 + 21);
            WriteObjects(); WriteAnim(); Write("objdata.dat", objdata); Write("position.dat", position);
        }

        private static int Column(int column, int slot) => 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(column).Sum()) + slot * LevelObjectStore.ColumnWidths[column];
        public void WriteObjects() => Write("objects.dat", Objects);
        public void WriteAnim(byte[]? data = null) => Write("anim.dat", data ?? Anim);
        private void Write(string name, byte[] payload)
        {
            byte[] header = new byte[64];
            "PFIL"u8.CopyTo(header);
            File.WriteAllBytes(Path.Combine(Data, name), _pfil ? GameLZSS.CompressPfil(payload, header) : payload);
        }
        public Dictionary<string, byte[]> Hashes() => Directory.GetFiles(Data).ToDictionary(path => Path.GetFileName(path)!, path => SHA256.HashData(File.ReadAllBytes(path)));
        public void Dispose() => Directory.Delete(Map, true);
    }
}
