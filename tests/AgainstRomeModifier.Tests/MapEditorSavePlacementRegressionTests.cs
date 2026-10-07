using System.Buffers.Binary;
using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Single_soldier_survives_failed_save_retry_and_reload_as_a_unit_target()
    {
        string map = CreateFixture("ENDL_008");
        RunInSta(() =>
        {
            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1,
                new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_008", map, true, "Single", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });
            Guid id = Guid.NewGuid();
            form.PlacementSession.Add(new(type, 4000, 0, 5000, 0, 45, 1) { ScenarioId = id });
            var events = GetField<List<ScenarioEvent>>(form, "_events");
            events.Add(new("Single unit target") { Actions = [new(ScenarioActionKind.Message, "Ready")],
                Conditions = [new(ScenarioConditionKind.ObjectInArea, id, 3900, 4900, 4100, 5100)] });
            Assert.False(form.TrySaveMap(false, out Exception? error));
            Assert.IsType<FileNotFoundException>(error);
            Assert.True(form.PlacementSession.IsDirty);
            Assert.True(form.PlacementSession.CanUndo);
            Assert.False(File.Exists(Path.Combine(map, ScenarioDocument.FileName)));
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.Null(error);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.False(form.PlacementSession.CanUndo);
            var saved = ScenarioDocument.Load(map);
            var spawn = Assert.Single(saved.Spawns);
            Assert.Equal(1, spawn.Count);
            Assert.Equal(id, spawn.Id);
            Assert.Equal(id, Assert.Single(Assert.Single(saved.Events).Conditions).TargetId);
            byte[] script = File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile));
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.Equal(script, File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile)));
            Assert.Single(ScenarioDocument.Load(map).Spawns);
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(id, Assert.Single(form.PlacementSession.Capture()).ScenarioId);
            Assert.Equal(1, form.PlacementSession[0].UnitCount);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        });
    }

    [Fact]
    public void Pending_nature_addition_rolls_back_then_is_written_once_on_retry()
    {
        string map = CreateFixture("ENDL_009");
        WriteEmptyWorldStore(map, 8);
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_009", map, true, "Nature retry", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var nature = GetField<NatureEditSession>(form, "_natureSession");
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
                new Dictionary<int, string> { [1] = "LanGerLau00" });
            byte[] record = new byte[79];
            record[0] = 1;
            BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(75), 1);
            uint[] columns = new uint[18];
            columns[10] = 0xFFFF;
            var template = new LevelObjectTemplate(1, record, columns,
                LevelObjectStore.ObjDataWidths.Select(width => new byte[width]).ToArray(), new byte[17], new byte[17]);
            nature.Plant(new(template, "LanGerLau00", 4000, 0, 5000, 0.75f));
            nature.CommitStroke();
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Nature"));
            Invoke(form, "RefreshSceneMarkers");
            Assert.Equal(-200000, Assert.Single(NatureMarkers(form)).ObjectIndex);
            GetField<List<ScenarioEvent>>(form, "_events").Add(new("Trigger script write")
                { Actions = [new(ScenarioActionKind.Message, "Ready")] });
            Assert.False(form.TrySaveMap(false, out Exception? error));
            Assert.IsType<FileNotFoundException>(error);
            foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(Path.Combine(map, path)));
            Assert.Empty(LevelObjectStore.Load(map).Objects());
            Assert.Single(nature.Additions);
            Assert.True(nature.IsDirty);
            Assert.True(nature.CanUndo);
            Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
            File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            var added = Assert.Single(LevelObjectStore.Load(map).Objects());
            Assert.Equal(4000, added.X);
            Assert.Equal(5000, added.Z);
            Assert.Equal(0.75f, added.Rotation);
            Assert.Empty(nature.Additions);
            Assert.False(nature.IsDirty);
            Assert.False(nature.CanUndo);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            Assert.Equal(-100000 - added.Slot, Assert.Single(NatureMarkers(form)).ObjectIndex);
            byte[] objects = File.ReadAllBytes(Path.Combine(map, "DATA", "objects.dat"));
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.Equal(objects, File.ReadAllBytes(Path.Combine(map, "DATA", "objects.dat")));
            Assert.Single(LevelObjectStore.Load(map).Objects());
            nature.Remove([added.Slot], []);
            nature.CommitStroke();
            Invoke(form, "RefreshSceneMarkers");
            Assert.Empty(NatureMarkers(form));
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            Assert.Empty(LevelObjectStore.Load(map).Objects());
            Assert.Empty(NatureMarkers(form));
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        });
    }

    private static IEnumerable<MapSceneObject> NatureMarkers(MapEditorForm form)
    {
        var canvas = GetField<MapCanvasControl>(form, "_canvas");
        var objects = (IReadOnlyList<MapSceneObject>)typeof(MapCanvasControl)
            .GetField("_sceneObjects", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(canvas)!;
        return objects.Where(item => item.SourceFile == "DATA/objects.dat");
    }

    private static void WriteEmptyWorldStore(string map, int count)
    {
        byte[] objects = new byte[16 + count * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
        BinaryPrimitives.WriteInt32LittleEndian(objects, 1);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(4), count);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(8), 30);
        BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(12), 30);
        File.WriteAllBytes(Path.Combine(map, "DATA", "objects.dat"), objects);
        byte[] data = new byte[8 + count * LevelObjectStore.ObjDataWidths.Sum()];
        BinaryPrimitives.WriteInt32LittleEndian(data, 1);
        BinaryPrimitives.WriteInt32LittleEndian(data.AsSpan(4), count);
        File.WriteAllBytes(Path.Combine(map, "DATA", "objdata.dat"), data);
        byte[] positions = new byte[8 + count * 2 * LevelObjectStore.PositionSize];
        BinaryPrimitives.WriteInt32LittleEndian(positions, 1);
        BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(4), count * 2);
        File.WriteAllBytes(Path.Combine(map, "DATA", "position.dat"), positions);
    }
}
