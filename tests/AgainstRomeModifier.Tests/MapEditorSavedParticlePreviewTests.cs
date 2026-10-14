using System.Buffers.Binary;
using System.Numerics;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

internal sealed class SavedParticlePreviewFactAttribute : FactAttribute
{
    public SavedParticlePreviewFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustParticlePool.TryEnsureLoaded(out var why)
            || !RustParticleLightProjection.TryEnsureLoaded(out why)
            || !RustNativeData.TryEnsureProjectionLoaded(out why))
            Skip = "Saved particle preview native ABI unavailable: " + why;
    }
}

public sealed partial class MapEditorSaveTransactionTests
{
    private static RustParticlePool SavedParticleHandle(MapEditorForm form)
        => GetField<RustParticlePool>(form, "_savedParticlePool");

    private static void AssertParticleCandidateRejected(MapEditorForm form)
    {
        var set = SavedLightHostScene(form);
        Assert.NotNull(form.SavedLightPreviewDiagnostic);
        Assert.Contains("particle", form.SavedLightPreviewDiagnostic!, StringComparison.OrdinalIgnoreCase);
        Assert.Equal(2, set.Instances.Length);
        Assert.DoesNotContain(new SceneLightSuppressionKey(SdlPlacedObjectsFile.FileName, -5000), set.SuppressedObjects);
        Assert.Equal(2, FamilyPreviewScene(form).Count(item => item.SourceFile == "DATA/objects.dat"));
        Assert.Single(FamilyPreviewScene(form), item => item.SourceFile == SdlPlacedObjectsFile.FileName);
    }

    [SavedParticlePreviewFact]
    public void SavedParticle_host_copy_move_undo_uses_immutable_Rust_baseline_and_all_children()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        string path = Path.Combine(map, "DATA", "particle.dat");
        byte[] emitters = File.ReadAllBytes(path);
        // Move the link to the final inactive child; CountRaw is zero. Rust must
        // preserve its light's saved float position, not invent child/sprite data.
        int first = 12 + 12 + 5 * 35, last = 12 + 12 + 63 * 35;
        emitters.AsSpan(first, 35).CopyTo(emitters.AsSpan(last, 35));
        emitters[last] = 0;
        BinaryPrimitives.WriteInt16LittleEndian(emitters.AsSpan(first + 33), -1);
        BinaryPrimitives.WriteInt32LittleEndian(emitters.AsSpan(12 + 1024 * 2252), 0);
        File.WriteAllBytes(path, emitters);
        var disk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            var pool = SavedParticleHandle(form);
            byte[] baseline = pool.CopyPayload();
            var saved = pool.Capture(0)!;
            Assert.Equal(0, saved.CountRaw);
            Assert.Equal(0, pool.ChildAny(0, 63)[0]);
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            AssertPreviewMembers(form, 0);
            Assert.Equal(4, SavedLightHostScene(form).Instances.Length);
            var copy = form.PlacementSession[0];
            form.PlacementSession.Edit(0, copy with { WorldX = copy.WorldX + .125f, WorldY = copy.WorldY + .25f });
            AssertPreviewMembers(form, 0);
            var lights = SavedLightHostScene(form);
            Assert.Equal(4, lights.Instances.Length);
            Assert.Equal(2, lights.Instances.Count(light => light.SavedSlot == 1
                && light.WorldPosition == new Vector3(100, 20, 200)));
            Assert.Contains(new SceneLightSuppressionKey(SdlPlacedObjectsFile.FileName, -5000), lights.SuppressedObjects);
            Assert.True(form.PlacementSession.Undo());
            Assert.Equal(baseline, pool.CopyPayload());
            Assert.Equal(saved.RecordBytes(), pool.Capture(0)!.RecordBytes());
            Assert.Equal(saved.StateBytes(), pool.Capture(0)!.StateBytes());
            AssertNativeVisualSnapshot(map, disk);
        });
    }

    [SavedParticlePreviewFact]
    public void SavedParticle_host_active_link_projection_and_committed_save_replace_baseline_without_mutating_source()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        var source = LevelObjectStore.Load(map).CaptureFamily(0, 100)!;
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            var handle = SavedParticleHandle(form);
            byte[] baseline = handle.CopyPayload();
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            var copy = form.PlacementSession[0];
            form.PlacementSession.Edit(0, copy with { WorldX = 101.125f, WorldY = 20.25f, WorldZ = 202.5f });
            AssertPreviewMembers(form, 0);
            Assert.Contains(SavedLightHostScene(form).Instances, light => light.SavedSlot == 1
                && light.WorldPosition == new Vector3(0, 30, 212));
            Assert.Equal(baseline, handle.CopyPayload());
            Assert.True(form.TrySaveMap(false, out var error), error?.ToString());
            Assert.True(handle.IsDisposed);
            Assert.NotSame(handle, SavedParticleHandle(form));
            var after = LevelObjectStore.Load(map).CaptureFamily(0, 100)!;
            Assert.Equal(source.Particles[0].RecordBytes(), after.Particles[0].RecordBytes());
            Assert.Equal(source.Particles[0].StateWords(), after.Particles[0].StateWords());
            Assert.Equal(4, SavedLightHostScene(form).Instances.Length);
            Assert.Null(form.SavedLightPreviewDiagnostic);
        });
    }

    [SavedParticlePreviewFact]
    public void SavedParticle_host_failed_same_map_reload_keeps_handle_and_rejects_divergent_record_state_active_and_owner()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        string path = Path.Combine(map, "DATA", "particle.dat");
        byte[] original = File.ReadAllBytes(path);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            var handle = SavedParticleHandle(form);
            byte[] baseline = handle.CopyPayload();
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            Assert.Equal(4, SavedLightHostScene(form).Instances.Length);
            // A failed Rust reload with the identical closed store is still safe.
            var identicalStore = LevelObjectStore.Load(map);
            File.WriteAllBytes(path, [1, 2, 3]);
            Invoke(form, "ClearFamilyPreview");
            Invoke(form, "LoadFamilyPreview", identicalStore, map);
            Assert.Same(handle, SavedParticleHandle(form));
            Assert.Equal(4, SavedLightHostScene(form).Instances.Length);
            Assert.NotNull(form.SavedLightPreviewDiagnostic);
            foreach (int offset in new[] { 12 + 10, 12 + 1024 * 2252 + 1024 * 4, 12 })
            {
                byte[] divergent = (byte[])original.Clone();
                divergent[offset] ^= 2; // Opaque record, state column, then nonzero ActiveRaw.
                File.WriteAllBytes(path, divergent);
                var newStore = LevelObjectStore.Load(map);
                Assert.NotNull(newStore.CaptureFamily(0, 100));
                File.WriteAllBytes(path, [1, 2, 3]);
                Invoke(form, "LoadFamilyPreview", newStore, map);
                Assert.Same(handle, SavedParticleHandle(form));
                Assert.False(handle.IsDisposed);
                AssertParticleCandidateRejected(form);
                Assert.Contains("does not match", form.SavedLightPreviewDiagnostic!);
                Assert.Equal(baseline, handle.CopyPayload());
            }
            // Keep the family closed while transferring the emitter backlink to
            // its existing child object: matching UID membership alone is insufficient.
            byte[] ownerChanged = (byte[])original.Clone();
            BinaryPrimitives.WriteInt16LittleEndian(ownerChanged.AsSpan(12 + 6), 1);
            File.WriteAllBytes(path, ownerChanged);
            string objectPath = Path.Combine(map, "DATA", "objects.dat");
            byte[] originalObjects = File.ReadAllBytes(objectPath);
            byte[] ownerObjects = (byte[])originalObjects.Clone();
            BinaryPrimitives.WriteInt16LittleEndian(ownerObjects.AsSpan(ColumnOffset(11, 0, 10)), -1);
            BinaryPrimitives.WriteInt16LittleEndian(ownerObjects.AsSpan(ColumnOffset(11, 1, 10)), 0);
            File.WriteAllBytes(objectPath, ownerObjects);
            var ownerStore = LevelObjectStore.Load(map);
            Assert.NotNull(ownerStore.CaptureFamily(0, 100));
            File.WriteAllBytes(objectPath, originalObjects);
            File.WriteAllBytes(path, [1, 2, 3]);
            Invoke(form, "LoadFamilyPreview", ownerStore, map);
            AssertParticleCandidateRejected(form);
            Assert.Contains("does not match", form.SavedLightPreviewDiagnostic!);
            Assert.Same(handle, SavedParticleHandle(form));
            File.WriteAllBytes(path, original);
            Invoke(form, "LoadFamilyPreview", identicalStore, map);
            Assert.True(handle.IsDisposed);
            Assert.Equal(4, SavedLightHostScene(form).Instances.Length);
            Assert.Null(form.SavedLightPreviewDiagnostic);
        });
    }

    [SavedParticlePreviewFact]
    public void SavedParticle_host_missing_baseline_rejects_whole_family_but_direct_light_only_still_projects()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            Invoke(form, "ClearSavedParticles");
            File.Delete(Path.Combine(map, "DATA", "particle.dat"));
            Invoke(form, "LoadSavedParticles", map);
            AssertParticleCandidateRejected(form);
        });
        // Independent TEMP fixture: no particle links, even with a failed particle baseline.
        SetupSavedLightHost(map, false);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            Invoke(form, "ClearSavedParticles");
            File.WriteAllBytes(Path.Combine(map, "DATA", "particle.dat"), [1, 2, 3]);
            Invoke(form, "LoadSavedParticles", map);
            AssertPreviewMembers(form, 0);
            Assert.Equal(2, SavedLightHostScene(form).Instances.Length);
            Assert.Contains(new SceneLightSuppressionKey(SdlPlacedObjectsFile.FileName, -5000),
                form.SavedLightPreview.SuppressedObjects);
            Assert.NotNull(form.SavedLightPreviewDiagnostic);
        });
    }

    [SavedParticlePreviewFact]
    public void SavedParticle_host_particle_family_without_lights_still_requires_matching_baseline()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        string data = Path.Combine(map, "DATA");
        byte[] particles = File.ReadAllBytes(Path.Combine(data, "particle.dat"));
        BinaryPrimitives.WriteInt16LittleEndian(particles.AsSpan(12 + 12 + 5 * 35 + 33), -1);
        File.WriteAllBytes(Path.Combine(data, "particle.dat"), particles);
        byte[] objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
        BinaryPrimitives.WriteInt16LittleEndian(objects.AsSpan(ColumnOffset(12, 0, 10)), -1);
        File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
        var lights = NativeLightPool.Load(Path.Combine(data, "light.dat"));
        lights.ReleaseBatch([0, 1]);
        File.WriteAllBytes(Path.Combine(data, "light.dat"), lights.ToPayload());
        var family = LevelObjectStore.Load(map).FamilyAt(0, 100)!;
        Assert.NotNull(family); Assert.Empty(family.LightSlots); Assert.Single(family.ParticleSlots);
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            AssertPreviewMembers(form, 0);
            Invoke(form, "ClearSavedParticles");
            Assert.Equal(2, FamilyPreviewScene(form).Count(item => item.SourceFile == "DATA/objects.dat"));
            Assert.Single(FamilyPreviewScene(form), item => item.SourceFile == SdlPlacedObjectsFile.FileName);
            Assert.NotNull(form.SavedLightPreviewDiagnostic);
        });
    }

    [SavedParticlePreviewFact]
    public void SavedParticle_host_map_change_and_Form_disposal_free_each_adopted_handle_once()
    {
        string map = CreateFixture("ENDL_012"); SetupSavedLightHost(map, true);
        string other = CreateFixture("ENDL_013"); SetupSavedLightHost(other, true);
        var disk = SnapshotDirectory(map); var otherDisk = SnapshotDirectory(other);
        RunInSta(() =>
        {
            long opened = RustParticlePool.OpenedHandleCount, freed = RustParticlePool.FreedHandleCount;
            var form = FamilyPreviewForm(map);
            try
            {
                var first = SavedParticleHandle(form);
                Assert.Equal(opened + 1, RustParticlePool.OpenedHandleCount);
                Invoke(form, "LoadFamilyPreview", LevelObjectStore.Load(other), other);
                var second = SavedParticleHandle(form);
                Assert.True(first.IsDisposed); Assert.False(second.IsDisposed);
                Assert.Equal(freed + 1, RustParticlePool.FreedHandleCount);
                Assert.Equal(opened + 2, RustParticlePool.OpenedHandleCount);
                // ClearFamilyPreview sees a different selected map and releases the new handle.
                Invoke(form, "ClearFamilyPreview");
                Assert.True(second.IsDisposed);
                Assert.Equal(freed + 2, RustParticlePool.FreedHandleCount);
                Invoke(form, "ClearFamilyPreview");
                Invoke(form, "LoadFamilyPreview", LevelObjectStore.Load(map), map);
                var third = SavedParticleHandle(form);
                form.Dispose(); form.Dispose();
                Assert.True(third.IsDisposed);
                Assert.Equal(freed + 3, RustParticlePool.FreedHandleCount);
                Assert.Equal(opened + 3, RustParticlePool.OpenedHandleCount);
            }
            finally { form.Dispose(); }
            AssertNativeVisualSnapshot(map, disk); AssertNativeVisualSnapshot(other, otherDisk);
        });
    }
}
