using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Saved_family_copy_survives_failed_save_retry_and_reload_with_full_disk_and_memory_invariants()
    {
        string map = CreateFixture("ENDL_012");
        const int count = 10;
        SetupSyntheticFamilyWorldStore(map, count);

        // 預留 previous Scenario root Guid 與 DataSlot 綁定
        Guid sourceGuid = Guid.NewGuid();
        var initialScenario = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", 100f, 200f, 0, Angle: 0, Y: 20f, Prebuilt: true) { Id = sourceGuid }],
            DataSlots = [new ScenarioDataSlot(0, 100u) { SpawnId = sourceGuid }],
            // Prebuilt-only placements need no BCI. An enabled message event
            // forces script injection after DATA/Scenario writes for late rollback.
            Events = [new ScenarioEvent("FamilyCopyRetry", 5)
            { Actions = [new(ScenarioActionKind.Message, "Copied")] }]
        };
        using (var rollback = new FileRollbackScope()) { initialScenario.Save(map, rollback); rollback.Commit(); }

        var beforeDisk = SnapshotDirectory(map);

        RunInSta(() =>
        {
            var buildingType = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "BauTest" });

            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "SavedFamilyCopy", "Test"));
            _ = form.Handle;

            // 注入真實 catalog SdlObjectType，使 LoadSelectedMap 能將 previous Scenario 載入為 UI placed object
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { buildingType });
            Invoke(form, "LoadSelectedMap");

            Assert.Single(form.PlacementSession.Capture());
            Assert.Equal(sourceGuid, form.PlacementSession[0].ScenarioId);
            Assert.Equal(Guid.Empty, form.PlacementSession[0].CopySourceScenarioId);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(form.PlacementSession.IsDirty);

            // 經由真實 UI 入口（_placedList 選取 + _placedDuplicateButton / DuplicateSelectedPlacedObjects）觸發複製
            _ = form.PlacedList.Handle;
            Assert.Single(form.PlacedList.Items);
            form.PlacedList.Items[0].Selected = true;
            Assert.Single(form.PlacedList.SelectedItems);

            // Headless STA fixture does not show the button's parent tab. Invoke the
            // same editor command wired to Click so visibility cannot suppress it.
            form.DuplicateSelectedPlacedObjects();

            Assert.Equal(2, form.PlacementSession.Count);
            Assert.Equal(2, form.PlacedList.Items.Count);
            Assert.True(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanUndo);
            Assert.True(GetProperty<bool>(form, "IsDirty"));

            var placedAfterDup = form.PlacementSession.Capture();
            SdlPlacedObject sourcePlaced = placedAfterDup[0];
            SdlPlacedObject copiedPlaced = placedAfterDup[1];

            Assert.Equal(sourceGuid, sourcePlaced.ScenarioId);
            Assert.Equal(Guid.Empty, sourcePlaced.CopySourceScenarioId);

            Guid newScenarioId = copiedPlaced.ScenarioId;
            Assert.NotEqual(Guid.Empty, newScenarioId);
            Assert.NotEqual(sourceGuid, newScenarioId);
            Assert.Equal(sourceGuid, copiedPlaced.CopySourceScenarioId);

            // 驗證按既有 DuplicateMany 預設 256f 偏移
            Assert.Equal(sourcePlaced.WorldX + 256f, copiedPlaced.WorldX);
            Assert.Equal(sourcePlaced.WorldZ + 256f, copiedPlaced.WorldZ);
            Assert.Equal(sourcePlaced.Team, copiedPlaced.Team);
            Assert.Equal(sourcePlaced.Angle, copiedPlaced.Angle);

            // 驗證 Undo / Redo 保留同 copyID 與 sourceGuid
            Assert.True(form.PlacementSession.Undo());
            Assert.Single(form.PlacementSession.Capture());
            Assert.False(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanRedo);
            Assert.Equal(sourceGuid, form.PlacementSession[0].ScenarioId);
            Assert.Equal(Guid.Empty, form.PlacementSession[0].CopySourceScenarioId);

            Assert.True(form.PlacementSession.Redo());
            Assert.Equal(2, form.PlacementSession.Count);
            Assert.True(form.PlacementSession.IsDirty);
            Assert.Equal(newScenarioId, form.PlacementSession[1].ScenarioId);
            Assert.Equal(sourceGuid, form.PlacementSession[1].CopySourceScenarioId);

            // 1. 第一次 TrySaveMap：缺少 script BCI 使其在 nativeStore.Save 之後 late-fail 回滾
            Assert.False(form.TrySaveMap(showSuccess: false, out Exception? error));
            Assert.NotNull(error);
            Assert.IsType<FileNotFoundException>(error);

            // 所有磁碟 snapshot unchanged（包括 Scenario 與 DATA）
            AssertSnapshotUnchanged(map, beforeDisk);

            // UI dirty / undo / provenance 保留
            Assert.True(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanUndo);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.Equal(2, form.PlacementSession.Count);
            Assert.Equal(newScenarioId, form.PlacementSession[1].ScenarioId);
            Assert.Equal(sourceGuid, form.PlacementSession[1].CopySourceScenarioId);

            // 2. 補有效 SCRIPT BCI 後重試成功
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());

            Assert.True(form.TrySaveMap(showSuccess: false, out error), error?.ToString());
            Assert.Null(error);

            // 成功後 provenance 清空、dirty 清除、CanUndo false
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(form.PlacementSession.IsDirty);
            Assert.False(form.PlacementSession.CanUndo);
            var placedAfterSave = form.PlacementSession.Capture();
            Assert.Equal(2, placedAfterSave.Count);
            Assert.Equal(Guid.Empty, placedAfterSave[0].CopySourceScenarioId);
            Assert.Equal(Guid.Empty, placedAfterSave[1].CopySourceScenarioId);

            // 驗證重讀 DATA 新舊 family valid
            var reloadedStore = LevelObjectStore.Load(map);
            var sourceFamily = reloadedStore.FamilyAt(0, 100u);
            Assert.NotNull(sourceFamily);
            Assert.Equal(2, sourceFamily.ObjectSlots.Count); // root (0) + child (1)
            Assert.Single(sourceFamily.LightSlots);  // direct light (0)

            // 驗證 Scenario 儲存結果：只新增 root binding，不為每 child 增加 binding
            var savedScenario = ScenarioDocument.Load(map);
            Assert.Equal(2, savedScenario.Spawns.Count);
            Assert.Contains(savedScenario.Spawns, s => s.Id == sourceGuid && s.Prebuilt);
            Assert.Contains(savedScenario.Spawns, s => s.Id == newScenarioId && s.Prebuilt);

            Assert.Equal(2, savedScenario.DataSlots.Count);
            var sourceSlotBinding = savedScenario.DataSlots.Single(b => b.SpawnId == sourceGuid);
            Assert.Equal(0, sourceSlotBinding.Slot);
            Assert.Equal(100u, sourceSlotBinding.Uid);

            var copiedSlotBinding = savedScenario.DataSlots.Single(b => b.SpawnId == newScenarioId);
            int newRootSlot = copiedSlotBinding.Slot;
            uint newRootUid = copiedSlotBinding.Uid;

            Assert.NotEqual(0, newRootSlot);
            Assert.NotEqual(100u, newRootUid);

            // 驗證 copied family 有效且完整性
            var copiedFamily = reloadedStore.FamilyAt(newRootSlot, newRootUid);
            Assert.NotNull(copiedFamily);
            Assert.Equal(sourceFamily.ObjectSlots.Count, copiedFamily.ObjectSlots.Count);
            Assert.Equal(sourceFamily.LightSlots.Count, copiedFamily.LightSlots.Count);

            // 驗證 source objects 所有 fields 未變
            var sourceRootObj = reloadedStore.Objects().Single(o => o.Slot == 0);
            Assert.Equal(100u, sourceRootObj.Uid);
            Assert.Equal(100f, sourceRootObj.X);
            Assert.Equal(20f, sourceRootObj.Y);
            Assert.Equal(200f, sourceRootObj.Z);

            var sourceChildObj = reloadedStore.Objects().Single(o => o.Slot == 1);
            Assert.Equal(101u, sourceChildObj.Uid);
            Assert.Equal(110f, sourceChildObj.X);
            Assert.Equal(20f, sourceChildObj.Y);
            Assert.Equal(210f, sourceChildObj.Z);

            // 驗證 copied root 位移符合 DuplicateMany 256f
            var copiedRootObj = reloadedStore.Objects().Single(o => o.Slot == newRootSlot);
            Assert.Equal(sourceRootObj.X + 256f, copiedRootObj.X);
            Assert.Equal(sourceRootObj.Y, copiedRootObj.Y);
            Assert.Equal(sourceRootObj.Z + 256f, copiedRootObj.Z);

            // Verify supported native diagnostics; file presence does not
            // validate the opaque formats or prove game loading.
            var diagnostics = form.RefreshMapDiagnostics();
            Assert.DoesNotContain(diagnostics, d => d.Code.StartsWith("native-data-", StringComparison.Ordinal));

            // 3. 再 Save 不重複分配 IDs 或增 objects（byte idempotence）
            var diskAfterSuccess = SnapshotDirectory(map);
            Assert.True(form.TrySaveMap(showSuccess: false, out error), error?.ToString());
            Assert.Null(error);
            AssertSnapshotUnchanged(map, diskAfterSuccess);

            // 4. Reload UI：驗證 source/copied Guid 與預建狀態保持
            using var reloadedForm = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "SavedFamilyCopyReloaded", "Test"));
            _ = reloadedForm.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloadedForm, new[] { buildingType });
            Invoke(reloadedForm, "LoadSelectedMap");

            var reloadedPlaced = reloadedForm.PlacementSession.Capture();
            Assert.Equal(2, reloadedPlaced.Count);
            Assert.Contains(reloadedPlaced, p => p.ScenarioId == sourceGuid && p.CopySourceScenarioId == Guid.Empty);
            Assert.Contains(reloadedPlaced, p => p.ScenarioId == newScenarioId && p.CopySourceScenarioId == Guid.Empty);
            Assert.False(GetProperty<bool>(reloadedForm, "IsDirty"));
        });
    }

    private static void SetupSyntheticFamilyWorldStore(string map, int count)
    {
        string data = Path.Combine(map, "DATA");
        Directory.CreateDirectory(data);

        // 1. objects.dat
        byte[] objects = new byte[16 + count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(4), count);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(8), 30);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(12), 30);

        // 初始化未使用的 links 為 0xFFFF 哨兵
        for (int slot = 0; slot < count; slot++)
        {
            for (int col = 0; col < LevelObjectStore.ColumnWidths.Length; col++)
            {
                if (LevelObjectStore.ColumnWidths[col] == 2)
                    BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(col, slot, count)), ushort.MaxValue);
            }
            foreach (int field in new[] { 67, 69, 71, 73, 75, 77 })
                BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(16 + slot * LevelObjectStore.RecordSize + field), ushort.MaxValue);
        }

        // Root (slot 0, UID 100)
        int rec0 = 16 + 0 * LevelObjectStore.RecordSize;
        objects[rec0] = 1; // active
        BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec0 + 3), 100u);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 67), 0); // pos0
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 69), 1); // pos1
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 71), 0); // anim 0
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 73), 0); // gfx 0
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 75), 555); // type 555
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 77), 0); // action 0
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 0, count)), 0); // objdata 0
        BinaryPrimitives.WriteInt16LittleEndian(objects.AsSpan(ColumnOffset(12, 0, count)), 0); // direct light 0

        // Child (slot 1, UID 101)
        int rec1 = 16 + 1 * LevelObjectStore.RecordSize;
        objects[rec1] = 1; // active
        BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec1 + 3), 101u);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 67), 2); // pos2
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 69), 3); // pos3
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 71), 1); // anim 1
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 73), 1); // gfx 1
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 75), 555); // type 555
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 77), 1); // action 1
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 1, count)), 1); // objdata 1
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(13, 1, count)), 0); // parent = root (0)

        // 2. objdata.dat (平行 segments)
        byte[] objdata = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
        BinaryPrimitives.WriteInt32LittleEndian(objdata.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(objdata.AsSpan(4), count);
        objdata[8 + 0 * LevelObjectStore.ObjDataWidths[0]] = 1;
        objdata[8 + 1 * LevelObjectStore.ObjDataWidths[0]] = 1;

        // 3. position.dat
        int posCount = count * 2;
        byte[] positions = new byte[8 + posCount * LevelObjectStore.PositionSize];
        BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(4), posCount);

        void WritePos(int pSlot, float x, float y, float z)
        {
            int off = 8 + pSlot * LevelObjectStore.PositionSize;
            positions[off] = 1; // active
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 1), x);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 5), y);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 9), z);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 13), 0f); // angle
        }

        WritePos(0, 100f, 20f, 200f);
        WritePos(1, 100f, 20f, 200f);
        WritePos(2, 110f, 20f, 210f);
        WritePos(3, 110f, 20f, 210f);

        // 4. anim.dat
        byte[] anim = new byte[8 + count * 25];
        BinaryPrimitives.WriteInt32LittleEndian(anim.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(anim.AsSpan(4), count);
        anim[8 + 0 * 21] = 1;
        anim[8 + 1 * 21] = 1;

        // 5. gfxtype.dat
        byte[] gfxtype = new byte[8 + count * 17];
        BinaryPrimitives.WriteInt32LittleEndian(gfxtype.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(gfxtype.AsSpan(4), count);
        gfxtype[8 + 0 * 15] = 1;
        gfxtype[8 + 1 * 15] = 1;

        // 6. action.dat
        byte[] action = new byte[12 + count * 25];
        BinaryPrimitives.WriteInt32LittleEndian(action.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(action.AsSpan(4), count);
        BinaryPrimitives.WriteInt32LittleEndian(action.AsSpan(8), 6);
        action[12 + 0 * 25] = 1;
        action[12 + 1 * 25] = 1;

        // 7. engine.dat
        byte[] engine = new byte[94];
        BinaryPrimitives.WriteInt32LittleEndian(engine.AsSpan(0), 1);
        BinaryPrimitives.WriteUInt32LittleEndian(engine.AsSpan(34), 1000u); // nextUid counter

        // 8. light.dat
        byte[] lightBytes = new byte[NativeLightPool.PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(lightBytes.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(lightBytes.AsSpan(4), 1024);
        lightBytes.AsSpan(8 + 1024 * 50).Fill(255);
        var lightPool = NativeLightPool.ParsePayload(lightBytes);
        // Direct light owner is slot 0 (root), parent is -1
        lightPool.AddBatch([new(new(100f, 20f, 200f), new(1f, 1f, 1f), 350f, 0, 0, 0, 2, 0, -1)]);
        lightBytes = lightPool.ToPayload();

        // 9. particle.dat
        byte[] particle = new byte[NativeParticlePool.PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(4), 1024);
        BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(8), 64);
        var particles = NativeParticlePool.ParsePayload(particle);
        particles.ReleaseBatch(Enumerable.Range(0, 1024).ToArray());
        particle = particles.ToPayload();

        // 10. lager.dat
        byte[] lager = new byte[NativeLagerPool.PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(4), 3200);
        BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(8), 6);
        BinaryPrimitives.WriteInt32LittleEndian(lager.AsSpan(12), 10);
        var lagers = NativeLagerPool.ParsePayload(lager);
        lagers.ReleaseBatch(Enumerable.Range(0, 3200).ToArray());
        lager = lagers.ToPayload();

        // 11. biglager.dat
        byte[] biglager = new byte[NativeBigLagerPool.PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(biglager.AsSpan(0), 1);
        BinaryPrimitives.WriteInt32LittleEndian(biglager.AsSpan(4), 32);
        BinaryPrimitives.WriteInt32LittleEndian(biglager.AsSpan(8), 800);
        var bigs = NativeBigLagerPool.ParsePayload(biglager);
        bigs.ReleaseBatch(Enumerable.Range(0, 32).ToArray());
        biglager = bigs.ToPayload();

        // 寫入各必要資料池
        File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
        File.WriteAllBytes(Path.Combine(data, "objdata.dat"), objdata);
        File.WriteAllBytes(Path.Combine(data, "position.dat"), positions);
        File.WriteAllBytes(Path.Combine(data, "anim.dat"), anim);
        File.WriteAllBytes(Path.Combine(data, "gfxtype.dat"), gfxtype);
        File.WriteAllBytes(Path.Combine(data, "action.dat"), action);
        File.WriteAllBytes(Path.Combine(data, "engine.dat"), engine);
        File.WriteAllBytes(Path.Combine(data, "light.dat"), lightBytes);
        File.WriteAllBytes(Path.Combine(data, "particle.dat"), particle);
        File.WriteAllBytes(Path.Combine(data, "lager.dat"), lager);
        File.WriteAllBytes(Path.Combine(data, "biglager.dat"), biglager);

        // An inactive hierarchy pool with no object backlinks.
        byte[] hierarchy = new byte[12 + count * 105];
        BinaryPrimitives.WriteInt32LittleEndian(hierarchy, 1);
        BinaryPrimitives.WriteInt32LittleEndian(hierarchy.AsSpan(4), count);
        BinaryPrimitives.WriteInt32LittleEndian(hierarchy.AsSpan(8), 50);
        File.WriteAllBytes(Path.Combine(data, "hirarchy.dat"), hierarchy);

        // Remaining opaque pools are presence-only placeholders for this
        // transaction fixture, not valid game map assets.
        foreach (string file in NativeMapLoadContract.RequiredDataFiles)
        {
            string filePath = Path.Combine(data, file);
            if (!File.Exists(filePath))
                File.WriteAllBytes(filePath, []);
        }
    }

    private static int ColumnOffset(int col, int slot, int totalCount)
        => 16 + totalCount * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Take(col).Sum())
           + slot * LevelObjectStore.ColumnWidths[col];

}
