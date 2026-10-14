using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeMapEditor.Modules.Placement;
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
        IReadOnlyDictionary<Guid, ScenarioFamilyCopyRequest>? familyRequests = null;
        if (store is not null && _placedObjects.Any(item => item.CopySourceScenarioId != Guid.Empty || item.NativeSourceIdentity.HasValue))
        {
            try
            {
                familyRequests = PlacementEditSession.BuildFamilyCopyRequests(_placedObjects, previous, store, ResolveNativeAlias);
            }
            catch (Exception ex) when (ex is InvalidDataException or InvalidOperationException)
            {
                // Stale or invalid copy intents must not crash overall diagnostics;
                // they remain unhandled so normal construction-site warnings or preflights apply.
                familyRequests = null;
            }
        }
        var objects = new List<MapCheckObject>();
        foreach (var item in _placedObjects)
        {
            bool building = item.Type.Category == SdlObjectCategory.Building;
            bool isPrebuilt = building && item.Team is >= 0 and <= 8 || item.RawRotation.HasValue;
            var spawn = new ScenarioSpawn(AliasOf(item.Type), item.WorldX, item.WorldZ, item.Team,
                item.Type.Category == SdlObjectCategory.Figure && !item.Type.IsAnimal && !item.RawRotation.HasValue ? Math.Max(1, item.UnitCount) : 0,
                item.RawRotation is { } raw ? ScenarioLevelObjects.RadiansToDisplayAngle(raw) : (int)MathF.Round(item.Angle),
                item.WorldY,
                Prebuilt: isPrebuilt) { Id = item.ScenarioId, RawRotation = item.RawRotation };
            bool hasTemplate = false;
            if (building)
            {
                if (familyRequests is not null && familyRequests.TryGetValue(item.ScenarioId, out var req))
                {
                    if (req.IsNativeSource)
                    {
                        hasTemplate = true;
                    }
                    else if (previous.Spawns.Any(source => source.Id == item.CopySourceScenarioId
                        && source.Prebuilt && source.Alias.Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase)))
                    {
                        hasTemplate = true;
                    }
                }
                else
                {
                    var old = previous.Spawns.FirstOrDefault(value => value.Id == spawn.Id && value.Prebuilt && value.Alias.Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase));
                    var binding = old is null ? null : ScenarioObjectIdentity.DataBinding(previous, old.Id);
                    if (binding is not null)
                    {
                        hasTemplate = store?.OwnedTemplate(binding.Slot, binding.Uid) is not null
                            || IsRetainedClosedFamily(store, previous, spawn, binding);
                    }
                    if (!hasTemplate) hasTemplate = BuildingTemplateFor(spawn) is not null;
                }
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
        var missingPools = NativeMapLoadContract.FindMissingDataFiles(_selected.DirectoryPath);
        if (missingPools.Count > 0)
        {
            string files = string.Join(", ", missingPools);
            issues.Add(new MapIssue(MapIssueSeverity.Warning, "native-data-missing",
                $"地圖缺少必要的 DATA 檔案：{files}。缺檔可能導致遊戲資料池未完整載入。請從完整地圖範本重新建立，或從此地圖的備份還原缺檔；不要建立空檔。仍可編輯及儲存，補齊前無法使用遊戲預覽。",
                $"Required DATA files are missing: {files}. Missing files may leave game data pools incompletely loaded. Recreate from a complete map template or restore missing files from this map's backup; do not create empty files. Editing and saving remain available; restore the files before game preview."));
        }
        IReadOnlyDictionary<int, NativeObjectDefinition>? definitions = null;
        try { definitions = NativeObjectDefinitions.Load(_gamePath); }
        catch (Exception ex) when (ex is IOException or InvalidDataException)
        {
            issues.Add(new MapIssue(MapIssueSeverity.Warning, "native-object-definitions-invalid",
                $"無法讀取物件定義 objdef.dau：{ex.Message}。第三位置需求尚未驗證；請從備份還原定義檔。",
                $"Cannot read objdef.dau: {ex.Message}. Third-position requirements have not been verified; restore the definition file from backup."));
        }
        // These are saved DATA diagnostics. Pending editor changes may repair them on save.
        foreach (NativePoolIssue issue in NativePoolDiagnostics.DiagnoseDirectory(Path.Combine(_selected.DirectoryPath, "DATA"), definitions).Issues)
            issues.Add(new MapIssue(MapIssueSeverity.Warning, "native-data-" + issue.Code,
                $"已儲存的 DATA 資料：{issue.Message}。請檢查備份或修復資料；此結果尚未包含未儲存的編輯。",
                $"Saved DATA: {issue.Code}; object slot {issue.ObjectSlot}, target {issue.TargetPool ?? "pool"} slot {issue.TargetSlot}. Check the map backup or repair the data. Pending edits are not included in this result."));
        // 進階檢查只作警告（連通性與預算皆為估計），不阻擋儲存。
        issues.AddRange(MapDiagnostics.ConvertToIssues(MapDiagnostics.AnalyzeNavMesh(snapshot)));
        var budget = AgainstRomeMapEditor.Modules.Profiling.MapBudgetProfiler.Analyze(new(
            Spawns: objects.Select(item => item.Spawn).ToArray(),
            CollisionSize: snapshot.CollisionSize, Collision: snapshot.Collision,
            HeightSize: snapshot.HeightSize, Heights: snapshot.Heights,
            HeightStep: snapshot.HeightStep, WaterLevel: snapshot.WaterLevel));
        issues.AddRange(budget.Bottlenecks.Select(item => new MapIssue(MapIssueSeverity.Warning, item.Code,
            item.ChineseDescription, item.EnglishDescription, WorldX: item.WorldX, WorldZ: item.WorldZ)));

        var designRequest = new MapDesignValidationRequest(
            Objects: objects.Select(item => item.Spawn).ToArray(),
            Events: snapshot.Events,
            Collision: snapshot.Collision,
            CollisionSize: snapshot.CollisionSize,
            Heights: snapshot.Heights,
            HeightSize: snapshot.HeightSize,
            HeightStep: snapshot.HeightStep,
            WaterLevel: snapshot.WaterLevel);
        foreach (var d in MapDesignValidator.Validate(designRequest))
        {
            issues.Add(new MapIssue(
                d.Severity == DesignIssueSeverity.Error ? MapIssueSeverity.Error : MapIssueSeverity.Warning,
                d.Code, d.Chinese, d.English, d.ObjectId, -1, d.WorldX, d.WorldZ));
        }
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

    private static bool IsRetainedClosedFamily(LevelObjectStore? store, ScenarioDocument previous, ScenarioSpawn spawn, ScenarioDataSlot binding)
    {
        if (store is null || binding.SpawnId == Guid.Empty) return false;
        var old = previous.Spawns.FirstOrDefault(value => value.Id == binding.SpawnId && value.Prebuilt);
        if (old is null || !old.Alias.Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase)) return false;
        if (store.UidAt(binding.Slot) != binding.Uid) return false;
        var root = store.Objects().FirstOrDefault(obj => obj.Slot == binding.Slot);
        if (root is null || root.Uid != binding.Uid) return false;
        var family = store.FamilyAt(binding.Slot, binding.Uid);
        if (family is null) return false;
        if (old.Team != spawn.Team || root.Team != spawn.Team)
            return false;

        if (old.RawRotation is { } oldRaw || spawn.RawRotation is { } spawnRaw)
        {
            if (old.RawRotation is not { } oRaw || spawn.RawRotation is not { } sRaw
                || !float.IsFinite(oRaw) || !float.IsFinite(sRaw)
                || BitConverter.SingleToInt32Bits(oRaw) != BitConverter.SingleToInt32Bits(sRaw)
                || BitConverter.SingleToInt32Bits(root.Rotation) != BitConverter.SingleToInt32Bits(sRaw)
                || ScenarioLevelObjects.NormalizeAngle(old.Angle) != ScenarioLevelObjects.NormalizeAngle(spawn.Angle))
                return false;
        }
        else
        {
            double actualAngle = root.Rotation * (180d / Math.PI);
            double desiredAngle = ((spawn.Angle % 360d) + 360d) % 360d;
            double angleDifference = Math.Abs(((actualAngle - desiredAngle + 540d) % 360d + 360d) % 360d - 180d);
            double oldAngle = ((old.Angle % 360d) + 360d) % 360d;
            if (oldAngle != desiredAngle || !double.IsFinite(actualAngle) || angleDifference > .001d)
                return false;
        }
        return true;
    }
}
