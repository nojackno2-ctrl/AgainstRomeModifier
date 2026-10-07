using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using System.Globalization;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm : Form
{
    private readonly MapCanvasControl _canvas = new();
    private Map3DViewControl? _view3d;
    private Panel? _canvasHost;
    private readonly PictureBox _overview = new() { Dock = DockStyle.Fill, SizeMode = PictureBoxSizeMode.Zoom, BackColor = WinFormsTheme.Window };
    private readonly ListBox _palette = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly TextBox _paletteSearch = new() { Dock = DockStyle.Top, PlaceholderText = "搜尋草地、沙地、泥土、岩地…" };
    private readonly PictureBox _currentMaterialSwatch = new() { Width = 54, Height = 54, BackColor = Color.DimGray, SizeMode = PictureBoxSizeMode.Zoom };
    private readonly Label _currentMaterialLabel = new() { AutoSize = true, Text = "目前筆刷：尚未取樣", ForeColor = WinFormsTheme.TextPrimary, Font = WinFormsTheme.CreateFont(10F, FontStyle.Bold) };
    private readonly string _gamePath;
    private readonly TextBox _title = new() { Dock = DockStyle.Top };
    private readonly ErrorProvider _gameTextErrors = new() { BlinkStyle = ErrorBlinkStyle.NeverBlink };
    private readonly TextBox _subtitle = new() { Dock = DockStyle.Top };
    private readonly TextBox _briefing = new() { Dock = DockStyle.Top, Multiline = true, Height = 110, ScrollBars = ScrollBars.Vertical, AcceptsReturn = true };
    private readonly TextBox[] _teamNames = Enumerable.Range(0, 8).Select(_ => new TextBox { Dock = DockStyle.Top }).ToArray();
    private readonly NumericUpDown _waterLevel = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 4096 };
    private readonly TextBox _waterColor = new() { Dock = DockStyle.Top };
    private readonly NumericUpDown _waterWarpShift = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 64 };
    private readonly NumericUpDown _waterBumpAmplitude = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 1024 };
    private readonly NumericUpDown _waterBumpFrequency = new() { Dock = DockStyle.Top, Minimum = 1, Maximum = 16 };
    private readonly NumericUpDown _flashProbability = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 1000 };
    private readonly NumericUpDown _dayStart = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 24 };
    private readonly NumericUpDown _dayEnd = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 24 };
    private readonly CheckBox _rain = new() { Dock = DockStyle.Top, Text = "水面雨滴" };
    private readonly Button _waterColorButton = new() { Dock = DockStyle.Top, Height = 34, Text = "選擇水面顏色…" };
    private readonly ComboBox _brushSize = new() { Dock = DockStyle.Top, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly CheckBox _showGrid = new() { Dock = DockStyle.Top, Height = 30, Text = "顯示格線", Checked = true };
    private readonly CheckBox _showObjects = new() { Dock = DockStyle.Top, Height = 30, Text = "顯示建築與場景物件", Checked = true };
    private readonly CheckBox _gameLighting = new() { AutoSize = true, Text = "遊戲光照" };
    private readonly NumericUpDown _gameHour = new() { Width = 76, DecimalPlaces = 2, Minimum = 0, Maximum = 24, Increment = .25m, Value = 12, Enabled = false };
    private readonly Label _gameHourLabel = new() { AutoSize = true, Anchor = AnchorStyles.Left, Text = "時刻" };
    private readonly CheckBox _stampMode = new() { Dock = DockStyle.Top, Height = 30, Text = "圖塊印章（原版道路、河流、岩壁等）" };
    private readonly CheckBox _stampOtherRegions = new() { Dock = DockStyle.Top, Height = 30, Text = "顯示其他地區圖塊", Visible = false };
    private string? _stampTexture;
    private (int X, int Y)? _lastStampTile;
    private readonly CheckBox _autoBridge = new() { Dock = DockStyle.Top, Height = 30, Text = "自動過渡（插入中介材質）", Checked = true };
    private readonly TrackBar _reliefScale = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 200, Value = 100, TickFrequency = 25 };
    private readonly ListView _sceneList = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, HeaderStyle = ColumnHeaderStyle.Nonclickable };
    private readonly Label _sceneSummary = new() { Dock = DockStyle.Top, Height = 64, Padding = new Padding(10, 16, 10, 8), ForeColor = WinFormsTheme.TextSecondary };
    private readonly NumericUpDown _sceneTeam = new() { Dock = DockStyle.Fill, Minimum = -1, Maximum = 15 };
    private readonly NumericUpDown _sceneX = SceneCoordinateInput();
    private readonly NumericUpDown _sceneY = SceneCoordinateInput();
    private readonly NumericUpDown _sceneZ = SceneCoordinateInput();
    private readonly NumericUpDown _sceneAngle = new() { Dock = DockStyle.Fill, Minimum = -360, Maximum = 360, DecimalPlaces = 2, Increment = 15 };
    private readonly Button _sceneApplyButton = new() { Dock = DockStyle.Fill, Height = 32, Enabled = false };
    private readonly Button _sceneRestoreButton = new() { Dock = DockStyle.Fill, Height = 32, Enabled = false };
    private readonly Button _sceneDuplicateButton = new() { Dock = DockStyle.Fill, Height = 32, Enabled = false };
    private readonly Button _sceneDeleteButton = new() { Dock = DockStyle.Fill, Height = 32, Enabled = false };
    private readonly Button _sceneAddButton = new() { Dock = DockStyle.Fill, Height = 32, Enabled = false };
    private readonly Button _sceneTranslateButton = new() { Dock = DockStyle.Fill, Height = 32, Enabled = false };
    private readonly Label _modeBanner = new() { Dock = DockStyle.Top, Height = 34, TextAlign = ContentAlignment.MiddleCenter };
    private readonly ToolStripStatusLabel _status = new() { Spring = true, TextAlign = ContentAlignment.MiddleLeft };
    private readonly ToolStripButton _saveButton = new("儲存") { Enabled = false };
    private readonly ToolStripButton _gamePreviewButton = new("選用：啟動遊戲測試") { Enabled = false };
    private readonly ToolStripButton _undoButton = new("復原") { Enabled = false };
    private readonly ToolStripButton _aiMapButton = new("AI 製圖…") { Enabled = false };
    private readonly ToolStripButton _redoButton = new("重做") { Enabled = false };
    private readonly ToolStripButton _textureTool = new("材質筆刷") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _sceneMoveTool = new("移動場景物件") { CheckOnClick = true };
    private readonly ToolStripButton _resetTerrainButton = new("還原地表") { Enabled = false };
    private readonly ToolStripButton _heightTool = new("地形高度") { CheckOnClick = true };
    private readonly ToolStripButton _blankTerrainButton = new("重設平坦地形…") { Enabled = false };
    private readonly ToolStripButton _collisionTool = new("通行區域") { CheckOnClick = true };
    private readonly ToolStripButton _placeTool = new("放置物件") { CheckOnClick = true };
    private readonly ToolStripComboBox _terrainOperation = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 96, Visible = false };
    private readonly ToolStripComboBox _terrainStrength = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 72, Visible = false };
    private readonly ToolStripButton _view2dButton = new("2D 俯視") { CheckOnClick = true };
    private readonly ToolStripButton _view3dButton = new("3D 場景") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _3dDiagnosticsButton = new("3D 診斷") { Visible = false };
    private readonly ToolStripButton _retryDisplayButton = new("重試顯示") { Visible = false };
    private readonly ToolStripButton _mapMenuButton = new("地圖選單");
    private readonly ToolStripButton _btnLangZH = new("繁體中文") { Alignment = ToolStripItemAlignment.Right };
    private readonly ToolStripButton _btnLangEN = new("English") { Alignment = ToolStripItemAlignment.Right };
    private readonly ToolStripLabel _currentMapLabel = new();
    private readonly ToolStripLabel _lblTerrainGroup = new("地表：");
    private Label _paletteHeader = null!;
    private readonly Label _lblBrushInstructions = new() { AutoSize = true, MaximumSize = new Size(200, 0), ForeColor = WinFormsTheme.TextSecondary };
    private readonly Label _lblBrushSizeTitle = new() { Dock = DockStyle.Top, Height = 22, ForeColor = WinFormsTheme.TextSecondary };
    private readonly Label _lblReliefScaleTitle = new() { Dock = DockStyle.Top, Height = 22, ForeColor = WinFormsTheme.TextSecondary };
    private readonly Label _lblOverviewTitle = new() { Dock = DockStyle.Top, Height = 27, TextAlign = ContentAlignment.MiddleCenter, ForeColor = WinFormsTheme.TextPrimary, BackColor = WinFormsTheme.SurfaceRaised };
    private readonly ToolStripStatusLabel _lblStatusInstructions = new();

    private Label _lblTitle = null!;
    private Label _lblSubtitle = null!;
    private Label _lblBriefing = null!;
    private Label _lblTeamNamesTitle = null!;
    private Label _lblWaterLevel = null!;
    private Label _lblWaterColor = null!;
    private Label _lblWaterWarpShift = null!;
    private Label _lblWaterBumpAmplitude = null!;
    private Label _lblWaterBumpFrequency = null!;
    private Label _lblFlashProbability = null!;
    private Label _lblDayStart = null!;
    private Label _lblDayEnd = null!;
    private readonly Label[] _teamLabels = new Label[8];
    private Label _lblSceneTeam = null!;
    private Label _lblSceneX = null!;
    private Label _lblSceneY = null!;
    private Label _lblSceneZ = null!;
    private Label _lblSceneAngle = null!;
    private TabControl _inspectorTabs = null!;
    private Label _sceneWarningLabel = null!;
    private GameMapInfo? _selected;
    private BodenTexturesDocument? _texturesDocument;
    private TerrainBlendEditSession? _terrainBlendSession;
    private FloorTextureLibrary? _floorTextures;
    // 原遊戲 ALR/APT 物件素材（唯讀）；缺檔或格式錯誤時為 null，場景退回標記點。
    private AgainstRomeMapEditor.NativeAssets.NativeSpriteCatalog? _spriteCatalog;
    // 原遊戲物件陰影（shad.dat，唯讀）；缺檔或格式錯誤時為 null，不畫陰影。
    private AgainstRomeMapEditor.NativeAssets.NativeShadowCatalog? _shadowCatalog;
    private AgainstRomeMapEditor.NativeAssets.SceneLightingContext? _lightingContext;
    // 3D 點選到的可移除自然物件（Delete 移除）；選取其他物件或場景變更時清除。
    private MapSceneObject? _pickedNature;
    private FloorMaterialCatalog? _floorMaterials;
    private FloorMaterial? _activeMaterial;
    private IReadOnlyList<MapSceneObject> _sceneObjects = Array.Empty<MapSceneObject>();
    private IReadOnlyList<MapSceneObject> _sceneOriginalObjects = Array.Empty<MapSceneObject>();
    private IReadOnlyList<MapSceneObject> _sceneSavedObjects = Array.Empty<MapSceneObject>();
    private readonly List<SdlSceneObjectRemoval> _sceneRemovals = new();
    private readonly List<StagedSceneAddition> _sceneAdditions = new();
    private readonly Dictionary<string, SdlVector3> _settlementOffsets = new(StringComparer.OrdinalIgnoreCase);
    private IReadOnlyDictionary<string, SdlVector3> _settlementOrigins = new Dictionary<string, SdlVector3>();
    private int _nextSceneAdditionId;
    private string[] _savedTextures = Array.Empty<string>();
    private bool _propertyDirty;
    private bool _loading;
    private float _heightMapStep = 4;
    private bool _allowClose;
    private bool _sceneLoaded;
    private string? _last3DDiagnostic;
    private string? _terrainBlendNotice;
    private TerrainHeightEditSession? _terrainLayers;
    private TerrainLayer? _bodenLayer, _embossLayer, _collisionLayer;
    private int _flattenTarget = -1;
    private int _roughnessSeed = Random.Shared.Next(); // 每筆畫換一組粗糙化花紋
    private readonly bool _startWithBlankTerrain;
    /// <summary>空白地形：儲存時把 vertex.bmp 重設為白色（無色調）、smooth.bmp 重設為 0（不額外平滑）。</summary>
    private bool _resetAuxiliaryLayers;
    private (int X, int Y)? _lastTerrainTile;
    private readonly HashSet<int> _terrainStrokeTiles = new();

    private bool TextureDirty() => _terrainBlendSession?.IsDirty == true;

    private bool SceneDirty() => _sceneLoaded && (_sceneRemovals.Count > 0 || _sceneAdditions.Count > 0 || _settlementOffsets.Count > 0 || SdlSceneEditService.HasChanges(_sceneSavedObjects, _sceneObjects));

    /// <summary>畫布／清單實際呈現的場景：排除暫存刪除、加入暫存新增物件，並套用暫存的聚落整體平移。</summary>
    private IReadOnlyList<MapSceneObject> EffectiveSceneObjects()
    {
        IEnumerable<MapSceneObject> placed = _placedObjects.Select((item, index) =>
            new MapSceneObject(item.Type.NameDef, item.WorldX, item.WorldY, item.WorldZ, item.Team, SdlPlacedObjectsFile.FileName, -5000 - index, Angle: item.Angle));
        if (_editMode == EditMode.Nature) placed = placed.Concat(NatureDisplayObjects());
        if (_sceneRemovals.Count == 0 && _sceneAdditions.Count == 0 && _settlementOffsets.Count == 0) return _sceneObjects.Concat(placed).ToArray();
        HashSet<string> removed = _sceneRemovals.Select(item => item.SourceFile.ToUpperInvariant() + "|" + item.ObjectIndex).ToHashSet();
        return _sceneObjects.Where(item => !removed.Contains(SceneKey(item))).Concat(_sceneAdditions.Select(item => item.Display)).Select(WithSettlementOffset).Concat(placed).ToArray();
    }

    /// <summary>2D 畫布用 <paramref name="effective"/>；3D 場景另含地圖 DATA 的全部物件，呈現與遊戲相同的完整畫面。</summary>
    private void PushSceneObjects(IReadOnlyList<MapSceneObject> effective)
    {
        _canvas.UpdateSceneObjects(effective);
        _view3d?.UpdateSceneObjects(SceneObjectsFor3D(effective));
    }

    /// <summary>
    /// 3D 場景：<paramref name="effective"/> 加上 DATA/objects.dat 的物件（樹木、岩石、原版建築等）。
    /// 地景物件依自然物件編輯的待移除／待新增狀態呈現；自然模式下 effective 已含地景物件，不重複加入。
    /// </summary>
    private IReadOnlyList<MapSceneObject> SceneObjectsFor3D(IReadOnlyList<MapSceneObject> effective)
    {
        IEnumerable<MapSceneObject> level = _levelObjects
            .Where(item => !ObjDefNames.IsLandscape(_objdefNames.GetValueOrDefault(item.TypeId)))
            .Select(item => new MapSceneObject(_objdefNames.GetValueOrDefault(item.TypeId) ?? "", item.X, item.Y, item.Z, item.Team, "DATA/objects.dat", -100000 - item.Slot));
        if (_editMode != EditMode.Nature) level = level.Concat(NatureDisplayObjects());
        return ExpandTroopsFor3D(effective).Concat(level).ToArray();
    }

    /// <summary>記憶體中的物件世界座標以已存檔的 refpos 為準；暫存平移只在呈現時加上。</summary>
    private MapSceneObject WithSettlementOffset(MapSceneObject item) => _settlementOffsets.TryGetValue(item.SourceFile, out SdlVector3 offset)
        ? item with { WorldX = item.WorldX + offset.X, WorldY = item.WorldY + offset.Y, WorldZ = item.WorldZ + offset.Z }
        : item;

    private sealed class StagedSceneAddition
    {
        public required int Id { get; init; }
        public required string TemplateFile { get; init; }
        public required int TemplateIndex { get; init; }
        public required bool FromCatalog { get; init; }
        /// <summary>Display.SourceFile 是寫入目標聚落檔；與模板檔不同時即為跨聚落的自由新增。</summary>
        public required MapSceneObject Display { get; set; }
        public SdlSceneObjectAddition ToAddition() => new(TemplateFile, TemplateIndex, Display.Team, Display.LocalX, Display.LocalY, Display.LocalZ,
            Display.Angle, Display.SourceFile.Equals(TemplateFile, StringComparison.OrdinalIgnoreCase) ? null : Display.SourceFile);
    }

    private bool IsDirty => _propertyDirty || TextureDirty() || SceneDirty() || _terrainLayers?.IsDirty == true || _resetAuxiliaryLayers || PlacedDirty() || NatureDirty() || EventsDirty();

    public MapEditorForm(string gamePath, GameMapInfo selectedMap, bool startWithBlankTerrain = false)
    {
        _startWithBlankTerrain = startWithBlankTerrain;
        AutoScaleDimensions = new SizeF(96F, 96F); AutoScaleMode = AutoScaleMode.Dpi; // 以 96 DPI 設計，PerMonitorV2 下依實際 DPI 縮放固定像素版面
        Width = 1440; Height = 900; MinimumSize = new Size(1100, 700); StartPosition = FormStartPosition.CenterScreen;
        BackColor = WinFormsTheme.Window; ForeColor = WinFormsTheme.TextPrimary; Font = WinFormsTheme.CreateFont(9F);
        _gamePath = gamePath;
        _formationDefinition = LoadFormationDefinition(gamePath);
        _selected = selectedMap;
        _gameTextErrors.ContainerControl = this;
        _floorTextures = new FloorTextureLibrary(Path.Combine(gamePath, "floortex.dat"));
        _floorMaterials = new FloorMaterialCatalog(_floorTextures);
        _spriteCatalog = OpenSpriteCatalog(gamePath);
        _shadowCatalog = OpenShadowCatalog(gamePath);
        _lightingContext = AgainstRomeMapEditor.NativeAssets.SceneLightingContext.TryCreate(selectedMap.DirectoryPath, gamePath);
        BuildInterface();
        WinFormsTheme.Apply(this);
        WinFormsTheme.StylePrimaryButton(_sceneApplyButton);
        WinFormsTheme.StyleDangerButton(_sceneDeleteButton);
        WireEvents();
        KeyPreview = true;
        Shown += (_, _) =>
        {
            LoadSelectedMap();
            if (_startWithBlankTerrain && _selected?.IsCustom == true) ApplyBlankTerrain(confirm: false);
        };
        FormClosing += (_, e) => { if (!_allowClose && !ConfirmDiscardOrSave()) e.Cancel = true; };
        UpdateLanguageButtonStyles();
        ApplyLanguageToUI();
    }

    public bool ReturnToMapMenu { get; private set; }

    private void BuildInterface()
    {
        var commands = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(10, 6, 10, 6), BackColor = WinFormsTheme.Surface, ForeColor = WinFormsTheme.TextPrimary };
        commands.Items.AddRange(new ToolStripItem[] {
            _mapMenuButton, new ToolStripSeparator(), _currentMapLabel, new ToolStripSeparator(),
            _saveButton, _gamePreviewButton, new ToolStripSeparator(), _undoButton, _redoButton, new ToolStripSeparator(), _aiMapButton,
            new ToolStripSeparator { Alignment = ToolStripItemAlignment.Right },
            _btnLangEN, _btnLangZH
        });

        var tools = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(10, 5, 10, 5), BackColor = WinFormsTheme.SurfaceRaised, ForeColor = WinFormsTheme.TextPrimary };
        tools.Items.AddRange(new ToolStripItem[] { _lblTerrainGroup, _textureTool, _heightTool, _collisionTool, _natureTool, _placeTool, _sceneMoveTool, _terrainOperation, _terrainStrength, _resetTerrainButton, _blankTerrainButton, _regionToolsButton, _boxSelectButton, new ToolStripSeparator(), _view2dButton, _view3dButton, _3dDiagnosticsButton, _retryDisplayButton });

        tools.Items.Insert(tools.Items.IndexOf(_boxSelectButton) + 1, _layoutMenu);
        _paletteHeader = SectionHeader("地表繪製");
        var palettePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(10), BackColor = WinFormsTheme.Surface };
        var currentBrush = new FlowLayoutPanel { Dock = DockStyle.Top, Height = 76, Padding = new Padding(4), FlowDirection = FlowDirection.LeftToRight };
        var currentText = new FlowLayoutPanel { Width = 205, Height = 66, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        currentText.Controls.Add(_currentMaterialLabel); currentText.Controls.Add(_lblBrushInstructions);
        currentBrush.Controls.Add(_currentMaterialSwatch); currentBrush.Controls.Add(currentText);
        var lightingOptions = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 3, Padding = new Padding(0, 2, 0, 2) };
        lightingOptions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        lightingOptions.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); lightingOptions.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        lightingOptions.Controls.Add(_gameLighting, 0, 0); lightingOptions.Controls.Add(_gameHourLabel, 1, 0); lightingOptions.Controls.Add(_gameHour, 2, 0);
        var brushOptions = new Panel { Dock = DockStyle.Top, Height = 240 };
        brushOptions.Controls.Add(_autoBridge); brushOptions.Controls.Add(lightingOptions); brushOptions.Controls.Add(_showObjects); brushOptions.Controls.Add(_showGrid); brushOptions.Controls.Add(_reliefScale);
        brushOptions.Controls.Add(_lblReliefScaleTitle); brushOptions.Controls.Add(_lblBrushSizeTitle); brushOptions.Controls.Add(_brushSize);
        palettePanel.Controls.Add(_palette); palettePanel.Controls.Add(_paletteSearch); palettePanel.Controls.Add(_stampOtherRegions); palettePanel.Controls.Add(_stampMode); palettePanel.Controls.Add(brushOptions); palettePanel.Controls.Add(currentBrush); palettePanel.Controls.Add(_paletteHeader);

        var properties = BuildPropertiesPanel();
        _inspectorTabs = new TabControl { Dock = DockStyle.Fill };
        _inspectorTabs.TabPages.Add(new TabPage("地表") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[0].Controls.Add(palettePanel);
        _inspectorTabs.TabPages.Add(new TabPage("地圖屬性") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[1].Controls.Add(properties);
        var scenePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(10), BackColor = WinFormsTheme.Surface };
        _sceneList.Columns.Add("類型", 65); _sceneList.Columns.Add("物件", 170); _sceneList.Columns.Add("隊伍", 65); _sceneList.Columns.Add("來源", 120);
        var sceneEditor = new TableLayoutPanel { Dock = DockStyle.Bottom, Height = 352, ColumnCount = 2, Padding = new Padding(8), BackColor = WinFormsTheme.SurfaceRaised };
        sceneEditor.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 74)); sceneEditor.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        _sceneWarningLabel = new Label { AutoSize = true, MaximumSize = new Size(250, 0), ForeColor = WinFormsTheme.Warning };
        sceneEditor.Controls.Add(_sceneWarningLabel, 0, 0); sceneEditor.SetColumnSpan(_sceneWarningLabel, 2);
        _lblSceneTeam = AddSceneField(sceneEditor, 1, "隊伍", _sceneTeam);
        _lblSceneX = AddSceneField(sceneEditor, 2, "相對 X", _sceneX);
        _lblSceneY = AddSceneField(sceneEditor, 3, "相對 Y", _sceneY);
        _lblSceneZ = AddSceneField(sceneEditor, 4, "相對 Z", _sceneZ);
        _lblSceneAngle = AddSceneField(sceneEditor, 5, "角度", _sceneAngle);
        sceneEditor.Controls.Add(_sceneApplyButton, 0, 6); sceneEditor.Controls.Add(_sceneRestoreButton, 1, 6);
        sceneEditor.Controls.Add(_sceneDuplicateButton, 0, 7); sceneEditor.Controls.Add(_sceneDeleteButton, 1, 7);
        sceneEditor.Controls.Add(_sceneAddButton, 0, 8); sceneEditor.Controls.Add(_sceneTranslateButton, 1, 8);
        scenePanel.Controls.Add(_sceneList); scenePanel.Controls.Add(sceneEditor); scenePanel.Controls.Add(_sceneSummary);
        FitWrappedLabelHeight(_sceneSummary);
        _inspectorTabs.TabPages.Add(new TabPage("場景物件") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[2].Controls.Add(scenePanel);
        _inspectorTabs.TabPages.Add(new TabPage("放置物件") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[3].Controls.Add(BuildPlacementPanel());
        _inspectorTabs.TabPages.Add(new TabPage("自然物件") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[4].Controls.Add(BuildNaturePanel());
        _inspectorTabs.TabPages.Add(new TabPage("事件") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[^1].Controls.Add(BuildEventsPanel());
        _mapCheckTab = new TabPage("地圖檢查") { BackColor = WinFormsTheme.Surface };
        _inspectorTabs.TabPages.Add(_mapCheckTab); _mapCheckTab.Controls.Add(BuildMapCheckPanel());

        _canvasHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(10), BackColor = WinFormsTheme.Window };
        _canvasHost.Controls.Add(_canvas); _canvasHost.Controls.Add(_modeBanner);
        try
        {
            _view3d = new Map3DViewControl { Visible = true };
            _canvasHost.Controls.Add(_view3d);
            _view3d.BringToFront();
        }
        catch (Exception ex)
        {
            Disable3DView("無法建立 OpenGL 3D 控制項。", ex);
        }
        var overviewHost = new Panel { Width = 190, Height = 190, Anchor = AnchorStyles.Right | AnchorStyles.Bottom, Padding = new Padding(5), BackColor = WinFormsTheme.BorderStrong };
        overviewHost.Controls.Add(_overview); overviewHost.Controls.Add(_lblOverviewTitle);
        _canvasHost.Controls.Add(overviewHost); overviewHost.BringToFront();
        _canvasHost.Resize += (_, _) => overviewHost.Location = new Point(Math.Max(12, _canvasHost.ClientSize.Width - overviewHost.Width - 18), Math.Max(46, _canvasHost.ClientSize.Height - overviewHost.Height - 18));
        _overview.MouseDown += (_, e) => NavigateMinimap(e);
        _overview.MouseMove += (_, e) => NavigateMinimap(e);
        _overview.Paint += (_, e) => DrawMinimapIndicator(e.Graphics);

        var centerRight = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1140, 760), SplitterDistance = 820 };
        centerRight.Panel1.Controls.Add(_canvasHost); centerRight.Panel2.Controls.Add(_inspectorTabs); centerRight.Panel2MinSize = 280;
        var statusStrip = new StatusStrip { BackColor = WinFormsTheme.Surface, ForeColor = WinFormsTheme.TextSecondary };
        statusStrip.Items.Add(_status); statusStrip.Items.Add(_lblStatusInstructions);
        Controls.Add(centerRight); Controls.Add(tools); Controls.Add(commands); Controls.Add(statusStrip);
        // WinForms 依 z-order 由後往前配置 Dock；Fill 必須最後配置（位於最前），否則上方工具列會蓋住分頁標籤與畫布頂端。
        centerRight.BringToFront();
        SetActiveView(_view3d is not null);
    }

    private Panel BuildPropertiesPanel()
    {
        var table = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 1, Padding = new Padding(10) };
        _lblTitle = AddField(table, "地圖名稱", _title);
        _lblSubtitle = AddField(table, "地圖副標題", _subtitle);
        _lblBriefing = AddField(table, "任務說明", _briefing);
        var teams = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 2 };
        teams.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 60)); teams.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        for (int index = 0; index < _teamNames.Length; index++)
        {
            var lbl = new Label { Text = $"隊伍 {index}", AutoSize = true, Anchor = AnchorStyles.Left, ForeColor = WinFormsTheme.TextSecondary };
            _teamLabels[index] = lbl;
            teams.Controls.Add(lbl, 0, index);
            teams.Controls.Add(_teamNames[index], 1, index);
        }
        _lblTeamNamesTitle = AddField(table, "隊伍名稱", teams);
        _lblWaterLevel = AddField(table, "水面高度", _waterLevel);
        _lblWaterColor = AddField(table, "水面顏色", _waterColorButton);
        _lblWaterWarpShift = AddField(table, "水面波動位移", _waterWarpShift);
        _lblWaterBumpAmplitude = AddField(table, "水面凹凸幅度", _waterBumpAmplitude);
        _lblWaterBumpFrequency = AddField(table, "水面凹凸頻率", _waterBumpFrequency);
        _lblFlashProbability = AddField(table, "每秒閃電機率", _flashProbability);
        _lblDayStart = AddField(table, "日出時間", _dayStart);
        _lblDayEnd = AddField(table, "日落時間", _dayEnd);
        AddField(table, "", _rain);
        var panel = new Panel { Dock = DockStyle.Fill, AutoScroll = true, BackColor = WinFormsTheme.Surface }; panel.Controls.Add(table); return panel;
    }

    private void WireEvents()
    {
        _palette.SelectedIndexChanged += (_, _) => { if (_palette.SelectedItem is PaletteItem item) SelectBrush(item); };
        _palette.DrawMode = DrawMode.OwnerDrawFixed; _palette.ItemHeight = 36; _palette.DrawItem += DrawPaletteItem;
        _paletteSearch.TextChanged += (_, _) => LoadPalette(_paletteSearch.Text);
        _stampMode.CheckedChanged += (_, _) => { CommitStroke(); UpdateContinuousStampPainting(); _stampOtherRegions.Visible = _stampMode.Checked; _paletteSearch.Text = ""; LoadPalette(); };
        _stampOtherRegions.CheckedChanged += (_, _) => LoadPalette(_paletteSearch.Text);
        _brushSize.SelectedIndexChanged += (_, _) => _canvas.BrushSize = BrushSizes[Math.Clamp(_brushSize.SelectedIndex, 0, BrushSizes.Length - 1)];
        _brushSize.SelectedIndexChanged += (_, _) => { if (_view3d is not null) _view3d.BrushSize = _canvas.BrushSize; };
        _showGrid.CheckedChanged += (_, _) => { _canvas.ShowGrid = _showGrid.Checked; _canvas.Invalidate(); if (_view3d is not null) { _view3d.ShowGrid = _showGrid.Checked; _view3d.Invalidate(); } };
        _showObjects.CheckedChanged += (_, _) => { _canvas.ShowObjects = _showObjects.Checked; _canvas.Invalidate(); if (_view3d is not null) { _view3d.ShowObjects = _showObjects.Checked; _view3d.Invalidate(); } };
        _gameLighting.CheckedChanged += (_, _) => { _gameHour.Enabled = _gameLighting.Checked; if (_view3d is not null) _view3d.GameLightingEnabled = _gameLighting.Checked; };
        _gameHour.ValueChanged += (_, _) => { if (_view3d is not null) _view3d.GameHour = (float)_gameHour.Value; };
        _reliefScale.ValueChanged += (_, _) => _view3d?.SetReliefScale(_reliefScale.Value / 100f);
        _waterColorButton.Click += (_, _) => ChooseWaterColor();
        _waterLevel.ValueChanged += (_, _) => UpdateWaterPreview();
        _sceneList.SelectedIndexChanged += (_, _) => SelectSceneObject();
        _sceneApplyButton.Click += (_, _) => ApplySelectedSceneObjectEdit();
        _sceneRestoreButton.Click += (_, _) => RestoreOpeningSceneObjects();
        _sceneDuplicateButton.Click += (_, _) => DuplicateSelectedSceneObject();
        _sceneDeleteButton.Click += (_, _) => ToggleDeleteSelectedSceneObject();
        _sceneAddButton.Click += (_, _) => AddSceneObjectFromCatalog();
        _sceneTranslateButton.Click += (_, _) => TranslateSelectedSettlement();
        _canvas.TexturePainted += (_, e) => PaintTexture(e);
        _canvas.TextureSampled += (_, e) => SelectSampledTexture(e.Texture);
        _canvas.StrokeEnded += (_, _) => CommitStroke();
        _canvas.TileHovered += (_, e) => ShowTerrainHover(e);
        _canvas.SceneObjectMoved += (_, e) => MoveSelectedSceneObject(e);
        _canvas.ViewChanged += (_, _) => _overview.Invalidate();
        if (_view3d is not null)
        {
            Map3DViewControl view3d = _view3d;
            _view3d.TexturePainted += (_, e) => PaintTexture(e);
            _view3d.TextureSampled += (_, e) => SelectSampledTexture(e.Texture);
            _view3d.StrokeEnded += (_, _) => CommitStroke();
            _view3d.TileHovered += (_, e) => ShowTerrainHover(e);
            _view3d.SceneObjectMoved += (_, e) => MoveSelectedSceneObject(e);
            _view3d.SceneObjectPicked += (_, e) => SelectPickedSceneObject(e.SceneObject);
            _view3d.CameraChanged += (_, _) => _overview.Invalidate();
            _view3d.InitializationFailed += (_, ex) => { if (!IsDisposed && !Disposing && IsHandleCreated) BeginInvoke(() => { if (!view3d.IsReady) Disable3DView(view3d.LastFailureReason ?? (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "OpenGL 3.3 initialization failed." : "OpenGL 3.3 初始化失敗。"), ex); }); };
        }
        _view2dButton.Click += (_, _) => SetActiveView(use3D: false);
        _view3dButton.Click += (_, _) => SetActiveView(use3D: true);
        _3dDiagnosticsButton.Click += (_, _) => Show3DDiagnostics();
        _retryDisplayButton.Click += (_, _) => TryReloadDisplayResources(out _);
        _mapMenuButton.Click += (_, _) => ReturnToMenu();
        _textureTool.Click += (_, _) => SetEditMode(EditMode.Texture);
        _sceneMoveTool.Click += (_, _) => SetEditMode(EditMode.SceneMove);
        _heightTool.Click += (_, _) => SetEditMode(EditMode.Height);
        _collisionTool.Click += (_, _) => SetEditMode(EditMode.Collision);
        _placeTool.Click += (_, _) => SetEditMode(EditMode.PlaceObject);
        _natureTool.Click += (_, _) => SetEditMode(EditMode.Nature);
        _natureCategory.SelectedIndexChanged += (_, _) => RefreshNatureTypes();
        _placeCategory.SelectedIndexChanged += (_, _) => RefreshPlacementTypes();
        _placeTribe.SelectedIndexChanged += (_, _) => RefreshPlacementTypes();
        _placeTypes.SelectedIndexChanged += (_, _) => { _placeCount.Enabled = (_placeTypes.SelectedItem as PlacementTypeItem)?.Type.Category == SdlObjectCategory.Figure; UpdatePlacementPreview(); };
        _placeTeam.ValueChanged += (_, _) => UpdatePlacementPreview();
        _placeAngle.ValueChanged += (_, _) => UpdatePlacementPreview();
        _placedList.SelectedIndexChanged += (_, _) => _placedDeleteButton.Enabled = _selected?.IsCustom == true && _placedList.SelectedItems.Count > 0;
        _placedDeleteButton.Click += (_, _) => DeleteSelectedPlacedObjects();
        _resetTerrainButton.Click += (_, _) => ResetTerrain();
        _saveButton.Click += (_, _) => SaveMap(showSuccess: true);
        _gamePreviewButton.Click += (_, _) => PreviewInGame();
        _undoButton.Click += (_, _) => Undo(); _redoButton.Click += (_, _) => Redo();
        _aiMapButton.Click += (_, _) => OpenAiMapDialog();
        _blankTerrainButton.Click += (_, _) => ApplyBlankTerrain(confirm: true);
        _regionToolsButton.Click += (_, _) => RunRegionTool();
        InitializeLayoutTools();
        _boxSelectButton.Click += (_, _) => BeginBoxSelection();

        _btnLangZH.Click += (s, e) => {
            if (AgainstRomeModifier.Loc.CurrentLanguage != AgainstRomeModifier.Language.TraditionalChinese) {
                AgainstRomeModifier.Loc.CurrentLanguage = AgainstRomeModifier.Language.TraditionalChinese;
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
            }
        };
        _btnLangEN.Click += (s, e) => {
            if (AgainstRomeModifier.Loc.CurrentLanguage != AgainstRomeModifier.Language.English) {
                AgainstRomeModifier.Loc.CurrentLanguage = AgainstRomeModifier.Language.English;
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
            }
        };

        KeyDown += (_, e) => HandleShortcut(e);
        foreach (Control control in EditablePropertyControls())
        {
            if (control is TextBox text) text.TextChanged += (_, _) => { UpdateGameTextWarning(text); MarkDirty(); };
            else if (control is NumericUpDown numeric) numeric.ValueChanged += (_, _) => MarkDirty();
            else if (control is CheckBox check) check.CheckedChanged += (_, _) => MarkDirty();
        }
    }

    private void UpdateLanguageButtonStyles()
    {
        bool isZh = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.TraditionalChinese;
        _btnLangZH.Checked = isZh;
        _btnLangEN.Checked = !isZh;
    }

    private void ApplyLanguageToUI()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        Text = isEn ? "Against Rome Map Editor" : "Against Rome 地圖編輯器";

        _mapMenuButton.Text = isEn ? "Map Menu" : "地圖選單";
        _saveButton.Text = isEn ? "Save" : "儲存";
        _gamePreviewButton.Text = isEn ? "Test in Game" : "選用：啟動遊戲測試";
        _undoButton.Text = isEn ? "Undo" : "復原";
        _redoButton.Text = isEn ? "Redo" : "重做";

        _lblTerrainGroup.Text = isEn ? "Terrain:" : "地表：";
        _textureTool.Text = isEn ? "Texture Brush" : "材質筆刷";
        _sceneMoveTool.Text = isEn ? "Move Selected Object" : "移動選取物件";
        _sceneMoveTool.ToolTipText = isEn
            ? "Select an SDL object in the Scene Objects tab, then drag on the 2D or 3D terrain. Movement stays pending until Save."
            : "先在「場景物件」分頁選取 SDL 物件，再於 2D 或 3D 地表拖曳；移動會暫存到按下「儲存」為止。";
        _resetTerrainButton.Text = isEn ? "Reset Terrain" : "還原地表";
        _heightTool.Text = isEn ? "Terrain Height" : "地形高度";
        _placeTool.Text = isEn ? "Place Objects" : "放置物件";
        _natureTool.Text = isEn ? "Nature" : "自然物件";
        _natureTool.ToolTipText = isEn ? "Plant or remove trees, grass, bushes and reeds (map world objects)." : "種植或移除樹木、草叢、灌木與蘆葦（地圖世界物件）。";
        LocalizeNatureTab(isEn);
        _placeTool.ToolTipText = isEn
            ? "Place buildings, unit groups and characters that exist when the map loads (like a scenario editor)."
            : "像場景編輯器一樣放置建築、部隊與人物，地圖載入時就會存在。";
        LocalizePlacementTab(isEn);
        LocalizeEvents(isEn);
        _aiMapButton.Text = isEn ? "AI Map Maker…" : "AI 製圖…";
        _blankTerrainButton.Text = isEn ? "Reset Flat Terrain…" : "重設平坦地形…";
        _regionToolsButton.Text = isEn ? "Region tools…" : "區域工具…";
        _layoutMenu.Text = isEn ? "Layouts" : "配置";
        _exportPlacementLayout.Text = isEn ? "Save selected settlement / objects…" : "保存選取的聚落／物件…";
        _exportNatureLayout.Text = isEn ? "Save forest region…" : "保存森林區域…";
        _importLayout.Text = isEn ? "Load and apply layout…" : "載入並套用配置…";
        _boxSelectButton.Text = isEn ? "Box select" : "框選";
        _boxSelectButton.ToolTipText = isEn ? "Drag a rectangle to select placed objects; Escape cancels." : "拖曳矩形選取放置物件；Escape 取消。";
        _blankTerrainButton.ToolTipText = isEn
            ? "Flatten the whole map just above the water level, paint one base material, clear blocked ground, and reset vertex colors / smoothing / lighting on Save. Settlements are kept."
            : "整張地圖整平到略高於水面、鋪單一基礎材質、清除阻擋區，儲存時重設頂點色、平滑遮罩與光照；聚落保留。";
        _aiMapButton.ToolTipText = isEn
            ? "Describe the map in words; a local Ollama model plans hills, rivers, lakes, materials and blocked areas, then the editor applies them (undoable, not saved until you Save)."
            : "用文字描述地圖，由本機 Ollama 模型規劃山丘、河流、湖泊、材質與阻擋區，再由編輯器套用（可復原，按「儲存」才寫入）。";
        _heightTool.ToolTipText = isEn
            ? "Raise, lower, smooth or flatten the terrain (boden.bmp). Lighting is re-baked and height caches are rebuilt by the game."
            : "升高、降低、平滑或整平地形（boden.bmp）；儲存時重烘光照並讓遊戲重建高度快取。";
        _collisionTool.Text = isEn ? "Passability" : "通行區域";
        _collisionTool.ToolTipText = isEn
            ? "Paint blocked / passable ground (collision.bmp). Red overlay = blocked."
            : "繪製阻擋／可通行地面（collision.bmp）；紅色疊圖為阻擋。";
        PopulateTerrainToolOptions();
        _view2dButton.Text = isEn ? "2D View" : "2D 俯視";
        _view3dButton.Text = isEn ? "3D View" : "3D 場景";
        _3dDiagnosticsButton.Text = isEn ? "3D Diagnostics" : "3D 診斷";
        _retryDisplayButton.Text = isEn ? "Retry display" : "重試顯示";
        _retryDisplayButton.ToolTipText = isEn ? "Reload display resources and keep unsaved edits." : "重新載入顯示素材，保留未儲存的編輯。";

        _paletteHeader.Text = isEn ? "Terrain Painting" : "地表繪製";
        _paletteSearch.PlaceholderText = isEn ? "Search grass, sand, mud, rock..." : "搜尋草地、沙地、泥土、岩地…";
        _lblBrushInstructions.Text = isEn ? "Right-click: Sample | Left-click & drag: Draw." : "右鍵取樣，左鍵拖曳繪製。";

        _lblBrushSizeTitle.Text = isEn ? "Brush Size" : "筆刷大小";
        _showGrid.Text = isEn ? "Show grid" : "顯示格線";
        _showObjects.Text = isEn ? "Show buildings and objects" : "顯示建築與場景物件";
        _gameLighting.Text = isEn ? "Game lighting" : "遊戲光照";
        _gameHourLabel.Text = isEn ? "Hour" : "時刻";
        _autoBridge.Text = isEn ? "Auto transition (insert bridge materials)" : "自動過渡（插入中介材質）";
        _stampMode.Text = isEn ? "Tile stamp (original roads, rivers, cliffs...)" : "圖塊印章（原版道路、河流、岩壁等）";
        _stampOtherRegions.Text = isEn ? "Show other regions' tiles" : "顯示其他地區圖塊";
        _lblReliefScaleTitle.Text = isEn ? "Relief Scaling (Approx)" : "地形起伏（近似顯示）";

        _brushSize.Items.Clear();
        _brushSize.Items.AddRange(isEn
            ? new object[] { "Fine (1 tile)", "Medium (3 x 3)", "Large (5 x 5)", "Huge (9 x 9)", "Region (15 x 15)" }
            : new object[] { "精細（1 格）", "中型（3 × 3）", "大型（5 × 5）", "特大（9 × 9）", "區域（15 × 15）" });
        _brushSize.SelectedIndex = Math.Max(0, Array.IndexOf(BrushSizes, _canvas.BrushSize));

        if (_inspectorTabs.TabPages.Count >= 3)
        {
            _inspectorTabs.TabPages[0].Text = isEn ? "Terrain" : "地表";
            _inspectorTabs.TabPages[1].Text = isEn ? "Map Properties" : "地圖屬性";
            _inspectorTabs.TabPages[2].Text = isEn ? "Scene Objects" : "場景物件";
            if (_inspectorTabs.TabPages.Count >= 4) _inspectorTabs.TabPages[3].Text = isEn ? "Place Objects" : "放置物件";
            if (_inspectorTabs.TabPages.Count >= 5) _inspectorTabs.TabPages[4].Text = isEn ? "Nature" : "自然物件";
        }

        if (_lblTitle != null) _lblTitle.Text = isEn ? "Map Title" : "地圖名稱";
        if (_lblSubtitle != null) _lblSubtitle.Text = isEn ? "Map Subtitle" : "地圖副標題";
        if (_lblBriefing != null) _lblBriefing.Text = isEn ? "Briefing Text" : "任務說明";
        if (_lblTeamNamesTitle != null) _lblTeamNamesTitle.Text = isEn ? "Team Names" : "隊伍名稱";
        if (_lblWaterLevel != null) _lblWaterLevel.Text = isEn ? "Water Level" : "水面高度";
        if (_lblWaterColor != null) _lblWaterColor.Text = isEn ? "Water Color" : "水面顏色";
        if (_lblWaterWarpShift != null) _lblWaterWarpShift.Text = isEn ? "Water Warp Shift" : "水面波動位移";
        if (_lblWaterBumpAmplitude != null) _lblWaterBumpAmplitude.Text = isEn ? "Water Bump Amplitude" : "水面凹凸幅度";
        if (_lblWaterBumpFrequency != null) _lblWaterBumpFrequency.Text = isEn ? "Water Bump Frequency" : "水面凹凸頻率";
        if (_lblFlashProbability != null) _lblFlashProbability.Text = isEn ? "Lightning Prob/sec" : "每秒閃電機率";
        if (_lblDayStart != null) _lblDayStart.Text = isEn ? "Sunrise Time" : "日出時間";
        if (_lblDayEnd != null) _lblDayEnd.Text = isEn ? "Sunset Time" : "日落時間";
        _rain.Text = isEn ? "Rain on Water" : "水面雨滴";
        _waterColorButton.Text = isEn ? "Choose Color..." : "選擇水面顏色…";

        for (int i = 0; i < _teamLabels.Length; i++)
        {
            if (_teamLabels[i] != null)
                _teamLabels[i].Text = isEn ? $"Team {i}" : $"隊伍 {i}";
        }

        if (_sceneList.Columns.Count >= 4)
        {
            _sceneList.Columns[0].Text = isEn ? "Type" : "類型";
            _sceneList.Columns[1].Text = isEn ? "Object" : "物件";
            _sceneList.Columns[2].Text = isEn ? "Team" : "隊伍";
            _sceneList.Columns[3].Text = isEn ? "Source" : "來源";
        }

        _sceneWarningLabel.Text = isEn
            ? "SDL editing: drag or type positions, change team/angle, copy, add by type, delete objects, or move a whole settlement on custom maps."
            : "SDL 編輯：可拖曳或輸入座標、修改隊伍與角度、複製、依類型新增、刪除物件，或整體平移聚落。";
        _lblSceneTeam.Text = isEn ? "Team" : "隊伍";
        _lblSceneX.Text = isEn ? "Rel X" : "相對 X";
        _lblSceneY.Text = isEn ? "Rel Y" : "相對 Y";
        _lblSceneZ.Text = isEn ? "Rel Z" : "相對 Z";
        _lblSceneAngle.Text = isEn ? "Angle" : "角度";

        _sceneApplyButton.Text = isEn ? "Apply to Buffer" : "套用至待儲存";
        _sceneRestoreButton.Text = isEn ? "Restore to Initial" : "還原到本次開啟時";
        _sceneDuplicateButton.Text = isEn ? "Copy Object" : "複製物件";
        _sceneAddButton.Text = isEn ? "Add Object…" : "新增物件…";
        _sceneTranslateButton.Text = isEn ? "Move Settlement…" : "平移聚落…";
        UpdateSceneEditButtons();

        _lblOverviewTitle.Text = isEn ? "Map Overview" : "地圖概覽";
        _lblStatusInstructions.Text = isEn
            ? "  Scroll: Zoom | Mid-Drag: Pan | Brush: Draw | Move Object: Drag selected SDL object | Ctrl+S: Save"
            : "  滾輪縮放　中鍵平移　材質筆刷：繪製　移動物件：拖曳選取的 SDL 物件　Ctrl+S 儲存";

        UpdateStatus();
        foreach (TextBox text in EditablePropertyControls().OfType<TextBox>()) UpdateGameTextWarning(text);
        UpdateEditorState();
        UpdatePaletteBrushLabel();
        LocalizeMapDiagnostics(isEn);

        if (_selected is not null)
        {
            _currentMapLabel.Text = isEn
                ? $"Current Map: {_selected.DisplayName ?? _selected.Id} ({_selected.Id})"
                : $"目前地圖：{_selected.DisplayName ?? _selected.Id}（{_selected.Id}）";
        }
    }

    private void LoadSelectedMap()
    {
        if (_selected is null) { UpdateEditorState(); return; }
        try
        {
            _loading = true; string map = _selected.DirectoryPath;
            _boxSelectButton.Checked = false; _boxStart = _boxEnd = null; ShowBoxSelection(null); UpdateContinuousStampPainting();
            _lastStampTile = null;
            var put = PutTextDocument.Load(Path.Combine(map, "TEXT", "US", "briefing.put"));
            _title.Text = put.GetValue("briefing_titel_1") ?? "";
            _subtitle.Text = put.GetValue("briefing_titel_2") ?? "";
            _briefing.Text = put.GetCompositeValue("briefing_text") ?? "";
            for (int index = 0; index < _teamNames.Length; index++) _teamNames[index].Text = put.GetValue($"briefing_text_teamname{index}") ?? "";
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini"));
            _waterLevel.Value = ParseDecimal(ini.GetValue("Waterlevel"), _waterLevel); _waterColor.Text = ini.GetValue("WaterColor") ?? "";
            _waterWarpShift.Value = ParseDecimal(ini.GetValue("WaterWarpShift"), _waterWarpShift);
            _waterBumpAmplitude.Value = ParseDecimal(ini.GetValue("WaterBumpAmplitude"), _waterBumpAmplitude);
            _waterBumpFrequency.Value = ParseDecimal(ini.GetValue("WaterBumpFrequency"), _waterBumpFrequency);
            _flashProbability.Value = ParseDecimal(ini.GetValue("FlashPropability"), _flashProbability);
            _heightMapStep = float.TryParse(ini.GetValue("Heightmapstep"), NumberStyles.Float, CultureInfo.InvariantCulture, out float heightStep) && float.IsFinite(heightStep) && heightStep > 0 ? heightStep : 4;
            if (TryParseGameColor(_waterColor.Text, out Color waterColor)) { _waterColorButton.BackColor = waterColor; _waterColorButton.ForeColor = waterColor.GetBrightness() < .45f ? Color.White : Color.Black; }
            _dayStart.Value = ParseDecimal(ini.GetValue("DayStartTime"), _dayStart); _dayEnd.Value = ParseDecimal(ini.GetValue("DayEndTime"), _dayEnd); _rain.Checked = ini.GetValue("RainDropsOnWater") == "1";
            _texturesDocument = BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")); _savedTextures = _texturesDocument.Textures.ToArray(); InitializeTerrainBlendSession(); InitializeTerrainLayers(map); _propertyDirty = false;
            _sceneObjects = SdlSceneCatalog.LoadDirectory(map); _sceneOriginalObjects = _sceneObjects.ToArray(); _sceneSavedObjects = _sceneObjects.ToArray(); _sceneLoaded = true;
            _sceneRemovals.Clear(); _sceneAdditions.Clear(); _settlementOffsets.Clear();
            _settlementOrigins = SdlSceneCatalog.LoadSettlementOrigins(map);
            if (_objectCatalog.Count == 0) _objectCatalog = BuildSpawnCatalog(_gamePath);
            _placementSession.Load(LoadScenarioPlacements(map).ToArray());
            PopulatePlacementFilters(); RefreshPlacedList();
            LoadEvents(map);
            if (_objdefNames.Count == 0) _objdefNames = ObjDefNames.Load(_gamePath);
            LoadLevelObjects(map);
            LoadPalette(); LoadEditingScene(); UpdateEditorState();

            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            _currentMapLabel.Text = isEn
                ? $"Current Map: {_selected.DisplayName ?? _selected.Id} ({_selected.Id})"
                : $"目前地圖：{_selected.DisplayName ?? _selected.Id}（{_selected.Id}）";
        }
        catch (Exception ex) { ShowError(ex); }
        finally { _loading = false; }
    }

    private void LoadEditingScene(bool preserveView = false)
    {
        if (_selected is null) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string minimapPath = Path.Combine(_selected.DirectoryPath, "minimap.bmp");
        LoadSceneList(_sceneObjects);
        IReadOnlyList<MapSceneObject> effectiveObjects = EffectiveSceneObjects();
        if (!TryParseGameColor(_waterColor.Text, out Color sceneWaterColor)) sceneWaterColor = Color.SteelBlue;
        _canvas.SpriteCatalog = _spriteCatalog; // 2D 也顯示原生 sprite，3D 不可用時同樣有效
        bool hasRealTextures = _texturesDocument is not null && _floorTextures is not null && _canvas.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _savedTextures, _selected.DirectoryPath, _floorTextures, effectiveObjects, (float)_waterLevel.Value, _heightMapStep, sceneWaterColor, preserveView);
        bool has3DScene = false;
        if (_view3d is not null && _texturesDocument is not null && _floorTextures is not null)
        {
            _view3d.BrushTexture = _canvas.BrushTexture;
            _view3d.BrushSize = _canvas.BrushSize;
            _view3d.ShowGrid = _showGrid.Checked;
            _view3d.ShowObjects = _showObjects.Checked;
            _view3d.EditingEnabled = _selected.IsCustom;
            _view3d.SpriteCatalog = _spriteCatalog;
            _view3d.ShadowCatalog = _shadowCatalog;
            _view3d.LightingContext = _lightingContext;
            _view3d.GameLightingEnabled = _gameLighting.Checked;
            _view3d.GameHour = (float)_gameHour.Value;
            try { has3DScene = _view3d.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _selected.DirectoryPath, _floorTextures, SceneObjectsFor3D(effectiveObjects), (float)_waterLevel.Value, _heightMapStep, sceneWaterColor); _view3d.SetReliefScale(_reliefScale.Value / 100f); }
            catch (Exception ex) { Disable3DView(isEn ? "Failed to load 3D map resources." : "載入 3D 地圖資源失敗。", ex); }
        }
        if (has3DScene && _view3d?.IsReady == true)
        {
            _view3dButton.Enabled = true;
            _view3dButton.ToolTipText = "";
            _3dDiagnosticsButton.Visible = false;
            _retryDisplayButton.Visible = false;
            _last3DDiagnostic = null;
        }
        if (_terrainLayers?.HeightsDirty == true) ApplyHeightsToViews();
        UpdateCollisionOverlay();
        Image? oldOverview = _overview.Image; _overview.Image = null; oldOverview?.Dispose();
        if (File.Exists(minimapPath)) using (var source = new Bitmap(minimapPath)) _overview.Image = new Bitmap(source);
        _canvas.EditingEnabled = _selected.IsCustom;

        if (hasRealTextures)
        {
            if (_view3dButton.Checked && has3DScene)
            {
                _modeBanner.Text = _selected.IsCustom
                    ? (isEn ? $"Offline 3D Scene (Approx. View) - Real terrain & {_canvas.SceneObjectCount} scene objects" : $"離線 3D 場景（近似顯示）- 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件")
                    : (isEn ? $"Offline 3D Scene (Approx. View) - Original map read-only, contains {_canvas.SceneObjectCount} scene objects" : $"離線 3D 場景（近似顯示）- 原廠地圖僅供瀏覽，含 {_canvas.SceneObjectCount} 個場景物件");
            }
            else
            {
                _modeBanner.Text = _selected.IsCustom
                    ? (isEn ? $"Offline Map Scene - Real terrain & {_canvas.SceneObjectCount} scene objects; Wheel: Zoom, Middle Drag: Pan" : $"離線地圖場景 - 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件；滾輪縮放，中鍵平移")
                    : (isEn ? $"Offline Map Scene - Real terrain & {_canvas.SceneObjectCount} scene objects; Original map read-only" : $"離線地圖場景 - 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件；原廠地圖僅供瀏覽");
            }
        }
        else
        {
            _modeBanner.Text = isEn
                ? "floortex.dat not found, only simplified terrain can be displayed; please select a complete game folder"
                : "找不到 floortex.dat，目前只能顯示簡化地表；請選擇完整的遊戲資料夾";
        }

        _modeBanner.BackColor = _canvas.EditingEnabled ? Color.FromArgb(38, 95, 72) : Color.FromArgb(86, 69, 40);
        if (!has3DScene)
        {
            if (_view3d is not null && _view3dButton.Enabled) Disable3DView(_view3d.LastFailureReason ?? (isEn ? "3D scene lacks required resources." : "3D 場景缺少必要資源。"));
            else
            {
                SetActiveView(use3D: false);
                if (_last3DDiagnostic is not null)
                {
                    _modeBanner.Text = isEn
                        ? "3D unavailable; please click \"3D Diagnostics\" to view the actual reasons."
                        : "3D 無法啟用；請按「3D 診斷」查看實際原因。";
                    _modeBanner.BackColor = Color.FromArgb(86, 69, 40);
                }
            }
        }
        UpdateStatus();
    }

    private void PaintTexture(TexturePaintEventArgs e)
    {
        if (_ignoreCancelledBoxStroke) return;
        if (_boxSelectButton.Checked)
        {
            _boxStart ??= (e.X, e.Y); _boxEnd = (e.X, e.Y);
            var start = _boxStart.Value;
            ShowBoxSelection(Rectangle.FromLTRB(Math.Min(start.X, e.X), Math.Min(start.Y, e.Y), Math.Max(start.X, e.X) + 1, Math.Max(start.Y, e.Y) + 1));
            return;
        }
        if (TerrainLayerMode) { PaintTerrainLayer(e); return; }
        if (_editMode == EditMode.PlaceObject) { PlaceObjectAt(e); return; }
        if (_editMode == EditMode.Nature) { PaintNature(e); return; }
        if (_selected is null || !_selected.IsCustom || _texturesDocument is null || _terrainBlendSession is null) return;
        if (_stampMode.Checked)
        {
            if (_stampTexture is null) return;
            // 不把無效游標位置連回地圖；滑鼠兩次回報之間沿用地形工具的路徑補點。
            int dimension = _texturesDocument.Dimension;
            if (e.X < 0 || e.Y < 0 || e.X >= dimension || e.Y >= dimension) { _lastStampTile = null; return; }
            IEnumerable<(int X, int Y)> tiles = _lastStampTile is { } last
                ? TerrainStrokePath.Between(last.X, last.Y, e.X, e.Y) : new[] { (e.X, e.Y) };
            _lastStampTile = (e.X, e.Y);
            bool changed = false;
            foreach (var tile in tiles)
            {
                if (_terrainBlendSession.StampTexture(tile.X, tile.Y, _stampTexture) is not { } stamped) continue;
                ApplyTexture(stamped.X, stamped.Y, stamped.After);
                changed = true;
            }
            if (changed) { _terrainBlendNotice = null; UpdateEditorState(); }
            return;
        }
        if (_activeMaterial is null) return;
        float radius = _canvas.BrushSize / 2f + .26f;
        TerrainBlendPaintResult result = _terrainBlendSession.PaintCircle(e.X + .5f, e.Y + .5f, radius, _activeMaterial.Id, autoBridge: _autoBridge.Checked);
        foreach (TerrainTextureChange change in result.TextureChanges) ApplyTexture(change.X, change.Y, change.After);
        _terrainBlendNotice = result.Succeeded
            ? null
            : _autoBridge.Checked
                ? $"此筆觸即使插入中介材質也無法由原版 transition tile 完整表達，已整筆復原（{result.Issues.Count} 格）。"
                : $"此筆觸無法由原版 transition tile 完整表達，已整筆復原（{result.Issues.Count} 格）；可勾選「自動過渡」。";
        UpdateEditorState();
    }

    // 一次筆畫（滑鼠按下到放開）內觸及的所有格子合併為單一 undo 項目。
    private void CommitStroke()
    {
        _ignoreCancelledBoxStroke = false;
        if (_boxSelectButton.Checked && _boxStart is { } start && _boxEnd is { } end)
        {
            _boxStart = _boxEnd = null; _boxSelectButton.Checked = false;
            ApplyRegionTool(TerrainRegionOperation.SelectObjects, [start, end], 1);
        }
        _lastStampTile = null;
        _flattenTarget = -1; _roughnessSeed = Random.Shared.Next(); _lastTerrainTile = null; _terrainStrokeTiles.Clear();
        if (_natureSession.CommitStroke()) UpdateEditorState();
        if (_terrainLayers?.CommitStroke() == true) UpdateEditorState();
        if (_terrainBlendSession?.CommitStroke() != true) return;
        UpdateEditorState();
    }

    private void ApplyTexture(int x, int y, string texture)
    {
        _texturesDocument!.SetTexture(x, y, texture); _canvas.SetTexture(x, y, texture); _view3d?.SetTexture(x, y, texture);
    }

    private void Undo()
    {
        CommitStroke();
        if (_editMode == EditMode.PlaceObject)
        {
            if (_placementSession.Undo()) { RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState(); }
            return;
        }
        if (_editMode == EditMode.Nature)
        {
            if (_natureSession.Undo()) { RefreshSceneMarkers(); UpdateEditorState(); }
            return;
        }
        // 移動模式中可在 3D 點選刪除自然物件：先復原那類變更，沒有時才退回材質筆畫。
        if (_editMode == EditMode.SceneMove && _natureSession.Undo()) { RefreshSceneMarkers(); UpdateEditorState(); return; }
        if (TerrainLayerMode)
        {
            if (_terrainLayers?.Undo() is { } undone) { ApplyTerrainLayerStroke(undone); UpdateEditorState(); }
            return;
        }
        if (_texturesDocument is null || _terrainBlendSession?.Undo() is not { } stroke) return;
        foreach (TerrainTextureChange change in stroke) ApplyTexture(change.X, change.Y, change.After);
        _terrainBlendNotice = null;
        UpdateEditorState();
    }

    private void Redo()
    {
        CommitStroke();
        if (_editMode == EditMode.PlaceObject)
        {
            if (_placementSession.Redo()) { RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState(); }
            return;
        }
        if (_editMode == EditMode.Nature)
        {
            if (_natureSession.Redo()) { RefreshSceneMarkers(); UpdateEditorState(); }
            return;
        }
        if (_editMode == EditMode.SceneMove && _natureSession.Redo()) { RefreshSceneMarkers(); UpdateEditorState(); return; }
        if (TerrainLayerMode)
        {
            if (_terrainLayers?.Redo() is { } redone) { ApplyTerrainLayerStroke(redone); UpdateEditorState(); }
            return;
        }
        if (_texturesDocument is null || _terrainBlendSession?.Redo() is not { } stroke) return;
        foreach (TerrainTextureChange change in stroke) ApplyTexture(change.X, change.Y, change.After);
        _terrainBlendNotice = null;
        UpdateEditorState();
    }

    private void UpdateWaterPreview()
    {
        if (_loading || _selected is null) return;
        if (!TryParseGameColor(_waterColor.Text, out Color color)) color = Color.SteelBlue;
        _canvas.UpdateWaterOverlay(_selected.DirectoryPath, (float)_waterLevel.Value, _heightMapStep, color);
        _view3d?.UpdateWater((float)_waterLevel.Value, color);
    }

    private MapSceneObject? SelectedSceneDisplay() => _sceneList.SelectedItems.Count != 1 ? null : _sceneList.SelectedItems[0].Tag switch
    {
        MapSceneObject item => item,
        StagedSceneAddition addition => addition.Display,
        _ => null,
    };

    private void FocusSelectedSceneObject()
    {
        if (SelectedSceneDisplay() is not { } selected) return;
        MapSceneObject item = WithSettlementOffset(selected);
        float tileX = item.WorldX / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
        float tileZ = item.WorldZ / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
        _canvas.FocusTile(tileX, tileZ);
        _view3d?.FocusTile(tileX, tileZ);
    }

    private void SelectSceneObject()
    {
        MapSceneObject? item = SelectedSceneDisplay();
        UpdateSceneEditButtons();
        if (item is null) return;
        _sceneTeam.Value = Math.Clamp(item.Team, (int)_sceneTeam.Minimum, (int)_sceneTeam.Maximum);
        _sceneX.Value = ClampSceneCoordinate(item.LocalX, _sceneX);
        _sceneY.Value = ClampSceneCoordinate(item.LocalY, _sceneY);
        _sceneZ.Value = ClampSceneCoordinate(item.LocalZ, _sceneZ);
        _sceneAngle.Value = Math.Clamp((decimal)(item.Angle ?? 0), _sceneAngle.Minimum, _sceneAngle.Maximum);
        FocusSelectedSceneObject();
    }

    /// <summary>筆刷邊長（tile）；9／15 供山丘、湖泊等大範圍地形與材質一次塗佈。</summary>
    private static readonly int[] BrushSizes = [1, 3, 5, 9, 15];
    private enum EditMode { Texture, SceneMove, Height, Collision, PlaceObject, Nature }
    private EditMode _editMode = EditMode.Texture;
    private const string TerrainToolBrushToken = "\u0001terrain-tool"; // 讓視圖在未取樣材質時仍送出筆刷事件；不是材質名稱。
    private bool TerrainLayerMode => _editMode is EditMode.Height or EditMode.Collision;

    private void SetEditMode(EditMode mode)
    {
        bool cancelledBox = _boxSelectButton.Checked && _boxStart.HasValue;
        _boxSelectButton.Checked = false; _boxStart = _boxEnd = null;
        ShowBoxSelection(null);
        CommitStroke();
        _ignoreCancelledBoxStroke = cancelledBox;
        _editMode = mode;
        UpdateContinuousStampPainting();
        _textureTool.Checked = mode == EditMode.Texture;
        _sceneMoveTool.Checked = mode == EditMode.SceneMove;
        _heightTool.Checked = mode == EditMode.Height;
        _collisionTool.Checked = mode == EditMode.Collision;
        _placeTool.Checked = mode == EditMode.PlaceObject;
        _natureTool.Checked = mode == EditMode.Nature;
        if (mode == EditMode.PlaceObject && _inspectorTabs.TabPages.Count >= 4) _inspectorTabs.SelectedIndex = 3;
        if (mode == EditMode.Nature) { EnsureNatureCatalog(); if (_inspectorTabs.TabPages.Count >= 5) _inspectorTabs.SelectedIndex = 4; }
        if (mode == EditMode.SceneMove && _inspectorTabs.TabPages.Count >= 3) _inspectorTabs.SelectedIndex = 2;
        PopulateTerrainToolOptions();
        bool needsToken = TerrainLayerMode || mode is EditMode.PlaceObject or EditMode.Nature;
        if (needsToken && string.IsNullOrWhiteSpace(_canvas.BrushTexture)) SetBrushToken(TerrainToolBrushToken);
        else if (!needsToken && _canvas.BrushTexture == TerrainToolBrushToken) SetBrushToken(null);
        UpdateCollisionOverlay();
        RefreshSceneMarkers(); // 自然物件標記只在自然物件模式顯示
        UpdatePlacementPreview();
        UpdateSceneEditButtons();
        UpdateEditorState();
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _status.Text = mode switch
        {
            EditMode.SceneMove => isEn ? "Move mode: click an object in 3D (or pick it in the list) and drag it; Delete removes it. Arrow keys scroll, Home = game scale. Changes remain pending until Save." : "物件移動模式：在 3D 直接點選物件（或從清單選取）後拖曳，Delete 刪除；方向鍵捲動、Home 切回遊戲比例。按下「儲存」前只會暫存在記憶體。",
            EditMode.Height => _terrainLayers is null
                ? (isEn ? "This map has no editable boden.bmp height map." : "此地圖沒有可編輯的 boden.bmp 高度圖。")
                : (isEn ? "Height mode: left-drag to apply the selected operation; Ctrl+Z / Ctrl+Y undo and redo." : "地形高度模式：左鍵拖曳套用所選操作；Ctrl+Z／Ctrl+Y 復原與重做。"),
            EditMode.Collision => _terrainLayers?.HasCollision != true
                ? (isEn ? "This map has no editable collision.bmp." : "此地圖沒有可編輯的 collision.bmp。")
                : (isEn ? "Passability mode: red = blocked. Left-drag to block or clear (2D and 3D views show the overlay)." : "通行區域模式：紅色為阻擋；左鍵拖曳設定阻擋或可通行（2D／3D 檢視顯示疊圖）。"),
            EditMode.Nature => isEn
                ? "Nature mode: drag to plant the selected tree/grass/bush (one per tile), or switch to Remove and drag to clear landscape objects."
                : "自然物件模式：拖曳以種植所選的樹木／草叢／灌木（每格一株），或切換為「移除」後拖曳清除地景物件。",
            EditMode.PlaceObject => isEn
                ? "Place mode: pick an object type in the Place Objects tab, then click the map. Placed objects exist when the map loads."
                : "放置模式：在「放置物件」分頁選類型後點擊地圖；放置的物件會在地圖載入時直接存在。",
            _ => isEn ? "Texture brush mode." : "材質筆刷模式：右鍵取樣，左鍵拖曳繪製。",
        };
    }

    private void SetBrushToken(string? token)
    {
        _canvas.BrushTexture = token;
        if (_view3d is not null) _view3d.BrushTexture = token;
    }

    private void ReturnToMenu()
    {
        if (!ConfirmDiscardOrSave()) return;
        ReturnToMapMenu = true; _allowClose = true; Close();
    }

    private string GetLocalizedMaterialName(FloorMaterial material)
    {
        if (AgainstRomeModifier.Loc.CurrentLanguage != AgainstRomeModifier.Language.English)
            return material.DisplayName;

        return material.Id.ToUpperInvariant() switch {
            FloorMaterialCatalog.PathMaterialId => "Dirt Path",
            "BB" => "Grass",
            "BA" => "Dark Green Grass",
            "BC" => "Bright Green Grass",
            "BD" => "Olive Grass",
            "BW" => "Light Green Grass",
            "BS" => "Weed Grass",
            "BM" => "Sparse Grass",
            "BT" => "Muddy Grass",
            "BU" => "Dry Grass",
            "BE" => "Withered Grass",
            "BX" => "Red Clay Grass",
            "BV" => "Grey Green Wasteland",
            "BL" => "Light Brown Wasteland",
            "B5" => "Sand",
            "BJ" => "Yellow Soil",
            "B2" => "Dry Soil",
            "B4" => "Yellow Brown Soil",
            "B6" => "Dark Mud",
            "B7" => "Rough Mud",
            "B9" => "Light Mud",
            "B1" => "Light Grey Mud",
            "BI" => "Grey Brown Soil",
            "B8" => "Dark Brown Gravel",
            "B3" => "Grey Rock",
            "BG" => "Gravel",
            "BK" => "Grey Scree",
            "BR" => "Weathered Rock",
            "BO" => "Mossy Rock",
            _ => material.DisplayName
        };
    }

    private string GetLocalizedCategory(string category)
    {
        if (AgainstRomeModifier.Loc.CurrentLanguage != AgainstRomeModifier.Language.English)
            return category;

        return category switch {
            "草地" => "Grassland",
            "荒地" => "Wasteland",
            "沙地" => "Sand",
            "土地" => "Dirt",
            "岩地" => "Rock",
            "道路" => "Roads",
            _ => category
        };
    }

    private void LoadPalette(string? filter = null)
    {
        string? selected = (_palette.SelectedItem as PaletteItem)?.Key ?? _activeMaterial?.Id ?? _canvas.BrushTexture; _palette.Items.Clear();
        if (_texturesDocument is null) return;
        if (_stampMode.Checked)
        {
            LoadStampPalette(filter, selected);
            return;
        }
        IEnumerable<FloorMaterial> materials = _floorMaterials?.Materials ?? Array.Empty<FloorMaterial>();
        if (!string.IsNullOrWhiteSpace(filter)) materials = materials.Where(material =>
            GetLocalizedMaterialName(material).Contains(filter, StringComparison.CurrentCultureIgnoreCase) ||
            GetLocalizedCategory(material.Category).Contains(filter, StringComparison.CurrentCultureIgnoreCase));
        // 依「在這張地圖上能不能用」排序：已使用 → 可直接銜接 → 可自動過渡 → 難以銜接（加註），玩家不必逐一試。
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        IReadOnlyCollection<string> mapMaterials = MapMaterialIds();
        var ranked = materials
            .Select((material, order) => (material, order, tier: _floorMaterials is null ? 0 : _floorMaterials.MapSuitability(material.Id, mapMaterials)))
            .OrderBy(item => item.tier).ThenBy(item => item.order).ToArray();
        // 未搜尋時隱藏難以銜接的材質（多為其他地區的地表），搜尋時仍全部列出；整張圖都無法辨識時不隱藏，避免調色盤變空。
        if (string.IsNullOrWhiteSpace(filter) && ranked.Any(item => item.tier < 3)) ranked = ranked.Where(item => item.tier < 3).ToArray();
        PaletteItem[] items = ranked
            .Select(item => new PaletteItem(item.material.Id, item.material.RepresentativeTexture,
                GetLocalizedMaterialName(item.material) + (item.tier == 3 ? (isEn ? " (hard to blend on this map)" : "（此地圖難以銜接）") : ""), item.material))
            .ToArray();
        _palette.BeginUpdate();
        _palette.Items.AddRange(items.Cast<object>().ToArray());
        int index = selected is null ? -1 : Array.FindIndex(items, item => StringComparer.OrdinalIgnoreCase.Equals(item.Key, selected));
        _palette.SelectedIndex = index >= 0 ? index : items.Length > 0 && string.IsNullOrWhiteSpace(filter) ? 0 : -1;
        _palette.EndUpdate();
    }

    private void DrawPaletteItem(object? sender, DrawItemEventArgs e)
    {
        e.DrawBackground(); if (e.Index < 0 || _palette.Items[e.Index] is not PaletteItem item) return;
        Font baseFont = e.Font ?? Font;
        var thumbnail = new Rectangle(e.Bounds.X + 5, e.Bounds.Y + 4, 28, e.Bounds.Height - 8);
        Bitmap? texture = _floorTextures?.Get(item.PreviewTexture);
        if (texture is not null) { e.Graphics.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBilinear; e.Graphics.DrawImage(texture, thumbnail); }
        else { using var color = new SolidBrush(_canvas.GetTexturePreviewColor(item.PreviewTexture)); e.Graphics.FillRectangle(color, thumbnail); }
        using (var border = new Pen(Color.FromArgb(120, 0, 0, 0))) e.Graphics.DrawRectangle(border, thumbnail);
        TextRenderer.DrawText(e.Graphics, item.Name, baseFont, new Rectangle(e.Bounds.X + 40, e.Bounds.Y, e.Bounds.Width - 44, e.Bounds.Height), e.ForeColor, TextFormatFlags.VerticalCenter | TextFormatFlags.Left | TextFormatFlags.EndEllipsis);
        e.DrawFocusRectangle();
    }

    private void MarkDirty() { if (_loading || _selected is null || !_selected.IsCustom) return; _propertyDirty = true; UpdateEditorState(); }
    private string FriendlyTextureName(string? texture)
    {
        string defaultName = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Terrain Boundary" : "地表交界";
        var mat = _floorMaterials?.FindByTexture(texture);
        return mat != null ? GetLocalizedMaterialName(mat) : defaultName;
    }

    private void ShowTerrainHover(TileHoverEventArgs e)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string hover = isEn
            ? $"Tile ({e.X}, {e.Y})  {FriendlyTextureName(e.Texture)}"
            : $"格子 ({e.X}, {e.Y})　{FriendlyTextureName(e.Texture)}";
        _status.Text = _terrainBlendNotice is null ? hover : _terrainBlendNotice + "　" + hover;
    }

    /// <summary>目前地圖實際使用的材質（依 tile 解析；每次重排調色盤時重算，塗上新材質後排序隨之更新）。</summary>
    private IReadOnlyCollection<string> MapMaterialIds()
    {
        if (_texturesDocument is null || _floorMaterials is null) return Array.Empty<string>();
        return _texturesDocument.Textures
            .SelectMany(texture => _floorMaterials.TryResolveNativeCorners(texture, out var corners) ? corners : Array.Empty<string>())
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
    }

    private void LoadStampPalette(string? filter, string? selected)
    {
        IEnumerable<string> names = _floorTextures?.Names ?? Array.Empty<string>();
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        var sets = (_texturesDocument?.Textures ?? Array.Empty<string>())
            .Select(FloorMaterialCatalog.RegionalTileSet).OfType<string>().ToHashSet(StringComparer.OrdinalIgnoreCase);
        // 只依實際圖塊系列判斷，沒有 L 系列證據時不猜測地區；搜尋及手動勾選仍可使用所有圖塊。
        if (!_stampOtherRegions.Checked && string.IsNullOrWhiteSpace(filter) && sets.Count > 0)
            names = names.Where(name => FloorMaterialCatalog.RegionalTileSet(name) is not { } set || sets.Contains(set));
        PaletteItem[] items = names
            .Select(name => (Name: name, Category: StampCategory(name)))
            .Where(item => string.IsNullOrWhiteSpace(filter) || item.Name.Contains(filter, StringComparison.OrdinalIgnoreCase)
                || LocalizedStampCategory(item.Category, isEn).Contains(filter, StringComparison.CurrentCultureIgnoreCase))
            .OrderBy(item => item.Category).ThenBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .Select(item => new PaletteItem("tile:" + item.Name, item.Name, $"{LocalizedStampCategory(item.Category, isEn)}：{item.Name}", null))
            .ToArray();
        _palette.BeginUpdate();
        _palette.Items.AddRange(items.Cast<object>().ToArray());
        int index = selected is null ? -1 : Array.FindIndex(items, item => StringComparer.OrdinalIgnoreCase.Equals(item.Key, selected));
        _palette.SelectedIndex = index >= 0 ? index : items.Length > 0 && string.IsNullOrWhiteSpace(filter) ? 0 : -1;
        _palette.EndUpdate();
    }

    private void UpdateContinuousStampPainting()
    {
        _canvas.ContinuousPaint = _editMode == EditMode.Texture && _stampMode.Checked;
        if (_view3d is not null) _view3d.ContinuousPaint = _canvas.ContinuousPaint;
    }

    /// <summary>依原版 floortex 命名把圖塊分組；數字越小越常用於裝飾（排在前面）。</summary>
    internal static int StampCategory(string name)
    {
        string upper = name.ToUpperInvariant();
        bool Any(params string[] prefixes) => prefixes.Any(prefix => upper.StartsWith(prefix, StringComparison.Ordinal));
        if (Any("WEG", "H_WEG", "V_WEG", "PFAD", "PFLASTER", "LUXUSWEG", "PLATZ")) return 0;
        if (Any("FLUSS", "ERDEFLUSS")) return 1;
        if (Any("FELS", "ITA_FELS")) return 2;
        if (Any("MARMOR", "STEINBODEN", "STADT")) return 3;
        if (upper.Length > 1 && upper[0] == 'L' && char.IsDigit(upper[1])) return 5;
        if (upper.Length > 1 && char.IsDigit(upper[0])) return 6;
        return 4;
    }

    private static string LocalizedStampCategory(int category, bool isEn) => category switch
    {
        0 => isEn ? "Roads & plazas" : "道路與廣場",
        1 => isEn ? "Rivers" : "河流",
        2 => isEn ? "Cliffs" : "岩壁",
        3 => isEn ? "Paving" : "地板",
        5 => isEn ? "Regional (L)" : "地區地表 L",
        6 => isEn ? "Blend tiles" : "過渡圖塊",
        _ => isEn ? "Other" : "其他",
    };

    private void SelectBrush(PaletteItem item)
    {
        CommitStroke();
        _stampTexture = item.Material is null && item.Key.StartsWith("tile:", StringComparison.Ordinal) ? item.PreviewTexture : null;
        _activeMaterial = item.Material;
        _canvas.BrushTexture = item.PreviewTexture; if (_view3d is not null) _view3d.BrushTexture = item.PreviewTexture;
        _currentMaterialSwatch.Image = _floorTextures?.Get(item.PreviewTexture);
        _currentMaterialSwatch.BackColor = _canvas.GetTexturePreviewColor(item.PreviewTexture);
        _currentMaterialLabel.Text = (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Active Brush: " : "目前筆刷：") + item.Name;
        UpdateStatus();
    }

    private void SelectSampledTexture(string texture)
    {
        if (_stampMode.Checked)
        {
            // 印章模式取樣：直接選取游標下的原版圖塊，方便複製道路等片段。
            SelectBrush(new PaletteItem("tile:" + texture, texture, texture, null));
            return;
        }
        FloorMaterial? material = _floorMaterials?.FindByTexture(texture);
        if (material is null) {
            _status.Text = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English
                ? "This is a terrain boundary; please choose a drawing material from the right."
                : "這裡是地表交界；請直接從右側選擇要繪製的地表。";
            return;
        }
        SelectBrush(new PaletteItem(material.Id, material.RepresentativeTexture, GetLocalizedMaterialName(material), material));
    }

    private void RefreshOverview()
    {
        if (_selected is null) return;
        string minimapPath = Path.Combine(_selected.DirectoryPath, "minimap.bmp");
        Image? old = _overview.Image; _overview.Image = null; old?.Dispose();
        if (File.Exists(minimapPath)) using (var source = new Bitmap(minimapPath)) _overview.Image = new Bitmap(source);
    }

    private void NavigateMinimap(MouseEventArgs e)
    {
        if (e.Button != MouseButtons.Left || _overview.Image is null || _selected is null) return;
        int dimension = _texturesDocument?.Dimension ?? _canvas.MapDimension;
        if (dimension <= 0) dimension = 64;

        Rectangle imageBounds = MinimapNavigator.CalculateZoomedImageBounds(_overview.ClientSize, _overview.Image.Size);
        if (!MinimapNavigator.TryPointToTile(e.Location, imageBounds, dimension, out System.Numerics.Vector2 tilePosition)) return;

        if (_view3dButton.Checked && _view3d is not null && _view3d.Visible)
        {
            _view3d.FocusTile(tilePosition.X, tilePosition.Y);
        }
        else
        {
            _canvas.FocusTile(tilePosition.X, tilePosition.Y);
        }
        _overview.Invalidate();
    }

    private void DrawMinimapIndicator(Graphics graphics)
    {
        if (_overview.Image is null || _selected is null) return;
        int dimension = _texturesDocument?.Dimension ?? _canvas.MapDimension;
        if (dimension <= 0) dimension = 64;

        Rectangle imageBounds = MinimapNavigator.CalculateZoomedImageBounds(_overview.ClientSize, _overview.Image.Size);
        if (imageBounds.Width <= 0 || imageBounds.Height <= 0) return;

        graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;

        if (_view3dButton.Checked && _view3d is not null && _view3d.Visible)
        {
            // 3D 視圖：以攝影機目標 tile (CameraTarget.X, CameraTarget.Z) 為中心繪製十字標記與小框
            System.Numerics.Vector3 target = _view3d.CameraTarget;
            PointF center = MinimapNavigator.TileToPoint(new System.Numerics.Vector2(target.X, target.Z), imageBounds, dimension);

            using var shadowPen = new Pen(Color.FromArgb(180, 0, 0, 0), 3f);
            using var indicatorPen = new Pen(Color.FromArgb(255, 255, 230, 40), 1.5f);

            // 十字線
            const float crossArm = 6f;
            graphics.DrawLine(shadowPen, center.X - crossArm, center.Y, center.X + crossArm, center.Y);
            graphics.DrawLine(shadowPen, center.X, center.Y - crossArm, center.X, center.Y + crossArm);
            graphics.DrawLine(indicatorPen, center.X - crossArm, center.Y, center.X + crossArm, center.Y);
            graphics.DrawLine(indicatorPen, center.X, center.Y - crossArm, center.X, center.Y + crossArm);

            // 中心外圍小方框
            const float boxHalf = 4f;
            var boxRect = new RectangleF(center.X - boxHalf, center.Y - boxHalf, boxHalf * 2, boxHalf * 2);
            graphics.DrawRectangle(shadowPen, boxRect.X, boxRect.Y, boxRect.Width, boxRect.Height);
            graphics.DrawRectangle(indicatorPen, boxRect.X, boxRect.Y, boxRect.Width, boxRect.Height);
        }
        else
        {
            // 2D 畫布：繪製目前可視區域矩形
            Rectangle sceneBounds = _canvas.SceneBoundsRectangle;
            RectangleF viewRect = MinimapNavigator.CalculateCanvasVisibleMinimapRect(sceneBounds, _canvas.ClientSize, imageBounds, dimension);
            if (viewRect.Width > 0 && viewRect.Height > 0)
            {
                using var shadowPen = new Pen(Color.FromArgb(180, 0, 0, 0), 3f);
                using var indicatorPen = new Pen(Color.FromArgb(255, 255, 230, 40), 1.5f);
                using var fillBrush = new SolidBrush(Color.FromArgb(35, 255, 255, 255));

                graphics.FillRectangle(fillBrush, viewRect);
                graphics.DrawRectangle(shadowPen, viewRect.X, viewRect.Y, viewRect.Width, viewRect.Height);
                graphics.DrawRectangle(indicatorPen, viewRect.X, viewRect.Y, viewRect.Width, viewRect.Height);
            }
        }
    }

    private void InitializeTerrainBlendSession()
    {
        _terrainBlendSession = null;
        _terrainBlendNotice = null;
        if (_texturesDocument is null || _floorMaterials is null || _floorMaterials.Materials.Count == 0) return;
        string fallback = _texturesDocument.Textures.Select(texture => _floorMaterials.FindByTexture(texture)?.Id).FirstOrDefault(id => id is not null)
            ?? _floorMaterials.Materials[0].Id;
        NativeTerrainImportResult import = TerrainBlendAuthoringMap.Import(_texturesDocument.Dimension, _texturesDocument.Textures, _floorMaterials, fallback);
        _terrainBlendSession = new TerrainBlendEditSession(import, _texturesDocument.Textures, _floorMaterials);
        if (import.UnresolvedTileIndices.Count > 0 || import.CornerConflicts.Count > 0)
        {
            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            _terrainBlendNotice = isEn
                ? $"Kept original boundaries: {import.UnresolvedTileIndices.Count} unresolved tiles, {import.CornerConflicts.Count} corner conflicts; only rebaking brush stroke areas."
                : $"已保留原圖交界：{import.UnresolvedTileIndices.Count} 個未知 tile、{import.CornerConflicts.Count} 個角點衝突；只重烘筆刷實際碰到的區域。";
        }
    }

    private void UpdateEditorState()
    {
        _regionToolsButton.Enabled = _selected?.IsCustom == true && _terrainBlendSession is not null;
        _layoutMenu.Enabled = _selected?.IsCustom == true;
        _boxSelectButton.Enabled = _selected?.IsCustom == true && _terrainBlendSession is not null;
        InvalidateMapDiagnostics();
        bool editable = _selected?.IsCustom == true; _saveButton.Enabled = editable && IsDirty; _gamePreviewButton.Enabled = _selected is not null; _undoButton.Enabled = editable && (TerrainLayerMode ? _terrainLayers?.CanUndo == true : _terrainBlendSession?.CanUndo == true); _redoButton.Enabled = editable && (TerrainLayerMode ? _terrainLayers?.CanRedo == true : _terrainBlendSession?.CanRedo == true); _resetTerrainButton.Enabled = editable && ((_texturesDocument is not null && TextureDirty()) || _terrainLayers?.IsDirty == true || _resetAuxiliaryLayers);
        _heightTool.Enabled = editable && _terrainLayers is not null; _aiMapButton.Enabled = editable && _terrainLayers is not null; _blankTerrainButton.Enabled = editable && _terrainLayers is not null; _placeTool.Enabled = editable && _objectCatalog.Count > 0; _natureTool.Enabled = editable && _natureStoreAvailable;
        if (_editMode == EditMode.Nature)
        {
            _undoButton.Enabled = editable && _natureSession.CanUndo;
            _redoButton.Enabled = editable && _natureSession.CanRedo;
        }
        if (_editMode == EditMode.PlaceObject)
        {
            _undoButton.Enabled = editable && _placementSession.CanUndo;
            _redoButton.Enabled = editable && _placementSession.CanRedo;
        }
        if (_editMode == EditMode.SceneMove)
        {
            _undoButton.Enabled |= editable && _natureSession.CanUndo;
            _redoButton.Enabled |= editable && _natureSession.CanRedo;
        }
        _collisionTool.Enabled = editable && _terrainLayers?.HasCollision == true;
        UpdateSceneEditButtons();
        UpdateEventButtons();
        _sceneRestoreButton.Enabled = editable && _sceneLoaded && (_sceneRemovals.Count > 0 || _sceneAdditions.Count > 0 || SdlSceneEditService.HasChanges(_sceneOriginalObjects, _sceneObjects));
        foreach (Control control in EditablePropertyControls()) control.Enabled = editable;
        _palette.Enabled = editable; UpdateStatus();
    }

    private void UpdatePaletteBrushLabel()
    {
        if (_activeMaterial is not null)
        {
            _currentMaterialLabel.Text = (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Active Brush: " : "目前筆刷：") + GetLocalizedMaterialName(_activeMaterial);
        }
        else
        {
            _currentMaterialLabel.Text = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Active Brush: None" : "目前筆刷：尚未取樣";
        }
    }

    private void UpdateStatus()
    {
        if (_selected is null)
        {
            _status.Text = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English
                ? "No map selected"
                : "尚未選擇地圖";
            return;
        }

        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string mapType = _selected.IsCustom
            ? (isEn ? "Custom Map, Editable" : "自製地圖，可編輯")
            : (isEn ? "Original Map, Read-Only" : "原廠地圖，唯讀");
        string dirtyMark = IsDirty
            ? (isEn ? "  ● Unsaved Changes" : "  ● 尚未儲存")
            : "";
        string notice = _terrainBlendNotice is null ? "" : "　" + _terrainBlendNotice;

        _status.Text = $"{_selected.Id} - {mapType}{dirtyMark}{notice}";
    }

    private void SetActiveView(bool use3D)
    {
        if (use3D && (_view3d is null || !_view3dButton.Enabled)) use3D = false;
        _view2dButton.Checked = !use3D; _view3dButton.Checked = use3D;
        _canvas.Visible = !use3D;
        if (_view3d is not null) _view3d.Visible = use3D;
        _modeBanner.BringToFront();
        _overview.Invalidate();
    }

    private void Disable3DView(string reason, Exception? exception = null)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _last3DDiagnostic = Build3DDiagnostic(reason, exception);
        _view3dButton.Enabled = false;
        _view3dButton.ToolTipText = reason;
        _3dDiagnosticsButton.Visible = true;
        _retryDisplayButton.Visible = true;
        _3dDiagnosticsButton.ToolTipText = isEn ? "View 3D diagnostics" : "查看 3D 場景無法啟用的實際原因";
        SetActiveView(use3D: false);
        _modeBanner.Text = isEn
            ? "3D unavailable: " + reason + "  Please click \"3D Diagnostics\"."
            : "3D 無法啟用：" + reason + "　請按「3D 診斷」。";
        _modeBanner.BackColor = Color.FromArgb(86, 69, 40);
    }

    private string Build3DDiagnostic(string reason, Exception? exception)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string mapPath = _selected?.DirectoryPath ?? (isEn ? "(No map selected)" : "（尚未選擇地圖）");
        string floorPath = Path.Combine(_gamePath, "floortex.dat");
        string heightPath = _selected is null ? (isEn ? "(No map selected)" : "（尚未選擇地圖）") : Path.Combine(mapPath, "boden.bmp");
        return isEn
            ? string.Join(Environment.NewLine,
                "Against Rome Map Editor - 3D Diagnostics",
                $"Reason: {reason}",
                $"Map: {_selected?.Id ?? "(None)"}",
                $"floortex.dat: {(File.Exists(floorPath) ? "Present" : "Missing")} - {floorPath}",
                $"boden.bmp: {(File.Exists(heightPath) ? "Present" : "Missing")} - {heightPath}",
                $"OpenGL: {_view3d?.ContextDescription ?? "Context not created"}",
                $"OS: {Environment.OSVersion}",
                $"Arch: {System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture}",
                exception is null ? "Exception: None" : "Exception: " + exception)
            : string.Join(Environment.NewLine,
                "Against Rome Map Editor - 3D 診斷",
                $"原因：{reason}",
                $"地圖：{_selected?.Id ?? "（無）"}",
                $"floortex.dat：{(File.Exists(floorPath) ? "存在" : "缺少")} - {floorPath}",
                $"boden.bmp：{(File.Exists(heightPath) ? "存在" : "缺少")} - {heightPath}",
                $"OpenGL：{_view3d?.ContextDescription ?? "尚未建立 context"}",
                $"作業系統：{Environment.OSVersion}",
                $"程序架構：{System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture}",
                exception is null ? "例外：無" : "例外：" + exception);
    }

    private void Show3DDiagnostics()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string diagnostic = _last3DDiagnostic ?? (isEn ? "No 3D diagnostics diagnostic info." : "目前沒有 3D 失敗診斷。");
        using var dialog = new Form { Text = isEn ? "3D Scene Diagnostics" : "3D 場景診斷", Width = 760, Height = 520, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor };
        var text = new TextBox { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Both, WordWrap = false, Text = diagnostic, Font = new Font(FontFamily.GenericMonospace, 9F) };
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(10), FlowDirection = FlowDirection.RightToLeft };
        var close = new Button { Text = isEn ? "Close" : "關閉", DialogResult = DialogResult.OK, Width = 90 };
        var copy = new Button { Text = isEn ? "Copy" : "複製診斷", Width = 110 };
        copy.Click += (_, _) => { try { Clipboard.SetText(diagnostic); } catch { } };
        buttons.Controls.Add(close); buttons.Controls.Add(copy); dialog.Controls.Add(text); dialog.Controls.Add(buttons); dialog.AcceptButton = close;
        dialog.ShowDialog(this);
    }

    private void LoadSceneList(IReadOnlyList<MapSceneObject> objects)
    {
        _sceneList.BeginUpdate(); _sceneList.Items.Clear();
        int buildingIndex = 0, unitIndex = 0, objectIndex = 0;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        HashSet<string> removed = _sceneRemovals.Select(item => item.SourceFile.ToUpperInvariant() + "|" + item.ObjectIndex).ToHashSet();
        foreach (MapSceneObject item in objects.OrderBy(x => x.Kind).ThenBy(x => x.Team))
        {
            string kindText = item.Kind;
            if (isEn)
            {
                kindText = item.Kind switch {
                    "建築" => "Building",
                    "單位" => "Unit",
                    _ => "Other"
                };
            }
            if (item.Kind == "建築") buildingIndex++; else if (item.Kind == "單位") unitIndex++; else objectIndex++;
            bool pendingRemoval = removed.Contains(SceneKey(item));
            var row = new ListViewItem(kindText) { Tag = item };
            row.SubItems.Add(pendingRemoval ? (isEn ? $"{item.Name} (pending delete)" : $"{item.Name}（將刪除）") : item.Name);
            row.SubItems.Add(item.Team >= 0 ? item.Team.ToString() : "-");
            row.SubItems.Add($"{item.SourceFile} / {item.ObjectIndex:0000}");
            if (pendingRemoval) row.ForeColor = Color.IndianRed;
            _sceneList.Items.Add(row);
        }
        foreach (StagedSceneAddition addition in _sceneAdditions)
        {
            MapSceneObject item = addition.Display;
            string kindText = isEn
                ? item.Kind switch { "建築" => "Building", "單位" => "Unit", _ => "Other" }
                : item.Kind;
            if (item.Kind == "建築") buildingIndex++; else if (item.Kind == "單位") unitIndex++; else objectIndex++;
            var row = new ListViewItem(kindText) { Tag = addition, ForeColor = Color.MediumSpringGreen };
            row.SubItems.Add(addition.FromCatalog
                ? (isEn ? $"{item.Name} (pending add)" : $"{item.Name}（待新增）")
                : (isEn ? $"{item.Name} (pending copy)" : $"{item.Name}（待複製）"));
            row.SubItems.Add(item.Team >= 0 ? item.Team.ToString() : "-");
            row.SubItems.Add(isEn ? $"{item.SourceFile} / new" : $"{item.SourceFile} / 新增");
            _sceneList.Items.Add(row);
        }
        _sceneList.EndUpdate();

        int pendingText = _sceneRemovals.Count + _sceneAdditions.Count + _settlementOffsets.Count;
        string pendingSuffix = pendingText == 0 ? "" : isEn
            ? $"  Pending: {_sceneAdditions.Count} additions, {_sceneRemovals.Count} deletions, {_settlementOffsets.Count} settlement moves."
            : $"　待儲存：新增 {_sceneAdditions.Count}、刪除 {_sceneRemovals.Count}、聚落平移 {_settlementOffsets.Count}。";
        _sceneSummary.Text = isEn
            ? $"Buildings: {buildingIndex} | Units: {unitIndex} | Others: {objectIndex}{pendingSuffix}\nSelect an item to focus it; Move Selected Object lets you drag it on 2D/3D terrain."
            : $"建築 {buildingIndex}　單位 {unitIndex}　其他 {objectIndex}{pendingSuffix}\n選取項目可定位；啟用「移動選取物件」後可在 2D／3D 地表拖曳。";
    }

    private void ChooseWaterColor()
    {
        using var dialog = new ColorDialog { FullOpen = true };
        if (TryParseGameColor(_waterColor.Text, out Color color)) dialog.Color = color;
        if (dialog.ShowDialog(this) != DialogResult.OK) return;
        _waterColor.Text = $"0x{dialog.Color.B:X2}{dialog.Color.G:X2}{dialog.Color.R:X2}".ToLowerInvariant();
        _waterColorButton.BackColor = dialog.Color; _waterColorButton.ForeColor = dialog.Color.GetBrightness() < .45f ? Color.White : Color.Black;
        UpdateWaterPreview();
    }

    private static bool TryParseGameColor(string value, out Color color)
    {
        color = Color.SteelBlue; string hex = value.Trim().Replace("0x", "", StringComparison.OrdinalIgnoreCase);
        if (hex.Length != 6 || !int.TryParse(hex, System.Globalization.NumberStyles.HexNumber, null, out int bgr)) return false;
        color = Color.FromArgb(bgr & 0xff, (bgr >> 8) & 0xff, (bgr >> 16) & 0xff); return true;
    }

    private void HandleShortcut(KeyEventArgs e)
    {
        // 3D 場景取得焦點時，Delete 對選取的場景物件執行「刪除／取消刪除」（與按鈕相同，儲存前可還原）。
        if (e.KeyCode == Keys.Delete && !e.Control && _view3d is { Focused: true })
        {
            if (DeletePickedNature()) { e.SuppressKeyPress = true; return; }
            if (_sceneDeleteButton.Enabled) { ToggleDeleteSelectedSceneObject(); e.SuppressKeyPress = true; return; }
        }
        if (!e.Control) return;
        if (e.KeyCode == Keys.S) {
            if (_selected?.IsCustom == true) SaveMap(showSuccess: false);
            else
            {
                _status.Text = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English
                    ? "Original maps are read-only; copy to a custom map first."
                    : "原廠地圖為唯讀，無法儲存；請先複製為自製地圖。";
            }
        }
        else if (e.KeyCode == Keys.Z) Undo();
        else if (e.KeyCode == Keys.Y) Redo();
        else return;
        e.SuppressKeyPress = true;
    }

    /// <summary>Dock=Top 的多行說明依目前寬度、字型與文字調整高度，避免換行（含高 DPI 字型）後被截斷。</summary>
    private static void FitWrappedLabelHeight(Label label)
    {
        int width = -1;
        void Fit(bool force)
        {
            if (label.Width <= 0 || (!force && label.Width == width)) return;
            width = label.Width;
            label.Height = label.GetPreferredSize(new Size(label.Width, 0)).Height;
        }
        label.SizeChanged += (_, _) => Fit(false);
        label.TextChanged += (_, _) => Fit(true);
        label.FontChanged += (_, _) => Fit(true);
    }

    private static Label SectionHeader(string text) => new() { Text = text, Dock = DockStyle.Top, Height = 38, Font = WinFormsTheme.CreateDisplayFont(10F), Padding = new Padding(4, 10, 0, 0), ForeColor = WinFormsTheme.TextPrimary };

    private static Label AddField(TableLayoutPanel table, string label, Control control)
    {
        var lbl = new Label { Text = label, AutoSize = true, Margin = new Padding(3, 11, 3, 4), ForeColor = WinFormsTheme.TextSecondary };
        table.Controls.Add(lbl);
        table.Controls.Add(control);
        return lbl;
    }

    private static Label AddSceneField(TableLayoutPanel table, int row, string label, Control control)
    {
        var lbl = new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left, ForeColor = WinFormsTheme.TextSecondary };
        table.Controls.Add(lbl, 0, row);
        table.Controls.Add(control, 1, row);
        return lbl;
    }

    private static NumericUpDown SceneCoordinateInput() => new() { Dock = DockStyle.Fill, Minimum = -32768, Maximum = 32768, DecimalPlaces = 2, Increment = 16 };
    private static decimal ClampSceneCoordinate(float value, NumericUpDown input) => Math.Clamp((decimal)value, input.Minimum, input.Maximum);
    private static string SceneKey(MapSceneObject item) => item.SourceFile.ToUpperInvariant() + "|" + item.ObjectIndex;
    private IEnumerable<Control> EditablePropertyControls()
    {
        yield return _title; yield return _subtitle; yield return _briefing;
        foreach (TextBox teamName in _teamNames) yield return teamName;
        yield return _waterLevel; yield return _waterColor; yield return _waterWarpShift; yield return _waterBumpAmplitude; yield return _waterBumpFrequency; yield return _flashProbability;
        yield return _dayStart; yield return _dayEnd; yield return _rain;
    }
    private static decimal ParseDecimal(string? value, NumericUpDown control) => decimal.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out decimal parsed) ? Math.Clamp(parsed, control.Minimum, control.Maximum) : control.Minimum;

    private void UpdateGameTextWarning(TextBox text)
    {
        if (ReferenceEquals(text, _waterColor)) return;
        bool en = Loc.CurrentLanguage == Language.English;
        _gameTextErrors.SetError(text, MapTextDocument.CanEncodeGameText(text.Text) ? "" : en
            ? "The game supports CP1251 text (Latin/Cyrillic). Chinese characters and emoji cannot be saved."
            : "遊戲支援 CP1251 文字（拉丁／西里爾字母）；中文與 emoji 無法儲存。");
    }

    private void ShowError(Exception ex)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        MessageBox.Show(this, ex.Message, isEn ? "Map Editor Error" : "地圖編輯器錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
    }

    private static AgainstRomeMapEditor.NativeAssets.NativeSpriteCatalog? OpenSpriteCatalog(string gamePath)
    {
        try { return AgainstRomeMapEditor.NativeAssets.NativeSpriteCatalog.Open(gamePath); }
        catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException) { return null; }
    }

    private static AgainstRomeMapEditor.NativeAssets.NativeShadowCatalog? OpenShadowCatalog(string gamePath)
    {
        try { return AgainstRomeMapEditor.NativeAssets.NativeShadowCatalog.Open(gamePath); }
        catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException or NotSupportedException) { return null; }
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing) { _lightingContext?.Dispose(); _lightingContext = null; }
        if (disposing) { _gameTextErrors.Dispose(); _floorTextures?.Dispose(); _floorTextures = null; _spriteCatalog?.Dispose(); _spriteCatalog = null; _shadowCatalog?.Dispose(); _shadowCatalog = null; Image? overview = _overview.Image; _overview.Image = null; overview?.Dispose(); }
        base.Dispose(disposing);
    }

    private sealed record PaletteItem(string Key, string PreviewTexture, string Name, FloorMaterial? Material);
}
