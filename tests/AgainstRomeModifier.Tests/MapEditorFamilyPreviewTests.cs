using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    private static readonly NativeAnimationVisualState PreviewRootState = new(2, 3, 4, 5, 6);
    private static readonly NativeAnimationVisualState PreviewChildState = new(7, 8, 9, 10, 11);
    private static readonly float PreviewChildDiskDegrees = (float)(1.25d * (180d / Math.PI));
    private static readonly float PreviewChildRawRotation = (float)(PreviewChildDiskDegrees * (Math.PI / 180d));

    // Use the same closed, cross-pool family used by actual host save transaction tests.
    // Opaque presence-only pools in that fixture are not claimed to be playable game assets.
    private static float SetupPreviewFamily(string map, bool pfil, bool fractional)
    {
        const int count = 10;
        SetupSyntheticFamilyWorldStore(map, count);
        string data = Path.Combine(map, "DATA");
        var objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
        int child = 16 + LevelObjectStore.RecordSize;
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(child + 1), 4);
        BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(child + 75), 556);
        File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
        float rootDiskDegrees = fractional ? 37.25f : 0f;
        var positions = File.ReadAllBytes(Path.Combine(data, "position.dat"));
        for (int slot = 0; slot < 4; slot++)
        {
            int offset = 8 + slot * LevelObjectStore.PositionSize;
            // Native position records store degrees; Objects() exposes float32 radians.
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(offset + 13), slot < 2 ? rootDiskDegrees : PreviewChildDiskDegrees);
            if (slot >= 2) BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(offset + 5), 23.125f);
        }
        File.WriteAllBytes(Path.Combine(data, "position.dat"), positions);
        var anim = File.ReadAllBytes(Path.Combine(data, "anim.dat"));
        foreach (var (slot, state) in new[] { (0, PreviewRootState), (1, PreviewChildState) })
        {
            int offset = 8 + slot * 21;
            foreach (var (field, value) in new[] { (5, state.AlrFrame), (13, state.AptAxis0),
                (17, state.AptAxis1), (15, state.AptAxis2), (19, state.AptAxis3) })
                BinaryPrimitives.WriteInt16LittleEndian(anim.AsSpan(offset + field), value);
        }
        File.WriteAllBytes(Path.Combine(data, "anim.dat"), anim);
        if (pfil)
            foreach (string path in Directory.EnumerateFiles(data))
            {
                byte[] bytes = File.ReadAllBytes(path);
                if (bytes.Length == 0) continue;
                byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
                File.WriteAllBytes(path, GameLZSS.CompressPfil(bytes, header));
            }
        var store = LevelObjectStore.Load(map);
        Assert.Equal(2, store.FamilyAt(0, 100)!.ObjectSlots.Count);
        var root = store.Objects().Single(item => item.Slot == 0);
        var loadedChild = store.Objects().Single(item => item.Slot == 1);
        Assert.Equal(BitConverter.SingleToInt32Bits((float)(rootDiskDegrees * (Math.PI / 180d))),
            BitConverter.SingleToInt32Bits(root.Rotation));
        Assert.Equal(BitConverter.SingleToInt32Bits(PreviewChildRawRotation), BitConverter.SingleToInt32Bits(loadedChild.Rotation));
        Assert.InRange(Math.Abs((double)loadedChild.Rotation - 1.25d), 0d, 0.0000002d);
        return root.Rotation;
    }

    private MapEditorForm FamilyPreviewForm(string map)
    {
        var form = new MapEditorForm(_root, new GameMapInfo("ENDL_012", map, true, "Family preview", "Test"));
        _ = form.Handle;
        var type = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
            new Dictionary<string, string> { ["alias"] = "BauTest" });
        typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });
        typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
            new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerChild" });
        Invoke(form, "LoadSelectedMap");
        return form;
    }

    private static IReadOnlyList<MapSceneObject> FamilyPreviewScene(MapEditorForm form)
        => (IReadOnlyList<MapSceneObject>)Invoke(form, "SceneObjectsFor3D",
            Invoke(form, "EffectiveSceneObjects")!)!;

    private static void AssertPreviewMembers(MapEditorForm form, int index)
    {
        var placement = form.PlacementSession[index];
        var members = FamilyPreviewScene(form).Where(item => item.SourceFile == SdlPlacedObjectsFile.FileName
            && item.ObjectIndex == -5000 - index).ToArray();
        Assert.Equal(2, members.Length);
        var root = Assert.Single(members, item => item.Name == "BauGerTest");
        var child = Assert.Single(members, item => item.Name == "BauGerChild");
        Assert.Equal(placement.WorldX, root.WorldX); Assert.Equal(placement.WorldY, root.WorldY);
        Assert.Equal(placement.WorldZ, root.WorldZ);
        Assert.Equal(root.WorldX + 10, child.WorldX); Assert.Equal(root.WorldY + 3.125f, child.WorldY);
        Assert.Equal(root.WorldZ + 10, child.WorldZ);
        Assert.Equal(0, root.Team); Assert.Equal(4, child.Team);
        Assert.Equal(PreviewRootState, root.NativeVisualState); Assert.Equal(PreviewChildState, child.NativeVisualState);
        Assert.Equal((float)((double)PreviewChildRawRotation * (180d / Math.PI)), child.Angle);
        Assert.Equal(root.SourceFile, child.SourceFile); Assert.Equal(root.ObjectIndex, child.ObjectIndex);
    }

    [Theory]
    [InlineData(false, false)]
    [InlineData(true, false)]
    [InlineData(false, true)]
    [InlineData(true, true)]
    public void Full_family_preview_copies_moves_undo_pick_rollback_retry_reload_and_second_save(bool pfil, bool native)
    {
        string map = CreateFixture("ENDL_012");
        float raw = SetupPreviewFamily(map, pfil, fractional: native);
        Guid sourceId = Guid.NewGuid();
        var scenario = new ScenarioDocument
        {
            Events = [new ScenarioEvent("PreviewRollback", 5) { Actions = [new(ScenarioActionKind.Message, "Test")] }]
        };
        if (!native)
        {
            scenario.Spawns.Add(new ScenarioSpawn("BauTest", 100, 200, 0, Y: 20, Prebuilt: true) { Id = sourceId });
            scenario.DataSlots.Add(new ScenarioDataSlot(0, 100) { SpawnId = sourceId });
        }
        using (var rollback = new FileRollbackScope()) { scenario.Save(map, rollback); rollback.Commit(); }
        var originalDisk = SnapshotDirectory(map);
        var originalFamily = LevelObjectStore.Load(map).CaptureFamily(0, 100)!;
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            if (native)
            {
                form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
                Assert.Equal(BitConverter.SingleToInt32Bits(raw), BitConverter.SingleToInt32Bits(form.PlacementSession[0].RawRotation!.Value));
                Assert.Equal((float)((double)raw * (180d / Math.PI)), form.PlacementSession[0].Angle);
            }
            int first = native ? 0 : form.PlacementSession.Duplicate(0);
            int second = form.PlacementSession.Duplicate(first);
            int third = form.PlacementSession.Duplicate(second);
            AssertPreviewMembers(form, first); AssertPreviewMembers(form, second); AssertPreviewMembers(form, third);
            Assert.True(form.PlacementSession.Undo());
            Assert.True(form.PlacementSession.Redo()); AssertPreviewMembers(form, third);
            var moved = form.PlacementSession[0];
            form.PlacementSession.Edit(0, moved with { WorldX = moved.WorldX + .125f, WorldY = moved.WorldY + .25f, WorldZ = moved.WorldZ + .5f });
            AssertPreviewMembers(form, 0); AssertPreviewMembers(form, second);
            Assert.True(form.PlacementSession.Undo()); Assert.Equal(moved.WorldX, form.PlacementSession[0].WorldX);
            AssertPreviewMembers(form, 0);
            Assert.True(form.PlacementSession.Redo()); AssertPreviewMembers(form, 0);
            // Pending duplicates still use the original baseline, not the subsequently moved placement.
            Assert.Equal(moved.WorldX + (native ? 256 : 512), form.PlacementSession[second].WorldX);
            var scene = FamilyPreviewScene(form);
            if (native)
            {
                Assert.Equal(2, scene.Count(item => item.SourceFile == "DATA/objects.dat"));
                Assert.Contains(scene, item => item.ObjectIndex == -100001 && item.WorldX == 110);
            }
            else Assert.DoesNotContain(scene, item => item.SourceFile == "DATA/objects.dat");
            Invoke(form, "RefreshPlacedList"); _ = form.PlacedList.Handle;
            Invoke(form, "SelectPickedSceneObject", scene.Single(item => item.ObjectIndex == -5000 - second && item.Name == "BauGerChild"));
            Assert.True(form.PlacedList.Items[second].Selected);
            AssertNativeVisualSnapshot(map, originalDisk);
            // Preview must use the loaded snapshot, even if the DATA files become unreadable.
            string objectPath = Path.Combine(map, "DATA", "objects.dat");
            byte[] objectBytes = File.ReadAllBytes(objectPath);
            try
            {
                File.WriteAllBytes(objectPath, [1, 2, 3]);
                Assert.Equal(scene, FamilyPreviewScene(form));
            }
            finally { File.WriteAllBytes(objectPath, objectBytes); }

            var cache = GetField<LevelObjectStore>(form, "_familyPreviewStore");
            Assert.False(form.TrySaveMap(false, out var error)); Assert.IsType<FileNotFoundException>(error);
            Assert.Same(cache, GetField<LevelObjectStore>(form, "_familyPreviewStore"));
            AssertNativeVisualSnapshot(map, originalDisk); AssertPreviewMembers(form, second);
            Assert.True(form.PlacementSession.CanUndo);
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.NotSame(cache, GetField<LevelObjectStore>(form, "_familyPreviewStore"));
            for (int index = 0; index < form.PlacementSession.Count; index++) AssertPreviewMembers(form, index);
            var saved = ScenarioDocument.Load(map); var store = LevelObjectStore.Load(map);
            foreach (var binding in saved.DataSlots)
            {
                Assert.Equal(2, store.FamilyAt(binding.Slot, binding.Uid)!.ObjectSlots.Count);
                Assert.Equal(PreviewRootState, store.AnimationVisualStateAt(binding.Slot, binding.Uid));
            }
            if (native)
            {
                var sourceAfter = store.CaptureFamily(0, 100)!;
                foreach (var (slot, expected) in originalFamily.Objects)
                {
                    var actual = sourceAfter.Objects[slot];
                    Assert.Equal(expected.Record, actual.Record); Assert.Equal(expected.Columns, actual.Columns);
                    Assert.Equal(expected.Position0, actual.Position0); Assert.Equal(expected.Position1, actual.Position1);
                    Assert.Equal(expected.Position2, actual.Position2); Assert.Equal(expected.AnimRecord, actual.AnimRecord);
                    Assert.Equal(expected.GfxtypeRecord, actual.GfxtypeRecord); Assert.Equal(expected.ActionRecord, actual.ActionRecord);
                    Assert.Equal(expected.AnimTail0, actual.AnimTail0); Assert.Equal(expected.AnimTail1, actual.AnimTail1);
                    Assert.Equal(expected.GfxtypeTail, actual.GfxtypeTail);
                    for (int segment = 0; segment < expected.ObjData.Length; segment++)
                        Assert.Equal(expected.ObjData[segment], actual.ObjData[segment]);
                }
                Assert.Equal(originalFamily.Lights.Keys.Order(), sourceAfter.Lights.Keys.Order());
                foreach (var (slot, light) in originalFamily.Lights)
                    Assert.Equal(light.Fields, sourceAfter.Lights[slot].Fields);
                var root = store.Objects().Single(item => item.Slot == 0);
                var child = store.Objects().Single(item => item.Slot == 1);
                Assert.Equal((100f, 20f, 200f, raw, 100u), (root.X, root.Y, root.Z, root.Rotation, root.Uid));
                Assert.Equal((110f, 23.125f, 210f, PreviewChildRawRotation, 101u), (child.X, child.Y, child.Z, child.Rotation, child.Uid));
            }
            var afterSave = SnapshotDirectory(map);
            Assert.True(form.TrySaveMap(false, out error), error?.ToString()); AssertNativeVisualSnapshot(map, afterSave);
            using var reloaded = FamilyPreviewForm(map);
            for (int index = 0; index < reloaded.PlacementSession.Count; index++) AssertPreviewMembers(reloaded, index);
            // Copies of a now saved copy resolve through its newly allocated UID/binding.
            int copiedSaved = reloaded.PlacementSession.Duplicate(second);
            AssertPreviewMembers(reloaded, copiedSaved);
            reloaded.PlacementSession.RemoveAt(0);
            Assert.DoesNotContain(FamilyPreviewScene(reloaded), item => item.SourceFile == "DATA/objects.dat"
                && !native);
            Assert.Equal((reloaded.PlacementSession.Count * 2) + (native ? 2 : 0), FamilyPreviewScene(reloaded)
                .Count(item => item.SourceFile is SdlPlacedObjectsFile.FileName or "DATA/objects.dat"));
            Assert.True(reloaded.PlacementSession.Undo()); AssertPreviewMembers(reloaded, 0);
            Assert.True(reloaded.PlacementSession.Redo());
        });
    }

    [Theory]
    [InlineData("uid")]
    [InlineData("rotation")]
    [InlineData("alias")]
    [InlineData("team")]
    [InlineData("precision")]
    public void Pending_native_copy_metadata_is_validated_before_member_projection(string fault)
    {
        string map = CreateFixture("ENDL_012"); SetupPreviewFamily(map, true, true);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            AssertPreviewMembers(form, 0);
            var copy = form.PlacementSession[0];
            var invalid = fault switch
            {
                "uid" => copy with { NativeSourceIdentity = new NativeObjectIdentity(0, 0x01000064u) },
                "rotation" => copy with { RawRotation = MathF.BitIncrement(copy.RawRotation!.Value) },
                "alias" => copy with { Type = copy.Type with { TemplateFields = new Dictionary<string, string> { ["alias"] = "ForeignAlias" } } },
                "precision" => copy with { WorldY = MathF.BitIncrement(copy.WorldY) },
                _ => copy with { Team = 1 }
            };
            // Load a tampered session to exercise the rendering boundary, bypassing UI edit guards.
            form.PlacementSession.Load([invalid]);
            var before = SnapshotDirectory(map); var scene = FamilyPreviewScene(form);
            Assert.Equal(2, scene.Count(item => item.SourceFile == "DATA/objects.dat"));
            Assert.Single(scene, item => item.SourceFile == SdlPlacedObjectsFile.FileName);
            AssertNativeVisualSnapshot(map, before);
            string other = CreateFixture("ENDL_006");
            Invoke(form, "LoadLevelObjects", other);
            Assert.Null(typeof(MapEditorForm).GetField("_familyPreviewStore", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form));
        });
    }

    [Theory]
    [InlineData("stale")]
    [InlineData("shared")]
    [InlineData("malformed")]
    public void Invalid_family_preview_falls_back_without_suppressing_foreign_native_members(string fault)
    {
        string map = CreateFixture("ENDL_012"); SetupPreviewFamily(map, false, false);
        Guid id = Guid.NewGuid();
        var scenario = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", 100, 200, 0, Y: 20, Prebuilt: true) { Id = id }],
            DataSlots = [new ScenarioDataSlot(0, fault == "stale" ? 0x01000064u : 100u) { SpawnId = id }]
        };
        if (fault == "shared")
        {
            Guid foreign = Guid.NewGuid();
            scenario.Spawns.Add(new ScenarioSpawn("BauTest", 110, 210, 4, Y: 23.125f, Prebuilt: true) { Id = foreign });
            scenario.DataSlots.Add(new ScenarioDataSlot(1, 101) { SpawnId = foreign });
        }
        if (fault == "malformed")
        {
            // ObjDataColumn is 1 in the current schema. Sharing only between members
            // is allowed; guard root 3 shares root 0's objdata from outside the family.
            AddSyntheticGuardRoots(map, 10);
            var malformed = LevelObjectStore.Load(map);
            Assert.Contains(malformed.Objects(), item => item.Slot == 3 && item.Uid == 103u);
            Assert.Null(malformed.FamilyAt(0, 100));
        }
        using (var rollback = new FileRollbackScope()) { scenario.Save(map, rollback); rollback.Commit(); }
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            form.PlacementSession.Duplicate(0);
            var before = SnapshotDirectory(map);
            var scene = FamilyPreviewScene(form);
            Assert.Contains(scene, item => item.SourceFile == "DATA/objects.dat" && item.ObjectIndex == -100000);
            Assert.Contains(scene, item => item.SourceFile == "DATA/objects.dat" && item.ObjectIndex == -100001);
            Assert.Single(scene, item => item.SourceFile == SdlPlacedObjectsFile.FileName && item.ObjectIndex == -5001);
            AssertNativeVisualSnapshot(map, before);
        });
    }
}
