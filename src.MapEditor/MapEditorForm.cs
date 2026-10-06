using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using System.Diagnostics;

namespace AgainstRomeMapEditor;

internal sealed class MapEditorForm : Form
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
    private readonly ToolStripButton _collisionTool = new("通行區域") { CheckOnClick = true };
    private readonly ToolStripComboBox _terrainOperation = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 96, Visible = false };
    private readonly ToolStripComboBox _terrainStrength = new() { DropDownStyle = ComboBoxStyle.DropDownList, Width = 72, Visible = false };
    private readonly ToolStripButton _view2dButton = new("2D 俯視") { CheckOnClick = true };
    private readonly ToolStripButton _view3dButton = new("3D 場景") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _3dDiagnosticsButton = new("3D 診斷") { Visible = false };
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
    private (int X, int Y)? _lastTerrainTile;
    private readonly HashSet<int> _terrainStrokeTiles = new();

    private bool TextureDirty() => _terrainBlendSession?.IsDirty == true;

    private bool SceneDirty() => _sceneLoaded && (_sceneRemovals.Count > 0 || _sceneAdditions.Count > 0 || _settlementOffsets.Count > 0 || SdlSceneEditService.HasChanges(_sceneSavedObjects, _sceneObjects));

    /// <summary>畫布／清單實際呈現的場景：排除暫存刪除、加入暫存新增物件，並套用暫存的聚落整體平移。</summary>
    private IReadOnlyList<MapSceneObject> EffectiveSceneObjects()
    {
        if (_sceneRemovals.Count == 0 && _sceneAdditions.Count == 0 && _settlementOffsets.Count == 0) return _sceneObjects;
        HashSet<string> removed = _sceneRemovals.Select(item => item.SourceFile.ToUpperInvariant() + "|" + item.ObjectIndex).ToHashSet();
        return _sceneObjects.Where(item => !removed.Contains(SceneKey(item))).Concat(_sceneAdditions.Select(item => item.Display)).Select(WithSettlementOffset).ToArray();
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

    private bool IsDirty => _propertyDirty || TextureDirty() || SceneDirty() || _terrainLayers?.IsDirty == true;

    public MapEditorForm(string gamePath, GameMapInfo selectedMap)
    {
        Width = 1440; Height = 900; MinimumSize = new Size(1100, 700); StartPosition = FormStartPosition.CenterScreen;
        BackColor = WinFormsTheme.Window; ForeColor = WinFormsTheme.TextPrimary; Font = WinFormsTheme.CreateFont(9F);
        _gamePath = gamePath;
        _selected = selectedMap;
        _floorTextures = new FloorTextureLibrary(Path.Combine(gamePath, "floortex.dat"));
        _floorMaterials = new FloorMaterialCatalog(_floorTextures);
        BuildInterface();
        WinFormsTheme.Apply(this);
        WinFormsTheme.StylePrimaryButton(_sceneApplyButton);
        WinFormsTheme.StyleDangerButton(_sceneDeleteButton);
        WireEvents();
        KeyPreview = true;
        Shown += (_, _) => LoadSelectedMap();
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
        tools.Items.AddRange(new ToolStripItem[] { _lblTerrainGroup, _textureTool, _heightTool, _collisionTool, _sceneMoveTool, _terrainOperation, _terrainStrength, _resetTerrainButton, new ToolStripSeparator(), _view2dButton, _view3dButton, _3dDiagnosticsButton });

        _paletteHeader = SectionHeader("地表繪製");
        var palettePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(10), BackColor = WinFormsTheme.Surface };
        var currentBrush = new FlowLayoutPanel { Dock = DockStyle.Top, Height = 76, Padding = new Padding(4), FlowDirection = FlowDirection.LeftToRight };
        var currentText = new FlowLayoutPanel { Width = 205, Height = 66, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        currentText.Controls.Add(_currentMaterialLabel); currentText.Controls.Add(_lblBrushInstructions);
        currentBrush.Controls.Add(_currentMaterialSwatch); currentBrush.Controls.Add(currentText);
        var brushOptions = new Panel { Dock = DockStyle.Top, Height = 174 };
        brushOptions.Controls.Add(_showObjects); brushOptions.Controls.Add(_showGrid); brushOptions.Controls.Add(_reliefScale); 
        brushOptions.Controls.Add(_lblReliefScaleTitle); brushOptions.Controls.Add(_lblBrushSizeTitle); brushOptions.Controls.Add(_brushSize);
        palettePanel.Controls.Add(_palette); palettePanel.Controls.Add(_paletteSearch); palettePanel.Controls.Add(brushOptions); palettePanel.Controls.Add(currentBrush); palettePanel.Controls.Add(_paletteHeader);

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
        _inspectorTabs.TabPages.Add(new TabPage("場景物件") { BackColor = WinFormsTheme.Surface }); _inspectorTabs.TabPages[2].Controls.Add(scenePanel);

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

        var centerRight = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1140, 760), SplitterDistance = 820 };
        centerRight.Panel1.Controls.Add(_canvasHost); centerRight.Panel2.Controls.Add(_inspectorTabs); centerRight.Panel2MinSize = 280;
        var statusStrip = new StatusStrip { BackColor = WinFormsTheme.Surface, ForeColor = WinFormsTheme.TextSecondary };
        statusStrip.Items.Add(_status); statusStrip.Items.Add(_lblStatusInstructions);
        Controls.Add(centerRight); Controls.Add(tools); Controls.Add(commands); Controls.Add(statusStrip);
        commands.BringToFront(); tools.BringToFront();
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
        _brushSize.SelectedIndexChanged += (_, _) => _canvas.BrushSize = _brushSize.SelectedIndex switch { 1 => 3, 2 => 5, _ => 1 };
        _brushSize.SelectedIndexChanged += (_, _) => { if (_view3d is not null) _view3d.BrushSize = _canvas.BrushSize; };
        _showGrid.CheckedChanged += (_, _) => { _canvas.ShowGrid = _showGrid.Checked; _canvas.Invalidate(); if (_view3d is not null) { _view3d.ShowGrid = _showGrid.Checked; _view3d.Invalidate(); } };
        _showObjects.CheckedChanged += (_, _) => { _canvas.ShowObjects = _showObjects.Checked; _canvas.Invalidate(); if (_view3d is not null) { _view3d.ShowObjects = _showObjects.Checked; _view3d.Invalidate(); } };
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
        if (_view3d is not null)
        {
            Map3DViewControl view3d = _view3d;
            _view3d.TexturePainted += (_, e) => PaintTexture(e);
            _view3d.TextureSampled += (_, e) => SelectSampledTexture(e.Texture);
            _view3d.StrokeEnded += (_, _) => CommitStroke();
            _view3d.TileHovered += (_, e) => ShowTerrainHover(e);
            _view3d.SceneObjectMoved += (_, e) => MoveSelectedSceneObject(e);
            _view3d.InitializationFailed += (_, ex) => BeginInvoke(() => Disable3DView(view3d.LastFailureReason ?? (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "OpenGL 3.3 initialization failed." : "OpenGL 3.3 初始化失敗。"), ex));
        }
        _view2dButton.Click += (_, _) => SetActiveView(use3D: false);
        _view3dButton.Click += (_, _) => SetActiveView(use3D: true);
        _3dDiagnosticsButton.Click += (_, _) => Show3DDiagnostics();
        _mapMenuButton.Click += (_, _) => ReturnToMenu();
        _textureTool.Click += (_, _) => SetEditMode(EditMode.Texture);
        _sceneMoveTool.Click += (_, _) => SetEditMode(EditMode.SceneMove);
        _heightTool.Click += (_, _) => SetEditMode(EditMode.Height);
        _collisionTool.Click += (_, _) => SetEditMode(EditMode.Collision);
        _resetTerrainButton.Click += (_, _) => ResetTerrain();
        _saveButton.Click += (_, _) => SaveMap(showSuccess: true);
        _gamePreviewButton.Click += (_, _) => PreviewInGame();
        _undoButton.Click += (_, _) => Undo(); _redoButton.Click += (_, _) => Redo();
        _aiMapButton.Click += (_, _) => OpenAiMapDialog();
        
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
            if (control is TextBox text) text.TextChanged += (_, _) => MarkDirty();
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
        _aiMapButton.Text = isEn ? "AI Map Maker…" : "AI 製圖…";
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

        _paletteHeader.Text = isEn ? "Terrain Painting" : "地表繪製";
        _paletteSearch.PlaceholderText = isEn ? "Search grass, sand, mud, rock..." : "搜尋草地、沙地、泥土、岩地…";
        _lblBrushInstructions.Text = isEn ? "Right-click: Sample | Left-click & drag: Draw." : "右鍵取樣，左鍵拖曳繪製。";

        _lblBrushSizeTitle.Text = isEn ? "Brush Size" : "筆刷大小";
        _lblReliefScaleTitle.Text = isEn ? "Relief Scaling (Approx)" : "地形起伏（近似顯示）";

        _brushSize.Items.Clear();
        if (isEn)
        {
            _brushSize.Items.AddRange(new object[] { "Fine (1 tile)", "Medium (3 x 3)", "Large (5 x 5)" });
        }
        else
        {
            _brushSize.Items.AddRange(new object[] { "精細（1 格）", "中型（3 × 3）", "大型（5 × 5）" });
        }
        _brushSize.SelectedIndex = _canvas.BrushSize switch { 3 => 1, 5 => 2, _ => 0 };

        if (_inspectorTabs.TabPages.Count >= 3)
        {
            _inspectorTabs.TabPages[0].Text = isEn ? "Terrain" : "地表";
            _inspectorTabs.TabPages[1].Text = isEn ? "Map Properties" : "地圖屬性";
            _inspectorTabs.TabPages[2].Text = isEn ? "Scene Objects" : "場景物件";
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
        UpdateEditorState();
        UpdatePaletteBrushLabel();
        
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
            _heightMapStep = float.TryParse(ini.GetValue("Heightmapstep"), out float heightStep) && heightStep > 0 ? heightStep : 4;
            if (TryParseGameColor(_waterColor.Text, out Color waterColor)) { _waterColorButton.BackColor = waterColor; _waterColorButton.ForeColor = waterColor.GetBrightness() < .45f ? Color.White : Color.Black; }
            _dayStart.Value = ParseDecimal(ini.GetValue("DayStartTime"), _dayStart); _dayEnd.Value = ParseDecimal(ini.GetValue("DayEndTime"), _dayEnd); _rain.Checked = ini.GetValue("RainDropsOnWater") == "1";
            _texturesDocument = BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")); _savedTextures = _texturesDocument.Textures.ToArray(); InitializeTerrainBlendSession(); InitializeTerrainLayers(map); _propertyDirty = false;
            _sceneObjects = SdlSceneCatalog.LoadDirectory(map); _sceneOriginalObjects = _sceneObjects.ToArray(); _sceneSavedObjects = _sceneObjects.ToArray(); _sceneLoaded = true;
            _sceneRemovals.Clear(); _sceneAdditions.Clear(); _settlementOffsets.Clear();
            _settlementOrigins = SdlSceneCatalog.LoadSettlementOrigins(map);
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
        bool hasRealTextures = _texturesDocument is not null && _floorTextures is not null && _canvas.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _savedTextures, _selected.DirectoryPath, _floorTextures, effectiveObjects, (float)_waterLevel.Value, _heightMapStep, sceneWaterColor, preserveView);
        bool has3DScene = false;
        if (_view3d is not null && _texturesDocument is not null && _floorTextures is not null)
        {
            _view3d.BrushTexture = _canvas.BrushTexture;
            _view3d.BrushSize = _canvas.BrushSize;
            _view3d.ShowGrid = _showGrid.Checked;
            _view3d.ShowObjects = _showObjects.Checked;
            _view3d.EditingEnabled = _selected.IsCustom;
            try { has3DScene = _view3d.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _selected.DirectoryPath, _floorTextures, effectiveObjects, (float)_waterLevel.Value, _heightMapStep, sceneWaterColor); _view3d.SetReliefScale(_reliefScale.Value / 100f); }
            catch (Exception ex) { Disable3DView(isEn ? "Failed to load 3D map resources." : "載入 3D 地圖資源失敗。", ex); }
        }
        if (_terrainLayers?.HeightsDirty == true) ApplyHeightsToViews();
        if (_editMode == EditMode.Collision) _canvas.SetCollisionOverlay(_terrainLayers?.CollisionSize ?? 0, _terrainLayers?.Collision);
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
        if (TerrainLayerMode) { PaintTerrainLayer(e); return; }
        if (_selected is null || !_selected.IsCustom || _texturesDocument is null || _terrainBlendSession is null || _activeMaterial is null) return;
        float radius = _canvas.BrushSize / 2f + .26f;
        TerrainBlendPaintResult result = _terrainBlendSession.PaintCircle(e.X + .5f, e.Y + .5f, radius, _activeMaterial.Id);
        foreach (TerrainTextureChange change in result.TextureChanges) ApplyTexture(change.X, change.Y, change.After);
        _terrainBlendNotice = result.Succeeded
            ? null
            : $"此筆觸無法由原版 transition tile 完整表達，已整筆復原（{result.Issues.Count} 格）。";
        UpdateEditorState();
    }

    // 一次筆畫（滑鼠按下到放開）內觸及的所有格子合併為單一 undo 項目。
    private void CommitStroke()
    {
        _flattenTarget = -1; _lastTerrainTile = null; _terrainStrokeTiles.Clear();
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

    private enum EditMode { Texture, SceneMove, Height, Collision }
    private EditMode _editMode = EditMode.Texture;
    private const string TerrainToolBrushToken = "\u0001terrain-tool"; // 讓視圖在未取樣材質時仍送出筆刷事件；不是材質名稱。
    private bool TerrainLayerMode => _editMode is EditMode.Height or EditMode.Collision;

    private void SetEditMode(EditMode mode)
    {
        CommitStroke();
        _editMode = mode;
        _textureTool.Checked = mode == EditMode.Texture;
        _sceneMoveTool.Checked = mode == EditMode.SceneMove;
        _heightTool.Checked = mode == EditMode.Height;
        _collisionTool.Checked = mode == EditMode.Collision;
        if (mode == EditMode.SceneMove && _inspectorTabs.TabPages.Count >= 3) _inspectorTabs.SelectedIndex = 2;
        PopulateTerrainToolOptions();
        if (TerrainLayerMode && string.IsNullOrWhiteSpace(_canvas.BrushTexture)) SetBrushToken(TerrainToolBrushToken);
        else if (!TerrainLayerMode && _canvas.BrushTexture == TerrainToolBrushToken) SetBrushToken(null);
        _canvas.SetCollisionOverlay(_terrainLayers?.CollisionSize ?? 0, mode == EditMode.Collision ? _terrainLayers?.Collision : null);
        UpdateSceneEditButtons();
        UpdateEditorState();
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _status.Text = mode switch
        {
            EditMode.SceneMove => isEn ? "Move mode: select an SDL object, then drag on the terrain. Changes remain pending until Save." : "物件移動模式：選取 SDL 物件後在地表拖曳；按下「儲存」前只會暫存在記憶體。",
            EditMode.Height => _terrainLayers is null
                ? (isEn ? "This map has no editable boden.bmp height map." : "此地圖沒有可編輯的 boden.bmp 高度圖。")
                : (isEn ? "Height mode: left-drag to apply the selected operation; Ctrl+Z / Ctrl+Y undo and redo." : "地形高度模式：左鍵拖曳套用所選操作；Ctrl+Z／Ctrl+Y 復原與重做。"),
            EditMode.Collision => _terrainLayers?.HasCollision != true
                ? (isEn ? "This map has no editable collision.bmp." : "此地圖沒有可編輯的 collision.bmp。")
                : (isEn ? "Passability mode: red = blocked. Left-drag to block or clear (2D view shows the overlay)." : "通行區域模式：紅色為阻擋；左鍵拖曳設定阻擋或可通行（2D 檢視顯示疊圖）。"),
            _ => isEn ? "Texture brush mode." : "材質筆刷模式：右鍵取樣，左鍵拖曳繪製。",
        };
    }

    private void SetBrushToken(string? token)
    {
        _canvas.BrushTexture = token;
        if (_view3d is not null) _view3d.BrushTexture = token;
    }

    private void PopulateTerrainToolOptions()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        int operation = Math.Max(0, _terrainOperation.SelectedIndex), strength = _terrainStrength.SelectedIndex < 0 ? 1 : _terrainStrength.SelectedIndex;
        _terrainOperation.Items.Clear();
        if (_editMode == EditMode.Height)
            _terrainOperation.Items.AddRange(isEn ? new object[] { "Raise", "Lower", "Smooth", "Flatten" } : new object[] { "升高", "降低", "平滑", "整平" });
        else
            _terrainOperation.Items.AddRange(isEn ? new object[] { "Block", "Passable" } : new object[] { "阻擋", "可通行" });
        _terrainOperation.SelectedIndex = Math.Min(operation, _terrainOperation.Items.Count - 1);
        _terrainStrength.Items.Clear();
        _terrainStrength.Items.AddRange(isEn ? new object[] { "Gentle", "Medium", "Strong" } : new object[] { "輕", "中", "強" });
        _terrainStrength.SelectedIndex = strength;
        _terrainOperation.Visible = TerrainLayerMode;
        _terrainStrength.Visible = _editMode == EditMode.Height;
    }

    private int TerrainStrength => _terrainStrength.SelectedIndex switch { 0 => 2, 2 => 14, _ => 6 };

    private void InitializeTerrainLayers(string map)
    {
        _terrainLayers = null; _bodenLayer = _embossLayer = _collisionLayer = null; _flattenTarget = -1;
        if (_texturesDocument is null) return;
        int dimension = _texturesDocument.Dimension;
        TerrainLayer? boden = TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp"));
        if (boden is null || boden.Width != boden.Height || boden.Width < 2 || (boden.Width - 1) % dimension != 0) return;
        TerrainLayer? emboss = TerrainLayerFiles.Read(Path.Combine(map, "emboss.bmp"));
        if (emboss is not null && (emboss.Width != boden.Width || emboss.Height != boden.Height)) emboss = null;
        TerrainLayer? collision = TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"));
        if (collision is not null && (collision.Width != collision.Height || collision.Width % dimension != 0)) collision = null;
        _bodenLayer = boden; _embossLayer = emboss; _collisionLayer = collision;
        _terrainLayers = new TerrainHeightEditSession(boden.Width, boden.Green, emboss?.Green, collision?.Width ?? 0, collision?.Green);
    }

    private void PaintTerrainLayer(TexturePaintEventArgs e)
    {
        if (_selected?.IsCustom != true || _terrainLayers is null || _texturesDocument is null) return;
        // 視圖只在滑鼠移動時回報 tile；快速拖曳會跳格，因此在同一筆畫內補齊上一點到目前點之間的 tile。
        IEnumerable<(int X, int Y)> tiles = _lastTerrainTile is { } last
            ? TerrainStrokePath.Between(last.X, last.Y, e.X, e.Y)
            : new[] { (e.X, e.Y) };
        _lastTerrainTile = (e.X, e.Y);
        bool heightsChanged = false, collisionChanged = false;
        foreach ((int x, int y) in tiles)
        {
            if (!_terrainStrokeTiles.Add(y * _texturesDocument.Dimension + x)) continue; // 同一筆畫不重複套用同一格。
            (bool height, bool collision) = PaintTerrainTile(x, y);
            heightsChanged |= height; collisionChanged |= collision;
        }
        if (heightsChanged) ApplyHeightsToViews();
        if (collisionChanged) _canvas.SetCollisionOverlay(_terrainLayers.CollisionSize, _terrainLayers.Collision);
        UpdateEditorState();
    }

    private (bool Heights, bool Collision) PaintTerrainTile(int tileX, int tileY)
    {
        int dimension = _texturesDocument!.Dimension;
        float radiusTiles = _canvas.BrushSize / 2f + .26f;
        if (_editMode == EditMode.Height)
        {
            float step = (_terrainLayers!.VertexSize - 1) / (float)dimension;
            float centerX = (tileX + .5f) * step, centerY = (tileY + .5f) * step;
            var operation = (TerrainHeightOperation)Math.Clamp(_terrainOperation.SelectedIndex, 0, 3);
            if (operation == TerrainHeightOperation.Flatten && _flattenTarget < 0)
                _flattenTarget = _terrainLayers.Heights[Math.Clamp((int)MathF.Round(centerY), 0, _terrainLayers.VertexSize - 1) * _terrainLayers.VertexSize + Math.Clamp((int)MathF.Round(centerX), 0, _terrainLayers.VertexSize - 1)];
            return (_terrainLayers.PaintHeight(centerX, centerY, radiusTiles * step + 1, operation, TerrainStrength, _flattenTarget).Count > 0, false);
        }
        if (_terrainLayers!.HasCollision)
        {
            float step = _terrainLayers.CollisionSize / (float)dimension;
            var operation = _terrainOperation.SelectedIndex == 1 ? TerrainCollisionOperation.Clear : TerrainCollisionOperation.Block;
            return (false, _terrainLayers.PaintCollision((tileX + .5f) * step, (tileY + .5f) * step, radiusTiles * step, operation).Count > 0);
        }
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _terrainBlendNotice = isEn ? "This map has no collision.bmp to edit." : "此地圖沒有可編輯的 collision.bmp。";
        return (false, false);
    }

    private void OpenAiMapDialog()
    {
        if (_selected?.IsCustom != true || _terrainLayers is null || _texturesDocument is null) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        using var dialog = new Form
        {
            Text = isEn ? "AI Map Maker (local Ollama)" : "AI 製圖（本機 Ollama）",
            Width = 640, Height = 520, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor,
            FormBorderStyle = FormBorderStyle.Sizable, MinimizeBox = false, MaximizeBox = false, ShowInTaskbar = false, MinimumSize = new Size(520, 420),
        };
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12) };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 90)); layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 36)); layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 92));
        var modelBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDown };
        var prompt = new TextBox
        {
            Dock = DockStyle.Fill, Multiline = true, ScrollBars = ScrollBars.Vertical, AcceptsReturn = true,
            Text = isEn
                ? "A green river valley running from the north-west to the south-east, a high mountain range in the north-east, a small lake in the south-west, sandy ground near the river and several wide flat meadows for villages."
                : "一條從西北流向東南的河谷，東北方是高聳山脈，西南有一座小湖，河岸是沙地，另外保留幾片寬廣平坦的草地讓村莊發展。",
        };
        var status = new Label
        {
            Dock = DockStyle.Fill, ForeColor = WinFormsTheme.TextSecondary,
            Text = isEn
                ? "The plan is applied as one undoable step for heights (Terrain Height mode) and textures (Texture mode). Nothing is written until you click Save."
                : "結果會套用為可復原的步驟（高度在「地形高度」模式復原、材質在「材質筆刷」模式復原）；按「儲存」前不會寫入任何檔案。",
        };
        layout.Controls.Add(new Label { Text = isEn ? "Model" : "模型", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 0); layout.Controls.Add(modelBox, 1, 0);
        layout.Controls.Add(new Label { Text = isEn ? "Description" : "地圖描述", AutoSize = true, Anchor = AnchorStyles.Left | AnchorStyles.Top }, 0, 1); layout.Controls.Add(prompt, 1, 1);
        layout.Controls.Add(status, 0, 2); layout.SetColumnSpan(status, 2);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(12, 8, 12, 8), FlowDirection = FlowDirection.RightToLeft };
        var generate = new Button { Text = isEn ? "Generate && Apply" : "生成並套用", Width = 130, Height = 34 };
        var close = new Button { Text = isEn ? "Close" : "關閉", DialogResult = DialogResult.Cancel, Width = 90, Height = 34 };
        buttons.Controls.Add(generate); buttons.Controls.Add(close);
        dialog.Controls.Add(layout); dialog.Controls.Add(buttons); dialog.CancelButton = close;
        WinFormsTheme.Apply(dialog); WinFormsTheme.StylePrimaryButton(generate);

        using var planner = new OllamaMapPlanner();
        using var cancellation = new CancellationTokenSource();
        dialog.FormClosing += (_, _) => cancellation.Cancel();
        dialog.Shown += async (_, _) =>
        {
            try
            {
                IReadOnlyList<string> models = await planner.ListModelsAsync(cancellation.Token);
                modelBox.Items.AddRange(models.Cast<object>().ToArray());
                string? preferred = models.FirstOrDefault(name => name.StartsWith("gemma", StringComparison.OrdinalIgnoreCase) || name.StartsWith("qwen", StringComparison.OrdinalIgnoreCase)) ?? (models.Count > 0 ? models[0] : null);
                if (preferred is not null) modelBox.Text = preferred;
                if (models.Count == 0) status.Text = isEn ? "Ollama has no models. Install one first, e.g. `ollama pull gemma3:12b`." : "Ollama 尚未安裝任何模型，請先執行例如 `ollama pull gemma3:12b`。";
            }
            catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or System.Text.Json.JsonException)
            {
                status.Text = (isEn ? $"Cannot reach Ollama at {planner.Endpoint}. Start Ollama and reopen this dialog. " : $"無法連線到 {planner.Endpoint} 的 Ollama，請先啟動 Ollama 再開啟此視窗。") + ex.Message;
            }
        };
        generate.Click += async (_, _) =>
        {
            generate.Enabled = false; prompt.Enabled = false; modelBox.Enabled = false;
            status.Text = isEn ? $"Generating with {modelBox.Text}… (large local models can take a minute)" : $"正在以 {modelBox.Text} 規劃地圖…（大型本機模型可能需要一分鐘）";
            try
            {
                AiMaterialOption[] materials = _floorMaterials?.Materials.Select(material => new AiMaterialOption(material.Id, material.DisplayName + " / " + GetLocalizedMaterialName(material))).ToArray() ?? Array.Empty<AiMaterialOption>();
                float water = _heightMapStep > 0 ? (float)_waterLevel.Value / _heightMapStep : 0;
                (AiMapPlan plan, _) = await planner.GeneratePlanAsync(modelBox.Text.Trim(), prompt.Text, materials, water, cancellation.Token);
                AiMapApplyResult result = ApplyAiMapPlan(plan);
                status.Text = (plan.Summary is { Length: > 0 } summary ? summary + Environment.NewLine : "") + (isEn
                    ? $"Applied {plan.Features.Count} features: {result.HeightSamplesChanged} height samples, {result.MaterialStrokes - result.RejectedMaterialStrokes}/{result.MaterialStrokes} material areas, {result.CollisionPixelsChanged} passability pixels. Review, then Save."
                    : $"已套用 {plan.Features.Count} 個特徵：高度 {result.HeightSamplesChanged} 點、材質 {result.MaterialStrokes - result.RejectedMaterialStrokes}/{result.MaterialStrokes} 區、通行 {result.CollisionPixelsChanged} 點。請檢視後再按「儲存」。");
                if (result.RejectedMaterialStrokes > 0)
                    status.Text += isEn ? " Some material areas could not be expressed with the game's transition tiles and were skipped." : "部分材質區無法以原版 transition tile 表達，已略過。";
            }
            catch (OperationCanceledException) { }
            catch (Exception ex) when (ex is HttpRequestException or InvalidDataException or ArgumentException or System.Text.Json.JsonException or KeyNotFoundException)
            {
                status.Text = (isEn ? "AI map generation failed: " : "AI 製圖失敗：") + ex.Message;
            }
            finally
            {
                if (!dialog.IsDisposed) { generate.Enabled = true; prompt.Enabled = true; modelBox.Enabled = true; }
            }
        };
        dialog.ShowDialog(this);
    }

    /// <summary>把 AI 計畫套用到目前的高度／材質／通行狀態（每類各成一個可復原步驟），並更新 2D／3D 預覽。</summary>
    internal AiMapApplyResult ApplyAiMapPlan(AiMapPlan plan)
    {
        if (_terrainLayers is null || _texturesDocument is null) throw new InvalidOperationException("此地圖沒有可編輯的高度圖。");
        CommitStroke();
        float water = _heightMapStep > 0 ? (float)_waterLevel.Value / _heightMapStep : 0;
        AiMapApplyResult result = AiMapPlanApplier.Apply(plan, _terrainLayers, _texturesDocument.Dimension, water, (materialId, x, y, radius) =>
        {
            if (_terrainBlendSession is null) return false;
            TerrainBlendPaintResult paint = _terrainBlendSession.PaintCircle(x, y, radius, materialId);
            foreach (TerrainTextureChange change in paint.TextureChanges) ApplyTexture(change.X, change.Y, change.After);
            return paint.Succeeded;
        });
        _terrainLayers.CommitStroke();
        _terrainBlendSession?.CommitStroke();
        ApplyHeightsToViews();
        if (_editMode == EditMode.Collision) _canvas.SetCollisionOverlay(_terrainLayers.CollisionSize, _terrainLayers.Collision);
        UpdateEditorState();
        return result;
    }

    private void ApplyHeightsToViews()
    {
        if (_terrainLayers is null) return;
        if (!TryParseGameColor(_waterColor.Text, out Color waterColor)) waterColor = Color.SteelBlue;
        _canvas.SetHeightSamples(_terrainLayers.VertexSize, _terrainLayers.HeightsDirty ? _terrainLayers.Heights : null, (float)_waterLevel.Value, _heightMapStep, waterColor);
        _view3d?.SetHeightSamples(_terrainLayers.Heights);
    }

    private void ApplyTerrainLayerStroke(TerrainLayerStroke stroke)
    {
        if (stroke.Heights.Count > 0) ApplyHeightsToViews();
        if (stroke.Collision.Count > 0 && _editMode == EditMode.Collision) _canvas.SetCollisionOverlay(_terrainLayers!.CollisionSize, _terrainLayers.Collision);
    }

    private void MoveSelectedSceneObject(SceneObjectMoveEventArgs e)
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1 || SelectedSceneDisplay() is not { } source) return;
        if (_sceneList.SelectedItems[0].Tag is MapSceneObject pending && _sceneRemovals.Any(removal =>
            removal.SourceFile.Equals(pending.SourceFile, StringComparison.OrdinalIgnoreCase) && removal.ObjectIndex == pending.ObjectIndex)) return;

        // 拖曳落點是畫面上的有效座標（含暫存聚落平移）；換回以已存檔 refpos 為準的記憶體座標。
        MapSceneObject effectiveSource = WithSettlementOffset(source);
        MapSceneObject effectiveMoved = SceneObjectPositioning.MoveToWorldPosition(effectiveSource, e.WorldX, e.WorldZ);
        MapSceneObject moved = effectiveMoved with
        {
            WorldX = source.WorldX + effectiveMoved.WorldX - effectiveSource.WorldX,
            WorldZ = source.WorldZ + effectiveMoved.WorldZ - effectiveSource.WorldZ,
        };
        if (_sceneList.SelectedItems[0].Tag is StagedSceneAddition addition)
        {
            addition.Display = moved;
        }
        else if (_sceneList.SelectedItems[0].Tag is MapSceneObject selected)
        {
            int index = _sceneObjects.ToList().FindIndex(item => SceneKey(item) == SceneKey(selected));
            if (index < 0) return;
            MapSceneObject[] objects = _sceneObjects.ToArray();
            objects[index] = moved;
            _sceneObjects = objects;
            _sceneList.SelectedItems[0].Tag = moved;
        }
        else return;

        _sceneX.Value = ClampSceneCoordinate(moved.LocalX, _sceneX);
        _sceneY.Value = ClampSceneCoordinate(moved.LocalY, _sceneY);
        _sceneZ.Value = ClampSceneCoordinate(moved.LocalZ, _sceneZ);
        IReadOnlyList<MapSceneObject> effective = EffectiveSceneObjects();
        _canvas.UpdateSceneObjects(effective);
        _view3d?.UpdateSceneObjects(effective);
        UpdateEditorState();
        if (e.Completed)
        {
            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            _status.Text = isEn
                ? $"Object moved to world ({effectiveMoved.WorldX:0}, {effectiveMoved.WorldZ:0}); click Save to write the SDL change."
                : $"物件已移到世界座標 ({effectiveMoved.WorldX:0}, {effectiveMoved.WorldZ:0})；按「儲存」才會寫入 SDL。";
        }
    }

    private void ApplySelectedSceneObjectEdit()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1) return;
        float localX = (float)_sceneX.Value, localY = (float)_sceneY.Value, localZ = (float)_sceneZ.Value;
        int team = (int)_sceneTeam.Value;
        float angle = (float)_sceneAngle.Value;
        if (_sceneList.SelectedItems[0].Tag is StagedSceneAddition addition)
        {
            addition.Display = MoveSceneObject(addition.Display, team, localX, localY, localZ, angle);
        }
        else if (_sceneList.SelectedItems[0].Tag is MapSceneObject selected)
        {
            int index = _sceneObjects.ToList().FindIndex(item => SceneKey(item) == SceneKey(selected));
            if (index < 0) return;
            MapSceneObject[] objects = _sceneObjects.ToArray();
            objects[index] = MoveSceneObject(selected, team, localX, localY, localZ, angle);
            _sceneObjects = objects;
        }
        else return;
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    // 原檔沒有 angle 欄位的物件維持 null：不新增遊戲未定義的欄位。
    private static MapSceneObject MoveSceneObject(MapSceneObject source, int team, float localX, float localY, float localZ, float angle) => source with
    {
        Team = team,
        Angle = source.Angle is null ? null : angle,
        WorldX = source.WorldX + localX - source.LocalX,
        WorldY = source.WorldY + localY - source.LocalY,
        WorldZ = source.WorldZ + localZ - source.LocalZ,
        LocalX = localX,
        LocalY = localY,
        LocalZ = localZ
    };

    /// <summary>以選取物件為模板暫存一個複製件（唯一安全的新增路徑：所有欄位沿用原版物件）。</summary>
    private void DuplicateSelectedSceneObject()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1) return;
        (string templateFile, int templateIndex, MapSceneObject source) = _sceneList.SelectedItems[0].Tag switch
        {
            MapSceneObject item => (item.SourceFile, item.ObjectIndex, item),
            StagedSceneAddition staged => (staged.TemplateFile, staged.TemplateIndex, staged.Display),
            _ => (null!, -1, null!),
        };
        if (source is null) return;
        const float offset = 64f; // 錯開四分之一 tile，避免複製件與原件完全重疊而難以選取。
        int id = _nextSceneAdditionId++;
        var display = source with
        {
            ObjectIndex = -1000 - id, // 負索引：畫布顯示用，絕不寫入檔案；儲存時由 AddObject 重新編號。
            WorldX = source.WorldX + offset,
            WorldZ = source.WorldZ + offset,
            LocalX = source.LocalX + offset,
            LocalZ = source.LocalZ + offset,
        };
        _sceneAdditions.Add(new StagedSceneAddition { Id = id, TemplateFile = templateFile, TemplateIndex = templateIndex, FromCatalog = false, Display = display });
        LoadEditingScene(preserveView: true);
        SelectSceneListItem(tag => tag is StagedSceneAddition added && added.Id == id);
        UpdateEditorState();
    }

    /// <summary>暫存刪除選取物件；再按一次可取消。刪除暫存複製件則直接移除該複製件。</summary>
    private void ToggleDeleteSelectedSceneObject()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1) return;
        if (_sceneList.SelectedItems[0].Tag is StagedSceneAddition addition)
        {
            _sceneAdditions.RemoveAll(item => item.Id == addition.Id);
        }
        else if (_sceneList.SelectedItems[0].Tag is MapSceneObject selected)
        {
            int existing = _sceneRemovals.FindIndex(item =>
                item.SourceFile.Equals(selected.SourceFile, StringComparison.OrdinalIgnoreCase) && item.ObjectIndex == selected.ObjectIndex);
            if (existing >= 0) _sceneRemovals.RemoveAt(existing);
            else _sceneRemovals.Add(new SdlSceneObjectRemoval(selected.SourceFile, selected.ObjectIndex));
            string key = SceneKey(selected);
            LoadEditingScene(preserveView: true);
            SelectSceneListItem(tag => tag is MapSceneObject item && SceneKey(item) == key);
            UpdateEditorState();
            return;
        }
        else return;
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    /// <summary>
    /// 依物件類型自由新增：從本地圖既有物件挑一個同類型模板（沿用其 def/namedef/nation 等全部欄位），
    /// 放入使用者指定的聚落檔。新物件預設放在聚落原點附近，之後可拖曳或輸入座標調整。
    /// </summary>
    private void AddSceneObjectFromCatalog()
    {
        if (_selected?.IsCustom != true || _sceneObjects.Count == 0 || _settlementOrigins.Count == 0) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        MapSceneObject[] templates = _sceneObjects
            .GroupBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First())
            .OrderBy(item => item.Kind).ThenBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .ToArray();
        string[] settlements = _settlementOrigins.Keys.OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray();
        string defaultSettlement = SelectedSceneDisplay() is { } current && _settlementOrigins.ContainsKey(current.SourceFile) ? current.SourceFile : settlements[0];

        using var dialog = CreateSceneDialog(isEn ? "Add Scene Object" : "新增場景物件", 230);
        var table = (TableLayoutPanel)dialog.Controls[0];
        var typeBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        foreach (MapSceneObject template in templates) typeBox.Items.Add($"[{KindText(template.Kind, isEn)}] {template.Name}");
        var settlementBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        settlementBox.Items.AddRange(settlements);
        var teamBox = new NumericUpDown { Dock = DockStyle.Fill, Minimum = -1, Maximum = 15 };
        AddSceneField(table, 0, isEn ? "Type" : "物件類型", typeBox);
        AddSceneField(table, 1, isEn ? "Settlement" : "目標聚落", settlementBox);
        AddSceneField(table, 2, isEn ? "Team" : "隊伍", teamBox);
        settlementBox.SelectedIndexChanged += (_, _) =>
        {
            // 預設採用目標聚落中最常見的隊伍，讓新物件歸屬該聚落的勢力。
            string file = (string)settlementBox.SelectedItem!;
            int team = _sceneObjects.Where(item => item.SourceFile.Equals(file, StringComparison.OrdinalIgnoreCase))
                .GroupBy(item => item.Team).OrderByDescending(group => group.Count()).Select(group => group.Key).DefaultIfEmpty(-1).First();
            teamBox.Value = Math.Clamp(team, -1, 15);
        };
        typeBox.SelectedIndex = SelectedSceneDisplay() is { } selectedItem
            ? Math.Max(0, Array.FindIndex(templates, item => item.Name.Equals(selectedItem.Name, StringComparison.OrdinalIgnoreCase)))
            : 0;
        settlementBox.SelectedItem = defaultSettlement;
        if (dialog.ShowDialog(this) != DialogResult.OK || typeBox.SelectedIndex < 0 || settlementBox.SelectedItem is not string targetFile) return;

        MapSceneObject source = templates[typeBox.SelectedIndex];
        SdlVector3 origin = _settlementOrigins[targetFile];
        int id = _nextSceneAdditionId++;
        const float offset = 96f; // 避開聚落原點常見的主建築，方便選取。
        const float mapSize = SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize;
        float worldX = Math.Clamp(origin.X + offset, 0, mapSize), worldZ = Math.Clamp(origin.Z + offset, 0, mapSize);
        var display = source with
        {
            ObjectIndex = -1000 - id, // 負索引：畫布顯示用，絕不寫入檔案；儲存時由 AddObject 重新編號。
            SourceFile = targetFile,
            Team = (int)teamBox.Value,
            WorldX = worldX,
            WorldY = origin.Y,
            WorldZ = worldZ,
            LocalX = worldX - origin.X,
            LocalY = 0,
            LocalZ = worldZ - origin.Z,
        };
        _sceneAdditions.Add(new StagedSceneAddition { Id = id, TemplateFile = source.SourceFile, TemplateIndex = source.ObjectIndex, FromCatalog = true, Display = display });
        LoadEditingScene(preserveView: true);
        SelectSceneListItem(tag => tag is StagedSceneAddition added && added.Id == id);
        UpdateEditorState();
        _status.Text = isEn
            ? $"Added {source.Name} to {targetFile}; drag it or type a position, then Save."
            : $"已將 {source.Name} 暫存新增到 {targetFile}；可拖曳或輸入座標，按「儲存」才會寫入。";
    }

    /// <summary>整體平移選取物件所屬的聚落：只改 refpos，整個聚落（含待新增物件）一起移動。</summary>
    private void TranslateSelectedSettlement()
    {
        if (_selected?.IsCustom != true || SelectedSceneDisplay() is not { } selected || !_settlementOrigins.ContainsKey(selected.SourceFile)) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string file = selected.SourceFile;
        _settlementOffsets.TryGetValue(file, out SdlVector3 currentOffset);

        using var dialog = CreateSceneDialog(isEn ? $"Move Settlement - {file}" : $"平移聚落 - {file}", 230);
        var table = (TableLayoutPanel)dialog.Controls[0];
        NumericUpDown dx = SceneCoordinateInput(), dy = SceneCoordinateInput(), dz = SceneCoordinateInput();
        dx.Value = ClampSceneCoordinate(currentOffset.X, dx); dy.Value = ClampSceneCoordinate(currentOffset.Y, dy); dz.Value = ClampSceneCoordinate(currentOffset.Z, dz);
        AddSceneField(table, 0, isEn ? "Offset X" : "平移 X", dx);
        AddSceneField(table, 1, isEn ? "Offset Y" : "平移 Y", dy);
        AddSceneField(table, 2, isEn ? "Offset Z" : "平移 Z", dz);
        if (dialog.ShowDialog(this) != DialogResult.OK) return;

        var offset = new SdlVector3((float)dx.Value, (float)dy.Value, (float)dz.Value);
        const float mapSize = SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize;
        bool outside = _sceneObjects.Concat(_sceneAdditions.Select(item => item.Display))
            .Where(item => item.SourceFile.Equals(file, StringComparison.OrdinalIgnoreCase))
            .Any(item => item.WorldX + offset.X is < 0 or > mapSize || item.WorldZ + offset.Z is < 0 or > mapSize);
        if (outside)
        {
            MessageBox.Show(this, isEn ? "This offset would move part of the settlement outside the map." : "此平移量會讓部分聚落物件超出地圖範圍，已取消。",
                Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        if (offset == default) _settlementOffsets.Remove(file); else _settlementOffsets[file] = offset;
        LoadEditingScene(preserveView: true);
        SelectSceneListItem(tag => tag is MapSceneObject item && SceneKey(item) == SceneKey(selected)
            || tag is StagedSceneAddition added && ReferenceEquals(added.Display, selected));
        UpdateEditorState();
    }

    private Form CreateSceneDialog(string title, int height)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        var form = new Form { Text = title, Width = 460, Height = height, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor, FormBorderStyle = FormBorderStyle.FixedDialog, MinimizeBox = false, MaximizeBox = false, ShowInTaskbar = false };
        var table = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12) };
        table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 96)); table.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(12, 8, 12, 8), FlowDirection = FlowDirection.RightToLeft };
        var ok = new Button { Text = isEn ? "OK" : "確定", DialogResult = DialogResult.OK, Width = 100, Height = 34 };
        var cancel = new Button { Text = isEn ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, Width = 90, Height = 34 };
        buttons.Controls.Add(ok); buttons.Controls.Add(cancel);
        form.Controls.Add(table); form.Controls.Add(buttons);
        form.AcceptButton = ok; form.CancelButton = cancel;
        WinFormsTheme.Apply(form);
        WinFormsTheme.StylePrimaryButton(ok);
        return form;
    }

    private static string KindText(string kind, bool isEn) => !isEn ? kind : kind switch { "建築" => "Building", "單位" => "Unit", _ => "Other" };

    private void SelectSceneListItem(Func<object?, bool> match)
    {
        foreach (ListViewItem row in _sceneList.Items)
        {
            if (!match(row.Tag)) continue;
            row.Selected = true; row.EnsureVisible();
            return;
        }
    }

    private void RestoreOpeningSceneObjects()
    {
        if (_selected?.IsCustom != true) return;
        if (_sceneRemovals.Count == 0 && _sceneAdditions.Count == 0 && _settlementOffsets.Count == 0 && !SdlSceneEditService.HasChanges(_sceneOriginalObjects, _sceneObjects)) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string msg = isEn
            ? "Do you want to restore all unsaved SDL edits (teams, positions, angles, pending additions, deletions and settlement moves) to the state when this map was opened?\nYou still need to click \"Save\" to write them back to the custom map."
            : "要將所有待儲存的 SDL 變更（隊伍、位置、角度、待新增、待刪除與聚落平移）還原到本次開啟地圖時的狀態嗎？\n還原後仍需按「儲存」才會寫回自製地圖。";
        string title = isEn ? "Restore SDL Verification Changes" : "還原 SDL 驗證變更";
        if (MessageBox.Show(this, msg, title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        _sceneObjects = _sceneOriginalObjects.ToArray();
        _sceneRemovals.Clear(); _sceneAdditions.Clear(); _settlementOffsets.Clear();
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    private void UpdateSceneEditButtons()
    {
        bool editable = _selected?.IsCustom == true;
        bool selected = _sceneList.SelectedItems.Count == 1 && SelectedSceneDisplay() is not null;
        _sceneApplyButton.Enabled = editable && selected;
        _sceneDuplicateButton.Enabled = editable && selected;
        _sceneDeleteButton.Enabled = editable && selected;
        _sceneAddButton.Enabled = editable && _sceneObjects.Count > 0 && _settlementOrigins.Count > 0;
        _sceneTranslateButton.Enabled = editable && selected && SelectedSceneDisplay() is { } target && _settlementOrigins.ContainsKey(target.SourceFile);
        _sceneAngle.Enabled = editable && selected && SelectedSceneDisplay()?.Angle is not null;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        bool pendingRemoval = selected && _sceneList.SelectedItems[0].Tag is MapSceneObject item && _sceneRemovals.Any(removal =>
            removal.SourceFile.Equals(item.SourceFile, StringComparison.OrdinalIgnoreCase) && removal.ObjectIndex == item.ObjectIndex);
        bool canMove = editable && selected && !pendingRemoval && _sceneMoveTool.Checked;
        _canvas.SceneMoveEnabled = canMove;
        if (_view3d is not null) _view3d.SceneMoveEnabled = canMove;
        _sceneDeleteButton.Text = pendingRemoval
            ? (isEn ? "Undo Delete" : "取消刪除")
            : (isEn ? "Delete Object" : "刪除物件");
    }

    private bool SaveMap(bool showSuccess)
    {
        if (_selected is null || !_selected.IsCustom) return false;
        try
        {
            using var rollback = new FileRollbackScope(); string map = _selected.DirectoryPath;
            var put = PutTextDocument.Load(Path.Combine(map, "TEXT", "US", "briefing.put"));
            put.SetValue("briefing_titel_1", _title.Text.Trim()); put.SetValue("briefing_titel_2", _subtitle.Text.Trim()); put.SetCompositeValue("briefing_text", _briefing.Text);
            for (int index = 0; index < _teamNames.Length; index++) put.SetValue($"briefing_text_teamname{index}", _teamNames[index].Text.Trim());
            put.Save(rollback);
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini")); ini.SetValue("Waterlevel", _waterLevel.Value.ToString()); ini.SetValue("WaterColor", _waterColor.Text.Trim());
            ini.SetValue("WaterWarpShift", _waterWarpShift.Value.ToString()); ini.SetValue("WaterBumpAmplitude", _waterBumpAmplitude.Value.ToString()); ini.SetValue("WaterBumpFrequency", _waterBumpFrequency.Value.ToString()); ini.SetValue("FlashPropability", _flashProbability.Value.ToString());
            ini.SetValue("DayStartTime", _dayStart.Value.ToString()); ini.SetValue("DayEndTime", _dayEnd.Value.ToString()); ini.SetValue("RainDropsOnWater", _rain.Checked ? "1" : "0"); ini.Save(rollback);
            bool sceneStructureChanged = _sceneRemovals.Count > 0 || _sceneAdditions.Count > 0 || _settlementOffsets.Count > 0;
            SdlSceneEditService.SaveChanges(map, _sceneSavedObjects, _sceneObjects, rollback,
                _sceneRemovals, _sceneAdditions.Select(item => item.ToAddition()).ToArray(),
                _settlementOffsets.Select(pair => new SdlSettlementTranslation(pair.Key, pair.Value.X, pair.Value.Y, pair.Value.Z)).ToArray());
            _texturesDocument?.Save(rollback);
            bool heightsChanged = _terrainLayers?.HeightsDirty == true && _bodenLayer is not null;
            bool collisionChanged = _terrainLayers?.CollisionDirty == true && _collisionLayer is not null;
            byte[]? savedEmboss = null;
            if (heightsChanged)
            {
                TerrainLayerFiles.Write(Path.Combine(map, "boden.bmp"), _bodenLayer!, _terrainLayers!.Heights, rollback);
                savedEmboss = _terrainLayers.BuildEmboss();
                if (savedEmboss is not null && _embossLayer is not null) TerrainLayerFiles.Write(Path.Combine(map, "emboss.bmp"), _embossLayer, savedEmboss, rollback);
                // skydens／visible／cliprect／shadows.dat 以高度總和為鍵；刪除後由遊戲在載入時重算。
                TerrainLayerFiles.InvalidateHeightCaches(map, rollback);
            }
            if (collisionChanged) TerrainLayerFiles.Write(Path.Combine(map, "collision.bmp"), _collisionLayer!, _terrainLayers!.Collision!, rollback);
            // 只有地表確實被繪製過才重生小地圖，避免僅改標題／水面等屬性時用近似圖覆蓋原始 minimap.bmp。
            if (TextureDirty() || heightsChanged)
            {
                byte[]? minimap = _canvas.RenderMinimapBmp();
                if (minimap is not null) AgainstRomeModifier.Core.Services.SafeFileWriter.WriteAllBytes(Path.Combine(map, "minimap.bmp"), minimap, rollback);
            }
            rollback.Commit();
            if (heightsChanged || collisionChanged)
            {
                // 以寫回後的檔案作為下一次保留原像素的基準；光照係數沿用開圖時由原版資料擬合的值。
                if (heightsChanged) { _bodenLayer = TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp")); if (_embossLayer is not null) _embossLayer = TerrainLayerFiles.Read(Path.Combine(map, "emboss.bmp")); }
                if (collisionChanged) _collisionLayer = TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"));
                _terrainLayers!.CommitBaseline(savedEmboss);
            }
            _terrainBlendSession?.CommitBaseline(); _savedTextures = _terrainBlendSession?.CurrentTextures.ToArray() ?? _texturesDocument?.Textures.ToArray() ?? Array.Empty<string>(); _propertyDirty = false;
            if (sceneStructureChanged)
            {
                // 複製／刪除已寫回並重新編號，記憶體中的 object 索引不再對應檔案；
                // 從磁碟重讀並重定基準（「還原到本次開啟時」自此以本次儲存後狀態為起點）。
                _sceneRemovals.Clear(); _sceneAdditions.Clear(); _settlementOffsets.Clear();
                _sceneObjects = SdlSceneCatalog.LoadDirectory(map);
                _settlementOrigins = SdlSceneCatalog.LoadSettlementOrigins(map);
                _sceneOriginalObjects = _sceneObjects.ToArray();
                _sceneSavedObjects = _sceneObjects.ToArray();
                LoadEditingScene(preserveView: true);
            }
            else _sceneSavedObjects = _sceneObjects.ToArray();
            _canvas.CommitBaseline(); // 變更高亮只存在於 2D 檢視，3D 無對應狀態
            RefreshOverview();
            UpdateEditorState();
            if (showSuccess)
            {
                bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
                MessageBox.Show(this, isEn ? "Map saved successfully." : "地圖已安全儲存。", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
            return true;
        }
        catch (Exception ex) { ShowError(ex); return false; }
    }

    private void PreviewInGame()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected is null) { MessageBox.Show(this, isEn ? "Please select a map to preview." : "請先選擇要預覽的地圖。", Text); return; }
        if (_selected.IsCustom && IsDirty && !SaveMap(showSuccess: false)) return;
        string exePath = Path.Combine(_gamePath, "Against_Rome.exe");
        if (!File.Exists(exePath)) { MessageBox.Show(this, isEn ? "Against_Rome.exe not found in game folder." : "遊戲路徑中找不到 Against_Rome.exe。", Text, MessageBoxButtons.OK, MessageBoxIcon.Error); return; }
        try
        {
            Process.Start(new ProcessStartInfo(exePath) { WorkingDirectory = _gamePath, UseShellExecute = true });
            string msg = isEn
                ? $"Game launched.\n\nTo test the custom map, enter \"Endless Mode\" and select {_selected.Id} ({_selected.DisplayName ?? "Unnamed"}).\nMap editing and offline 3D view do not require launching the game."
                : $"遊戲已啟動。\n\n若要額外測試自製地圖，請進入「無盡模式」並選擇 {_selected.Id}（{_selected.DisplayName ?? "未命名"}）。\n地圖編輯與離線場景顯示不需要啟動遊戲。";
            string title = isEn ? "Optional Game Test" : "選用遊戲測試";
            MessageBox.Show(this, msg, title, MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
        catch (Exception ex) { ShowError(ex); }
    }

    private bool ConfirmDiscardOrSave()
    {
        if (!IsDirty) return true;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string msg = isEn 
            ? "The current map has unsaved changes.\n\nYes: Save and continue\nNo: Discard changes\nCancel: Stay on current map" 
            : "目前地圖有尚未儲存的變更。\n\n是：儲存後繼續\n否：放棄變更\n取消：留在目前地圖";
        string title = isEn ? "Unsaved Changes" : "尚未儲存";
        DialogResult result = MessageBox.Show(this, msg, title, MessageBoxButtons.YesNoCancel, MessageBoxIcon.Warning);
        return result switch { DialogResult.Yes => SaveMap(showSuccess: false), DialogResult.No => true, _ => false };
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
            _ => category
        };
    }

    private void LoadPalette(string? filter = null)
    {
        string? selected = (_palette.SelectedItem as PaletteItem)?.Key ?? _activeMaterial?.Id ?? _canvas.BrushTexture; _palette.Items.Clear();
        if (_texturesDocument is null) return;
        IEnumerable<FloorMaterial> materials = _floorMaterials?.Materials ?? Array.Empty<FloorMaterial>();
        if (!string.IsNullOrWhiteSpace(filter)) materials = materials.Where(material =>
            GetLocalizedMaterialName(material).Contains(filter, StringComparison.CurrentCultureIgnoreCase) ||
            GetLocalizedCategory(material.Category).Contains(filter, StringComparison.CurrentCultureIgnoreCase));
        PaletteItem[] items = materials.Select(material => new PaletteItem(material.Id, material.RepresentativeTexture, GetLocalizedMaterialName(material), material)).ToArray();
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

    private void SelectBrush(PaletteItem item)
    {
        _activeMaterial = item.Material;
        _canvas.BrushTexture = item.PreviewTexture; if (_view3d is not null) _view3d.BrushTexture = item.PreviewTexture;
        _currentMaterialSwatch.Image = _floorTextures?.Get(item.PreviewTexture);
        _currentMaterialSwatch.BackColor = _canvas.GetTexturePreviewColor(item.PreviewTexture);
        _currentMaterialLabel.Text = (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Active Brush: " : "目前筆刷：") + item.Name;
        UpdateStatus();
    }

    private void SelectSampledTexture(string texture)
    {
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
        bool editable = _selected?.IsCustom == true; _saveButton.Enabled = editable && IsDirty; _gamePreviewButton.Enabled = _selected is not null; _undoButton.Enabled = editable && (TerrainLayerMode ? _terrainLayers?.CanUndo == true : _terrainBlendSession?.CanUndo == true); _redoButton.Enabled = editable && (TerrainLayerMode ? _terrainLayers?.CanRedo == true : _terrainBlendSession?.CanRedo == true); _resetTerrainButton.Enabled = editable && ((_texturesDocument is not null && TextureDirty()) || _terrainLayers?.IsDirty == true);
        _heightTool.Enabled = editable && _terrainLayers is not null; _aiMapButton.Enabled = editable && _terrainLayers is not null; _collisionTool.Enabled = editable && _terrainLayers?.HasCollision == true;
        UpdateSceneEditButtons();
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
    }

    private void Disable3DView(string reason, Exception? exception = null)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _last3DDiagnostic = Build3DDiagnostic(reason, exception);
        _view3dButton.Enabled = false;
        _view3dButton.ToolTipText = reason;
        _3dDiagnosticsButton.Visible = true;
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

    private void ResetTerrain()
    {
        if (_texturesDocument is null || _savedTextures.Length != _texturesDocument.Textures.Count) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string msg = isEn
            ? "Do you want to discard unsaved terrain drawing changes?\nMap metadata and environment settings will not be affected."
            : "要放棄這次尚未儲存的地表繪製嗎？\n地圖名稱與環境設定不會受影響。";
        string title = isEn ? "Reset Terrain" : "還原地表";
        if (MessageBox.Show(this, msg, title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        if (_terrainBlendSession is null) return;
        IReadOnlyList<TerrainTextureChange> changes = _terrainBlendSession.ResetToBaseline();
        _texturesDocument.SetTextures(_terrainBlendSession.CurrentTextures); // 批次寫回，避免逐格重新解析整份 boden.txt。
        foreach (TerrainTextureChange change in changes)
        {
            _canvas.SetTexture(change.X, change.Y, change.After); _view3d?.SetTexture(change.X, change.Y, change.After);
        }
        _terrainBlendNotice = null;
        UpdateEditorState();
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
    private static decimal ParseDecimal(string? value, NumericUpDown control) => decimal.TryParse(value, out decimal parsed) ? Math.Clamp(parsed, control.Minimum, control.Maximum) : control.Minimum;
    
    private void ShowError(Exception ex)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        MessageBox.Show(this, ex.Message, isEn ? "Map Editor Error" : "地圖編輯器錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing) { _floorTextures?.Dispose(); _floorTextures = null; Image? overview = _overview.Image; _overview.Image = null; overview?.Dispose(); }
        base.Dispose(disposing);
    }

    private sealed record PaletteItem(string Key, string PreviewTexture, string Name, FloorMaterial? Material);
}

