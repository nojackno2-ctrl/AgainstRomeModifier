using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Campaign_dialog_merges_current_events_with_cancel_validation_undo_and_redo()
    {
        string map = CreateFixture("ENDL_005");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Waves", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var catalog = ScriptObjectAliases.Parse("""
                [ObjDefName]
                HUN_KAVINF00 = FigHunKavInf00
                HUN_KAVSCH00 = FigHunKavSch00
                """).Select(alias => new SdlObjectType(alias.NameDef, -1, SdlObjectCategory.Figure, alias.Tribe, 0,
                    new Dictionary<string, string> { ["alias"] = alias.Alias })).ToList();
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, catalog);
            var session = (ScenarioEventSession)typeof(MapEditorForm).GetProperty("EventSession", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            var existing = new ScenarioEvent("Unsaved event") { Actions = [new(ScenarioActionKind.Message, Text: "Keep this")] };
            session.Add(existing);
            var baseline = session.Baseline;
            form.CampaignWaveDialogRunner = _ => DialogResult.Cancel;
            form.RunCampaignWaves();
            Assert.Equal(existing.Name, Assert.Single(session.Capture()).Name);
            Assert.Equal(existing.Actions, session.Capture()[0].Actions);
            CampaignMissionPlan? plan = null;
            form.CampaignWaveDialogRunner = dialog =>
            {
                dialog.WaveCount = 2; dialog.FirstDelaySeconds = 30; dialog.IntervalSeconds = 45;
                plan = dialog.Plan;
                return DialogResult.OK;
            };
            form.RunCampaignWaves();
            Assert.Equal(3, session.Count);
            Assert.Equal(new[] { 30, 75 }, session.Capture().Skip(1).Select(e => e.DelaySeconds));
            Assert.Equal("Unsaved event", session.Capture()[0].Name);
            Assert.True(session.IsDirty);
            Assert.Equal(baseline.Count, session.Baseline.Count);
            Invoke(form, "Undo");
            Assert.Single(session.Capture());
            Assert.True(session.IsDirty); // Existing unsaved edit remains.
            Invoke(form, "Redo");
            Assert.Equal(3, session.Count);
            // A test runner returning OK cannot bypass the compiler's unknown-alias rejection.
            form.CampaignWaveDialogRunner = dialog => { dialog.ArchetypeId = AiArchetypeCatalog.IdRomanFortress; return DialogResult.OK; };
            form.RunCampaignWaves();
            Assert.Equal(3, session.Count);
            Assert.False(form.ApplyCampaignWaves(plan! with { FactionProfiles = [new(1, AiArchetypeCatalog.IdNomadicRaider)] }).Success);
            Assert.Equal(3, session.Count);
            form.MarkEventGraphChanged();
            Assert.False(form.ApplyCampaignWaves(plan!).Success);
            Assert.Equal(3, session.Count);
            Assert.Empty(ScenarioDocument.Load(map).Events);
        });
    }

    [Fact]
    public void Merging_scenario_events_renames_collisions_and_respects_the_event_cap()
    {
        string map = CreateFixture("ENDL_005");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Waves", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var session = (ScenarioEventSession)typeof(MapEditorForm).GetProperty("EventSession", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            var batch = new[] { new ScenarioEvent("Wave_01") { Actions = [new(ScenarioActionKind.Message, Text: "A")] } };
            Assert.True(form.MergeScenarioEvents(batch));
            Assert.True(form.MergeScenarioEvents(batch));
            Assert.Equal(new[] { "Wave_01", "Wave_01_2" }, session.Capture().Select(e => e.Name));
            Invoke(form, "Undo");
            Assert.Single(session.Capture());
            var tooMany = Enumerable.Range(0, 256).Select(i => new ScenarioEvent($"E{i}") { Actions = [new(ScenarioActionKind.Message, Text: "x")] }).ToList();
            Assert.False(form.MergeScenarioEvents(tooMany));
            Assert.Single(session.Capture());
        });
    }

    [Fact]
    public void Objective_and_wild_lair_menu_items_open_their_dialogs_and_cancel_leaves_events_untouched()
    {
        string map = CreateFixture("ENDL_005");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Menus", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            var session = (ScenarioEventSession)typeof(MapEditorForm).GetProperty("EventSession", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            ToolStripMenuItem Item(string field) => (ToolStripMenuItem)typeof(MapEditorForm).GetField(field, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)!;
            var objective = Item("_objectiveStudio"); var lair = Item("_wildLairs");
            Assert.True(objective.Enabled); Assert.True(lair.Enabled);
            int opened = 0;
            form.ObjectiveStudioDialogRunner = _ => { opened++; return DialogResult.Cancel; };
            form.WildLairDialogRunner = _ => { opened++; return DialogResult.Cancel; };
            objective.PerformClick(); lair.PerformClick();
            Assert.Equal(2, opened);
            Assert.Equal(0, session.Count);
        });
    }
}
