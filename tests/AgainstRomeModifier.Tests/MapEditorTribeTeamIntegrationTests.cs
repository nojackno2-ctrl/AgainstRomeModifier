using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData("Ger")]
    [InlineData("Hun")]
    [InlineData("Kel")]
    [InlineData("Rom")]
    public void Tribe_team_matrix_preserves_buildings_units_and_events_across_retry_and_fresh_form(string tribe)
    {
        string map = CreateFixture();
        WriteEmptyWorldStore(map, 32);
        string building = tribe.ToUpperInvariant() + "_HOUSE", unit = tribe.ToUpperInvariant() + "_INF01";
        string aliases = Path.Combine(_root, "SYSTEM", "CLAK", "cl_scint.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(aliases)!);
        byte[] header = new byte[64];
        "PFIL"u8.CopyTo(header);
        File.WriteAllBytes(aliases, GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes(
            $"[ObjDefName]\r\n{building}=Bau{tribe}Hau00\r\n{unit}=Fig{tribe}Inf00\r\n"), header));
        var before = SnapshotDirectory(map);
        Guid[] buildingIds = Enumerable.Range(0, 9).Select(_ => Guid.NewGuid()).ToArray();
        Guid[] unitIds = Enumerable.Range(0, 8).Select(_ => Guid.NewGuid()).ToArray();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Matrix", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var catalog = GetField<IReadOnlyList<SdlObjectType>>(form, "_objectCatalog");
            Assert.All(catalog, type => Assert.Equal(tribe, type.Tribe));
            var house = Assert.Single(catalog, type => type.Category == SdlObjectCategory.Building);
            var soldier = Assert.Single(catalog, type => type.Category == SdlObjectCategory.Figure);
            SetMatrixField(form, "_objdefNames", new Dictionary<int, string> { [1] = house.NameDef });
            byte[] record = new byte[LevelObjectStore.RecordSize];
            record[0] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(75), 1);
            uint[] columns = new uint[18]; columns[10] = 0xFFFF;
            var template = new LevelObjectTemplate(1, record, columns,
                LevelObjectStore.ObjDataWidths.Select(width => new byte[width]).ToArray(), new byte[17], new byte[17]);
            SetMatrixField(form, "_buildingTemplates", new Dictionary<int, LevelObjectTemplate> { [1] = template });
            for (int team = 0; team <= 8; team++)
                form.PlacementSession.Add(new(house, 1000 + team * 200, 320, 2000, team, 90) { ScenarioId = buildingIds[team] });
            var events = GetField<List<ScenarioEvent>>(form, "_events");
            for (int team = 0; team < 8; team++)
            {
                form.PlacementSession.Add(new(soldier, 4000 + team * 200, 0, 5000, team, 45, team == 0 ? 1 : 20) { ScenarioId = unitIds[team] });
                events.Add(new($"Team {team}", 1) {
                    Conditions = [new(ScenarioConditionKind.ObjectExists, buildingIds[team]),
                        new(ScenarioConditionKind.ObjectInArea, unitIds[team], 3900, 4900, 6000, 5100)],
                    Actions = [new(ScenarioActionKind.Diplomacy, Team: team, OtherTeam: (team + 1) % 8),
                        new(ScenarioActionKind.SpawnUnit, Team: team, Alias: unit, Count: 1, X: 7000, Z: 8000)] });
            }
            // DATA and scenario are written before the missing script fails; the complete transaction must roll back.
            Assert.False(form.TrySaveMap(false, out Exception? error));
            Assert.IsType<FileNotFoundException>(error);
            AssertSnapshotUnchanged(map, before);
            Assert.True(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanUndo);
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            var saved = ScenarioDocument.Load(map);
            Assert.Equal(17, saved.Spawns.Count);
            Assert.Equal(9, saved.DataSlots.Count);
            Assert.Equal(8, saved.Events.Count);
            var world = LevelObjectStore.Load(map);
            Assert.Equal(9, world.Objects().Count);
            foreach (var spawn in saved.Spawns.Where(spawn => spawn.Prebuilt))
            {
                var binding = Assert.Single(saved.DataSlots, slot => slot.SpawnId == spawn.Id);
                var obj = Assert.Single(world.Objects(), obj => obj.Slot == binding.Slot);
                Assert.Equal(binding.Uid, obj.Uid);
                Assert.Equal(spawn.Team, obj.Team);
                Assert.Equal(spawn.X, obj.X); Assert.Equal(spawn.Y, obj.Y); Assert.Equal(spawn.Z, obj.Z);
                Assert.Equal(MathF.PI / 2, obj.Rotation, 5);
            }
            var savedBytes = SnapshotDirectory(map);
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            AssertSnapshotUnchanged(map, savedBytes);
        });
        RunInSta(() =>
        {
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Fresh", "Test"));
            _ = reopened.Handle;
            Invoke(reopened, "LoadSelectedMap");
            var placed = reopened.PlacementSession.Capture();
            Assert.Equal(17, placed.Count);
            Assert.Equal(buildingIds.Concat(unitIds).Order(), placed.Select(item => item.ScenarioId).Order());
            foreach (int team in Enumerable.Range(0, 8))
            {
                var soldier = Assert.Single(placed, item => item.ScenarioId == unitIds[team]);
                Assert.Equal(tribe, soldier.Type.Tribe); Assert.Equal(team, soldier.Team);
                Assert.Equal(team == 0 ? 1 : 20, soldier.UnitCount);
            }
            var events = GetField<List<ScenarioEvent>>(reopened, "_events");
            Assert.Equal(8, events.Count);
            for (int team = 0; team < 8; team++)
            {
                Assert.Equal(buildingIds[team], events[team].Conditions[0].TargetId);
                Assert.Equal(unitIds[team], events[team].Conditions[1].TargetId);
                Assert.Equal(team, events[team].Actions[0].Team);
                Assert.Equal((team + 1) % 8, events[team].Actions[0].OtherTeam);
                Assert.Equal(unit, events[team].Actions[1].Alias);
                Assert.Equal(team, events[team].Actions[1].Team);
            }
            Assert.False(GetProperty<bool>(reopened, "IsDirty"));
            // Removing a referenced target must reject the save before any disk change, and undo must restore the ID.
            var beforeDelete = SnapshotDirectory(map);
            reopened.PlacementSession.RemoveMany([0]);
            Assert.False(reopened.TrySaveMap(false, out Exception? error));
            Assert.IsType<InvalidDataException>(error);
            AssertSnapshotUnchanged(map, beforeDelete);
            Assert.True(reopened.PlacementSession.Undo());
            Assert.Equal(buildingIds[0], reopened.PlacementSession[0].ScenarioId);
            Assert.True(reopened.TrySaveMap(false, out error), error?.ToString());
            AssertSnapshotUnchanged(map, beforeDelete);
            var house = placed[8].Type;
            SetMatrixField(reopened, "_objdefNames", new Dictionary<int, string> { [1] = house.NameDef });
            SetMatrixField(reopened, "_buildingTemplates", new Dictionary<int, LevelObjectTemplate>
                { [1] = Assert.Single(LevelObjectStore.Load(map).Templates()) });
            reopened.PlacementSession.Edit(8, 0, 9000, 321, 10000, 180, 0);
            reopened.PlacementSession.Edit(16, 7, 11000, 0, 12000, 270, 1);
            Assert.True(reopened.TrySaveMap(false, out error), error?.ToString());
            var edited = ScenarioDocument.Load(map);
            var moved = Assert.Single(edited.Spawns, spawn => spawn.Id == buildingIds[8]);
            Assert.Equal(0, moved.Team); Assert.Equal(9000, moved.X); Assert.Equal(321, moved.Y);
            var binding = Assert.Single(edited.DataSlots, slot => slot.SpawnId == moved.Id);
            var dataObject = Assert.Single(LevelObjectStore.Load(map).Objects(), obj => obj.Slot == binding.Slot);
            Assert.Equal(binding.Uid, dataObject.Uid);
            Assert.Equal(moved.X, dataObject.X); Assert.Equal(moved.Y, dataObject.Y); Assert.Equal(0, dataObject.Team);
            Assert.Equal(9, LevelObjectStore.Load(map).Objects().Count);
            Assert.Equal(1, Assert.Single(edited.Spawns, spawn => spawn.Id == unitIds[7]).Count);
            Assert.Equal(unitIds[7], edited.Events[7].Conditions[1].TargetId);
            var editedBytes = SnapshotDirectory(map);
            Assert.True(reopened.TrySaveMap(false, out error), error?.ToString());
            AssertSnapshotUnchanged(map, editedBytes);
            Invoke(reopened, "LoadSelectedMap");
            Assert.Equal(1, reopened.PlacementSession[16].UnitCount);
            Assert.Equal(buildingIds[8], reopened.PlacementSession[8].ScenarioId);
        });
    }

    [Theory]
    [InlineData(true, -1)]
    [InlineData(true, 8)]
    [InlineData(false, -2)]
    [InlineData(false, 16)]
    public void Invalid_placement_team_rejects_before_file_access_and_keeps_edits(bool figure, int team)
    {
        string map = CreateFixture();
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Teams", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var type = new SdlObjectType(figure ? "FigGerInf00" : "FXTest", 1,
                figure ? SdlObjectCategory.Figure : SdlObjectCategory.Effect, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "TEST" });
            SetMatrixField(form, "_objectCatalog", new[] { type });
            var placement = new SdlPlacedObject(type, 4000, 0, 5000, 0, UnitCount: figure ? 1 : 0) { ScenarioId = Guid.NewGuid() };
            Assert.Throws<ArgumentOutOfRangeException>(() => form.PlacementSession.Add(placement with { Team = team }));
            Assert.Empty(form.PlacementSession.Capture());
            form.PlacementSession.Add(placement);
            // Normal commands already reject this; inject a damaged pending snapshot to test the independent save guard.
            var pending = (List<SdlPlacedObject>)typeof(PlacementEditSession).GetField("_objects", BindingFlags.Instance | BindingFlags.NonPublic)!
                .GetValue(form.PlacementSession)!;
            pending[0] = pending[0] with { Team = team };
            using (var locked = File.Open(Path.Combine(map, "TEXT", "US", "briefing.put"), FileMode.Open, FileAccess.ReadWrite, FileShare.None))
            {
                Assert.False(form.TrySaveMap(false, out Exception? error));
                Assert.IsType<InvalidDataException>(error); // Not IOException: no briefing access before validation.
                Assert.True(form.PlacementSession.IsDirty);
                Assert.True(form.PlacementSession.CanUndo);
            }
            AssertSnapshotUnchanged(map, before);
        });
    }

    private static void SetMatrixField(MapEditorForm form, string name, object value)
        => typeof(MapEditorForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, value);

    private static void AssertSnapshotUnchanged(string map, Dictionary<string, byte[]> before)
    {
        var after = SnapshotDirectory(map);
        Assert.Equal(before.Keys.Order(), after.Keys.Order());
        foreach (var (file, bytes) in before) Assert.Equal(bytes, after[file]);
    }
}
