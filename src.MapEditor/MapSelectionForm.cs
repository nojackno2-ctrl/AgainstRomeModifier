using AgainstRomeModifier.Maps;
using AgainstRomeModifier;

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
    private readonly PictureBox _preview = new() { Dock = DockStyle.Fill, SizeMode = PictureBoxSizeMode.Zoom, BackColor = WinFormsTheme.Window };
    private readonly Label _previewPlaceholder = new() { Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleCenter, ForeColor = WinFormsTheme.TextMuted };
    private readonly Label _previewTitle = new() { Dock = DockStyle.Top, Height = 62, Padding = new Padding(12, 14, 12, 4), Font = WinFormsTheme.CreateDisplayFont(11F), ForeColor = WinFormsTheme.TextPrimary };
    private readonly Label _previewDetails = new() { Dock = DockStyle.Bottom, Height = 76, Padding = new Padding(12), ForeColor = WinFormsTheme.TextSecondary };
    private readonly Button _loadButton = new() { Width = 120, Height = 38, Enabled = false };
    private readonly Button _newButton = new() { Width = 170, Height = 38 };
    private readonly Button _blankButton = new() { Width = 145, Height = 38 };
    private readonly Button _copyButton = new() { Width = 150, Height = 38, Enabled = false };
    private readonly Button _deleteButton = new() { Width = 130, Height = 38, Enabled = false };
    private readonly Label _hint = new() { Dock = DockStyle.Bottom, Height = 46, Padding = new Padding(4, 0, 0, 0), TextAlign = ContentAlignment.MiddleLeft, ForeColor = WinFormsTheme.TextSecondary };
    private readonly Label _header = new() { Dock = DockStyle.Top, Height = 64, Padding = new Padding(18, 20, 0, 0), Font = WinFormsTheme.CreateDisplayFont(13F), ForeColor = WinFormsTheme.TextPrimary };
    private readonly Label _lblGamePath = new() { AutoSize = true, Anchor = AnchorStyles.Left };
    private readonly Button _browse = new() { AutoSize = true, Dock = DockStyle.Fill };
    private readonly Button _refresh = new() { AutoSize = true, Dock = DockStyle.Fill };
    private readonly Button _exit = new() { Width = 100, Height = 38, DialogResult = DialogResult.Cancel };
    private Button btnLangZH = null!;
    private Button btnLangEN = null!;
    private readonly string? _preferredMapId;


    public MapSelectionForm(string gamePath, string? preferredMapId = null)
    {
        AutoScaleDimensions = new SizeF(96F, 96F); AutoScaleMode = AutoScaleMode.Dpi; // 以 96 DPI 設計，PerMonitorV2 下依實際 DPI 縮放固定像素版面
        Width = 1080; Height = 660; MinimumSize = new Size(900, 520); StartPosition = FormStartPosition.CenterScreen;
        BackColor = WinFormsTheme.Window; ForeColor = WinFormsTheme.TextPrimary; Font = WinFormsTheme.CreateFont(9F);
        _gamePath.Text = gamePath; _preferredMapId = preferredMapId;
        BuildInterface();
        WinFormsTheme.Apply(this);
        WinFormsTheme.StylePrimaryButton(_loadButton);
        WinFormsTheme.StyleDangerButton(_deleteButton);
        WireEvents();
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
        var pathRow = new TableLayoutPanel { Dock = DockStyle.Top, Height = 46, MinimumSize = new Size(0, 46), AutoSize = true, Padding = new Padding(12, 5, 12, 5), ColumnCount = 4 };
        pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); pathRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        pathRow.Controls.Add(_lblGamePath, 0, 0); pathRow.Controls.Add(_gamePath, 1, 0);
        pathRow.Controls.Add(_browse, 2, 0); pathRow.Controls.Add(_refresh, 3, 0);

        _customMaps.Columns.Add("地圖", 120); _customMaps.Columns.Add("名稱", 300); _customMaps.Columns.Add("類型", 110);
        _originalMaps.Columns.Add("地圖", 120); _originalMaps.Columns.Add("名稱", 270); _originalMaps.Columns.Add("類型", 140);
        var customPage = new TabPage("自製地圖") { BackColor = WinFormsTheme.Surface, Padding = new Padding(6) };
        var originalPage = new TabPage("原版地圖（不含劇情）") { BackColor = WinFormsTheme.Surface, Padding = new Padding(6) };
        customPage.Controls.Add(_customMaps); originalPage.Controls.Add(_originalMaps); _mapTabs.TabPages.Add(customPage); _mapTabs.TabPages.Add(originalPage);
        var previewImageHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(12), BackColor = WinFormsTheme.Surface };
        previewImageHost.Controls.Add(_previewPlaceholder); previewImageHost.Controls.Add(_preview);
        _previewPlaceholder.BringToFront();
        var previewPanel = new Panel { Dock = DockStyle.Fill, BackColor = WinFormsTheme.Surface };
        previewPanel.Controls.Add(previewImageHost); previewPanel.Controls.Add(_previewTitle); previewPanel.Controls.Add(_previewDetails);
        var contentSplit = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel2, Size = new Size(1030, 480), SplitterDistance = 680, Panel1MinSize = 480, Panel2MinSize = 280, BackColor = WinFormsTheme.Window };
        contentSplit.Panel1.Controls.Add(_mapTabs); contentSplit.Panel2.Controls.Add(previewPanel);
        var listHost = new Panel { Dock = DockStyle.Fill, Padding = new Padding(16), BackColor = WinFormsTheme.Surface };
        listHost.Controls.Add(contentSplit); listHost.Controls.Add(_hint);
        var actions = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 70, Padding = new Padding(14, 16, 14, 10), FlowDirection = FlowDirection.RightToLeft, BackColor = WinFormsTheme.Surface };
        actions.Controls.Add(_exit); actions.Controls.Add(_loadButton); actions.Controls.Add(_newButton); actions.Controls.Add(_blankButton); actions.Controls.Add(_copyButton); actions.Controls.Add(_deleteButton);

        btnLangZH = new Button {
            Text = "繁體中文",
            Size = new Size(90, 30),
            FlatStyle = FlatStyle.Flat,
            Font = WinFormsTheme.CreateFont(9F),
            Cursor = Cursors.Hand,
            Anchor = AnchorStyles.Top | AnchorStyles.Right
        };
        btnLangZH.Click += (s, e) => {
            if (AgainstRomeModifier.Loc.CurrentLanguage != AgainstRomeModifier.Language.TraditionalChinese) {
                AgainstRomeModifier.Loc.CurrentLanguage = AgainstRomeModifier.Language.TraditionalChinese;
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
            }
        };

        btnLangEN = new Button {
            Text = "English",
            Size = new Size(90, 30),
            FlatStyle = FlatStyle.Flat,
            Font = WinFormsTheme.CreateFont(9F),
            Cursor = Cursors.Hand,
            Anchor = AnchorStyles.Top | AnchorStyles.Right
        };
        btnLangEN.Click += (s, e) => {
            if (AgainstRomeModifier.Loc.CurrentLanguage != AgainstRomeModifier.Language.English) {
                AgainstRomeModifier.Loc.CurrentLanguage = AgainstRomeModifier.Language.English;
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
            }
        };

        Controls.Add(listHost); Controls.Add(actions); Controls.Add(pathRow); Controls.Add(_header);
        Controls.Add(btnLangZH); Controls.Add(btnLangEN);
        btnLangZH.BringToFront(); btnLangEN.BringToFront();

        AcceptButton = _loadButton; CancelButton = _exit;
        _browse.Click += (_, _) => Browse(); _refresh.Click += (_, _) => RefreshMaps();

        LayoutLanguageButtons();
        this.Resize += (_, _) => LayoutLanguageButtons();

        UpdateLanguageButtonStyles();
        ApplyLanguageToUI();
    }

    private void WireEvents()
    {
        _customMaps.SelectedIndexChanged += (_, _) => UpdateSelectionState();
        _originalMaps.SelectedIndexChanged += (_, _) => UpdateSelectionState();
        _customMaps.DoubleClick += (_, _) => LoadSelected(); _originalMaps.DoubleClick += (_, _) => LoadSelected();
        _mapTabs.SelectedIndexChanged += (_, _) => UpdateSelectionState();
        _loadButton.Click += (_, _) => LoadSelected();
        _newButton.Click += (_, _) => CreateMap();
        _blankButton.Click += (_, _) => CreateBlankMap();
        _copyButton.Click += (_, _) => CopySelectedMap();
        _deleteButton.Click += (_, _) => DeleteSelected();
    }

    private void LayoutLanguageButtons()
    {
        // 間距以按鈕高度換算（96 DPI 時為右邊界 20、間隔 10、上緣 14），DPI 縮放後仍保持比例。
        int unit = btnLangEN.Height;
        btnLangEN.Location = new Point(ClientSize.Width - btnLangEN.Width - unit * 2 / 3, unit * 14 / 30);
        btnLangZH.Location = new Point(btnLangEN.Left - btnLangZH.Width - unit / 3, btnLangEN.Top);
    }

    private void UpdateLanguageButtonStyles()
    {
        bool isZh = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.TraditionalChinese;
        WinFormsTheme.StyleLanguageButton(btnLangZH, isZh);
        WinFormsTheme.StyleLanguageButton(btnLangEN, !isZh);
    }

    private void ApplyLanguageToUI()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        Text = isEn ? "Against Rome Map Editor - Selection" : "Against Rome 地圖選單";
        _header.Text = isEn ? "Select a map, or build an editable custom map from endless template" : "選擇地圖，或從無盡範本建立可編輯的自製地圖";
        _lblGamePath.Text = isEn ? "Game Path:" : "遊戲路徑：";
        _browse.Text = isEn ? "Browse…" : "瀏覽…";
        _refresh.Text = isEn ? "Refresh" : "重新整理";

        _customMaps.Columns[0].Text = isEn ? "Map ID" : "地圖";
        _customMaps.Columns[1].Text = isEn ? "Name" : "名稱";
        _customMaps.Columns[2].Text = isEn ? "Type" : "類型";

        _originalMaps.Columns[0].Text = isEn ? "Map ID" : "地圖";
        _originalMaps.Columns[1].Text = isEn ? "Name" : "名稱";
        _originalMaps.Columns[2].Text = isEn ? "Type" : "類型";

        _mapTabs.TabPages[0].Text = isEn ? "Custom Maps" : "自製地圖";
        _mapTabs.TabPages[1].Text = isEn ? "Original Maps (Excl. Campaign)" : "原版地圖（不含劇情）";

        _loadButton.Text = isEn ? "Load Map" : "讀取地圖";
        _newButton.Text = isEn ? "Build from Endless" : "從無盡範本建立";
        _blankButton.Text = isEn ? "Flat Template" : "平坦範本地圖";
        _copyButton.Text = isEn ? "Copy to Custom" : "複製到自製地圖";
        _deleteButton.Text = isEn ? "Delete Custom" : "刪除自製地圖";
        _exit.Text = isEn ? "Exit" : "離開";

        UpdatePreview(SelectedItem());
    }

    private void RefreshMaps(string? selectMapId = null)
    {
        bool updatingCustom = false;
        bool updatingOriginal = false;
        try
        {
            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            string? preferred = selectMapId ?? SelectedItem()?.Id ?? _preferredMapId;
            GameMapInfo[] maps = _catalog.List(GamePath).Where(IsSelectableMap).ToArray();
            GameMapInfo[] customMaps = maps.Where(map => map.IsCustom).ToArray();
            GameMapInfo[] originalMaps = maps.Where(map => !map.IsCustom).ToArray();
            _customMaps.BeginUpdate(); updatingCustom = true;
            _originalMaps.BeginUpdate(); updatingOriginal = true;
            _customMaps.Items.Clear(); _originalMaps.Items.Clear(); _originalMaps.Groups.Clear();
            foreach (GameMapInfo map in customMaps)
            {
                var item = new ListViewItem(map.Id) { Tag = map }; 
                item.SubItems.Add(map.DisplayName ?? (isEn ? "(No Title)" : "（無標題）")); 
                item.SubItems.Add(isEn ? "Custom Map" : "自製地圖"); 
                _customMaps.Items.Add(item);
            }
            foreach (IGrouping<string, GameMapInfo> group in originalMaps.GroupBy(map => map.Category))
            {
                string groupTitle = AgainstRomeModifier.Loc.GetFactionName(group.Key);
                var listGroup = new ListViewGroup($"{groupTitle}（{group.Count()}）", HorizontalAlignment.Left); _originalMaps.Groups.Add(listGroup);
                foreach (GameMapInfo map in group)
                {
                    var item = new ListViewItem(map.Id) { Tag = map, Group = listGroup };
                    item.SubItems.Add(map.DisplayName ?? (isEn ? "(No Title)" : "（無標題）")); 
                    item.SubItems.Add(AgainstRomeModifier.Loc.GetFactionName(map.Category)); 
                    _originalMaps.Items.Add(item);
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
            _hint.Text = maps.Length == 0 
                ? (isEn ? "No valid non-campaign maps found. Please verify game path." : "找不到可用的非劇情地圖。請確認遊戲路徑。") 
                : (isEn ? "\"Build from Endless\" keeps template terrain; \"Flat Template\" flattens terrain and keeps the template's settlements and scripts. Original maps are read-only." : "「從無盡範本建立」保留範本地形；「平坦範本地圖」整平地形並保留範本聚落與腳本。原版地圖維持唯讀。");
        }
        catch (Exception ex) {
            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            MessageBox.Show(this, ex.Message, isEn ? "Failed to read maps" : "無法讀取地圖", MessageBoxButtons.OK, MessageBoxIcon.Error); 
        }
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
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        GameMapInfo? source = SelectNewMapTemplate(_catalog.List(GamePath));
        if (source is null) { MessageBox.Show(this, isEn ? "No map available as a base for the new map." : "沒有可作為新地圖基礎的地圖。", Text); return; }
        string name = PromptName(isEn ? "Build from Endless" : "從無盡範本建立", source.DisplayName ?? "New Custom Map",
            MapTemplateInventory.Read(GamePath, source.DirectoryPath).Describe(source.Id, false, isEn));
        if (string.IsNullOrWhiteSpace(name)) return;
        CloneAndOpen(source, name, isEn ? "Failed to create map" : "無法新建地圖");
    }

    /// <summary>
    /// 平坦範本地圖：複製無盡範本（保留聚落、腳本、DATA 與快取規則），
    /// 開啟後編輯器立即把地形設為待儲存的空白地形（整平、單一材質、清除阻擋、重設頂點色／平滑／光照）。
    /// </summary>
    private void CreateBlankMap()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        GameMapInfo? source = SelectNewMapTemplate(_catalog.List(GamePath));
        if (source is null) { MessageBox.Show(this, isEn ? "No map available as a base for the new map." : "沒有可作為新地圖基礎的地圖。", Text); return; }
        // 地圖名稱寫入遊戲的 CP1251 文字檔，中文無法編碼；預設名稱一律用英文。
        string name = PromptName(isEn ? "Build Flat Template" : "建立平坦範本地圖", "Flat Template Map",
            MapTemplateInventory.Read(GamePath, source.DirectoryPath).Describe(source.Id, true, isEn));
        if (string.IsNullOrWhiteSpace(name)) return;
        CreatedBlankMap = true;
        CloneAndOpen(source, name, isEn ? "Failed to create map" : "無法新建地圖");
        if (DialogResult != DialogResult.OK) CreatedBlankMap = false;
    }

    /// <summary>為 true 時，呼叫端應以空白地形開啟 <see cref="SelectedMap"/>。</summary>
    public bool CreatedBlankMap { get; private set; }

    private void CopySelectedMap()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        GameMapInfo? source = SelectedItem();
        if (source is null) return;
        if (!CanCloneToCustom(source))
        {
            MessageBox.Show(this,
                isEn ? "Only Endless Mode (ENDL) maps can be copied to custom maps.\n\nOther map types lack endless settlement definitions and will not be playable." : "只有無盡模式（ENDL）地圖能複製為自製地圖。\n\n其他類型的地圖缺少無盡聚落定義，複製後會在無盡選單中無法正常遊玩。",
                isEn ? "Cannot Copy Map" : "無法複製此地圖", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        string suggestedName = SuggestedCopyName(source);
        string name = PromptName(isEn ? "Copy to Custom Map" : "複製到自製地圖", suggestedName,
            MapTemplateInventory.Read(GamePath, source.DirectoryPath).Describe(source.Id, false, isEn));
        if (string.IsNullOrWhiteSpace(name)) return;
        CloneAndOpen(source, name, isEn ? "Failed to copy map" : "無法複製地圖");
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

    internal static GameMapInfo? SelectNewMapTemplate(IEnumerable<GameMapInfo> maps)
        => maps.FirstOrDefault(map => IsSelectableMap(map) && !map.IsCustom && map.Id.StartsWith("ENDL_", StringComparison.OrdinalIgnoreCase));

    internal static string SuggestedCopyName(GameMapInfo source) => $"{source.DisplayName ?? source.Id} - Copy";

    private void DeleteSelected()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        GameMapInfo? map = SelectedItem(); if (map?.IsCustom != true || map.EndlessSlot is null) return;
        string msg = isEn 
            ? $"Are you sure you want to permanently delete this custom map?\n\n{map.DisplayName ?? map.Id}\n{map.Id}"
            : $"確定要刪除這張自製地圖嗎？\n\n{map.DisplayName ?? map.Id}\n{map.Id}";
        string title = isEn ? "Delete Custom Map" : "刪除自製地圖";
        DialogResult result = MessageBox.Show(this, msg, title, MessageBoxButtons.YesNo, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2);
        if (result != DialogResult.Yes) return;
        try { _deleter.Delete(GamePath, map.EndlessSlot.Value); RefreshMaps(); }
        catch (Exception ex) { MessageBox.Show(this, ex.Message, isEn ? "Failed to delete map" : "無法刪除地圖", MessageBoxButtons.OK, MessageBoxIcon.Error); }
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
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        Image? oldImage = _preview.Image;
        _preview.Image = null;
        oldImage?.Dispose();
        _previewTitle.Text = map?.DisplayName ?? map?.Id ?? (isEn ? "Map Preview" : "地圖預覽");
        
        string customText = isEn ? "Custom Map" : "自製地圖";
        string originalText = isEn ? " (Original)" : "（原版）";
        string factionName = map is not null ? AgainstRomeModifier.Loc.GetFactionName(map.Category) : "";
        
        _previewDetails.Text = map is null 
            ? (isEn ? "Please select a map from the left." : "請從左側選取一張地圖。") 
            : $"{map.Id}\n{(map.IsCustom ? customText : factionName + originalText)}";

        if (map is not null)
        {
            try { _preview.Image = LoadPreviewImage(map.DirectoryPath); }
            catch (Exception ex) {
                System.Diagnostics.Debug.WriteLine($"載入地圖預覽失敗 ({map.DirectoryPath}): {ex.Message}");
                _preview.Image = null;
            }
        }
        _previewPlaceholder.Text = map is null 
            ? (isEn ? "Select a map to view preview" : "選取地圖以顯示預覽") 
            : (isEn ? "No preview image available for this map" : "這張地圖沒有可用的預覽圖");
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
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        using var dialog = new FolderBrowserDialog { Description = isEn ? "Select Against Rome Game Folder" : "選擇 Against Rome 遊戲資料夾", SelectedPath = Directory.Exists(GamePath) ? GamePath : "" };
        if (dialog.ShowDialog(this) != DialogResult.OK) return; _gamePath.Text = dialog.SelectedPath; RefreshMaps();
    }

    private string PromptName(string title, string value, string? details = null)
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        using var form = BuildNameDialog(title, value, details, isEn, BackColor, ForeColor, out TextBox input);
        return form.ShowDialog(this) == DialogResult.OK ? input.Text : "";
    }

    internal static Form BuildNameDialog(string title, string value, string? details, bool isEn, Color back, Color fore, out TextBox input)
    {
        var form = new Form { Text = title, Width = details is null ? 450 : 560, Height = details is null ? 210 : 420,
            AutoScaleDimensions = new SizeF(96, 96), AutoScaleMode = AutoScaleMode.Dpi,
            StartPosition = FormStartPosition.CenterParent, BackColor = back, ForeColor = fore, FormBorderStyle = FormBorderStyle.FixedDialog, MinimizeBox = false, MaximizeBox = false };
        var label = new Label { Text = isEn ? "Map Name (Latin letters and digits; the game cannot display Chinese)" : "地圖名稱（請用英文或數字，遊戲無法顯示中文）", Dock = DockStyle.Top, Height = 34, Padding = new Padding(12, 10, 0, 0) };
        input = new TextBox { Text = value, Dock = DockStyle.Top, Margin = new Padding(12) };
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(12, 8, 12, 8), FlowDirection = FlowDirection.RightToLeft };
        var ok = new Button { Text = isEn ? "Create & Open" : "建立並開啟", DialogResult = DialogResult.OK, Width = 130, Height = 34 };
        var cancel = new Button { Text = isEn ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, Width = 90, Height = 34 };
        buttons.Controls.Add(ok); buttons.Controls.Add(cancel);
        if (details is not null) form.Controls.Add(new TextBox { Text = details, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Vertical, Dock = DockStyle.Fill, BorderStyle = BorderStyle.None, BackColor = back, ForeColor = fore });
        form.Controls.Add(input); form.Controls.Add(label); form.Controls.Add(buttons);
        form.AcceptButton = ok; form.CancelButton = cancel;
        form.ActiveControl = input;
        return form;
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing) { Image? image = _preview.Image; _preview.Image = null; image?.Dispose(); }
        base.Dispose(disposing);
    }
}
