using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeMapEditor.Modules.Settlement;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripDropDownButton _layoutMenu = new("配置");
    private readonly ToolStripMenuItem _generateSettlement = new("一鍵生成對戰基地…");
    private readonly ToolStripMenuItem _exportPlacementLayout = new("保存選取的聚落／物件…");
    private readonly ToolStripMenuItem _exportNatureLayout = new("保存森林區域…");
    private readonly ToolStripMenuItem _importLayout = new("載入並套用配置…");
    private IReadOnlyDictionary<int, LevelObjectTemplate>? _layoutNativeTemplates;
    internal Func<LayoutApplyDialog, DialogResult> LayoutDialogRunner { get; set; } = dialog => dialog.ShowDialog();
    internal Func<SettlementGeneratorDialog, DialogResult> SettlementGeneratorDialogRunner { get; set; } = dialog => dialog.ShowDialog();

    private void InitializeLayoutTools()
    {
        _layoutMenu.DropDownItems.AddRange([_generateSettlement, _campaignWaves, new ToolStripSeparator(), _exportPlacementLayout, _exportNatureLayout, _importLayout]);
        _campaignWaves.Click += (_, _) => WithLayoutErrors(RunCampaignWaves);
        _generateSettlement.Click += (_, _) => WithLayoutErrors(RunSettlementGenerator);
        _exportPlacementLayout.Click += (_, _) => WithLayoutErrors(() => SaveLayoutFile(CapturePlacementLayout(
            _placedList.SelectedItems.Cast<ListViewItem>().Select(row => (int)row.Tag!).ToArray())));
        _exportNatureLayout.Click += (_, _) => WithLayoutErrors(() =>
        {
            if (_texturesDocument is null) return;
            using var dialog = new TerrainRegionDialog(_texturesDocument.Dimension);
            dialog.UseRectangleInput(Loc.CurrentLanguage == Language.English ? "Capture forest region" : "保存森林區域");
            if (RegionDialogRunner(dialog) != DialogResult.OK) return;
            var vertices = dialog.ReadVertices();
            var cells = TerrainRegionPlanner.Rectangle(_texturesDocument.Dimension, vertices[0], vertices[1]);
            SaveLayoutFile(CaptureNatureLayout(Rectangle.FromLTRB(cells.Min(c => c.X), cells.Min(c => c.Y), cells.Max(c => c.X) + 1, cells.Max(c => c.Y) + 1)));
        });
        _importLayout.Click += (_, _) => WithLayoutErrors(() =>
        {
            using var file = new OpenFileDialog { Filter = "Map layout (*.arm-layout.json)|*.arm-layout.json|JSON (*.json)|*.json" };
            if (file.ShowDialog(this) != DialogResult.OK) return;
            if (new FileInfo(file.FileName).Length > 2_000_000) throw new InvalidDataException("Layout is too large.");
            var preset = MapLayoutPresets.Deserialize(File.ReadAllText(file.FileName));
            RunLayoutApply(preset);
        });
    }

    private void WithLayoutErrors(Action action)
    {
        try { action(); }
        catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException or ArgumentException or System.Text.Json.JsonException)
        { _status.Text = (Loc.CurrentLanguage == Language.English ? "Layout failed: " : "配置操作失敗：") + ex.Message; }
    }

    internal void RunSettlementGenerator()
    {
        if (_selected?.IsCustom != true) return;
        using var dialog = new SettlementGeneratorDialog();
        if (SettlementGeneratorDialogRunner(dialog) != DialogResult.OK) return;
        ApplySettlementGeneration(dialog.PlayerCount, dialog.SelectedTribe, dialog.Seed, dialog.IncludeNature);
    }

    internal void ApplySettlementGeneration(int playerCount, SettlementTribe tribe, int seed, bool includeNature = true)
    {
        if (_selected?.IsCustom != true) throw new InvalidOperationException("Select a custom map.");
        bool isEn = Loc.CurrentLanguage == Language.English;

        int dimension = 256;
        float tileWorldSize = 64f;

        var evaluator = new SettlementSiteEvaluator(
            dimension: dimension,
            sampleHeight: (wx, wz) => LayoutGroundHeight(wx, wz),
            isBlocked: (tx, tz) => _terrainLayers?.Collision is { } c && tx >= 0 && tz >= 0 && tx < 256 && tz < 256 && c[tz * 256 + tx] > 128,
            waterLevel: (float)_waterLevel.Value,
            tileWorldSize: tileWorldSize);

        var planner = new ResourceClusterPlanner(
            dimension: dimension,
            sampleHeight: (wx, wz) => LayoutGroundHeight(wx, wz),
            isBlocked: (tx, tz) => _terrainLayers?.Collision is { } c && tx >= 0 && tz >= 0 && tx < 256 && tz < 256 && c[tz * 256 + tx] > 128,
            waterLevel: (float)_waterLevel.Value,
            tileWorldSize: tileWorldSize);

        var balancer = new MultiplayerFairnessBalancer(evaluator, planner, dimension, tileWorldSize);
        var tribes = Enumerable.Repeat(tribe, playerCount).ToArray();
        var mode = playerCount == 2 ? SymmetryMode.CentralSymmetry : SymmetryMode.RotationalSymmetry;

        MultiplayerDistributionResult distribution = balancer.Generate(playerCount, mode, tribes, baseSeed: seed);

        CommitStroke();

        var placedIndices = SettlementGeneratorEngine.ApplyToPlacementSession(
            distribution,
            _objectCatalog,
            _placementSession,
            (wx, wz) => LayoutGroundHeight(wx, wz));

        bool natureApplied = false;
        if (includeNature && _natureStoreAvailable)
        {
            var natureTemplates = NatureLayoutTemplates();
            if (natureTemplates.Count > 0)
            {
                natureApplied = SettlementGeneratorEngine.ApplyToNatureSession(
                    distribution,
                    natureTemplates,
                    _natureSession,
                    (wx, wz) => LayoutGroundHeight(wx, wz));
            }
        }

        _lastActionWasSettlementGeneration = true;
        _lastActionWasSettlementGenerationUndone = false;

        SetEditMode(EditMode.PlaceObject);
        RefreshPlacedList();
        RefreshSceneMarkers();
        UpdateEditorState();

        int totalResources = distribution.Players.Sum(p => p.ForestTrees.Count + p.StoneQuarries.Count);
        _status.Text = isEn
            ? $"Generated {playerCount}-player settlement: {placedIndices.Count} placed objects" + (natureApplied ? $", {totalResources} nature resources" : "") + " (single Undo available)"
            : $"已生成 {playerCount} 人基地：{placedIndices.Count} 個放置物件" + (natureApplied ? $"、{totalResources} 個自然資源" : "") + "（可單步復原）";
    }

    internal void RunLayoutApply(MapLayoutPreset preset)
    {
        using var dialog = new LayoutApplyDialog(preset);
        if (LayoutDialogRunner(dialog) == DialogResult.OK) ApplyLayout(preset, dialog.AnchorX, dialog.AnchorZ, dialog.Rotation, dialog.Team);
    }

    private void SaveLayoutFile(MapLayoutPreset preset)
    {
        string json = MapLayoutPresets.Serialize(preset);
        using var file = new SaveFileDialog { Filter = "Map layout (*.arm-layout.json)|*.arm-layout.json", DefaultExt = "arm-layout.json", AddExtension = true, FileName = preset.Kind == MapLayoutKind.Nature ? "forest.arm-layout.json" : "settlement.arm-layout.json" };
        if (file.ShowDialog(this) != DialogResult.OK) return;
        File.WriteAllText(file.FileName, json);
        _status.Text = Loc.CurrentLanguage == Language.English ? $"Saved {preset.Entries.Count} layout objects." : $"已保存配置，共 {preset.Entries.Count} 個物件。";
    }

    internal MapLayoutPreset CapturePlacementLayout(IReadOnlyList<int> indices)
    {
        var objects = indices.Distinct().Select(index => _placementSession[index]).ToArray();
        if (objects.Length == 0) throw new InvalidDataException("Select placed objects first.");
        float x = objects.Min(item => item.WorldX), z = objects.Min(item => item.WorldZ);
        var preset = new MapLayoutPreset(1, MapLayoutKind.Placement, objects.Select(item => new MapLayoutEntry(AliasOf(item.Type),
            item.WorldX - x, item.WorldZ - z, item.WorldY - LayoutGroundHeight(item.WorldX, item.WorldZ), item.Angle, item.Team, item.UnitCount)).ToArray());
        MapLayoutPresets.Validate(preset); return preset;
    }

    internal MapLayoutPreset CaptureNatureLayout(Rectangle tiles)
    {
        if (_texturesDocument is null || tiles.Width <= 0 || tiles.Height <= 0 || tiles.Left < 0 || tiles.Top < 0 ||
            tiles.Right > _texturesDocument.Dimension || tiles.Bottom > _texturesDocument.Dimension) throw new ArgumentOutOfRangeException(nameof(tiles));
        float x = tiles.Left * 256f, z = tiles.Top * 256f;
        bool Contains(float px, float pz) => px >= x && pz >= z && px < tiles.Right * 256f && pz < tiles.Bottom * 256f;
        var entries = _levelObjects.Where(item => IsRemovableNature(item) && !_natureRemovals.Contains(item.Slot) && Contains(item.X, item.Z))
            .Select(item => new MapLayoutEntry(_objdefNames[item.TypeId], item.X - x, item.Z - z,
                item.Y - LayoutGroundHeight(item.X, item.Z), item.Rotation * 180 / MathF.PI))
            .Concat(_natureAdditions.Where(item => Contains(item.X, item.Z)).Select(item => new MapLayoutEntry(item.Name, item.X - x, item.Z - z,
                item.Y - LayoutGroundHeight(item.X, item.Z), item.Rotation * 180 / MathF.PI))).ToArray();
        var preset = new MapLayoutPreset(1, MapLayoutKind.Nature, entries); MapLayoutPresets.Validate(preset); return preset;
    }

    private float LayoutGroundHeight(float worldX, float worldZ)
    {
        if (_terrainLayers is null || _texturesDocument is null) return 0;
        float step = (_terrainLayers.VertexSize - 1) / (_texturesDocument.Dimension * 256f);
        int x = Math.Clamp((int)MathF.Round(worldX * step), 0, _terrainLayers.VertexSize - 1), z = Math.Clamp((int)MathF.Round(worldZ * step), 0, _terrainLayers.VertexSize - 1);
        return _terrainLayers.Heights[z * _terrainLayers.VertexSize + x] * _heightMapStep;
    }

    private IReadOnlyDictionary<int, LevelObjectTemplate> CurrentLayoutTemplates()
        => _layoutNativeTemplates ??= _natureStoreAvailable && _selected is not null
            ? LevelObjectStore.Load(_selected.DirectoryPath).Templates().ToDictionary(template => template.TypeId)
            : new Dictionary<int, LevelObjectTemplate>();

    private Dictionary<string, LevelObjectTemplate> NatureLayoutTemplates()
        => CurrentLayoutTemplates().Values.Concat(_natureTemplates.Values).Where(template => ObjDefNames.IsLandscape(_objdefNames.GetValueOrDefault(template.TypeId)))
            .GroupBy(template => _objdefNames[template.TypeId], StringComparer.OrdinalIgnoreCase).ToDictionary(group => group.Key, group => group.First(), StringComparer.OrdinalIgnoreCase);

    internal void ApplyLayout(MapLayoutPreset preset, float worldX, float worldZ, float rotation = 0, int? team = null)
    {
        if (_selected?.IsCustom != true) throw new InvalidOperationException("Select a custom map.");
        MapLayoutPresets.Validate(preset);
        if (preset.Kind == MapLayoutKind.Placement)
        {
            var plan = MapLayoutPresets.PlanPlacements(preset, _objectCatalog, worldX, worldZ, rotation, LayoutGroundHeight, team);
            _placementSession.AddMany(plan); SetEditMode(EditMode.PlaceObject); RefreshPlacedList();
            var ids = plan.Select(item => item.ScenarioId).ToHashSet();
            foreach (ListViewItem row in _placedList.Items) row.Selected = ids.Contains(_placementSession[(int)row.Tag!].ScenarioId);
        }
        else
        {
            if (!_natureStoreAvailable) throw new InvalidDataException("This map has no editable native nature data.");
            var templates = NatureLayoutTemplates();
            if (preset.Entries.Any(entry => !templates.ContainsKey(entry.Type)))
            {
                // Import can be used before opening Nature; resolve other species from the same local catalog.
                _natureTemplates = _natureCatalogTask?.GetAwaiter().GetResult() ?? LevelObjectStore.LoadOfficialTemplates(_gamePath,
                    id => ObjDefNames.IsLandscape(_objdefNames.GetValueOrDefault(id)));
                _natureCatalogTask ??= Task.FromResult(_natureTemplates);
                PopulateNatureCategories(); templates = NatureLayoutTemplates();
            }
            var plan = MapLayoutPresets.PlanNature(preset, templates, worldX, worldZ, rotation, LayoutGroundHeight);
            _natureSession.PlantMany(plan); SetEditMode(EditMode.Nature);
        }
        RefreshSceneMarkers(); UpdateEditorState(); RefreshMapDiagnostics();
    }
}
