using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ComboBox _placeCategory = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ComboBox _placeTribe = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ListBox _placeTypes = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly NumericUpDown _placeTeam = new() { Dock = DockStyle.Fill, Minimum = -1, Maximum = 15, Value = 0 };
    private readonly NumericUpDown _placeCount = new() { Dock = DockStyle.Fill, Minimum = 1, Maximum = 20, Value = 10 };
    private readonly NumericUpDown _placeAngle = new() { Dock = DockStyle.Fill, Minimum = 0, Maximum = 359, Increment = 45 };
    private readonly ListView _placedList = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, HeaderStyle = ColumnHeaderStyle.Nonclickable, MultiSelect = true };
    private readonly Button _placedEditButton = new() { Height = 32, Enabled = false };
    private readonly Button _placedDuplicateButton = new() { Height = 32, Enabled = false };
    private readonly Button _placedDeleteButton = new() { Height = 32, Enabled = false };
    private readonly Label _placeHint = new() { Dock = DockStyle.Top, AutoSize = true, MaximumSize = new Size(300, 0), Padding = new Padding(4, 6, 4, 4), ForeColor = WinFormsTheme.TextSecondary };
    private Label _lblPlaceCategory = null!, _lblPlaceTribe = null!, _lblPlaceTeam = null!, _lblPlaceCount = null!, _lblPlaceAngle = null!;
    private IReadOnlyList<SdlObjectType> _objectCatalog = Array.Empty<SdlObjectType>();
    private readonly PlacementEditSession _placementSession = new();
    private IReadOnlyList<SdlPlacedObject> _placedObjects => _placementSession.Capture();
    private bool PlacedDirty() => _placementSession.IsDirty;

    internal Func<PlacedObjectEditDialog, DialogResult> EditDialogRunner { get; set; } = dialog => dialog.ShowDialog();
    internal Button PlacedEditButton => _placedEditButton;
    internal Button PlacedDuplicateButton => _placedDuplicateButton;
    internal Button PlacedDeleteButton => _placedDeleteButton;
    internal ListView PlacedList => _placedList;
    internal PlacementEditSession PlacementSession => _placementSession;

    private Control BuildPlacementPanel()
    {
        var panel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = WinFormsTheme.Surface };
        panel.SizeChanged += (_, _) => _placeHint.MaximumSize = new Size(Math.Max(1, panel.ClientSize.Width - panel.Padding.Horizontal), 0);
        var options = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 2, BackColor = WinFormsTheme.SurfaceRaised, Padding = new Padding(6) };
        options.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70)); options.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        _lblPlaceCategory = AddSceneField(options, 0, "類別", _placeCategory);
        _lblPlaceTribe = AddSceneField(options, 1, "部族", _placeTribe);
        _lblPlaceTeam = AddSceneField(options, 2, "隊伍", _placeTeam);
        _lblPlaceCount = AddSceneField(options, 3, "人數", _placeCount);
        _lblPlaceAngle = AddSceneField(options, 4, "角度", _placeAngle);

        var placedHost = new Panel { Dock = DockStyle.Bottom, Height = 230, Padding = new Padding(0, 8, 0, 0) };
        _placedList.Columns.Add("物件", 140);
        _placedList.Columns.Add("隊伍", 45);
        _placedList.Columns.Add("位置", 80);
        _placedList.Columns.Add("角度", 45);

        var buttonRow = new TableLayoutPanel
        {
            Dock = DockStyle.Bottom,
            AutoSize = true,
            AutoSizeMode = AutoSizeMode.GrowAndShrink,
            ColumnCount = 2,
            RowCount = 2,
            Padding = new Padding(0, 4, 0, 0),
            BackColor = WinFormsTheme.Surface
        };
        buttonRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        buttonRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        buttonRow.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        buttonRow.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        foreach (Button button in new[] { _placedEditButton, _placedDuplicateButton, _placedDeleteButton })
        {
            button.AutoSize = true;
            button.MinimumSize = new Size(0, 32);
        }

        _placedEditButton.Dock = DockStyle.Fill;
        _placedDuplicateButton.Dock = DockStyle.Fill;
        _placedDeleteButton.Dock = DockStyle.Fill;

        buttonRow.Controls.Add(_placedEditButton, 0, 0);
        buttonRow.Controls.Add(_placedDuplicateButton, 1, 0);
        buttonRow.Controls.Add(_placedDeleteButton, 0, 1);
        buttonRow.SetColumnSpan(_placedDeleteButton, 2);

        placedHost.Controls.Add(_placedList);
        placedHost.Controls.Add(buttonRow);

        panel.Controls.Add(_placeTypes);
        panel.Controls.Add(placedHost);
        panel.Controls.Add(options);
        panel.Controls.Add(_placeHint);

        WinFormsTheme.StyleSecondaryButton(_placedEditButton);
        WinFormsTheme.StyleSecondaryButton(_placedDuplicateButton);
        WinFormsTheme.StyleDangerButton(_placedDeleteButton);

        _placedList.SelectedIndexChanged += (_, _) => UpdatePlacedButtonsState();
        _placedList.DoubleClick += (_, _) => EditSelectedPlacedObject();
        _placedEditButton.Click += (_, _) => EditSelectedPlacedObject();
        _placedDuplicateButton.Click += (_, _) => DuplicateSelectedPlacedObjects();

        return panel;
    }

    internal void UpdatePlacedButtonsState()
    {
        bool isCustom = _selected?.IsCustom == true;
        int count = _placedList.SelectedItems.Count;
        _placedDeleteButton.Enabled = isCustom && count > 0;
        _placedDuplicateButton.Enabled = isCustom && count > 0;
        _placedEditButton.Enabled = isCustom && count == 1;
    }

    /// <summary>放置目錄：cl_scint.ini [ObjDefName] 的別名（建築、人物…），TemplateFields["alias"] 保存別名。</summary>
    private static IReadOnlyList<SdlObjectType> BuildSpawnCatalog(string gamePath)
        => ScriptObjectAliases.Load(gamePath)
            .Select(alias => new SdlObjectType(alias.NameDef, -1, alias.Category, alias.Tribe, 0, new Dictionary<string, string> { ["alias"] = alias.Alias }))
            .OrderBy(type => type.Category).ThenBy(type => type.Tribe).ThenBy(type => type.NameDef, StringComparer.OrdinalIgnoreCase)
            .ToArray();

    private static string AliasOf(SdlObjectType type) => type.TemplateFields.TryGetValue("alias", out string? alias) ? alias : type.NameDef;

    private IReadOnlyDictionary<int, LevelObjectTemplate>? _buildingTemplates;

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
        UpdateScriptBuildingNotice(new ScenarioDocument());
        ScenarioDocument scenario;
        try { scenario = ScenarioDocument.Load(map); }
        catch (System.Text.Json.JsonException) { yield break; }
        UpdateScriptBuildingNotice(scenario);
        foreach (ScenarioSpawn spawn in scenario.Spawns)
        {
            SdlObjectType? type = _objectCatalog.FirstOrDefault(item => AliasOf(item).Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase));
            if (type is null) continue;
            yield return new SdlPlacedObject(type, spawn.X, spawn.Y, spawn.Z, spawn.Team, spawn.Angle, spawn.Count > 1 ? spawn.Count : 1) { ScenarioId = spawn.Id };
        }
    }

    private string[] _scriptBuildingAliases = [];

    private void UpdateScriptBuildingNotice(ScenarioDocument scenario)
    {
        _scriptBuildingAliases = scenario.Spawns.Where(spawn => !spawn.Prebuilt && _objectCatalog.Any(type =>
                type.Category == SdlObjectCategory.Building && AliasOf(type).Equals(spawn.Alias, StringComparison.OrdinalIgnoreCase)))
            .Select(spawn => spawn.Alias).Distinct(StringComparer.OrdinalIgnoreCase).Order(StringComparer.OrdinalIgnoreCase).ToArray();
        LocalizePlacementTab(AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English);
    }

    private string ScriptBuildingNotice(bool isEn)
    {
        if (_scriptBuildingAliases.Length == 0) return "";
        string names = string.Join(", ", _scriptBuildingAliases.Take(5));
        if (_scriptBuildingAliases.Length > 5) names += isEn ? $" (+{_scriptBuildingAliases.Length - 5} more)" : $"（另 {_scriptBuildingAliases.Length - 5} 種）";
        return isEn
            ? $"Saved buildings created by script as 0% construction sites at game start: {names}."
            : $"已儲存建築中，以下類型會在開局由腳本建立為 0% 工地：{names}。";
    }

    private void LocalizePlacementTab(bool isEn)
    {
        if (_lblPlaceCategory is null) return;
        _lblPlaceCategory.Text = isEn ? "Category" : "類別";
        _lblPlaceTribe.Text = isEn ? "Tribe" : "部族";
        _lblPlaceTeam.Text = isEn ? "Team" : "隊伍";
        _lblPlaceCount.Text = isEn ? "Soldiers" : "人數";
        _lblPlaceAngle.Text = isEn ? "Angle" : "角度";
        _placedEditButton.Text = isEn ? "Edit..." : "編輯...";
        _placedDuplicateButton.Text = isEn ? "Duplicate" : "複製選取";
        _placedDeleteButton.Text = isEn ? "Delete Selected" : "刪除選取的物件";
        _placeHint.Text = isEn
            ? "Pick a type, activate \"Place Objects\", then click the map. Buildings with a finished template are placed directly in the map (teams 0–8; 8 = neutral); others start as construction sites. Characters use units of 1–20 soldiers, teams 0–7. Team 0 is the human player. Keep buildings clear of trees and water."
            : "選類型後啟用「放置物件」並點擊地圖。有完工範本的建築直接放入地圖（隊伍 0–8；8＝中立），其他建築在開局生成工地。人物以部隊建立，人數 1–20、隊伍 0–7；隊伍 0 為玩家。建築請避開樹木與水域。";
        string notice = ScriptBuildingNotice(isEn);
        if (notice.Length > 0) _placeHint.Text += "\n\n" + notice;
        if (_placedList.Columns.Count >= 4)
        {
            _placedList.Columns[0].Text = isEn ? "Object" : "物件";
            _placedList.Columns[1].Text = isEn ? "Team" : "隊伍";
            _placedList.Columns[2].Text = isEn ? "Tile" : "位置";
            _placedList.Columns[3].Text = isEn ? "Angle" : "角度";
        }
        PopulatePlacementFilters();
        RefreshPlacedList();
    }

    private sealed record PlacementTypeItem(SdlObjectType Type, string Text) { public override string ToString() => Text; }
    private sealed record FilterItem(object? Value, string Text) { public override string ToString() => Text; }

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
        IReadOnlyList<SdlPlacedObject> placements = _placementSession.Capture();
        for (int index = 0; index < placements.Count; index++)
        {
            SdlPlacedObject item = placements[index];
            var row = new ListViewItem(ObjectDisplayName(item.Type, isEn) + (item.UnitCount > 1 ? $" ×{item.UnitCount}" : "")) { Tag = index };
            row.SubItems.Add(item.Team.ToString(System.Globalization.CultureInfo.InvariantCulture));
            row.SubItems.Add($"{(int)(item.WorldX / 256)},{(int)(item.WorldZ / 256)}");
            row.SubItems.Add($"{(int)item.Angle}°");
            _placedList.Items.Add(row);
        }
        _placedList.EndUpdate();
        UpdatePlacedButtonsState();
    }

    /// <summary>放置模式下，在 3D 游標格顯示所選物件的原生 sprite 半透明預覽。</summary>
    private void UpdatePlacementPreview()
    {
        if (_view3d is null) return;
        string? name = _editMode == EditMode.PlaceObject && _selected?.IsCustom == true ? (_placeTypes.SelectedItem as PlacementTypeItem)?.Type.NameDef : null;
        _view3d.SetPlacementPreview(name, (int)_placeTeam.Value);
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
        _placementSession.Add(new SdlPlacedObject(selection.Type, worldX, worldY, worldZ, (int)_placeTeam.Value, (float)_placeAngle.Value,
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

    internal void DuplicateSelectedPlacedObjects()
    {
        if (_selected?.IsCustom != true || _placedList.SelectedItems.Count == 0) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        var selectedIndices = _placedList.SelectedItems.Cast<ListViewItem>()
            .Select(row => (int)row.Tag!)
            .OrderByDescending(index => index)
            .ToList();

        try
        {
            _placementSession.DuplicateMany(selectedIndices, offsetX: 256f, offsetZ: 256f);
        }
        catch (ArgumentOutOfRangeException)
        {
            _status.Text = isEn
                ? "Cannot duplicate: a shifted object would be outside the map or has invalid values. No objects were changed."
                : "無法複製：偏移後有物件超出地圖或數值無效；所有物件均未變更。";
            return;
        }

        RefreshPlacedList();
        IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
        _canvas.UpdateSceneObjects(effective); _view3d?.UpdateSceneObjects(effective);
        UpdateEditorState();
        _status.Text = isEn
            ? $"Duplicated {selectedIndices.Count} placed object(s); Save to write."
            : $"已複製 {selectedIndices.Count} 個放置物件；按「儲存」才會寫入。";
    }

    internal void EditSelectedPlacedObject()
    {
        if (_selected?.IsCustom != true || _placedList.SelectedItems.Count != 1) return;
        int index = (int)_placedList.SelectedItems[0].Tag!;
        IReadOnlyList<SdlPlacedObject> items = _placementSession.Capture();
        if (index < 0 || index >= items.Count) return;
        SdlPlacedObject current = items[index];
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;

        using var dialog = new PlacedObjectEditDialog(current, isEn);
        if (EditDialogRunner(dialog) == DialogResult.OK && dialog.Result is not null)
        {
            SdlPlacedObject updated = dialog.Result;
            _placementSession.Edit(index, updated);
            RefreshPlacedList();
            IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
            _canvas.UpdateSceneObjects(effective); _view3d?.UpdateSceneObjects(effective);
            UpdateEditorState();
            _status.Text = isEn
                ? $"Updated {ObjectDisplayName(updated.Type, isEn)}."
                : $"已更新 {ObjectDisplayName(updated.Type, isEn)}。";
        }
    }

    private void DeleteSelectedPlacedObjects()
    {
        if (_selected?.IsCustom != true || _placedList.SelectedItems.Count == 0) return;
        _placementSession.RemoveMany(_placedList.SelectedItems.Cast<ListViewItem>().Select(row => (int)row.Tag!));
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

    private static readonly Dictionary<string, string> GermanObjectWords = new(StringComparer.OrdinalIgnoreCase)
    {
        ["Haupthaus"] = "主屋", ["Wohnhaus"] = "住宅", ["Wohnzelt"] = "居住帳篷", ["Lagerhaus"] = "倉庫", ["Lagerzelt"] = "倉庫帳篷",
        ["Bauernhof"] = "農場", ["Schlachterei"] = "屠宰場", ["Schreinerei"] = "木工坊", ["Mine"] = "礦場", ["Pferdestall"] = "馬廄",
        ["Waffenschmiede"] = "兵器鋪", ["Goldschmiede"] = "金匠鋪", ["Mauer"] = "城牆", ["Mauerecke"] = "城牆轉角", ["Mauertor"] = "城門",
        ["Tor"] = "城門", ["Turm"] = "塔樓", ["Palisade"] = "木柵", ["Palisadenecke"] = "木柵轉角", ["Palisadentor"] = "木柵門",
        ["Opferstaette"] = "祭壇", ["Anfuehrer"] = "領主", ["Kampf_Icon"] = "戰鬥部隊", ["Zivil_Icon"] = "平民隊",
    };

    internal static string ObjectDisplayName(SdlObjectType type, bool isEn)
    {
        string tail = type.NameDef.Contains('_') ? type.NameDef[(type.NameDef.IndexOf('_') + 1)..] : type.NameDef;
        string tribe = type.Tribe.Length > 0 ? TribeText(type.Tribe, isEn) + " " : "";
        if (isEn) return $"{tribe}{tail.Replace('_', ' ')}";
        string? known = GermanObjectWords.FirstOrDefault(pair => tail.Equals(pair.Key, StringComparison.OrdinalIgnoreCase) || tail.EndsWith(pair.Key, StringComparison.OrdinalIgnoreCase)).Value;
        return known is null ? $"{tribe}{tail.Replace('_', ' ')}" : $"{tribe}{known}（{tail}）";
    }
}

/// <summary>放置物件編輯對話框，編輯隊伍、世界座標、旋轉角度與部隊人數；強制保留 persistent ScenarioId。</summary>
internal sealed class PlacedObjectEditDialog : Form
{
    private readonly SdlPlacedObject _original;
    private readonly bool _isEn;
    private readonly bool _isFigure;
    private readonly Label _lblInfo;
    private readonly Label _lblId;
    private readonly NumericUpDown _teamInput;
    private readonly NumericUpDown _posXInput;
    private readonly NumericUpDown _posYInput;
    private readonly NumericUpDown _posZInput;
    private readonly NumericUpDown _angleInput;
    private readonly NumericUpDown _countInput;
    private readonly Label _lblHint;
    private readonly Button _btnOk;
    private readonly Button _btnCancel;

    internal SdlPlacedObject? Result { get; private set; }
    internal NumericUpDown TeamInput => _teamInput;
    internal NumericUpDown PosXInput => _posXInput;
    internal NumericUpDown PosYInput => _posYInput;
    internal NumericUpDown PosZInput => _posZInput;
    internal NumericUpDown AngleInput => _angleInput;
    internal NumericUpDown CountInput => _countInput;
    internal Button OkButton => _btnOk;
    internal Button CancelButtonControl => _btnCancel;

    internal PlacedObjectEditDialog(SdlPlacedObject original, bool isEn)
    {
        _original = original ?? throw new ArgumentNullException(nameof(original));
        _isEn = isEn;
        _isFigure = original.Type.Category == SdlObjectCategory.Figure;

        AutoScaleDimensions = new SizeF(96F, 96F); AutoScaleMode = AutoScaleMode.Dpi; // 以 96 DPI 設計，PerMonitorV2 下依實際 DPI 縮放固定像素版面
        Text = isEn ? "Edit Placed Object" : "編輯放置物件";
        Size = new Size(500, 440);
        StartPosition = FormStartPosition.CenterParent;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;

        var panel = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 9,
            Padding = new Padding(12),
            BackColor = WinFormsTheme.Surface
        };
        panel.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 95));
        panel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        // 欄位列依內容高度排列；剩餘空間交給最後的空白列，避免最後一個標籤被拉伸而與輸入框錯位。
        for (int row = 0; row < 8; row++) panel.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        panel.RowStyles.Add(new RowStyle(SizeType.Percent, 100));

        string name = MapEditorForm.ObjectDisplayName(original.Type, isEn);
        _lblInfo = new Label
        {
            Dock = DockStyle.Fill,
            Text = $"{name} ({original.Type.NameDef})",
            ForeColor = WinFormsTheme.TextPrimary,
            TextAlign = ContentAlignment.MiddleLeft
        };
        _lblId = new Label
        {
            Dock = DockStyle.Fill,
            Text = original.ScenarioId == Guid.Empty ? "(None)" : original.ScenarioId.ToString("D"),
            ForeColor = WinFormsTheme.TextMuted,
            TextAlign = ContentAlignment.MiddleLeft
        };

        _teamInput = new NumericUpDown
        {
            Dock = DockStyle.Fill,
            Minimum = _isFigure ? 0 : -1,
            Maximum = _isFigure ? 7 : 15,
            Value = Math.Clamp(original.Team, _isFigure ? 0 : -1, _isFigure ? 7 : 15)
        };

        _posXInput = new NumericUpDown
        {
            Dock = DockStyle.Fill,
            Minimum = 0,
            Maximum = 16383,
            Value = Math.Clamp((decimal)original.WorldX, 0, 16383)
        };

        _posYInput = new NumericUpDown
        {
            Dock = DockStyle.Fill,
            Minimum = -1000,
            Maximum = 10000,
            Value = Math.Clamp((decimal)original.WorldY, -1000, 10000)
        };

        _posZInput = new NumericUpDown
        {
            Dock = DockStyle.Fill,
            Minimum = 0,
            Maximum = 16383,
            Value = Math.Clamp((decimal)original.WorldZ, 0, 16383)
        };

        _angleInput = new NumericUpDown
        {
            Dock = DockStyle.Fill,
            Minimum = 0,
            Maximum = 359,
            Increment = 45,
            Value = Math.Clamp((decimal)original.Angle, 0, 359)
        };

        _countInput = new NumericUpDown
        {
            Dock = DockStyle.Fill,
            Minimum = 1,
            Maximum = 20,
            Value = _isFigure ? Math.Clamp(original.UnitCount > 0 ? original.UnitCount : 1, 1, 20) : 1,
            Enabled = _isFigure
        };

        _lblHint = new Label
        {
            Dock = DockStyle.Fill,
            ForeColor = WinFormsTheme.TextSecondary,
            Text = _isFigure
                ? (isEn ? "Units: team 0–7, count 1–20 (team 0 = player)." : "人物／部隊：隊伍 0–7，人數 1–20（隊伍 0 為玩家）。")
                : (isEn ? "Buildings: team 0–15 (team 8 = neutral); count does not apply." : "建築：隊伍 0–15（隊伍 8 為中立）；人數不適用。")
        };

        AddField(panel, 0, isEn ? "Object" : "物件", _lblInfo);
        AddField(panel, 1, isEn ? "Scenario ID" : "識別碼", _lblId);
        AddField(panel, 2, isEn ? "Team" : "隊伍", _teamInput);
        AddField(panel, 3, isEn ? "Position X" : "世界 X", _posXInput);
        AddField(panel, 4, isEn ? "Position Z" : "世界 Z", _posZInput);
        AddField(panel, 5, isEn ? "Height Y" : "高度 Y", _posYInput);
        AddField(panel, 6, isEn ? "Angle" : "角度", _angleInput);
        AddField(panel, 7, isEn ? "Soldiers" : "人數", _countInput);

        var buttonPanel = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 44,
            AutoSize = true, // 依按鈕實際高度（含字型與 DPI）加高，不壓縮按鈕
            Padding = new Padding(8),
            BackColor = WinFormsTheme.SurfaceRaised
        };

        _btnCancel = new Button { Text = isEn ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, AutoSize = true };
        _btnOk = new Button { Text = isEn ? "OK" : "確定", AutoSize = true };
        _btnOk.Click += (_, _) => OnOk();

        WinFormsTheme.StyleSecondaryButton(_btnCancel);
        WinFormsTheme.StylePrimaryButton(_btnOk);

        buttonPanel.Controls.Add(_btnCancel);
        buttonPanel.Controls.Add(_btnOk);

        var hintPanel = new Panel { Dock = DockStyle.Bottom, Height = 36, Padding = new Padding(12, 4, 12, 4), BackColor = WinFormsTheme.Surface };
        hintPanel.Controls.Add(_lblHint);

        Controls.Add(panel);
        Controls.Add(hintPanel);
        Controls.Add(buttonPanel);

        AcceptButton = _btnOk;
        CancelButton = _btnCancel;
        WinFormsTheme.Apply(this);
    }

    private static void AddField(TableLayoutPanel panel, int row, string label, Control control)
    {
        panel.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left, ForeColor = WinFormsTheme.TextSecondary }, 0, row);
        panel.Controls.Add(control, 1, row);
    }

    internal void SetInputs(int? team = null, float? worldX = null, float? worldY = null, float? worldZ = null, float? angle = null, int? unitCount = null)
    {
        if (team.HasValue) _teamInput.Value = Math.Clamp(team.Value, (int)_teamInput.Minimum, (int)_teamInput.Maximum);
        if (worldX.HasValue) _posXInput.Value = Math.Clamp((decimal)worldX.Value, _posXInput.Minimum, _posXInput.Maximum);
        if (worldY.HasValue) _posYInput.Value = Math.Clamp((decimal)worldY.Value, _posYInput.Minimum, _posYInput.Maximum);
        if (worldZ.HasValue) _posZInput.Value = Math.Clamp((decimal)worldZ.Value, _posZInput.Minimum, _posZInput.Maximum);
        if (angle.HasValue) _angleInput.Value = Math.Clamp((decimal)angle.Value, _angleInput.Minimum, _angleInput.Maximum);
        if (unitCount.HasValue && _countInput.Enabled) _countInput.Value = Math.Clamp(unitCount.Value, (int)_countInput.Minimum, (int)_countInput.Maximum);
    }

    internal void ClickOk() => OnOk();

    private void OnOk()
    {
        int team = (int)_teamInput.Value;
        int count = _isFigure ? (int)_countInput.Value : 0;

        if (_isFigure)
        {
            if (team is < 0 or > 7)
            {
                MessageBox.Show(this, _isEn ? "Units need a team between 0 and 7." : "人物與部隊的隊伍必須介於 0 與 7。", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
            if (count is < 1 or > 20)
            {
                MessageBox.Show(this, _isEn ? "Unit count must be between 1 and 20." : "部隊人數必須介於 1 與 20。", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
        }
        else
        {
            if (team is < -1 or > 15)
            {
                MessageBox.Show(this, _isEn ? "Team must be between 0 and 15." : "隊伍必須介於 0 與 15。", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
        }

        Result = _original with
        {
            Team = team,
            WorldX = (float)_posXInput.Value,
            WorldY = (float)_posYInput.Value,
            WorldZ = (float)_posZInput.Value,
            Angle = (float)_angleInput.Value,
            UnitCount = count,
            ScenarioId = _original.ScenarioId // Preserves persistent ScenarioId!
        };
        DialogResult = DialogResult.OK;
        Close();
    }
}
