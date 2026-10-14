using System.Buffers.Binary;
using System.Drawing;
using System.Numerics;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    private static int SavedLightTestOffset(int column, int slot)
    {
        int[] widths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
        return 8 + widths.Take(column).Sum() * 1024 + slot * widths[column];
    }

    private static void SavedLightTestFloat(byte[] bytes, int column, int slot, float value)
        => BinaryPrimitives.WriteSingleLittleEndian(bytes.AsSpan(SavedLightTestOffset(column, slot)), value);

    private static void SetupSavedLightHost(string map, bool particle)
    {
        SetupPreviewFamily(map, false, false);
        string data = Path.Combine(map, "DATA");
        byte[] lights = File.ReadAllBytes(Path.Combine(data, "light.dat"));
        SavedLightTestFloat(lights, 4, 0, 2.5f);
        SavedLightTestFloat(lights, 5, 0, .25f);
        SavedLightTestFloat(lights, 6, 0, .5f);
        SavedLightTestFloat(lights, 7, 0, 400);
        SavedLightTestFloat(lights, 8, 0, 100);
        if (particle)
        {
            // Independent linked-light slot; source XYZ differs from child fixed XYZ on purpose.
            for (int column = 0; column < 16; column++)
            {
                int width = column is 9 or 13 or 14 or 15 ? 2 : 4;
                lights.AsSpan(SavedLightTestOffset(column, 0), width).CopyTo(lights.AsSpan(SavedLightTestOffset(column, 1), width));
            }
            BinaryPrimitives.WriteInt16LittleEndian(lights.AsSpan(SavedLightTestOffset(13, 1)), -1);
            byte[] emitters = File.ReadAllBytes(Path.Combine(data, "particle.dat"));
            BinaryPrimitives.WriteUInt16LittleEndian(emitters.AsSpan(12), 1);
            BinaryPrimitives.WriteInt16LittleEndian(emitters.AsSpan(12 + 6), 0);
            int child = 12 + 12 + 5 * 35;
            emitters[child] = 1;
            BinaryPrimitives.WriteInt32LittleEndian(emitters.AsSpan(child + 1), -65537);
            BinaryPrimitives.WriteInt32LittleEndian(emitters.AsSpan(child + 5), 30 * 65536 + 5);
            BinaryPrimitives.WriteInt32LittleEndian(emitters.AsSpan(child + 9), 210 * 65536 + 9);
            BinaryPrimitives.WriteInt16LittleEndian(emitters.AsSpan(child + 33), 1);
            File.WriteAllBytes(Path.Combine(data, "particle.dat"), emitters);
            byte[] objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(11, 0, 10)), 0);
            File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
        }
        File.WriteAllBytes(Path.Combine(data, "light.dat"), lights);
        Assert.NotNull(LevelObjectStore.Load(map).CaptureFamily(0, 100));
    }

    private static SavedSceneLightSet SavedLightHostScene(MapEditorForm form)
    {
        _ = FamilyPreviewScene(form);
        return form.SavedLightPreview;
    }

    [NativeParticleProjectionFact]
    public void SavedLight_host_native_copy_copyoriginal_sameposition_move_undo_reload_and_late_save_failure()
    {
        string map = CreateFixture("ENDL_012");
        SetupSavedLightHost(map, true);
        var scenario = new ScenarioDocument
        {
            Events = [new ScenarioEvent("SavedLightRollback", 5) { Actions = [new(ScenarioActionKind.Message, "Test")] }]
        };
        using (var rollback = new FileRollbackScope()) { scenario.Save(map, rollback); rollback.Commit(); }
        var disk = SnapshotDirectory(map);
        var source = LevelObjectStore.Load(map).CaptureFamily(0, 100)!;
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            var handle = GetField<RustLightPool>(form, "_savedLightPool");
            byte[] baseline = handle.CopyPayload();
            Assert.Equal(2, SavedLightHostScene(form).Instances.Length);
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            int second = form.PlacementSession.Duplicate(0);
            Assert.Equal(form.PlacementSession[0].NativeSourceIdentity, form.PlacementSession[second].NativeSourceIdentity);
            var target = form.PlacementSession[0];
            form.PlacementSession.Edit(second, form.PlacementSession[second] with
            { WorldX = target.WorldX, WorldY = target.WorldY, WorldZ = target.WorldZ });
            var set = SavedLightHostScene(form);
            Assert.Equal(6, set.Instances.Length); // Two source + two per copy, including same-position instances.
            Assert.Equal(2, set.Instances.Count(light => light.SavedSlot == 0 && light.WorldPosition.X == target.WorldX));
            var direct = set.Instances.First(light => light.SavedSlot == 0);
            Assert.Equal(new Vector3(2.5f, .25f, .5f), direct.Color);
            Assert.Equal(400, direct.Radius); Assert.Equal(100, direct.BaselineRadius);
            Assert.Contains(new SceneLightSuppressionKey("DATA/objects.dat", -100000), set.SuppressedObjects);
            Assert.Contains(new SceneLightSuppressionKey(SdlPlacedObjectsFile.FileName, -5000 - second), set.SuppressedObjects);
            float dx = target.WorldX - 100;
            float dz = target.WorldZ - 200;
            Assert.Equal(2, set.Instances.Count(light => light.SavedSlot == 1
                && light.WorldPosition == new Vector3((int)(-65537 + dx * 65536) >> 16, 30, (int)(210 * 65536 + 9 + dz * 65536) >> 16)));
            form.PlacementSession.RemoveAt(second);
            Assert.Equal(4, SavedLightHostScene(form).Instances.Length);
            Assert.True(form.PlacementSession.Undo()); Assert.Equal(6, SavedLightHostScene(form).Instances.Length);
            Assert.Equal(baseline, handle.CopyPayload()); AssertNativeVisualSnapshot(map, disk);
            Assert.False(form.TrySaveMap(false, out var error)); Assert.IsType<FileNotFoundException>(error);
            Assert.Same(handle, GetField<RustLightPool>(form, "_savedLightPool"));
            Assert.Equal(baseline, handle.CopyPayload()); AssertNativeVisualSnapshot(map, disk);
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.True(handle.IsDisposed);
            Assert.Equal(6, SavedLightHostScene(form).Instances.Length);
            var after = LevelObjectStore.Load(map).CaptureFamily(0, 100)!;
            foreach (var pair in source.Lights) Assert.Equal(pair.Value.Fields, after.Lights[pair.Key].Fields);
            foreach (var pair in source.Particles) Assert.Equal(pair.Value.RecordBytes(), after.Particles[pair.Key].RecordBytes());
            using var reloaded = FamilyPreviewForm(map);
            Assert.Equal(6, SavedLightHostScene(reloaded).Instances.Length);
            int third = reloaded.PlacementSession.Duplicate(0);
            Assert.Equal(8, SavedLightHostScene(reloaded).Instances.Length);
            reloaded.PlacementSession.RemoveAt(third);
            Assert.Equal(6, SavedLightHostScene(reloaded).Instances.Length);
        });
    }

    [NativeParticleProjectionFact]
    public void SavedLight_host_retained_move_delete_undo_and_late_particle_failure_are_atomic()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        Guid id = Guid.NewGuid();
        var scenario = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", 100, 200, 0, Y: 20, Prebuilt: true) { Id = id }],
            DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = id }]
        };
        using (var rollback = new FileRollbackScope()) { scenario.Save(map, rollback); rollback.Commit(); }
        var disk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            var old = form.PlacementSession[0];
            form.PlacementSession.Edit(0, old with { WorldX = old.WorldX + .125f, WorldY = old.WorldY + .25f });
            var set = SavedLightHostScene(form);
            Assert.Equal(2, set.Instances.Length);
            Assert.Equal(new Vector3(100.125f, 20.25f, 200), Assert.Single(set.Instances, light => light.SavedSlot == 0).WorldPosition);
            Assert.Equal(new Vector3(-1, 30, 210), Assert.Single(set.Instances, light => light.SavedSlot == 1).WorldPosition);
            form.PlacementSession.RemoveAt(0); Assert.Empty(SavedLightHostScene(form).Instances);
            Assert.True(form.PlacementSession.Undo()); Assert.Equal(2, SavedLightHostScene(form).Instances.Length);
            // Direct float candidate succeeds, then Rust particle checked fixed addition overflows.
            form.PlacementSession.Edit(0, old with { WorldY = old.WorldY + 32767 });
            var failed = SavedLightHostScene(form);
            Assert.NotNull(form.SavedLightPreviewDiagnostic);
            Assert.Equal(2, failed.Instances.Length);
            Assert.All(failed.Instances, light => Assert.Equal(new Vector3(100, 20, 200), light.WorldPosition));
            Assert.DoesNotContain(new SceneLightSuppressionKey(SdlPlacedObjectsFile.FileName, -5000), failed.SuppressedObjects);
            Assert.Equal(2, FamilyPreviewScene(form).Count(item => item.SourceFile == "DATA/objects.dat"));
            AssertNativeVisualSnapshot(map, disk);
        });
    }

    [NativeProjectionFact]
    public void SavedLight_host_invalid_reload_preserves_preview_and_private_handle_is_disposed()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, false);
        RunInSta(() =>
        {
            var form = FamilyPreviewForm(map);
            var handle = GetField<RustLightPool>(form, "_savedLightPool");
            var baseline = SavedLightHostScene(form).Instances;
            string path = Path.Combine(map, "DATA", "light.dat"); byte[] original = File.ReadAllBytes(path);
            try
            {
                File.WriteAllBytes(path, [1, 2, 3]); Invoke(form, "LoadSavedLights", map);
                Assert.NotNull(form.SavedLightPreviewDiagnostic);
                Assert.Same(handle, GetField<RustLightPool>(form, "_savedLightPool"));
                Assert.Equal(baseline.ToArray(), SavedLightHostScene(form).Instances.ToArray());
                Assert.False(handle.IsDisposed);
            }
            finally { File.WriteAllBytes(path, original); form.Dispose(); }
            Assert.True(handle.IsDisposed);
        });
    }

    [NativeProjectionFact]
    public void SavedLight_host_actualGL_reads_real_Rust_RGB_effective_nativeY_and_relief0()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            var form = Assert.IsType<MapEditorForm>(view.FindForm());
            string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, false);
            string path = Path.Combine(map, "DATA", "light.dat"); byte[] bytes = File.ReadAllBytes(path);
            SavedLightTestFloat(bytes, 1, 0, 32 * SavedLightTile);
            SavedLightTestFloat(bytes, 2, 0, SavedLightNativeGroundY);
            SavedLightTestFloat(bytes, 3, 0, 32 * SavedLightTile);
            SavedLightTestFloat(bytes, 5, 0, 0); SavedLightTestFloat(bytes, 6, 0, 0);
            File.WriteAllBytes(path, bytes); Invoke(form, "LoadSavedLights", map);
            byte[] original = File.ReadAllBytes(path);
            view.GameLightingEnabled = true; view.GameHour = 0;
            view.UpdateSavedSceneLights(null);
            using var ambientFrame = view.CaptureFrame(640, 480)!;
            Color ambient = SavedLightPixel(ambientFrame, camera, 33.25f, 32.25f, 1);
            view.UpdateSavedSceneLights(form.SavedLightPreview);
            using var litFrame = view.CaptureFrame(640, 480)!;
            Color lit = SavedLightPixel(litFrame, camera, 33.25f, 32.25f, 1);
            Assert.True(lit.R > ambient.R * 2); Assert.InRange(Math.Abs(lit.G - ambient.G), 0, 4);
            view.SetReliefScale(0);
            using var flatFrame = view.CaptureFrame(640, 480)!;
            Color flat = SavedLightPixel(flatFrame, camera, 33.25f, 32.25f, 0);
            Assert.InRange(Math.Abs(flat.R - lit.R), 0, 6);
            Assert.Equal(original, File.ReadAllBytes(path));
        });
    }
}
