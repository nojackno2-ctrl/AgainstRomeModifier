using System.Buffers.Binary;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class LevelObjectDirectLightTests
{
    [Fact]
    public void Scenario_candidate_second_copy_capacity_failure_preserves_source_and_documents()
    {
        using var map = FamilyFixture(false, 8);
        var store = LevelObjectStore.Load(map.Map);
        var before = store.Objects(); var files = map.Files();
        var root = before.Single(item => item.Slot == 0);
        Guid sourceId = Guid.NewGuid(), firstId = Guid.NewGuid(), secondId = Guid.NewGuid();
        var original = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Y: root.Y, Prebuilt: true) { Id = sourceId };
        var binding = new ScenarioDataSlot(0, 100) { SpawnId = sourceId };
        var previous = new ScenarioDocument { Spawns = [original], DataSlots = [binding] };
        var next = new ScenarioDocument
        {
            Spawns = [original, original with { Id = firstId, X = root.X + 1 }, original with { Id = secondId, X = root.X + 2 }],
            DataSlots = [binding]
        };
        var nextSpawns = next.Spawns.ToArray();
        Assert.NotNull(store.CreateFamilyCopyCandidate(0, 100, 0, 0, 0));
        Assert.Throws<InvalidOperationException>(() => ScenarioLevelObjects.ApplyToCandidate(store, previous, next,
            _ => throw new InvalidOperationException("No catalogue allowed"),
            new Dictionary<Guid, ScenarioFamilyCopyRequest> { [firstId] = new(0, 100), [secondId] = new(0, 100) }));
        Assert.Equal(before, store.Objects());
        Assert.Equal(new[] { original }, previous.Spawns);
        Assert.Equal(nextSpawns, next.Spawns);
        Assert.Equal(new[] { binding }, previous.DataSlots);
        Assert.Equal(new[] { binding }, next.DataSlots);
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in files) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Scenario_candidate_single_object_copy_keeps_planned_uid_and_saved_light_fields(bool pfil)
    {
        using var map = new Fixture(pfil, 8);
        var store = LevelObjectStore.Load(map.Map);
        var root = Assert.Single(store.Objects());
        var plan = Assert.IsType<LevelObjectFamilyCoreAllocation>(store.PlanFamilyCoreAllocation(0, 100));
        Guid sourceId = Guid.NewGuid(), copyId = Guid.NewGuid();
        var original = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Y: root.Y, Prebuilt: true) { Id = sourceId };
        var duplicate = original with { Id = copyId, X = root.X + 1.25f };
        var previous = new ScenarioDocument { Spawns = [original], DataSlots = [new(0, 100) { SpawnId = sourceId }] };
        var next = new ScenarioDocument { Spawns = [original, duplicate], DataSlots = previous.DataSlots.ToList() };
        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next,
            _ => throw new InvalidOperationException("Saved copies must not need the catalogue"),
            new Dictionary<Guid, ScenarioFamilyCopyRequest> { [copyId] = new(0, 100) });
        var binding = Assert.Single(result.Scenario.DataSlots.Where(item => item.SpawnId == copyId));
        Assert.Equal(plan.Objects[0], binding.Slot);
        Assert.Equal(plan.Uids[0], binding.Uid);
        Assert.NotNull(result.Store.FamilyAt(binding.Slot, binding.Uid));
        using var rollback = new FileRollbackScope();
        result.Store.Save(map.Map, rollback);
        byte[] lights = NativeLightPool.Load(map.LightPath).ToPayload();
        int copiedLight = plan.Lights[0];
        Assert.Equal(123u, BinaryPrimitives.ReadUInt32LittleEndian(lights.AsSpan(8 + 1024 * 42 + copiedLight * 4)));
        Assert.Equal(333f, BinaryPrimitives.ReadSingleLittleEndian(lights.AsSpan(8 + 1024 * 32 + copiedLight * 4)));
        Assert.Equal(100u, store.UidAt(0));
        Assert.Equal(previous.DataSlots, next.DataSlots);
    }

    [Theory]
    [InlineData(false, "parent")]
    [InlineData(true, "parent")]
    [InlineData(false, "particle")]
    [InlineData(true, "particle")]
    [InlineData(false, "small")]
    [InlineData(true, "small")]
    [InlineData(false, "big")]
    [InlineData(true, "big")]
    public void Family_copy_candidate_preserves_source_and_saves_independent_remapped_family(bool pfil, string kind)
    {
        using var map = kind switch { "particle" => ParticleFamilyFixture(pfil, 14),
            "small" => CampFamilyFixture(pfil, false, 14), "big" => CampFamilyFixture(pfil, true, 14),
            _ => FamilyFixture(pfil, 14) };
        var source = LevelObjectStore.Load(map.Map); var objects = source.Objects(); var files = map.Files();
        var family = source.FamilyAt(0, 100)!; var root = objects.Single(obj => obj.Slot == 0);
        var copy = Assert.IsType<LevelObjectFamilyCopyCandidate>(source.CreateFamilyCopyCandidate(0, 100, 81920, -32768, 131072));
        Assert.Equal(objects, source.Objects());
        var copied = Assert.IsType<LevelObjectFamily>(copy.Store.FamilyAt(copy.RootSlot, copy.RootUid));
        Assert.Equal(family.ObjectSlots.Count, copied.ObjectSlots.Count);
        Assert.Equal(family.LightSlots.Count, copied.LightSlots.Count);
        Assert.Equal(family.ParticleSlots.Count, copied.ParticleSlots.Count);
        Assert.Equal(family.LagerSlots.Count, copied.LagerSlots.Count);
        Assert.Equal(family.BigLagerSlots.Count, copied.BigLagerSlots.Count);
        Assert.DoesNotContain(copied.ObjectSlots, family.ObjectSlots.Contains);
        var moved = copy.Store.Objects().Single(obj => obj.Slot == copy.RootSlot);
        Assert.Equal(root.X + 1.25f, moved.X); Assert.Equal(root.Y - .5f, moved.Y); Assert.Equal(root.Z + 2, moved.Z);
        using (var rollback = new FileRollbackScope())
        {
            copy.Store.Save(map.Map, rollback);
            var reloaded = LevelObjectStore.Load(map.Map);
            Assert.NotNull(reloaded.FamilyAt(0, 100));
            Assert.NotNull(reloaded.FamilyAt(copy.RootSlot, copy.RootUid));
        }
        foreach (var pair in files) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }
    [Fact]
    public void Family_core_allocation_is_repeatable_and_does_not_consume_slots_or_uids()
    {
        using var map = FamilyFixture(false, 12);
        var store = LevelObjectStore.Load(map.Map); var before = store.Objects();
        var first = Assert.IsType<LevelObjectFamilyCoreAllocation>(store.PlanFamilyCoreAllocation(0, 100));
        var second = Assert.IsType<LevelObjectFamilyCoreAllocation>(store.PlanFamilyCoreAllocation(0, 100));
        Assert.Equal(first.Objects.OrderBy(pair => pair.Key), second.Objects.OrderBy(pair => pair.Key));
        Assert.Equal(first.Uids.OrderBy(pair => pair.Key), second.Uids.OrderBy(pair => pair.Key));
        Assert.Equal(first.Anim.OrderBy(pair => pair.Key), second.Anim.OrderBy(pair => pair.Key));
        Assert.Equal(first.GfxType.OrderBy(pair => pair.Key), second.GfxType.OrderBy(pair => pair.Key));
        Assert.Equal(first.Action.OrderBy(pair => pair.Key), second.Action.OrderBy(pair => pair.Key));
        Assert.Equal(first.Lights.OrderBy(pair => pair.Key), second.Lights.OrderBy(pair => pair.Key));
        Assert.Equal(first.Particles.OrderBy(pair => pair.Key), second.Particles.OrderBy(pair => pair.Key));
        Assert.Equal(first.Lagers.OrderBy(pair => pair.Key), second.Lagers.OrderBy(pair => pair.Key));
        Assert.Equal(first.BigLagers.OrderBy(pair => pair.Key), second.BigLagers.OrderBy(pair => pair.Key));
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Equal(family.LightSlots.Order(), first.Lights.Keys.Order());
        Assert.DoesNotContain(first.Lights.Values, family.LightSlots.Contains);
        Assert.Equal(first.Positions.Count, first.Positions.Values.Distinct().Count());
        Assert.DoesNotContain(first.Objects.Values, slot => before.Any(obj => obj.Slot == slot));
        Assert.DoesNotContain(first.Uids.Values, uid => before.Any(obj => obj.Uid == uid));
        Assert.Equal(before, store.Objects());
        Assert.Null(store.PlanFamilyCoreAllocation(0, 101));
    }
    [Fact]
    public void Scenario_unchanged_family_does_not_quantize_active_particle_light_saved_state()
    {
        using var map = ParticleFamilyFixture(false);
        string path = Path.Combine(map.Data, "particle.dat"); byte[] bytes = File.ReadAllBytes(path); int child = 12 + 7 * 2252 + 12;
        bytes[child] = 1; BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 1), -32768);
        File.WriteAllBytes(path, bytes); // Light6 deliberately retains its different saved XYZ.
        var store = LevelObjectStore.Load(map.Map); var before = map.Files(); Guid id = Guid.NewGuid();
        var spawn = new ScenarioSpawn("BauTest", 10, 30, 0, Y: 20, Prebuilt: true) { Id = id };
        var previous = new ScenarioDocument { Spawns = [spawn], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = id }] };
        var next = new ScenarioDocument { Spawns = [spawn], DataSlots = previous.DataSlots.ToList() };
        Assert.Empty(ScenarioLevelObjects.Apply(store, previous, next, _ => null));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in before) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Scenario_duplicate_or_child_family_binding_is_rejected_before_native_mutation(bool childBinding)
    {
        using var map = FamilyFixture(false); var store = LevelObjectStore.Load(map.Map); var before = map.Files();
        Guid id = Guid.NewGuid(); var spawn = new ScenarioSpawn("BauTest", 10, 30, 0, Y: 20, Prebuilt: true) { Id = id };
        var binding = new ScenarioDataSlot(0, 100) { SpawnId = id };
        var previous = new ScenarioDocument { Spawns = [spawn], DataSlots = [binding] };
        if (childBinding)
        {
            var child = store.Objects().Single(obj => obj.Slot == 1); Guid childId = Guid.NewGuid();
            previous.Spawns.Add(spawn with { Id = childId, X = child.X, Y = child.Y, Z = child.Z });
            previous.DataSlots.Add(new ScenarioDataSlot(child.Slot, child.Uid) { SpawnId = childId });
        }
        else previous.DataSlots.Add(binding);
        var next = new ScenarioDocument { Spawns = previous.Spawns.ToList(), DataSlots = previous.DataSlots.ToList() };
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, previous, next, _ => null));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in before) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Theory]
    [InlineData(false, "parent")]
    [InlineData(true, "parent")]
    [InlineData(false, "particle")]
    [InlineData(true, "particle")]
    [InlineData(false, "small-camp")]
    [InlineData(true, "small-camp")]
    [InlineData(false, "large-camp")]
    [InlineData(true, "large-camp")]
    public void Scenario_family_move_preserves_binding_without_catalog_and_repeated_save_does_not_move_again(bool pfil, string kind)
    {
        using var map = kind switch
        {
            "particle" => ParticleFamilyFixture(pfil),
            "small-camp" => CampFamilyFixture(pfil, false),
            "large-camp" => CampFamilyFixture(pfil, true),
            _ => FamilyFixture(pfil)
        };
        var store = LevelObjectStore.Load(map.Map); var before = store.Objects(); var root = before.Single(obj => obj.Slot == 0);
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100)); var members = family.ObjectSlots.ToHashSet();
        var snapshot = Assert.IsType<LevelObjectFamilySnapshot>(store.CaptureFamily(0, 100));
        Assert.Equal(family.ObjectSlots.Order(), snapshot.Objects.Keys.Order());
        Assert.Equal(family.LightSlots.Order(), snapshot.Lights.Keys.Order());
        Assert.Equal(family.ParticleSlots.Order(), snapshot.Particles.Keys.Order());
        Assert.Equal(family.LagerSlots.Order(), snapshot.Lagers.Keys.Order());
        Assert.Equal(family.BigLagerSlots.Order(), snapshot.BigLagers.Keys.Order());
        Assert.Null(store.CaptureFamily(0, 101));
        var candidate = store.CloneForEditing();
        Assert.Equal(before, candidate.Objects());
        Assert.True(candidate.RemoveFamilyIfUid(0, 100));
        Assert.Null(candidate.CaptureFamily(0, 100));
        Assert.Equal(before, store.Objects());
        var retained = Assert.IsType<LevelObjectFamilySnapshot>(store.CaptureFamily(0, 100));
        foreach (int slot in snapshot.Particles.Keys)
            Assert.Equal(snapshot.Particles[slot].RecordBytes(), retained.Particles[slot].RecordBytes());
        foreach (int slot in snapshot.Lagers.Keys)
            Assert.Equal(snapshot.Lagers[slot].RecordBytes(), retained.Lagers[slot].RecordBytes());
        foreach (int slot in snapshot.BigLagers.Keys)
            Assert.Equal(snapshot.BigLagers[slot].RecordBytes(), retained.BigLagers[slot].RecordBytes());
        Guid id = Guid.NewGuid(); var old = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true) { Id = id };
        var ordinary = new ScenarioSpawn("Unit", 100, 200, 1) { Id = Guid.NewGuid() };
        var binding = new ScenarioDataSlot(0, 100) { SpawnId = id };
        var previous = new ScenarioDocument { Spawns = [old, ordinary], DataSlots = [binding] };
        var next = new ScenarioDocument
        {
            Spawns = [ordinary, old with { X = old.X + 1.25f, Y = old.Y - .5f, Z = old.Z + 2f, Angle = 360 }],
            DataSlots = previous.DataSlots.ToList()
        };
        var files = map.Files(); int catalogueCalls = 0;
        Assert.Empty(ScenarioLevelObjects.Apply(store, previous, next, _ => { catalogueCalls++; return null; }));
        Assert.Equal(0, catalogueCalls); Assert.Equal(binding, Assert.Single(next.DataSlots));
        foreach (var obj in before)
            Assert.Equal(members.Contains(obj.Slot) ? obj with { X = obj.X + 1.25f, Y = obj.Y - .5f, Z = obj.Z + 2f } : obj,
                store.Objects().Single(current => current.Slot == obj.Slot));
        using (var rollback = new FileRollbackScope()) store.Save(map.Map, rollback);
        foreach (var pair in files) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        var saved = map.Files(); var reloaded = LevelObjectStore.Load(map.Map);
        Assert.Equal(store.Objects(), reloaded.Objects());
        var unchanged = new ScenarioDocument { Spawns = next.Spawns.ToList(), DataSlots = next.DataSlots.ToList() };
        Assert.Empty(ScenarioLevelObjects.Apply(reloaded, next, unchanged, _ => throw new InvalidOperationException("Retained family should not need catalogue")));
        Assert.Equal(binding, Assert.Single(unchanged.DataSlots));
        using (var rollback = new FileRollbackScope()) { reloaded.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in saved) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
        // A later delete still uses the preserved original root binding.
        var deleted = new ScenarioDocument { Spawns = [ordinary] };
        Assert.Empty(ScenarioLevelObjects.Apply(reloaded, unchanged, deleted, _ => null));
        Assert.Empty(deleted.DataSlots); Assert.Equal(before.Where(obj => !members.Contains(obj.Slot)), reloaded.Objects());
    }

    [Theory]
    [InlineData("angle")]
    [InlineData("team")]
    [InlineData("alias")]
    [InlineData("fraction")]
    [InlineData("distance")]
    [InlineData("nonfinite")]
    [InlineData("stale-previous-angle")]
    [InlineData("actual-angle")]
    public void Scenario_unsupported_family_edit_leaves_native_files_unchanged(string fault)
    {
        using var map = FamilyFixture(false); var store = LevelObjectStore.Load(map.Map);
        Guid id = Guid.NewGuid(); var old = new ScenarioSpawn("BauTest", 10, 30, 0, Y: 20, Prebuilt: true) { Id = id };
        var changed = fault switch
        {
            "angle" => old with { Angle = 90 }, "team" => old with { Team = 1 }, "alias" => old with { Alias = "BauOther" },
            "fraction" => old with { X = 10.000001f }, "distance" => old with { X = 40000 },
            "nonfinite" => old with { X = float.NaN }, _ => old
        };
        if (fault == "stale-previous-angle") old = old with { Angle = 90 };
        if (fault == "actual-angle")
        {
            string path = Path.Combine(map.Data, "position.dat"); byte[] positions = File.ReadAllBytes(path);
            F32(positions, 8 + 13, 90); File.WriteAllBytes(path, positions); store = LevelObjectStore.Load(map.Map);
        }
        var previous = new ScenarioDocument { Spawns = [old], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = id }] };
        var next = new ScenarioDocument { Spawns = [changed], DataSlots = previous.DataSlots.ToList() };
        var before = map.Files();
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, previous, next, _ => null));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in before) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Theory]
    [InlineData(false, "parent")]
    [InlineData(true, "parent")]
    [InlineData(false, "particle")]
    [InlineData(true, "particle")]
    [InlineData(false, "small-camp")]
    [InlineData(true, "small-camp")]
    [InlineData(false, "large-camp")]
    [InlineData(true, "large-camp")]
    public void Family_translation_preserves_identity_links_other_bytes_and_all_file_rollback(bool pfil, string kind)
    {
        using var map = kind switch
        {
            "particle" => ParticleFamilyFixture(pfil),
            "small-camp" => CampFamilyFixture(pfil, false),
            "large-camp" => CampFamilyFixture(pfil, true),
            _ => FamilyFixture(pfil)
        };
        // A third position shared inside the family must translate once; its direction must survive.
        string objectPath = Path.Combine(map.Data, "objects.dat"), positionPath = Path.Combine(map.Data, "position.dat");
        byte[] objectPacked = File.ReadAllBytes(objectPath), positionPacked = File.ReadAllBytes(positionPath);
        byte[] sourceObjects = GameLZSS.DecompressPfil(objectPacked), sourcePositions = GameLZSS.DecompressPfil(positionPacked);
        int objectCount = BinaryPrimitives.ReadInt32LittleEndian(sourceObjects.AsSpan(4));
        int third = BinaryPrimitives.ReadInt32LittleEndian(sourcePositions.AsSpan(4)) - 1;
        U16(sourceObjects, Fixture.Column(10, 0, objectCount), (ushort)third);
        U16(sourceObjects, Fixture.Column(10, 1, objectCount), (ushort)third);
        sourcePositions[8 + third * 17] = 1;
        F32(sourcePositions, 8 + third * 17 + 1, 1000); F32(sourcePositions, 8 + third * 17 + 5, 2000);
        F32(sourcePositions, 8 + third * 17 + 9, 3000); F32(sourcePositions, 8 + third * 17 + 13, 123);
        File.WriteAllBytes(objectPath, pfil ? GameLZSS.CompressPfil(sourceObjects, objectPacked[..64]) : sourceObjects);
        File.WriteAllBytes(positionPath, pfil ? GameLZSS.CompressPfil(sourcePositions, positionPacked[..64]) : sourcePositions);
        if (kind == "particle")
        {
            string path = Path.Combine(map.Data, "particle.dat"); byte[] packed = File.ReadAllBytes(path);
            byte[] bytes = GameLZSS.DecompressPfil(packed); int child = 12 + 7 * 2252 + 12;
            bytes[child] = 1;
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 1), -32768);
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 5), 131072);
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 9), 16384);
            File.WriteAllBytes(path, pfil ? GameLZSS.CompressPfil(bytes, packed[..64]) : bytes);
            var lights = NativeLightPool.Load(map.LightPath); lights.SetPositionsBatch([new(6, new(-1, 2, 0))]);
            using var rollback = new FileRollbackScope(); lights.Save(map.LightPath, rollback); rollback.Commit();
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map); var objectsBefore = store.Objects();
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        var members = family.ObjectSlots.ToHashSet();
        var expected = before.ToDictionary(pair => pair.Key, pair => GameLZSS.DecompressPfil(pair.Value).ToArray());
        byte[] objectBytes = expected[Path.Combine(map.Data, "objects.dat")];
        int count = BinaryPrimitives.ReadInt32LittleEndian(objectBytes.AsSpan(4));
        var positionSlots = family.ObjectSlots.SelectMany(slot => new[] {
            (int)BinaryPrimitives.ReadUInt16LittleEndian(objectBytes.AsSpan(16 + slot * 79 + 67)),
            (int)BinaryPrimitives.ReadUInt16LittleEndian(objectBytes.AsSpan(16 + slot * 79 + 69)),
            (int)BinaryPrimitives.ReadUInt16LittleEndian(objectBytes.AsSpan(Fixture.Column(10, slot, count))) })
            .Where(slot => slot != ushort.MaxValue).Distinct();
        byte[] positionBytes = expected[Path.Combine(map.Data, "position.dat")]; float[] delta = [1.25f, -.5f, 2f];
        foreach (int slot in positionSlots)
            for (int axis = 0; axis < 3; axis++)
            {
                int offset = 8 + slot * 17 + 1 + axis * 4;
                BinaryPrimitives.WriteSingleLittleEndian(positionBytes.AsSpan(offset), BinaryPrimitives.ReadSingleLittleEndian(positionBytes.AsSpan(offset)) + delta[axis]);
            }
        byte[] lightBytes = expected[map.LightPath];
        foreach (int slot in family.LightSlots)
            for (int axis = 0; axis < 3; axis++)
            {
                int offset = 8 + (axis + 1) * 4096 + slot * 4;
                float value = kind == "particle" && slot == 6 ? new float[] { 0, 1, 2 }[axis]
                    : BinaryPrimitives.ReadSingleLittleEndian(lightBytes.AsSpan(offset)) + delta[axis];
                BinaryPrimitives.WriteSingleLittleEndian(lightBytes.AsSpan(offset), value);
            }
        if (kind == "particle")
        {
            byte[] bytes = expected[Path.Combine(map.Data, "particle.dat")]; int child = 12 + 7 * 2252 + 12;
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 1), 49152);
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 5), 98304);
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(child + 9), 147456);
        }
        Assert.False(store.TranslateFamilyIfUidFixed(0, 101, 81920, -32768, 131072));
        Assert.True(store.TranslateFamilyIfUidFixed(0, 100, 81920, -32768, 131072));
        foreach (var old in objectsBefore)
        {
            var moved = store.Objects().Single(obj => obj.Slot == old.Slot);
            Assert.Equal(members.Contains(old.Slot) ? old with { X = old.X + 1.25f, Y = old.Y - .5f, Z = old.Z + 2f } : old, moved);
        }
        Assert.Throws<IOException>((Action)(() =>
        {
            using var rollback = new FileRollbackScope(); store.Save(map.Map, rollback);
            throw new IOException("Injected later family save failure");
        }));
        foreach (var pair in before) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in expected) Assert.Equal(pair.Value, GameLZSS.DecompressPfil(File.ReadAllBytes(pair.Key)));
        var reloaded = LevelObjectStore.Load(map.Map);
        var reloadedFamily = Assert.IsType<LevelObjectFamily>(reloaded.FamilyAt(0, 100));
        Assert.Equal(family.ObjectSlots, reloadedFamily.ObjectSlots); Assert.Equal(family.LagerSlots, reloadedFamily.LagerSlots);
        Assert.Equal(family.BigLagerSlots, reloadedFamily.BigLagerSlots); Assert.Equal(family.ParticleSlots, reloadedFamily.ParticleSlots);
        Assert.Equal(family.LightSlots, reloadedFamily.LightSlots);
        Assert.True(reloaded.TranslateFamilyIfUidFixed(0, 100, -81920, 32768, -131072));
        using (var rollback = new FileRollbackScope()) { reloaded.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in before) Assert.Equal(GameLZSS.DecompressPfil(pair.Value), GameLZSS.DecompressPfil(File.ReadAllBytes(pair.Key)));
    }

    [Theory]
    [InlineData("position-nan")]
    [InlineData("light-infinity")]
    [InlineData("particle-overflow")]
    [InlineData("child-light-conflict")]
    public void Family_translation_late_candidate_failure_leaves_every_original_pool_unchanged(string fault)
    {
        using var map = ParticleFamilyFixture(false);
        string path = Path.Combine(map.Data, "particle.dat"); byte[] particle = File.ReadAllBytes(path);
        int child = 12 + 7 * 2252 + 12; particle[child] = 1;
        if (fault == "particle-overflow") BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(child + 1), int.MaxValue);
        if (fault == "child-light-conflict")
        {
            particle[child + 35] = 1; U16(particle, child + 35 + 33, 6);
            BinaryPrimitives.WriteInt32LittleEndian(particle.AsSpan(child + 35 + 1), 131072);
        }
        File.WriteAllBytes(path, particle);
        if (fault == "position-nan")
        {
            path = Path.Combine(map.Data, "position.dat"); byte[] bytes = File.ReadAllBytes(path);
            F32(bytes, 9, float.NaN); File.WriteAllBytes(path, bytes);
        }
        if (fault == "light-infinity")
        {
            byte[] bytes = File.ReadAllBytes(map.LightPath); F32(bytes, 8 + 4096 + 4 * 4, float.PositiveInfinity);
            File.WriteAllBytes(map.LightPath, bytes);
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        Assert.NotNull(store.FamilyAt(0, 100));
        Action move = () => store.TranslateFamilyIfUidFixed(0, 100, 65536, 0, 0);
        if (fault == "particle-overflow") Assert.Throws<OverflowException>(move);
        else Assert.Throws<InvalidDataException>(move);
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in before) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Theory]
    [InlineData(false, false)]
    [InlineData(false, true)]
    [InlineData(true, false)]
    [InlineData(true, true)]
    public void Camp_family_includes_nested_garrisons_and_descendants_and_preserves_external_pools(bool pfil, bool large)
    {
        using var map = CampFamilyFixture(pfil, large);
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Equal(new[] { 0, 1, 2, 3, 4 }, family.ObjectSlots.Order()); Assert.Equal(new[] { 7, 8 }, family.LagerSlots);
        Assert.Equal(large ? new[] { 2 } : Array.Empty<int>(), family.BigLagerSlots);
        Assert.False(store.Remove(3)); Assert.Null(store.OwnedTemplate(3, store.UidAt(3)!.Value));
        Assert.True(store.RemoveFamilyIfUid(0, 100)); Assert.Equal(5, Assert.Single(store.Objects()).Slot);
        using (var rollback = new FileRollbackScope()) store.Save(map.Map, rollback);
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        Assert.Empty(NativeLagerPool.Load(Path.Combine(map.Data, "lager.dat")).ActiveLagers());
        Assert.Null(NativeBigLagerPool.Load(Path.Combine(map.Data, "biglager.dat")).Capture(2));
        Assert.Equal(3, Assert.Single(NativeBigLagerPool.Load(Path.Combine(map.Data, "biglager.dat")).ActiveLagers()).Slot);
        Assert.Equal(5, Assert.Single(NativeLightPool.Load(map.LightPath).ActiveLights()).OwnerObject);
        Assert.Equal(5, Assert.Single(LevelObjectStore.Load(map.Map).Objects()).Slot);
        Assert.Equal(ushort.MaxValue, map.ReadColumn(5, 0)); Assert.Equal(ushort.MaxValue, map.ReadColumn(5, 3));
        var outside = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(5, store.UidAt(5)!.Value));
        Assert.Equal(0, store.Add(outside, 500, 20, 30, 0)); // no stale garrison reservation
    }

    [Theory]
    [InlineData("cycle")]
    [InlineData("outside-camp")]
    [InlineData("outside-member")]
    [InlineData("shared-big")]
    [InlineData("outside-big-member")]
    [InlineData("external-parent")]
    [InlineData("member-range")]
    [InlineData("inactive-member")]
    [InlineData("occupancy")]
    [InlineData("missing-big")]
    public void Camp_family_faults_are_rejected_before_any_pool_changes(string fault)
    {
        using var map = CampFamilyFixture(false, true);
        string campPath = Path.Combine(map.Data, "lager.dat"), bigPath = Path.Combine(map.Data, "biglager.dat");
        byte[] camp = File.ReadAllBytes(campPath), big = File.ReadAllBytes(bigPath);
        int record = 16 + 7 * 43, bigRecord = 12 + 2 * 1601;
        if (fault == "cycle") U16(big, bigRecord + 1, 0);
        if (fault == "member-range") U16(big, bigRecord + 1, 100);
        if (fault == "inactive-member") U16(big, bigRecord + 1, 6);
        if (fault == "occupancy") U16(camp, record + 9, 2);
        if (fault == "outside-member") { camp[16 + 9 * 43] = 1; U16(camp, 16 + 9 * 43 + 23, 3); }
        if (fault == "shared-big") { camp[16 + 9 * 43] = 1; U16(camp, 16 + 3200 * 43 + 9 * 2, 2); }
        if (fault == "outside-big-member") U16(big, 12 + 3 * 1601 + 1, 4);
        File.WriteAllBytes(campPath, camp); File.WriteAllBytes(bigPath, big);
        if (fault == "missing-big") File.Delete(bigPath);
        if (fault is "outside-camp" or "external-parent")
        {
            string path = Path.Combine(map.Data, "objects.dat"); byte[] objects = File.ReadAllBytes(path);
            U16(objects, Fixture.Column(fault == "outside-camp" ? 5 : 13, fault == "outside-camp" ? 5 : 3, 7), fault == "outside-camp" ? (ushort)7 : (ushort)5);
            File.WriteAllBytes(path, objects);
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        Assert.Null(store.FamilyAt(0, 100)); Assert.False(store.RemoveFamilyIfUid(0, 100));
        Assert.False(store.TranslateFamilyIfUidFixed(0, 100, 65536, 0, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
    }

    private static Fixture CampFamilyFixture(bool pfil, bool large, int count = 7)
    {
        var map = FamilyFixture(pfil, count); var store = LevelObjectStore.Load(map.Map);
        var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(3, store.UidAt(3)!.Value));
        Assert.Equal(4, store.Add(template, 400, 20, 30, 0)); Assert.Equal(5, store.Add(template, 500, 20, 30, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        string path = Path.Combine(map.Data, "objects.dat"); byte[] packed = File.ReadAllBytes(path), objects = GameLZSS.DecompressPfil(packed);
        U16(objects, Fixture.Column(5, 0, count), 7); U16(objects, Fixture.Column(5, 3, count), 8); U16(objects, Fixture.Column(13, 4, count), 3);
        Write("objects.dat", objects);
        byte[] camp = new byte[NativeLagerPool.PayloadLength]; U32(camp, 0, 1); U32(camp, 4, 3200); U32(camp, 8, 6); U32(camp, 12, 10);
        var camps = NativeLagerPool.ParsePayload(camp); camps.ReleaseBatch(Enumerable.Range(0, 3200).ToArray()); camp = camps.ToPayload();
        camp[16 + 7 * 43] = 1; U16(camp, 16 + 7 * 43 + 9, 1);
        camp[16 + 8 * 43] = 1; U16(camp, 16 + 8 * 43 + 9, 1); U16(camp, 16 + 8 * 43 + 23, 4);
        if (!large) U16(camp, 16 + 7 * 43 + 23, 3);
        else U16(camp, 16 + 3200 * 43 + 7 * 2, 2);
        Write("lager.dat", camp);
        byte[] big = new byte[NativeBigLagerPool.PayloadLength]; U32(big, 0, 1); U32(big, 4, 32); U32(big, 8, 800);
        var bigs = NativeBigLagerPool.ParsePayload(big); bigs.ReleaseBatch(Enumerable.Range(0, 32).ToArray()); big = bigs.ToPayload();
        if (large) { big[12 + 2 * 1601] = 1; U16(big, 12 + 2 * 1601 + 1, 3); }
        big[12 + 3 * 1601] = 1; // unrelated unreferenced active slot must survive
        Write("biglager.dat", big);
        return map;
        void Write(string name, byte[] bytes)
        {
            byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
            File.WriteAllBytes(Path.Combine(map.Data, name), pfil ? GameLZSS.CompressPfil(bytes, header) : bytes);
        }
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Garrison_member_with_no_own_links_cannot_be_individually_deleted_or_copied(bool large)
    {
        using var map = CampFamilyFixture(false, large);
        string objectPath = Path.Combine(map.Data, "objects.dat"); byte[] objects = File.ReadAllBytes(objectPath);
        U16(objects, Fixture.Column(5, 3, 7), ushort.MaxValue); U16(objects, Fixture.Column(13, 4, 7), ushort.MaxValue);
        File.WriteAllBytes(objectPath, objects);
        string path = Path.Combine(map.Data, "lager.dat"); var camps = NativeLagerPool.Load(path); camps.ReleaseBatch([8]);
        using (var rollback = new FileRollbackScope()) { camps.Save(path, rollback); rollback.Commit(); }
        var store = LevelObjectStore.Load(map.Map);
        Assert.True(store.Objects().Single(obj => obj.Slot == 3).Linked);
        Assert.False(store.Remove(3)); Assert.Null(store.OwnedTemplate(3, store.UidAt(3)!.Value));
    }

    [Fact]
    public void Inactive_garrison_reference_reserves_only_free_object_slot_before_allocation()
    {
        using var map = CampFamilyFixture(false, true);
        string path = Path.Combine(map.Data, "biglager.dat"); byte[] bytes = File.ReadAllBytes(path);
        U16(bytes, 12 + 3 * 1601 + 1, 6); File.WriteAllBytes(path, bytes);
        var store = LevelObjectStore.Load(map.Map); var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(5, store.UidAt(5)!.Value));
        var before = map.Files(); Assert.Equal(-1, store.Add(template, 600, 20, 30, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var (file, original) in before) Assert.Equal(original, File.ReadAllBytes(file));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Particle_family_deletion_releases_child_lights_and_rolls_back_every_pool(bool pfil)
    {
        using var map = ParticleFamilyFixture(pfil);
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Equal(new[] { 7 }, family.ParticleSlots); Assert.Contains(6, family.LightSlots);
        var diagnostics = NativePoolDiagnostics.DiagnoseDirectory(map.Data);
        Assert.Equal(1, diagnostics.ActiveEmitters);
        Assert.DoesNotContain(diagnostics.Issues, issue => issue.TargetPool == "particle");
        Assert.True(store.RemoveFamilyIfUid(0, 100));
        using (var rollback = new FileRollbackScope()) store.Save(map.Map, rollback);
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        Assert.Empty(NativeParticlePool.Load(Path.Combine(map.Data, "particle.dat")).ActiveEmitters());
        Assert.Equal(3, Assert.Single(NativeLightPool.Load(map.LightPath).ActiveLights()).OwnerObject);
        Assert.Equal(ushort.MaxValue, map.ReadColumn(11, 1));
        Assert.Equal(3, Assert.Single(LevelObjectStore.Load(map.Map).Objects()).Slot);
        Assert.DoesNotContain(NativePoolDiagnostics.DiagnoseDirectory(map.Data).Issues, issue => issue.TargetPool == "particle");
    }

    [Theory]
    [InlineData("owner")]
    [InlineData("light-index")]
    [InlineData("external-emitter")]
    [InlineData("orphan-owner")]
    [InlineData("outside-backlink")]
    [InlineData("missing")]
    [InlineData("invalid-header")]
    [InlineData("inactive-light")]
    public void Particle_family_bad_or_shared_links_reject_without_mutating_any_pool(string fault)
    {
        using var map = ParticleFamilyFixture(false); string particlePath = Path.Combine(map.Data, "particle.dat");
        byte[] particles = File.ReadAllBytes(particlePath); int record = 12 + 7 * 2252;
        if (fault == "missing") File.Delete(particlePath);
        else
        {
            if (fault == "owner") U16(particles, record + 6, 3);
            if (fault == "light-index") U16(particles, record + 12 + 33, 1024);
            if (fault == "inactive-light") U16(particles, record + 12 + 33, 700);
            if (fault == "invalid-header") U32(particles, 8, 63);
            if (fault is "external-emitter" or "orphan-owner")
            {
                int second = 12 + 8 * 2252; U16(particles, second, 1);
                U16(particles, second + 6, fault == "external-emitter" ? (ushort)3 : (ushort)1);
                if (fault == "external-emitter") U16(particles, second + 12 + 33, 6);
            }
            File.WriteAllBytes(particlePath, particles);
        }
        if (fault == "outside-backlink")
        {
            string path = Path.Combine(map.Data, "objects.dat"); byte[] objects = File.ReadAllBytes(path);
            U16(objects, Fixture.Column(11, 3, 5), 7); File.WriteAllBytes(path, objects);
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        string expected = fault switch
        {
            "owner" or "outside-backlink" => "particle-owner-mismatch",
            "light-index" => "particle-light-out-of-range",
            "external-emitter" => "particle-light-shared",
            "orphan-owner" => "particle-backlink-mismatch",
            "invalid-header" => "invalid-particle",
            "inactive-light" => "particle-light-unavailable",
            _ => "particle-pool-unavailable"
        };
        Assert.Contains(NativePoolDiagnostics.DiagnoseDirectory(map.Data).Issues, issue => issue.Code == expected);
        Assert.Null(store.FamilyAt(0, 100)); Assert.False(store.RemoveFamilyIfUid(0, 100));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
    }

    private static Fixture ParticleFamilyFixture(bool pfil, int count = 5)
    {
        var map = FamilyFixture(pfil, count);
        string objectPath = Path.Combine(map.Data, "objects.dat"); byte[] packed = File.ReadAllBytes(objectPath);
        byte[] objects = GameLZSS.DecompressPfil(packed); U16(objects, Fixture.Column(11, 1, count), 7);
        File.WriteAllBytes(objectPath, pfil ? GameLZSS.CompressPfil(objects, packed[..64]) : objects);
        var lights = NativeLightPool.Load(map.LightPath);
        Assert.Equal(new[] { 6 }, lights.AddBatch([new(new(100, 20, 30), new(1, 1, 1), 350, 0, 0, 0, 2, -1)]));
        using (var rollback = new FileRollbackScope()) { lights.Save(map.LightPath, rollback); rollback.Commit(); }
        byte[] bytes = new byte[NativeParticlePool.PayloadLength]; U32(bytes, 0, 1); U32(bytes, 4, 1024); U32(bytes, 8, 64);
        var particles = NativeParticlePool.ParsePayload(bytes); particles.ReleaseBatch(Enumerable.Range(0, 1024).ToArray());
        bytes = particles.ToPayload(); int record = 12 + 7 * 2252;
        U16(bytes, record, 1); U16(bytes, record + 6, 1); U16(bytes, record + 12 + 33, 6);
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        File.WriteAllBytes(Path.Combine(map.Data, "particle.dat"), pfil ? GameLZSS.CompressPfil(bytes, header) : bytes);
        return map;
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Family_delete_includes_descendants_and_parent_lights_and_rolls_back_all_files(bool pfil)
    {
        using var map = FamilyFixture(pfil);
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        var family = Assert.IsType<LevelObjectFamily>(store.FamilyAt(0, 100));
        Assert.Equal(new[] { 0, 1, 2 }, family.ObjectSlots); Assert.Equal(5, family.LightSlots.Count);
        Assert.Null(store.FamilyAt(1, store.UidAt(1)!.Value));
        Assert.False(store.Remove(0)); Assert.False(store.Remove(1));
        Assert.True(store.RemoveFamilyIfUid(0, 100));
        Assert.Equal(3, Assert.Single(store.Objects()).Slot);
        Assert.Throws<IOException>((Action)(() =>
        {
            using var rollback = new FileRollbackScope(); store.Save(map.Map, rollback);
            throw new IOException("simulate a later map save failure");
        }));
        foreach (var (path, source) in before) Assert.Equal(source, File.ReadAllBytes(path));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        Assert.Equal(3, Assert.Single(NativeLightPool.Load(map.LightPath).ActiveLights()).OwnerObject);
        Assert.DoesNotContain(NativePoolDiagnostics.DiagnoseDirectory(map.Data).Issues, issue => issue.TargetPool == "light");
        Assert.Equal(ushort.MaxValue, map.ReadColumn(13, 1)); Assert.Equal(ushort.MaxValue, map.ReadColumn(12, 1));
        // Freed root must not remain reserved by stale parent/light caches.
        var outside = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(3, store.UidAt(3)!.Value));
        Assert.Equal(0, store.Add(outside, 400, 0, 400, 0));
    }

    [Theory]
    [InlineData("stale-uid")]
    [InlineData("cycle")]
    [InlineData("outside-position")]
    [InlineData("outside-data")]
    [InlineData("outside-light")]
    [InlineData("unsupported-link")]
    [InlineData("mixed-light")]
    [InlineData("missing-light")]
    public void Invalid_or_externally_shared_family_is_rejected_before_mutation(string fault)
    {
        using var map = FamilyFixture(false);
        if (fault == "missing-light") File.Delete(map.LightPath);
        else if (fault == "mixed-light")
        {
            byte[] data = File.ReadAllBytes(map.LightPath); U16(data, 8 + 1024 * 50 + 4 * 2, 3); File.WriteAllBytes(map.LightPath, data);
        }
        else
        {
            byte[] data = File.ReadAllBytes(Path.Combine(map.Data, "objects.dat"));
            switch (fault)
            {
                case "cycle": U16(data, Fixture.Column(13, 0, 5), 2); break;
                case "outside-position": U16(data, 16 + 3 * 79 + 67, BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(16 + 2 * 79 + 67))); break;
                case "outside-data": U16(data, Fixture.Column(1, 3, 5), map.ReadColumn(1, 2)); break;
                case "outside-light": U16(data, Fixture.Column(12, 3, 5), 4); break;
                case "unsupported-link": U16(data, Fixture.Column(3, 1, 5), 1); break;
            }
            File.WriteAllBytes(Path.Combine(map.Data, "objects.dat"), data);
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        uint uid = fault == "stale-uid" ? 999u : 100u;
        Assert.Null(store.FamilyAt(0, uid)); Assert.False(store.RemoveFamilyIfUid(0, uid));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var (path, source) in before) Assert.Equal(source, File.ReadAllBytes(path));
    }

    [Fact]
    public void Scenario_deleting_prebuilt_root_removes_family_but_updating_it_stays_protected()
    {
        using var map = FamilyFixture(false);
        Guid id = Guid.NewGuid();
        var previous = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", 10, 30, 1, 0, 0, 20, Prebuilt: true) { Id = id }],
            DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = id }]
        };
        var store = LevelObjectStore.Load(map.Map);
        var retained = new ScenarioDocument { Spawns = previous.Spawns.ToList(), DataSlots = previous.DataSlots.ToList() };
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, previous, retained, _ => null));
        Assert.Equal(4, store.Objects().Count);
        Assert.Empty(ScenarioLevelObjects.Apply(store, previous, new ScenarioDocument(), _ => null));
        Assert.Equal(3, Assert.Single(store.Objects()).Slot);
    }

    private static Fixture FamilyFixture(bool pfil, int count = 5, bool withEngine = false)
    {
        var map = new Fixture(pfil, count, withEngine); var store = LevelObjectStore.Load(map.Map);
        var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(0, 100));
        for (int slot = 1; slot < 4; slot++) Assert.Equal(slot, store.Add(template, slot * 100, 20, 30, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        string path = Path.Combine(map.Data, "objects.dat"); byte[] packed = File.ReadAllBytes(path), objects = GameLZSS.DecompressPfil(packed);
        U16(objects, Fixture.Column(13, 1, count), 0); U16(objects, Fixture.Column(13, 2, count), 1);
        File.WriteAllBytes(path, pfil ? GameLZSS.CompressPfil(objects, packed[..64]) : objects);
        var lights = NativeLightPool.Load(map.LightPath);
        Assert.Equal(new[] { 4, 5 }, lights.AddBatch([new(new(10, 40, 30), new(1, 1, 1), 350, 0, 0, 0, 2, -1, 0),
            new(new(200, 40, 30), new(1, 1, 1), 350, 0, 0, 0, 2, -1, 2)]));
        using (var rollback = new FileRollbackScope()) { lights.Save(map.LightPath, rollback); rollback.Commit(); }
        return map;
    }

    [Theory]
    [InlineData(0f, 90f, 300f)]
    [InlineData(90f, 100f, 310f)]
    [InlineData(180f, 110f, 300f)]
    [InlineData(270f, 100f, 290f)]
    public void Owned_copy_recomputes_owner_light_for_new_rotation_and_keeps_state(float rotation, float expectedX, float expectedZ)
    {
        using var map = new Fixture(false);
        var definitions = new Dictionary<int, NativeObjectDefinition>
        { [555] = new(555, true, 0) { DirectLightDefinition = 2, DirectLightHeight = -7, DirectLightDistance = 10 } };
        var store = LevelObjectStore.Load(map.Map, definitions);
        var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(0, 100));
        float radians = rotation * MathF.PI / 180f;
        Assert.Equal(1, store.Add(template, 100, 200, 300, radians));
        Assert.InRange(Math.Abs(store.Objects().Single(obj => obj.Slot == 1).Rotation - radians), 0, .000001f);
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        var pool = NativeLightPool.Load(map.LightPath); var light = pool.ActiveLights()[1];
        Assert.InRange(Math.Abs(light.Position.X - expectedX), 0, .0001f);
        Assert.Equal(193f, light.Position.Y);
        Assert.InRange(Math.Abs(light.Position.Z - expectedZ), 0, .0001f);
        byte[] payload = pool.ToPayload();
        Assert.Equal(123u, BinaryPrimitives.ReadUInt32LittleEndian(payload.AsSpan(8 + 1024 * 42 + 4)));
        Assert.Equal(333f, BinaryPrimitives.ReadSingleLittleEndian(payload.AsSpan(8 + 1024 * 32 + 4)));
        Assert.Equal(1, light.OwnerObject); Assert.Equal((ushort)1, map.ReadColumn(12, 1));
        byte[] objects = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(map.Data, "objects.dat")));
        int positionSlot = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + 79 + 67));
        byte[] positions = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(map.Data, "position.dat")));
        float savedDegrees = BinaryPrimitives.ReadSingleLittleEndian(positions.AsSpan(8 + positionSlot * 17 + 13));
        Assert.InRange(Math.Abs(savedDegrees - rotation), 0, .0001f);
    }

    [Fact]
    public void Light_transform_definition_layers_preserve_omissions_and_native_signed_height()
    {
        using var map = new Fixture(false);
        string directory = Path.Combine(map.Root, "SYSTEM", "DATA_MP", "DEFAULTS"); Directory.CreateDirectory(directory);
        string[] fields = Enumerable.Repeat("0", 72).ToArray();
        fields[0] = "555"; fields[1] = "1"; fields[66] = "2"; fields[67] = "65529"; fields[70] = "10"; fields[71] = "90";
        File.WriteAllText(Path.Combine(directory, "objdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
        File.WriteAllText(Path.Combine(directory, "objdef.d09"), "[ObjectDefaults]\n555,1\n");
        var definition = NativeObjectDefinitions.Load(map.Root)[555];
        Assert.Equal(-7, definition.DirectLightHeight); Assert.Equal(10f, definition.DirectLightDistance); Assert.Equal(90f, definition.DirectLightAngle);
        var position = definition.DirectLightPosition(new(100, 200, 300), 180);
        Assert.InRange(Math.Abs(position.X - 100), 0, .0001f); Assert.Equal(193f, position.Y); Assert.Equal(310f, position.Z);
        Assert.Throws<ArgumentOutOfRangeException>(() => definition.DirectLightPosition(new(float.NaN, 0, 0), 0));
        fields[70] = "NaN";
        File.WriteAllText(Path.Combine(directory, "objdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
        Assert.Throws<InvalidDataException>(() => NativeObjectDefinitions.Load(map.Root));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Copy_move_remove_save_and_rollback_keep_independent_light_backlinks(bool pfil)
    {
        using var map = new Fixture(pfil);
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        var template = Assert.Single(store.Templates());
        Assert.Equal(1, store.Add(template, 100, 200, 300, .5f));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        var lights = NativeLightPool.Load(map.LightPath);
        Assert.Equal(2, lights.ActiveLights().Count);
        Assert.Equal(new NativeSavedLight(1, 1, new(100, 205, 300), 1, -1, 2), lights.ActiveLights()[1]);
        byte[] payload = lights.ToPayload();
        Assert.Equal(123u, BinaryPrimitives.ReadUInt32LittleEndian(payload.AsSpan(8 + 1024 * 42 + 4)));
        Assert.Equal(333f, BinaryPrimitives.ReadSingleLittleEndian(payload.AsSpan(8 + 1024 * 32 + 4)));
        Assert.Equal((ushort)1, map.ReadColumn(12, 1));
        Assert.DoesNotContain(NativePoolDiagnostics.DiagnoseDirectory(map.Data).Issues, issue => issue.TargetPool == "light");

        store = LevelObjectStore.Load(map.Map);
        uint uid = store.UidAt(1)!.Value;
        var owned = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(1, uid));
        Assert.True(store.RemoveIfUid(1, uid));
        Assert.Equal(1, store.Add(owned, 110, 210, 310, .7f));
        var saved = map.Files();
        Assert.Throws<IOException>((Action)(() =>
        {
            using var rollback = new FileRollbackScope(); store.Save(map.Map, rollback);
            throw new IOException("simulate a later map-save failure");
        }));
        foreach (var (path, bytes) in saved) Assert.Equal(bytes, File.ReadAllBytes(path));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        Assert.Equal(new System.Numerics.Vector3(110, 215, 310), NativeLightPool.Load(map.LightPath).ActiveLights()[1].Position);
        Assert.True(store.Remove(1));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        Assert.Single(NativeLightPool.Load(map.LightPath).ActiveLights());
        Assert.Equal(ushort.MaxValue, map.ReadColumn(12, 1));
        Assert.Equal(before[map.LightPath], File.ReadAllBytes(map.LightPath));
    }

    [Fact]
    public void Official_palette_initializes_current_light_definition_instead_of_saved_copy()
    {
        using var map = new Fixture(false);
        var template = LevelObjectStore.LoadOfficialTemplates(map.Root, _ => true)[555];
        var definitions = new Dictionary<int, NativeObjectDefinition>
        { [555] = new(555, true, 0) { DirectLightDefinition = 2 } };
        var lights = NativeLightDefinitions.Parse("[LightDefault]\n2,1,.2,.4,.6,700,2,.25,9,Current");
        var store = LevelObjectStore.Load(map.Map, definitions, lights);
        Assert.Equal(1, store.Add(template, 100, 200, 300, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        byte[] bytes = NativeLightPool.Load(map.LightPath).ToPayload();
        Assert.Equal(700f, BinaryPrimitives.ReadSingleLittleEndian(bytes.AsSpan(8 + 1024 * 28 + 4)));
        Assert.Equal(700f, BinaryPrimitives.ReadSingleLittleEndian(bytes.AsSpan(8 + 1024 * 32 + 4)));
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(8 + 1024 * 42 + 4)));
        Assert.Equal(new System.Numerics.Vector3(100, 200, 300), NativeLightPool.Load(map.LightPath).ActiveLights()[1].Position);
    }

    [Theory]
    [InlineData("light-full")]
    [InlineData("object-full")]
    [InlineData("position-full")]
    public void Capacity_failure_leaves_every_pool_unchanged(string pool)
    {
        using var map = new Fixture(false);
        var template = Assert.Single(LevelObjectStore.Load(map.Map).Templates());
        if (pool == "light-full")
        {
            byte[] bytes = File.ReadAllBytes(map.LightPath);
            for (int i = 0; i < 1024; i++) U32(bytes, 8 + i * 4, 1);
            File.WriteAllBytes(map.LightPath, bytes);
        }
        else
        {
            string path = Path.Combine(map.Data, pool == "object-full" ? "objects.dat" : "position.dat");
            byte[] bytes = File.ReadAllBytes(path);
            for (int i = 0; i < (pool == "object-full" ? 4 : 10); i++) bytes[(pool == "object-full" ? 16 : 8) + i * (pool == "object-full" ? 79 : 17)] = 1;
            // objects loader needs an empty record; keep slot3 empty but reserve it through a child parent.
            if (pool == "object-full") { bytes[16 + 3 * 79] = 0; U16(bytes, Fixture.Column(13, 0), 3); }
            File.WriteAllBytes(path, bytes);
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        Assert.Equal(-1, store.Add(template, 100, 200, 300, 0));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData("missing")]
    [InlineData("owner")]
    [InlineData("parent")]
    [InlineData("duplicate")]
    public void Broken_or_unimplemented_light_family_is_not_released_or_borrowed(string fault)
    {
        using var map = new Fixture(false);
        byte[] bytes = File.ReadAllBytes(map.LightPath);
        if (fault == "missing") File.Delete(map.LightPath);
        else
        {
            if (fault == "owner") U16(bytes, 8 + 1024 * 50, 3);
            if (fault == "parent") U16(bytes, 8 + 1024 * 52, 0);
            if (fault == "duplicate")
            {
                U32(bytes, 12, 1); U16(bytes, 8 + 1024 * 50 + 2, 0);
            }
            File.WriteAllBytes(map.LightPath, bytes);
        }
        var before = map.Files(); var store = LevelObjectStore.Load(map.Map);
        Assert.Empty(store.Templates()); Assert.False(store.Remove(0)); Assert.Null(store.OwnedTemplate(0, 100));
        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var (path, source) in before) Assert.Equal(source, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData(false, "parent")]
    [InlineData(true, "parent")]
    [InlineData(false, "particle")]
    [InlineData(true, "particle")]
    [InlineData(false, "small-camp")]
    [InlineData(true, "small-camp")]
    [InlineData(false, "large-camp")]
    [InlineData(true, "large-camp")]
    public void Scenario_candidate_family_copy_creates_new_root_binding_and_preserves_source_without_catalog(bool pfil, string kind)
    {
        using var map = kind switch
        {
            "particle" => ParticleFamilyFixture(pfil, 16),
            "small-camp" => CampFamilyFixture(pfil, false, 16),
            "large-camp" => CampFamilyFixture(pfil, true, 16),
            _ => FamilyFixture(pfil, 16)
        };
        var store = LevelObjectStore.Load(map.Map);
        var sourceObjectsBefore = store.Objects();
        var sourceFilesBefore = map.Files();
        var sourceRoot = sourceObjectsBefore.Single(obj => obj.Slot == 0);
        var sourceFamily = store.FamilyAt(0, 100)!;

        Guid sourceId = Guid.NewGuid();
        Guid copyTargetId = Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", sourceRoot.X, sourceRoot.Z, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = sourceId };
        var copySpawn = new ScenarioSpawn("BauTest", sourceRoot.X + 1.25f, sourceRoot.Z + 2.0f, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y - .5f, Prebuilt: true) { Id = copyTargetId };

        var sourceBinding = new ScenarioDataSlot(0, 100) { SpawnId = sourceId };
        var previous = new ScenarioDocument
        {
            Spawns = [sourceSpawn],
            DataSlots = [sourceBinding]
        };
        var next = new ScenarioDocument
        {
            Spawns = [sourceSpawn, copySpawn],
            DataSlots = previous.DataSlots.ToList()
        };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyTargetId] = new(0, 100)
        };

        var previousSpawnsBefore = previous.Spawns.Select(s => s with { }).ToList();
        var previousSlotsBefore = previous.DataSlots.Select(d => d with { }).ToList();
        var nextSpawnsBefore = next.Spawns.Select(s => s with { }).ToList();
        var nextSlotsBefore = next.DataSlots.Select(d => d with { }).ToList();

        int catalogCallCount = 0;
        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next, _ => { catalogCallCount++; return null; }, requests);

        // templateFor is not called for family copy or retained family
        Assert.Equal(0, catalogCallCount);
        Assert.Empty(result.Skipped);

        // Inputs store and documents are completely unmodified
        Assert.Equal(sourceObjectsBefore, store.Objects());
        Assert.Equal(previousSpawnsBefore, previous.Spawns);
        Assert.Equal(previousSlotsBefore, previous.DataSlots);
        Assert.Equal(nextSpawnsBefore, next.Spawns);
        Assert.Equal(nextSlotsBefore, next.DataSlots);

        // Result documents have exactly 2 prebuilt bindings (source and copy root only, no children bindings)
        Assert.Equal(2, result.Scenario.DataSlots.Count);
        var resultSourceBinding = Assert.Single(result.Scenario.DataSlots.Where(b => b.SpawnId == sourceId));
        var resultCopyBinding = Assert.Single(result.Scenario.DataSlots.Where(b => b.SpawnId == copyTargetId));

        Assert.Equal(0, resultSourceBinding.Slot);
        Assert.Equal(100u, resultSourceBinding.Uid);
        Assert.NotEqual(0, resultCopyBinding.Slot);
        Assert.NotEqual(100u, resultCopyBinding.Uid);

        // Copied family is closed and independent in candidate store
        var copiedFamily = Assert.IsType<LevelObjectFamily>(result.Store.FamilyAt(resultCopyBinding.Slot, resultCopyBinding.Uid));
        Assert.Equal(sourceFamily.ObjectSlots.Count, copiedFamily.ObjectSlots.Count);
        Assert.Equal(sourceFamily.LightSlots.Count, copiedFamily.LightSlots.Count);
        Assert.Equal(sourceFamily.ParticleSlots.Count, copiedFamily.ParticleSlots.Count);
        Assert.Equal(sourceFamily.LagerSlots.Count, copiedFamily.LagerSlots.Count);
        Assert.Equal(sourceFamily.BigLagerSlots.Count, copiedFamily.BigLagerSlots.Count);
        Assert.DoesNotContain(copiedFamily.ObjectSlots, sourceFamily.ObjectSlots.Contains);

        var copiedRootObj = result.Store.Objects().Single(obj => obj.Slot == resultCopyBinding.Slot);
        Assert.Equal(sourceRoot.X + 1.25f, copiedRootObj.X);
        Assert.Equal(sourceRoot.Y - .5f, copiedRootObj.Y);
        Assert.Equal(sourceRoot.Z + 2.0f, copiedRootObj.Z);

        // Candidate can be saved and reloaded
        using (var rollback = new FileRollbackScope())
        {
            result.Store.Save(map.Map, rollback);
            var reloaded = LevelObjectStore.Load(map.Map);
            Assert.NotNull(reloaded.FamilyAt(0, 100));
            Assert.NotNull(reloaded.FamilyAt(resultCopyBinding.Slot, resultCopyBinding.Uid));
        }
        foreach (var pair in sourceFilesBefore)
            Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Scenario_candidate_family_copy_sequential_allocations_are_independent(bool pfil)
    {
        using var map = FamilyFixture(pfil, 20);
        var store = LevelObjectStore.Load(map.Map);
        var sourceRoot = store.Objects().Single(obj => obj.Slot == 0);

        Guid sourceId = Guid.NewGuid();
        Guid copy1Id = Guid.NewGuid();
        Guid copy2Id = Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", sourceRoot.X, sourceRoot.Z, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = sourceId };
        var copy1Spawn = new ScenarioSpawn("BauTest", sourceRoot.X + 1.25f, sourceRoot.Z + 2.0f, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = copy1Id };
        var copy2Spawn = new ScenarioSpawn("BauTest", sourceRoot.X + 2.5f, sourceRoot.Z + 4.0f, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = copy2Id };

        var previous = new ScenarioDocument { Spawns = [sourceSpawn], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = sourceId }] };
        var next = new ScenarioDocument { Spawns = [sourceSpawn, copy1Spawn, copy2Spawn], DataSlots = previous.DataSlots.ToList() };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copy1Id] = new(0, 100),
            [copy2Id] = new(0, 100)
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next, _ => null, requests);
        Assert.Empty(result.Skipped);
        Assert.Equal(3, result.Scenario.DataSlots.Count);

        var copy1Binding = result.Scenario.DataSlots.Single(b => b.SpawnId == copy1Id);
        var copy2Binding = result.Scenario.DataSlots.Single(b => b.SpawnId == copy2Id);

        Assert.NotEqual(copy1Binding.Slot, copy2Binding.Slot);
        Assert.NotEqual(copy1Binding.Uid, copy2Binding.Uid);

        var family1 = result.Store.FamilyAt(copy1Binding.Slot, copy1Binding.Uid)!;
        var family2 = result.Store.FamilyAt(copy2Binding.Slot, copy2Binding.Uid)!;

        Assert.Empty(family1.ObjectSlots.Intersect(family2.ObjectSlots));
        Assert.Empty(family1.LightSlots.Intersect(family2.LightSlots));
    }

    [Theory]
    [InlineData("delete-source")]
    [InlineData("move-source")]
    public void Scenario_candidate_family_copy_uses_source_snapshot_when_source_is_deleted_or_moved_in_next(string mode)
    {
        using var map = FamilyFixture(false, 16);
        var store = LevelObjectStore.Load(map.Map);
        var sourceRoot = store.Objects().Single(obj => obj.Slot == 0);

        Guid sourceId = Guid.NewGuid();
        Guid copyTargetId = Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", sourceRoot.X, sourceRoot.Z, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = sourceId };
        var copySpawn = new ScenarioSpawn("BauTest", sourceRoot.X + 1.25f, sourceRoot.Z + 2.0f, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = copyTargetId };

        var previous = new ScenarioDocument { Spawns = [sourceSpawn], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = sourceId }] };
        var next = new ScenarioDocument
        {
            Spawns = mode == "delete-source" ? [copySpawn] : [sourceSpawn with { X = sourceRoot.X + 5.0f }, copySpawn],
            DataSlots = previous.DataSlots.ToList()
        };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyTargetId] = new(0, 100)
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next, _ => null, requests);
        Assert.Empty(result.Skipped);

        var copyBinding = Assert.Single(result.Scenario.DataSlots.Where(b => b.SpawnId == copyTargetId));
        var copyFamily = result.Store.FamilyAt(copyBinding.Slot, copyBinding.Uid);
        Assert.NotNull(copyFamily);

        var copyRootObj = result.Store.Objects().Single(obj => obj.Slot == copyBinding.Slot);
        Assert.Equal(sourceRoot.X + 1.25f, copyRootObj.X);

        if (mode == "delete-source")
        {
            Assert.Null(result.Store.FamilyAt(0, 100));
            Assert.DoesNotContain(result.Scenario.DataSlots, b => b.SpawnId == sourceId);
        }
        else
        {
            var movedSourceRoot = result.Store.Objects().Single(obj => obj.Slot == 0);
            Assert.Equal(sourceRoot.X + 5.0f, movedSourceRoot.X);
        }
    }

    [Theory]
    [InlineData("stale-uid")]
    [InlineData("team")]
    [InlineData("angle")]
    [InlineData("alias")]
    [InlineData("non-prebuilt")]
    [InlineData("existing-target-guid")]
    [InlineData("fraction")]
    public void Scenario_candidate_family_copy_validation_failures_leave_inputs_unchanged(string fault)
    {
        using var map = FamilyFixture(false, 16);
        var store = LevelObjectStore.Load(map.Map);
        var sourceObjectsBefore = store.Objects();
        var filesBefore = map.Files();
        var sourceRoot = sourceObjectsBefore.Single(obj => obj.Slot == 0);

        Guid sourceId = Guid.NewGuid();
        Guid copyTargetId = fault == "existing-target-guid" ? sourceId : Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", sourceRoot.X, sourceRoot.Z, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = sourceId };
        var copySpawn = new ScenarioSpawn(
            fault == "alias" ? "BauDifferent" : "BauTest",
            fault == "fraction" ? sourceRoot.X + 1.000001f : sourceRoot.X + 1.25f,
            sourceRoot.Z + 2.0f,
            fault == "team" ? 2 : sourceRoot.Team,
            Angle: fault == "angle" ? 90 : 0,
            Y: sourceRoot.Y,
            Prebuilt: fault != "non-prebuilt")
        {
            Id = copyTargetId
        };

        var previous = new ScenarioDocument { Spawns = [sourceSpawn], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = sourceId }] };
        var next = new ScenarioDocument
        {
            Spawns = fault == "existing-target-guid" ? [copySpawn] : [sourceSpawn, copySpawn],
            DataSlots = previous.DataSlots.ToList()
        };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyTargetId] = new(0, fault == "stale-uid" ? 999u : 100u)
        };

        var prevSpawnsBefore = previous.Spawns.Select(s => s with { }).ToList();
        var prevSlotsBefore = previous.DataSlots.Select(d => d with { }).ToList();
        var nextSpawnsBefore = next.Spawns.Select(s => s with { }).ToList();
        var nextSlotsBefore = next.DataSlots.Select(d => d with { }).ToList();

        Assert.ThrowsAny<Exception>(() => ScenarioLevelObjects.ApplyToCandidate(store, previous, next, _ => null, requests));

        // Inputs verified unchanged
        Assert.Equal(sourceObjectsBefore, store.Objects());
        Assert.Equal(prevSpawnsBefore, previous.Spawns);
        Assert.Equal(prevSlotsBefore, previous.DataSlots);
        Assert.Equal(nextSpawnsBefore, next.Spawns);
        Assert.Equal(nextSlotsBefore, next.DataSlots);

        using (var rollback = new FileRollbackScope()) { store.Save(map.Map, rollback); rollback.Commit(); }
        foreach (var pair in filesBefore) Assert.Equal(pair.Value, File.ReadAllBytes(pair.Key));
    }

    [Fact]
    public void Scenario_candidate_without_family_copy_preserves_retained_and_template_behavior()
    {
        using var map = FamilyFixture(false, 16);
        var store = LevelObjectStore.Load(map.Map);
        var sourceRoot = store.Objects().Single(obj => obj.Slot == 0);
        var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(3, store.UidAt(3)!.Value));

        Guid retainedId = Guid.NewGuid();
        Guid newTemplateId = Guid.NewGuid();

        var retainedSpawn = new ScenarioSpawn("BauTest", sourceRoot.X, sourceRoot.Z, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = retainedId };
        var templateSpawn = new ScenarioSpawn("BauNew", 800, 900, 0, Angle: 0, Y: 10, Prebuilt: true) { Id = newTemplateId };

        var previous = new ScenarioDocument { Spawns = [retainedSpawn], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = retainedId }] };
        var next = new ScenarioDocument
        {
            Spawns = [retainedSpawn with { X = sourceRoot.X + 1.25f }, templateSpawn],
            DataSlots = previous.DataSlots.ToList()
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next, spawn => spawn.Id == newTemplateId ? template : null);
        Assert.Empty(result.Skipped);
        Assert.Equal(2, result.Scenario.DataSlots.Count);

        var retainedBinding = result.Scenario.DataSlots.Single(b => b.SpawnId == retainedId);
        var templateBinding = result.Scenario.DataSlots.Single(b => b.SpawnId == newTemplateId);

        Assert.Equal(0, retainedBinding.Slot);
        Assert.Equal(100u, retainedBinding.Uid);
        Assert.Equal(retainedSpawn.X + 1.25f, result.Store.Objects().Single(o => o.Slot == 0).X);

        Assert.True(templateBinding.Slot >= 0);
        Assert.NotEqual(0, templateBinding.Slot);
        Assert.Equal(800f, result.Store.Objects().Single(o => o.Slot == templateBinding.Slot).X);
    }

    [Fact]
    public void Scenario_apply_mutates_store_in_place_and_updates_next_bindings()
    {
        using var map = FamilyFixture(false, 16);
        var store = LevelObjectStore.Load(map.Map);
        var sourceRoot = store.Objects().Single(obj => obj.Slot == 0);
        var initialObjects = store.Objects();
        var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(3, store.UidAt(3)!.Value));

        Guid retainedId = Guid.NewGuid();
        Guid newTemplateId = Guid.NewGuid();

        var retainedSpawn = new ScenarioSpawn("BauTest", sourceRoot.X, sourceRoot.Z, sourceRoot.Team, Angle: 0, Y: sourceRoot.Y, Prebuilt: true) { Id = retainedId };
        var templateSpawn = new ScenarioSpawn("BauNew", 800, 900, 0, Angle: 0, Y: 10, Prebuilt: true) { Id = newTemplateId };

        var previous = new ScenarioDocument { Spawns = [retainedSpawn], DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = retainedId }] };
        var next = new ScenarioDocument
        {
            Spawns = [retainedSpawn with { X = sourceRoot.X + 1.25f, Y = sourceRoot.Y - .5f, Z = sourceRoot.Z + 2.0f }, templateSpawn],
            DataSlots = previous.DataSlots.ToList()
        };

        var skipped = ScenarioLevelObjects.Apply(store, previous, next, spawn => spawn.Id == newTemplateId ? template : null);
        Assert.Empty(skipped);

        // next bindings are mutated in place
        Assert.Equal(2, next.DataSlots.Count);
        var retainedBinding = next.DataSlots.Single(b => b.SpawnId == retainedId);
        var templateBinding = next.DataSlots.Single(b => b.SpawnId == newTemplateId);

        Assert.Equal(0, retainedBinding.Slot);
        Assert.Equal(100u, retainedBinding.Uid);
        Assert.True(templateBinding.Slot >= 0);
        Assert.NotEqual(0, templateBinding.Slot);

        // store is mutated directly in place (not a discarded clone)
        var movedRoot = store.Objects().Single(o => o.Slot == 0);
        Assert.Equal(sourceRoot.X + 1.25f, movedRoot.X);
        Assert.Equal(sourceRoot.Y - .5f, movedRoot.Y);
        Assert.Equal(sourceRoot.Z + 2.0f, movedRoot.Z);

        var addedObject = store.Objects().Single(o => o.Slot == templateBinding.Slot);
        Assert.Equal(800f, addedObject.X);
        Assert.Equal(900f, addedObject.Z);
        Assert.Equal(templateBinding.Uid, addedObject.Uid);

        // Calling Save on the passed-in store persists the applied translation and additions
        using (var rollback = new FileRollbackScope())
        {
            store.Save(map.Map, rollback);
            rollback.Commit();
        }
        var reloaded = LevelObjectStore.Load(map.Map);
        var reloadedRoot = reloaded.Objects().Single(o => o.Slot == 0);
        Assert.Equal(sourceRoot.X + 1.25f, reloadedRoot.X);
        var reloadedAdded = reloaded.Objects().Single(o => o.Slot == templateBinding.Slot);
        Assert.Equal(800f, reloadedAdded.X);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Full_objects_pool_candidate_clones_saves_and_recycles_after_remove(bool pfil)
    {
        // FamilyFixture(count=7): 4 active objects (root family 3 + independent slot 3), 3 free slots (4, 5, 6).
        // Copying root family (3 objects) consumes all 3 free slots, exactly filling the 7-object pool to capacity.
        using var map = FamilyFixture(pfil, count: 7, withEngine: true);
        var source = LevelObjectStore.Load(map.Map);
        var initialObjects = source.Objects();
        var initialFiles = map.Files();
        Assert.Equal(4, initialObjects.Count);
        uint? initialEngineCursor = map.ReadEngineCursor();
        Assert.Equal(104u, initialEngineCursor); // FamilyFixture adds three objects after root UID 100.

        // 1. Plan is non-null and exactly fills the remaining capacity
        var plan = source.PlanFamilyCoreAllocation(0, 100);
        Assert.NotNull(plan);
        Assert.Equal(3, plan.Objects.Count);
        Assert.Equal(new[] { 4, 5, 6 }, plan.Objects.Values.OrderBy(s => s).ToArray());

        // 2. CreateFamilyCopyCandidate succeeds to exactly full 7 objects without throwing
        var candidateResult = source.CreateFamilyCopyCandidate(0, 100, 81920, -32768, 131072);
        Assert.NotNull(candidateResult);
        var candidate = candidateResult.Store;
        Assert.Equal(7, candidate.Capacity);
        Assert.Equal(7, candidate.Objects().Count);

        // 3. Source store and files remain unchanged
        Assert.Equal(initialObjects, source.Objects());
        foreach (var (path, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(path));

        // 4. Save and reload full 7 objects: both new and old families are closed
        using (var rollback = new FileRollbackScope())
        {
            candidate.Save(map.Map, rollback);
            rollback.Commit();
        }
        var reloadedFull = LevelObjectStore.Load(map.Map);
        Assert.Equal(7, reloadedFull.Objects().Count);
        var oldFamily = reloadedFull.FamilyAt(0, 100);
        Assert.NotNull(oldFamily);
        Assert.Equal(new[] { 0, 1, 2 }, oldFamily.ObjectSlots.OrderBy(s => s).ToArray());

        int newRoot = candidateResult.RootSlot;
        uint newUid = candidateResult.RootUid;
        var copiedFamily = reloadedFull.FamilyAt(newRoot, newUid);
        Assert.NotNull(copiedFamily);
        Assert.Equal(new[] { 4, 5, 6 }, copiedFamily.ObjectSlots.OrderBy(s => s).ToArray());

        // 5. CloneForEditing on full store preserves all bytes without throwing
        var clonedFull = reloadedFull.CloneForEditing();
        Assert.Equal(7, clonedFull.Objects().Count);
        Assert.NotNull(clonedFull.FamilyAt(0, 100));
        Assert.NotNull(clonedFull.FamilyAt(newRoot, newUid));

        // 6. Loaded full map Add returns -1 and leaves original bytes unchanged
        var candidateTemplate = clonedFull.OwnedTemplate(3, clonedFull.UidAt(3)!.Value);
        Assert.NotNull(candidateTemplate);
        var fullBytesBeforeAdd = map.Files();
        Assert.Equal(-1, clonedFull.Add(candidateTemplate, 500, 20, 30, 0));
        using (var rollback = new FileRollbackScope())
        {
            clonedFull.Save(map.Map, rollback);
            rollback.Commit();
        }
        foreach (var (path, bytes) in fullBytesBeforeAdd)
            Assert.Equal(bytes, File.ReadAllBytes(path));

        // 7. Remove copied family: original 4 objects remain fully unchanged across Save and reload
        Assert.True(clonedFull.RemoveFamilyIfUid(newRoot, newUid));
        Assert.Equal(4, clonedFull.Objects().Count);
        using (var rollback = new FileRollbackScope())
        {
            clonedFull.Save(map.Map, rollback);
            rollback.Commit();
        }
        var reloadedAfterRemove = LevelObjectStore.Load(map.Map);
        Assert.Equal(4, reloadedAfterRemove.Objects().Count);
        var originalFour = reloadedAfterRemove.Objects().OrderBy(o => o.Slot).ToList();
        var initialFour = initialObjects.OrderBy(o => o.Slot).ToList();
        for (int i = 0; i < 4; i++)
        {
            Assert.Equal(initialFour[i].Slot, originalFour[i].Slot);
            Assert.Equal(initialFour[i].Uid, originalFour[i].Uid);
            Assert.Equal(initialFour[i].TypeId, originalFour[i].TypeId);
            Assert.Equal(initialFour[i].Team, originalFour[i].Team);
            Assert.Equal(initialFour[i].X, originalFour[i].X);
            Assert.Equal(initialFour[i].Y, originalFour[i].Y);
            Assert.Equal(initialFour[i].Z, originalFour[i].Z);
            Assert.Equal(initialFour[i].Rotation, originalFour[i].Rotation);
            Assert.Equal(initialFour[i].Linked, originalFour[i].Linked);
        }

        // 8. Reallocate can reuse recycled slots, UIDs do not collide
        var reallocatedSlot = reloadedAfterRemove.Add(candidateTemplate, 600, 20, 30, 0);
        Assert.True(reallocatedSlot is 4 or 5 or 6);
        uint? reallocatedUid = reloadedAfterRemove.UidAt(reallocatedSlot);
        Assert.NotNull(reallocatedUid);
        Assert.DoesNotContain(initialObjects, o => o.Uid == reallocatedUid.Value);
        Assert.DoesNotContain(candidate.Objects(), o => o.Uid == reallocatedUid.Value);

        using (var rollback = new FileRollbackScope())
        {
            reloadedAfterRemove.Save(map.Map, rollback);
            rollback.Commit();
        }

        // 9. engine.dat cursor progressed and retired UIDs are preserved in history
        uint? finalCursor = map.ReadEngineCursor();
        Assert.NotNull(finalCursor);
        Assert.True(finalCursor.Value >= reallocatedUid.Value + 1);
    }

    private static void U16(byte[] bytes, int offset, ushort value) => BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset), value);
    private static void U32(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
    private static void F32(byte[] bytes, int offset, float value) => BinaryPrimitives.WriteSingleLittleEndian(bytes.AsSpan(offset), value);

    private sealed class Fixture : IDisposable
    {
        public string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmDirectLight_" + Guid.NewGuid().ToString("N"));
        public string Map => Path.Combine(Root, "MAPS", "TEST");
        public string Data => Path.Combine(Map, "DATA");
        public string LightPath => Path.Combine(Data, "light.dat");
        private readonly int _count;
        public static int Column(int column, int slot, int count = 4) => 16 + count * (79 + LevelObjectStore.ColumnWidths.Take(column).Sum()) + slot * LevelObjectStore.ColumnWidths[column];
        public Fixture(bool pfil, int count = 4, bool withEngine = false)
        {
            _count = count;
            Directory.CreateDirectory(Data);
            byte[] objects = new byte[16 + count * (79 + LevelObjectStore.ColumnWidths.Sum())];
            U32(objects, 0, 1); U32(objects, 4, (uint)count); U32(objects, 8, 30); U32(objects, 12, 30);
            for (int slot = 0; slot < count; slot++)
            {
                for (int column = 0; column < 18; column++)
                    if (LevelObjectStore.ColumnWidths[column] == 2) U16(objects, Column(column, slot, count), ushort.MaxValue);
                foreach (int field in new[] { 67, 69, 71, 73, 75, 77 }) U16(objects, 16 + slot * 79 + field, ushort.MaxValue);
            }
            objects[16] = 1; U32(objects, 19, 100); U16(objects, 16 + 75, 555);
            U16(objects, 16 + 67, 0); U16(objects, 16 + 69, 1); U16(objects, Column(1, 0, count), 0); U16(objects, Column(12, 0, count), 0);
            byte[] data = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()]; U32(data, 0, 1); U32(data, 4, (uint)count); data[8] = 1;
            int positionCount = Math.Max(10, count * 2);
            byte[] positions = new byte[8 + positionCount * 17]; U32(positions, 0, 1); U32(positions, 4, (uint)positionCount);
            for (int slot = 0; slot < 2; slot++) { positions[8 + slot * 17] = 1; F32(positions, 9 + slot * 17, 10); F32(positions, 13 + slot * 17, 20); F32(positions, 17 + slot * 17, 30); }
            byte[] light = new byte[NativeLightPool.PayloadLength]; U32(light, 0, 1); U32(light, 4, 1024);
            light.AsSpan(8 + 1024 * 50).Fill(255);
            var pool = NativeLightPool.ParsePayload(light);
            pool.AddBatch([new(new(10, 25, 30), new(1.2f, .8f, .4f), 350, 1, .1f, 7, 2, 0)]);
            light = pool.ToPayload(); U32(light, 8 + 1024 * 42, 123); F32(light, 8 + 1024 * 32, 333);
            Write("objects.dat", objects); Write("objdata.dat", data); Write("position.dat", positions); Write("light.dat", light);
            if (withEngine)
            {
                byte[] engine = new byte[94];
                U32(engine, 0, 1);
                U32(engine, 34, 101); // Initial cursor
                Write("engine.dat", engine);
            }
            void Write(string name, byte[] payload)
            {
                byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
                File.WriteAllBytes(Path.Combine(Data, name), pfil ? GameLZSS.CompressPfil(payload, header) : payload);
            }
        }
        public Dictionary<string, byte[]> Files() => Directory.GetFiles(Data).ToDictionary(path => path, File.ReadAllBytes);
        public ushort ReadColumn(int column, int slot) => BinaryPrimitives.ReadUInt16LittleEndian(GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(Data, "objects.dat"))).AsSpan(Column(column, slot, _count)));
        public uint? ReadEngineCursor()
        {
            string path = Path.Combine(Data, "engine.dat");
            if (!File.Exists(path)) return null;
            byte[] raw = File.ReadAllBytes(path);
            byte[] data = raw.Length >= 64 && raw[0] == 'P' && raw[1] == 'F' && raw[2] == 'I' && raw[3] == 'L'
                ? GameLZSS.DecompressPfil(raw)
                : raw;
            return BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(34));
        }
        public void Dispose() => Directory.Delete(Root, true);
    }
}
