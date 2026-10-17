using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using System.Buffers.Binary;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeRootFamilyCopyIntentTests
{
    private static SdlObjectType SampleBuildingType(string name = "BauGerHouse", string alias = "HOUSE") =>
        new(name, 2, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = alias });

    [Fact]
    public void Staged_native_copy_has_transient_identity_and_empty_copy_source_guid()
    {
        var session = new PlacementEditSession();
        Guid id = Guid.NewGuid();
        float rawRotation = 1.25f;

        var nativeCopy = new SdlPlacedObject(SampleBuildingType(), 500f, 10f, 600f, Team: 0,
            Angle: (float)((double)rawRotation * (180d / Math.PI)), UnitCount: 0)
        {
            ScenarioId = id,
            CopySourceScenarioId = Guid.Empty,
            NativeSourceIdentity = new NativeObjectIdentity(5, 500u),
            RawRotation = rawRotation
        };

        session.Add(nativeCopy);

        Assert.Equal(1, session.Count);
        Assert.True(session.IsDirty);
        var item = session[0];
        Assert.Equal(id, item.ScenarioId);
        Assert.Equal(Guid.Empty, item.CopySourceScenarioId);
        Assert.Equal(new NativeObjectIdentity(5, 500u), item.NativeSourceIdentity);
        Assert.Equal(rawRotation, item.RawRotation);
    }

    [Fact]
    public void Duplicate_of_pending_native_copy_retains_native_identity_without_fabricating_guid_source()
    {
        var session = new PlacementEditSession();
        Guid origId = Guid.NewGuid();
        float rawRotation = 0.7853982f; // pi / 4

        var pendingNative = new SdlPlacedObject(SampleBuildingType(), 200f, 0f, 300f, Team: 0,
            Angle: 45f, UnitCount: 0)
        {
            ScenarioId = origId,
            CopySourceScenarioId = Guid.Empty,
            NativeSourceIdentity = new NativeObjectIdentity(2, 202u),
            RawRotation = rawRotation
        };
        session.Load([pendingNative]);

        var dupIndices = session.DuplicateMany([0], offsetX: 256f, offsetZ: 256f);
        Assert.Single(dupIndices);
        Assert.Equal(2, session.Count);

        var dup = session[1];
        Assert.NotEqual(origId, dup.ScenarioId);
        // Pending duplicate-of-duplicate MUST NOT fabricate previous source Guid
        Assert.Equal(Guid.Empty, dup.CopySourceScenarioId);
        Assert.Equal(new NativeObjectIdentity(2, 202u), dup.NativeSourceIdentity);
        Assert.Equal(rawRotation, dup.RawRotation);
        Assert.Equal(456f, dup.WorldX);
        Assert.Equal(556f, dup.WorldZ);
    }

    [Fact]
    public void AcceptSavedChanges_clears_native_source_identity_while_preserving_raw_rotation()
    {
        var session = new PlacementEditSession();
        Guid id = Guid.NewGuid();
        float rawRotation = 2.1f;

        var nativeCopy = new SdlPlacedObject(SampleBuildingType(), 100f, 0f, 100f, Team: 1,
            Angle: (float)((double)rawRotation * (180d / Math.PI)), UnitCount: 0)
        {
            ScenarioId = id,
            CopySourceScenarioId = Guid.Empty,
            NativeSourceIdentity = new NativeObjectIdentity(8, 808u),
            RawRotation = rawRotation
        };
        session.Load([nativeCopy]);

        Assert.Equal(new NativeObjectIdentity(8, 808u), session[0].NativeSourceIdentity);
        Assert.Equal(rawRotation, session[0].RawRotation);

        // Ordinary AcceptChanges maintains provenance
        session.AcceptChanges();
        Assert.Equal(new NativeObjectIdentity(8, 808u), session[0].NativeSourceIdentity);
        Assert.Equal(rawRotation, session[0].RawRotation);

        // AcceptSavedChanges clears source provenance but retains raw rotation
        session.AcceptSavedChanges();
        Assert.Null(session[0].NativeSourceIdentity);
        Assert.Equal(Guid.Empty, session[0].CopySourceScenarioId);
        Assert.Equal(rawRotation, session[0].RawRotation);

        // Now duplicating this saved native-copied root follows ordinary GUID-source with same RawRotation
        var dupIndices = session.DuplicateMany([0], offsetX: 100f, offsetZ: 100f);
        Assert.Single(dupIndices);
        var savedDup = session[1];
        Assert.Equal(id, savedDup.CopySourceScenarioId);
        Assert.Null(savedDup.NativeSourceIdentity);
        Assert.Equal(rawRotation, savedDup.RawRotation);
    }

    [Fact]
    public void Undo_and_redo_preserves_native_provenance_and_tracks_dirty()
    {
        var session = new PlacementEditSession();
        Guid origId = Guid.NewGuid();
        float raw = 1.5707963f;

        var original = new SdlPlacedObject(SampleBuildingType(), 100f, 0f, 200f, Team: 0, Angle: 90f, UnitCount: 0)
        {
            ScenarioId = origId,
            NativeSourceIdentity = new NativeObjectIdentity(1, 101u),
            RawRotation = raw
        };
        session.Load([original]);

        session.DuplicateMany([0], offsetX: 256f, offsetZ: 256f);
        Assert.True(session.IsDirty);
        Assert.Equal(new NativeObjectIdentity(1, 101u), session[1].NativeSourceIdentity);

        Assert.True(session.Undo());
        Assert.Equal(1, session.Count);
        Assert.False(session.IsDirty);

        Assert.True(session.Redo());
        Assert.Equal(2, session.Count);
        Assert.True(session.IsDirty);
        Assert.Equal(new NativeObjectIdentity(1, 101u), session[1].NativeSourceIdentity);
        Assert.Equal(raw, session[1].RawRotation);
    }

    [Fact]
    public void Edit_and_EditMany_reject_modifying_team_or_angle_on_native_group_objects()
    {
        var session = new PlacementEditSession();
        var item = new SdlPlacedObject(SampleBuildingType(), 100f, 0f, 100f, Team: 0, Angle: 90f, UnitCount: 0)
        {
            ScenarioId = Guid.NewGuid(),
            NativeSourceIdentity = new NativeObjectIdentity(3, 303u),
            RawRotation = 1.5707963f
        };
        session.Load([item]);

        // Changing team is rejected
        Assert.Throws<InvalidOperationException>(() =>
            session.Edit(0, item with { Team = 1 }));

        // Changing angle is rejected
        Assert.Throws<InvalidOperationException>(() =>
            session.Edit(0, item with { Angle = 180f }));

        // EditMany changing team is rejected
        Assert.Throws<InvalidOperationException>(() =>
            session.EditMany([0], team: 2));

        // EditMany changing angle is rejected
        Assert.Throws<InvalidOperationException>(() =>
            session.EditMany([0], angle: 0f));

        // Moving position only is permitted and preserves all metadata
        session.Edit(0, item with { WorldX = 500f, WorldZ = 600f });
        Assert.Equal(500f, session[0].WorldX);
        Assert.Equal(600f, session[0].WorldZ);
        Assert.Equal(new NativeObjectIdentity(3, 303u), session[0].NativeSourceIdentity);
        Assert.Equal(1.5707963f, session[0].RawRotation);
    }

    [Fact]
    public void BuildFamilyCopyRequests_maps_valid_native_request_with_trusted_alias_resolver()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        Guid targetId = Guid.NewGuid();
        float rawRotation = root.Rotation;
        int expectedDisplay = ScenarioLevelObjects.RadiansToDisplayAngle(rawRotation);

        var placedType = SampleBuildingType("BauGerHouse", "HOUSE");
        var placedObject = new SdlPlacedObject(placedType, root.X + 256f, root.Y, root.Z + 256f, root.Team,
            Angle: expectedDisplay, UnitCount: 0)
        {
            ScenarioId = targetId,
            NativeSourceIdentity = new NativeObjectIdentity(0, root.Uid),
            RawRotation = rawRotation
        };

        var emptyScenario = new ScenarioDocument();
        string? TrustedResolver(int typeId) => typeId == root.TypeId ? "HOUSE" : null;

        var requests = PlacementEditSession.BuildFamilyCopyRequests(
            [placedObject], emptyScenario, store, TrustedResolver);

        Assert.Single(requests);
        Assert.True(requests.ContainsKey(targetId));
        var req = requests[targetId];
        Assert.Equal(0, req.SourceSlot);
        Assert.Equal(root.Uid, req.SourceUid);
        Assert.True(req.IsNativeSource);
    }

    [Fact]
    public void BuildFamilyCopyRequests_rejects_native_request_without_trusted_resolver()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        var placedObject = new SdlPlacedObject(SampleBuildingType(), root.X + 256f, root.Y, root.Z + 256f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), UnitCount: 0)
        {
            ScenarioId = Guid.NewGuid(),
            NativeSourceIdentity = new NativeObjectIdentity(0, root.Uid),
            RawRotation = root.Rotation
        };

        // Without nativeAliasResolver, it MUST reject
        Assert.Throws<InvalidDataException>(() =>
            PlacementEditSession.BuildFamilyCopyRequests([placedObject], new ScenarioDocument(), store, null));
    }

    [Fact]
    public void BuildFamilyCopyRequests_rejects_native_request_if_source_already_bound_in_previous_scenario()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        var scenario = new ScenarioDocument
        {
            DataSlots = [new ScenarioDataSlot(0, root.Uid) { SpawnId = Guid.NewGuid() }]
        };

        var placedObject = new SdlPlacedObject(SampleBuildingType("BauGerHouse", "HOUSE"), root.X + 256f, root.Y, root.Z + 256f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), UnitCount: 0)
        {
            ScenarioId = Guid.NewGuid(),
            NativeSourceIdentity = new NativeObjectIdentity(0, root.Uid),
            RawRotation = root.Rotation
        };

        string? Resolver(int typeId) => "HOUSE";

        // Must reject native source if it is already bound in scenario
        Assert.Throws<InvalidDataException>(() =>
            PlacementEditSession.BuildFamilyCopyRequests([placedObject], scenario, store, Resolver));
    }

    [Fact]
    public void BuildFamilyCopyRequests_rejects_mismatched_alias_or_team_or_rotation()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        var validScenario = new ScenarioDocument();
        string? Resolver(int typeId) => "HOUSE";

        // 1. Mismatched alias
        var badAlias = new SdlPlacedObject(SampleBuildingType("BauGerHouse", "OTHER_ALIAS"), root.X + 256f, root.Y, root.Z + 256f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), UnitCount: 0)
        {
            ScenarioId = Guid.NewGuid(),
            NativeSourceIdentity = new NativeObjectIdentity(0, root.Uid),
            RawRotation = root.Rotation
        };
        Assert.Throws<InvalidDataException>(() =>
            PlacementEditSession.BuildFamilyCopyRequests([badAlias], validScenario, store, Resolver));

        // 2. Mismatched team
        var badTeam = new SdlPlacedObject(SampleBuildingType("BauGerHouse", "HOUSE"), root.X + 256f, root.Y, root.Z + 256f, Team: root.Team + 1,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation), UnitCount: 0)
        {
            ScenarioId = Guid.NewGuid(),
            NativeSourceIdentity = new NativeObjectIdentity(0, root.Uid),
            RawRotation = root.Rotation
        };
        Assert.Throws<InvalidDataException>(() =>
            PlacementEditSession.BuildFamilyCopyRequests([badTeam], validScenario, store, Resolver));

        // 3. Mismatched rotation / angle
        var badAngle = new SdlPlacedObject(SampleBuildingType("BauGerHouse", "HOUSE"), root.X + 256f, root.Y, root.Z + 256f, root.Team,
            Angle: ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation) + 90, UnitCount: 0)
        {
            ScenarioId = Guid.NewGuid(),
            NativeSourceIdentity = new NativeObjectIdentity(0, root.Uid),
            RawRotation = root.Rotation
        };
        Assert.Throws<InvalidDataException>(() =>
            PlacementEditSession.BuildFamilyCopyRequests([badAngle], validScenario, store, Resolver));
    }

    private sealed class TestMapFixture : IDisposable
    {
        public string Map { get; }
        public string Data => Path.Combine(Map, "DATA");

        private static int ColumnOffset(int col, int slot, int count) =>
            16 + count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Take(col).Sum()) + slot * LevelObjectStore.ColumnWidths[col];

        public TestMapFixture(int count = 10)
        {
            Map = Path.Combine(Path.GetTempPath(), "ARM_NativeIntentTest_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(Data);

            byte[] objects = new byte[16 + count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(0), 1);
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(4), (uint)count);
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(8), 30);
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(12), 30);

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

            // Root (slot 0, UID 100, TypeId 555)
            int rec0 = 16 + 0 * LevelObjectStore.RecordSize;
            objects[rec0] = 1;
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec0 + 3), 100u);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 67), 0);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 69), 0);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec0 + 75), 555);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 0, count)), 0);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(12, 0, count)), 0); // Direct light 0

            // Child (slot 1, UID 101, TypeId 556)
            int rec1 = 16 + 1 * LevelObjectStore.RecordSize;
            objects[rec1] = 1;
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(rec1 + 3), 101u);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 67), 1);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 69), 1);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(rec1 + 75), 556);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(1, 1, count)), 1);

            // Connect child (slot 1) parent to root (slot 0)
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(ColumnOffset(13, 1, count)), 0);

            byte[] objdata = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
            BinaryPrimitives.WriteUInt32LittleEndian(objdata.AsSpan(0), 1);
            BinaryPrimitives.WriteUInt32LittleEndian(objdata.AsSpan(4), (uint)count);
            objdata[8] = 1; // slot 0 active
            int seg0Stride = LevelObjectStore.ObjDataWidths[0];
            objdata[8 + seg0Stride] = 1; // slot 1 active

            int positionCount = Math.Max(12, count * 2);
            byte[] positions = new byte[8 + positionCount * LevelObjectStore.PositionSize];
            BinaryPrimitives.WriteUInt32LittleEndian(positions.AsSpan(0), 1);
            BinaryPrimitives.WriteUInt32LittleEndian(positions.AsSpan(4), (uint)positionCount);

            // Position 0 for root: XYZ = (100, 20, 200), rotation in degrees = 0
            positions[8] = 1;
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(9), 100f);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(13), 20f);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(17), 200f);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(21), 0f);

            // Position 1 for child: XYZ = (110, 20, 210), rotation in degrees = 0
            positions[8 + 1 * LevelObjectStore.PositionSize] = 1;
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(8 + 1 * LevelObjectStore.PositionSize + 1), 110f);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(8 + 1 * LevelObjectStore.PositionSize + 5), 20f);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(8 + 1 * LevelObjectStore.PositionSize + 9), 210f);
            BinaryPrimitives.WriteSingleLittleEndian(positions.AsSpan(8 + 1 * LevelObjectStore.PositionSize + 13), 0f);

            byte[] light = new byte[NativeLightPool.PayloadLength];
            BinaryPrimitives.WriteUInt32LittleEndian(light.AsSpan(0), 1);
            BinaryPrimitives.WriteUInt32LittleEndian(light.AsSpan(4), 1024);
            light.AsSpan(8 + 1024 * 50).Fill(255);
            var pool = NativeLightPool.ParsePayload(light);
            pool.AddBatch([new(new(100, 40, 200), new(1, 1, 1), 350, 0, 0, 0, 2, 0, -1)]);
            light = pool.ToPayload();

            byte[] engine = new byte[94];
            BinaryPrimitives.WriteUInt32LittleEndian(engine.AsSpan(0), 1);
            BinaryPrimitives.WriteUInt32LittleEndian(engine.AsSpan(34), 200u);

            File.WriteAllBytes(Path.Combine(Data, "objects.dat"), objects);
            File.WriteAllBytes(Path.Combine(Data, "objdata.dat"), objdata);
            File.WriteAllBytes(Path.Combine(Data, "position.dat"), positions);
            File.WriteAllBytes(Path.Combine(Data, "light.dat"), light);
            File.WriteAllBytes(Path.Combine(Data, "engine.dat"), engine);
        }

        public void Dispose()
        {
            try { if (Directory.Exists(Map)) Directory.Delete(Map, recursive: true); } catch { }
        }
    }
}
