using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void FamilyParticleMode_host_follows_adopted_snapshot_and_rejects_stale_uid_and_context()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        string data = Path.Combine(map, "DATA");
        File.WriteAllText(Path.Combine(_root, "SelectedDefs", "objdef.d09"), "[ObjectDefaults]\n227,1,1000,0,0,258,0,3\n");
        byte[] objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
        int link = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + 73));
        byte[] gfx = File.ReadAllBytes(Path.Combine(data, "gfxtype.dat"));
        int count = BinaryPrimitives.ReadInt32LittleEndian(gfx.AsSpan(4));
        BinaryPrimitives.WriteInt16LittleEndian(gfx.AsSpan(8 + count * 15 + link * 2), 99);
        File.WriteAllBytes(Path.Combine(data, "gfxtype.dat"), gfx);
        var disk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            Assert.True(form.TryResolveSavedFamilyParticleMode(0, 100, 0, 3, out var mode, out var reason));
            Assert.Null(reason);
            object resolved = mode!;
            Assert.Equal(99, resolved.GetType().GetProperty("RawMode")!.GetValue(resolved));
            Assert.Equal(3, resolved.GetType().GetProperty("ResolvedMode")!.GetValue(resolved));
            Assert.Equal(0x4caf80u, resolved.GetType().GetProperty("LeafAddress")!.GetValue(resolved));
            Assert.True(form.TryResolveSavedFamilyParticleMode(0, 100, 0, out mode, out reason));
            Assert.Null(reason); Assert.Equal(3, mode!.GetType().GetProperty("ResolvedMode")!.GetValue(mode));
            AdoptFamilyParticleTestAsset(form);
            var modeImage = Assert.Single(form.DecodeSavedParticlePreviewImages(0));
            Assert.Null(modeImage.ModeDiagnostic);
            object imageMode = modeImage.GetType().GetProperty("Mode")!.GetValue(modeImage)!;
            Assert.Equal(3, imageMode.GetType().GetProperty("ResolvedMode")!.GetValue(imageMode));
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "objdef.d09"), "[ObjectDefaults]\n227,1,1000,0,0,258,0,7\n");
            Invoke(form, "LoadSelectedMap");
            Assert.True(form.TryResolveSavedFamilyParticleMode(0, 100, 0, out mode, out reason));
            Assert.Null(reason); Assert.Equal(7, mode!.GetType().GetProperty("ResolvedMode")!.GetValue(mode));
            modeImage = Assert.Single(form.DecodeSavedParticlePreviewImages(0));
            Assert.Null(modeImage.ModeDiagnostic);
            imageMode = modeImage.GetType().GetProperty("Mode")!.GetValue(modeImage)!;
            Assert.Equal(7, imageMode.GetType().GetProperty("ResolvedMode")!.GetValue(imageMode));
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "objdef.d09"), "[ObjectDefaults]\n227,1,1000,0,0,258,0,99\n");
            Invoke(form, "LoadSelectedMap");
            modeImage = Assert.Single(form.DecodeSavedParticlePreviewImages(0));
            Assert.Contains("unknown", modeImage.ModeDiagnostic);
            imageMode = modeImage.GetType().GetProperty("Mode")!.GetValue(modeImage)!;
            Assert.Null(imageMode.GetType().GetProperty("LeafAddress")!.GetValue(imageMode));
            Assert.False(form.TryResolveSavedFamilyParticleMode(0, 100, 0, null, out mode, out reason));
            Assert.Null(mode); Assert.Contains("context", reason);
            Assert.False(form.TryResolveSavedFamilyParticleMode(0, 100, 1, 3, out mode, out reason));
            Assert.Null(mode); Assert.Contains("family", reason);
            var field = typeof(MapEditorForm).GetField("_levelObjects", BindingFlags.NonPublic | BindingFlags.Instance)!;
            var original = ((IReadOnlyList<LevelWorldObject>)field.GetValue(form)!).ToArray();
            field.SetValue(form, original.Select(item => item.Slot == 0 ? item with { Uid = 999 } : item).ToArray());
            Assert.False(form.TryResolveSavedFamilyParticleMode(0, 100, 0, 3, out mode, out reason));
            Assert.Null(mode); Assert.Contains("UID", reason);
            Assert.False(form.TryResolveSavedFamilyParticleMode(0, 100, 0, out mode, out reason));
            Assert.Null(mode); Assert.Contains("UID", reason);
        });
        var after = SnapshotDirectory(map); foreach (string path in disk.Keys) Assert.Equal(disk[path], after[path]);
    }
    private static int FamilyParticleX(SavedFamilyParticleFrame frame)
    {
        object child = frame.GetType().GetProperty("Child")!.GetValue(frame)!;
        return (int)child.GetType().GetProperty("X")!.GetValue(child)!;
    }

    [Fact]
    public void FamilyParticleFrames_scene_native_copy_move_duplicate_undo_uses_original_baseline()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        var disk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map); AdoptFamilyParticleTestAsset(form);
            byte[] pool = SavedParticleHandle(form).CopyPayload();
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            _ = FamilyPreviewScene(form);
            Guid first = form.PlacementSession[0].ScenarioId;
            Assert.Equal(2, form.SavedFamilyParticleFrames.Count);
            int firstX = FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames, f => f.TargetScenarioId == first));
            int secondIndex = form.PlacementSession.Duplicate(0);
            Guid second = form.PlacementSession[secondIndex].ScenarioId;
            _ = FamilyPreviewScene(form);
            Assert.Equal(3, form.SavedFamilyParticleFrames.Count);
            int secondX = FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames, f => f.TargetScenarioId == second));
            var target = form.PlacementSession[0];
            form.PlacementSession.Edit(0, target with { WorldX = 100.125f, WorldY = 20.25f, WorldZ = 200.5f });
            _ = FamilyPreviewScene(form);
            Assert.Equal(-1, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames, f => f.TargetScenarioId == first)));
            Assert.Equal(secondX, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames, f => f.TargetScenarioId == second)));
            Assert.Equal(-2, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames, f => f.TargetScenarioId == Guid.Empty)));
            var movedImages = form.DecodeSavedParticlePreviewImages(0);
            Assert.Equal(3, movedImages.Count);
            Assert.All(movedImages, image => Assert.Null(image.Diagnostic));
            Assert.All(movedImages, image => Assert.Null(image.ModeDiagnostic));
            Assert.All(movedImages, image =>
            {
                Assert.Null(image.GeometryDiagnostic);
                object rect = image.GetType().GetProperty("Rect")!.GetValue(image)!;
                Assert.NotNull(rect);
                Assert.Equal(-6, rect.GetType().GetProperty("Left")!.GetValue(rect));
                Assert.Equal(-9, rect.GetType().GetProperty("Top")!.GetValue(rect));
                Assert.Equal(-4, rect.GetType().GetProperty("Right")!.GetValue(rect));
                Assert.Equal(-8, rect.GetType().GetProperty("Bottom")!.GetValue(rect));
            });
            object sharedMode = movedImages[0].GetType().GetProperty("Mode")!.GetValue(movedImages[0])!;
            Assert.All(movedImages, image => Assert.Same(sharedMode, image.GetType().GetProperty("Mode")!.GetValue(image)));
            Assert.Equal(-1, FamilyParticleX(Assert.Single(movedImages, image => image.Candidate.TargetScenarioId == first).Candidate));
            var particleHeights = new TerrainHeightField(2, 2, new byte[4], nativeHeightMapStep: 64);
            var particleView = System.Numerics.Matrix4x4.CreateTranslation(0, 0, -10);
            var batch = form.CaptureSavedParticleRenderBatch(0, particleHeights, particleView);
            Assert.Empty(batch.Diagnostics); Assert.Equal(3, batch.Draws.Count);
            float[] objects = new float[96];
            for (int vertex = 0; vertex < 12; vertex++) objects[vertex * 8 + 2] = vertex < 6 ? -5 : 5;
            var combined = SceneCombinedSpritePlan.Build(objects, batch, particleView);
            Assert.Equal(5, combined.Count);
            Assert.Equal(SceneSpriteDrawKind.Object, combined[0].Kind); Assert.Equal(0, combined[0].FirstVertex);
            Assert.Equal(SceneSpriteDrawKind.Object, combined[4].Kind); Assert.Equal(6, combined[4].FirstVertex);
            Assert.Same(objects, combined[0].Vertices); Assert.Same(objects, combined[4].Vertices);
            Assert.All(combined.Skip(1).Take(3), draw =>
            {
                Assert.Equal(SceneSpriteDrawKind.ParticleReplacement, draw.Kind);
                Assert.Same(batch.Draws[draw.SourceIndex].Vertices, draw.Vertices);
            });
            Assert.Throws<InvalidDataException>(() => SceneCombinedSpritePlan.Build(new float[1], batch, particleView));
            Assert.Empty(SceneCombinedSpritePlan.Build(Array.Empty<float>(),
                batch with { Draws = Array.Empty<ParticleSpriteDraw>() }, particleView));
            objects[2] = float.NaN;
            Assert.Throws<InvalidDataException>(() => SceneCombinedSpritePlan.Build(objects, batch, particleView));
            Assert.All(batch.Draws, draw =>
            {
                Assert.Equal(48, draw.Vertices.Length);
                Assert.Equal(FamilyParticleX(draw.Source.Candidate) / 256f, draw.Vertices[0]);
                Assert.Equal(-6 * SceneObjectRenderer.TilesPerSpritePixel, draw.Vertices[3]);
                Assert.Equal(8 * SceneObjectRenderer.TilesPerSpritePixel, draw.Vertices[4]);
            });
            var hidden = SceneParticleRenderer.Build(movedImages, particleHeights,
                System.Numerics.Matrix4x4.CreateTranslation(0, 0, 10));
            Assert.Empty(hidden.Draws);
            var noNativeHeight = SceneParticleRenderer.Build(movedImages, new TerrainHeightField(2, 2, new byte[4]), particleView);
            Assert.Empty(noNativeHeight.Draws); Assert.Equal(3, noNativeHeight.Diagnostics.Count);
            var unresolved = SceneParticleRenderer.Build(movedImages.Select(image => image with
                { ModeDiagnostic = "Unknown native draw leaf." }).ToArray(), particleHeights, particleView);
            Assert.Empty(unresolved.Draws); Assert.Equal(3, unresolved.Diagnostics.Count);
            Assert.True(form.PlacementSession.Undo()); _ = FamilyPreviewScene(form);
            Assert.Equal(firstX, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames, f => f.TargetScenarioId == first)));
            Assert.True(form.PlacementSession.Undo()); _ = FamilyPreviewScene(form); Assert.Equal(2, form.SavedFamilyParticleFrames.Count);
            Assert.True(form.PlacementSession.Redo()); _ = FamilyPreviewScene(form); Assert.Equal(3, form.SavedFamilyParticleFrames.Count);
            Assert.Equal(pool, SavedParticleHandle(form).CopyPayload());
        });
        var after = SnapshotDirectory(map); foreach (string path in disk.Keys) Assert.Equal(disk[path], after[path]);
    }

    [Fact]
    public void FamilyParticleFrames_scene_bound_move_delete_and_undo_replace_baseline_once()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        Guid id = Guid.NewGuid();
        var scenario = new AgainstRomeModifier.Scripting.ScenarioDocument();
        scenario.Spawns.Add(new AgainstRomeModifier.Scripting.ScenarioSpawn("BauTest", 100, 200, 0, Y: 20, Prebuilt: true) { Id = id });
        scenario.DataSlots.Add(new AgainstRomeModifier.Scripting.ScenarioDataSlot(0, 100) { SpawnId = id });
        using (var rollback = new FileRollbackScope()) { scenario.Save(map, rollback); rollback.Commit(); }
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map); AdoptFamilyParticleTestAsset(form);
            _ = FamilyPreviewScene(form);
            Assert.Equal(id, Assert.Single(form.SavedFamilyParticleFrames).TargetScenarioId);
            var target = form.PlacementSession[0];
            form.PlacementSession.Edit(0, target with { WorldX = 100.125f });
            _ = FamilyPreviewScene(form); Assert.Equal(-1, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames)));
            form.PlacementSession.RemoveAt(0); _ = FamilyPreviewScene(form); Assert.Empty(form.SavedFamilyParticleFrames);
            Assert.True(form.PlacementSession.Undo()); _ = FamilyPreviewScene(form);
            Assert.Equal(-1, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames)));
            Assert.True(form.PlacementSession.Undo()); _ = FamilyPreviewScene(form);
            Assert.Equal(-2, FamilyParticleX(Assert.Single(form.SavedFamilyParticleFrames)));
        });
    }

    private void SetupFamilyParticleFrameFixture(string map)
    {
        SetupSavedLightHost(map, true); SetupParticleDefinitionSources();
        string objects = Path.Combine(_root, "SelectedDefs", "objdef.dau");
        File.WriteAllText(objects, File.ReadAllText(objects).Replace("\n830,", "\n555,"));
        string path = Path.Combine(map, "DATA", "particle.dat");
        byte[] bytes = File.ReadAllBytes(path);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(14), 6);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(24 + 5 * 35 + 25), 14001);
        File.WriteAllBytes(path, bytes);
    }

    [Fact]
    public void FamilyParticleFrames_images_refresh_definitions_and_do_not_decode_unverified_velocity()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map); AdoptFamilyParticleTestAsset(form);
            Assert.Null(Assert.Single(form.DecodeSavedParticlePreviewImages(0)).Diagnostic);
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "partpar.d09"),
                "[ParticleParameterDefault]\n22,1,N,64,15000,0,0,0,0,0,0,0,0,0,1\n");
            Invoke(form, "LoadSelectedMap");
            var unavailable = Assert.Single(form.DecodeSavedParticlePreviewImages(0));
            Assert.Null(unavailable.GetType().GetProperty("Image")!.GetValue(unavailable));
            Assert.Contains("241", unavailable.Diagnostic);
        });
    }

    private static void AdoptFamilyParticleTestAsset(MapEditorForm form)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        foreach (uint word in new uint[] { 0x41524C41, 6, 0, 4, 8, 0, 4, 1, 1, 0,
            0, 0, 0, 0, 0, 12, 18, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
        writer.Write(-1); writer.Write(12u); writer.Write(0u);
        writer.Write(2u | (1u << 11) | (2u << 22)); writer.Write(2u);
        writer.Write(0x102030u); writer.Write(0x405060u); writer.Write(new byte[] { 1, 0, 0, 0 });
        writer.Write(0u); writer.Write(2u); writer.Write(0); writer.Write(0); writer.Write(0);
        byte[] bytes = stream.ToArray();
        var field = typeof(MapEditorForm).GetField("_spriteCatalog", BindingFlags.NonPublic | BindingFlags.Instance)!;
        (field.GetValue(form) as IDisposable)?.Dispose();
        field.SetValue(form, field.FieldType.GetMethod("FromText", BindingFlags.Public | BindingFlags.Static)!
            .Invoke(null, new object?[] { "", "258,particle.alr\n", "", (Func<string, byte[]?>)(_ => bytes),
                (Func<string, byte[]?>)(_ => null), null }));
    }

    [Fact]
    public void FamilyParticleFrames_capture_closed_family_baseline_and_scene_refresh_without_writes()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        var disk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map); AdoptFamilyParticleTestAsset(form);
            Assert.True(form.TryCaptureSavedFamilyParticleFrames(0, 100, out var frames, out var reason));
            Assert.Null(reason); var frame = Assert.Single(frames);
            Assert.Equal((0, 0, 0, 555), (frame.RootSlot, frame.EmitterSlot, frame.OwnerSlot, frame.OwnerType));
            object child = frame.GetType().GetProperty("Child")!.GetValue(frame)!;
            Assert.Equal(5u, child.GetType().GetProperty("Ordinal")!.GetValue(child));
            Assert.Equal(3u, child.GetType().GetProperty("Frame")!.GetValue(child));
            Assert.Equal(-2, child.GetType().GetProperty("X")!.GetValue(child));
            byte[] baseline = SavedParticleHandle(form).CopyPayload();
            Assert.True(form.TryCaptureSavedFamilyParticleFrames(0, 100, out var moved, out reason, 1, 0, 0));
            object movedChild = Assert.Single(moved).GetType().GetProperty("Child")!.GetValue(moved[0])!;
            Assert.Equal(-1, movedChild.GetType().GetProperty("X")!.GetValue(movedChild));
            Assert.Equal(3u, movedChild.GetType().GetProperty("Frame")!.GetValue(movedChild));
            Assert.Equal(baseline, SavedParticleHandle(form).CopyPayload());
            _ = FamilyPreviewScene(form);
            Assert.Single(form.SavedFamilyParticleFrames); Assert.Null(form.SavedFamilyParticleFrameDiagnostic);
            var image = Assert.Single(form.DecodeSavedParticlePreviewImages(0));
            Assert.Null(image.Diagnostic);
            object asset = image.GetType().GetProperty("Image")!.GetValue(image)!;
            Assert.Equal(3u, asset.GetType().GetProperty("FrameIndex")!.GetValue(asset));
            object pixels = asset.GetType().GetProperty("Image")!.GetValue(asset)!;
            Assert.Equal(new uint[] { 0xFF605040, 0 },
                (IReadOnlyList<uint>)pixels.GetType().GetProperty("ArgbPixels")!.GetValue(pixels)!);
            var badPalette = Assert.Single(form.DecodeSavedParticlePreviewImages(1));
            Assert.Null(badPalette.GetType().GetProperty("Image")!.GetValue(badPalette));
            Assert.Contains("palette", badPalette.Diagnostic);
            form.Dispose(); Assert.Empty(form.SavedFamilyParticleFrames);
        });
        var after = SnapshotDirectory(map); Assert.Equal(disk.Keys.Order(), after.Keys.Order());
        foreach (string path in disk.Keys) Assert.Equal(disk[path], after[path]);
    }

    [Fact]
    public void FamilyParticleFrames_reject_stale_owner_uid_type_and_failed_baseline_reload()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map); AdoptFamilyParticleTestAsset(form);
            var field = typeof(MapEditorForm).GetField("_levelObjects", BindingFlags.NonPublic | BindingFlags.Instance)!;
            var original = ((IReadOnlyList<LevelWorldObject>)field.GetValue(form)!).ToArray();
            foreach (var replacement in new[] { original[0] with { Uid = 999 }, original[0] with { TypeId = 830 } })
            {
                field.SetValue(form, original.Select(o => o.Slot == replacement.Slot ? replacement : o).ToArray());
                Assert.False(form.TryCaptureSavedFamilyParticleFrames(0, 100, out var frames, out var reason));
                Assert.Empty(frames); Assert.Contains("identity", reason);
            }
            field.SetValue(form, original);
            Assert.False(form.TryCaptureSavedFamilyParticleFrames(0, 999, out _, out _));
            typeof(MapEditorForm).GetField("_savedParticleLoadDiagnostic", BindingFlags.NonPublic | BindingFlags.Instance)!
                .SetValue(form, "Saved particle baseline load failed.");
            Assert.False(form.TryCaptureSavedFamilyParticleFrames(0, 100, out var result, out var diagnostic));
            Assert.Empty(result); Assert.Contains("load failed", diagnostic);
            _ = FamilyPreviewScene(form); Assert.Empty(form.SavedFamilyParticleFrames);
            Assert.Contains("load failed", form.SavedFamilyParticleFrameDiagnostic);
        });
    }

    [Fact]
    public void FamilyParticleFrames_reject_changed_record_from_another_closed_snapshot_atomically()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map); AdoptFamilyParticleTestAsset(form);
            Assert.True(form.TryCaptureSavedFamilyParticleFrames(0, 100, out _, out _));
            string path = Path.Combine(map, "DATA", "particle.dat");
            byte[] bytes = File.ReadAllBytes(path);
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(24 + 5 * 35 + 1), 42);
            File.WriteAllBytes(path, bytes); // Deliberately construct a distinct TEMP source snapshot.
            var changed = LevelObjectStore.Load(map); Assert.NotNull(changed.CaptureFamily(0, 100));
            typeof(MapEditorForm).GetField("_familyPreviewStore", BindingFlags.NonPublic | BindingFlags.Instance)!
                .SetValue(form, changed);
            Assert.False(form.TryCaptureSavedFamilyParticleFrames(0, 100, out var frames, out var reason));
            Assert.Empty(frames); Assert.Contains("does not match", reason);
            _ = FamilyPreviewScene(form); Assert.Empty(form.SavedFamilyParticleFrames);
            Assert.Contains("does not match", form.SavedFamilyParticleFrameDiagnostic);
            Assert.Equal(bytes, File.ReadAllBytes(path));
        });
    }
}
