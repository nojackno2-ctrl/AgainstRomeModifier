using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Map_check_is_readonly_locates_object_and_event_and_rejects_invalid_save()
    {
        string map = CreateFixture();
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Check", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", false);
            var type = new SdlObjectType("FigGerUnit", 1, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "UNIT" });
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });
            var unit = new SdlPlacedObject(type, 8192, 0, 8192, 0, 0, 10) { ScenarioId = Guid.NewGuid() };
            form.PlacementSession.Add(unit);
            form.PlacementSession.Add(new(type, 8200, 0, 8200, 0, 0, 10) { ScenarioId = Guid.NewGuid() });
            Invoke(form, "RefreshPlacedList");
            var events = GetField<List<ScenarioEvent>>(form, "_events");
            events.Add(new("Deleted target") { Conditions = [new(ScenarioConditionKind.ObjectExists, Guid.NewGuid())], Actions = [new(ScenarioActionKind.Victory)] });
            Invoke(form, "RefreshEventList", 0); Invoke(form, "UpdateEditorState");
            var before = SnapshotDirectory(map);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            var issues = form.RefreshMapDiagnostics();
            AssertSnapshotUnchanged(map, before); Assert.True(form.PlacementSession.CanUndo);
            Assert.Contains(issues, issue => issue.Code == "overlap");
            Assert.Contains(issues, issue => issue.Code == "event-target");
            var rows = GetField<ListView>(form, "_mapIssues"); _ = rows.Handle;
            var objectRow = rows.Items.Cast<ListViewItem>().First(row => row.Tag is MapIssue issue && issue.Code == "overlap" && issue.ObjectId == unit.ScenarioId);
            objectRow.Selected = true; form.LocateMapIssue();
            Assert.Equal(unit.ScenarioId, form.PlacementSession[(int)Assert.Single(form.PlacedList.SelectedItems.Cast<ListViewItem>()).Tag!].ScenarioId);
            rows.SelectedItems.Clear();
            var eventRow = rows.Items.Cast<ListViewItem>().Single(row => row.Tag is MapIssue issue && issue.Code == "event-target");
            eventRow.Selected = true; form.LocateMapIssue();
            Assert.Equal(0, GetField<ListBox>(form, "_eventList").SelectedIndex);
            Assert.Equal(5, GetField<TabControl>(form, "_inspectorTabs").SelectedIndex);
            Assert.False(form.TrySaveMap(false, out Exception? error)); Assert.IsType<InvalidDataException>(error);
            Assert.Equal("地圖檢查", GetField<TabControl>(form, "_inspectorTabs").SelectedTab!.Text);
            AssertSnapshotUnchanged(map, before); Assert.True(GetProperty<bool>(form, "IsDirty"));
        });
    }

    [Fact]
    public void Map_check_warnings_allow_save_and_panel_localizes_without_losing_results()
    {
        string map = CreateFixture();
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        RunInSta(() =>
        {
            var previous = Loc.CurrentLanguage;
            try
            {
                Loc.OverrideLanguageForTesting(Language.TraditionalChinese);
                using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Warnings", "Test"));
                OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
                typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
                form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
                form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", false);
                GetField<TextBox>(form, "_title").Text = "Warnings are acceptable";
                var issues = form.RefreshMapDiagnostics();
                Assert.Contains(issues, issue => issue.Code == "no-start");
                Assert.DoesNotContain(issues, issue => issue.Severity == MapIssueSeverity.Error);
                Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                var snapshot = SnapshotDirectory(map);
                Loc.OverrideLanguageForTesting(Language.English); Invoke(form, "LocalizeMapDiagnostics", true);
                Assert.Equal("Map Check", GetField<TabPage>(form, "_mapCheckTab").Text);
                var rows = GetField<ListView>(form, "_mapIssues");
                Assert.Contains(rows.Items.Cast<ListViewItem>(), row => row.SubItems[1].Text.Contains("No editor-placed"));
                AssertSnapshotUnchanged(map, snapshot);
                string? output = Environment.GetEnvironmentVariable("ARM_DIAGNOSTICS_OUTPUT");
                if (!string.IsNullOrWhiteSpace(output))
                {
                    Directory.CreateDirectory(output);
                    var tabs = GetField<TabControl>(form, "_inspectorTabs"); tabs.SelectedTab = GetField<TabPage>(form, "_mapCheckTab");
                    form.Size = new Size(1100, 760); form.PerformLayout(); Application.DoEvents();
                    rows.Items[0].Selected = true;
                    using var image = new Bitmap(form.Width, form.Height); form.DrawToBitmap(image, new Rectangle(Point.Empty, form.Size));
                    image.Save(Path.Combine(output, "map-check-en.png"));
                    Loc.OverrideLanguageForTesting(Language.TraditionalChinese); Invoke(form, "LocalizeMapDiagnostics", false);
                    rows.Items[0].Selected = true;
                    using var chinese = new Bitmap(form.Width, form.Height); form.DrawToBitmap(chinese, new Rectangle(Point.Empty, form.Size));
                    chinese.Save(Path.Combine(output, "map-check-zh.png"));
                }
            }
            finally { Loc.OverrideLanguageForTesting(previous); }
        });
    }
}
