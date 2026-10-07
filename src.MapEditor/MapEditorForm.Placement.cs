using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private bool PlacedDirty() => !_placedObjects.SequenceEqual(_placedBaseline);

    private Control BuildPlacementPanel()
    {
        var panel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = WinFormsTheme.Surface };
        var options = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 2, BackColor = WinFormsTheme.SurfaceRaised, Padding = new Padding(6) };
        options.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70)); options.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        _lblPlaceCategory = AddSceneField(options, 0, "類別", _placeCategory);
        _lblPlaceTribe = AddSceneField(options, 1, "部族", _placeTribe);
        _lblPlaceTeam = AddSceneField(options, 2, "隊伍", _placeTeam);
        _lblPlaceCount = AddSceneField(options, 3, "人數", _placeCount);
        _lblPlaceAngle = AddSceneField(options, 4, "角度", _placeAngle);
        var placedHost = new Panel { Dock = DockStyle.Bottom, Height = 230, Padding = new Padding(0, 8, 0, 0) };
        _placedList.Columns.Add("物件", 150); _placedList.Columns.Add("隊伍", 50); _placedList.Columns.Add("位置", 90);
        placedHost.Controls.Add(_placedList); placedHost.Controls.Add(_placedDeleteButton);
        panel.Controls.Add(_placeTypes); panel.Controls.Add(placedHost); panel.Controls.Add(options); panel.Controls.Add(_placeHint);
        WinFormsTheme.StyleDangerButton(_placedDeleteButton);
        return panel;
    }

    /// <summary>放置目錄：cl_scint.ini [ObjDefName] 的別名（建築、人物…），TemplateFields["alias"] 保存別名。</summary>
    private static IReadOnlyList<SdlObjectType> BuildSpawnCatalog(string gamePath)
        => ScriptObjectAliases.Load(gamePath)
            .Select(alias => new SdlObjectType(alias.NameDef, -1, alias.Category, alias.Tribe, 0, new Dictionary<string, string> { ["alias"] = alias.Alias }))
            .OrderBy(type => type.Category).ThenBy(type => type.Tribe).ThenBy(type => type.NameDef, StringComparer.OrdinalIgnoreCase)
            .ToArray();

    private static string AliasOf(SdlObjectType type) => type.TemplateFields.TryGetValue("alias", out string? alias) ? alias : type.NameDef;

    /// <summary>以原版地圖 DATA 中同類型的完工建築作為範本（首次需要時同步讀取）。</summary>
    private LevelObjectTemplate? BuildingTemplateFor(ScenarioSpawn spawn)
    {
        SdlObjectType? type = _objectCatalog.FirstOrDefault(item => AliasOf(item).Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase));
        if (type is null) return null;
        int typeId = _objdefNames.FirstOrDefault(pair => pair.Value.Equals(type.NameDef, StringComparison.OrdinalIgnoreCase), new KeyValuePair<int, string>(-1, "")).Key;
        if (typeId < 0) return null;
        IReadOnlyDictionary<int, string> names = _objdefNames;
        _buildingTemplates ??= LevelObjectStore.LoadOfficialTemplates(_gamePath, id => names.TryGetValue(id, out string? name) && name.StartsWith("Bau", StringComparison.OrdinalIgnoreCase));
        return _buildingTemplates.GetValueOrDefault(typeId);
    }

    private IEnumerable<SdlPlacedObject> LoadScenarioPlacements(string map)
    {
        ScenarioDocument scenario;
        try { scenario = ScenarioDocument.Load(map); }
        catch (System.Text.Json.JsonException) { yield break; }
        foreach (ScenarioSpawn spawn in scenario.Spawns)
        {
            SdlObjectType? type = _objectCatalog.FirstOrDefault(item => AliasOf(item).Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase));
            if (type is null) continue;
            yield return new SdlPlacedObject(type, spawn.X, spawn.Y, spawn.Z, spawn.Team, spawn.Angle, spawn.Count > 1 ? spawn.Count : 1) { ScenarioId = spawn.Id };
        }
    }

    private void LocalizePlacementTab(bool isEn)
    {
        if (_lblPlaceCategory is null) return;
        _lblPlaceCategory.Text = isEn ? "Category" : "類別"; _lblPlaceTribe.Text = isEn ? "Tribe" : "部族"; _lblPlaceTeam.Text = isEn ? "Team" : "隊伍";
        _lblPlaceCount.Text = isEn ? "Soldiers" : "人數"; _lblPlaceAngle.Text = isEn ? "Angle" : "角度";
        _placedDeleteButton.Text = isEn ? "Delete Selected" : "刪除選取的物件";
        _placeHint.Text = isEn
            ? "Pick a type, then click the map with \"Place Objects\" active. Buildings are written into the map as finished buildings (team 8 = neutral); characters and units are created by the map script when the game starts. Team 0 is the human player; characters with a count above 1 spawn as a unit. Keep buildings clear of trees and water."
            : "選類型後啟用「放置物件」並點擊地圖。建築會以完工狀態寫入地圖（隊伍 8＝中立）；人物與部隊在開局時由地圖腳本建立。隊伍 0 為玩家；人物的人數大於 1 時會生成一支部隊。建築請避開樹木與水域。";
        if (_placedList.Columns.Count >= 3)
        {
            _placedList.Columns[0].Text = isEn ? "Object" : "物件"; _placedList.Columns[1].Text = isEn ? "Team" : "隊伍"; _placedList.Columns[2].Text = isEn ? "Tile" : "位置";
        }
        PopulatePlacementFilters();
        RefreshPlacedList();
    }

    private void PopulatePlacementFilters()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        int category = Math.Max(0, _placeCategory.SelectedIndex), tribe = Math.Max(0, _placeTribe.SelectedIndex);
        _placeCategory.Items.Clear();
        _placeCategory.Items.Add(new FilterItem(null, isEn ? "All" : "全部"));
        foreach (SdlObjectCategory value in Enum.GetValues<SdlObjectCategory>())
            if (_objectCatalog.Any(type => type.Category == value)) _placeCategory.Items.Add(new FilterItem(value, CategoryText(value, isEn)));
        _placeTribe.Items.Clear();
        _placeTribe.Items.Add(new FilterItem(null, isEn ? "All" : "全部"));
        foreach (string code in new[] { "Ger", "Hun", "Kel", "Rom", "" })
            if (_objectCatalog.Any(type => type.Tribe == code)) _placeTribe.Items.Add(new FilterItem(code, TribeText(code, isEn)));
        _placeCategory.SelectedIndex = Math.Min(category, _placeCategory.Items.Count - 1);
        _placeTribe.SelectedIndex = Math.Min(tribe, _placeTribe.Items.Count - 1);
        RefreshPlacementTypes();
    }

    private void RefreshPlacementTypes()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        object? category = (_placeCategory.SelectedItem as FilterItem)?.Value, tribe = (_placeTribe.SelectedItem as FilterItem)?.Value;
        string? selected = (_placeTypes.SelectedItem as PlacementTypeItem)?.Type.NameDef;
        _placeTypes.BeginUpdate(); _placeTypes.Items.Clear();
        foreach (SdlObjectType type in _objectCatalog.Where(type => (category is null || type.Category.Equals(category)) && (tribe is null || type.Tribe.Equals(tribe))))
            _placeTypes.Items.Add(new PlacementTypeItem(type, ObjectDisplayName(type, isEn)));
        _placeTypes.EndUpdate();
        for (int index = 0; index < _placeTypes.Items.Count; index++)
            if (((PlacementTypeItem)_placeTypes.Items[index]).Type.NameDef == selected) { _placeTypes.SelectedIndex = index; break; }
        if (_placeTypes.SelectedIndex < 0 && _placeTypes.Items.Count > 0) _placeTypes.SelectedIndex = 0;
    }

    private void RefreshPlacedList()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _placedList.BeginUpdate(); _placedList.Items.Clear();
        for (int index = 0; index < _placedObjects.Count; index++)
        {
            SdlPlacedObject item = _placedObjects[index];
            var row = new ListViewItem(ObjectDisplayName(item.Type, isEn) + (item.UnitCount > 1 ? $" ×{item.UnitCount}" : "")) { Tag = index };
            row.SubItems.Add(item.Team.ToString(System.Globalization.CultureInfo.InvariantCulture));
            row.SubItems.Add($"{(int)(item.WorldX / 256)},{(int)(item.WorldZ / 256)}");
            _placedList.Items.Add(row);
        }
        _placedList.EndUpdate();
        _placedDeleteButton.Enabled = false;
    }

    /// <summary>在點擊的 tile 中心放置一個物件（每次按下只放一個）；高度取自目前地形，世界座標為絕對值。</summary>
    private void PlaceObjectAt(TexturePaintEventArgs e)
    {
        if (_selected?.IsCustom != true || _lastTerrainTile is not null) return; // 同一次按下（拖曳）只放一個
        _lastTerrainTile = (e.X, e.Y);
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_placeTypes.SelectedItem is not PlacementTypeItem selection)
        {
            _status.Text = isEn ? "Pick an object type in the Place Objects tab first." : "請先在「放置物件」分頁選擇物件類型。";
            return;
        }
        const float tileWorld = SdlSceneCatalog.WorldUnitsPerMapPixel * 4f;
        float worldX = (e.X + .5f) * tileWorld, worldZ = (e.Y + .5f) * tileWorld, worldY = 0;
        if (_terrainLayers is not null)
        {
            int step = (_terrainLayers.VertexSize - 1) / 64, vx = Math.Clamp(e.X * step + step / 2, 0, _terrainLayers.VertexSize - 1), vy = Math.Clamp(e.Y * step + step / 2, 0, _terrainLayers.VertexSize - 1);
            worldY = _terrainLayers.Heights[vy * _terrainLayers.VertexSize + vx] * _heightMapStep; // 遊戲高度 = boden.bmp 綠通道 × Heightmapstep
        }
        bool figure = selection.Type.Category == SdlObjectCategory.Figure;
        if (figure && _placeTeam.Value is < 0 or > 7)
        {
            _status.Text = isEn ? "Units need a team between 0 and 7." : "人物與部隊的隊伍必須介於 0 與 7。";
            return;
        }
        _placedObjects.Add(new SdlPlacedObject(selection.Type, worldX, worldY, worldZ, (int)_placeTeam.Value, (float)_placeAngle.Value,
            figure ? (int)_placeCount.Value : 0) { ScenarioId = Guid.NewGuid() });
        RefreshPlacedList();
        IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
        _canvas.UpdateSceneObjects(effective); _view3d?.UpdateSceneObjects(effective);
        UpdateEditorState();
        _status.Text = isEn ? $"Placed {selection.Text} at tile ({e.X},{e.Y}); Save to write it." : $"已在格子 ({e.X},{e.Y}) 放置 {selection.Text}；按「儲存」才會寫入。";
        // 建築與樹木／草叢重疊時，以腳本生成會被遊戲的放置檢查拒絕；寫入 DATA 則會穿模。
        const float clearance = 384;
        if (selection.Type.Category == SdlObjectCategory.Building && NatureDisplayObjects().Any(item => MathF.Abs(item.WorldX - worldX) < clearance && MathF.Abs(item.WorldZ - worldZ) < clearance))
            _status.Text += isEn ? " Warning: trees or other landscape objects are very close; clear them with the Nature tool." : " 注意：附近有樹木或其他地景物件，建議用「自然物件 → 移除」清出空地。";
    }

    private void DeleteSelectedPlacedObjects()
    {
        if (_selected?.IsCustom != true || _placedList.SelectedItems.Count == 0) return;
        foreach (int index in _placedList.SelectedItems.Cast<ListViewItem>().Select(row => (int)row.Tag!).OrderByDescending(index => index)) _placedObjects.RemoveAt(index);
        RefreshPlacedList();
        IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
        _canvas.UpdateSceneObjects(effective); _view3d?.UpdateSceneObjects(effective);
        UpdateEditorState();
    }

    private static string CategoryText(SdlObjectCategory category, bool isEn) => category switch
    {
        SdlObjectCategory.Building => isEn ? "Buildings" : "建築",
        SdlObjectCategory.UnitGroup => isEn ? "Unit groups" : "部隊",
        SdlObjectCategory.Figure => isEn ? "Characters" : "人物",
        SdlObjectCategory.Effect => isEn ? "Effects" : "特效",
        _ => isEn ? "Other" : "其他",
    };

    private static string TribeText(string code, bool isEn) => code switch
    {
        "Ger" => isEn ? "Germans" : "日耳曼",
        "Hun" => isEn ? "Huns" : "匈人",
        "Kel" => isEn ? "Celts" : "凱爾特",
        "Rom" => isEn ? "Romans" : "羅馬",
        _ => isEn ? "Common" : "通用",
    };

    private static string ObjectDisplayName(SdlObjectType type, bool isEn)
    {
        string tail = type.NameDef.Contains('_') ? type.NameDef[(type.NameDef.IndexOf('_') + 1)..] : type.NameDef;
        string tribe = type.Tribe.Length > 0 ? TribeText(type.Tribe, isEn) + " " : "";
        if (isEn) return $"{tribe}{tail.Replace('_', ' ')}";
        string? known = GermanObjectWords.FirstOrDefault(pair => tail.Equals(pair.Key, StringComparison.OrdinalIgnoreCase) || tail.EndsWith(pair.Key, StringComparison.OrdinalIgnoreCase)).Value;
        return known is null ? $"{tribe}{tail.Replace('_', ' ')}" : $"{tribe}{known}（{tail}）";
    }
}
