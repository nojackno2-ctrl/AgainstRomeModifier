using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using System.Buffers.Binary;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PlacementFamilyCopyIntentTests
{
    private static SdlObjectType SampleBuildingType(string name = "BauTest", string alias = "BauTest") =>
        new(name, 2, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = alias });

    private static SdlObjectType SampleFigureType(string name = "FigTest", string alias = "FigTest") =>
        new(name, 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = alias });

    private static SdlPlacedObject CreateBuilding(Guid? id = null, Guid? copySource = null, int team = 0, float x = 500, float z = 600, float angle = 0, string alias = "BauTest") =>
        new(SampleBuildingType(name: alias, alias: alias), x, 0, z, team, angle, 0)
        {
            ScenarioId = id ?? Guid.NewGuid(),
            CopySourceScenarioId = copySource ?? Guid.Empty
        };

    private static SdlPlacedObject CreateFigure(Guid? id = null, Guid? copySource = null, int team = 0, float x = 100, float z = 200, float angle = 0, int count = 10, string alias = "FigTest") =>
        new(SampleFigureType(name: alias, alias: alias), x, 0, z, team, angle, count)
        {
            ScenarioId = id ?? Guid.NewGuid(),
            CopySourceScenarioId = copySource ?? Guid.Empty
        };

    [Fact]
    public void DuplicateMany_sets_copy_source_guid_and_duplicate_of_duplicate_points_to_original_source()
    {
        var session = new PlacementEditSession();
        Guid origId = Guid.NewGuid();
        var original = CreateBuilding(id: origId, x: 100, z: 200);
        session.Load([original]);

        // First duplicate
        var dupIndices = session.DuplicateMany([0], offsetX: 20, offsetZ: 30);
        Assert.Single(dupIndices);
        Assert.Equal(2, session.Count);

        var dup1 = session[1];
        Assert.NotEqual(origId, dup1.ScenarioId);
        Assert.Equal(origId, dup1.CopySourceScenarioId);
        Assert.Equal(120f, dup1.WorldX);
        Assert.Equal(230f, dup1.WorldZ);

        // Duplicate-of-duplicate: duplicate dup1
        var dup2Indices = session.DuplicateMany([1], offsetX: 10, offsetZ: 10);
        Assert.Single(dup2Indices);
        Assert.Equal(3, session.Count);

        var dup2 = session[2];
        Assert.NotEqual(origId, dup2.ScenarioId);
        Assert.NotEqual(dup1.ScenarioId, dup2.ScenarioId);
        // Crucial requirement: duplicate-of-pendingduplicate must still point to earliest root source
        Assert.Equal(origId, dup2.CopySourceScenarioId);
        Assert.Equal(130f, dup2.WorldX);
        Assert.Equal(240f, dup2.WorldZ);
    }

    [Fact]
    public void DuplicateMany_with_multiple_items_preserves_respective_provenance()
    {
        var session = new PlacementEditSession();
        Guid idA = Guid.NewGuid();
        Guid idB = Guid.NewGuid();
        var itemA = CreateBuilding(id: idA, x: 100, z: 100);
        var itemB = CreateBuilding(id: idB, x: 300, z: 300);
        session.Load([itemA, itemB]);

        session.DuplicateMany([0, 1], offsetX: 50, offsetZ: 50);
        Assert.Equal(4, session.Count);

        // Find duplicates
        var copies = session.Capture().Where(obj => obj.CopySourceScenarioId != Guid.Empty).ToList();
        Assert.Equal(2, copies.Count);

        Assert.Contains(copies, c => c.CopySourceScenarioId == idA && c.WorldX == 150f);
        Assert.Contains(copies, c => c.CopySourceScenarioId == idB && c.WorldX == 350f);
    }

    [Fact]
    public void Undo_redo_preserves_provenance_and_dirty_evaluates_source_guid()
    {
        var session = new PlacementEditSession();
        Guid origId = Guid.NewGuid();
        var original = CreateBuilding(id: origId, x: 100, z: 200);
        session.Load([original]);

        session.DuplicateMany([0], offsetX: 20, offsetZ: 30);
        Assert.True(session.IsDirty);
        Assert.Equal(origId, session[1].CopySourceScenarioId);

        // Undo
        Assert.True(session.Undo());
        Assert.Equal(1, session.Count);
        Assert.False(session.IsDirty);

        // Redo
        Assert.True(session.Redo());
        Assert.Equal(2, session.Count);
        Assert.True(session.IsDirty);
        Assert.Equal(origId, session[1].CopySourceScenarioId);

        // Reset restores baseline
        session.Reset();
        Assert.Equal(1, session.Count);
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void AcceptSavedChanges_clears_provenance_while_ordinary_AcceptChanges_preserves_it()
    {
        var session = new PlacementEditSession();
        Guid origId = Guid.NewGuid();
        var original = CreateBuilding(id: origId, x: 100, z: 200);
        session.Load([original]);

        session.DuplicateMany([0], offsetX: 50, offsetZ: 50);
        Assert.Equal(origId, session[1].CopySourceScenarioId);
        Assert.True(session.IsDirty);

        // 1. Ordinary AcceptChanges (e.g. used during Load or intermediate baseline)
        // Must preserve existing semantics and NOT wipe provenance
        session.AcceptChanges();
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.Equal(origId, session[1].CopySourceScenarioId);

        // 2. AcceptSavedChanges (called only after successful Save)
        // Explicitly clears CopySourceScenarioId from all items
        session.AcceptSavedChanges();
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.Equal(Guid.Empty, session[0].CopySourceScenarioId);
        Assert.Equal(Guid.Empty, session[1].CopySourceScenarioId);
    }

    [Fact]
    public void BuildFamilyCopyRequests_maps_only_prebuilt_with_valid_binding_and_rejects_unsupported()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        Guid sourceId = Guid.NewGuid();
        Guid copyTargetId = Guid.NewGuid();
        Guid unboundCopyId = Guid.NewGuid();
        Guid figureCopyId = Guid.NewGuid();
        Guid figureSourceId = Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true) { Id = sourceId };
        var previous = new ScenarioDocument
        {
            Spawns = [sourceSpawn, new ScenarioSpawn("FigTest", 200, 200, 0) { Id = figureSourceId }],
            DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = sourceId }]
        };

        var placed = new List<SdlPlacedObject>
        {
            // 1. Valid duplicate of source
            CreateBuilding(id: copyTargetId, copySource: sourceId, team: root.Team, x: root.X + 10, z: root.Z + 10, angle: 0),
            // 2. Duplicate of an object with no previous binding (e.g. newly spawned in session)
            CreateBuilding(id: unboundCopyId, copySource: Guid.NewGuid(), team: 0, x: 200, z: 200, angle: 0),
            // 3. Figure duplicate (script generated, not prebuilt building)
            CreateFigure(id: figureCopyId, copySource: figureSourceId, team: 0, x: 300, z: 300, angle: 0)
        };

        var requests = PlacementEditSession.BuildFamilyCopyRequests(placed, previous, store);

        // Only valid prebuilt with bound previous source is in requests
        Assert.Single(requests);
        Assert.True(requests.ContainsKey(copyTargetId));
        Assert.Equal(0, requests[copyTargetId].SourceSlot);
        Assert.Equal(100u, requests[copyTargetId].SourceUid);

        // Unbound and figure are excluded from saved-family copy (kept for template/script fallback)
        Assert.False(requests.ContainsKey(unboundCopyId));
        Assert.False(requests.ContainsKey(figureCopyId));
    }

    [Fact]
    public void BuildFamilyCopyRequests_rejects_team_or_angle_mutation()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        Guid sourceId = Guid.NewGuid();
        Guid copyTargetId = Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true) { Id = sourceId };
        var previous = new ScenarioDocument
        {
            Spawns = [sourceSpawn],
            DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = sourceId }]
        };

        // Different team -> must throw InvalidDataException, not silently fallback
        var teamMutated = new List<SdlPlacedObject>
        {
            CreateBuilding(id: copyTargetId, copySource: sourceId, team: root.Team + 1, x: root.X + 10, z: root.Z + 10, angle: 0)
        };
        Assert.NotNull(store.FamilyAt(0, 100));
        Assert.Contains("隊伍", Assert.Throws<InvalidDataException>(() => PlacementEditSession.BuildFamilyCopyRequests(teamMutated, previous, store)).Message);

        // Different angle -> must throw InvalidDataException
        var angleMutated = new List<SdlPlacedObject>
        {
            CreateBuilding(id: copyTargetId, copySource: sourceId, team: root.Team, x: root.X + 10, z: root.Z + 10, angle: 90)
        };
        Assert.Contains("方向", Assert.Throws<InvalidDataException>(() => PlacementEditSession.BuildFamilyCopyRequests(angleMutated, previous, store)).Message);
    }

    [Theory]
    [InlineData("duplicate-binding")]
    [InlineData("script-downgrade")]
    [InlineData("stale-uid")]
    [InlineData("alias-mismatch")]
    public void Bound_family_copy_errors_are_rejected_instead_of_falling_back(string fault)
    {
        using var fixture = new TestMapFixture();
        var store = LevelObjectStore.Load(fixture.Map);
        Assert.NotNull(store.FamilyAt(0, 100));
        var root = store.Objects().Single(item => item.Slot == 0);
        Guid sourceId = Guid.NewGuid(), targetId = Guid.NewGuid();
        var previous = new ScenarioDocument
        {
            Spawns = [new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Prebuilt: true) { Id = sourceId }],
            DataSlots = [new(0, 100) { SpawnId = sourceId }]
        };
        var target = CreateBuilding(targetId, sourceId, root.Team, root.X + 10, root.Z + 10, 0);
        Assert.Single(PlacementEditSession.BuildFamilyCopyRequests([target], previous, store));
        string expected;
        if (fault == "duplicate-binding")
        { previous.DataSlots.Add(previous.DataSlots[0]); expected = "重複"; }
        else if (fault == "script-downgrade")
        { target = target with { Team = 9 }; expected = "降級"; }
        else if (fault == "alias-mismatch")
        { target = CreateBuilding(targetId, sourceId, root.Team, root.X + 10, root.Z + 10, 0, alias: "DifferentAlias"); expected = "別名"; }
        else
        { previous.DataSlots[0] = previous.DataSlots[0] with { Uid = 999 }; expected = "UID"; }
        Assert.Contains(expected, Assert.Throws<InvalidDataException>(() =>
            PlacementEditSession.BuildFamilyCopyRequests([target], previous, store)).Message);
        Assert.Equal(100u, store.UidAt(0));
    }

    [Fact]
    public void BuildFamilyCopyRequests_source_deleted_in_ui_still_resolves_via_previous_binding()
    {
        using var fixture = new TestMapFixture(count: 10);
        var store = LevelObjectStore.Load(fixture.Map);
        var root = store.Objects().Single(o => o.Slot == 0);

        Guid sourceId = Guid.NewGuid();
        Guid copyTargetId = Guid.NewGuid();

        var sourceSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true) { Id = sourceId };
        var previous = new ScenarioDocument
        {
            Spawns = [sourceSpawn],
            DataSlots = [new ScenarioDataSlot(0, 100) { SpawnId = sourceId }]
        };

        // Source object was deleted from UI placements list, only copyTarget remains
        var placed = new List<SdlPlacedObject>
        {
            CreateBuilding(id: copyTargetId, copySource: sourceId, team: root.Team, x: root.X + 15, z: root.Z + 25, angle: 0)
        };

        var requests = PlacementEditSession.BuildFamilyCopyRequests(placed, previous, store);
        Assert.Single(requests);
        Assert.Equal(0, requests[copyTargetId].SourceSlot);
        Assert.Equal(100u, requests[copyTargetId].SourceUid);
    }

    private static void U16(byte[] bytes, int offset, ushort value) => BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(offset), value);
    private static void U32(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
    private static void F32(byte[] bytes, int offset, float value) => BinaryPrimitives.WriteSingleLittleEndian(bytes.AsSpan(offset), value);

    private sealed class TestMapFixture : IDisposable
    {
        public string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmFamilyIntent_" + Guid.NewGuid().ToString("N"));
        public string Map => Path.Combine(Root, "MAPS", "TEST");
        public string Data => Path.Combine(Map, "DATA");

        public static int Column(int column, int slot, int count) =>
            16 + count * (79 + LevelObjectStore.ColumnWidths.Take(column).Sum()) + slot * LevelObjectStore.ColumnWidths[column];

        public TestMapFixture(int count = 10)
        {
            Directory.CreateDirectory(Data);
            byte[] objects = new byte[16 + count * (79 + LevelObjectStore.ColumnWidths.Sum())];
            U32(objects, 0, 1); U32(objects, 4, (uint)count); U32(objects, 8, 30); U32(objects, 12, 30);
            for (int slot = 0; slot < count; slot++)
            {
                for (int col = 0; col < 18; col++)
                    if (LevelObjectStore.ColumnWidths[col] == 2) U16(objects, Column(col, slot, count), ushort.MaxValue);
                foreach (int field in new[] { 67, 69, 71, 73, 75, 77 }) U16(objects, 16 + slot * 79 + field, ushort.MaxValue);
            }

            // Root slot 0
            objects[16] = 1; U32(objects, 19, 100); U16(objects, 16 + 75, 555);
            U16(objects, 16 + 67, 0); U16(objects, 16 + 69, 1);
            U16(objects, Column(1, 0, count), 0); U16(objects, Column(12, 0, count), 0);

            // Child slot 1
            objects[16 + 79] = 1; U32(objects, 19 + 79, 101); U16(objects, 16 + 79 + 75, 555);
            U16(objects, 16 + 79 + 67, 1); U16(objects, 16 + 79 + 69, 1);
            U16(objects, Column(1, 1, count), 1);
            U16(objects, Column(13, 1, count), 0); // Parent is 0

            byte[] data = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
            U32(data, 0, 1); U32(data, 4, (uint)count);
            data[8] = 1; data[9] = 1; // Segment 0 is a parallel array, not per-object interleaved records.

            int positionCount = count * 2;
            byte[] positions = new byte[8 + positionCount * 17];
            U32(positions, 0, 1); U32(positions, 4, (uint)positionCount);
            for (int slot = 0; slot < 2; slot++)
            {
                positions[8 + slot * 17] = 1;
                F32(positions, 9 + slot * 17, 100 + slot * 10);
                F32(positions, 13 + slot * 17, 20);
                F32(positions, 17 + slot * 17, 200 + slot * 10);
            }

            byte[] light = new byte[NativeLightPool.PayloadLength];
            U32(light, 0, 1); U32(light, 4, 1024);
            light.AsSpan(8 + 1024 * 50).Fill(255);
            var pool = NativeLightPool.ParsePayload(light);
            pool.AddBatch([new(new(100, 20, 200), new(1, 1, 1), 350, 0, 0, 0, 2, 0, -1)]);
            light = pool.ToPayload();

            File.WriteAllBytes(Path.Combine(Data, "objects.dat"), objects);
            File.WriteAllBytes(Path.Combine(Data, "objdata.dat"), data);
            File.WriteAllBytes(Path.Combine(Data, "position.dat"), positions);
            File.WriteAllBytes(Path.Combine(Data, "light.dat"), light);
        }

        public void Dispose() => Directory.Delete(Root, true);
    }
}
