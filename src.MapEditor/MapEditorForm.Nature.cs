using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeMapEditor.Modules.Nature;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripButton _natureTool = new("自然物件") { CheckOnClick = true };
    private readonly ComboBox _natureCategory = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ComboBox _natureOperation = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ListBox _natureTypes = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly Label _natureHint = new() { Dock = DockStyle.Top, Height = 70, Padding = new Padding(4, 6, 4, 4), ForeColor = WinFormsTheme.TextSecondary };
    private Label _lblNatureCategory = null!, _lblNatureOperation = null!;
    private IReadOnlyList<LevelWorldObject> _levelObjects = Array.Empty<LevelWorldObject>();
    private IReadOnlyDictionary<int, string> _objdefNames = new Dictionary<int, string>();
    private readonly NatureEditSession _natureSession = new();
    private IReadOnlySet<int> _natureRemovals => _natureSession.RemovedSlots;
    private IReadOnlyList<NatureAddition> _natureAdditions => _natureSession.Additions;
    private bool _natureStoreAvailable;
    private Task<IReadOnlyDictionary<int, LevelObjectTemplate>>? _natureCatalogTask;
    private IReadOnlyDictionary<int, LevelObjectTemplate> _natureTemplates = new Dictionary<int, LevelObjectTemplate>();
    private readonly Random _natureRandom = new();
    private bool NatureDirty() => _natureSession.IsDirty;
    private Control BuildNaturePanel()
    {
        var panel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = WinFormsTheme.Surface };
        var options = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 2, BackColor = WinFormsTheme.SurfaceRaised, Padding = new Padding(6) };
        options.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70)); options.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        _lblNatureOperation = AddSceneField(options, 0, "操作", _natureOperation);
        _lblNatureCategory = AddSceneField(options, 1, "類別", _natureCategory);
        panel.Controls.Add(_natureTypes); panel.Controls.Add(options); panel.Controls.Add(_natureHint);
        FitWrappedLabelHeight(_natureHint);
        return panel;
    }

    private void LocalizeNatureTab(bool isEn)
    {
        if (_lblNatureCategory is null) return;
        _lblNatureCategory.Text = isEn ? "Category" : "類別"; _lblNatureOperation.Text = isEn ? "Action" : "操作";
        int operation = Math.Max(0, _natureOperation.SelectedIndex);
        _natureOperation.Items.Clear();
        _natureOperation.Items.AddRange(isEn ? new object[] { "Plant", "Remove" } : new object[] { "種植", "移除" });
        _natureOperation.SelectedIndex = operation;
        _natureHint.Text = isEn
            ? "Drag on the map with \"Nature\" active. Plant uses the brush size (one object per tile); Remove clears trees, grass and bushes under the brush. Script markers and linked objects are never touched."
            : "啟用「自然物件」後在地圖拖曳。種植：每格一株；移除：清除筆刷範圍內的樹木、草叢、灌木。腳本標記與連結物件不會被更動。";
        PopulateNatureCategories();
    }

    private static string NatureCategory(string name)
    {
        string code = name.Length >= 9 ? name.Substring(6, 3) : "";
        return code switch
        {
            "Nad" or "Lau" or "Bau" or "Pal" or "Bir" or "Eic" or "Buc" or "Kie" or "Tan" or "Fic" or "Obs" or "Zyp" or "Pin" => "tree",
            "Gra" or "Bli" or "Blu" => "grass",
            "Dor" or "Bod" or "Bus" or "Str" or "Ger" or "Hec" => "bush",
            "Sch" or "Ent" or "Wei" or "Ser" => "water",
            "Ste" or "Fel" or "Sto" or "Kie" => "rock",
            _ => "other",
        };
    }

    private static string NatureCategoryText(string key, bool isEn) => key switch
    {
        "tree" => isEn ? "Trees" : "樹木", "grass" => isEn ? "Grass & flowers" : "草叢與花", "bush" => isEn ? "Bushes" : "灌木",
        "water" => isEn ? "Reeds & water plants" : "蘆葦與水生植物", "rock" => isEn ? "Rocks" : "岩石", "all" => isEn ? "All" : "全部",
        _ => isEn ? "Other landscape" : "其他地景",
    };

    private void PopulateNatureCategories()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        int selected = Math.Max(0, _natureCategory.SelectedIndex);
        _natureCategory.Items.Clear();
        _natureCategory.Items.Add(new FilterItem("all", NatureCategoryText("all", isEn)));
        foreach (string key in new[] { "tree", "grass", "bush", "water", "rock", "other" })
            if (_natureTemplates.Keys.Any(id => _objdefNames.TryGetValue(id, out string? name) && NatureCategory(name) == key))
                _natureCategory.Items.Add(new FilterItem(key, NatureCategoryText(key, isEn)));
        _natureCategory.SelectedIndex = Math.Min(selected, _natureCategory.Items.Count - 1);
        RefreshNatureTypes();
    }

    private sealed record NatureTypeItem(LevelObjectTemplate Template, string Name) { public override string ToString() => Name; }

    private void RefreshNatureTypes()
    {
        string key = (_natureCategory.SelectedItem as FilterItem)?.Value as string ?? "all";
        int? selected = (_natureTypes.SelectedItem as NatureTypeItem)?.Template.TypeId;
        _natureTypes.BeginUpdate(); _natureTypes.Items.Clear();
        foreach ((int id, LevelObjectTemplate template) in _natureTemplates.OrderBy(pair => _objdefNames.GetValueOrDefault(pair.Key), StringComparer.OrdinalIgnoreCase))
        {
            string name = _objdefNames.GetValueOrDefault(id) ?? id.ToString(System.Globalization.CultureInfo.InvariantCulture);
            if (key != "all" && NatureCategory(name) != key) continue;
            _natureTypes.Items.Add(new NatureTypeItem(template, name));
        }
        _natureTypes.EndUpdate();
        for (int index = 0; index < _natureTypes.Items.Count; index++)
            if (((NatureTypeItem)_natureTypes.Items[index]).Template.TypeId == selected) { _natureTypes.SelectedIndex = index; break; }
        if (_natureTypes.SelectedIndex < 0 && _natureTypes.Items.Count > 0) _natureTypes.SelectedIndex = 0;
    }

    /// <summary>從所有原版地圖收集可複製的地景物件範本（背景執行一次）。</summary>
    private void EnsureNatureCatalog()
    {
        if (_natureCatalogTask is not null) return;
        string gamePath = _gamePath;
        IReadOnlyDictionary<int, string> names = _objdefNames;
        _natureCatalogTask = Task.Run(() => LevelObjectStore.LoadOfficialTemplates(gamePath,
            id => names.TryGetValue(id, out string? name) && ObjDefNames.IsLandscape(name)));
        _natureCatalogTask.ContinueWith(task =>
        {
            if (task.IsCompletedSuccessfully && !IsDisposed)
                BeginInvoke(() => { _natureTemplates = task.Result; PopulateNatureCategories(); });
        }, TaskScheduler.Default);
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _natureTypes.Items.Clear(); _natureTypes.Items.Add(isEn ? "Loading landscape catalog from original maps…" : "正在從原版地圖讀取地景目錄…");
    }

    private void LoadLevelObjects(string map)
    {
        _natureSession.Clear();
        _natureStoreAvailable = false;
        _levelObjects = Array.Empty<LevelWorldObject>();
        try
        {
            if (!File.Exists(Path.Combine(map, "DATA", "objects.dat"))) return;
            _levelObjects = LevelObjectStore.Load(map).Objects();
            _natureStoreAvailable = true;
        }
        catch (Exception ex) when (ex is InvalidDataException or IOException) { }
    }

    private bool IsRemovableNature(LevelWorldObject item) => !item.Linked && ObjDefNames.IsLandscape(_objdefNames.GetValueOrDefault(item.TypeId));

    private IEnumerable<MapSceneObject> NatureDisplayObjects()
    {
        IReadOnlySet<int> removals = _natureSession.RemovedSlots;
        IReadOnlyList<NatureAddition> additions = _natureSession.Additions;
        foreach (LevelWorldObject item in _levelObjects)
            if (!removals.Contains(item.Slot) && ObjDefNames.IsLandscape(_objdefNames.GetValueOrDefault(item.TypeId)))
                yield return new MapSceneObject(_objdefNames.GetValueOrDefault(item.TypeId) ?? "", item.X, item.Y, item.Z, 8, "DATA/objects.dat", -100000 - item.Slot);
        for (int index = 0; index < additions.Count; index++)
        {
            NatureAddition addition = additions[index];
            yield return new MapSceneObject(addition.Name, addition.X, addition.Y, addition.Z, 8, "DATA/objects.dat", -200000 - index);
        }
    }

    private void RefreshSceneMarkers()
    {
        IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
        _canvas.UpdateSceneObjects(effective); _view3d?.UpdateSceneObjects(effective);
    }

    private void PaintNature(TexturePaintEventArgs e)
    {
        if (_selected?.IsCustom != true || _texturesDocument is null) return;
        // 與地形筆刷相同：補齊快速拖曳時跳過的格子。
        IEnumerable<(int X, int Y)> tiles = _lastTerrainTile is { } last ? TerrainStrokePath.Between(last.X, last.Y, e.X, e.Y) : new[] { (e.X, e.Y) };
        _lastTerrainTile = (e.X, e.Y);
        bool changed = false;
        foreach ((int x, int y) in tiles)
            if (_terrainStrokeTiles.Add(y * _texturesDocument.Dimension + x)) changed |= PaintNatureTile(x, y);
        if (!changed) return;
        RefreshSceneMarkers();
        UpdateEditorState();
    }

    private bool PaintNatureTile(int tileX, int tileY)
    {
        var e = (X: tileX, Y: tileY);
        const float tileWorld = SdlSceneCatalog.WorldUnitsPerMapPixel * 4f;
        float radiusTiles = _canvas.BrushSize / 2f + .26f;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_natureOperation.SelectedIndex == 1)
        {
            float cx = (e.X + .5f) * tileWorld, cz = (e.Y + .5f) * tileWorld, radius = radiusTiles * tileWorld;
            IReadOnlySet<int> removals = _natureSession.RemovedSlots;
            List<int> removed = _levelObjects.Where(item => IsRemovableNature(item) && !removals.Contains(item.Slot)
                && (item.X - cx) * (item.X - cx) + (item.Z - cz) * (item.Z - cz) <= radius * radius).Select(item => item.Slot).ToList();
            List<NatureAddition> removedAdditions = _natureAdditions.Where(item => (item.X - cx) * (item.X - cx) + (item.Z - cz) * (item.Z - cz) <= radius * radius).ToList();
            if (removed.Count == 0 && removedAdditions.Count == 0) return false;
            _natureSession.Remove(removed, removedAdditions);
        }
        else
        {
            if (_natureTypes.SelectedItem is not NatureTypeItem type)
            {
                _status.Text = isEn ? "Pick a landscape type in the Nature tab first (the catalog may still be loading)." : "請先在「自然物件」分頁選擇類型（目錄可能仍在載入）。";
                return false;
            }
            // 每格一株，於格內隨機偏移與旋轉，避免整齊排列。
            float x = (e.X + .2f + (float)_natureRandom.NextDouble() * .6f) * tileWorld, z = (e.Y + .2f + (float)_natureRandom.NextDouble() * .6f) * tileWorld;
            float y = 0;
            if (_terrainLayers is not null)
            {
                int step = (_terrainLayers.VertexSize - 1) / _texturesDocument!.Dimension;
                int vx = Math.Clamp((int)MathF.Round(x / tileWorld * step), 0, _terrainLayers.VertexSize - 1), vy = Math.Clamp((int)MathF.Round(z / tileWorld * step), 0, _terrainLayers.VertexSize - 1);
                y = _terrainLayers.Heights[vy * _terrainLayers.VertexSize + vx] * _heightMapStep;
            }
            var addition = new NatureAddition(type.Template, type.Name, x, y, z, (float)(_natureRandom.NextDouble() * Math.PI * 2));
            _natureSession.Plant(addition);
        }
        return true;
    }

}
