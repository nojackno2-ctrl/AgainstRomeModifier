using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using System.Diagnostics;

namespace AgainstRomeMapEditor;

internal sealed class MapEditorForm : Form
{
    private readonly EndlessMapCatalog _catalog = new();
    private readonly GameMapCatalog _gameMapCatalog = new();
    private readonly EndlessMapCloner _cloner = new();
    private readonly EndlessMapDeleter _deleter = new();
    private readonly MapCanvasControl _canvas = new();
    private readonly PictureBox _overview = new() { Dock = DockStyle.Fill, SizeMode = PictureBoxSizeMode.Zoom, BackColor = Color.FromArgb(18, 21, 27) };
    private readonly ListView _maps = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, MultiSelect = false, HideSelection = false };
    private readonly ListBox _palette = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly TextBox _paletteSearch = new() { Dock = DockStyle.Top, PlaceholderText = "搜尋地表樣式…" };
    private readonly Panel _currentMaterialSwatch = new() { Width = 54, Height = 54, BackColor = Color.DimGray };
    private readonly Label _currentMaterialLabel = new() { AutoSize = true, Text = "目前筆刷：尚未取樣", ForeColor = Color.White, Font = new Font("Microsoft JhengHei UI", 10F, FontStyle.Bold) };
    private readonly TextBox _gamePath = new() { Width = 430 };
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
    private readonly ListView _sceneList = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, HeaderStyle = ColumnHeaderStyle.Nonclickable };
    private readonly Label _sceneSummary = new() { Dock = DockStyle.Top, Height = 54, Padding = new Padding(8), ForeColor = Color.Gainsboro };
    private readonly Label _modeBanner = new() { Dock = DockStyle.Top, Height = 34, TextAlign = ContentAlignment.MiddleCenter };
    private readonly ToolStripStatusLabel _status = new() { Spring = true, TextAlign = ContentAlignment.MiddleLeft };
    private readonly ToolStripButton _saveButton = new("儲存") { Enabled = false };
    private readonly ToolStripButton _deleteButton = new("刪除地圖") { Enabled = false };
    private readonly ToolStripButton _gamePreviewButton = new("選用：啟動遊戲測試") { Enabled = false };
    private readonly ToolStripButton _undoButton = new("復原") { Enabled = false };
    private readonly ToolStripButton _redoButton = new("重做") { Enabled = false };
    private readonly ToolStripButton _textureTool = new("材質筆刷") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _resetTerrainButton = new("還原地表") { Enabled = false };
    private readonly Stack<TextureChange> _undo = new();
    private readonly Stack<TextureChange> _redo = new();
    private GameMapInfo? _selected;
    private BodenTexturesDocument? _texturesDocument;
    private string[] _savedTextures = Array.Empty<string>();
    private bool _dirty;
    private bool _loading;
    private bool _selectionGuard;
    private int? _requestedSlot;
    private Dictionary<string, string> _textureLabels = new(StringComparer.OrdinalIgnoreCase);
    private float _heightMapStep = 4;

    public MapEditorForm(EditorArguments arguments)
    {
        Text = "Against Rome 地圖編輯器";
        Width = 1440; Height = 900; MinimumSize = new Size(1100, 700); StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(30, 34, 42); ForeColor = Color.Gainsboro;
        _requestedSlot = arguments.SelectedSlot;
        BuildInterface(); WireEvents();
        KeyPreview = true;
        _gamePath.Text = arguments.GamePath ?? DetectGamePath();
        Shown += (_, _) => RefreshMaps(_requestedSlot);
        FormClosing += (_, e) => { if (!ConfirmDiscardOrSave()) e.Cancel = true; };
    }

    private void BuildInterface()
    {
        var commands = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(8, 5, 8, 5), BackColor = Color.FromArgb(42, 47, 58), ForeColor = Color.White, RenderMode = ToolStripRenderMode.System };
        var browse = new ToolStripButton("遊戲路徑…");
        var refresh = new ToolStripButton("重新整理");
        var clone = new ToolStripButton("複製為自製地圖");
        commands.Items.AddRange(new ToolStripItem[] { browse, new ToolStripControlHost(_gamePath), refresh, new ToolStripSeparator(), clone, _deleteButton, _saveButton, _gamePreviewButton, new ToolStripSeparator(), _undoButton, _redoButton });
        browse.Click += (_, _) => Browse(); refresh.Click += (_, _) => RefreshMaps(_selected?.EndlessSlot); clone.Click += (_, _) => CloneSelected();
        _deleteButton.Click += (_, _) => DeleteSelected();

        var tools = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(8, 4, 8, 4), BackColor = Color.FromArgb(36, 40, 49), ForeColor = Color.White };
        tools.Items.AddRange(new ToolStripItem[] { new ToolStripLabel("地表："), _textureTool, _resetTerrainButton });

        _maps.Columns.Add("地圖", 105); _maps.Columns.Add("名稱", 145); _maps.Columns.Add("類型", 90);
        var mapHeader = SectionHeader("地圖");
        var left = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        left.Controls.Add(_maps); left.Controls.Add(mapHeader);

        var paletteHeader = SectionHeader("地表繪製");
        var palettePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        var currentBrush = new FlowLayoutPanel { Dock = DockStyle.Top, Height = 76, Padding = new Padding(4), FlowDirection = FlowDirection.LeftToRight };
        var currentText = new FlowLayoutPanel { Width = 205, Height = 66, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        currentText.Controls.Add(_currentMaterialLabel); currentText.Controls.Add(new Label { AutoSize = true, MaximumSize = new Size(200, 0), Text = "右鍵取樣，左鍵拖曳繪製。", ForeColor = Color.Silver });
        currentBrush.Controls.Add(_currentMaterialSwatch); currentBrush.Controls.Add(currentText);
        _brushSize.Items.AddRange(new object[] { "精細（1 格）", "中型（3 × 3）", "大型（5 × 5）" }); _brushSize.SelectedIndex = 0;
        var brushOptions = new Panel { Dock = DockStyle.Top, Height = 118 };
        brushOptions.Controls.Add(_showObjects); brushOptions.Controls.Add(_showGrid); brushOptions.Controls.Add(new Label { Dock = DockStyle.Top, Height = 22, Text = "筆刷大小", ForeColor = Color.Gainsboro }); brushOptions.Controls.Add(_brushSize);
        palettePanel.Controls.Add(_palette); palettePanel.Controls.Add(_paletteSearch); palettePanel.Controls.Add(brushOptions); palettePanel.Controls.Add(currentBrush); palettePanel.Controls.Add(paletteHeader);

        var properties = BuildPropertiesPanel();
        var inspectorTabs = new TabControl { Dock = DockStyle.Fill };
        inspectorTabs.TabPages.Add(new TabPage("地表") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[0].Controls.Add(palettePanel);
        inspectorTabs.TabPages.Add(new TabPage("地圖屬性") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[1].Controls.Add(properties);
        var scenePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        _sceneList.Columns.Add("類型", 90); _sceneList.Columns.Add("編號", 90); _sceneList.Columns.Add("隊伍", 70);
        scenePanel.Controls.Add(_sceneList); scenePanel.Controls.Add(_sceneSummary);
        inspectorTabs.TabPages.Add(new TabPage("場景物件") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[2].Controls.Add(scenePanel);

        var canvasHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(20, 23, 29) };
        canvasHost.Controls.Add(_canvas); canvasHost.Controls.Add(_modeBanner);
        var overviewHost = new Panel { Width = 190, Height = 190, Anchor = AnchorStyles.Right | AnchorStyles.Bottom, Padding = new Padding(5), BackColor = Color.FromArgb(55, 61, 72) };
        overviewHost.Controls.Add(_overview); overviewHost.Controls.Add(new Label { Text = "地圖概覽", Dock = DockStyle.Top, Height = 25, TextAlign = ContentAlignment.MiddleCenter, ForeColor = Color.White, BackColor = Color.FromArgb(42, 47, 58) });
        canvasHost.Controls.Add(overviewHost); overviewHost.BringToFront();
        canvasHost.Resize += (_, _) => overviewHost.Location = new Point(Math.Max(12, canvasHost.ClientSize.Width - overviewHost.Width - 18), Math.Max(46, canvasHost.ClientSize.Height - overviewHost.Height - 18));

        var centerRight = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1140, 760), SplitterDistance = 820 };
        centerRight.Panel1.Controls.Add(canvasHost); centerRight.Panel2.Controls.Add(inspectorTabs); centerRight.Panel2MinSize = 280;
        var main = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel1, Size = new Size(1400, 760), SplitterDistance = 280 };
        main.Panel1.Controls.Add(left); main.Panel2.Controls.Add(centerRight); main.Panel1MinSize = 240;

        var statusStrip = new StatusStrip { BackColor = Color.FromArgb(42, 47, 58), ForeColor = Color.Gainsboro };
        statusStrip.Items.Add(_status); statusStrip.Items.Add(new ToolStripStatusLabel("滾輪縮放　中鍵平移　右鍵取樣　左鍵繪製　Ctrl+S 儲存"));
        Controls.Add(main); Controls.Add(tools); Controls.Add(commands); Controls.Add(statusStrip);
        commands.BringToFront(); tools.BringToFront();
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
        _maps.SelectedIndexChanged += (_, _) => OnMapSelectionChanged();
        _palette.SelectedIndexChanged += (_, _) => { if (_palette.SelectedItem is PaletteItem item) SelectBrush(item.Id); };
        _palette.DrawMode = DrawMode.OwnerDrawFixed; _palette.ItemHeight = 30; _palette.DrawItem += DrawPaletteItem;
        _paletteSearch.TextChanged += (_, _) => LoadPalette(_paletteSearch.Text);
        _brushSize.SelectedIndexChanged += (_, _) => _canvas.BrushSize = _brushSize.SelectedIndex switch { 1 => 3, 2 => 5, _ => 1 };
        _showGrid.CheckedChanged += (_, _) => { _canvas.ShowGrid = _showGrid.Checked; _canvas.Invalidate(); };
        _showObjects.CheckedChanged += (_, _) => { _canvas.ShowObjects = _showObjects.Checked; _canvas.Invalidate(); };
        _waterColorButton.Click += (_, _) => ChooseWaterColor();
        _canvas.TexturePainted += (_, e) => PaintTexture(e);
        _canvas.TextureSampled += (_, e) => SelectBrush(e.Texture);
        _canvas.TileHovered += (_, e) => _status.Text = $"格子 ({e.X}, {e.Y})　{FriendlyTextureName(e.Texture)}";
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

    private void RefreshMaps(int? selectSlot)
    {
        if (!ConfirmDiscardOrSave()) return;
        string? preferredMapId = null;
        try
        {
            _selectionGuard = true; _maps.BeginUpdate(); _maps.Items.Clear();
            _maps.Groups.Clear();
            string[] groupOrder = { "自製地圖", "劇情戰役", "歷史戰役", "教學", "無盡模式", "多人地圖", "其他地圖" };
            IReadOnlyList<GameMapInfo> availableMaps = _gameMapCatalog.List(_gamePath.Text);
            preferredMapId = availableMaps.FirstOrDefault(map => map.EndlessSlot == selectSlot)?.Id ?? availableMaps.FirstOrDefault()?.Id;
            var groups = groupOrder.ToDictionary(category => category, category => new ListViewGroup(category, HorizontalAlignment.Left));
            foreach (string category in groupOrder)
            {
                int count = availableMaps.Count(map => (map.IsCustom ? "自製地圖" : map.Category) == category);
                if (count == 0) continue;
                groups[category].Header = $"{category}（{count}）"; _maps.Groups.Add(groups[category]);
            }
            foreach (GameMapInfo map in availableMaps)
            {
                string category = map.IsCustom ? "自製地圖" : map.Category;
                var item = new ListViewItem(map.Id) { Tag = map, Group = groups[category] }; item.SubItems.Add(map.DisplayName ?? "(無標題)"); item.SubItems.Add(category); _maps.Items.Add(item);
            }
        }
        catch (Exception ex) { ShowError(ex); }
        finally { _maps.EndUpdate(); _selectionGuard = false; }
        if (preferredMapId is not null)
        {
            ListViewItem? preferred = _maps.Items.Cast<ListViewItem>().FirstOrDefault(item => item.Tag is GameMapInfo map && StringComparer.OrdinalIgnoreCase.Equals(map.Id, preferredMapId));
            if (preferred is not null) { preferred.Selected = true; preferred.Focused = true; preferred.EnsureVisible(); }
        }
        LoadSelectedFromList();
    }

    private void OnMapSelectionChanged()
    {
        if (_selectionGuard || _maps.SelectedItems.Count != 1) return;
        if (!ConfirmDiscardOrSave()) { ReselectCurrent(); return; }
        LoadSelectedFromList();
    }

    private void LoadSelectedFromList()
    {
        _selected = _maps.SelectedItems.Count == 1 ? _maps.SelectedItems[0].Tag as GameMapInfo : null;
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
            _texturesDocument = BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")); _savedTextures = _texturesDocument.Textures.ToArray(); BuildTextureLabels(); _dirty = false; _undo.Clear(); _redo.Clear();
            LoadPalette(); LoadEditingScene(); UpdateEditorState();
        }
        catch (Exception ex) { ShowError(ex); }
        finally { _loading = false; }
    }

    private void LoadEditingScene()
    {
        if (_selected is null) return;
        _textureTool.Checked = true;
        string minimapPath = Path.Combine(_selected.DirectoryPath, "minimap.bmp");
        IReadOnlyList<MapSceneObject> sceneObjects = SdlSceneCatalog.LoadDirectory(_selected.DirectoryPath);
        LoadSceneList(sceneObjects);
        if (!TryParseGameColor(_waterColor.Text, out Color sceneWaterColor)) sceneWaterColor = Color.SteelBlue;
        bool hasRealTextures = _texturesDocument is not null && _canvas.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _savedTextures, _selected.DirectoryPath, Path.Combine(_gamePath.Text.Trim(), "floortex.dat"), sceneObjects, (float)_waterLevel.Value, _heightMapStep, sceneWaterColor);
        Image? oldOverview = _overview.Image; _overview.Image = null; oldOverview?.Dispose();
        if (File.Exists(minimapPath)) using (var source = new Bitmap(minimapPath)) _overview.Image = new Bitmap(source);
        _canvas.EditingEnabled = _selected.IsCustom;
        _modeBanner.Text = hasRealTextures
            ? (_selected.IsCustom ? $"離線地圖場景 — 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件；滾輪縮放，中鍵平移" : $"離線地圖場景 — 真實地表、地勢與 {_canvas.SceneObjectCount} 個場景物件；原廠地圖僅供瀏覽")
            : "找不到 floortex.dat，目前只能顯示簡化地表；請選擇完整的遊戲資料夾";
        _modeBanner.BackColor = _canvas.EditingEnabled ? Color.FromArgb(38, 95, 72) : Color.FromArgb(86, 69, 40);
        UpdateStatus();
    }

    private void PaintTexture(TexturePaintEventArgs e)
    {
        if (_selected is null || !_selected.IsCustom || _texturesDocument is null) return;
        _texturesDocument.SetTexture(e.X, e.Y, e.Texture); _undo.Push(new TextureChange(e.X, e.Y, e.PreviousTexture, e.Texture)); _redo.Clear(); _dirty = true; UpdateEditorState();
    }

    private void Undo()
    {
        if (_texturesDocument is null || _undo.Count == 0) return;
        TextureChange change = _undo.Pop(); _texturesDocument.SetTexture(change.X, change.Y, change.Before); _canvas.SetTexture(change.X, change.Y, change.Before); _redo.Push(change); _dirty = true; UpdateEditorState();
    }

    private void Redo()
    {
        if (_texturesDocument is null || _redo.Count == 0) return;
        TextureChange change = _redo.Pop(); _texturesDocument.SetTexture(change.X, change.Y, change.After); _canvas.SetTexture(change.X, change.Y, change.After); _undo.Push(change); _dirty = true; UpdateEditorState();
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
            _texturesDocument?.Save(rollback); rollback.Commit(); _savedTextures = _texturesDocument?.Textures.ToArray() ?? Array.Empty<string>(); _dirty = false; LoadEditingScene(); UpdateEditorState();
            if (showSuccess) MessageBox.Show(this, "地圖已安全儲存。", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            return true;
        }
        catch (Exception ex) { ShowError(ex); return false; }
    }

    private void PreviewInGame()
    {
        if (_selected is null) { MessageBox.Show(this, "請先選擇要預覽的地圖。", Text); return; }
        if (_selected.IsCustom && _dirty && !SaveMap(showSuccess: false)) return;
        string exePath = Path.Combine(_gamePath.Text.Trim(), "Against_Rome.exe");
        if (!File.Exists(exePath)) { MessageBox.Show(this, "遊戲路徑中找不到 Against_Rome.exe。", Text, MessageBoxButtons.OK, MessageBoxIcon.Error); return; }
        try
        {
            Process.Start(new ProcessStartInfo(exePath) { WorkingDirectory = _gamePath.Text.Trim(), UseShellExecute = true });
            MessageBox.Show(this, $"遊戲已啟動。\n\n若要額外測試自製地圖，請進入「無盡模式」並選擇 {_selected.Id}（{_selected.DisplayName ?? "未命名"}）。\n地圖編輯與離線場景顯示不需要啟動遊戲。", "選用遊戲測試", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
        catch (Exception ex) { ShowError(ex); }
    }

    private bool ConfirmDiscardOrSave()
    {
        if (!_dirty) return true;
        DialogResult result = MessageBox.Show(this, "目前地圖有尚未儲存的變更。\n\n是：儲存後繼續\n否：放棄變更\n取消：留在目前地圖", "尚未儲存", MessageBoxButtons.YesNoCancel, MessageBoxIcon.Warning);
        return result switch { DialogResult.Yes => SaveMap(showSuccess: false), DialogResult.No => true, _ => false };
    }

    private void CloneSelected()
    {
        if (_selected is null) { MessageBox.Show(this, "請先從左側選擇來源地圖。", Text); return; }
        if (!ConfirmDiscardOrSave()) return;
        string name = Prompt("新地圖名稱", _selected.DisplayName ?? _selected.Id); if (string.IsNullOrWhiteSpace(name)) return;
        try { int slot = _catalog.GetNextFreeSlot(_gamePath.Text); _cloner.Clone(_gamePath.Text, _selected.Id, slot, name.Trim()); RefreshMaps(slot); }
        catch (Exception ex) { ShowError(ex); }
    }

    private void DeleteSelected()
    {
        if (_selected is null || !_selected.IsCustom) return;
        if (!ConfirmDiscardOrSave()) return;
        string name = _selected.DisplayName ?? "未命名地圖";
        DialogResult result = MessageBox.Show(this,
            $"確定要永久刪除這張自製地圖嗎？\n\n{name}\n{_selected.Id}\n\n刪除後無法復原。",
            "刪除地圖", MessageBoxButtons.YesNo, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2);
        if (result != DialogResult.Yes) return;

        try
        {
            _deleter.Delete(_gamePath.Text, _selected.EndlessSlot ?? throw new InvalidOperationException("自製地圖沒有有效槽位。"));
            _selected = null; _texturesDocument = null; _savedTextures = Array.Empty<string>(); _dirty = false; _undo.Clear(); _redo.Clear();
            RefreshMaps(null);
        }
        catch (Exception ex) { ShowError(ex); }
    }

    private void LoadPalette(string? filter = null)
    {
        string? selected = (_palette.SelectedItem as PaletteItem)?.Id ?? _canvas.BrushTexture; _palette.Items.Clear();
        if (_texturesDocument is null) return;
        IEnumerable<string> values = _texturesDocument.Textures.Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(FriendlyTextureName, StringComparer.CurrentCultureIgnoreCase);
        if (!string.IsNullOrWhiteSpace(filter)) values = values.Where(x => FriendlyTextureName(x).Contains(filter, StringComparison.CurrentCultureIgnoreCase));
        PaletteItem[] items = values.Select(x => new PaletteItem(x, FriendlyTextureName(x))).ToArray(); _palette.Items.AddRange(items.Cast<object>().ToArray());
        int index = selected is null ? -1 : Array.FindIndex(items, x => StringComparer.OrdinalIgnoreCase.Equals(x.Id, selected)); _palette.SelectedIndex = index;
    }

    private void DrawPaletteItem(object? sender, DrawItemEventArgs e)
    {
        e.DrawBackground(); if (e.Index < 0 || _palette.Items[e.Index] is not PaletteItem item) return;
        Font baseFont = e.Font ?? Font;
        using var color = new SolidBrush(_canvas.GetTexturePreviewColor(item.Id)); e.Graphics.FillRectangle(color, e.Bounds.X + 5, e.Bounds.Y + 5, 20, e.Bounds.Height - 10);
        TextRenderer.DrawText(e.Graphics, item.Name, baseFont, new Rectangle(e.Bounds.X + 32, e.Bounds.Y, e.Bounds.Width - 36, e.Bounds.Height), e.ForeColor, TextFormatFlags.VerticalCenter | TextFormatFlags.Left);
        e.DrawFocusRectangle();
    }

    private void MarkDirty() { if (_loading || _selected is null || !_selected.IsCustom) return; _dirty = true; UpdateEditorState(); }
    private void BuildTextureLabels()
    {
        _textureLabels = _texturesDocument?.Textures.Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(x => x, StringComparer.OrdinalIgnoreCase).Select((id, index) => (id, name: $"地表樣式 {index + 1:000}")).ToDictionary(x => x.id, x => x.name, StringComparer.OrdinalIgnoreCase) ?? new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
    }
    private string FriendlyTextureName(string? texture) => texture != null && _textureLabels.TryGetValue(texture, out string? name) ? name : "未知地表樣式";
    private void SelectBrush(string texture)
    {
        _canvas.BrushTexture = texture; _currentMaterialSwatch.BackColor = _canvas.GetTexturePreviewColor(texture); _currentMaterialLabel.Text = "目前筆刷：" + FriendlyTextureName(texture); UpdateStatus();
    }
    private void UpdateEditorState()
    {
        bool editable = _selected?.IsCustom == true; _deleteButton.Enabled = editable; _saveButton.Enabled = editable && _dirty; _gamePreviewButton.Enabled = _selected is not null; _undoButton.Enabled = editable && _undo.Count > 0; _redoButton.Enabled = editable && _redo.Count > 0; _resetTerrainButton.Enabled = editable && _texturesDocument is not null;
        foreach (Control control in EditablePropertyControls()) control.Enabled = editable;
        _palette.Enabled = editable; UpdateStatus();
    }
    private void UpdateStatus() { _status.Text = _selected is null ? "尚未選擇地圖" : $"{_selected.Id} — {(_selected.IsCustom ? "自製地圖，可編輯" : "原廠地圖，唯讀")}{(_dirty ? "  ● 尚未儲存" : "")}"; }
    private void ResetTerrain()
    {
        if (_texturesDocument is null || _savedTextures.Length != _texturesDocument.Textures.Count) return;
        if (MessageBox.Show(this, "要放棄這次尚未儲存的地表繪製嗎？\n地圖名稱與環境設定不會受影響。", "還原地表", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        for (int index = 0; index < _savedTextures.Length; index++)
        {
            int x = index % _texturesDocument.Dimension, y = index / _texturesDocument.Dimension;
            _texturesDocument.SetTexture(x, y, _savedTextures[index]); _canvas.SetTexture(x, y, _savedTextures[index]);
        }
        _undo.Clear(); _redo.Clear(); MarkDirty();
    }

    private void LoadSceneList(IReadOnlyList<MapSceneObject> objects)
    {
        _sceneList.BeginUpdate(); _sceneList.Items.Clear();
        int buildingIndex = 0, unitIndex = 0, objectIndex = 0;
        foreach (MapSceneObject item in objects.OrderBy(x => x.Kind).ThenBy(x => x.Team))
        {
            int index = item.Kind switch { "建築" => ++buildingIndex, "單位" => ++unitIndex, _ => ++objectIndex };
            var row = new ListViewItem(item.Kind); row.SubItems.Add($"{index:000}"); row.SubItems.Add(item.Team > 0 ? $"隊伍 {item.Team}" : "中立"); _sceneList.Items.Add(row);
        }
        _sceneList.EndUpdate();
        _sceneSummary.Text = $"建築 {buildingIndex}　單位 {unitIndex}　其他 {objectIndex}\n此處顯示地圖中的場景配置。";
    }
    private void ChooseWaterColor()
    {
        using var dialog = new ColorDialog { FullOpen = true };
        if (TryParseGameColor(_waterColor.Text, out Color color)) dialog.Color = color;
        if (dialog.ShowDialog(this) != DialogResult.OK) return;
        _waterColor.Text = $"0x{dialog.Color.B:X2}{dialog.Color.G:X2}{dialog.Color.R:X2}".ToLowerInvariant();
        _waterColorButton.BackColor = dialog.Color; _waterColorButton.ForeColor = dialog.Color.GetBrightness() < .45f ? Color.White : Color.Black;
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
        if (e.KeyCode == Keys.S) SaveMap(showSuccess: false); else if (e.KeyCode == Keys.Z) Undo(); else if (e.KeyCode == Keys.Y) Redo(); else return;
        e.SuppressKeyPress = true;
    }
    private void ReselectCurrent() { if (_selected is null) return; _selectionGuard = true; foreach (ListViewItem item in _maps.Items) item.Selected = item.Tag is GameMapInfo map && StringComparer.OrdinalIgnoreCase.Equals(map.Id, _selected.Id); _selectionGuard = false; }
    private void Browse() { if (!ConfirmDiscardOrSave()) return; using var dialog = new FolderBrowserDialog { Description = "選擇 Against Rome 安裝資料夾" }; if (dialog.ShowDialog(this) == DialogResult.OK) { _gamePath.Text = dialog.SelectedPath; RefreshMaps(null); } }
    private static Label SectionHeader(string text) => new() { Text = text, Dock = DockStyle.Top, Height = 34, Font = new Font("Microsoft JhengHei UI", 9F, FontStyle.Bold), Padding = new Padding(4, 8, 0, 0), ForeColor = Color.White };
    private static void AddField(TableLayoutPanel table, string label, Control control) { table.Controls.Add(new Label { Text = label, AutoSize = true, Margin = new Padding(3, 10, 3, 3), ForeColor = Color.Gainsboro }); table.Controls.Add(control); }
    private IEnumerable<Control> EditablePropertyControls()
    {
        yield return _title; yield return _subtitle; yield return _briefing;
        foreach (TextBox teamName in _teamNames) yield return teamName;
        yield return _waterLevel; yield return _waterColor; yield return _waterWarpShift; yield return _waterBumpAmplitude; yield return _waterBumpFrequency; yield return _flashProbability;
        yield return _dayStart; yield return _dayEnd; yield return _rain;
    }
    private static decimal ParseDecimal(string? value, NumericUpDown control) => decimal.TryParse(value, out decimal parsed) ? Math.Clamp(parsed, control.Minimum, control.Maximum) : control.Minimum;
    private static string Prompt(string title, string value) { using var form = new Form { Text = title, Width = 430, Height = 150, StartPosition = FormStartPosition.CenterParent }; var input = new TextBox { Text = value, Dock = DockStyle.Top, Margin = new Padding(12) }; var ok = new Button { Text = "確定", DialogResult = DialogResult.OK, Dock = DockStyle.Bottom, Height = 36 }; form.Controls.Add(input); form.Controls.Add(ok); form.AcceptButton = ok; return form.ShowDialog() == DialogResult.OK ? input.Text : ""; }
    private static string DetectGamePath() => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Against Rome");
    private void ShowError(Exception ex) => MessageBox.Show(this, ex.Message, "地圖編輯器錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
    private sealed record TextureChange(int X, int Y, string Before, string After);
    private sealed record PaletteItem(string Id, string Name);
}
