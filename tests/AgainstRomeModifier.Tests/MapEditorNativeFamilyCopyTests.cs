using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Native_family_copy_via_3d_pick_and_copy_command_survives_failed_save_retry_and_reload()
    {
        string map = CreateFixture("ENDL_012");
        const int count = 10;
        SetupSyntheticFamilyWorldStore(map, count);

        // 原圖 native 未 binding：初始 Scenario 完全沒有 Spawns 與 DataSlots！
        // 唯放一個 enabled message event，使第一次 Save 在 script injection 階段因缺少 BCI 檔案而 late rollback。
        var initialScenario = new ScenarioDocument
        {
            Spawns = [],
            DataSlots = [],
            Events = [new ScenarioEvent("ForceBciRollback", 5)
            { Actions = [new(ScenarioActionKind.Message, "NativeCopied")] }]
        };
        using (var rollback = new FileRollbackScope()) { initialScenario.Save(map, rollback); rollback.Commit(); }

        var beforeDisk = SnapshotDirectory(map);

        RunInSta(() =>
        {
            var buildingType = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "BauTest" });

            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "NativeFamilyCopy", "Test"));
            _ = form.Handle;

            // 注入真實 catalog 與 objdefNames
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { buildingType });
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
                new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerTestChild" });

            Invoke(form, "LoadSelectedMap");

            // 驗證初始狀態：無放置物件，無 dirty
            Assert.Equal(0, form.PlacementSession.Count);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(form.PlacementSession.IsDirty);
            Assert.False(form.NativeCopyButton.Enabled);

            // 1. 模擬 3D Pick：選取原生 root (slot 0, UID 100)
            form.SelectNativeSceneObject(0);

            Assert.NotNull(form.ActiveNativeSource);
            Assert.Equal(0, form.ActiveNativeSource.Slot);
            Assert.Equal(100u, form.ActiveNativeSource.Uid);
            Assert.Equal("BauTest", form.ActiveNativeSource.Alias);
            Assert.True(form.NativeCopyButton.Enabled);

            // 2. 執行原生複製（複製指令）
            form.CopyActiveNativeFamily();

            Assert.Equal(1, form.PlacementSession.Count);
            Assert.True(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanUndo);
            Assert.True(GetProperty<bool>(form, "IsDirty"));

            var stagedItem = form.PlacementSession[0];
            Guid newScenarioId = stagedItem.ScenarioId;
            Assert.NotEqual(Guid.Empty, newScenarioId);
            // 關鍵契約：原圖未 binding root 不建立 source Scenario/binding，不編造 previous sourceGuid
            Assert.Equal(Guid.Empty, stagedItem.CopySourceScenarioId);
            Assert.Equal(new NativeObjectIdentity(0, 100u), stagedItem.NativeSourceIdentity);
            Assert.Equal(0f, stagedItem.RawRotation);
            Assert.Equal(100f + 256f, stagedItem.WorldX);
            Assert.Equal(200f + 256f, stagedItem.WorldZ);

            // 驗證 Undo / Redo 完整身份保留
            Assert.True(form.PlacementSession.Undo());
            Assert.Equal(0, form.PlacementSession.Count);
            Assert.False(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanRedo);

            Assert.True(form.PlacementSession.Redo());
            Assert.Equal(1, form.PlacementSession.Count);
            Assert.True(form.PlacementSession.IsDirty);
            Assert.Equal(newScenarioId, form.PlacementSession[0].ScenarioId);
            Assert.Equal(Guid.Empty, form.PlacementSession[0].CopySourceScenarioId);
            Assert.Equal(new NativeObjectIdentity(0, 100u), form.PlacementSession[0].NativeSourceIdentity);
            Assert.Equal(0f, form.PlacementSession[0].RawRotation);

            // 3. 第一次 TrySaveMap：缺少 script BCI 使其在 nativeStore.Save 之後 late-fail 回滾
            Assert.False(form.TrySaveMap(showSuccess: false, out Exception? error));
            Assert.NotNull(error);
            Assert.IsType<FileNotFoundException>(error);

            // 驗證所有磁碟 snapshot unchanged（原圖來源未被寫入或污染）
            AssertSnapshotUnchanged(map, beforeDisk);

            // UI dirty / undo / provenance 保留
            Assert.True(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanUndo);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.Equal(1, form.PlacementSession.Count);
            Assert.Equal(newScenarioId, form.PlacementSession[0].ScenarioId);
            Assert.Equal(new NativeObjectIdentity(0, 100u), form.PlacementSession[0].NativeSourceIdentity);

            // 4. 補有效 SCRIPT BCI 後重試成功
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());

            Assert.True(form.TrySaveMap(showSuccess: false, out error), error?.ToString());
            Assert.Null(error);

            // 成功後 provenance 清空、raw 保留、dirty 清除、CanUndo false
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(form.PlacementSession.IsDirty);
            Assert.False(form.PlacementSession.CanUndo);
            var placedAfterSave = form.PlacementSession.Capture();
            Assert.Single(placedAfterSave);
            Assert.Null(placedAfterSave[0].NativeSourceIdentity);
            Assert.Equal(Guid.Empty, placedAfterSave[0].CopySourceScenarioId);
            Assert.Equal(0f, placedAfterSave[0].RawRotation);

            // 5. 驗證 DATA 與 Scenario 狀態
            var reloadedStore = LevelObjectStore.Load(map);
            // 來源原 objects 完全不變（未受污染）
            var sourceRootObj = reloadedStore.Objects().Single(o => o.Slot == 0);
            Assert.Equal(100u, sourceRootObj.Uid);
            Assert.Equal(100f, sourceRootObj.X);
            Assert.Equal(20f, sourceRootObj.Y);
            Assert.Equal(200f, sourceRootObj.Z);

            var sourceFamily = reloadedStore.FamilyAt(0, 100u);
            Assert.NotNull(sourceFamily);

            // Scenario 驗證：只綁定 new root，原圖來源 root 無 scenario binding！
            var savedScenario = ScenarioDocument.Load(map);
            Assert.Single(savedScenario.Spawns);
            var savedSpawn = savedScenario.Spawns[0];
            Assert.Equal(newScenarioId, savedSpawn.Id);
            Assert.True(savedSpawn.Prebuilt);

            Assert.Single(savedScenario.DataSlots);
            var newRootBinding = savedScenario.DataSlots[0];
            Assert.Equal(newScenarioId, newRootBinding.SpawnId);
            int newRootSlot = newRootBinding.Slot;
            uint newRootUid = newRootBinding.Uid;

            Assert.NotEqual(0, newRootSlot);
            Assert.NotEqual(100u, newRootUid);

            // 驗證新建立的 copied family 完整且有效
            var copiedFamily = reloadedStore.FamilyAt(newRootSlot, newRootUid);
            Assert.NotNull(copiedFamily);
            Assert.Equal(sourceFamily.ObjectSlots.Count, copiedFamily.ObjectSlots.Count);
            Assert.Equal(sourceFamily.LightSlots.Count, copiedFamily.LightSlots.Count);

            var copiedRootObj = reloadedStore.Objects().Single(o => o.Slot == newRootSlot);
            Assert.Equal(sourceRootObj.X + 256f, copiedRootObj.X);
            Assert.Equal(sourceRootObj.Y, copiedRootObj.Y);
            Assert.Equal(sourceRootObj.Z + 256f, copiedRootObj.Z);

            // 6. 二次儲存 byte idempotence
            var diskAfterSuccess = SnapshotDirectory(map);
            Assert.True(form.TrySaveMap(showSuccess: false, out error), error?.ToString());
            Assert.Null(error);
            AssertSnapshotUnchanged(map, diskAfterSuccess);

            // 7. Reload UI：驗證存檔狀態還原
            using var reloadedForm = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "NativeReloaded", "Test"));
            _ = reloadedForm.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloadedForm, new[] { buildingType });
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloadedForm,
                new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerTestChild" });
            Invoke(reloadedForm, "LoadSelectedMap");

            var reloadedPlaced = reloadedForm.PlacementSession.Capture();
            Assert.Single(reloadedPlaced);
            Assert.Equal(newScenarioId, reloadedPlaced[0].ScenarioId);
            Assert.Equal(Guid.Empty, reloadedPlaced[0].CopySourceScenarioId);
            Assert.Null(reloadedPlaced[0].NativeSourceIdentity);
            Assert.Equal(0f, reloadedPlaced[0].RawRotation);
            Assert.False(GetProperty<bool>(reloadedForm, "IsDirty"));
        });
    }

    [Fact]
    public void Native_selection_rejects_child_shared_unknown_alias_and_already_bound_root()
    {
        string map = CreateFixture("ENDL_012");
        const int count = 10;
        SetupSyntheticFamilyWorldStore(map, count);
        AddSyntheticGuardRoots(map, count);

        // 綁定 slot 0
        Guid boundGuid = Guid.NewGuid();
        var initialScenario = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", 100f, 200f, 0, Angle: 0, Y: 20f, Prebuilt: true) { Id = boundGuid }],
            DataSlots = [new ScenarioDataSlot(0, 100u) { SpawnId = boundGuid }]
        };
        using (var rollback = new FileRollbackScope()) { initialScenario.Save(map, rollback); rollback.Commit(); }

        RunInSta(() =>
        {
            var buildingType = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "BauTest" });

            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "NativeGuards", "Test"));
            _ = form.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { buildingType });
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
                new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerTestChild" });

            Invoke(form, "LoadSelectedMap");

            // 1. Slot 0 已經在 Scenario 中 bound：挑選時拒絕，按鈕維持 disabled
            form.SelectNativeSceneObject(0);
            Assert.Null(form.ActiveNativeSource);
            Assert.False(form.NativeCopyButton.Enabled);
            Assert.Contains("場景綁定", form.NativeSourceLabel.Text);

            // 2. Slot 1 是 child 物件（非 root）：挑選時拒絕
            form.SelectNativeSceneObject(1);
            Assert.Null(form.ActiveNativeSource);
            Assert.False(form.NativeCopyButton.Enabled);
            Assert.Contains("非獨立", form.NativeSourceLabel.Text);

            // 3. Slot 2 是獨立 root 但 TypeId 777 在放置目錄無對應別名：挑選時拒絕
            form.SelectNativeSceneObject(2);
            Assert.Null(form.ActiveNativeSource);
            Assert.False(form.NativeCopyButton.Enabled);
            Assert.Contains("無對應別名", form.NativeSourceLabel.Text);

            // 4. Slot 3 是 root 但與外部物件共用 objdata 資料：挑選時拒絕
            form.SelectNativeSceneObject(3);
            Assert.Null(form.ActiveNativeSource);
            Assert.False(form.NativeCopyButton.Enabled);
            Assert.Contains("非獨立", form.NativeSourceLabel.Text);

            // 5. Slot 9 是 inactive / 空槽位：挑選時拒絕
            form.SelectNativeSceneObject(9);
            Assert.Null(form.ActiveNativeSource);
            Assert.False(form.NativeCopyButton.Enabled);

            // 6. Slot 4 是合法獨立未綁定 root：挑選成功，但若磁碟 UID 變更（stale UID），執行複製時拒絕並清除選取
            form.SelectNativeSceneObject(4);
            Assert.NotNull(form.ActiveNativeSource);
            Assert.Equal(4, form.ActiveNativeSource.Slot);
            Assert.Equal(104u, form.ActiveNativeSource.Uid);
            Assert.True(form.NativeCopyButton.Enabled);

            // 模擬選取後外部修改使 store 中的 UID 與選取時過時不一致
            string objectsPath = Path.Combine(map, "DATA", "objects.dat");
            byte[] objs = File.ReadAllBytes(objectsPath);
            int rec4 = 16 + 4 * LevelObjectStore.RecordSize;
            BinaryPrimitives.WriteUInt32LittleEndian(objs.AsSpan(rec4 + 3), 999u);
            File.WriteAllBytes(objectsPath, objs);

            string placementsBeforeStaleCopy = System.Text.Json.JsonSerializer.Serialize(form.PlacementSession.Capture());
            form.CopyActiveNativeFamily();
            Assert.Null(form.ActiveNativeSource);
            Assert.False(form.NativeCopyButton.Enabled);
            Assert.Equal(placementsBeforeStaleCopy, System.Text.Json.JsonSerializer.Serialize(form.PlacementSession.Capture()));
        });
    }

    [Fact]
    public void Fractional_raw_rotation_is_preserved_bit_exact_on_load_and_save()
    {
        string map = CreateFixture("ENDL_012");
        const int count = 10;
        float fractionalRadians = 0.654321f;
        SetupSyntheticFamilyWithFractionalRotation(map, count, fractionalRadians);

        // Derive expectedRaw from loaded store exact readback contract
        var initialStore = LevelObjectStore.Load(map);
        float expectedRaw = initialStore.Objects().Single(o => o.Slot == 0).Rotation;
        Assert.True(float.IsFinite(expectedRaw) && expectedRaw != 0f);

        var initialScenario = new ScenarioDocument();
        using (var rollback = new FileRollbackScope()) { initialScenario.Save(map, rollback); rollback.Commit(); }

        RunInSta(() =>
        {
            var buildingType = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "BauTest" });

            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "FractionalTest", "Test"));
            _ = form.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { buildingType });
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
                new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerTestChild" });

            Invoke(form, "LoadSelectedMap");

            // 3D Pick slot 0
            form.SelectNativeSceneObject(0);
            Assert.NotNull(form.ActiveNativeSource);
            Assert.Equal(BitConverter.SingleToInt32Bits(expectedRaw), BitConverter.SingleToInt32Bits(form.ActiveNativeSource.Rotation));

            // Copy
            form.CopyActiveNativeFamily();
            Assert.Single(form.PlacementSession.Capture());
            var copy = form.PlacementSession[0];
            Assert.Equal(BitConverter.SingleToInt32Bits(expectedRaw), BitConverter.SingleToInt32Bits(copy.RawRotation!.Value));

            // 精確 degrees 而非整數 round
            float expectedExactDeg = (float)((double)expectedRaw * (180d / Math.PI));
            Assert.Equal(expectedExactDeg, copy.Angle);

            // 成功存檔（無 events 故無 BCI 需求）
            Assert.True(form.TrySaveMap(showSuccess: false, out Exception? error), error?.ToString());
            Assert.Null(error);

            // 驗證存檔 Scenario 帶有 bit-exact RawRotation
            var savedScenario = ScenarioDocument.Load(map);
            Assert.Single(savedScenario.Spawns);
            Assert.NotNull(savedScenario.Spawns[0].RawRotation);
            Assert.Equal(BitConverter.SingleToInt32Bits(expectedRaw), BitConverter.SingleToInt32Bits(savedScenario.Spawns[0].RawRotation!.Value));

            // 驗證 Reload 後精確保留
            using var reloaded = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "FractionalReload", "Test"));
            _ = reloaded.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloaded, new[] { buildingType });
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloaded,
                new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerTestChild" });
            Invoke(reloaded, "LoadSelectedMap");

            var reloadedPlaced = reloaded.PlacementSession.Capture();
            Assert.Single(reloadedPlaced);
            Assert.Equal(BitConverter.SingleToInt32Bits(expectedRaw), BitConverter.SingleToInt32Bits(reloadedPlaced[0].RawRotation!.Value));
            Assert.Equal(expectedExactDeg, reloadedPlaced[0].Angle);
        });
    }

    private static void SetupSyntheticFamilyWithFractionalRotation(string map, int count, float rotationRadians)
    {
        SetupSyntheticFamilyWorldStore(map, count);
        string posPath = Path.Combine(map, "DATA", "position.dat");
        byte[] positions = File.ReadAllBytes(posPath);

        // Native position +12 (byte offset 13 from start of record) stores angle in degrees.
        // Seed requested radians-derived disk degrees.
        float diskDegrees = (float)((double)rotationRadians * (180d / Math.PI));
        int recPos0 = 8 + 0 * LevelObjectStore.PositionSize;
        BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(recPos0 + 13), diskDegrees);
        File.WriteAllBytes(posPath, positions);
    }

    private static void AddSyntheticGuardRoots(string map, int count)
    {
        string data = Path.Combine(map, "DATA");
        byte[] objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
        byte[] objdata = File.ReadAllBytes(Path.Combine(data, "objdata.dat"));
        byte[] positions = File.ReadAllBytes(Path.Combine(data, "position.dat"));
        byte[] anim = File.ReadAllBytes(Path.Combine(data, "anim.dat"));
        byte[] gfxtype = File.ReadAllBytes(Path.Combine(data, "gfxtype.dat"));
        byte[] action = File.ReadAllBytes(Path.Combine(data, "action.dat"));

        void WritePos(int pSlot, float x, float y, float z)
        {
            int off = 8 + pSlot * LevelObjectStore.PositionSize;
            positions[off] = 1;
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 1), x);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 5), y);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 9), z);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(off + 13), 0f);
        }

        // Slot 2: Valid independent root, but TypeId 777 has no alias in catalog / _objdefNames
        int rec2 = 16 + 2 * LevelObjectStore.RecordSize;
        objects[rec2] = 1;
        BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec2 + 3), 102u);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec2 + 67), 4);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec2 + 69), 5);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec2 + 71), 2);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec2 + 73), 2);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec2 + 75), 777); // Unknown TypeId
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec2 + 77), 2);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 2, count)), 2);

        objdata[8 + 2 * LevelObjectStore.ObjDataWidths[0]] = 1;
        WritePos(4, 300f, 20f, 400f);
        WritePos(5, 300f, 20f, 400f);
        anim[8 + 2 * 21] = 1;
        gfxtype[8 + 2 * 15] = 1;
        action[12 + 2 * 25] = 1;

        // Slot 3: Root with TypeId 555, but shares ObjDataColumn 0 with Slot 0 -> FamilyAt returns null (shared data)
        int rec3 = 16 + 3 * LevelObjectStore.RecordSize;
        objects[rec3] = 1;
        BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec3 + 3), 103u);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec3 + 67), 6);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec3 + 69), 7);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec3 + 71), 3);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec3 + 73), 3);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec3 + 75), 555);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec3 + 77), 3);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 3, count)), 0); // Shared with slot 0!

        WritePos(6, 500f, 20f, 600f);
        WritePos(7, 500f, 20f, 600f);
        anim[8 + 3 * 21] = 1;
        gfxtype[8 + 3 * 15] = 1;
        action[12 + 3 * 25] = 1;

        // Slot 4: Valid independent root with TypeId 555 (UID 104) for stale UID test
        int rec4 = 16 + 4 * LevelObjectStore.RecordSize;
        objects[rec4] = 1;
        BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec4 + 3), 104u);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec4 + 67), 8);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec4 + 69), 9);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec4 + 71), 4);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec4 + 73), 4);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec4 + 75), 555);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec4 + 77), 4);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 4, count)), 4);

        objdata[8 + 4 * LevelObjectStore.ObjDataWidths[0]] = 1;
        WritePos(8, 700f, 20f, 800f);
        WritePos(9, 700f, 20f, 800f);
        anim[8 + 4 * 21] = 1;
        gfxtype[8 + 4 * 15] = 1;
        action[12 + 4 * 25] = 1;

        File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
        File.WriteAllBytes(Path.Combine(data, "objdata.dat"), objdata);
        File.WriteAllBytes(Path.Combine(data, "position.dat"), positions);
        File.WriteAllBytes(Path.Combine(data, "anim.dat"), anim);
        File.WriteAllBytes(Path.Combine(data, "gfxtype.dat"), gfxtype);
        File.WriteAllBytes(Path.Combine(data, "action.dat"), action);
    }
}
