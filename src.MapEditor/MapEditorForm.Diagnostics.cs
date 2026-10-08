using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ListView _mapIssues = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, MultiSelect = false, ShowItemToolTips = true };
    private readonly Button _checkMap = new() { AutoSize = true };
    private readonly Button _locateMapIssue = new() { AutoSize = true };
    private readonly Button _compareSaved = new() { AutoSize = true };
    private readonly Label _mapCheckSummary = new() { Dock = DockStyle.Top, AutoSize = true, Padding = new Padding(4, 6, 4, 6) };
    private readonly TextBox _mapIssueDetails = new() { Dock = DockStyle.Bottom, Multiline = true, ReadOnly = true, WordWrap = true, ScrollBars = ScrollBars.Vertical, Height = 100 };
    private TabPage? _mapCheckTab;
    private IReadOnlyList<MapIssue> _lastMapIssues = [];
    private bool _mapDiagnosticsCurrent;

    private Control BuildMapCheckPanel()
    {
        var panel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8) };
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true };
        buttons.Controls.AddRange([_checkMap, _locateMapIssue, _compareSaved]);
        _compareSaved.Click += (_, _) => { _mapIssueDetails.Text = BuildSavedDiffReport(); };
        _mapIssues.Columns.Add("", 72); _mapIssues.Columns.Add("", 250);
        _mapIssues.Resize += (_, _) => _mapIssues.Columns[1].Width = Math.Max(100, _mapIssues.ClientSize.Width - _mapIssues.Columns[0].Width - 8);
        _checkMap.Click += (_, _) => RefreshMapDiagnostics();
        _locateMapIssue.Click += (_, _) => LocateMapIssue();
        _mapIssues.DoubleClick += (_, _) => LocateMapIssue();
        _mapIssues.SelectedIndexChanged += (_, _) =>
        {
            _locateMapIssue.Enabled = _mapDiagnosticsCurrent && _mapIssues.SelectedItems.Count > 0;
            bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            _mapIssueDetails.Text = _mapIssues.SelectedItems.Count == 1 && _mapIssues.SelectedItems[0].Tag is MapIssue issue ? (en ? issue.English : issue.Chinese) : "";
        };
        panel.Resize += (_, _) => _mapCheckSummary.MaximumSize = new Size(Math.Max(100, panel.ClientSize.Width - panel.Padding.Horizontal), 0);
        panel.Controls.Add(_mapIssues); panel.Controls.Add(_mapCheckSummary); panel.Controls.Add(_mapIssueDetails); panel.Controls.Add(buttons);
        return panel;
    }

    internal IReadOnlyList<MapIssue> RefreshMapDiagnostics()
    {
        _lastMapIssues = CollectMapDiagnostics(_terrainLayers);
        _mapDiagnosticsCurrent = true;
        RenderMapIssues();
        return _lastMapIssues;
    }

    private IReadOnlyList<MapIssue> CollectMapDiagnostics(TerrainHeightEditSession? layers)
    {
        if (_selected is null) return [];
        ScenarioDocument previous = ScenarioDocument.Load(_selected.DirectoryPath);
        LevelObjectStore? store = null;
        if (_placedObjects.Any(item => item.Type.Category == SdlObjectCategory.Building) && _natureStoreAvailable)
            store = LevelObjectStore.Load(_selected.DirectoryPath);
        var objects = new List<MapCheckObject>();
        foreach (var item in _placedObjects)
        {
            bool building = item.Type.Category == SdlObjectCategory.Building;
            var spawn = new ScenarioSpawn(AliasOf(item.Type), item.WorldX, item.WorldZ, item.Team,
                item.Type.Category == SdlObjectCategory.Figure && !item.Type.IsAnimal ? Math.Max(1, item.UnitCount) : 0,
                (int)MathF.Round(item.Angle), item.WorldY, Prebuilt: building && item.Team is >= 0 and <= 8) { Id = item.ScenarioId };
            bool hasTemplate = false;
            if (building)
            {
                var old = previous.Spawns.FirstOrDefault(value => value.Id == spawn.Id && value.Prebuilt && value.Alias.Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase));
                var binding = old is null ? null : ScenarioObjectIdentity.DataBinding(previous, old.Id);
                hasTemplate = binding is not null && store?.OwnedTemplate(binding.Slot, binding.Uid) is not null;
                if (!hasTemplate) hasTemplate = BuildingTemplateFor(spawn) is not null;
            }
            objects.Add(new(spawn, building, hasTemplate));
        }
        // An incomplete catalog can omit persisted placements from the UI. Event-only/property saves retain
        // those spawns; diagnostics must inspect the same effective scenario rather than report false deletions.
        if (!PlacedDirty())
        {
            var visibleIds = objects.Select(item => item.Spawn.Id).ToHashSet();
            foreach (ScenarioSpawn saved in previous.Spawns.Where(spawn => !visibleIds.Contains(spawn.Id)))
                objects.Add(new(saved, saved.Prebuilt, false));
        }
        var snapshot = new MapCheckSnapshot(objects, EventSession.Capture(), _objectCatalog.Select(AliasOf).ToArray(),
            layers?.CollisionSize ?? 0, layers?.Collision, layers?.VertexSize ?? 0,
            layers?.Heights, _heightMapStep, (float)_waterLevel.Value);
        var issues = new List<MapIssue>(MapDiagnostics.Check(snapshot));
        // 進階檢查只作警告（連通性與預算皆為估計），不阻擋儲存。
        issues.AddRange(MapDiagnostics.ConvertToIssues(MapDiagnostics.AnalyzeNavMesh(snapshot)));
        var budget = AgainstRomeMapEditor.Modules.Profiling.MapBudgetProfiler.Analyze(new(
            Spawns: objects.Select(item => item.Spawn).ToArray(),
            CollisionSize: snapshot.CollisionSize, Collision: snapshot.Collision,
            HeightSize: snapshot.HeightSize, Heights: snapshot.Heights,
            HeightStep: snapshot.HeightStep, WaterLevel: snapshot.WaterLevel));
        issues.AddRange(budget.Bottlenecks.Select(item => new MapIssue(MapIssueSeverity.Warning, item.Code,
            item.ChineseDescription, item.EnglishDescription, WorldX: item.WorldX, WorldZ: item.WorldZ)));
        return issues;
    }

    private void LocalizeMapDiagnostics(bool en)
    {
        if (_mapCheckTab is not null) _mapCheckTab.Text = en ? "Map Check" : "地圖檢查";
        if (_consoleTab is not null) _consoleTab.Text = en ? "Console" : "控制台";
        _checkMap.Text = en ? "Check now" : "立即檢查";
        _locateMapIssue.Text = en ? "Locate" : "定位";
        _compareSaved.Text = en ? "Compare with saved" : "與已儲存版本比較";
        _mapIssues.Columns[0].Text = en ? "Severity" : "程度";
        _mapIssues.Columns[1].Text = en ? "Issue" : "問題";
        RenderMapIssues();
    }

    private void RenderMapIssues()
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _mapIssues.BeginUpdate(); _mapIssues.Items.Clear();
        foreach (MapIssue issue in _lastMapIssues)
        {
            var row = new ListViewItem(issue.Severity == MapIssueSeverity.Error ? (en ? "Error" : "錯誤") : (en ? "Warning" : "警告")) { Tag = issue, ToolTipText = en ? issue.English : issue.Chinese };
            row.SubItems.Add(en ? issue.English : issue.Chinese);
            _mapIssues.Items.Add(row);
        }
        _mapIssues.EndUpdate();
        int errors = _lastMapIssues.Count(issue => issue.Severity == MapIssueSeverity.Error);
        int warnings = _lastMapIssues.Count - errors;
        _mapCheckSummary.Text = en
            ? $"{errors} errors, {warnings} warnings. Warnings allow saving. Connectivity is an estimate; check game pathfinding."
            : $"{errors} 個錯誤，{warnings} 個警告。警告仍可儲存；連通性為估計，須另驗證遊戲尋路。";
        _locateMapIssue.Enabled = false;
    }

    private void InvalidateMapDiagnostics()
    {
        _mapDiagnosticsCurrent = false; _locateMapIssue.Enabled = false;
        _checkMap.Enabled = _selected is not null;
        if (_mapCheckTab is null) return;
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _mapCheckSummary.Text = en ? "Check before saving. The last result may be stale after edits." : "儲存前請檢查；編輯後上次結果可能已過時。";
    }

    internal void LocateMapIssue()
    {
        if (!_mapDiagnosticsCurrent) { RefreshMapDiagnostics(); return; }
        if (_mapIssues.SelectedItems.Count != 1 || _mapIssues.SelectedItems[0].Tag is not MapIssue issue) return;
        if (issue.WorldX is { } x && issue.WorldZ is { } z && float.IsFinite(x) && float.IsFinite(z))
        { _canvas.FocusTile(x / 256, z / 256); _view3d?.FocusTile(x / 256, z / 256); }
        if (issue.EventIndex >= 0 && issue.EventIndex < _events.Count)
        { _inspectorTabs.SelectedIndex = 5; _eventList.SelectedIndex = issue.EventIndex; }
        else if (issue.ObjectId != Guid.Empty)
        {
            int index = Enumerable.Range(0, _placedObjects.Count).FirstOrDefault(index => _placedObjects[index].ScenarioId == issue.ObjectId, -1);
            if (index >= 0)
            {
                if (_placedList.Items.Count != _placedObjects.Count) RefreshPlacedList();
                _inspectorTabs.SelectedIndex = 3; _ = _placedList.Handle;
                _placedList.SelectedItems.Clear(); _placedList.Items[index].Selected = true; _placedList.EnsureVisible(index);
            }
        }
    }
}
