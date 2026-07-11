using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using System.Diagnostics;

namespace AgainstRomeMapEditor;

internal sealed class MapEditorForm : Form
{
    private readonly EndlessMapCatalog _catalog = new();
    private readonly EndlessMapCloner _cloner = new();
    private readonly MapCanvasControl _canvas = new();
    private readonly ListView _maps = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, MultiSelect = false, HideSelection = false };
    private readonly ListBox _palette = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly TextBox _paletteSearch = new() { Dock = DockStyle.Top, PlaceholderText = "搜尋材質名稱…" };
    private readonly Panel _advancedPaletteHost = new() { Dock = DockStyle.Fill, Visible = false };
    private readonly CheckBox _showAdvancedPalette = new() { Dock = DockStyle.Top, Height = 30, Text = "顯示進階材質庫（內部 ID）" };
    private readonly Panel _currentMaterialSwatch = new() { Width = 54, Height = 54, BackColor = Color.DimGray };
    private readonly Label _currentMaterialLabel = new() { AutoSize = true, Text = "目前筆刷：尚未取樣", ForeColor = Color.White, Font = new Font("Microsoft JhengHei UI", 10F, FontStyle.Bold) };
    private readonly TextBox _gamePath = new() { Width = 430 };
    private readonly TextBox _title = new() { Dock = DockStyle.Top };
    private readonly NumericUpDown _waterLevel = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 4096 };
    private readonly TextBox _waterColor = new() { Dock = DockStyle.Top };
    private readonly NumericUpDown _dayStart = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 24 };
    private readonly NumericUpDown _dayEnd = new() { Dock = DockStyle.Top, Minimum = 0, Maximum = 24 };
    private readonly CheckBox _rain = new() { Dock = DockStyle.Top, Text = "水面雨滴" };
    private readonly Label _modeBanner = new() { Dock = DockStyle.Top, Height = 34, TextAlign = ContentAlignment.MiddleCenter };
    private readonly ToolStripStatusLabel _status = new() { Spring = true, TextAlign = ContentAlignment.MiddleLeft };
    private readonly ToolStripButton _saveButton = new("儲存") { Enabled = false };
    private readonly ToolStripButton _gamePreviewButton = new("在遊戲中預覽") { Enabled = false };
    private readonly ToolStripButton _undoButton = new("復原") { Enabled = false };
    private readonly ToolStripButton _redoButton = new("重做") { Enabled = false };
    private readonly ToolStripButton _textureTool = new("材質筆刷") { CheckOnClick = true, Checked = true };
    private readonly ToolStripButton _minimapView = new("地圖底圖") { CheckOnClick = true };
    private readonly ToolStripButton _collisionView = new("碰撞預覽") { CheckOnClick = true };
    private readonly Stack<TextureChange> _undo = new();
    private readonly Stack<TextureChange> _redo = new();
    private EndlessMapInfo? _selected;
    private BodenTexturesDocument? _texturesDocument;
    private string[] _savedTextures = Array.Empty<string>();
    private bool _dirty;
    private bool _loading;
    private bool _selectionGuard;
    private int? _requestedSlot;
    private Dictionary<string, string> _textureLabels = new(StringComparer.OrdinalIgnoreCase);

    public MapEditorForm(EditorArguments arguments)
    {
        Text = "Against Rome 地圖編輯器";
        Width = 1440; Height = 900; MinimumSize = new Size(1100, 700); StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(30, 34, 42); ForeColor = Color.Gainsboro;
        _requestedSlot = arguments.SelectedSlot;
        BuildInterface(); WireEvents();
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
        commands.Items.AddRange(new ToolStripItem[] { browse, new ToolStripControlHost(_gamePath), refresh, new ToolStripSeparator(), clone, _saveButton, _gamePreviewButton, new ToolStripSeparator(), _undoButton, _redoButton });
        browse.Click += (_, _) => Browse(); refresh.Click += (_, _) => RefreshMaps(_selected?.Slot); clone.Click += (_, _) => CloneSelected();

        var tools = new ToolStrip { GripStyle = ToolStripGripStyle.Hidden, Dock = DockStyle.Top, Padding = new Padding(8, 4, 8, 4), BackColor = Color.FromArgb(36, 40, 49), ForeColor = Color.White };
        var height = new ToolStripButton("高度筆刷（待驗證）") { Enabled = false };
        var collision = new ToolStripButton("碰撞筆刷（待驗證）") { Enabled = false };
        var objects = new ToolStripButton("物件配置（待校正）") { Enabled = false };
        tools.Items.AddRange(new ToolStripItem[] { new ToolStripLabel("工具："), _textureTool, height, collision, objects, new ToolStripSeparator(), new ToolStripLabel("檢視："), _minimapView, _collisionView });

        _maps.Columns.Add("槽位", 82); _maps.Columns.Add("名稱", 155); _maps.Columns.Add("類型", 70);
        var mapHeader = SectionHeader("地圖");
        var left = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        left.Controls.Add(_maps); left.Controls.Add(mapHeader);

        var paletteHeader = SectionHeader("材質工具");
        var palettePanel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(34, 38, 47) };
        var currentBrush = new FlowLayoutPanel { Dock = DockStyle.Top, Height = 76, Padding = new Padding(4), FlowDirection = FlowDirection.LeftToRight };
        var currentText = new FlowLayoutPanel { Width = 205, Height = 66, FlowDirection = FlowDirection.TopDown, WrapContents = false };
        currentText.Controls.Add(_currentMaterialLabel); currentText.Controls.Add(new Label { AutoSize = true, MaximumSize = new Size(200, 0), Text = "在地圖上按右鍵吸取材質，\n再用左鍵拖曳繪製。", ForeColor = Color.Silver });
        currentBrush.Controls.Add(_currentMaterialSwatch); currentBrush.Controls.Add(currentText);
        _advancedPaletteHost.Controls.Add(_palette); _advancedPaletteHost.Controls.Add(_paletteSearch);
        palettePanel.Controls.Add(_advancedPaletteHost); palettePanel.Controls.Add(_showAdvancedPalette); palettePanel.Controls.Add(currentBrush); palettePanel.Controls.Add(paletteHeader);

        var properties = BuildPropertiesPanel();
        var inspectorTabs = new TabControl { Dock = DockStyle.Fill };
        inspectorTabs.TabPages.Add(new TabPage("材質") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[0].Controls.Add(palettePanel);
        inspectorTabs.TabPages.Add(new TabPage("地圖屬性") { BackColor = Color.FromArgb(34, 38, 47) }); inspectorTabs.TabPages[1].Controls.Add(properties);

        var canvasHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(20, 23, 29) };
        canvasHost.Controls.Add(_canvas); canvasHost.Controls.Add(_modeBanner);

        var centerRight = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1140, 760), SplitterDistance = 820 };
        centerRight.Panel1.Controls.Add(canvasHost); centerRight.Panel2.Controls.Add(inspectorTabs); centerRight.Panel2MinSize = 280;
        var main = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel1, Size = new Size(1400, 760), SplitterDistance = 280 };
        main.Panel1.Controls.Add(left); main.Panel2.Controls.Add(centerRight); main.Panel1MinSize = 240;

        var statusStrip = new StatusStrip { BackColor = Color.FromArgb(42, 47, 58), ForeColor = Color.Gainsboro };
        statusStrip.Items.Add(_status); statusStrip.Items.Add(new ToolStripStatusLabel("右鍵：吸取材質　左鍵拖曳：繪製"));
        Controls.Add(main); Controls.Add(tools); Controls.Add(commands); Controls.Add(statusStrip);
        commands.BringToFront(); tools.BringToFront();
    }

    private Panel BuildPropertiesPanel()
    {
        var table = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 1, Padding = new Padding(10) };
        AddField(table, "地圖名稱", _title); AddField(table, "Waterlevel", _waterLevel); AddField(table, "WaterColor (0xBBGGRR)", _waterColor);
        AddField(table, "DayStartTime", _dayStart); AddField(table, "DayEndTime", _dayEnd); AddField(table, "", _rain);
        var panel = new Panel { Dock = DockStyle.Fill, AutoScroll = true, BackColor = Color.FromArgb(34, 38, 47) }; panel.Controls.Add(table); return panel;
    }

    private void WireEvents()
    {
        _maps.SelectedIndexChanged += (_, _) => OnMapSelectionChanged();
        _palette.SelectedIndexChanged += (_, _) => { if (_palette.SelectedItem is PaletteItem item) SelectBrush(item.Id); };
        _palette.DrawMode = DrawMode.OwnerDrawFixed; _palette.ItemHeight = 30; _palette.DrawItem += DrawPaletteItem;
        _paletteSearch.TextChanged += (_, _) => LoadPalette(_paletteSearch.Text);
        _showAdvancedPalette.CheckedChanged += (_, _) => _advancedPaletteHost.Visible = _showAdvancedPalette.Checked;
        _canvas.TexturePainted += (_, e) => PaintTexture(e);
        _canvas.TextureSampled += (_, e) => SelectBrush(e.Texture);
        _canvas.TileHovered += (_, e) => _status.Text = $"格子 ({e.X}, {e.Y})　{FriendlyTextureName(e.Texture)}";
        _textureTool.Click += (_, _) => ShowLayer(MapCanvasLayer.Textures);
        _minimapView.Click += (_, _) => ShowLayer(MapCanvasLayer.Minimap);
        _collisionView.Click += (_, _) => ShowLayer(MapCanvasLayer.Collision);
        _saveButton.Click += (_, _) => SaveMap(showSuccess: true);
        _gamePreviewButton.Click += (_, _) => PreviewInGame();
        _undoButton.Click += (_, _) => Undo(); _redoButton.Click += (_, _) => Redo();
        foreach (Control control in new Control[] { _title, _waterLevel, _waterColor, _dayStart, _dayEnd, _rain })
        {
            if (control is TextBox text) text.TextChanged += (_, _) => MarkDirty();
            else if (control is NumericUpDown numeric) numeric.ValueChanged += (_, _) => MarkDirty();
            else if (control is CheckBox check) check.CheckedChanged += (_, _) => MarkDirty();
        }
    }

    private void RefreshMaps(int? selectSlot)
    {
        if (!ConfirmDiscardOrSave()) return;
        try
        {
            _selectionGuard = true; _maps.BeginUpdate(); _maps.Items.Clear();
            foreach (EndlessMapInfo map in _catalog.List(_gamePath.Text))
            {
                var item = new ListViewItem(map.Id) { Tag = map }; item.SubItems.Add(map.DisplayName ?? "(無標題)"); item.SubItems.Add(map.IsCustom ? "自製" : "原廠"); _maps.Items.Add(item);
                if (map.Slot == selectSlot) { item.Selected = true; item.Focused = true; }
            }
            if (_maps.SelectedItems.Count == 0 && _maps.Items.Count > 0) _maps.Items[0].Selected = true;
        }
        catch (Exception ex) { ShowError(ex); }
        finally { _maps.EndUpdate(); _selectionGuard = false; }
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
        _selected = _maps.SelectedItems.Count == 1 ? _maps.SelectedItems[0].Tag as EndlessMapInfo : null;
        if (_selected is null) { UpdateEditorState(); return; }
        try
        {
            _loading = true; string map = _selected.DirectoryPath;
            _title.Text = PutTextDocument.Load(Path.Combine(map, "TEXT", "US", "briefing.put")).GetValue("briefing_titel_1") ?? "";
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini"));
            _waterLevel.Value = ParseDecimal(ini.GetValue("Waterlevel"), _waterLevel); _waterColor.Text = ini.GetValue("WaterColor") ?? "";
            _dayStart.Value = ParseDecimal(ini.GetValue("DayStartTime"), _dayStart); _dayEnd.Value = ParseDecimal(ini.GetValue("DayEndTime"), _dayEnd); _rain.Checked = ini.GetValue("RainDropsOnWater") == "1";
            _texturesDocument = BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")); _savedTextures = _texturesDocument.Textures.ToArray(); BuildTextureLabels(); _dirty = false; _undo.Clear(); _redo.Clear();
            LoadPalette(); ShowLayer(MapCanvasLayer.Textures); UpdateEditorState();
        }
        catch (Exception ex) { ShowError(ex); }
        finally { _loading = false; }
    }

    private void ShowLayer(MapCanvasLayer layer)
    {
        if (_selected is null) return;
        _textureTool.Checked = layer == MapCanvasLayer.Textures; _minimapView.Checked = layer == MapCanvasLayer.Minimap; _collisionView.Checked = layer == MapCanvasLayer.Collision;
        if (layer == MapCanvasLayer.Textures && _texturesDocument is not null) _canvas.LoadTextures(_texturesDocument.Dimension, _texturesDocument.Textures, _savedTextures, Path.Combine(_selected.DirectoryPath, "minimap.bmp"));
        else _canvas.LoadBitmapLayer(_selected.DirectoryPath, layer);
        _canvas.EditingEnabled = layer == MapCanvasLayer.Textures && _selected.IsCustom;
        _modeBanner.Text = layer switch { MapCanvasLayer.Textures => _selected.IsCustom ? "材質編輯底圖 — 右鍵吸取，左鍵拖曳繪製；真實畫面請按「在遊戲中預覽」" : "原廠地圖唯讀 — 請先使用上方「複製為自製地圖」", MapCanvasLayer.Minimap => "地圖底圖（不是遊戲渲染畫面）", _ => "碰撞資料檢視（不是遊戲渲染畫面）" };
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
            var put = PutTextDocument.Load(Path.Combine(map, "TEXT", "US", "briefing.put")); put.SetValue("briefing_titel_1", _title.Text.Trim()); put.Save(rollback);
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini")); ini.SetValue("Waterlevel", _waterLevel.Value.ToString()); ini.SetValue("WaterColor", _waterColor.Text.Trim()); ini.SetValue("DayStartTime", _dayStart.Value.ToString()); ini.SetValue("DayEndTime", _dayEnd.Value.ToString()); ini.SetValue("RainDropsOnWater", _rain.Checked ? "1" : "0"); ini.Save(rollback);
            _texturesDocument?.Save(rollback); rollback.Commit(); _savedTextures = _texturesDocument?.Textures.ToArray() ?? Array.Empty<string>(); _dirty = false; ShowLayer(MapCanvasLayer.Textures); UpdateEditorState();
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
            MessageBox.Show(this, $"遊戲已啟動。\n\n請進入「無盡模式」並選擇 {_selected.Id}（{_selected.DisplayName ?? "未命名"}）。\n目前尚未發現可安全直接載入指定地圖的命令列參數。", "真實遊戲預覽", MessageBoxButtons.OK, MessageBoxIcon.Information);
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
        try { int slot = _catalog.GetNextFreeSlot(_gamePath.Text); _cloner.Clone(_gamePath.Text, _selected.Slot, slot, name.Trim()); RefreshMaps(slot); }
        catch (Exception ex) { ShowError(ex); }
    }

    private void LoadPalette(string? filter = null)
    {
        string? selected = (_palette.SelectedItem as PaletteItem)?.Id ?? _canvas.BrushTexture; _palette.Items.Clear();
        if (_texturesDocument is null) return;
        IEnumerable<string> values = _texturesDocument.Textures.Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(x => x, StringComparer.OrdinalIgnoreCase);
        if (!string.IsNullOrWhiteSpace(filter)) values = values.Where(x => x.Contains(filter, StringComparison.OrdinalIgnoreCase) || FriendlyTextureName(x).Contains(filter, StringComparison.OrdinalIgnoreCase));
        PaletteItem[] items = values.Select(x => new PaletteItem(x, FriendlyTextureName(x))).ToArray(); _palette.Items.AddRange(items.Cast<object>().ToArray());
        int index = selected is null ? -1 : Array.FindIndex(items, x => StringComparer.OrdinalIgnoreCase.Equals(x.Id, selected)); _palette.SelectedIndex = index;
    }

    private void DrawPaletteItem(object? sender, DrawItemEventArgs e)
    {
        e.DrawBackground(); if (e.Index < 0 || _palette.Items[e.Index] is not PaletteItem item) return;
        Font baseFont = e.Font ?? Font;
        using var color = new SolidBrush(_canvas.GetTexturePreviewColor(item.Id)); e.Graphics.FillRectangle(color, e.Bounds.X + 5, e.Bounds.Y + 5, 20, e.Bounds.Height - 10);
        TextRenderer.DrawText(e.Graphics, item.Name, baseFont, new Rectangle(e.Bounds.X + 32, e.Bounds.Y, 105, e.Bounds.Height), e.ForeColor, TextFormatFlags.VerticalCenter | TextFormatFlags.Left);
        using var idFont = new Font(baseFont.FontFamily, Math.Max(7, baseFont.Size - 1));
        TextRenderer.DrawText(e.Graphics, item.Id, idFont, new Rectangle(e.Bounds.X + 138, e.Bounds.Y, e.Bounds.Width - 140, e.Bounds.Height), Color.Gray, TextFormatFlags.VerticalCenter | TextFormatFlags.Left);
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
        bool editable = _selected?.IsCustom == true; _saveButton.Enabled = editable && _dirty; _gamePreviewButton.Enabled = _selected is not null; _undoButton.Enabled = editable && _undo.Count > 0; _redoButton.Enabled = editable && _redo.Count > 0;
        foreach (Control control in new Control[] { _title, _waterLevel, _waterColor, _dayStart, _dayEnd, _rain }) control.Enabled = editable;
        _palette.Enabled = editable; UpdateStatus();
    }
    private void UpdateStatus() { _status.Text = _selected is null ? "尚未選擇地圖" : $"{_selected.Id} — {(_selected.IsCustom ? "自製地圖，可編輯" : "原廠地圖，唯讀")}{(_dirty ? "  ● 尚未儲存" : "")}"; }
    private void ReselectCurrent() { if (_selected is null) return; _selectionGuard = true; foreach (ListViewItem item in _maps.Items) item.Selected = item.Tag is EndlessMapInfo map && map.Slot == _selected.Slot; _selectionGuard = false; }
    private void Browse() { if (!ConfirmDiscardOrSave()) return; using var dialog = new FolderBrowserDialog { Description = "選擇 Against Rome 安裝資料夾" }; if (dialog.ShowDialog(this) == DialogResult.OK) { _gamePath.Text = dialog.SelectedPath; RefreshMaps(null); } }
    private static Label SectionHeader(string text) => new() { Text = text, Dock = DockStyle.Top, Height = 34, Font = new Font("Microsoft JhengHei UI", 9F, FontStyle.Bold), Padding = new Padding(4, 8, 0, 0), ForeColor = Color.White };
    private static void AddField(TableLayoutPanel table, string label, Control control) { table.Controls.Add(new Label { Text = label, AutoSize = true, Margin = new Padding(3, 10, 3, 3), ForeColor = Color.Gainsboro }); table.Controls.Add(control); }
    private static decimal ParseDecimal(string? value, NumericUpDown control) => decimal.TryParse(value, out decimal parsed) ? Math.Clamp(parsed, control.Minimum, control.Maximum) : control.Minimum;
    private static string Prompt(string title, string value) { using var form = new Form { Text = title, Width = 430, Height = 150, StartPosition = FormStartPosition.CenterParent }; var input = new TextBox { Text = value, Dock = DockStyle.Top, Margin = new Padding(12) }; var ok = new Button { Text = "確定", DialogResult = DialogResult.OK, Dock = DockStyle.Bottom, Height = 36 }; form.Controls.Add(input); form.Controls.Add(ok); form.AcceptButton = ok; return form.ShowDialog() == DialogResult.OK ? input.Text : ""; }
    private static string DetectGamePath() => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Against Rome");
    private void ShowError(Exception ex) => MessageBox.Show(this, ex.Message, "地圖編輯器錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
    private sealed record TextureChange(int X, int Y, string Before, string After);
    private sealed record PaletteItem(string Id, string Name);
}
