using System.Buffers.Binary;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using System.Security.Cryptography;

namespace AgainstRomeModifier.Tests;

public sealed class LevelObjectStoreCrossPoolTests : IDisposable
{
    private static void U16(byte[] b, int offset, ushort val) => BinaryPrimitives.WriteUInt16LittleEndian(b.AsSpan(offset), val);
    private static void U32(byte[] b, int offset, uint val) => BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(offset), val);
    private static void F32(byte[] b, int offset, float val) => BinaryPrimitives.WriteSingleLittleEndian(b.AsSpan(offset), val);

    private readonly string _dir;
    private readonly string _dataDir;
    private const int Count = 4;
    private const int PosCount = 10;

    public LevelObjectStoreCrossPoolTests()
    {
        _dir = Path.Combine(Path.GetTempPath(), "ARM_CrossPoolTest_" + Guid.NewGuid().ToString("N"));
        _dataDir = Path.Combine(_dir, "DATA");
        Directory.CreateDirectory(_dataDir);
        InitializeStandardPools();
    }

    public void Dispose()
    {
        if (Directory.Exists(_dir))
            Directory.Delete(_dir, recursive: true);
    }

    private void InitializeStandardPools()
    {
        // 1. objects.dat
        int objLen = 16 + Count * (79 + LevelObjectStore.ColumnWidths.Sum());
        byte[] objects = new byte[objLen];
        U32(objects, 0, 1);
        U32(objects, 4, Count);
        U32(objects, 8, 30);
        U32(objects, 12, 30);

        // Slot 0: active object
        objects[16] = 1;
        U16(objects, 16 + 1, 2); // team 2
        U32(objects, 16 + 3, 100); // UID 100
        U16(objects, 16 + 67, 0); // pos0
        U16(objects, 16 + 69, 1); // pos1
        U16(objects, 16 + 71, 0); // anim 0
        U16(objects, 16 + 73, 0); // gfxtype 0
        U16(objects, 16 + 75, 555); // typeId 555
        U16(objects, 16 + 77, 0); // action 0

        // Slot 1..3: empty
        for (int s = 1; s < Count; s++)
        {
            int r = 16 + s * 79;
            objects[r] = 0;
            U16(objects, r + 1, 0x7FFF);
            U16(objects, r + 67, 0xFFFF);
            U16(objects, r + 69, 0xFFFF);
            U16(objects, r + 71, 0xFFFF);
            U16(objects, r + 73, 0xFFFF);
            U16(objects, r + 75, 0xFFFF);
            U16(objects, r + 77, 0xFFFF);
        }

        // Columns
        int colBase = 16 + Count * 79;
        for (int c = 0; c < LevelObjectStore.ColumnWidths.Length; c++)
        {
            int w = LevelObjectStore.ColumnWidths[c];
            for (int s = 0; s < Count; s++)
            {
                int off = colBase + s * w;
                if (c == 1) U16(objects, off, s == 0 ? (ushort)0 : (ushort)0xFFFF); // col 1: self
                else if (w == 2) U16(objects, off, 0xFFFF);
                else U32(objects, off, 0);
            }
            colBase += Count * w;
        }
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);

        // 2. objdata.dat
        int dataLen = 8 + Count * LevelObjectStore.ObjDataWidths.Sum();
        byte[] objdata = new byte[dataLen];
        U32(objdata, 0, 1);
        U32(objdata, 4, Count);
        objdata[8] = 1; // slot 0 active
        File.WriteAllBytes(Path.Combine(_dataDir, "objdata.dat"), objdata);

        // 3. position.dat
        int posLen = 8 + PosCount * 17;
        byte[] positions = new byte[posLen];
        U32(positions, 0, 1);
        U32(positions, 4, PosCount);
        positions[8] = 1; // pos 0 valid
        F32(positions, 9, 10f); F32(positions, 13, 20f); F32(positions, 17, 30f); F32(positions, 21, 0f);
        positions[8 + 17] = 1; // pos 1 valid
        F32(positions, 8 + 18, 10f); F32(positions, 8 + 22, 20f); F32(positions, 8 + 26, 30f); F32(positions, 8 + 30, 0f);
        File.WriteAllBytes(Path.Combine(_dataDir, "position.dat"), positions);

        // 4. anim.dat
        int animLen = 8 + Count * 25;
        byte[] anim = new byte[animLen];
        U32(anim, 0, 1);
        U32(anim, 4, Count);
        anim[8] = 1; // slot 0 active
        File.WriteAllBytes(Path.Combine(_dataDir, "anim.dat"), anim);

        // 5. gfxtype.dat
        int gfxLen = 8 + Count * 17;
        byte[] gfxtype = new byte[gfxLen];
        U32(gfxtype, 0, 1);
        U32(gfxtype, 4, Count);
        gfxtype[8] = 1; // slot 0 active
        U16(gfxtype, 8 + Count * 15, 99); // tail slot 0
        File.WriteAllBytes(Path.Combine(_dataDir, "gfxtype.dat"), gfxtype);

        // 6. action.dat
        int actLen = 12 + Count * 25;
        byte[] action = new byte[actLen];
        U32(action, 0, 1);
        U32(action, 4, Count);
        U32(action, 8, 6);
        action[12] = 1; // slot 0 active
        File.WriteAllBytes(Path.Combine(_dataDir, "action.dat"), action);

        // 7. engine.dat
        byte[] engine = new byte[94];
        U32(engine, 0, 1);
        U32(engine, 34, 200); // Counter = 200 (> UID 100)
        File.WriteAllBytes(Path.Combine(_dataDir, "engine.dat"), engine);

        // 8. hirarchy.dat
        int hirLen = 12 + 2 * 105;
        byte[] hirarchy = new byte[hirLen];
        U32(hirarchy, 0, 1);
        U32(hirarchy, 4, 2);
        U32(hirarchy, 8, 50);
        File.WriteAllBytes(Path.Combine(_dataDir, "hirarchy.dat"), hirarchy);
    }

    [Fact]
    public void Load_detects_all_auxiliary_pools()
    {
        var store = LevelObjectStore.Load(_dir);
        Assert.True(store.HasAnimPool);
        Assert.True(store.HasGfxtypePool);
        Assert.True(store.HasActionPool);
        Assert.True(store.HasEngineData);

        var report = NativePoolDiagnostics.DiagnoseDirectory(_dataDir);
        Assert.True(report.IsHealthy);
        Assert.Empty(report.Issues);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Deleted_object_uid_is_not_reused_after_save_and_optional_reload(bool reload)
    {
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        int first = store.Add(template, 1, 2, 3, 0, team: 3);
        uint uid = store.UidAt(first)!.Value;
        Assert.True(store.RemoveIfUid(first, uid));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        if (reload) store = LevelObjectStore.Load(_dir);
        int second = store.Add(template, 1, 2, 3, 0, team: 8);
        Assert.Equal(first, second);
        Assert.NotEqual(uid, store.UidAt(second));
        Assert.False(store.RemoveIfUid(second, uid));
        Assert.Equal(200u, uid); // persisted next counter, not max active UID + 1
        Assert.Equal(201u, store.UidAt(second));
    }

    [Fact]
    public void Stale_engine_counter_is_advanced_without_rewriting_existing_uids()
    {
        byte[] engine = File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat"));
        U32(engine, 34, 1);
        File.WriteAllBytes(Path.Combine(_dataDir, "engine.dat"), engine);
        var store = LevelObjectStore.Load(_dir);
        int added = store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0);
        Assert.Equal(100u, store.UidAt(0));
        Assert.Equal(101u, store.UidAt(added));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        Assert.Equal(102u, BinaryPrimitives.ReadUInt32LittleEndian(File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat")).AsSpan(34)));
    }

    [Fact]
    public void Uid_wrap_boundary_is_rejected_before_any_pool_changes()
    {
        byte[] engine = File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat"));
        U32(engine, 34, 0x01000000);
        File.WriteAllBytes(Path.Combine(_dataDir, "engine.dat"), engine);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.Throws<InvalidDataException>(() => store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Fact]
    public void Last_nonwrapping_uid_is_allocated_once_and_next_add_is_atomic()
    {
        byte[] engine = File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat"));
        U32(engine, 34, 0x00FFFFFF);
        File.WriteAllBytes(Path.Combine(_dataDir, "engine.dat"), engine);
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        int slot = store.Add(template, 1, 2, 3, 0);
        Assert.Equal(0x00FFFFFFu, store.UidAt(slot));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        Assert.Throws<InvalidDataException>(() => store.Add(template, 1, 2, 3, 0));
        using var secondRollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, secondRollback); secondRollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    private void RelocateAuxiliaryRecords()
    {
        foreach (var (file, header, stride) in new[] { ("anim.dat", 8, 21), ("gfxtype.dat", 8, 15), ("action.dat", 12, 25) })
        {
            string path = Path.Combine(_dataDir, file);
            byte[] bytes = File.ReadAllBytes(path);
            bytes.AsSpan(header, stride).CopyTo(bytes.AsSpan(header + 2 * stride, stride));
            bytes.AsSpan(header, stride).Clear();
            // An occupied unrelated slot whose bytes must survive both operations.
            bytes.AsSpan(header + stride, stride).Fill(0x37);
            if (file == "anim.dat")
            {
                U16(bytes, 8 + Count * 21 + 2 * 2, 0x1234);
                U16(bytes, 8 + Count * 23 + 2 * 2, 0x5678);
            }
            File.WriteAllBytes(path, bytes);
        }
        string dataPath = Path.Combine(_dataDir, "objdata.dat");
        byte[] data = File.ReadAllBytes(dataPath);
        int offset = 8;
        foreach (int width in LevelObjectStore.ObjDataWidths)
        {
            data.AsSpan(offset, width).CopyTo(data.AsSpan(offset + 2 * width, width));
            data.AsSpan(offset, width).Clear();
            data.AsSpan(offset + width, width).Fill(0x37);
            offset += Count * width;
        }
        File.WriteAllBytes(dataPath, data);
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        foreach (int link in new[] { 71, 73, 77 }) U16(objects, 16 + link, 2);
        U16(objects, 16 + Count * 79 + Count * 2, 2); // objdata link, column 1
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
    }

    private static LevelObjectTemplate WithOptionalPools(LevelObjectTemplate source, int mask)
        => new(source.TypeId, source.Record, source.Columns, source.ObjData, source.Position0, source.Position1,
            (mask & 1) != 0 ? source.AnimRecord : null, source.AnimTail0, source.AnimTail1,
            (mask & 2) != 0 ? source.GfxtypeRecord : null, source.GfxtypeTail,
            (mask & 4) != 0 ? source.ActionRecord : null, source.Position2);

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    [InlineData(3)]
    [InlineData(4)]
    [InlineData(5)]
    [InlineData(6)]
    [InlineData(7)]
    public void Null_auxiliary_templates_use_native_sentinel_and_do_not_allocate_unused_pools(int mask)
    {
        var template = WithOptionalPools(Assert.Single(LevelObjectStore.Load(_dir).Templates()), mask);
        var pools = new[] { ("anim.dat", 8, 21, 71), ("gfxtype.dat", 8, 15, 73), ("action.dat", 12, 25, 77) };
        for (int i = 0; i < pools.Length; i++)
            if ((mask & (1 << i)) == 0)
            {
                var (name, header, stride, _) = pools[i];
                byte[] bytes = File.ReadAllBytes(Path.Combine(_dataDir, name));
                for (int s = 0; s < Count; s++) bytes[header + s * stride] = 1;
                File.WriteAllBytes(Path.Combine(_dataDir, name), bytes);
            }
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        int slot = store.Add(template, 1, 2, 3, 0);
        Assert.Equal(1, slot);
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        for (int i = 0; i < pools.Length; i++)
        {
            var (name, _, _, link) = pools[i];
            int target = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + slot * 79 + link));
            if ((mask & (1 << i)) == 0)
            {
                Assert.Equal(0xFFFF, target);
                string path = Path.Combine(_dataDir, name); Assert.Equal(before[path], File.ReadAllBytes(path));
            }
            else Assert.Equal(1, target);
        }
    }

    [Theory]
    [InlineData("anim.dat", false)]
    [InlineData("gfxtype.dat", false)]
    [InlineData("action.dat", false)]
    [InlineData("anim.dat", true)]
    [InlineData("gfxtype.dat", true)]
    [InlineData("action.dat", true)]
    [InlineData("engine.dat", true)]
    public void Required_missing_or_corrupt_destination_pool_rejects_add_before_changes(string file, bool corrupt)
    {
        var template = Assert.Single(LevelObjectStore.Load(_dir).Templates());
        string path = Path.Combine(_dataDir, file);
        if (corrupt) File.WriteAllBytes(path, [1, 0, 0, 0]); else File.Delete(path);
        var before = Directory.GetFiles(_dataDir).ToDictionary(p => p, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        var error = Assert.Throws<InvalidDataException>(() => store.Add(template, 1, 2, 3, 0));
        Assert.Contains(file, error.Message);
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        Assert.Equal(before.Count, Directory.GetFiles(_dataDir).Length);
        foreach (var (p, expected) in before) Assert.Equal(expected, File.ReadAllBytes(p));
    }

    [Fact]
    public void Truncated_engine_is_not_accepted_or_mutated_as_a_complete_pool()
    {
        byte[] engine = File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat"));
        File.WriteAllBytes(Path.Combine(_dataDir, "engine.dat"), engine[..38]);
        var store = LevelObjectStore.Load(_dir);
        Assert.False(store.HasEngineData);
        Assert.False(store.Remove(0));
    }

    [Theory]
    [InlineData("anim.dat")]
    [InlineData("gfxtype.dat")]
    [InlineData("action.dat")]
    [InlineData("engine.dat")]
    public void Corrupt_present_pool_blocks_even_an_all_null_template_and_removal(string file)
    {
        var template = WithOptionalPools(Assert.Single(LevelObjectStore.Load(_dir).Templates()), 0);
        File.WriteAllBytes(Path.Combine(_dataDir, file), [1, 0, 0, 0]);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.Empty(store.Templates());
        Assert.Null(store.OwnedTemplate(0, 100));
        Assert.False(store.Remove(0));
        Assert.Throws<InvalidDataException>(() => store.Add(template, 1, 2, 3, 0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData("anim.dat", 8, 71)]
    [InlineData("gfxtype.dat", 8, 73)]
    [InlineData("action.dat", 12, 77)]
    public void Inactive_required_target_is_not_captured_as_an_optional_null_template(string file, int header, int link)
    {
        byte[] bytes = File.ReadAllBytes(Path.Combine(_dataDir, file)); bytes[header] = 0;
        File.WriteAllBytes(Path.Combine(_dataDir, file), bytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.Empty(store.Templates());
        Assert.Null(store.OwnedTemplate(0, 100));
        Assert.False(store.Remove(0));
        // An explicit native FFFF link has different semantics from an invalid active target.
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat")); U16(objects, 16 + link, 0xFFFF);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        Assert.Single(LevelObjectStore.Load(_dir).Templates());
    }

    [Theory]
    [InlineData(0, 2)]
    [InlineData(1, 3)]
    [InlineData(-1, 3)]
    public void Native_definition_controls_position_allocation_without_hardcoded_types(short afstr, int expected)
    {
        var definitions = new Dictionary<int, NativeObjectDefinition> { [555] = new(555, true, afstr) };
        var store = LevelObjectStore.Load(_dir, definitions);
        int slot = store.Add(Assert.Single(store.Templates()), 50, 60, 70, 1);
        Assert.Equal(1, slot);
        using (var rollback = new FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        int column = 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(10).Sum());
        int third = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(column + slot * 2));
        Assert.Equal(expected == 3, third != 0xFFFF);
        byte[] positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        Assert.Equal(2 + expected, Enumerable.Range(0, PosCount).Count(i => positions[8 + i * 17] != 0));
        if (expected == 3)
        {
            Assert.Equal(50, BinaryPrimitives.ReadSingleLittleEndian(positions.AsSpan(8 + third * 17 + 1)));
            Assert.Equal(60, BinaryPrimitives.ReadSingleLittleEndian(positions.AsSpan(8 + third * 17 + 5)));
            Assert.Equal(70, BinaryPrimitives.ReadSingleLittleEndian(positions.AsSpan(8 + third * 17 + 9)));
        }
        store = LevelObjectStore.Load(_dir, definitions); Assert.True(store.Remove(slot));
        using (var rollback = new FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        Assert.Equal(2, Enumerable.Range(0, PosCount).Count(i => positions[8 + i * 17] != 0));
    }

    [Theory]
    [InlineData("inactive")]
    [InlineData("unknown")]
    [InlineData("unexpected-third")]
    [InlineData("third-capacity")]
    public void Type_position_contract_is_preflighted_without_partial_writes(string fault)
    {
        if (fault == "unexpected-third") SetThirdPosition(0, 4);
        if (fault == "third-capacity")
        {
            byte[] positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
            for (int i = 4; i < PosCount; i++) positions[8 + i * 17] = 1;
            File.WriteAllBytes(Path.Combine(_dataDir, "position.dat"), positions);
        }
        var before = Directory.GetFiles(_dataDir).ToDictionary(p => p, File.ReadAllBytes);
        var definitions = new Dictionary<int, NativeObjectDefinition> { [fault == "unknown" ? 556 : 555] = new(555, fault != "inactive", (short)(fault == "third-capacity" ? 1 : 0)) };
        var store = LevelObjectStore.Load(_dir, definitions); var template = Assert.Single(store.Templates());
        if (fault == "third-capacity") Assert.Equal(-1, store.Add(template, 1, 2, 3, 0));
        else Assert.Throws<InvalidDataException>(() => store.Add(template, 1, 2, 3, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
    }

    private void SetThirdPosition(int objectSlot, ushort position, bool active = true)
    {
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        int column = 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(10).Sum());
        U16(objects, column + objectSlot * 2, position);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        if (position >= PosCount) return;
        byte[] positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        positions[8 + position * 17] = active ? (byte)1 : (byte)0;
        File.WriteAllBytes(Path.Combine(_dataDir, "position.dat"), positions);
    }

    [Fact]
    public void Three_position_template_roundtrips_with_independent_slots_and_releases_all_three()
    {
        SetThirdPosition(0, 4);
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        int slot = store.Add(template, 50, 60, 70, 1);
        Assert.Equal(1, slot);
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        int thirdColumn = 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(10).Sum());
        int third = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(thirdColumn + slot * 2));
        int[] links = [BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + slot * 79 + 67)),
            BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + slot * 79 + 69)), third];
        Assert.Equal(new[] { 2, 3, 5 }, links);
        byte[] positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        foreach (int link in links)
        {
            Assert.Equal(1, positions[8 + link * 17]);
            Assert.Equal(50f, BinaryPrimitives.ReadSingleLittleEndian(positions.AsSpan(9 + link * 17)));
        }
        Assert.True(NativePoolDiagnostics.DiagnoseDirectory(_dataDir).IsHealthy);
        store = LevelObjectStore.Load(_dir);
        Assert.NotNull(store.OwnedTemplate(slot, store.UidAt(slot)!.Value));
        Assert.True(store.Remove(slot));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        foreach (int link in links) Assert.Equal(0, positions[8 + link * 17]);
        Assert.Equal(1, positions[8 + 4 * 17]); // original third position preserved
    }

    [Fact]
    public void Position_allocator_uses_fragmented_free_slots_without_requiring_even_pairs()
    {
        byte[] positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        for (int i = 0; i < PosCount; i++) positions[8 + i * 17] = 1;
        positions[8 + 3 * 17] = positions[8 + 6 * 17] = 0;
        File.WriteAllBytes(Path.Combine(_dataDir, "position.dat"), positions);
        var store = LevelObjectStore.Load(_dir);
        Assert.Equal(1, store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0));
    }

    [Fact]
    public void Inactive_position_referenced_as_third_is_not_reallocated()
    {
        // Capture another valid source before introducing the broken third link.
        var template = Assert.Single(LevelObjectStore.Load(_dir).Templates());
        SetThirdPosition(0, 2, active: false);
        var store = LevelObjectStore.Load(_dir);
        int slot = store.Add(template, 1, 2, 3, 0);
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        Assert.NotEqual(2, BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + slot * 79 + 67)));
        Assert.NotEqual(2, BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + slot * 79 + 69)));
    }

    [Fact]
    public void Removal_refuses_a_primary_position_shared_with_another_objects_third()
    {
        var store = LevelObjectStore.Load(_dir);
        int slot = store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0);
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        SetThirdPosition(slot, 0);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        store = LevelObjectStore.Load(_dir);
        Assert.False(store.Remove(0));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Fact]
    public void Third_position_capacity_is_preflighted_before_any_pool_changes()
    {
        SetThirdPosition(0, 4);
        byte[] positions = File.ReadAllBytes(Path.Combine(_dataDir, "position.dat"));
        for (int i = 0; i < PosCount; i++) positions[8 + i * 17] = 1;
        positions[8 + 2 * 17] = positions[8 + 3 * 17] = 0;
        File.WriteAllBytes(Path.Combine(_dataDir, "position.dat"), positions);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.Equal(-1, store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData(2)]
    [InlineData(3)]
    [InlineData(4)]
    [InlineData(5)]
    [InlineData(8)]
    [InlineData(11)]
    [InlineData(13)]
    public void Native_reference_to_unmanaged_pool_protects_object_from_copy_and_remove(int referenceColumn)
    {
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        U16(objects, 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(referenceColumn).Sum()), 0);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.True(Assert.Single(store.Objects()).Linked);
        Assert.Empty(store.Templates());
        Assert.Null(store.OwnedTemplate(0, 100));
        Assert.False(store.Remove(0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData(1, "object-parent-inactive")]
    [InlineData(4, "object-parent-out-of-range")]
    [InlineData(0, "object-parent-self")]
    public void Invalid_parent_is_diagnosed_and_child_cannot_be_removed(int parent, string code)
    {
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        U16(objects, 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(13).Sum()), (ushort)parent);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        var report = NativePoolDiagnostics.DiagnoseDirectory(_dataDir);
        Assert.Contains(report.Issues, issue => issue.Code == code);
        Assert.False(LevelObjectStore.Load(_dir).Remove(0));
    }

    [Fact]
    public void Parent_and_child_are_protected_and_cached_child_template_cannot_be_copied()
    {
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 100, 0, 100, 0));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        byte[] objects = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat")));
        U16(objects, 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(13).Sum()) + 2, 0);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        store = LevelObjectStore.Load(_dir);
        Assert.All(store.Objects(), item => Assert.True(item.Linked));
        Assert.Empty(store.Templates());
        Assert.Null(store.OwnedTemplate(0, 100));
        Assert.False(store.Remove(0)); Assert.False(store.Remove(1));
        template.Columns[13] = 0;
        Assert.Throws<InvalidDataException>(() => store.Add(template, 200, 0, 200, 0));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(GameLZSS.DecompressPfil(bytes), GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        Assert.DoesNotContain(NativePoolDiagnostics.DiagnoseDirectory(_dataDir).Issues, issue => issue.Code.StartsWith("object-parent-", StringComparison.Ordinal));
    }

    [Fact]
    public void Dangling_parent_target_is_not_reused_until_blank_map_reset()
    {
        var template = Assert.Single(LevelObjectStore.Load(_dir).Templates());
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        U16(objects, 16 + Count * (79 + LevelObjectStore.ColumnWidths.Take(13).Sum()), 1);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        var store = LevelObjectStore.Load(_dir);
        Assert.Equal(2, store.Add(template, 100, 0, 100, 0));
        store.ResetForBlankMap();
        Assert.Equal(0, store.Add(template, 200, 0, 200, 0));
        Assert.Equal(1, store.Add(template, 300, 0, 300, 0));
    }

    [Fact]
    public void Installed_native_children_and_parents_cannot_be_deleted_independently()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game)) return;
        string source = Path.Combine(game, "MAPS", "HIST_004", "DATA");
        var hashes = new Dictionary<string, byte[]>();
        foreach (string file in new[] { "objects.dat", "position.dat", "objdata.dat", "anim.dat", "gfxtype.dat", "action.dat", "engine.dat", "hirarchy.dat" })
        {
            string path = Path.Combine(source, file);
            hashes[path] = SHA256.HashData(File.ReadAllBytes(path));
            File.Copy(path, Path.Combine(_dataDir, file), true);
        }
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, path => GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        byte[] objects = before[Path.Combine(_dataDir, "objects.dat")];
        int count = BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(4));
        int parentOffset = 16 + count * (79 + LevelObjectStore.ColumnWidths.Take(13).Sum());
        int child = Enumerable.Range(0, count).First(slot => objects[16 + slot * 79] != 0
            && BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(parentOffset + slot * 2)) != 0xFFFF);
        int parent = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(parentOffset + child * 2));
        var store = LevelObjectStore.Load(_dir, NativeObjectDefinitions.Load(game));
        var items = store.Objects().ToDictionary(item => item.Slot);
        Assert.True(items[child].Linked); Assert.True(items[parent].Linked);
        Assert.False(store.Remove(child)); Assert.False(store.Remove(parent));
        Assert.Null(store.OwnedTemplate(child, items[child].Uid));
        Assert.Null(store.OwnedTemplate(parent, items[parent].Uid));
        Assert.DoesNotContain(NativePoolDiagnostics.DiagnoseDirectory(_dataDir).Issues,
            issue => issue.Code.StartsWith("object-parent-", StringComparison.Ordinal));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(bytes, GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        foreach (var (path, hash) in hashes) Assert.Equal(hash, SHA256.HashData(File.ReadAllBytes(path)));
    }

    [Fact]
    public void Nonzero_objdata_float_scalar_does_not_make_object_linked()
    {
        byte[] data = File.ReadAllBytes(Path.Combine(_dataDir, "objdata.dat"));
        F32(data, 8 + Count * (9 + 8), .35f);
        File.WriteAllBytes(Path.Combine(_dataDir, "objdata.dat"), data);
        var store = LevelObjectStore.Load(_dir);
        Assert.False(Assert.Single(store.Objects()).Linked);
        Assert.Single(store.Templates());
        Assert.NotNull(store.OwnedTemplate(0, 100));
    }

    [Fact]
    public void Missing_third_position_template_bytes_are_rejected_before_mutation()
    {
        var source = Assert.Single(LevelObjectStore.Load(_dir).Templates());
        source.Columns[10] = 4;
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.Throws<InvalidDataException>(() => store.Add(source, 1, 2, 3, 0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Fact]
    public void Add_allocates_each_pool_independently_and_copies_referenced_template()
    {
        RelocateAuxiliaryRecords();
        var original = Directory.GetFiles(_dataDir).ToDictionary(path => Path.GetFileName(path)!, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 50, 60, 70, 1));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        foreach (int link in new[] { 71, 73, 77 })
            Assert.Equal(0, BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + 79 + link)));
        Assert.Equal(0, BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + Count * 79 + Count * 2 + 2)));
        foreach (var (file, header, stride) in new[] { ("anim.dat", 8, 21), ("gfxtype.dat", 8, 15), ("action.dat", 12, 25) })
        {
            byte[] bytes = File.ReadAllBytes(Path.Combine(_dataDir, file));
            Assert.Equal(original[file].AsSpan(header + stride, 2 * stride).ToArray(), bytes.AsSpan(header + stride, 2 * stride).ToArray());
            Assert.Equal(original[file].AsSpan(header + 2 * stride, stride).ToArray(), bytes.AsSpan(header, stride).ToArray());
        }
        byte[] anim = File.ReadAllBytes(Path.Combine(_dataDir, "anim.dat"));
        Assert.Equal(0x1234, BinaryPrimitives.ReadUInt16LittleEndian(anim.AsSpan(8 + Count * 21)));
        Assert.Equal(0x5678, BinaryPrimitives.ReadUInt16LittleEndian(anim.AsSpan(8 + Count * 23)));
        Assert.Equal(2, LevelObjectStore.Load(_dir).Objects().Count);
    }

    [Fact]
    public void Remove_releases_referenced_pool_slots_and_preserves_unrelated_slots()
    {
        RelocateAuxiliaryRecords();
        var store = LevelObjectStore.Load(_dir);
        Assert.True(store.Remove(0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (file, header, stride) in new[] { ("anim.dat", 8, 21), ("gfxtype.dat", 8, 15), ("action.dat", 12, 25) })
        {
            byte[] bytes = File.ReadAllBytes(Path.Combine(_dataDir, file));
            Assert.Equal(0, bytes[header + 2 * stride]);
            Assert.All(bytes.AsSpan(header + stride, stride).ToArray(), b => Assert.Equal(0x37, b));
        }
        byte[] data = File.ReadAllBytes(Path.Combine(_dataDir, "objdata.dat"));
        Assert.Equal(0, data[8 + 2 * 9]);
        Assert.Equal(0x37, data[8 + 9]);
    }

    [Theory]
    [InlineData("anim.dat", 8, 21)]
    [InlineData("gfxtype.dat", 8, 15)]
    [InlineData("action.dat", 12, 25)]
    [InlineData("objdata.dat", 8, 9)]
    public void Add_with_exhausted_related_pool_leaves_all_files_unchanged(string file, int header, int stride)
    {
        byte[] bytes = File.ReadAllBytes(Path.Combine(_dataDir, file));
        for (int slot = 0; slot < Count; slot++) bytes[header + slot * stride] = 1;
        File.WriteAllBytes(Path.Combine(_dataDir, file), bytes);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        var store = LevelObjectStore.Load(_dir);
        Assert.Equal(-1, store.Add(Assert.Single(store.Templates()), 50, 60, 70, 1));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Fact]
    public void Shared_related_slot_refuses_removal_without_partial_changes()
    {
        var store = LevelObjectStore.Load(_dir);
        Assert.Equal(1, store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        U16(objects, 16 + 79 + 71, 0); // both objects reference anim slot 0
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        store = LevelObjectStore.Load(_dir);
        Assert.False(store.Remove(0));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Fact]
    public void Malformed_template_is_rejected_before_any_pool_is_written()
    {
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        var malformed = new LevelObjectTemplate(template.TypeId, template.Record, template.Columns,
            template.ObjData, template.Position0, template.Position1, actionRecord: new byte[26]);
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        Assert.Throws<InvalidDataException>(() => store.Add(malformed, 1, 2, 3, 0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, expected) in before) Assert.Equal(expected, File.ReadAllBytes(path));
    }

    [Fact]
    public void Prebuilt_update_rejects_shared_owned_target_instead_of_leaving_a_duplicate()
    {
        var store = LevelObjectStore.Load(_dir);
        Assert.Equal(1, store.Add(Assert.Single(store.Templates()), 1, 2, 3, 0));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        byte[] objects = File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat"));
        U16(objects, 16 + 79 + 71, 0);
        File.WriteAllBytes(Path.Combine(_dataDir, "objects.dat"), objects);
        store = LevelObjectStore.Load(_dir);
        var previous = new ScenarioDocument { DataSlots = [new(0, store.UidAt(0)!.Value)] };
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, previous, new(), _ => null));
        Assert.Equal(2, store.Objects().Count);
    }

    [Theory]
    [InlineData(float.NaN)]
    [InlineData(float.PositiveInfinity)]
    [InlineData(float.NegativeInfinity)]
    public void Nonfinite_creation_moves_are_rejected_before_any_pool_changes(float moves)
    {
        var definitions = new Dictionary<int, NativeObjectDefinition> { [555] = new(555, true, 0, 321, 789, moves, -1234567) };
        var store = LevelObjectStore.Load(_dir, definitions);
        var template = Assert.Single(store.Templates());
        template.InitializeDefinitionVitals = true;
        var before = Directory.GetFiles(_dataDir).ToDictionary(path => path, File.ReadAllBytes);
        Assert.Throws<InvalidDataException>(() => store.Add(template, 100, 0, 100, 0));
        using var rollback = new AgainstRomeModifier.FileRollbackScope(); store.Save(_dir, rollback); rollback.Commit();
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
    }

    [Fact]
    public void Official_new_placement_uses_current_definition_vitals_but_owned_copy_preserves_them()
    {
        string game = Path.Combine(_dir, "GAME");
        string source = Path.Combine(game, "MAPS", "SOURCE", "DATA");
        Directory.CreateDirectory(source);
        foreach (string file in Directory.GetFiles(_dataDir)) File.Copy(file, Path.Combine(source, Path.GetFileName(file)));
        var sourceHashes = Directory.GetFiles(source).ToDictionary(path => path, File.ReadAllBytes);
        var catalog = LevelObjectStore.LoadOfficialTemplates(game, id => id == 555);
        Assert.Single(catalog);
        byte[][] originalTemplate = catalog[555].ObjData.Select(bytes => bytes.ToArray()).ToArray();
        // Definitions change after the palette cache is created; save must use the current catalog.
        string defaults = Path.Combine(game, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(defaults);
        string[] fields = Enumerable.Repeat("0", 153).ToArray();
        fields[0] = "555"; fields[1] = "1"; fields[19] = "321"; fields[21] = "789"; fields[4] = "1.75"; fields[152] = "-1234567"; fields[54] = "900";
        File.WriteAllText(Path.Combine(defaults, "objdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
        File.WriteAllText(Path.Combine(defaults, "typus.dau"), "[TypusDefaults]\n900,1,other-index,400\n");
        var definitions = NativeObjectDefinitions.Load(game);
        var store = LevelObjectStore.Load(_dir, definitions);
        int slot = store.Add(catalog[555], 100, 0, 100, 0);
        Assert.True(slot >= 0);
        var created = store.OwnedTemplate(slot, store.UidAt(slot)!.Value)!;
        Assert.NotNull(created);
        Assert.Equal(321f, BinaryPrimitives.ReadSingleLittleEndian(created.ObjData[0].AsSpan(1)));
        Assert.Equal(1f, BinaryPrimitives.ReadSingleLittleEndian(created.ObjData[0].AsSpan(5)));
        Assert.Equal(789f, BinaryPrimitives.ReadSingleLittleEndian(created.ObjData[1]));
        Assert.Equal(1f, BinaryPrimitives.ReadSingleLittleEndian(created.ObjData[1].AsSpan(4)));
        Assert.Equal(1.75f, BinaryPrimitives.ReadSingleLittleEndian(created.ObjData[2]));
        Assert.Equal(-1234567, BinaryPrimitives.ReadInt32LittleEndian(created.ObjData[16]));
        Assert.Equal((ushort)100, BinaryPrimitives.ReadUInt16LittleEndian(created.ObjData[4]));
        for (int segment = 2; segment < originalTemplate.Length; segment++)
            if (segment is not 2 and not 4 and not 16) Assert.Equal(originalTemplate[segment], created.ObjData[segment]);
        for (int segment = 0; segment < originalTemplate.Length; segment++) Assert.Equal(originalTemplate[segment], catalog[555].ObjData[segment]);
        using (var rollback = new AgainstRomeModifier.FileRollbackScope()) { store.Save(_dir, rollback); rollback.Commit(); }
        var changed = definitions.ToDictionary(pair => pair.Key, pair => pair.Value with { InitialHp = 999, InitialMorale = 888, InitialMoves = .5f, InitialMaxRuhm = 98765, InitialField1c = 0 });
        store = LevelObjectStore.Load(_dir, changed);
        int copy = store.Add(created, 200, 0, 200, 0);
        Assert.True(copy >= 0);
        var restored = store.OwnedTemplate(copy, store.UidAt(copy)!.Value)!;
        Assert.Equal(created.ObjData, restored.ObjData);
        foreach (var (path, bytes) in sourceHashes) Assert.Equal(bytes, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Installed_native_map_copy_roundtrips_add_remove_without_writing_originals(bool threePositions)
    {
        string? game = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(game)) return; // Opt-in asset acceptance, separate from synthetic coverage.
        string source = Path.Combine(game, "MAPS", "ENDL_000", "DATA");
        var sourceHashes = new Dictionary<string, byte[]>();
        foreach (string file in new[] { "objects.dat", "position.dat", "objdata.dat", "anim.dat", "gfxtype.dat", "action.dat", "engine.dat", "hirarchy.dat" })
        {
            string path = Path.Combine(source, file);
            Assert.True(File.Exists(path), path);
            sourceHashes[path] = SHA256.HashData(File.ReadAllBytes(path));
            File.Copy(path, Path.Combine(_dataDir, file), overwrite: true);
        }
        var definitions = NativeObjectDefinitions.Load(game);
        Assert.NotEmpty(definitions);
        var before = NativePoolDiagnostics.DiagnoseDirectory(_dataDir, definitions);
        Assert.True(before.IsHealthy, string.Join("; ", before.Issues.Select(i => i.Message)));
        var store = LevelObjectStore.Load(_dir, definitions);
        var template = store.Templates().First(t => (t.Position2 is not null) == threePositions);
        Assert.Equal(threePositions, definitions[template.TypeId].RequiresThirdPosition);
        if (threePositions)
        {
            uint[] columns = template.Columns.ToArray(); columns[10] = 0xFFFF;
            template = new(template.TypeId, template.Record, columns, template.ObjData, template.Position0, template.Position1,
                template.AnimRecord, template.AnimTail0, template.AnimTail1, template.GfxtypeRecord, template.GfxtypeTail, template.ActionRecord);
        }
        template.InitializeDefinitionVitals = true;
        int slot = store.Add(template, 8192, 0, 8192, 0);
        Assert.True(slot >= 0);
        uint createdUid = store.UidAt(slot)!.Value;
        Assert.Equal(before.EngineUidCounter, createdUid);
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        byte[] createdObjects = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(_dataDir, "objects.dat")));
        int objectCount = BinaryPrimitives.ReadInt32LittleEndian(createdObjects.AsSpan(4));
        int thirdColumn = 16 + objectCount * (79 + LevelObjectStore.ColumnWidths.Take(10).Sum());
        int third = BinaryPrimitives.ReadUInt16LittleEndian(createdObjects.AsSpan(thirdColumn + slot * 2));
        Assert.Equal(threePositions, third != 0xFFFF);
        var added = NativePoolDiagnostics.DiagnoseDirectory(_dataDir, definitions);
        Assert.True(added.IsHealthy, string.Join("; ", added.Issues.Select(i => i.Message)));
        Assert.Equal(before.ActiveObjects + 1, added.ActiveObjects);
        store = LevelObjectStore.Load(_dir, definitions);
        var savedTemplate = store.OwnedTemplate(slot, createdUid)!;
        Assert.NotNull(savedTemplate);
        Assert.Equal((float)definitions[template.TypeId].InitialHp!.Value, BinaryPrimitives.ReadSingleLittleEndian(savedTemplate.ObjData[0].AsSpan(1)));
        Assert.Equal(1f, BinaryPrimitives.ReadSingleLittleEndian(savedTemplate.ObjData[0].AsSpan(5)));
        Assert.Equal((float)definitions[template.TypeId].InitialMorale!.Value, BinaryPrimitives.ReadSingleLittleEndian(savedTemplate.ObjData[1]));
        Assert.Equal(1f, BinaryPrimitives.ReadSingleLittleEndian(savedTemplate.ObjData[1].AsSpan(4)));
        Assert.Equal(definitions[template.TypeId].InitialMoves!.Value, BinaryPrimitives.ReadSingleLittleEndian(savedTemplate.ObjData[2]));
        Assert.Equal(definitions[template.TypeId].InitialMaxRuhm!.Value, BinaryPrimitives.ReadInt32LittleEndian(savedTemplate.ObjData[16]));
        Assert.Equal(definitions[template.TypeId].InitialField1c!.Value, BinaryPrimitives.ReadUInt16LittleEndian(savedTemplate.ObjData[4]));
        Assert.True(store.Remove(slot));
        using (var rollback = new AgainstRomeModifier.FileRollbackScope())
        { store.Save(_dir, rollback); rollback.Commit(); }
        var removed = NativePoolDiagnostics.DiagnoseDirectory(_dataDir, definitions);
        Assert.True(removed.IsHealthy, string.Join("; ", removed.Issues.Select(i => i.Message)));
        Assert.Equal(before.ActiveObjects, removed.ActiveObjects);
        Assert.Equal(before.ActiveAnimSlots, removed.ActiveAnimSlots);
        Assert.Equal(before.ActiveGfxtypeSlots, removed.ActiveGfxtypeSlots);
        Assert.Equal(before.ActiveActionSlots, removed.ActiveActionSlots);
        store = LevelObjectStore.Load(_dir, definitions);
        int replacement = store.Add(template, 8192, 0, 8192, 0);
        Assert.Equal(slot, replacement);
        Assert.Equal(createdUid + 1, store.UidAt(replacement));
        Assert.False(store.RemoveIfUid(replacement, createdUid));
        foreach (var (path, expected) in sourceHashes) Assert.Equal(expected, SHA256.HashData(File.ReadAllBytes(path)));
    }

    [Fact]
    public void Add_synchronizes_anim_gfxtype_action_and_engine_counter()
    {
        var store = LevelObjectStore.Load(_dir);
        var template = Assert.Single(store.Templates());
        Assert.NotNull(template.AnimRecord);
        Assert.NotNull(template.GfxtypeRecord);
        Assert.NotNull(template.ActionRecord);

        // Add to slot 1
        int newSlot = store.Add(template, 50f, 60f, 70f, 1.5f, team: 3);
        Assert.Equal(1, newSlot);

        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback);
        rollback.Commit();

        // Check on disk that all 7 pools are updated and diagnostics pass
        var report = NativePoolDiagnostics.DiagnoseDirectory(_dataDir);
        Assert.True(report.IsHealthy, string.Join("; ", report.Issues.Select(i => i.Message)));
        Assert.Equal(2, report.ActiveObjects);
        Assert.Equal(2, report.ActiveAnimSlots);
        Assert.Equal(2, report.ActiveGfxtypeSlots);
        Assert.Equal(2, report.ActiveActionSlots);

        // Verify disk contents directly
        byte[] anim = File.ReadAllBytes(Path.Combine(_dataDir, "anim.dat"));
        Assert.Equal(1, anim[8 + 1 * 21]); // slot 1 active!

        byte[] gfxtype = File.ReadAllBytes(Path.Combine(_dataDir, "gfxtype.dat"));
        Assert.Equal(1, gfxtype[8 + 1 * 15]); // slot 1 active!

        byte[] action = File.ReadAllBytes(Path.Combine(_dataDir, "action.dat"));
        Assert.Equal(1, action[12 + 1 * 25]); // slot 1 active!

        byte[] engine = File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat"));
        uint engineCounter = BinaryPrimitives.ReadUInt32LittleEndian(engine.AsSpan(34));
        Assert.True(engineCounter > 101); // Counter >= 200
    }

    [Fact]
    public void Remove_cleans_anim_gfxtype_and_action_slots()
    {
        var store = LevelObjectStore.Load(_dir);
        Assert.True(store.Remove(0));

        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback);
        rollback.Commit();

        var report = NativePoolDiagnostics.DiagnoseDirectory(_dataDir);
        Assert.True(report.IsHealthy, string.Join("; ", report.Issues.Select(i => i.Message)));
        Assert.Equal(0, report.ActiveObjects);
        Assert.Equal(0, report.ActiveAnimSlots);
        Assert.Equal(0, report.ActiveGfxtypeSlots);
        Assert.Equal(0, report.ActiveActionSlots);

        byte[] anim = File.ReadAllBytes(Path.Combine(_dataDir, "anim.dat"));
        Assert.Equal(0, anim[8]); // slot 0 deactivated!

        byte[] gfxtype = File.ReadAllBytes(Path.Combine(_dataDir, "gfxtype.dat"));
        Assert.Equal(0, gfxtype[8]); // slot 0 deactivated!

        byte[] action = File.ReadAllBytes(Path.Combine(_dataDir, "action.dat"));
        Assert.Equal(0, action[12]); // slot 0 deactivated!
    }

    [Fact]
    public void ResetForBlankMap_clears_all_pools_and_resets_engine_counter()
    {
        var store = LevelObjectStore.Load(_dir);
        store.ResetForBlankMap();

        using var rollback = new AgainstRomeModifier.FileRollbackScope();
        store.Save(_dir, rollback);
        rollback.Commit();

        var report = NativePoolDiagnostics.DiagnoseDirectory(_dataDir);
        Assert.True(report.IsHealthy, string.Join("; ", report.Issues.Select(i => i.Message)));
        Assert.Equal(0, report.ActiveObjects);
        Assert.Equal(0, report.ActiveAnimSlots);
        Assert.Equal(0, report.ActiveGfxtypeSlots);
        Assert.Equal(0, report.ActiveActionSlots);

        byte[] engine = File.ReadAllBytes(Path.Combine(_dataDir, "engine.dat"));
        uint engineCounter = BinaryPrimitives.ReadUInt32LittleEndian(engine.AsSpan(34));
        Assert.Equal(1u, engineCounter);
    }
}
