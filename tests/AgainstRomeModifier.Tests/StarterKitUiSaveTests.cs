using System.Reflection;
using System.Buffers.Binary;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void StarterKit_UI_late_script_write_failure_preserves_family_undo_ids_then_retry_reload_is_idempotent()
    {
        using var official = new InstalledStarterKitFixture();
        string map = CreateFixture("ENDL_005");
        foreach (string file in Directory.EnumerateFiles(official.Root, "*", SearchOption.AllDirectories))
        {
            string destination = Path.Combine(_root, Path.GetRelativePath(official.Root, file));
            Directory.CreateDirectory(Path.GetDirectoryName(destination)!); File.Copy(file, destination, true);
        }
        // Deliberately flat, dry designer terrain; DATA and linked house remain official clones.
        WriteGray(Path.Combine(map, "boden.bmp"), 257, Enumerable.Repeat((byte)80, 257 * 257).ToArray());
        string ini = Path.Combine(map, "boden.ini");
        File.WriteAllText(ini, File.ReadAllText(ini).Replace("[Waterlevel]\r\n120", "[Waterlevel]\r\n0", StringComparison.Ordinal));
        File.Delete(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"));
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        byte[] idle = BciImage.CreateIdleLevel().Serialize();
        string script = Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile);
        File.WriteAllBytes(script, idle);
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile), idle);
        var initial = new ScenarioDocument
        {
            Events = [new ScenarioEvent("StarterKitLateWrite", 5) { Actions = [new(ScenarioActionKind.Message, "Starter")] }]
        };
        using (var rollback = new FileRollbackScope()) { initial.Save(map, rollback); rollback.Commit(); }
        var before = SnapshotDirectory(map);

        RunInSta(() =>
        {
            SdlObjectType[] catalog =
            [
                new(BlankMapStarterKit.MainHouseName, 232, SdlObjectCategory.Building, "Ger", 1,
                    new Dictionary<string, string> { ["alias"] = BlankMapStarterKit.MainHouseAlias }),
                new(BlankMapStarterKit.UnitName, 218, SdlObjectCategory.Figure, "Ger", 1,
                    new Dictionary<string, string> { ["alias"] = BlankMapStarterKit.UnitAlias })
            ];
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "StarterKit", "Test"));
            _ = form.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, catalog);
            Invoke(form, "LoadSelectedMap");
            Invoke(form, "ApplyStarterKit", 4, 8000f, 8000f);
            var kit = GetField<BlankMapStarterKit>(form, "_pendingStarterKit");
            var placed = form.PlacementSession.Capture().ToArray();
            Assert.Equal(5, placed.Length); Assert.All(placed, p => Assert.NotNull(p.RawRotation));
            Assert.True(form.PlacementSession.IsDirty); Assert.True(form.PlacementSession.CanUndo);
            AssertSnapshotUnchanged(map, before);
            Assert.True(form.PlacementSession.Undo()); Assert.Empty(form.PlacementSession.Capture());
            Assert.True(form.PlacementSession.Redo());
            Assert.Equal(placed.Select(StarterPlacementState), form.PlacementSession.Capture().Select(StarterPlacementState));
            // Allow idle-source validation/read, but reject the event script replacement.
            // This fails after native DATA and Scenario writes, forcing the real save rollback.
            using (var lockedScript = new FileStream(script, FileMode.Open, FileAccess.Read, FileShare.Read))
            {
                Assert.False(form.TrySaveMap(false, out var failure));
                Assert.True(failure is IOException, failure?.ToString()); Assert.Contains(LevelScriptInjector.ScriptFile, failure!.Message);
            }
            AssertSnapshotUnchanged(map, before);
            Assert.Equal(placed.Select(StarterPlacementState), form.PlacementSession.Capture().Select(StarterPlacementState));
            Assert.Same(kit, GetField<BlankMapStarterKit>(form, "_pendingStarterKit"));
            Assert.True(form.PlacementSession.IsDirty); Assert.True(form.PlacementSession.CanUndo);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(form.TrySaveMap(false, out var error), error?.ToString()); Assert.Null(error);
            Assert.False(form.PlacementSession.IsDirty); Assert.False(form.PlacementSession.CanUndo);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            StarterKitFamilyTests.AssertFamilyKit(kit, LevelObjectStore.Load(map), ScenarioDocument.Load(map), 4);
            var saved = SnapshotDirectory(map);
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            AssertSnapshotUnchanged(map, saved);
            using var reloaded = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "StarterKitReload", "Test"));
            _ = reloaded.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reloaded, catalog);
            Invoke(reloaded, "LoadSelectedMap");
            Assert.Equal(placed.Select(p => p.ScenarioId).Order(), reloaded.PlacementSession.Capture().Select(p => p.ScenarioId).Order());
            Assert.All(reloaded.PlacementSession.Capture(), p => { Assert.Equal(4, p.Team); Assert.NotNull(p.RawRotation); });
            Assert.False(GetProperty<bool>(reloaded, "IsDirty"));
        });
        official.AssertSourceUnchanged();
    }

    [Theory]
    [InlineData(10, 6)]
    [InlineData(6, 10)]
    public void StarterKit_cross_store_capacities_preserve_remapped_family_pool_bytes_on_reload(int sourceCount, int targetCount)
    {
        string sourceMap = CreateFixture("ENDL_020"), targetMap = CreateFixture("ENDL_021");
        SetupSyntheticFamilyWorldStore(sourceMap, sourceCount);
        SetupSyntheticFamilyWorldStore(targetMap, targetCount);
        StampStarterFamilyPools(sourceMap, sourceCount);
        var source = LevelObjectStore.Load(sourceMap);
        var target = LevelObjectStore.Load(targetMap); target.ResetForBlankMap();
        var sourceDisk = SnapshotDirectory(sourceMap); var targetDisk = SnapshotDirectory(targetMap);
        var sourceMemory = StarterStoreBytes(source, "source-before"); var targetMemory = StarterStoreBytes(target, "target-before");
        var candidate = Assert.IsType<LevelObjectFamilyCopyCandidate>(target.CreateFamilyImportCandidate(source, 0, 100, 500 * 65536, 7 * 65536, 600 * 65536, 4));
        AssertSnapshotUnchanged(sourceMap, sourceDisk); AssertSnapshotUnchanged(targetMap, targetDisk);
        AssertStarterStoreBytes(sourceMemory, StarterStoreBytes(source, "source-after"));
        AssertStarterStoreBytes(targetMemory, StarterStoreBytes(target, "target-after"));
        using (var rollback = new FileRollbackScope()) { candidate.Store.Save(targetMap, rollback); rollback.Commit(); }
        var saved = LevelObjectStore.Load(targetMap);
        Assert.Equal(targetCount, saved.Capacity);
        var family = Assert.IsType<LevelObjectFamily>(saved.FamilyAt(candidate.RootSlot, candidate.RootUid));
        Assert.Equal(candidate.Allocation.Objects.Values.Order(), family.ObjectSlots.Order());
        Assert.Equal(2, family.ObjectSlots.Count); Assert.Single(family.LightSlots); Assert.Equal(2, saved.Objects().Count);
        foreach (var pair in candidate.Allocation.Objects)
        {
            var original = source.Objects().Single(o => o.Slot == pair.Key);
            var actual = saved.Objects().Single(o => o.Slot == pair.Value);
            Assert.Equal(candidate.Allocation.Uids[pair.Key], actual.Uid); Assert.NotEqual(original.Uid, actual.Uid);
            Assert.Equal(4, actual.Team); Assert.Equal(original.TypeId, actual.TypeId);
            Assert.Equal(original.X + 500, actual.X); Assert.Equal(original.Y + 7, actual.Y); Assert.Equal(original.Z + 600, actual.Z);
            Assert.Equal(original.Rotation, actual.Rotation);
        }
        byte[] Read(string name) => File.ReadAllBytes(Path.Combine(targetMap, "DATA", name));
        byte[] objects = Read("objects.dat");
        foreach (var spec in new[] { ("anim.dat", 8, 21, 2, 71, candidate.Allocation.Anim),
            ("gfxtype.dat", 8, 15, 1, 73, candidate.Allocation.GfxType), ("action.dat", 12, 25, 0, 77, candidate.Allocation.Action) })
        {
            byte[] old = sourceDisk[Path.Combine("DATA", spec.Item1)], actual = Read(spec.Item1);
            Assert.Equal(2, spec.Item6.Count);
            foreach (var pair in spec.Item6)
            {
                Assert.NotEqual(pair.Key, pair.Value);
                Assert.Equal(old.AsSpan(spec.Item2 + pair.Key * spec.Item3, spec.Item3).ToArray(),
                    actual.AsSpan(spec.Item2 + pair.Value * spec.Item3, spec.Item3).ToArray());
                for (int tail = 0; tail < spec.Item4; tail++)
                    Assert.Equal(old.AsSpan(spec.Item2 + sourceCount * (spec.Item3 + tail * 2) + pair.Key * 2, 2).ToArray(),
                        actual.AsSpan(spec.Item2 + targetCount * (spec.Item3 + tail * 2) + pair.Value * 2, 2).ToArray());
                int member = pair.Key - (sourceCount - 2);
                Assert.Equal(pair.Value, BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + candidate.Allocation.Objects[member] * LevelObjectStore.RecordSize + spec.Item5)));
            }
        }
        byte[] oldData = sourceDisk[Path.Combine("DATA", "objdata.dat")], data = Read("objdata.dat");
        int prefix = 0;
        foreach (int width in LevelObjectStore.ObjDataWidths)
        {
            foreach (var pair in candidate.Allocation.ObjData)
            {
                Assert.NotEqual(pair.Key, pair.Value);
                Assert.Equal(oldData.AsSpan(8 + sourceCount * prefix + pair.Key * width, width).ToArray(),
                    data.AsSpan(8 + targetCount * prefix + pair.Value * width, width).ToArray());
                int member = pair.Key - (sourceCount - 2);
                Assert.Equal(pair.Value, BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, candidate.Allocation.Objects[member], targetCount))));
            }
            prefix += width;
        }
        AssertSnapshotUnchanged(sourceMap, sourceDisk);
        AssertStarterStoreBytes(sourceMemory, StarterStoreBytes(source, "source-final"));
        AssertStarterStoreBytes(targetMemory, StarterStoreBytes(target, "target-final"));
    }

    [Theory]
    [InlineData(0u)]
    [InlineData(99u)]
    [InlineData(101u)]
    public void StarterKit_cross_store_stale_source_uid_rejects_unchanged(uint uid)
    {
        string sourceMap = CreateFixture("ENDL_020"), targetMap = CreateFixture("ENDL_021");
        SetupSyntheticFamilyWorldStore(sourceMap, 10); SetupSyntheticFamilyWorldStore(targetMap, 6);
        var source = LevelObjectStore.Load(sourceMap); var target = LevelObjectStore.Load(targetMap); target.ResetForBlankMap();
        var sourceDisk = SnapshotDirectory(sourceMap); var targetDisk = SnapshotDirectory(targetMap);
        var sourceMemory = StarterStoreBytes(source, "source-before"); var targetMemory = StarterStoreBytes(target, "target-before");
        Assert.Null(target.CreateFamilyImportCandidate(source, 0, uid, 500, 0, 600));
        AssertStarterStoreBytes(sourceMemory, StarterStoreBytes(source, "source-after"));
        AssertStarterStoreBytes(targetMemory, StarterStoreBytes(target, "target-after"));
        AssertSnapshotUnchanged(sourceMap, sourceDisk); AssertSnapshotUnchanged(targetMap, targetDisk);
    }

    [Theory]
    [InlineData("water")]
    [InlineData("blocking")]
    [InlineData("extent")]
    [InlineData("moved-overlap")]
    public void StarterKit_UI_safety_rejection_preserves_disk_and_pending_state(string failure)
    {
        using var official = new InstalledStarterKitFixture();
        string map = CreateFixture("ENDL_005");
        foreach (string file in Directory.EnumerateFiles(official.Root, "*", SearchOption.AllDirectories))
        {
            string destination = Path.Combine(_root, Path.GetRelativePath(official.Root, file));
            Directory.CreateDirectory(Path.GetDirectoryName(destination)!); File.Copy(file, destination, true);
        }
        WriteGray(Path.Combine(map, "boden.bmp"), 257, Enumerable.Repeat(failure == "water" ? (byte)0 : (byte)80, 257 * 257).ToArray());
        if (failure == "blocking")
            WriteGray(Path.Combine(map, "collision.bmp"), 256, Enumerable.Repeat((byte)255, 256 * 256).ToArray());
        string ini = Path.Combine(map, "boden.ini");
        File.WriteAllText(ini, File.ReadAllText(ini).Replace("[Waterlevel]\r\n120", "[Waterlevel]\r\n0", StringComparison.Ordinal));
        File.Delete(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"));
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        byte[] idle = BciImage.CreateIdleLevel().Serialize();
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), idle);
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile), idle);
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            SdlObjectType[] catalog =
            [
                new(BlankMapStarterKit.MainHouseName, 232, SdlObjectCategory.Building, "Ger", 1,
                    new Dictionary<string, string> { ["alias"] = BlankMapStarterKit.MainHouseAlias }),
                new(BlankMapStarterKit.UnitName, 218, SdlObjectCategory.Figure, "Ger", 1,
                    new Dictionary<string, string> { ["alias"] = BlankMapStarterKit.UnitAlias })
            ];
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "StarterSafety", "Test"));
            _ = form.Handle;
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, catalog);
            Invoke(form, "LoadSelectedMap");
            if (failure != "moved-overlap")
            {
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                Assert.Throws<InvalidDataException>(() => form.ApplyStarterKit(4, 8000, failure == "extent" ? 200 : 8000));
                Assert.Empty(form.PlacementSession.Capture()); Assert.False(form.PlacementSession.CanUndo);
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                Assert.Null(typeof(MapEditorForm).GetField("_pendingStarterKit", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form));
            }
            else
            {
                form.ApplyStarterKit(4, 8000, 8000);
                var kit = GetField<BlankMapStarterKit>(form, "_pendingStarterKit");
                var placements = form.PlacementSession.Capture().ToArray();
                int houseIndex = Array.FindIndex(placements, p => p.Type.Category == SdlObjectCategory.Building);
                int unitIndex = Array.FindIndex(placements, p => p.Type.Category == SdlObjectCategory.Figure);
                var house = placements[houseIndex]; var unit = placements[unitIndex];
                form.PlacementSession.Edit(unitIndex, unit with { WorldX = house.WorldX, WorldZ = house.WorldZ });
                var moved = form.PlacementSession.Capture().Select(StarterPlacementState).ToArray();
                Assert.False(form.TrySaveMap(false, out var error)); Assert.IsType<InvalidDataException>(error);
                Assert.Equal(moved, form.PlacementSession.Capture().Select(StarterPlacementState));
                Assert.Same(kit, GetField<BlankMapStarterKit>(form, "_pendingStarterKit"));
                Assert.True(form.PlacementSession.IsDirty); Assert.True(form.PlacementSession.CanUndo);
                Assert.True(GetProperty<bool>(form, "IsDirty"));
                Assert.True(form.PlacementSession.Undo());
                Assert.Equal(placements.Select(StarterPlacementState), form.PlacementSession.Capture().Select(StarterPlacementState));
                Assert.True(form.PlacementSession.Redo());
                Assert.Equal(moved, form.PlacementSession.Capture().Select(StarterPlacementState));
            }
            AssertSnapshotUnchanged(map, before);
        });
        official.AssertSourceUnchanged();
    }

    private static void StampStarterFamilyPools(string map, int count)
    {
        string data = Path.Combine(map, "DATA");
        byte[] objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
        foreach (var spec in new[] { ("anim.dat", 8, 21, 2, 71), ("gfxtype.dat", 8, 15, 1, 73), ("action.dat", 12, 25, 0, 77) })
        {
            byte[] bytes = File.ReadAllBytes(Path.Combine(data, spec.Item1));
            for (int member = 0; member < 2; member++)
            {
                int slot = count - 2 + member;
                bytes.AsSpan(spec.Item2 + member * spec.Item3, spec.Item3).CopyTo(bytes.AsSpan(spec.Item2 + slot * spec.Item3));
                bytes[spec.Item2 + member * spec.Item3] = 0;
                bytes[spec.Item2 + slot * spec.Item3 + spec.Item3 - 1] = (byte)(0x30 + member + spec.Item5);
                for (int tail = 0; tail < spec.Item4; tail++)
                    BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(spec.Item2 + count * (spec.Item3 + tail * 2) + slot * 2), (ushort)(0x5100 + member * 0x100 + tail * 17 + spec.Item5));
                BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(16 + member * LevelObjectStore.RecordSize + spec.Item5), (ushort)slot);
            }
            File.WriteAllBytes(Path.Combine(data, spec.Item1), bytes);
        }
        byte[] objdata = File.ReadAllBytes(Path.Combine(data, "objdata.dat"));
        int prefix = 0, segment = 0;
        foreach (int width in LevelObjectStore.ObjDataWidths)
        {
            for (int member = 0; member < 2; member++)
            {
                objdata.AsSpan(8 + count * prefix + member * width, width).Clear();
                objdata.AsSpan(8 + count * prefix + (count - 2 + member) * width, width).Fill((byte)(0x20 + segment * 2 + member));
            }
            prefix += width; segment++;
        }
        for (int member = 0; member < 2; member++)
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, member, count)), (ushort)(count - 2 + member));
        File.WriteAllBytes(Path.Combine(data, "objdata.dat"), objdata); File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
    }

    private Dictionary<string, byte[]> StarterStoreBytes(LevelObjectStore store, string label)
    {
        string path = Path.Combine(_root, "Snapshots", label); Directory.CreateDirectory(path);
        using var rollback = new FileRollbackScope(); store.Save(path, rollback); rollback.Commit();
        return SnapshotDirectory(path);
    }

    private static void AssertStarterStoreBytes(Dictionary<string, byte[]> expected, Dictionary<string, byte[]> actual)
    {
        Assert.Equal(expected.Keys.Order(), actual.Keys.Order());
        foreach (var (file, bytes) in expected) Assert.Equal(bytes, actual[file]);
    }

    private static object StarterPlacementState(SdlPlacedObject item) =>
        (item.Type.NameDef, item.Type.Definition, item.WorldX, item.WorldY, item.WorldZ,
            item.Team, item.Angle, item.UnitCount, item.ScenarioId, item.CopySourceScenarioId, item.RawRotation);
}
