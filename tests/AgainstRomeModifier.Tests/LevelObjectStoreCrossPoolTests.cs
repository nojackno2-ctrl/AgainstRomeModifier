using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

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
