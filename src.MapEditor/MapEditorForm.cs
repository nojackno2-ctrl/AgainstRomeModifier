using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using System.Diagnostics;

namespace AgainstRomeMapEditor;

internal sealed class MapEditorForm : Form
{
    private readonly MapCanvasControl _canvas = new();
    private Map3DViewControl? _view3d;
    private Panel? _canvasHost;
    private readonly PictureBox _overview = new() { Dock = DockStyle.Fill, SizeMode = PictureBoxSizeMode.Zoom, BackColor = Color.FromArgb(18, 21, 27) };
    private readonly ListBox _palette = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly TextBox _paletteSearch = new() { Dock = DockStyle.Top, PlaceholderText = "搜尋草地、沙地、泥土、岩地…" };
    private readonly PictureBox _currentMaterialSwatch = new() { Width = 54, Height = 54, BackColor = Color.DimGray, SizeMode = PictureBoxSizeMode.Zoom };
    private readonly Label _currentMaterialLabel = new() { AutoSize = true, Text = "目前筆刷：尚未取樣", ForeColor = Color.White, Font = new Font("Microsoft JhengHei UI", 10F, FontStyle.Bold) };
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
    private readonly Label _sceneSummary = new() { Dock = DockStyle.Top, Height = 54, Padding = new Padding(8), ForeColor = Color.Gainsboro };
    private readonly NumericUpDown _sceneTeam = new() { Dock = DockStyle.Fill, Minimum = -1, Maximum = 15 };
    private readonly NumericUpDown _sceneX = SceneCoordinateInput();
    private readonly NumericUpDown _sceneY = SceneCoordinateInput();
    private readonly NumericUpDown _sceneZ = SceneCoordinateInput();
    private readonly Button _sceneApplyButton = new() { Dock = DockStyle.Fill, Height = 32, Text = "套用至待儲存", Enabled = false };
    private readonly Button _sceneRestoreButton = new() { Dock = DockStyle.Fill, Height = 32, Text = "還原到本次開啟時", Enabled = false };
    private readonly Label _modeBanner = new() { Dock = DockStyle.Top, Height = 34, TextAlign = ContentAlignment.MiddleCenter };
    private readonly ToolStripStatusLabel _status = new() { Spring = true, TextAlign = ContentAlignment.MiddleLeft };
    private readonly ToolStripButton _saveButton = new("儲存") { Enabled = false };
    private readonly ToolStripButton _gamePreviewButton = new("選用：啟動遊戲測試") { Enabled = false };
    private readonly ToolStripButton _undoButton = new("復原") { Enabled = false };
    private readonly ToolStripButton _redoButton = new("重做") { Enabled = false };
    private readonly ToolStripButton _textureTool = new("材質筆刷") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _resetTerrainButton = new("還原地表") { Enabled = false };
    private readonly ToolStripButton _view2dButton = new("2D 俯視") { CheckOnClick = true };
    private readonly ToolStripButton _view3dButton = new("3D 場景") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _3dDiagnosticsButton = new("3D 診斷") { Visible = false };
    private readonly ToolStripButton _mapMenuButton = new("地圖選單");
    private readonly ToolStripLabel _currentMapLabel = new();
    private GameMapInfo? _selected;
    private BodenTexturesDocument? _texturesDocument;
    private TerrainBlendEditSession? _terrainBlendSession;
    private FloorTextureLibrary? _floorTextures;
    private FloorMaterialCatalog? _floorMaterials;
    private FloorMaterial? _activeMaterial;
    private IReadOnlyList<MapSceneObject> _sceneObjects = Array.Empty<MapSceneObject>();
    private IReadOnlyList<MapSceneObject> _sceneOriginalObjects = Array.Empty<MapSceneObject>();
    private IReadOnlyList<MapSceneObject> _sceneSavedObjects = Array.Empty<MapSceneObject>();
    private string[] _savedTextures = Array.Empty<string>();
    private bool _propertyDirty;
    private bool _loading;
    private float _heightMapStep = 4;
    private bool _allowClose;
    private bool _sceneLoaded;
    private string? _last3DDiagnostic;
    private string? _terrainBlendNotice;

    private bool TextureDirty() => _terrainBlendSession?.IsDirty == true;

    private bool SceneDirty() => _sceneLoaded && SdlSceneEditService.HasChanges(_sceneSavedObjects, _sceneObjects);

    private bool IsDirty => _propertyDirty || TextureDirty() || SceneDirty();

    public MapEditorForm(string gamePath, GameMapInfo selectedMap)
    {
        Text = "Against Rome 地圖編輯器";
        Width = 1440; Height = 900; MinimumSize = new Size(1100, 700); StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(30, 34, 42); ForeColor = Color.Gainsboro;
        _gamePath = gamePath;
        _selected = selectedMap;
        _floorTextures = new FloorTextureLibrary(Path.Combine(gamePath, "floortex.dat"));
        _floorMaterials = new FloorMaterialCatalog(_floorTextures);
        BuildInterface(); WireEvents();
        KeyPreview = true;
        _currentMapLabel.Text = $"目前地圖：{selectedMap.DisplayName ?? selectedMap.Id}（{selectedMap.Id}）";
        Shown += (_, _) => LoadSelectedMap();
        FormClosing += (_, e) => { if (!_allowClose && !ConfirmDiscardOrSave()) e.Cancel = true; };
    }

    public bool ReturnToMapMenu { get; private set; }

    private void BuildInterface()
    {
        var commands = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(8, 5, 8, 5), BackColor = Color.FromArgb(42, 47, 58), ForeColor = Color.White, RenderMode = ToolStripRenderMode.System };
        commands.Items.AddRange(new ToolStripItem[] { _mapMenuButton, new ToolStripSeparator(), _currentMapLabel, new ToolStripSeparator(), _saveButton, _gamePreviewButton, new ToolStripSeparator(), _undoButton, _redoButton });

        var tools = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(8, 4, 8, 4), BackColor = Color.FromArgb(36, 40, 49), ForeColor = Color.White };
        tools.Items.AddRange(new ToolStripItem[] { new ToolStripLabel("地表："), _textureTool, _resetTerrainButton, new ToolStripSeparator(), _view2dButton, _view3dButton, _3dDiagnosticsButton });

        var paletteHeader = SectionHeader("地表繪製");
        var palettePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        var currentBrush = new FlowLayoutPanel { Dock = DockStyle.Top, Height = 76, Padding = new Padding(4), FlowDirection = FlowDirection.LeftToRight };
        var currentText = new FlowLayoutPanel { Width = 205, Height = 66, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        currentText.Controls.Add(_currentMaterialLabel); currentText.Controls.Add(new Label { AutoSize = true, MaximumSize = new Size(200, 0), Text = "右鍵取樣，左鍵拖曳繪製。", ForeColor = Color.Silver });
        currentBrush.Controls.Add(_currentMaterialSwatch); currentBrush.Controls.Add(currentText);
        _brushSize.Items.AddRange(new object[] { "精細（1 格）", "中型（3 × 3）", "大型（5 × 5）" }); _brushSize.SelectedIndex = 0;
        var brushOptions = new Panel { Dock = DockStyle.Top, Height = 174 };
        brushOptions.Controls.Add(_showObjects); brushOptions.Controls.Add(_showGrid); brushOptions.Controls.Add(_reliefScale); brushOptions.Controls.Add(new Label { Dock = DockStyle.Top, Height = 22, Text = "地形起伏（近似顯示）", ForeColor = Color.Gainsboro }); brushOptions.Controls.Add(new Label { Dock = DockStyle.Top, Height = 22, Text = "筆刷大小", ForeColor = Color.Gainsboro }); brushOptions.Controls.Add(_brushSize);
        palettePanel.Controls.Add(_palette); palettePanel.Controls.Add(_paletteSearch); palettePanel.Controls.Add(brushOptions); palettePanel.Controls.Add(currentBrush); palettePanel.Controls.Add(paletteHeader);

        var properties = BuildPropertiesPanel();
        var inspectorTabs = new TabControl { Dock = DockStyle.Fill };
        inspectorTabs.TabPages.Add(new TabPage("地表") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[0].Controls.Add(palettePanel);
        inspectorTabs.TabPages.Add(new TabPage("地圖屬性") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[1].Controls.Add(properties);
        var scenePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        _sceneList.Columns.Add("類型", 65); _sceneList.Columns.Add("物件", 170); _sceneList.Columns.Add("隊伍", 65); _sceneList.Columns.Add("來源", 120);
        var sceneEditor = new TableLayoutPanel { Dock = DockStyle.Bottom, Height = 232, ColumnCount = 2, Padding = new Padding(4), BackColor = Color.FromArgb(40, 45, 55) };
        sceneEditor.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 74)); sceneEditor.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var sceneWarning = new Label { AutoSize = true, MaximumSize = new Size(250, 0), Text = "SDL 驗證功能：只修改自製地圖既有物件的隊伍與相對座標。請先選取物件。", ForeColor = Color.Khaki };
        sceneEditor.Controls.Add(sceneWarning, 0, 0); sceneEditor.SetColumnSpan(sceneWarning, 2);
        AddSceneField(sceneEditor, 1, "隊伍", _sceneTeam); AddSceneField(sceneEditor, 2, "相對 X", _sceneX); AddSceneField(sceneEditor, 3, "相對 Y", _sceneY); AddSceneField(sceneEditor, 4, "相對 Z", _sceneZ);
        sceneEditor.Controls.Add(_sceneApplyButton, 0, 5); sceneEditor.Controls.Add(_sceneRestoreButton, 1, 5);
        scenePanel.Controls.Add(_sceneList); scenePanel.Controls.Add(sceneEditor); scenePanel.Controls.Add(_sceneSummary);
        inspectorTabs.TabPages.Add(new TabPage("場景物件") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[2].Controls.Add(scenePanel);

        _canvasHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(20, 23, 29) };
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
        var overviewHost = new Panel { Width = 190, Height = 190, Anchor = AnchorStyles.Right | AnchorStyles.Bottom, Padding = new Padding(5), BackColor = Color.FromArgb(55, 61, 72) };
        overviewHost.Controls.Add(_overview); overviewHost.Controls.Add(new Label { Text = "地圖概覽", Dock = DockStyle.Top, Height = 25, TextAlign = ContentAlignment.MiddleCenter, ForeColor = Color.White, BackColor = Color.FromArgb(42, 47, 58) });
        _canvasHost.Controls.Add(overviewHost); overviewHost.BringToFront();
        _canvasHost.Resize += (_, _) => overviewHost.Location = new Point(Math.Max(12, _canvasHost.ClientSize.Width - overviewHost.Width - 18), Math.Max(46, _canvasHost.ClientSize.Height - overviewHost.Height - 18));

        var centerRight = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1140, 760), SplitterDistance = 820 };
        centerRight.Panel1.Controls.Add(_canvasHost); centerRight.Panel2.Controls.Add(inspectorTabs); centerRight.Panel2MinSize = 280;
        var statusStrip = new StatusStrip { BackColor = Color.FromArgb(42, 47, 58), ForeColor = Color.Gainsboro };
        statusStrip.Items.Add(_status); statusStrip.Items.Add(new ToolStripStatusLabel("滾輪縮放　中鍵平移　右鍵取樣　左鍵繪製　Ctrl+S 儲存"));
        Controls.Add(centerRight); Controls.Add(tools); Controls.Add(commands); Controls.Add(statusStrip);
        commands.BringToFront(); tools.BringToFront();
        SetActiveView(_view3d is not null);
    }

    private Panel BuildPropertiesPanel()
    {
        var table = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 1, Padding = new Padding(10) };
        AddField(table, "地圖名稱", _title); AddField(table, "地圖副標題", _subtitle); AddField(table, "任務說明", _briefing);
        var teams = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 2 };
        teams.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 60)); teams.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        for (int index = 0; index < _teamNames.Length; index++)
        {
            teams.Controls.Add(new Label { Text = $"隊伍 {index}", AutoSize = true, Anchor = AnchorStyles.Left, ForeColor = Color.Gainsboro }, 0, index);
            teams.Controls.Add(_teamNames[index], 1, index);
        }
        AddField(table, "隊伍名稱", teams); AddField(table, "水面高度", _waterLevel); AddField(table, "水面顏色", _waterColorButton);
        AddField(table, "水面波動位移", _waterWarpShift); AddField(table, "水面凹凸幅度", _waterBumpAmplitude); AddField(table, "水面凹凸頻率", _waterBumpFrequency);
        AddField(table, "每秒閃電機率", _flashProbability);
        AddField(table, "日出時間", _dayStart); AddField(table, "日落時間", _dayEnd); AddField(table, "", _rain);
        var panel = new Panel { Dock = DockStyle.Fill, AutoScroll = true, BackColor = Color.FromArgb(34, 38, 47) }; panel.Controls.Add(table); return panel;
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
        _canvas.TexturePainted += (_, e) => PaintTexture(e);
        _canvas.TextureSampled += (_, e) => SelectSampledTexture(e.Texture);
        _canvas.StrokeEnded += (_, _) => CommitStroke();
        _canvas.TileHovered += (_, e) => ShowTerrainHover(e);
        if (_view3d is not null)
        {
            Map3DViewControl view3d = _view3d;
            _view3d.TexturePainted += (_, e) => PaintTexture(e);
            _view3d.TextureSampled += (_, e) => SelectSampledTexture(e.Texture);
            _view3d.StrokeEnded += (_, _) => CommitStroke();
            _view3d.TileHovered += (_, e) => ShowTerrainHover(e);
            _view3d.InitializationFailed += (_, ex) => BeginInvoke(() => Disable3DView(view3d.LastFailureReason ?? "OpenGL 3.3 初始化失敗。", ex));
        }
        _view2dButton.Click += (_, _) => SetActiveView(use3D: false);
        _view3dButton.Click += (_, _) => SetActiveView(use3D: true);
        _3dDiagnosticsButton.Click += (_, _) => Show3DDiagnostics();
        _mapMenuButton.Click += (_, _) => ReturnToMenu();
        _textureTool.Click += (_, _) => LoadEditingScene();
        _resetTerrainButton.Click += (_, _) => ResetTerrain();
        _saveButton.Click += (_, _) => SaveMap(showSuccess: true);
        _gamePreviewButton.Click += (_, _) => PreviewInGame();
        _undoButton.Click += (_, _) => Undo(); _redoButton.Click += (_, _) => Redo();
        KeyDown += (_, e) => HandleShortcut(e);
        foreach (Control control in EditablePropertyControls())
        {
            if (control is TextBox text) text.TextChanged += (_, _) => MarkDirty();
            else if (control is NumericUpDown numeric) numeric.ValueChanged += (_, _) => MarkDirty();
            else if (control is CheckBox check) check.CheckedChanged += (_, _) => MarkDirty();
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
            _texturesDocument = BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")); _savedTextures = _texturesDocument.Textures.ToArray(); InitializeTerrainBlendSession(); _propertyDirty = false;
            _sceneObjects = SdlSceneCatalog.LoadDirectory(map); _sceneOriginalObjects = _sceneObjects.ToArray(); _sceneSavedObjects = _sceneObjects.ToArray(); _sceneLoaded = true;
            LoadPalette(); LoadEditingScene(); UpdateEditorState();
        }
        catch (Exception ex) { ShowError(ex); }
        finally { _loading = false; }
    }

    private void LoadEditingScene(bool preserveView = false)
    {
        if (_selected is null) return;
        _textureTool.Checked = true;
        string minimapPath = Path.Combine(_selected.DirectoryPath, "minimap.bmp");
        LoadSceneList(_sceneObjects);
        if (!TryParseGameColor(_waterColor.Text, out Color sceneWaterColor)) sceneWaterColor = Color.SteelBlue;
        bool hasRealTextures = _texturesDocument is not null && _floorTextures is not null && _canvas.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _savedTextures, _selected.DirectoryPath, _floorTextures, _sceneObjects, (float)_waterLevel.Value, _heightMapStep, sceneWaterColor, preserveView);
        bool has3DScene = false;
        if (_view3d is not null && _texturesDocument is not null && _floorTextures is not null)
        {
            _view3d.BrushTexture = _canvas.BrushTexture;
            _view3d.BrushSize = _canvas.BrushSize;
            _view3d.ShowGrid = _showGrid.Checked;
            _view3d.ShowObjects = _showObjects.Checked;
            _view3d.EditingEnabled = _selected.IsCustom;
            try { has3DScene = _view3d.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _savedTextures, _selected.DirectoryPath, _floorTextures, _sceneObjects, (float)_waterLevel.Value, _heightMapStep, sceneWaterColor); _view3d.SetReliefScale(_reliefScale.Value / 100f); }
            catch (Exception ex) { Disable3DView("載入 3D 地圖資源失敗。", ex); }
        }
        Image? oldOverview = _overview.Image; _overview.Image = null; oldOverview?.Dispose();
        if (File.Exists(minimapPath)) using (var source = new Bitmap(minimapPath)) _overview.Image = new Bitmap(source);
        _canvas.EditingEnabled = _selected.IsCustom;
        _modeBanner.Text = hasRealTextures
            ? (_view3dButton.Checked && has3DScene
                ? (_selected.IsCustom ? $"離線 3D 場景（近似顯示）— 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件" : $"離線 3D 場景（近似顯示）— 原廠地圖僅供瀏覽，含 {_canvas.SceneObjectCount} 個場景物件")
                : (_selected.IsCustom ? $"離線地圖場景 — 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件；滾輪縮放，中鍵平移" : $"離線地圖場景 — 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件；原廠地圖僅供瀏覽"))
            : "找不到 floortex.dat，目前只能顯示簡化地表；請選擇完整的遊戲資料夾";
        _modeBanner.BackColor = _canvas.EditingEnabled ? Color.FromArgb(38, 95, 72) : Color.FromArgb(86, 69, 40);
        if (!has3DScene)
        {
            if (_view3d is not null && _view3dButton.Enabled) Disable3DView(_view3d.LastFailureReason ?? "3D 場景缺少必要資源。");
            else
            {
                SetActiveView(use3D: false);
                if (_last3DDiagnostic is not null)
                {
                    _modeBanner.Text = "3D 無法啟用；請按「3D 診斷」查看實際原因。";
                    _modeBanner.BackColor = Color.FromArgb(86, 69, 40);
                }
            }
        }
        UpdateStatus();
    }

    private void PaintTexture(TexturePaintEventArgs e)
    {
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
        if (_terrainBlendSession?.CommitStroke() != true) return;
        UpdateEditorState();
    }

    private void ApplyTexture(int x, int y, string texture)
    {
        _texturesDocument!.SetTexture(x, y, texture); _canvas.SetTexture(x, y, texture); _view3d?.SetTexture(x, y, texture);
    }

    private void Undo()
    {
        if (_texturesDocument is null || _terrainBlendSession?.Undo() is not { } stroke) return;
        foreach (TerrainTextureChange change in stroke) ApplyTexture(change.X, change.Y, change.After);
        _terrainBlendNotice = null;
        UpdateEditorState();
    }

    private void Redo()
    {
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

    private void FocusSelectedSceneObject()
    {
        if (_sceneList.SelectedItems.Count != 1 || _sceneList.SelectedItems[0].Tag is not MapSceneObject item) return;
        float tileX = item.WorldX / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
        float tileZ = item.WorldZ / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
        _canvas.FocusTile(tileX, tileZ);
        _view3d?.FocusTile(tileX, tileZ);
    }

    private void SelectSceneObject()
    {
        bool selected = _sceneList.SelectedItems.Count == 1 && _sceneList.SelectedItems[0].Tag is MapSceneObject;
        _sceneApplyButton.Enabled = selected && _selected?.IsCustom == true;
        if (!selected || _sceneList.SelectedItems[0].Tag is not MapSceneObject item) return;
        _sceneTeam.Value = Math.Clamp(item.Team, (int)_sceneTeam.Minimum, (int)_sceneTeam.Maximum);
        _sceneX.Value = ClampSceneCoordinate(item.LocalX, _sceneX);
        _sceneY.Value = ClampSceneCoordinate(item.LocalY, _sceneY);
        _sceneZ.Value = ClampSceneCoordinate(item.LocalZ, _sceneZ);
        FocusSelectedSceneObject();
    }

    private void ApplySelectedSceneObjectEdit()
    {
        if (_selected?.IsCustom != true || _sceneList.SelectedItems.Count != 1 || _sceneList.SelectedItems[0].Tag is not MapSceneObject selected) return;
        int index = _sceneObjects.ToList().FindIndex(item => SceneKey(item) == SceneKey(selected));
        if (index < 0) return;
        float localX = (float)_sceneX.Value, localY = (float)_sceneY.Value, localZ = (float)_sceneZ.Value;
        MapSceneObject updated = selected with
        {
            Team = (int)_sceneTeam.Value,
            WorldX = selected.WorldX + localX - selected.LocalX,
            WorldY = selected.WorldY + localY - selected.LocalY,
            WorldZ = selected.WorldZ + localZ - selected.LocalZ,
            LocalX = localX,
            LocalY = localY,
            LocalZ = localZ
        };
        MapSceneObject[] objects = _sceneObjects.ToArray(); objects[index] = updated; _sceneObjects = objects;
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
    }

    private void RestoreOpeningSceneObjects()
    {
        if (_selected?.IsCustom != true || !SdlSceneEditService.HasChanges(_sceneOriginalObjects, _sceneObjects)) return;
        if (MessageBox.Show(this, "要將所有待儲存的 SDL 隊伍與位置還原到本次開啟地圖時的值嗎？\n還原後仍需按「儲存」才會寫回自製地圖。", "還原 SDL 驗證變更", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        _sceneObjects = _sceneOriginalObjects.ToArray();
        LoadEditingScene(preserveView: true);
        UpdateEditorState();
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
            SdlSceneEditService.SaveChanges(map, _sceneSavedObjects, _sceneObjects, rollback);
            _texturesDocument?.Save(rollback);
            // 只有地表確實被繪製過才重生小地圖，避免僅改標題／水面等屬性時用近似圖覆蓋原始 minimap.bmp。
            if (TextureDirty())
            {
                byte[]? minimap = _canvas.RenderMinimapBmp();
                if (minimap is not null) AgainstRomeModifier.Core.Services.SafeFileWriter.WriteAllBytes(Path.Combine(map, "minimap.bmp"), minimap, rollback);
            }
            rollback.Commit();
            _terrainBlendSession?.CommitBaseline(); _savedTextures = _terrainBlendSession?.CurrentTextures.ToArray() ?? _texturesDocument?.Textures.ToArray() ?? Array.Empty<string>(); _sceneSavedObjects = _sceneObjects.ToArray(); _propertyDirty = false;
            _canvas.CommitBaseline(); _view3d?.CommitBaseline();
            RefreshOverview();
            UpdateEditorState();
            if (showSuccess) MessageBox.Show(this, "地圖已安全儲存。", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            return true;
        }
        catch (Exception ex) { ShowError(ex); return false; }
    }

    private void PreviewInGame()
    {
        if (_selected is null) { MessageBox.Show(this, "請先選擇要預覽的地圖。", Text); return; }
        if (_selected.IsCustom && IsDirty && !SaveMap(showSuccess: false)) return;
        string exePath = Path.Combine(_gamePath, "Against_Rome.exe");
        if (!File.Exists(exePath)) { MessageBox.Show(this, "遊戲路徑中找不到 Against_Rome.exe。", Text, MessageBoxButtons.OK, MessageBoxIcon.Error); return; }
        try
        {
            Process.Start(new ProcessStartInfo(exePath) { WorkingDirectory = _gamePath, UseShellExecute = true });
            MessageBox.Show(this, $"遊戲已啟動。\n\n若要額外測試自製地圖，請進入「無盡模式」並選擇 {_selected.Id}（{_selected.DisplayName ?? "未命名"}）。\n地圖編輯與離線場景顯示不需要啟動遊戲。", "選用遊戲測試", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
        catch (Exception ex) { ShowError(ex); }
    }

    private bool ConfirmDiscardOrSave()
    {
        if (!IsDirty) return true;
        DialogResult result = MessageBox.Show(this, "目前地圖有尚未儲存的變更。\n\n是：儲存後繼續\n否：放棄變更\n取消：留在目前地圖", "尚未儲存", MessageBoxButtons.YesNoCancel, MessageBoxIcon.Warning);
        return result switch { DialogResult.Yes => SaveMap(showSuccess: false), DialogResult.No => true, _ => false };
    }

    private void ReturnToMenu()
    {
        if (!ConfirmDiscardOrSave()) return;
        ReturnToMapMenu = true; _allowClose = true; Close();
    }

    private void LoadPalette(string? filter = null)
    {
        string? selected = (_palette.SelectedItem as PaletteItem)?.Key ?? _activeMaterial?.Id ?? _canvas.BrushTexture; _palette.Items.Clear();
        if (_texturesDocument is null) return;
        IEnumerable<FloorMaterial> materials = _floorMaterials?.Materials ?? Array.Empty<FloorMaterial>();
        if (!string.IsNullOrWhiteSpace(filter)) materials = materials.Where(material =>
            material.DisplayName.Contains(filter, StringComparison.CurrentCultureIgnoreCase) ||
            material.Category.Contains(filter, StringComparison.CurrentCultureIgnoreCase));
        PaletteItem[] items = materials.Select(material => new PaletteItem(material.Id, material.RepresentativeTexture, material.DisplayName, material)).ToArray();
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
    private string FriendlyTextureName(string? texture) => _floorMaterials?.FindByTexture(texture)?.DisplayName ?? "地表交界";
    private void ShowTerrainHover(TileHoverEventArgs e)
    {
        string hover = $"格子 ({e.X}, {e.Y})　{FriendlyTextureName(e.Texture)}";
        _status.Text = _terrainBlendNotice is null ? hover : _terrainBlendNotice + "　" + hover;
    }
    private void SelectBrush(PaletteItem item)
    {
        _activeMaterial = item.Material;
        _canvas.BrushTexture = item.PreviewTexture; if (_view3d is not null) _view3d.BrushTexture = item.PreviewTexture;
        _currentMaterialSwatch.Image = _floorTextures?.Get(item.PreviewTexture);
        _currentMaterialSwatch.BackColor = _canvas.GetTexturePreviewColor(item.PreviewTexture);
        _currentMaterialLabel.Text = "目前筆刷：" + item.Name;
        UpdateStatus();
    }
    private void SelectSampledTexture(string texture)
    {
        FloorMaterial? material = _floorMaterials?.FindByTexture(texture);
        if (material is null) { _status.Text = "這裡是地表交界；請直接從右側選擇要繪製的地表。"; return; }
        SelectBrush(new PaletteItem(material.Id, material.RepresentativeTexture, material.DisplayName, material));
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
            _terrainBlendNotice = $"已保留原圖交界：{import.UnresolvedTileIndices.Count} 個未知 tile、{import.CornerConflicts.Count} 個角點衝突；只重烘筆刷實際碰到的區域。";
    }
    private void UpdateEditorState()
    {
        bool editable = _selected?.IsCustom == true; _saveButton.Enabled = editable && IsDirty; _gamePreviewButton.Enabled = _selected is not null; _undoButton.Enabled = editable && _terrainBlendSession?.CanUndo == true; _redoButton.Enabled = editable && _terrainBlendSession?.CanRedo == true; _resetTerrainButton.Enabled = editable && _texturesDocument is not null && TextureDirty();
        _sceneApplyButton.Enabled = editable && _sceneList.SelectedItems.Count == 1; _sceneRestoreButton.Enabled = editable && _sceneLoaded && SdlSceneEditService.HasChanges(_sceneOriginalObjects, _sceneObjects);
        foreach (Control control in EditablePropertyControls()) control.Enabled = editable;
        _palette.Enabled = editable; UpdateStatus();
    }
    private void UpdateStatus() { _status.Text = _selected is null ? "尚未選擇地圖" : $"{_selected.Id} — {(_selected.IsCustom ? "自製地圖，可編輯" : "原廠地圖，唯讀")}{(IsDirty ? "  ● 尚未儲存" : "")}{(_terrainBlendNotice is null ? "" : "　" + _terrainBlendNotice)}"; }
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
        _last3DDiagnostic = Build3DDiagnostic(reason, exception);
        _view3dButton.Enabled = false;
        _view3dButton.ToolTipText = reason;
        _3dDiagnosticsButton.Visible = true;
        _3dDiagnosticsButton.ToolTipText = "查看 3D 場景無法啟用的實際原因";
        SetActiveView(use3D: false);
        _modeBanner.Text = "3D 無法啟用：" + reason + "　請按「3D 診斷」。";
        _modeBanner.BackColor = Color.FromArgb(86, 69, 40);
    }

    private string Build3DDiagnostic(string reason, Exception? exception)
    {
        string mapPath = _selected?.DirectoryPath ?? "（尚未選擇地圖）";
        string floorPath = Path.Combine(_gamePath, "floortex.dat");
        string heightPath = _selected is null ? "（尚未選擇地圖）" : Path.Combine(mapPath, "boden.bmp");
        return string.Join(Environment.NewLine,
            "Against Rome Map Editor — 3D 診斷",
            $"原因：{reason}",
            $"地圖：{_selected?.Id ?? "（無）"}",
            $"floortex.dat：{(File.Exists(floorPath) ? "存在" : "缺少")} — {floorPath}",
            $"boden.bmp：{(File.Exists(heightPath) ? "存在" : "缺少")} — {heightPath}",
            $"OpenGL：{_view3d?.ContextDescription ?? "尚未建立 context"}",
            $"作業系統：{Environment.OSVersion}",
            $"程序架構：{System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture}",
            exception is null ? "例外：無" : "例外：" + exception);
    }

    private void Show3DDiagnostics()
    {
        string diagnostic = _last3DDiagnostic ?? "目前沒有 3D 失敗診斷。";
        using var dialog = new Form { Text = "3D 場景診斷", Width = 760, Height = 520, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor };
        var text = new TextBox { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Both, WordWrap = false, Text = diagnostic, Font = new Font(FontFamily.GenericMonospace, 9F) };
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(10), FlowDirection = FlowDirection.RightToLeft };
        var close = new Button { Text = "關閉", DialogResult = DialogResult.OK, Width = 90 };
        var copy = new Button { Text = "複製診斷", Width = 110 };
        copy.Click += (_, _) => { try { Clipboard.SetText(diagnostic); } catch { } };
        buttons.Controls.Add(close); buttons.Controls.Add(copy); dialog.Controls.Add(text); dialog.Controls.Add(buttons); dialog.AcceptButton = close;
        dialog.ShowDialog(this);
    }
    private void ResetTerrain()
    {
        if (_texturesDocument is null || _savedTextures.Length != _texturesDocument.Textures.Count) return;
        if (MessageBox.Show(this, "要放棄這次尚未儲存的地表繪製嗎？\n地圖名稱與環境設定不會受影響。", "還原地表", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
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
        foreach (MapSceneObject item in objects.OrderBy(x => x.Kind).ThenBy(x => x.Team))
        {
            if (item.Kind == "建築") buildingIndex++; else if (item.Kind == "單位") unitIndex++; else objectIndex++;
            var row = new ListViewItem(item.Kind) { Tag = item }; row.SubItems.Add(item.Name); row.SubItems.Add(item.Team >= 0 ? item.Team.ToString() : "-"); row.SubItems.Add($"{item.SourceFile} / {item.ObjectIndex:0000}"); _sceneList.Items.Add(row);
        }
        _sceneList.EndUpdate();
        _sceneSummary.Text = $"建築 {buildingIndex}　單位 {unitIndex}　其他 {objectIndex}\n點清單項目可跳到該物件位置。";
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
        if (e.KeyCode == Keys.S) { if (_selected?.IsCustom == true) SaveMap(showSuccess: false); else _status.Text = "原廠地圖為唯讀，無法儲存；請先複製為自製地圖。"; }
        else if (e.KeyCode == Keys.Z) Undo();
        else if (e.KeyCode == Keys.Y) Redo();
        else return;
        e.SuppressKeyPress = true;
    }
    private static Label SectionHeader(string text) => new() { Text = text, Dock = DockStyle.Top, Height = 34, Font = new Font("Microsoft JhengHei UI", 9F, FontStyle.Bold), Padding = new Padding(4, 8, 0, 0), ForeColor = Color.White };
    private static void AddField(TableLayoutPanel table, string label, Control control) { table.Controls.Add(new Label { Text = label, AutoSize = true, Margin = new Padding(3, 10, 3, 3), ForeColor = Color.Gainsboro }); table.Controls.Add(control); }
    private static void AddSceneField(TableLayoutPanel table, int row, string label, Control control) { table.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left, ForeColor = Color.Gainsboro }, 0, row); table.Controls.Add(control, 1, row); }
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
    private void ShowError(Exception ex) => MessageBox.Show(this, ex.Message, "地圖編輯器錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);

    protected override void Dispose(bool disposing)
    {
        if (disposing) { _floorTextures?.Dispose(); _floorTextures = null; Image? overview = _overview.Image; _overview.Image = null; overview?.Dispose(); }
        base.Dispose(disposing);
    }

    private sealed record PaletteItem(string Key, string PreviewTexture, string Name, FloorMaterial? Material);
}
