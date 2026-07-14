using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed class MapSelectionForm : Form
{
    private readonly GameMapCatalog _catalog = new();
    private readonly EndlessMapCatalog _endlessCatalog = new();
    private readonly EndlessMapCloner _cloner = new();
    private readonly EndlessMapDeleter _deleter = new();
    private readonly TextBox _gamePath = new() { Dock = DockStyle.Fill };
    private readonly ListView _customMaps = CreateMapList();
    private readonly ListView _originalMaps = CreateMapList();
    private readonly TabControl _mapTabs = new() { Dock = DockStyle.Fill };
    private readonly PictureBox _preview = new() { Dock = DockStyle.Fill, SizeMode = PictureBoxSizeMode.Zoom, BackColor = Color.FromArgb(18, 21, 27) };
    private readonly Label _previewPlaceholder = new() { Dock = DockStyle.Fill, Text = "選取地圖以顯示預覽", TextAlign = ContentAlignment.MiddleCenter, ForeColor = Color.Gray };
    private readonly Label _previewTitle = new() { Dock = DockStyle.Top, Height = 58, Padding = new Padding(8, 12, 8, 4), Font = new Font("Microsoft JhengHei UI", 10F, FontStyle.Bold), ForeColor = Color.White };
    private readonly Label _previewDetails = new() { Dock = DockStyle.Bottom, Height = 72, Padding = new Padding(8), ForeColor = Color.Silver };
    private readonly Button _loadButton = new() { Text = "讀取地圖", Width = 120, Height = 38, Enabled = false };
    private readonly Button _newButton = new() { Text = "新建地圖", Width = 120, Height = 38 };
    private readonly Button _copyButton = new() { Text = "複製到自製地圖", Width = 150, Height = 38, Enabled = false };
    private readonly Button _deleteButton = new() { Text = "刪除自製地圖", Width = 130, Height = 38, Enabled = false };
    private readonly Label _hint = new() { Dock = DockStyle.Bottom, Height = 42, TextAlign = ContentAlignment.MiddleLeft, ForeColor = Color.Silver };
    private readonly string? _preferredMapId;

    public MapSelectionForm(string gamePath, string? preferredMapId = null)
    {
        Text = "Against Rome 地圖選單";
        Width = 1080; Height = 660; MinimumSize = new Size(840, 520); StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(30, 34, 42); ForeColor = Color.Gainsboro;
        _gamePath.Text = gamePath; _preferredMapId = preferredMapId;
        BuildInterface(); WireEvents();
        Shown += (_, _) => RefreshMaps();
    }

    public string GamePath => _gamePath.Text.Trim();
    public GameMapInfo? SelectedMap { get; private set; }

    internal static bool IsSelectableMap(GameMapInfo map)
        => !map.Id.StartsWith("KAMP_", StringComparison.OrdinalIgnoreCase);

    // 只有無盡（ENDL）地圖含有 Endlos_*_Siedlung*.sdl 聚落定義；複製其他類型會產生
    // 出現在無盡選單、卻缺少聚落且可能夾帶戰役腳本的壞地圖，因此禁止作為自製地圖來源。
    internal static bool CanCloneToCustom(GameMapInfo map)
        => map.IsCustom || map.Id.StartsWith("ENDL_", StringComparison.OrdinalIgnoreCase);

    private void BuildInterface()
    {
        var header = new Label
        {
            Text = "選擇要讀取的地圖，或從現有地圖建立新的自製地圖",
            Dock = DockStyle.Top, Height = 58, Padding = new Padding(14, 18, 0, 0),
            Font = new Font("Microsoft JhengHei UI", 12F, FontStyle.Bold), ForeColor = Color.White
        };
        var pathRow = new TableLayoutPanel { Dock = DockStyle.Top, Height = 46, Padding = new Padding(12, 5, 12, 5), ColumnCount = 4 };
        pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        var browse = new Button { Text = "瀏覽…", AutoSize = true, Dock = DockStyle.Fill };
        var refresh = new Button { Text = "重新整理", AutoSize = true, Dock = DockStyle.Fill };
        pathRow.Controls.Add(new Label { Text = "遊戲路徑：", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 0); pathRow.Controls.Add(_gamePath, 1, 0);
        pathRow.Controls.Add(browse, 2, 0); pathRow.Controls.Add(refresh, 3, 0);

        _customMaps.Columns.Add("地圖", 120); _customMaps.Columns.Add("名稱", 300); _customMaps.Columns.Add("類型", 110);
        _originalMaps.Columns.Add("地圖", 120); _originalMaps.Columns.Add("名稱", 270); _originalMaps.Columns.Add("類型", 140);
        var customPage = new TabPage("自製地圖") { BackColor = Color.FromArgb(34, 38, 47), Padding = new Padding(4) };
        var originalPage = new TabPage("原版地圖（不含劇情）") { BackColor = Color.FromArgb(34, 38, 47), Padding = new Padding(4) };
        customPage.Controls.Add(_customMaps); originalPage.Controls.Add(_originalMaps); _mapTabs.TabPages.Add(customPage); _mapTabs.TabPages.Add(originalPage);
        var previewImageHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8), BackColor = Color.FromArgb(26, 30, 38) };
        previewImageHost.Controls.Add(_previewPlaceholder); previewImageHost.Controls.Add(_preview);
        _previewPlaceholder.BringToFront();
        var previewPanel = new Panel { Dock = DockStyle.Fill, BackColor = Color.FromArgb(26, 30, 38) };
        previewPanel.Controls.Add(previewImageHost); previewPanel.Controls.Add(_previewTitle); previewPanel.Controls.Add(_previewDetails);
        var contentSplit = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1030, 480), SplitterDistance = 680, Panel1MinSize = 480, Panel2MinSize = 280, BackColor = Color.FromArgb(30, 34, 42) };
        contentSplit.Panel1.Controls.Add(_mapTabs); contentSplit.Panel2.Controls.Add(previewPanel);
        var listHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(12), BackColor = Color.FromArgb(34, 38, 47) };
        listHost.Controls.Add(contentSplit); listHost.Controls.Add(_hint);
        var actions = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 64, Padding = new Padding(12), FlowDirection = FlowDirection.RightToLeft, BackColor = Color.FromArgb(42, 47, 58) };
        var exit = new Button { Text = "離開", Width = 100, Height = 38, DialogResult = DialogResult.Cancel };
        actions.Controls.Add(exit); actions.Controls.Add(_loadButton); actions.Controls.Add(_newButton); actions.Controls.Add(_copyButton); actions.Controls.Add(_deleteButton);

        Controls.Add(listHost); Controls.Add(actions); Controls.Add(pathRow); Controls.Add(header);
        AcceptButton = _loadButton; CancelButton = exit;
        browse.Click += (_, _) => Browse(); refresh.Click += (_, _) => RefreshMaps();
    }

    private void WireEvents()
    {
        _customMaps.SelectedIndexChanged += (_, _) => UpdateSelectionState();
        _originalMaps.SelectedIndexChanged += (_, _) => UpdateSelectionState();
        _customMaps.DoubleClick += (_, _) => LoadSelected(); _originalMaps.DoubleClick += (_, _) => LoadSelected();
        _mapTabs.SelectedIndexChanged += (_, _) => UpdateSelectionState();
        _loadButton.Click += (_, _) => LoadSelected();
        _newButton.Click += (_, _) => CreateMap();
        _copyButton.Click += (_, _) => CopySelectedMap();
        _deleteButton.Click += (_, _) => DeleteSelected();
    }

    private void RefreshMaps(string? selectMapId = null)
    {
        bool updatingCustom = false;
        bool updatingOriginal = false;
        try
        {
            string? preferred = selectMapId ?? SelectedItem()?.Id ?? _preferredMapId;
            GameMapInfo[] maps = _catalog.List(GamePath).Where(IsSelectableMap).ToArray();
            GameMapInfo[] customMaps = maps.Where(map => map.IsCustom).ToArray();
            GameMapInfo[] originalMaps = maps.Where(map => !map.IsCustom).ToArray();
            _customMaps.BeginUpdate(); updatingCustom = true;
            _originalMaps.BeginUpdate(); updatingOriginal = true;
            _customMaps.Items.Clear(); _originalMaps.Items.Clear(); _originalMaps.Groups.Clear();
            foreach (GameMapInfo map in customMaps)
            {
                var item = new ListViewItem(map.Id) { Tag = map }; item.SubItems.Add(map.DisplayName ?? "（無標題）"); item.SubItems.Add("自製地圖"); _customMaps.Items.Add(item);
            }
            foreach (IGrouping<string, GameMapInfo> group in originalMaps.GroupBy(map => map.Category))
            {
                var listGroup = new ListViewGroup($"{group.Key}（{group.Count()}）", HorizontalAlignment.Left); _originalMaps.Groups.Add(listGroup);
                foreach (GameMapInfo map in group)
                {
                    var item = new ListViewItem(map.Id) { Tag = map, Group = listGroup };
                    item.SubItems.Add(map.DisplayName ?? "（無標題）"); item.SubItems.Add(map.Category); _originalMaps.Items.Add(item);
                }
            }
            _customMaps.EndUpdate(); updatingCustom = false;
            _originalMaps.EndUpdate(); updatingOriginal = false;
            ListViewItem? selected = BothLists().SelectMany(list => list.Items.Cast<ListViewItem>()).FirstOrDefault(item => item.Tag is GameMapInfo map && StringComparer.OrdinalIgnoreCase.Equals(map.Id, preferred));
            if (selected is null) selected = _customMaps.Items.Cast<ListViewItem>().FirstOrDefault() ?? _originalMaps.Items.Cast<ListViewItem>().FirstOrDefault();
            if (selected is not null)
            {
                _mapTabs.SelectedIndex = selected.ListView == _customMaps ? 0 : 1;
                selected.Selected = true; selected.Focused = true; selected.EnsureVisible();
            }
            _hint.Text = maps.Length == 0 ? "找不到可用的非劇情地圖。請確認遊戲路徑。" : "原版地圖為唯讀；只有無盡（ENDL）地圖能複製為可編輯的自製地圖。劇情地圖不會出現在此選單。";
        }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, "無法讀取地圖", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        finally
        {
            if (updatingCustom) _customMaps.EndUpdate();
            if (updatingOriginal) _originalMaps.EndUpdate();
            UpdateSelectionState();
        }
    }

    private void LoadSelected()
    {
        GameMapInfo? map = SelectedItem(); if (map is null) return;
        SelectedMap = map; DialogResult = DialogResult.OK; Close();
    }

    private void CreateMap()
    {
        GameMapInfo? source = SelectNewMapTemplate(_catalog.List(GamePath));
        if (source is null) { MessageBox.Show(this, "沒有可作為新地圖基礎的地圖。", Text); return; }
        string name = PromptName("新建地圖", source.DisplayName ?? "New Custom Map");
        if (string.IsNullOrWhiteSpace(name)) return;
        CloneAndOpen(source, name, "無法新建地圖");
    }

    private void CopySelectedMap()
    {
        GameMapInfo? source = SelectedItem();
        if (source is null) return;
        if (!CanCloneToCustom(source))
        {
            MessageBox.Show(this,
                "只有無盡模式（ENDL）地圖能複製為自製地圖。\n\n其他類型的地圖缺少無盡聚落定義，複製後會在無盡選單中無法正常遊玩。",
                "無法複製此地圖", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        string suggestedName = SuggestedCopyName(source);
        string name = PromptName("複製到自製地圖", suggestedName);
        if (string.IsNullOrWhiteSpace(name)) return;
        CloneAndOpen(source, name, "無法複製地圖");
    }

    private void CloneAndOpen(GameMapInfo source, string name, string errorTitle)
    {
        try
        {
            int slot = _endlessCatalog.GetNextFreeSlot(GamePath);
            _cloner.Clone(GamePath, source.Id, slot, name.Trim());
            string mapId = $"ENDL_{slot:000}"; RefreshMaps(mapId);
            SelectedMap = _catalog.Require(GamePath, mapId); DialogResult = DialogResult.OK; Close();
        }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, errorTitle, MessageBoxButtons.OK, MessageBoxIcon.Error); }
    }

    // 新建地圖只能以原廠無盡地圖為範本，確保產出的自製地圖帶有完整的聚落定義。
    internal static GameMapInfo? SelectNewMapTemplate(IEnumerable<GameMapInfo> maps)
        => maps.FirstOrDefault(map => IsSelectableMap(map) && !map.IsCustom && map.Id.StartsWith("ENDL_", StringComparison.OrdinalIgnoreCase));

    internal static string SuggestedCopyName(GameMapInfo source) => $"{source.DisplayName ?? source.Id} - Copy";

    private void DeleteSelected()
    {
        GameMapInfo? map = SelectedItem(); if (map?.IsCustom != true || map.EndlessSlot is null) return;
        DialogResult result = MessageBox.Show(this, $"確定要刪除這張自製地圖嗎？\n\n{map.DisplayName ?? map.Id}\n{map.Id}", "刪除自製地圖", MessageBoxButtons.YesNo, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2);
        if (result != DialogResult.Yes) return;
        try { _deleter.Delete(GamePath, map.EndlessSlot.Value); RefreshMaps(); }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, "無法刪除地圖", MessageBoxButtons.OK, MessageBoxIcon.Error); }
    }

    private void UpdateSelectionState()
    {
        GameMapInfo? map = SelectedItem();
        _loadButton.Enabled = map is not null;
        _copyButton.Enabled = map is not null && CanCloneToCustom(map);
        _deleteButton.Enabled = map?.IsCustom == true;
        UpdatePreview(map);
    }

    private void UpdatePreview(GameMapInfo? map)
    {
        Image? oldImage = _preview.Image;
        _preview.Image = null;
        oldImage?.Dispose();
        _previewTitle.Text = map?.DisplayName ?? map?.Id ?? "地圖預覽";
        _previewDetails.Text = map is null ? "請從左側選取一張地圖。" : $"{map.Id}\n{(map.IsCustom ? "自製地圖" : map.Category + "（原版）")}";

        if (map is not null)
        {
            try { _preview.Image = LoadPreviewImage(map.DirectoryPath); }
            catch { _preview.Image = null; }
        }
        _previewPlaceholder.Text = map is null ? "選取地圖以顯示預覽" : "這張地圖沒有可用的預覽圖";
        _previewPlaceholder.Visible = _preview.Image is null;
        if (_previewPlaceholder.Visible) _previewPlaceholder.BringToFront(); else _preview.BringToFront();
    }

    internal static Image? LoadPreviewImage(string mapDirectory)
    {
        string path = Path.Combine(mapDirectory, "minimap.bmp");
        if (!File.Exists(path)) return null;
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        using var source = Image.FromStream(stream);
        return new Bitmap(source);
    }
    private GameMapInfo? SelectedItem()
    {
        ListView active = _mapTabs.SelectedIndex == 0 ? _customMaps : _originalMaps;
        return active.SelectedItems.Count == 1 ? active.SelectedItems[0].Tag as GameMapInfo : null;
    }
    private IEnumerable<ListView> BothLists() { yield return _customMaps; yield return _originalMaps; }
    private static ListView CreateMapList() => new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, MultiSelect = false, HideSelection = false };
    private void Browse()
    {
        using var dialog = new FolderBrowserDialog { Description = "選擇 Against Rome 遊戲資料夾", SelectedPath = Directory.Exists(GamePath) ? GamePath : "" };
        if (dialog.ShowDialog(this) != DialogResult.OK) return; _gamePath.Text = dialog.SelectedPath; RefreshMaps();
    }
    private string PromptName(string title, string value)
    {
        using var form = new Form { Text = title, Width = 450, Height = 210, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor, FormBorderStyle = FormBorderStyle.FixedDialog, MinimizeBox = false, MaximizeBox = false };
        var label = new Label { Text = "地圖名稱", Dock = DockStyle.Top, Height = 34, Padding = new Padding(12, 10, 0, 0) };
        var input = new TextBox { Text = value, Dock = DockStyle.Top, Margin = new Padding(12) };
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(12, 8, 12, 8), FlowDirection = FlowDirection.RightToLeft };
        var ok = new Button { Text = "建立並開啟", DialogResult = DialogResult.OK, Width = 130, Height = 34 };
        var cancel = new Button { Text = "取消", DialogResult = DialogResult.Cancel, Width = 90, Height = 34 };
        buttons.Controls.Add(ok); buttons.Controls.Add(cancel);
        form.Controls.Add(input); form.Controls.Add(label); form.Controls.Add(buttons);
        form.AcceptButton = ok; form.CancelButton = cancel;
        return form.ShowDialog(this) == DialogResult.OK ? input.Text : "";
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing) { Image? image = _preview.Image; _preview.Image = null; image?.Dispose(); }
        base.Dispose(disposing);
    }
}
