using System.Buffers.Binary;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData(Language.English)]
    [InlineData(Language.TraditionalChinese)]
    public void Missing_building_template_keeps_target_identity_and_displays_construction_notice_after_save_and_reopen(Language language)
    {
        string map = CreateFixture();
        WriteEmptyWorldStore(map, 8);
        var building = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
            new Dictionary<string, string> { ["alias"] = "GER_HOUSE" });
        Guid id = Guid.NewGuid();
        RunInSta(() =>
        {
            Language previous = Loc.CurrentLanguage;
            Loc.OverrideLanguageForTesting(language);
            try
            {
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Missing template", "Test"));
                _ = form.Handle;
                Invoke(form, "LoadSelectedMap");
                SetMatrixField(form, "_objectCatalog", new[] { building });
                SetMatrixField(form, "_objdefNames", new Dictionary<int, string> { [2] = building.NameDef });
                SetMatrixField(form, "_buildingTemplates", new Dictionary<int, LevelObjectTemplate>());
                form.PlacementSession.Add(new(building, 4000, 320, 5000, 8) { ScenarioId = id });
                GetField<List<ScenarioEvent>>(form, "_events").Add(new("House exists") {
                    Conditions = [new(ScenarioConditionKind.ObjectExists, id)], Actions = [new(ScenarioActionKind.Message, "Ready")] });
                var before = SnapshotDirectory(map);
                Assert.False(form.TrySaveMap(false, out Exception? error));
                Assert.IsType<FileNotFoundException>(error);
                AssertSnapshotUnchanged(map, before);
                Assert.DoesNotContain("GER_HOUSE", GetField<Label>(form, "_placeHint").Text);
                Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
                File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
                Assert.True(form.TrySaveMap(false, out error), error?.ToString());
                var saved = ScenarioDocument.Load(map);
                var spawn = Assert.Single(saved.Spawns);
                Assert.False(spawn.Prebuilt); Assert.Equal(id, spawn.Id); Assert.Equal(8, spawn.Team);
                Assert.Empty(saved.DataSlots); Assert.Empty(LevelObjectStore.Load(map).Objects());
                Assert.Equal(id, saved.Events[0].Conditions[0].TargetId);
                AssertConstructionNotice(form, language);
                var after = SnapshotDirectory(map);
                Assert.True(form.TrySaveMap(false, out error), error?.ToString());
                AssertSnapshotUnchanged(map, after);
                AssertConstructionNotice(form, language);
                using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Reopened", "Test"));
                _ = reopened.Handle;
                SetMatrixField(reopened, "_objectCatalog", new[] { building });
                Invoke(reopened, "LoadSelectedMap");
                Assert.Equal(id, Assert.Single(reopened.PlacementSession.Capture()).ScenarioId);
                AssertConstructionNotice(reopened, language);
                // Once a finished template becomes available, an edited building returns to DATA and clears the notice.
                byte[] record = new byte[LevelObjectStore.RecordSize]; record[0] = 1;
                BinaryPrimitives.WriteUInt16LittleEndian(record.AsSpan(75), 2);
                uint[] columns = new uint[18]; columns[10] = 0xFFFF;
                var template = new LevelObjectTemplate(2, record, columns,
                    LevelObjectStore.ObjDataWidths.Select(width => new byte[width]).ToArray(), new byte[17], new byte[17]);
                SetMatrixField(reopened, "_objdefNames", new Dictionary<int, string> { [2] = building.NameDef });
                SetMatrixField(reopened, "_buildingTemplates", new Dictionary<int, LevelObjectTemplate> { [2] = template });
                reopened.PlacementSession.Edit(0, 8, 4200, 320, 5000, 0, 0);
                Assert.True(reopened.TrySaveMap(false, out error), error?.ToString());
                var completed = ScenarioDocument.Load(map);
                Assert.True(Assert.Single(completed.Spawns).Prebuilt);
                Assert.Equal(id, Assert.Single(completed.DataSlots).SpawnId);
                Assert.Single(LevelObjectStore.Load(map).Objects());
                Assert.DoesNotContain("GER_HOUSE", GetField<Label>(reopened, "_placeHint").Text);
                reopened.PlacementSession.RemoveMany([0]);
                GetField<List<ScenarioEvent>>(reopened, "_events").Clear();
                Assert.True(reopened.TrySaveMap(false, out error), error?.ToString());
                Assert.DoesNotContain("GER_HOUSE", GetField<Label>(reopened, "_placeHint").Text);
            }
            finally { Loc.OverrideLanguageForTesting(previous); }
        });
    }

    private static void AssertConstructionNotice(MapEditorForm form, Language language)
    {
        string hint = GetField<Label>(form, "_placeHint").Text;
        Assert.Contains("GER_HOUSE", hint);
        Assert.Contains(language == Language.English ? "construction sites" : "工地", hint);
    }
}
