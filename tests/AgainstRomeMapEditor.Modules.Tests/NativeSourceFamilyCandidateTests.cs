using System.Buffers.Binary;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed partial class NativeSourceFamilyCandidateTests
{
    private static void U16(byte[] bytes, int offset, ushort value) => BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset), value);
    private static void U32(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
    private static void F32(byte[] bytes, int offset, float value) => BinaryPrimitives.WriteSingleLittleEndian(bytes.AsSpan(offset), value);

    private sealed class NativeFixture : IDisposable
    {
        public string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmNativeFamily_" + Guid.NewGuid().ToString("N"));
        public string Map => Path.Combine(Root, "MAPS", "TEST");
        public string Data => Path.Combine(Map, "DATA");
        public string LightPath => Path.Combine(Data, "light.dat");

        public static int Column(int column, int slot, int count) =>
            16 + count * (79 + LevelObjectStore.ColumnWidths.Take(column).Sum()) + slot * LevelObjectStore.ColumnWidths[column];

        public NativeFixture(bool pfil, int count = 8, float rootRotationRadians = 0f)
        {
            Directory.CreateDirectory(Data);

            byte[] objects = new byte[16 + count * (79 + LevelObjectStore.ColumnWidths.Sum())];
            U32(objects, 0, 1);
            U32(objects, 4, (uint)count);
            U32(objects, 8, 30);
            U32(objects, 12, 30);

            for (int slot = 0; slot < count; slot++)
            {
                for (int column = 0; column < 18; column++)
                    if (LevelObjectStore.ColumnWidths[column] == 2)
                        U16(objects, Column(column, slot, count), ushort.MaxValue);
                foreach (int field in new[] { 67, 69, 71, 73, 75, 77 })
                    U16(objects, 16 + slot * 79 + field, ushort.MaxValue);
            }

            // Slot 0: Root UID 100, TypeId 555, Team 0, parent 0xFFFF
            objects[16] = 1;
            U32(objects, 19, 100);
            U16(objects, 16 + 75, 555); // TypeId
            U16(objects, 16 + 67, 0);   // Position slot 0
            U16(objects, 16 + 69, 0);
            U16(objects, Column(1, 0, count), 0);
            U16(objects, Column(12, 0, count), 0); // Direct light 0

            byte[] data = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
            U32(data, 0, 1);
            U32(data, 4, (uint)count);
            data[8] = 1;

            int positionCount = Math.Max(12, count * 2);
            byte[] positions = new byte[8 + positionCount * 17];
            U32(positions, 0, 1);
            U32(positions, 4, (uint)positionCount);

            // Position 0 for root: XYZ = (100, 20, 200), rotation in degrees
            float rootRotationDegrees = (float)(rootRotationRadians * (180d / Math.PI));
            positions[8] = 1;
            F32(positions, 9, 100);
            F32(positions, 13, 20);
            F32(positions, 17, 200);
            F32(positions, 21, rootRotationDegrees);

            byte[] light = new byte[NativeLightPool.PayloadLength];
            U32(light, 0, 1);
            U32(light, 4, 1024);
            light.AsSpan(8 + 1024 * 50).Fill(255);
            var pool = NativeLightPool.ParsePayload(light);
            pool.AddBatch([new(new(100, 40, 200), new(1, 1, 1), 350, 0, 0, 0, 2, 0, -1)]);
            light = pool.ToPayload();
            U32(light, 8 + 1024 * 42, 123);
            F32(light, 8 + 1024 * 32, 333);

            byte[] engine = new byte[94];
            U32(engine, 0, 1);
            U32(engine, 34, 101); // Initial cursor

            Write("objects.dat", objects);
            Write("objdata.dat", data);
            Write("position.dat", positions);
            Write("light.dat", light);
            Write("engine.dat", engine);

            void Write(string name, byte[] payload)
            {
                byte[] header = new byte[64];
                "PFIL"u8.CopyTo(header);
                File.WriteAllBytes(Path.Combine(Data, name), pfil ? GameLZSS.CompressPfil(payload, header) : payload);
            }
        }

        public static NativeFixture CreateWithFamily(bool pfil, int count = 8, float rootRotationRadians = 0f)
        {
            var fixture = new NativeFixture(pfil, count, rootRotationRadians);
            var store = LevelObjectStore.Load(fixture.Map);
            var template = Assert.IsType<LevelObjectTemplate>(store.OwnedTemplate(0, 100));

            // Add child slot 1, child slot 2, independent slot 3
            for (int slot = 1; slot < 4; slot++)
                Assert.Equal(slot, store.Add(template, slot * 100, 20, 30, 0));

            using (var rollback = new FileRollbackScope())
            {
                store.Save(fixture.Map, rollback);
                rollback.Commit();
            }

            // Wire parent links in objects.dat: slot 1 parent = 0, slot 2 parent = 1
            string path = Path.Combine(fixture.Data, "objects.dat");
            byte[] packed = File.ReadAllBytes(path);
            byte[] objects = packed.Length >= 64 && packed[0] == 'P' && packed[1] == 'F' && packed[2] == 'I' && packed[3] == 'L'
                ? GameLZSS.DecompressPfil(packed)
                : packed;

            U16(objects, Column(13, 1, count), 0);
            U16(objects, Column(13, 2, count), 1);

            byte[] header = new byte[64];
            "PFIL"u8.CopyTo(header);
            File.WriteAllBytes(path, pfil ? GameLZSS.CompressPfil(objects, header) : objects);

            // Re-apply light pool
            var lights = NativeLightPool.Load(fixture.LightPath);
            lights.AddBatch([
                new(new(110, 40, 210), new(1, 1, 1), 350, 0, 0, 0, 2, -1, 0),
                new(new(120, 40, 220), new(1, 1, 1), 350, 0, 0, 0, 2, -1, 1)
            ]);
            using (var rollback = new FileRollbackScope())
            {
                lights.Save(fixture.LightPath, rollback);
                rollback.Commit();
            }

            return fixture;
        }

        public Dictionary<string, byte[]> Files() => Directory.GetFiles(Data).ToDictionary(path => path, File.ReadAllBytes);
        public void Dispose() => Directory.Delete(Root, true);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Native_unbound_root_candidate_creates_new_root_only_binding_and_preserves_source_bytes_and_documents(bool pfil)
    {
        using var map = NativeFixture.CreateWithFamily(pfil, count: 12, rootRotationRadians: 0f);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument(); // No previous binding for native root
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 100f, root.Z + 150f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), Y: root.Y, Prebuilt: true)
        {
            Id = copyId,
            RawRotation = root.Rotation
        };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next,
            _ => throw new InvalidOperationException("Native copy should not invoke template catalog."),
            requests,
            typeId => typeId == 555 ? "BauTest" : null);

        // 1. Inputs must remain untouched
        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.Spawns);
        Assert.Empty(previous.DataSlots);
        Assert.Single(next.Spawns);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));

        // 2. Result scenario receives ONLY new root binding (not children, not source)
        Assert.Single(result.Scenario.DataSlots);
        var binding = result.Scenario.DataSlots[0];
        Assert.Equal(copyId, binding.SpawnId);
        Assert.NotEqual(0, binding.Slot);
        Assert.NotEqual(100u, binding.Uid);

        // 3. Result store contains both source family and copy family
        Assert.NotNull(result.Store.FamilyAt(0, 100));
        var copiedFamily = result.Store.FamilyAt(binding.Slot, binding.Uid);
        Assert.NotNull(copiedFamily);
        Assert.Equal(3, copiedFamily.ObjectSlots.Count); // Root + 2 children

        var movedRoot = result.Store.Objects().Single(o => o.Slot == binding.Slot);
        Assert.Equal(root.X + 100f, movedRoot.X);
        Assert.Equal(root.Z + 150f, movedRoot.Z);
        Assert.Equal(BitConverter.SingleToInt32Bits(root.Rotation), BitConverter.SingleToInt32Bits(movedRoot.Rotation));

        // 4. Saving candidate store preserves both families
        using (var rollback = new FileRollbackScope())
        {
            result.Store.Save(map.Map, rollback);
            rollback.Commit();
        }
        var reloadedStore = LevelObjectStore.Load(map.Map);
        Assert.NotNull(reloadedStore.FamilyAt(0, 100));
        Assert.NotNull(reloadedStore.FamilyAt(binding.Slot, binding.Uid));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Fractional_radian_rotation_is_retained_bit_exact_across_save_reload_and_retained_resave_move(bool pfil)
    {
        // Fractional radian: 1.234567f (~70.735 degrees, display angle 71)
        float fractionalRadians = 1.234567f;
        int expectedDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(fractionalRadians);
        Assert.Equal(71, expectedDisplayAngle);

        using var map = NativeFixture.CreateWithFamily(pfil, count: 12, rootRotationRadians: fractionalRadians);
        var store = LevelObjectStore.Load(map.Map);
        var root = store.Objects().Single(o => o.Slot == 0);
        Assert.Equal(BitConverter.SingleToInt32Bits(fractionalRadians), BitConverter.SingleToInt32Bits(root.Rotation));

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 50f, root.Z + 75f, root.Team,
            Angle: expectedDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = copyId,
            RawRotation = root.Rotation
        };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next,
            _ => null,
            requests,
            typeId => typeId == 555 ? "BauTest" : null);

        var binding = Assert.Single(result.Scenario.DataSlots);
        var candidateRoot = result.Store.Objects().Single(o => o.Slot == binding.Slot);
        Assert.Equal(BitConverter.SingleToInt32Bits(fractionalRadians), BitConverter.SingleToInt32Bits(candidateRoot.Rotation));

        // Save scenario document and store
        using (var rollback = new FileRollbackScope())
        {
            result.Scenario.Save(map.Map, rollback);
            result.Store.Save(map.Map, rollback);
            rollback.Commit();
        }

        // Reload scenario document and store: verify bit-exact retention
        var reloadedDoc = ScenarioDocument.Load(map.Map);
        var reloadedSpawn = Assert.Single(reloadedDoc.Spawns);
        Assert.Equal(fractionalRadians, reloadedSpawn.RawRotation);
        Assert.Equal(fractionalRadians, reloadedSpawn.RawNativeRotation);
        Assert.Equal(expectedDisplayAngle, reloadedSpawn.Angle);

        var reloadedStore = LevelObjectStore.Load(map.Map);
        var reloadedRoot = reloadedStore.Objects().Single(o => o.Slot == binding.Slot);
        Assert.Equal(BitConverter.SingleToInt32Bits(fractionalRadians), BitConverter.SingleToInt32Bits(reloadedRoot.Rotation));

        // Perform second Save (retained family move): fixed delta translation
        var secondPrevious = reloadedDoc;
        var movedSpawn = reloadedSpawn with
        {
            X = reloadedSpawn.X + 1.25f,
            Y = reloadedSpawn.Y - .5f,
            Z = reloadedSpawn.Z + 2.0f
        };
        var secondNext = new ScenarioDocument
        {
            Spawns = [movedSpawn],
            DataSlots = secondPrevious.DataSlots.ToList()
        };

        var skipped = ScenarioLevelObjects.Apply(reloadedStore, secondPrevious, secondNext, _ => null);
        Assert.Empty(skipped);

        // Binding is retained without allocating new slots
        var secondBinding = Assert.Single(secondNext.DataSlots);
        Assert.Equal(binding.Slot, secondBinding.Slot);
        Assert.Equal(binding.Uid, secondBinding.Uid);

        var movedStoreRoot = reloadedStore.Objects().Single(o => o.Slot == binding.Slot);
        Assert.Equal(candidateRoot.X + 1.25f, movedStoreRoot.X);
        Assert.Equal(candidateRoot.Y - .5f, movedStoreRoot.Y);
        Assert.Equal(candidateRoot.Z + 2.0f, movedStoreRoot.Z);
        Assert.Equal(BitConverter.SingleToInt32Bits(fractionalRadians), BitConverter.SingleToInt32Bits(movedStoreRoot.Rotation));
    }

    [Fact]
    public void Multiple_native_copies_produce_unique_sequential_allocations_and_independent_bindings()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 16);
        var store = LevelObjectStore.Load(map.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        Guid copy1Id = Guid.NewGuid(), copy2Id = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var spawn1 = new ScenarioSpawn("BauTest", root.X + 20f, root.Z + 30f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), Y: root.Y, Prebuilt: true)
        {
            Id = copy1Id,
            RawRotation = root.Rotation
        };
        var spawn2 = new ScenarioSpawn("BauTest", root.X + 40f, root.Z + 60f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), Y: root.Y, Prebuilt: true)
        {
            Id = copy2Id,
            RawRotation = root.Rotation
        };
        var next = new ScenarioDocument { Spawns = [spawn1, spawn2] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copy1Id] = new(0, 100, isNativeSource: true),
            [copy2Id] = new(0, 100, isNativeSource: true)
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(store, previous, next,
            _ => null,
            requests,
            typeId => typeId == 555 ? "BauTest" : null);

        Assert.Equal(2, result.Scenario.DataSlots.Count);
        var b1 = result.Scenario.DataSlots.Single(b => b.SpawnId == copy1Id);
        var b2 = result.Scenario.DataSlots.Single(b => b.SpawnId == copy2Id);

        // Allocations must be unique and disjoint
        Assert.NotEqual(b1.Slot, b2.Slot);
        Assert.NotEqual(b1.Uid, b2.Uid);
        Assert.True(b1.Slot != 0 && b2.Slot != 0);
        Assert.True(b1.Uid != 100 && b2.Uid != 100);

        var family1 = result.Store.FamilyAt(b1.Slot, b1.Uid);
        var family2 = result.Store.FamilyAt(b2.Slot, b2.Uid);
        Assert.NotNull(family1);
        Assert.NotNull(family2);
        Assert.Empty(family1.ObjectSlots.Intersect(family2.ObjectSlots));
    }

    [Fact]
    public void Ordinary_request_without_previous_binding_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument(); // No previous binding
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        // Ordinary request (IsNativeSource is false by default)
        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_with_null_or_missing_resolver_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        // Case A: nativeAliasResolver is null
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests, nativeAliasResolver: null));

        // Case B: nativeAliasResolver returns null
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests, nativeAliasResolver: _ => null));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_with_mismatched_alias_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        // Resolver returns different alias than targetSpawn.Alias
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "OtherAlias"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_with_mismatched_team_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        // Target spawn has team 2, but root has team 0
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, Team: 2,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Theory]
    [InlineData(null, 0)]
    [InlineData(float.NaN, 0)]
    [InlineData(float.PositiveInfinity, 0)]
    [InlineData(0.5f, 0)] // Differs from source 0f
    [InlineData(0f, 180)] // Contradictory display angle: RawRotation is 0f, but Angle is 180
    public void Native_request_with_invalid_raw_rotation_or_contradictory_angle_is_rejected_and_preserves_inputs(float? rawRotation, int angle)
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12, rootRotationRadians: 0f);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: angle, Y: root.Y, Prebuilt: true)
        {
            Id = copyId,
            RawRotation = rawRotation
        };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_with_stale_root_uid_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        // Wrong UID (9999 instead of 100)
        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 9999, isNativeSource: true)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_targeting_child_object_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var child = initialObjects.Single(o => o.Slot == 1);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", child.X + 10f, child.Z + 10f, child.Team,
            Angle: 0, Y: child.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        // Slot 1 is a child object of slot 0, not an independent root
        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(child.Slot, child.Uid, isNativeSource: true)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_with_invalid_closed_family_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);

        // Break independence while retaining the root's complete parent tree:
        // an outside object shares the root's objdata slot.
        string path = Path.Combine(map.Data, "objects.dat");
        byte[] objects = File.ReadAllBytes(path);
        U16(objects, NativeFixture.Column(1, 3, 12), 0);
        File.WriteAllBytes(path, objects);

        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_with_exhausted_capacity_is_rejected_and_preserves_inputs()
    {
        // Fixture with count: 4 (exactly 4 active objects: root family 3 + 1 independent, 0 free slots)
        using var map = NativeFixture.CreateWithFamily(false, count: 4);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };

        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        // Capacity exhausted throws InvalidOperationException
        Assert.Throws<InvalidOperationException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.DataSlots);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Native_request_for_already_bound_root_is_rejected_and_preserves_inputs()
    {
        using var map = NativeFixture.CreateWithFamily(false, count: 12);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        Guid existingId = Guid.NewGuid(), copyId = Guid.NewGuid();
        var existingSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true) { Id = existingId };
        var binding = new ScenarioDataSlot(0, 100) { SpawnId = existingId };
        var previous = new ScenarioDocument { Spawns = [existingSpawn], DataSlots = [binding] };

        var targetSpawn = new ScenarioSpawn("BauTest", root.X + 10f, root.Z + 10f, root.Team,
            Angle: 0, Y: root.Y, Prebuilt: true) { Id = copyId, RawRotation = 0f };
        var next = new ScenarioDocument { Spawns = [existingSpawn, targetSpawn], DataSlots = [binding] };

        // Attempting isNativeSource on an already-bound slot must be rejected
        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, _ => null, requests,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Equal(new[] { binding }, previous.DataSlots);
        Assert.Equal(new[] { binding }, next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Fact]
    public void Scenario_document_load_and_save_validates_raw_rotation_and_is_backward_compatible()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ArmScenarioTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            // 1. Valid spawn with RawRotation and matching Angle
            var validSpawn = new ScenarioSpawn("BauTest", 100, 200, 0, Angle: 57, Y: 10, Prebuilt: true)
            {
                Id = Guid.NewGuid(),
                RawRotation = 1.0f // ~57.296 degrees -> matches 57
            };
            var doc = new ScenarioDocument { Spawns = [validSpawn] };
            using (var rollback = new FileRollbackScope())
            {
                doc.Save(tempDir, rollback);
                rollback.Commit();
            }

            var loaded = ScenarioDocument.Load(tempDir);
            Assert.Equal(6, loaded.Version);
            var loadedSpawn = Assert.Single(loaded.Spawns);
            Assert.Equal(1.0f, loadedSpawn.RawRotation);
            Assert.Equal(1.0f, loadedSpawn.RawNativeRotation);
            Assert.Equal(57, loadedSpawn.Angle);

            // 2. Reject non-finite RawRotation
            var nanDoc = new ScenarioDocument
            {
                Spawns = [validSpawn with { RawRotation = float.NaN }]
            };
            using (var rollback = new FileRollbackScope())
            {
                Assert.Throws<InvalidDataException>(() => nanDoc.Save(tempDir, rollback));
            }

            // 3. Reject contradictory Angle / RawRotation
            var contradictoryDoc = new ScenarioDocument
            {
                Spawns = [validSpawn with { Angle = 180, RawRotation = 0f }]
            };
            using (var rollback = new FileRollbackScope())
            {
                Assert.Throws<InvalidDataException>(() => contradictoryDoc.Save(tempDir, rollback));
            }

            // 4. Backward compatible: legacy spawn without RawRotation loads and saves cleanly
            var legacySpawn = new ScenarioSpawn("BauTest", 100, 200, 0, Angle: 90, Y: 10, Prebuilt: true)
            {
                Id = Guid.NewGuid()
            };
            var legacyDoc = new ScenarioDocument { Spawns = [legacySpawn] };
            using (var rollback = new FileRollbackScope())
            {
                legacyDoc.Save(tempDir, rollback);
                rollback.Commit();
            }
            var loadedLegacy = ScenarioDocument.Load(tempDir);
            var loadedLegacySpawn = Assert.Single(loadedLegacy.Spawns);
            Assert.Null(loadedLegacySpawn.RawRotation);
            Assert.Null(loadedLegacySpawn.RawNativeRotation);
            Assert.Equal(90, loadedLegacySpawn.Angle);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Single_native_object_with_owned_template_candidate_save_reload_resave_preserves_bytes_and_translates_identically(bool pfil)
    {
        float fractionalRadians = 1.234567f;
        int expectedDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(fractionalRadians);

        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: fractionalRadians);
        var initialStore = LevelObjectStore.Load(map.Map);
        var sourceRoot = initialStore.Objects().Single(o => o.Slot == 0);

        // 1. Single native source object has a valid OwnedTemplate available
        var sourceTemplate = initialStore.OwnedTemplate(0, 100);
        Assert.NotNull(sourceTemplate);

        // Initial native copy creation via ApplyToCandidate
        Guid copyId = Guid.NewGuid();
        var previous = new ScenarioDocument();
        var targetSpawn = new ScenarioSpawn("BauTest", sourceRoot.X + 50f, sourceRoot.Z + 75f, sourceRoot.Team,
            Angle: expectedDisplayAngle, Y: sourceRoot.Y, Prebuilt: true)
        {
            Id = copyId,
            RawRotation = sourceRoot.Rotation
        };
        var next = new ScenarioDocument { Spawns = [targetSpawn] };
        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>
        {
            [copyId] = new(0, 100, isNativeSource: true)
        };

        var initialResult = ScenarioLevelObjects.ApplyToCandidate(
            initialStore, previous, next,
            _ => throw new InvalidOperationException("Initial copy should not invoke template callback."),
            requests,
            typeId => typeId == 555 ? "BauTest" : null);

        var initialBinding = Assert.Single(initialResult.Scenario.DataSlots);
        int newSlot = initialBinding.Slot;
        uint newUid = initialBinding.Uid;

        // Save initial candidate
        using (var rollback = new FileRollbackScope())
        {
            initialResult.Scenario.Save(map.Map, rollback);
            initialResult.Store.Save(map.Map, rollback);
            rollback.Commit();
        }

        // 2. Reload store and scenario document
        var reloadedStore = LevelObjectStore.Load(map.Map);
        var reloadedDoc = ScenarioDocument.Load(map.Map);

        // Both the source object and the new root object are single native roots with OwnedTemplate available
        Assert.NotNull(reloadedStore.OwnedTemplate(0, 100));
        Assert.NotNull(reloadedStore.OwnedTemplate(newSlot, newUid));

        var dataFilesBefore = map.Files();
        string scenarioPath = Path.Combine(map.Map, ScenarioDocument.FileName);
        byte[] scenarioBytesBefore = File.ReadAllBytes(scenarioPath);

        // 3. ApplyToCandidate with previous == next, no copy requests, template callback throws if invoked
        var noOpResult = ScenarioLevelObjects.ApplyToCandidate(
            reloadedStore, reloadedDoc, reloadedDoc,
            _ => throw new InvalidOperationException("No-op apply must not invoke template callback."),
            familyCopies: null);

        using (var rollback = new FileRollbackScope())
        {
            noOpResult.Scenario.Save(map.Map, rollback);
            noOpResult.Store.Save(map.Map, rollback);
            rollback.Commit();
        }

        // Entire DATA + Scenario bytes completely unchanged
        Assert.Equal(scenarioBytesBefore, File.ReadAllBytes(scenarioPath));
        foreach (var (filePath, bytes) in dataFilesBefore)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));

        // Reload and verify newroot UID, slot, raw serialized degrees, light opaque
        var reloadedNoOpDoc = ScenarioDocument.Load(map.Map);
        var reloadedNoOpBinding = Assert.Single(reloadedNoOpDoc.DataSlots);
        Assert.Equal(newSlot, reloadedNoOpBinding.Slot);
        Assert.Equal(newUid, reloadedNoOpBinding.Uid);

        var reloadedNoOpStore = LevelObjectStore.Load(map.Map);
        var reloadedNoOpRoot = reloadedNoOpStore.Objects().Single(o => o.Slot == newSlot);
        Assert.Equal(newUid, reloadedNoOpRoot.Uid);
        Assert.Equal(BitConverter.SingleToInt32Bits(sourceRoot.Rotation), BitConverter.SingleToInt32Bits(reloadedNoOpRoot.Rotation));

        // 4. Translate: bindings remain identical, direction raw unchanged, source original objects unchanged
        var movedSpawn = reloadedDoc.Spawns.Single(s => s.Id == copyId) with
        {
            X = targetSpawn.X + 10.0f,
            Z = targetSpawn.Z + 20.0f
        };
        var movedDoc = new ScenarioDocument
        {
            Spawns = [movedSpawn],
            DataSlots = reloadedDoc.DataSlots.Select(d => d with { }).ToList()
        };

        var moveResult = ScenarioLevelObjects.ApplyToCandidate(
            reloadedStore, reloadedDoc, movedDoc,
            _ => throw new InvalidOperationException("Translation must not invoke template callback."),
            familyCopies: null);

        using (var rollback = new FileRollbackScope())
        {
            moveResult.Scenario.Save(map.Map, rollback);
            moveResult.Store.Save(map.Map, rollback);
            rollback.Commit();
        }

        var finalDoc = ScenarioDocument.Load(map.Map);
        var finalBinding = Assert.Single(finalDoc.DataSlots);
        Assert.Equal(newSlot, finalBinding.Slot);
        Assert.Equal(newUid, finalBinding.Uid);

        var finalStore = LevelObjectStore.Load(map.Map);
        var finalMovedObj = finalStore.Objects().Single(o => o.Slot == newSlot);
        Assert.Equal(targetSpawn.X + 10.0f, finalMovedObj.X);
        Assert.Equal(targetSpawn.Z + 20.0f, finalMovedObj.Z);
        Assert.Equal(newUid, finalMovedObj.Uid);
        Assert.Equal(BitConverter.SingleToInt32Bits(sourceRoot.Rotation), BitConverter.SingleToInt32Bits(finalMovedObj.Rotation));

        var finalSourceObj = finalStore.Objects().Single(o => o.Slot == 0);
        Assert.Equal(sourceRoot.X, finalSourceObj.X);
        Assert.Equal(sourceRoot.Y, finalSourceObj.Y);
        Assert.Equal(sourceRoot.Z, finalSourceObj.Z);
        Assert.Equal(100u, finalSourceObj.Uid);
        Assert.Equal(BitConverter.SingleToInt32Bits(sourceRoot.Rotation), BitConverter.SingleToInt32Bits(finalSourceObj.Rotation));
    }

    [Theory]
    [InlineData(1, 0f)] // Angle 1 with RawRotation 0f (display angle 0 != 1)
    [InlineData(359, 0f)] // Angle 359 with RawRotation 0f
    public void Adjacent_display_angle_mismatch_is_rejected_by_save_load_and_apply_candidate(int angle, float rawRotation)
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ArmValidateAngle_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            var spawn = new ScenarioSpawn("BauTest", 100f, 200f, 0, Angle: angle, Y: 10f, Prebuilt: true)
            {
                Id = Guid.NewGuid(),
                RawRotation = rawRotation
            };
            var doc = new ScenarioDocument { Spawns = [spawn] };

            // 1. Save rejects contradictory angle / raw rotation
            using (var rollback = new FileRollbackScope())
            {
                Assert.Throws<InvalidDataException>(() => doc.Save(tempDir, rollback));
            }

            // 2. Load rejects contradictory angle / raw rotation from JSON
            string json = $$"""
            {
              "Version": 6,
              "Spawns": [
                {
                  "Id": "{{spawn.Id}}",
                  "Alias": "BauTest",
                  "X": 100.0,
                  "Z": 200.0,
                  "Team": 0,
                  "Angle": {{angle}},
                  "RawRotation": 0.0,
                  "Prebuilt": true
                }
              ],
              "DataSlots": [],
              "Events": []
            }
            """;
            File.WriteAllText(Path.Combine(tempDir, ScenarioDocument.FileName), json);
            Assert.Throws<InvalidDataException>(() => ScenarioDocument.Load(tempDir));

            // 3. ApplyToCandidate rejects upfront
            using var map = new NativeFixture(false, count: 8);
            var store = LevelObjectStore.Load(map.Map);
            var prev = new ScenarioDocument();
            Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(store, prev, doc, _ => null));

            // 4. Apply rejects upfront
            Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, prev, doc, _ => null));
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Non_prebuilt_spawn_with_non_null_raw_rotation_is_rejected_by_save_load_and_apply_candidate()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ArmValidatePrebuilt_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            // RawRotation is non-null finite float, but Prebuilt is false
            var spawn = new ScenarioSpawn("BauTest", 100f, 200f, 0, Angle: 0, Y: 10f, Prebuilt: false)
            {
                Id = Guid.NewGuid(),
                RawRotation = 0f
            };
            var doc = new ScenarioDocument { Spawns = [spawn] };

            // 1. Save rejects non-prebuilt with RawRotation
            using (var rollback = new FileRollbackScope())
            {
                Assert.Throws<InvalidDataException>(() => doc.Save(tempDir, rollback));
            }

            // 2. Load rejects non-prebuilt with RawRotation
            string json = $$"""
            {
              "Version": 6,
              "Spawns": [
                {
                  "Id": "{{spawn.Id}}",
                  "Alias": "BauTest",
                  "X": 100.0,
                  "Z": 200.0,
                  "Team": 0,
                  "Angle": 0,
                  "RawRotation": 0.0,
                  "Prebuilt": false
                }
              ],
              "DataSlots": [],
              "Events": []
            }
            """;
            File.WriteAllText(Path.Combine(tempDir, ScenarioDocument.FileName), json);
            Assert.Throws<InvalidDataException>(() => ScenarioDocument.Load(tempDir));

            // 3. ApplyToCandidate rejects upfront
            using var map = new NativeFixture(false, count: 8);
            var store = LevelObjectStore.Load(map.Map);
            var prev = new ScenarioDocument();
            Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(store, prev, doc, _ => null));

            // 4. Apply rejects upfront
            Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, prev, doc, _ => null));
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Legacy_null_raw_rotation_scenario_is_unaffected_and_loads_saves_and_applies_cleanly()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ArmValidateLegacy_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            var prebuiltSpawn = new ScenarioSpawn("BauTest", 100f, 200f, 0, Angle: 90, Y: 10f, Prebuilt: true)
            {
                Id = Guid.NewGuid()
            };
            var scriptSpawn = new ScenarioSpawn("BauTest", 150f, 250f, 0, Angle: 45, Y: 10f, Prebuilt: false)
            {
                Id = Guid.NewGuid()
            };

            var doc = new ScenarioDocument { Spawns = [prebuiltSpawn, scriptSpawn] };

            // 1. Save succeeds
            using (var rollback = new FileRollbackScope())
            {
                doc.Save(tempDir, rollback);
                rollback.Commit();
            }

            // 2. Load succeeds with null RawRotation
            var loaded = ScenarioDocument.Load(tempDir);
            Assert.Equal(2, loaded.Spawns.Count);

            var loadedPrebuilt = loaded.Spawns.Single(s => s.Id == prebuiltSpawn.Id);
            Assert.Null(loadedPrebuilt.RawRotation);
            Assert.Null(loadedLegacySpawn(loadedPrebuilt));
            Assert.Equal(90, loadedPrebuilt.Angle);
            Assert.True(loadedPrebuilt.Prebuilt);

            var loadedScript = loaded.Spawns.Single(s => s.Id == scriptSpawn.Id);
            Assert.Null(loadedScript.RawRotation);
            Assert.Null(loadedLegacySpawn(loadedScript));
            Assert.Equal(45, loadedScript.Angle);
            Assert.False(loadedScript.Prebuilt);

            // 3. ApplyToCandidate validates upfront without exception
            using var map = new NativeFixture(false, count: 8);
            var store = LevelObjectStore.Load(map.Map);
            var prev = new ScenarioDocument();
            var result = ScenarioLevelObjects.ApplyToCandidate(store, prev, loaded, _ => null);
            Assert.NotNull(result);
            Assert.Single(result.Skipped); // Prebuilt without template is skipped cleanly
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }

        static float? loadedLegacySpawn(ScenarioSpawn spawn) => spawn.RawNativeRotation;
    }
}
