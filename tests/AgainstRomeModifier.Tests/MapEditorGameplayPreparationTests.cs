using System.Drawing;
using System.Reflection;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>Prepare retained gameplay candidates from verified TEMP copies. This does not run the game.</summary>
    [Fact]
    public void Gameplay_candidates_prepare_empty_control_authored_scene_and_demo_without_changing_sources()
    {
        if (Environment.GetEnvironmentVariable("ARM_GAMEPLAY_PREPARE") != "1") return;
        string blank = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_GAMEPLAY_BLANK_SOURCE"));
        string demo = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_GAMEPLAY_DEMO_SOURCE"));
        string output = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_GAMEPLAY_OUTPUT"));
        Assert.False(Directory.Exists(output), "Use a new output directory.");
        Assert.False(output.StartsWith(blank + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase));
        Assert.False(output.StartsWith(demo + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase));
        var blankHashes = DemoHashes(blank);
        var demoHashes = DemoHashes(demo);
        string game = Path.Combine(output, "workspace", "game");
        CopyDemoDirectory(blank, game);
        string control = Path.Combine(game, "MAPS", "ENDL_005");
        Assert.True(CustomMapManifest.HasStandaloneLevel(control));
        Assert.Empty(LevelObjectStore.Load(control).Objects());
        Assert.Empty(ScenarioDocument.Load(control).Spawns);
        Assert.Empty(ScenarioDocument.Load(control).Events);
        var controlHashes = DemoHashes(control);
        var authored = new EndlessMapCloner().Clone(game, "ENDL_005", 8, "ARM Empty Playtest");
        Guid unitId = Guid.NewGuid();
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using (var form = new MapEditorForm(game, new GameMapInfo(authored.Id, authored.DirectoryPath, true, authored.DisplayName, "Test")))
            {
                typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
                form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
                form.Show(); Application.DoEvents();
                var type = GetField<IReadOnlyList<SdlObjectType>>(form, "_objectCatalog")
                    .Single(item => item.TemplateFields.TryGetValue("alias", out string? alias)
                        && alias.Equals("GER_INF01", StringComparison.OrdinalIgnoreCase));
                form.PlacementSession.Add(new SdlPlacedObject(type, 8192, 200, 8192, 0, 90, 10) { ScenarioId = unitId });
                var events = GetField<List<ScenarioEvent>>(form, "_events");
                events.Add(new("ARM empty start", 3) { Actions = [new(ScenarioActionKind.Message, "ARM empty scene: move the ten soldiers east to test victory.")] });
                events.Add(new("ARM empty area win", 1)
                {
                    Conditions = [new(ScenarioConditionKind.ObjectInArea, unitId, 9500, 7600, 10500, 8800)],
                    Actions = [new(ScenarioActionKind.Message, "ARM empty scene: target reached."), new(ScenarioActionKind.Victory)]
                });
                Assert.True(form.TrySaveMap(false, out var error), error?.ToString());
            }
            var bytes = DemoHashes(authored.DirectoryPath);
            using var fresh = new MapEditorForm(game, new GameMapInfo(authored.Id, authored.DirectoryPath, true, authored.DisplayName, "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(fresh, true);
            fresh.StartPosition = FormStartPosition.Manual; fresh.Location = new Point(-30000, -30000);
            fresh.Show(); Application.DoEvents();
            var unit = Assert.Single(fresh.PlacementSession.Capture());
            Assert.Equal(unitId, unit.ScenarioId); Assert.Equal(10, unit.UnitCount); Assert.Equal(0, unit.Team);
            Assert.True(fresh.TrySaveMap(false, out var freshError), freshError?.ToString());
            Assert.Equal(bytes, DemoHashes(authored.DirectoryPath));
        });
        var scenario = ScenarioDocument.Load(authored.DirectoryPath);
        Assert.Equal(unitId, Assert.Single(scenario.Spawns).Id);
        Assert.Equal(2, scenario.Events.Count);
        Assert.Equal(unitId, Assert.Single(scenario.Events.Single(item => item.Name == "ARM empty area win").Conditions).TargetId);
        Assert.Equal(controlHashes, DemoHashes(control));
        CopyDemoDirectory(control, Path.Combine(output, "candidates", "empty-control", "MAPS", "ENDL_005"));
        CopyDemoDirectory(authored.DirectoryPath, Path.Combine(output, "candidates", "empty-authored", "MAPS", "ENDL_008"));
        string demoMap = Path.Combine(demo, "MAPS", "ENDL_005");
        Assert.NotEmpty(ScenarioDocument.Load(demoMap).Spawns);
        Assert.NotEmpty(ScenarioDocument.Load(demoMap).Events);
        CopyDemoDirectory(demoMap, Path.Combine(output, "candidates", "demo", "MAPS", "ENDL_005"));
        Assert.Equal(DemoHashes(demoMap), DemoHashes(Path.Combine(output, "candidates", "demo", "MAPS", "ENDL_005")));
        Assert.Equal(blankHashes, DemoHashes(blank)); Assert.Equal(demoHashes, DemoHashes(demo));
        File.WriteAllText(Path.Combine(output, "candidate-hashes.json"), JsonSerializer.Serialize(DemoHashes(Path.Combine(output, "candidates")), Core.Services.JsonDefaults.Indented));
        File.WriteAllText(Path.Combine(output, "source-hashes.json"), JsonSerializer.Serialize(new { Blank = blankHashes, Demo = demoHashes }, Core.Services.JsonDefaults.Indented));
        File.WriteAllText(Path.Combine(output, "result.json"), JsonSerializer.Serialize(new
        {
            Status = "prepared-gameplay-not-run", SourceUnchanged = true, FreshEditorReload = true,
            Candidates = new[] { "empty-control/MAPS/ENDL_005", "empty-authored/MAPS/ENDL_008", "demo/MAPS/ENDL_005" },
            Unit = new { Id = unitId, Alias = "GER_INF01", Count = 10, Team = 0, X = 8192, Y = 200, Z = 8192 },
            VictoryArea = new { X1 = 9500, Z1 = 7600, X2 = 10500, Z2 = 8800 },
            Pending = "game startup, ownership and orders, resources and settlements, pathfinding, victory and game save/load"
        }, Core.Services.JsonDefaults.Indented));
    }
}
