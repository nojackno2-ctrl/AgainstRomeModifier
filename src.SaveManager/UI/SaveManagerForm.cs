using System;
using System.IO;
using System.Threading.Tasks;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier {
    public class SaveManagerForm : Form {
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        [DllImport("gdi32.dll", EntryPoint = "DeleteObject")]
        private static extern bool DeleteObject(IntPtr hObject);

        private Panel pnlTitleBar = null!;
        private Label lblMainTitle = null!;
        private Button btnClose = null!;
        private Button btnMinimize = null!;

        private Label lblGamePath = null!;
        private TextBox txtGamePath = null!;
        private Button btnBrowseGamePath = null!;

        private DataGridView dgvGameSaves = null!;
        private DataGridView dgvBackups = null!;
        private PictureBox picSavePreview = null!;
        private Label lblSaveDetail = null!;
        private Label lblGameSavesTitle = null!;
        private Label lblBackupsTitle = null!;
        private Label lblDetailTitle = null!;
        private Button btnBackupSave = null!;
        private Button btnRepairEndlessAi = null!;
        private Button btnDeleteSave = null!;
        private Button btnRefreshSaves = null!;
        private Button btnRestoreBackup = null!;
        private Button btnDeleteBackup = null!;
        private Button btnLangZH = null!;
        private Button btnLangEN = null!;

        private bool _savesRefreshInFlight;
        private bool _repairInFlight;
        private SaveBackupService saveBackupService = null!;

        private Font fontJhengHei115B = new Font("Microsoft JhengHei", 11.5F, FontStyle.Bold);
        private Font fontJhengHei95B = new Font("Microsoft JhengHei", 9.5F, FontStyle.Bold);
        private Font fontJhengHei9R = new Font("Microsoft JhengHei", 9F, FontStyle.Regular);
        private Font fontJhengHei10R = new Font("Microsoft JhengHei", 10F, FontStyle.Regular);
        private Font fontJhengHei105B = new Font("Microsoft JhengHei", 10.5F, FontStyle.Bold);

        private bool dragging;
        private Point dragStart = new Point(0, 0);

        public SaveManagerForm(string? initialGamePath = null) {
            InitializeComponent();
            saveBackupService = new SaveBackupService(Path.Combine(AppContext.BaseDirectory, "SavesBackup"));
            
            // 初始遊戲路徑解析與 Modifier 共用同一順序（明確路徑→同目錄可攜→登錄檔→預設）。
            txtGamePath.Text = GameDirectoryLocator.ResolveInitialGamePath(initialGamePath);

            UpdateLanguageButtonStyles();
            ApplyLanguageToUI();
        }

        private string GetGamePath() {
            return txtGamePath.Text.Trim();
        }

        private void InitializeComponent() {
            this.Text = "Against Rome Modifier - Save Manager";
            this.FormBorderStyle = FormBorderStyle.None;
            this.ClientSize = new Size(1180, 780);
            this.MinimumSize = new Size(960, 640);
            this.StartPosition = FormStartPosition.CenterScreen;
            this.BackColor = Color.FromArgb(9, 12, 18);
            this.ForeColor = Color.FromArgb(230, 235, 240);
            this.DoubleBuffered = true;

            void UpdateRoundedRegion() {
                if (Width <= 0 || Height <= 0) return;
                IntPtr ptr = CreateRoundRectRgn(0, 0, Width, Height, 15, 15);
                Region? previousRegion = this.Region;
                this.Region = Region.FromHrgn(ptr);
                DeleteObject(ptr);
                previousRegion?.Dispose();
            }
            this.Load += (s, e) => UpdateRoundedRegion();
            this.SizeChanged += (s, e) => UpdateRoundedRegion();

            this.Paint += (s, e) => {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                using (Pen p = new Pen(Color.FromArgb(0, 230, 255), 2)) {
                    using (GraphicsPath path = GetRoundPath(new Rectangle(0, 0, this.Width, this.Height), 15)) {
                        e.Graphics.DrawPath(p, path);
                    }
                }
            };

            pnlTitleBar = new Panel {
                Location = new Point(0, 0),
                Size = new Size(this.Width, 56),
                BackColor = Color.FromArgb(13, 17, 25),
                Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblMainTitle = new Label {
                Text = "AGAINST ROME 存檔管理器",
                Location = new Point(24, 16),
                Size = new Size(360, 26),
                Font = fontJhengHei115B,
                ForeColor = Color.FromArgb(226, 241, 252)
            };
            lblMainTitle.MouseDown += TitleBar_MouseDown;
            lblMainTitle.MouseMove += TitleBar_MouseMove;
            lblMainTitle.MouseUp += TitleBar_MouseUp;
            pnlTitleBar.Controls.Add(lblMainTitle);

            btnClose = new Button {
                Text = "×",
                Location = new Point(this.Width - 46, 12),
                Size = new Size(30, 30),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.Transparent,
                ForeColor = Color.White,
                Anchor = AnchorStyles.Top | AnchorStyles.Right
            };
            btnClose.FlatAppearance.BorderSize = 0;
            btnClose.Cursor = Cursors.Hand;
            btnClose.Click += (s, e) => this.Close();
            btnClose.MouseEnter += (s, e) => btnClose.BackColor = Color.FromArgb(232, 17, 35);
            btnClose.MouseLeave += (s, e) => btnClose.BackColor = Color.Transparent;
            pnlTitleBar.Controls.Add(btnClose);

            btnMinimize = new Button {
                Text = "—",
                Location = new Point(this.Width - 86, 12),
                Size = new Size(30, 30),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.Transparent,
                ForeColor = Color.White,
                Anchor = AnchorStyles.Top | AnchorStyles.Right
            };
            btnMinimize.FlatAppearance.BorderSize = 0;
            btnMinimize.Cursor = Cursors.Hand;
            btnMinimize.Click += (s, e) => this.WindowState = FormWindowState.Minimized;
            btnMinimize.MouseEnter += (s, e) => btnMinimize.BackColor = Color.FromArgb(45, 45, 55);
            btnMinimize.MouseLeave += (s, e) => btnMinimize.BackColor = Color.Transparent;
            btnLangZH = new Button {
                Text = "繁體中文",
                Location = new Point(this.Width - 276, 12),
                Size = new Size(80, 30),
                FlatStyle = FlatStyle.Flat,
                Font = fontJhengHei9R,
                Cursor = Cursors.Hand,
                Anchor = AnchorStyles.Top | AnchorStyles.Right
            };
            btnLangZH.FlatAppearance.BorderSize = 1;
            btnLangZH.Click += (s, e) => {
                if (Loc.CurrentLanguage != Language.TraditionalChinese) {
                    Loc.CurrentLanguage = Language.TraditionalChinese;
                    UpdateLanguageButtonStyles();
                    ApplyLanguageToUI();
                }
            };
            pnlTitleBar.Controls.Add(btnLangZH);

            btnLangEN = new Button {
                Text = "English",
                Location = new Point(this.Width - 186, 12),
                Size = new Size(80, 30),
                FlatStyle = FlatStyle.Flat,
                Font = fontJhengHei9R,
                Cursor = Cursors.Hand,
                Anchor = AnchorStyles.Top | AnchorStyles.Right
            };
            btnLangEN.FlatAppearance.BorderSize = 1;
            btnLangEN.Click += (s, e) => {
                if (Loc.CurrentLanguage != Language.English) {
                    Loc.CurrentLanguage = Language.English;
                    UpdateLanguageButtonStyles();
                    ApplyLanguageToUI();
                }
            };
            pnlTitleBar.Controls.Add(btnLangEN);

            pnlTitleBar.Controls.Add(btnMinimize);
            this.Controls.Add(pnlTitleBar);

            // Path Area
            Panel pnlTopBar = new Panel {
                Location = new Point(0, 56),
                Size = new Size(this.Width, 60),
                BackColor = Color.FromArgb(12, 16, 24),
                Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right
            };
            
            lblGamePath = new Label {
                Text = Loc.Get("GamePathLabel"),
                Location = new Point(16, 20),
                Size = new Size(80, 20),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(128, 143, 163)
            };
            
            Panel pathWrapper = new Panel {
                Location = new Point(100, 14),
                Size = new Size(400, 32),
                BackColor = Color.FromArgb(22, 28, 39)
            };
            pathWrapper.Paint += (s, e) => {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                using (Pen p = new Pen(Color.FromArgb(50, 52, 70), 1)) {
                    e.Graphics.DrawRectangle(p, 0, 0, pathWrapper.Width - 1, pathWrapper.Height - 1);
                }
            };
            
            txtGamePath = new TextBox {
                Location = new Point(9, 7),
                Size = new Size(382, 20),
                BackColor = pathWrapper.BackColor,
                ForeColor = Color.FromArgb(222, 230, 240),
                BorderStyle = BorderStyle.None,
                Font = fontJhengHei95B
            };
            pathWrapper.Controls.Add(txtGamePath);
            
            btnBrowseGamePath = new Button {
                Text = Loc.Get("BrowseButton"),
                Location = new Point(510, 14),
                Size = new Size(100, 32)
            };
            StyleButton(btnBrowseGamePath, Color.FromArgb(31, 37, 49), Color.FromArgb(230, 235, 240), Color.FromArgb(0, 220, 255));
            btnBrowseGamePath.Click += (s, e) => {
                using (var fbd = new FolderBrowserDialog()) {
                    if (fbd.ShowDialog() == DialogResult.OK) {
                        txtGamePath.Text = fbd.SelectedPath;
                        RefreshSavesAndBackups();
                    }
                }
            };
            
            pnlTopBar.Controls.Add(lblGamePath);
            pnlTopBar.Controls.Add(pathWrapper);
            pnlTopBar.Controls.Add(btnBrowseGamePath);
            this.Controls.Add(pnlTopBar);

            // Main Content Area
            Panel root = new Panel {
                Name = "rootContent",
                Location = new Point(0, 116),
                Size = new Size(this.Width, this.Height - 116),
                Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right,
                BackColor = Color.Transparent,
                Padding = new Padding(20)
            };
            this.Controls.Add(root);

            TableLayoutPanel contentLayout = new TableLayoutPanel {
                Name = "contentLayout",
                Dock = DockStyle.Fill,
                BackColor = Color.Transparent,
                ColumnCount = 2,
                RowCount = 1,
                Margin = Padding.Empty,
                Padding = Padding.Empty
            };
            contentLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 68F));
            contentLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 32F));
            contentLayout.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));
            root.Controls.Add(contentLayout);

            Panel pnlLeftSave = new Panel { Dock = DockStyle.Fill, Margin = new Padding(0, 0, 10, 0) };
            Panel pnlRightSave = new Panel { Dock = DockStyle.Fill, Margin = new Padding(0, 0, 0, 0) };
            contentLayout.Controls.Add(pnlLeftSave, 0, 0);
            contentLayout.Controls.Add(pnlRightSave, 1, 0);

            TableLayoutPanel listLayout = new TableLayoutPanel {
                Dock = DockStyle.Fill,
                BackColor = Color.Transparent,
                ColumnCount = 1,
                RowCount = 2,
                Margin = Padding.Empty,
                Padding = Padding.Empty
            };
            listLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100F));
            listLayout.RowStyles.Add(new RowStyle(SizeType.Percent, 50F));
            listLayout.RowStyles.Add(new RowStyle(SizeType.Percent, 50F));
            pnlLeftSave.Controls.Add(listLayout);

            Panel gameCard = new Panel { Name = "gameCard", Dock = DockStyle.Fill, Margin = new Padding(0, 0, 0, 5) };
            gameCard.Paint += CardPanel_Paint;
            Panel backupsCard = new Panel { Name = "backupsCard", Dock = DockStyle.Fill, Margin = new Padding(0, 5, 0, 0) };
            backupsCard.Paint += CardPanel_Paint;
            listLayout.Controls.Add(gameCard, 0, 0);
            listLayout.Controls.Add(backupsCard, 0, 1);

            Panel detailCard = new Panel { Dock = DockStyle.Fill };
            detailCard.Paint += CardPanel_Paint;
            pnlRightSave.Controls.Add(detailCard);

            lblGameSavesTitle = new Label { Text = "遊戲中存檔列表", Location = new Point(16, 15), AutoSize = true, Font = fontJhengHei105B, ForeColor = Color.FromArgb(0, 220, 255), BackColor = Color.Transparent };
            gameCard.Controls.Add(lblGameSavesTitle);
            dgvGameSaves = CreateSaveGrid(false);
            gameCard.Controls.Add(dgvGameSaves);

            btnBackupSave = new Button { Text = "備份此存檔", Size = new Size(140, 35) };
            StyleButton(btnBackupSave, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnBackupSave.Click += BtnBackupSave_Click;
            gameCard.Controls.Add(btnBackupSave);

            btnDeleteSave = new Button { Text = "刪除此存檔", Size = new Size(140, 35) };
            StyleButton(btnDeleteSave, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnDeleteSave.Click += BtnDeleteSave_Click;
            gameCard.Controls.Add(btnDeleteSave);

            btnRepairEndlessAi = new Button { Text = "修復無盡 AI", Size = new Size(200, 35) };
            StyleButton(btnRepairEndlessAi, Color.FromArgb(45, 45, 55), Color.FromArgb(255, 214, 64), Color.FromArgb(255, 214, 64));
            btnRepairEndlessAi.Click += BtnRepairEndlessAi_Click;
            gameCard.Controls.Add(btnRepairEndlessAi);

            btnRefreshSaves = new Button { Text = "重新整理", Size = new Size(140, 35) };
            StyleButton(btnRefreshSaves, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnRefreshSaves.Click += (s, e) => RefreshSavesAndBackups();
            gameCard.Controls.Add(btnRefreshSaves);

            TableLayoutPanel gameActions = new TableLayoutPanel {
                Name = "gameActions",
                ColumnCount = 4,
                RowCount = 1,
                BackColor = Color.Transparent,
                Margin = Padding.Empty,
                Padding = Padding.Empty
            };
            gameActions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 22F));
            gameActions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 22F));
            gameActions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 22F));
            gameActions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 34F));
            gameActions.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));
            gameCard.Controls.Add(gameActions);
            gameActions.Controls.Add(btnBackupSave, 0, 0);
            gameActions.Controls.Add(btnDeleteSave, 1, 0);
            gameActions.Controls.Add(btnRefreshSaves, 2, 0);
            gameActions.Controls.Add(btnRepairEndlessAi, 3, 0);
            foreach (Button button in gameActions.Controls) {
                button.Dock = DockStyle.Fill;
                button.Margin = new Padding(4, 4, 4, 4);
            }

            lblBackupsTitle = new Label { Text = "備份歷史列表", Location = new Point(16, 15), AutoSize = true, Font = fontJhengHei105B, ForeColor = Color.FromArgb(0, 220, 255), BackColor = Color.Transparent };
            backupsCard.Controls.Add(lblBackupsTitle);
            dgvBackups = CreateSaveGrid(true);
            backupsCard.Controls.Add(dgvBackups);

            btnRestoreBackup = new Button { Text = "還原此備份", Size = new Size(140, 35) };
            StyleButton(btnRestoreBackup, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnRestoreBackup.Click += BtnRestoreBackup_Click;
            backupsCard.Controls.Add(btnRestoreBackup);

            btnDeleteBackup = new Button { Text = "刪除此備份", Size = new Size(140, 35) };
            StyleButton(btnDeleteBackup, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnDeleteBackup.Click += BtnDeleteBackup_Click;
            backupsCard.Controls.Add(btnDeleteBackup);

            TableLayoutPanel backupActions = new TableLayoutPanel {
                Name = "backupActions",
                ColumnCount = 2,
                RowCount = 1,
                BackColor = Color.Transparent,
                Margin = Padding.Empty,
                Padding = Padding.Empty
            };
            backupActions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50F));
            backupActions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50F));
            backupActions.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));
            backupsCard.Controls.Add(backupActions);
            backupActions.Controls.Add(btnRestoreBackup, 0, 0);
            backupActions.Controls.Add(btnDeleteBackup, 1, 0);
            foreach (Button button in backupActions.Controls) {
                button.Dock = DockStyle.Fill;
                button.Margin = new Padding(4, 4, 4, 4);
            }

            lblDetailTitle = new Label { Text = "存檔詳細與預覽", Location = new Point(20, 20), AutoSize = true, Font = fontJhengHei105B, ForeColor = Color.FromArgb(0, 220, 255), BackColor = Color.Transparent };
            detailCard.Controls.Add(lblDetailTitle);
            picSavePreview = new PictureBox { SizeMode = PictureBoxSizeMode.Zoom, BackColor = Color.FromArgb(12, 12, 16) };
            detailCard.Controls.Add(picSavePreview);
            lblSaveDetail = new Label { ForeColor = Color.FromArgb(200, 205, 210), BackColor = Color.Transparent, Font = fontJhengHei10R };
            detailCard.Controls.Add(lblSaveDetail);

            void LayoutGameCard() {
                dgvGameSaves.Location = new Point(16, 48);
                dgvGameSaves.Size = new Size(Math.Max(0, gameCard.Width - 32), Math.Max(70, gameCard.Height - 112));
                gameActions.Location = new Point(12, Math.Max(54, gameCard.Height - 52));
                gameActions.Size = new Size(Math.Max(0, gameCard.Width - 24), 43);
            }
            gameCard.Resize += (s, e) => LayoutGameCard();

            void LayoutBackupsCard() {
                dgvBackups.Location = new Point(16, 48);
                dgvBackups.Size = new Size(Math.Max(0, backupsCard.Width - 32), Math.Max(70, backupsCard.Height - 112));
                backupActions.Location = new Point(12, Math.Max(54, backupsCard.Height - 52));
                backupActions.Size = new Size(Math.Max(0, backupsCard.Width - 24), 43);
            }
            backupsCard.Resize += (s, e) => LayoutBackupsCard();

            void LayoutDetailCard() {
                picSavePreview.Location = new Point(20, 54);
                picSavePreview.Size = new Size(Math.Max(80, detailCard.Width - 40), Math.Min(320, Math.Max(160, detailCard.Height * 2 / 5)));
                lblSaveDetail.Location = new Point(20, picSavePreview.Bottom + 16);
                lblSaveDetail.Size = new Size(Math.Max(80, detailCard.Width - 40), Math.Max(80, detailCard.Height - picSavePreview.Bottom - 34));
            }
            detailCard.Resize += (s, e) => LayoutDetailCard();

            LayoutGameCard();
            LayoutBackupsCard();
            LayoutDetailCard();
        }

        private DataGridView CreateSaveGrid(bool isBackup) {
            var dgv = new DataGridView {
                AllowUserToAddRows = false, AllowUserToDeleteRows = false, RowHeadersVisible = false,
                BackgroundColor = Color.FromArgb(10, 11, 16), ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(28, 30, 42), BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false, RowTemplate = { Height = 35 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect, MultiSelect = false, ReadOnly = true,
                AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.Fill
            };
            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 230, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 35;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;
            dgv.DefaultCellStyle.BackColor = Color.FromArgb(20, 21, 31);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(35, 37, 54);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;
            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(24, 25, 35);
            if (!isBackup) {
                dgv.Columns.Add("Folder", Loc.Get("HeaderFolder")); dgv.Columns["Folder"].FillWeight = 16;
                dgv.Columns.Add("Title", Loc.Get("HeaderSaveTitle")); dgv.Columns["Title"].FillWeight = 40;
                dgv.Columns.Add("Level", Loc.Get("HeaderLevel")); dgv.Columns["Level"].FillWeight = 18;
                dgv.Columns.Add("Time", Loc.Get("HeaderTime")); dgv.Columns["Time"].FillWeight = 26;
            } else {
                dgv.Columns.Add("File", Loc.Get("HeaderBackupFile")); dgv.Columns["File"].FillWeight = 18;
                dgv.Columns.Add("Title", Loc.Get("HeaderSaveTitle")); dgv.Columns["Title"].FillWeight = 30;
                dgv.Columns.Add("Level", Loc.Get("HeaderLevel")); dgv.Columns["Level"].FillWeight = 16;
                dgv.Columns.Add("Time", Loc.Get("HeaderBackupTime")); dgv.Columns["Time"].FillWeight = 22;
                dgv.Columns.Add("Folder", Loc.Get("HeaderOrigFolder")); dgv.Columns["Folder"].FillWeight = 14;
            }
            dgv.SelectionChanged += isBackup ? DgvBackups_SelectionChanged : DgvGameSaves_SelectionChanged;
            return dgv;
        }

        private async void RefreshSavesAndBackups() {
            if (_savesRefreshInFlight || saveBackupService == null) return;
            _savesRefreshInFlight = true;
            try {
                dgvGameSaves.Rows.Clear();
                dgvBackups.Rows.Clear();
                ReplaceSavePreview(null);
                lblSaveDetail.Text = "";

                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    return;
                }

                SaveBackupCatalog catalog = await Task.Run(() => saveBackupService.Scan(gamePath));
                foreach (GameSaveInfo save in catalog.Saves) {
                    dgvGameSaves.Rows.Add(
                        save.Folder,
                        save.Parsed ? save.Title : (Loc.Get("Unparsable")),
                        save.Parsed ? save.Level : (Loc.Get("Unknown")),
                        save.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"));
                }
                foreach (SaveBackupInfo backup in catalog.Backups) {
                    dgvBackups.Rows.Add(
                        backup.FileName,
                        backup.Parsed ? backup.Title : (Loc.Get("Unparsable")),
                        backup.Parsed ? backup.Level : (Loc.Get("Unknown")),
                        backup.BackupTime,
                        string.IsNullOrEmpty(backup.OrigFolder) ? (Loc.Get("Unknown")) : backup.OrigFolder);
                }
            } catch (Exception ex) {
                // 過去這裡完全靜默：掃描失敗時清單留空、使用者看不出原因。
                // 至少把錯誤顯示在詳細資訊區並留下診斷線索。
                System.Diagnostics.Debug.WriteLine("掃描存檔與備份失敗: " + ex);
                lblSaveDetail.Text = ex.Message;
            } finally {
                _savesRefreshInFlight = false;
            }
        }

        private void DgvGameSaves_SelectionChanged(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) return;
            try {
                DataGridViewRow row = dgvGameSaves.SelectedRows[0];
                string folder = Cell(row, 0);
                lblSaveDetail.Text = string.Format(Loc.Get("SaveDetailGameSave"),
                    folder, Cell(row, 1), Cell(row, 2), Cell(row, 3));
                string gamePath = GetGamePath();
                ReplaceSavePreview(string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)
                    ? null
                    : saveBackupService.ReadSavePreview(gamePath, folder));
            } catch (Exception ex) {
                System.Diagnostics.Debug.WriteLine("更新存檔選取預覽失敗: " + ex.Message);
            }
        }

        private void DgvBackups_SelectionChanged(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) return;
            try {
                DataGridViewRow row = dgvBackups.SelectedRows[0];
                string file = Cell(row, 0);
                lblSaveDetail.Text = string.Format(Loc.Get("SaveDetailBackup"),
                    file, Cell(row, 4), Cell(row, 1), Cell(row, 2), Cell(row, 3));
                ReplaceSavePreview(saveBackupService.ReadBackupPreview(file));
            } catch (Exception ex) {
                System.Diagnostics.Debug.WriteLine("更新備份選取預覽失敗: " + ex.Message);
            }
        }

        private void BtnBackupSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectSaveToBackup"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                DataGridViewRow row = dgvGameSaves.SelectedRows[0];
                string folder = Cell(row, 0).Trim();
                if (!SaveBackupService.IsSimpleName(folder)) {
                    MessageBox.Show(Loc.Get("MsgInvalidSaveDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                saveBackupService.CreateBackup(GetGamePath(), folder, Cell(row, 1), Cell(row, 2));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgBackupSaveSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (DirectoryNotFoundException) {
                MessageBox.Show(Loc.Get("MsgNoOrigFolderToBackup"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } catch (Exception ex) {
                MessageBox.Show((Loc.Get("MsgBackupSaveFailed")) + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void BtnRestoreBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectBackup"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                DataGridViewRow row = dgvBackups.SelectedRows[0];
                string file = Cell(row, 0);
                string folder = Cell(row, 4);
                if (!SaveBackupService.IsSimpleName(file)) return;
                if (!SaveBackupService.IsSimpleName(folder) || folder == (Loc.Get("Unknown"))) return;
                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    MessageBox.Show(Loc.Get("MsgGamePathNotSet"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                if (Directory.Exists(Path.Combine(gamePath, "SAVE", folder)) &&
                    MessageBox.Show(string.Format(Loc.Get("MsgConfirmOverwriteSave"), folder), Loc.Get("TitleWarning"),
                        MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;

                saveBackupService.RestoreBackup(gamePath, file, folder, msg => System.Diagnostics.Debug.WriteLine(msg));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgRestoreBackupSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                MessageBox.Show((Loc.Get("MsgRestoreBackupFailed")) + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private async void BtnRepairEndlessAi_Click(object? sender, EventArgs e) {
            if (_repairInFlight) return;
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectSaveToRepairAi"),
                    Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }

            DataGridViewRow row = dgvGameSaves.SelectedRows[0];
            string folder = Cell(row, 0).Trim();
            string title = Cell(row, 1);
                string level = Cell(row, 2);
                if (!SaveBackupService.IsSimpleName(folder)) return;
                if (MessageBox.Show(Loc.Get("MsgConfirmRepairEndlessAi"),
                    Loc.Get("TitleWarning"), MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes)
                return;

            _repairInFlight = true;
            btnRepairEndlessAi.Enabled = false;
            try {
                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath))
                    throw new DirectoryNotFoundException(Loc.Get("MsgGamePathNotSet"));

                EndlessSaveAiRepairResult result = await Task.Run(() => {
                    saveBackupService.CreateBackup(gamePath, folder, title, level);
                    return new EndlessSaveAiRepairService().Repair(gamePath, folder);
                });
                RefreshSavesAndBackups();
                string message = result == EndlessSaveAiRepairResult.Changed
                    ? (Loc.Get("MsgRepairEndlessAiSuccess"))
                    : (Loc.Get("MsgRepairEndlessAiAlready"));
                MessageBox.Show(message, Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                MessageBox.Show((Loc.Get("MsgRepairEndlessAiFailed")) + ex.Message,
                    Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                _repairInFlight = false;
                btnRepairEndlessAi.Enabled = true;
            }
        }

        private void BtnDeleteSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) return;
            try {
                string folder = Cell(dgvGameSaves.SelectedRows[0], 0).Trim();
                if (!SaveBackupService.IsSimpleName(folder)) return;
                if (MessageBox.Show(string.Format(Loc.Get("MsgConfirmDeleteSave"), folder), Loc.Get("TitleConfirm"),
                        MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
                saveBackupService.DeleteSave(GetGamePath(), folder);
                RefreshSavesAndBackups();
            } catch (Exception ex) {
                MessageBox.Show((Loc.Get("MsgDeleteSaveFailed")) + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void BtnDeleteBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) return;
            try {
                string file = Cell(dgvBackups.SelectedRows[0], 0);
                if (!SaveBackupService.IsSimpleName(file)) return;
                if (MessageBox.Show(string.Format(Loc.Get("MsgConfirmDeleteBackup"), file), Loc.Get("TitleConfirm"),
                        MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
                saveBackupService.DeleteBackup(file);
                RefreshSavesAndBackups();
            } catch (Exception ex) {
                MessageBox.Show((Loc.Get("MsgDeleteBackupFailed")) + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void ReplaceSavePreview(byte[]? bytes) {
            picSavePreview.Image?.Dispose();
            picSavePreview.Image = bytes == null ? null : LoadTga(bytes);
        }

        public static Bitmap? LoadTga(byte[] tgaBytes) {
            // 解析下沉至 Core.TgaDecoder（無 System.Drawing 依賴）；此處僅把緊密排列的
            // BGRA 緩衝包成 32bpp ARGB Bitmap。ModifierForm 亦有相同薄殼。
            TgaImage? image = TgaDecoder.Decode(tgaBytes);
            if (image == null) return null;
            var bmp = new Bitmap(image.Width, image.Height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            var bmpData = bmp.LockBits(new Rectangle(0, 0, image.Width, image.Height),
                System.Drawing.Imaging.ImageLockMode.WriteOnly, bmp.PixelFormat);
            try {
                int rowBytes = image.Width * 4;
                if (bmpData.Stride == rowBytes) {
                    System.Runtime.InteropServices.Marshal.Copy(image.Bgra, 0, bmpData.Scan0, image.Bgra.Length);
                } else {
                    for (int y = 0; y < image.Height; y++)
                        System.Runtime.InteropServices.Marshal.Copy(
                            image.Bgra, y * rowBytes, bmpData.Scan0 + y * bmpData.Stride, rowBytes);
                }
            } finally {
                bmp.UnlockBits(bmpData);
            }
            return bmp;
        }

        private static string Cell(DataGridViewRow row, int index) => row.Cells[index].Value?.ToString() ?? "";
        
        private GraphicsPath GetRoundPath(Rectangle r, int radius) {
            GraphicsPath path = new GraphicsPath();
            int d = radius * 2;
            path.AddArc(r.X, r.Y, d, d, 180, 90);
            path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
            path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
            path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
            path.CloseFigure();
            return path;
        }

        private void CardPanel_Paint(object? sender, PaintEventArgs e) {
            Panel pnl = (Panel)sender!;
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using (SolidBrush sb = new SolidBrush(Color.FromArgb(18, 22, 31))) {
                using (GraphicsPath path = GetRoundPath(new Rectangle(0, 0, pnl.Width, pnl.Height), 10)) {
                    e.Graphics.FillPath(sb, path);
                    using (Pen p = new Pen(Color.FromArgb(45, 53, 69), 1)) {
                        e.Graphics.DrawPath(p, path);
                    }
                }
            }
        }

        private void TitleBar_MouseDown(object? sender, MouseEventArgs e) {
            if (e.Button == MouseButtons.Left) { dragging = true; dragStart = new Point(e.X, e.Y); }
        }
        private void TitleBar_MouseMove(object? sender, MouseEventArgs e) {
            if (dragging) { Point p = PointToScreen(e.Location); Location = new Point(p.X - dragStart.X, p.Y - dragStart.Y); }
        }
        private void TitleBar_MouseUp(object? sender, MouseEventArgs e) { dragging = false; }

        private void StyleButton(Button btn, Color backColor, Color foreColor, Color hoverBorderColor) {
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 0;
            btn.BackColor = Color.Transparent;
            btn.ForeColor = foreColor;
            btn.Cursor = Cursors.Hand;
            btn.Font = fontJhengHei95B;

            bool isHovered = false;
            btn.MouseEnter += (s, e) => { isHovered = true; btn.Invalidate(); };
            btn.MouseLeave += (s, e) => { isHovered = false; btn.Invalidate(); };

            btn.Paint += (s, e) => {
                Graphics g = e.Graphics;
                g.SmoothingMode = SmoothingMode.AntiAlias;

                Rectangle rect = new Rectangle(0, 0, btn.Width, btn.Height);
                int radius = 6;

                using (GraphicsPath path = GetRoundPath(rect, radius)) {
                    Color startColor, endColor;
                    if (backColor == Color.FromArgb(98, 0, 238)) {
                        startColor = isHovered ? Color.FromArgb(52, 151, 255) : Color.FromArgb(34, 124, 246);
                        endColor = isHovered ? Color.FromArgb(66, 199, 255) : Color.FromArgb(43, 160, 255);
                    } else {
                        startColor = isHovered ? Color.FromArgb(47, 55, 70) : Color.FromArgb(31, 37, 49);
                        endColor = isHovered ? Color.FromArgb(56, 66, 84) : Color.FromArgb(38, 45, 59);
                    }

                    using (LinearGradientBrush brush = new LinearGradientBrush(rect, startColor, endColor, 45F)) {
                        g.FillPath(brush, path);
                    }

                    Color borderColor = isHovered ? hoverBorderColor : Color.FromArgb(50, 52, 70);
                    using (Pen p = new Pen(borderColor, 1.2F)) {
                        g.DrawPath(p, path);
                    }

                    if (!string.IsNullOrEmpty(btn.Text)) {
                        TextRenderer.DrawText(g, btn.Text, btn.Font, rect, foreColor,
                            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.WordBreak);
                    }
                }
            };
        }

        private void UpdateLanguageButtonStyles() {
            bool isZh = Loc.CurrentLanguage == Language.TraditionalChinese;

            btnLangZH.BackColor = isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangZH.ForeColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangZH.FlatAppearance.BorderColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);

            btnLangEN.BackColor = !isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangEN.ForeColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangEN.FlatAppearance.BorderColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);
        }

        private void ApplyLanguageToUI() {
            bool isEn = Loc.CurrentLanguage == Language.English;
            this.Text = isEn ? "Against Rome Modifier - Save Manager" : "Against Rome Modifier - 存檔管理器";
            lblMainTitle.Text = isEn ? "AGAINST ROME SAVE MANAGER" : "AGAINST ROME 存檔管理器";
            lblGamePath.Text = isEn ? "Game Path:" : "遊戲目錄：";
            btnBrowseGamePath.Text = isEn ? "Browse..." : "瀏覽...";
            lblGameSavesTitle.Text = isEn ? "In-Game Save List" : "遊戲中存檔列表";
            lblBackupsTitle.Text = isEn ? "Backup History List" : "備份歷史列表";
            lblDetailTitle.Text = isEn ? "Save Details & Preview" : "存檔詳細與預覽";
            btnBackupSave.Text = isEn ? "Backup Save" : "備份此存檔";
            btnRepairEndlessAi.Text = Loc.Get("BtnRepairEndlessAi");
            btnDeleteSave.Text = isEn ? "Delete Save" : "刪除此存檔";
            btnRefreshSaves.Text = isEn ? "Refresh" : "重新整理";
            btnRestoreBackup.Text = isEn ? "Restore Backup" : "還原此備份";
            btnDeleteBackup.Text = isEn ? "Delete Backup" : "刪除此備份";

            // Update column headers
            if (dgvGameSaves != null && dgvGameSaves.Columns.Count >= 4) {
                dgvGameSaves.Columns["Folder"].HeaderText = Loc.Get("HeaderFolder");
                dgvGameSaves.Columns["Title"].HeaderText = Loc.Get("HeaderSaveTitle");
                dgvGameSaves.Columns["Level"].HeaderText = Loc.Get("HeaderLevel");
                dgvGameSaves.Columns["Time"].HeaderText = Loc.Get("HeaderTime");
            }
            if (dgvBackups != null && dgvBackups.Columns.Count >= 5) {
                dgvBackups.Columns["File"].HeaderText = Loc.Get("HeaderBackupFile");
                dgvBackups.Columns["Title"].HeaderText = Loc.Get("HeaderSaveTitle");
                dgvBackups.Columns["Level"].HeaderText = Loc.Get("HeaderLevel");
                dgvBackups.Columns["Time"].HeaderText = Loc.Get("HeaderBackupTime");
                dgvBackups.Columns["Folder"].HeaderText = Loc.Get("HeaderOrigFolder");
            }

            // Refresh the grids to update localized values in rows (like "Unparsable", "Unknown", etc.)
            RefreshSavesAndBackups();
        }

        protected override void Dispose(bool disposing) {
            if (disposing) {
                // 預覽圖是本表單自行解碼的 Bitmap（PictureBox 不會代為釋放），
                // 不在這裡放掉就得等 finalizer 才會還回 GDI 資源。
                Image? preview = picSavePreview?.Image;
                if (picSavePreview != null) picSavePreview.Image = null;
                preview?.Dispose();
                fontJhengHei115B.Dispose();
                fontJhengHei95B.Dispose();
                fontJhengHei9R.Dispose();
                fontJhengHei10R.Dispose();
                fontJhengHei105B.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
