using System;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace AgainstRomeModifier {
    // 修改器的主表單類別，繼承自 Windows Form
    public partial class ModifierForm : Form {
        // 導入 Gdi32.dll 的 Win32 API，用於在 Windows 10/11 下為視窗建立圓角區域
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        // 標題列與控制按鈕
        private Panel pnlTitleBar = null!;
        private Label lblMainTitle = null!;
        private Button btnClose = null!;
        private Button btnMinimize = null!;

        // 左側導覽列按鈕
        private Panel pnlSidebar = null!;
        private Button btnNavSystem = null!;
        private Button btnNavDefaultStats = null!;
        private Button btnNavCurrentStats = null!;
        private Button btnNavDoc = null!;


        // 主要分頁控制項與分頁
        private TabControl mainTabControl = null!;
        private TabPage tabSystem = null!;
        private TabPage tabDefaultStats = null!;
        private TabPage tabCurrentStats = null!;
        private TabPage tabDoc = null!;
        private TextBox txtDoc = null!;
        private TabPage tabSaveManager = null!;
        private Button btnNavSaveManager = null!;
        
        // 存檔管理介面表格與預覽圖
        private DataGridView dgvGameSaves = null!;
        private DataGridView dgvBackups = null!;
        private PictureBox picSavePreview = null!;
        private Label lblSaveDetail = null!;
        
        // 兵種屬性分頁與網格
        private TabControl defaultStatsTabControl = null!;
        private Dictionary<string, DataGridView> defaultStatsGrids = new Dictionary<string, DataGridView>();
        private TabControl currentStatsTabControl = null!;
        private Dictionary<string, DataGridView> currentStatsGrids = new Dictionary<string, DataGridView>();

        // 介面上的卡片式群組容器
        private Panel pnlNumericCard = null!;
        private Panel pnlSwitchesCard = null!;
        private Panel pnlConsoleCard = null!;

        // 數值控制項 (NumericUpDown) 的宣告
        private Label lblPopLimit = null!;
        private NumericUpDown numPopLimit = null!;
        private Label lblCiviSpeed = null!;
        private NumericUpDown numCiviSpeed = null!;

        // 功能開關的核取方塊 (自訂 ModernToggle 開關)
        private ModernToggle chkFreeProd = null!;
        private ModernToggle chkFreeUpgrade = null!;
        private ModernToggle chkNoSpellCost = null!;
        private ModernToggle chkFocusLoss = null!;
        private ModernToggle chkBalance = null!;
        private ModernToggle chkToEng = null!;
        private ModernToggle chkInfiniteMorale = null!;

        // 匯出/匯入設定檔按鈕
        private Button btnPresetSave = null!;
        private Button btnPresetLoad = null!;

        // 兵種圖示快取字典
        private Dictionary<string, Bitmap> unitIcons = new Dictionary<string, Bitmap>();
        
        // 控制台與系統按鈕
        private Label lblGamePath = null!;
        private TextBox txtGamePath = null!;
        private Button btnBrowseGamePath = null!;
        private Button btnApply = null!;
        private Button btnRestore = null!;
        private Button btnLoadCurrent = null!;
        private Button btnStartGame = null!;
        private ContextMenuStrip menuRestore = null!;
        private TextBox txtLog = null!;
        
        // 記憶體原版檔案備份字典，用以在修改時直接讀取乾淨數據，避免疊加修改
        private Dictionary<string, byte[]> backupFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

        // 快取的備份單兵屬性欄位字典 (以兵種名稱為 Key)
        private Dictionary<string, string[]> _backupUnitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
        private bool _backupUnitRowsParsed = false;

        // 備份存檔快取
        private class BackupSaveCache {
            public string FileName { get; set; } = "";
            public string Title { get; set; } = "";
            public string Level { get; set; } = "";
            public string OrigFolder { get; set; } = "";
            public string BackupTimeStr { get; set; } = "";
            public DateTime LastWriteTime { get; set; }
        }
        private Dictionary<string, BackupSaveCache> _backupSaveCache = new Dictionary<string, BackupSaveCache>(StringComparer.OrdinalIgnoreCase);
        
        // 視窗拖曳狀態變數
        private bool dragging = false;
        private Point dragStart = new Point(0, 0);
        
        // 統一風格的字型物件宣告
        private Font fontJhengHei95B = new Font("Microsoft JhengHei", 9.5F, FontStyle.Bold);
        private Font fontJhengHei95R = new Font("Microsoft JhengHei", 9.5F, FontStyle.Regular);
        private Font fontJhengHei115B = new Font("Microsoft JhengHei", 11.5F, FontStyle.Bold);
        private Font fontJhengHei105B = new Font("Microsoft JhengHei", 10.5F, FontStyle.Bold);
        private Font fontJhengHei105R = new Font("Microsoft JhengHei", 10.5F, FontStyle.Regular);
        private Font fontJhengHei10B = new Font("Microsoft JhengHei", 10F, FontStyle.Bold);
        private Font fontJhengHei9R = new Font("Microsoft JhengHei", 9F, FontStyle.Regular);
        private Font fontJhengHei10R = new Font("Microsoft JhengHei", 10F, FontStyle.Regular);
        private Font fontConsolas95 = new Font("Consolas", 9.5F);
        private Font fontConsolas85 = new Font("Consolas", 8.5F, FontStyle.Regular);
        
        // 統一風格的按鈕基礎顏色
        private static readonly Color ColorBtnDefault = Color.FromArgb(45, 45, 55);
        private static readonly Color ColorBtnPrimary = Color.FromArgb(98, 0, 238);

        // 建構函式：初始化 UI 元件，載入備份檔並初始化現有設定
        public ModifierForm() {
            InitializeComponent();
            Log("修改器視窗建構完成，開始載入資料...");
            // 將內嵌的 Backup.zip 載入記憶體
            LoadBackupZipToMemory();
            // 初始化資料與讀取預設兵種資訊
            InitializeData();
            // 註冊表單關閉事件以正確釋放字型與圖形物件資源，防止記憶體洩漏
            this.FormClosing += (s, e) => {
                try {
                    foreach (var img in unitIcons.Values) {
                        if (img != null) img.Dispose();
                    }
                    unitIcons.Clear();
                    fontJhengHei95B.Dispose();
                    fontJhengHei95R.Dispose();
                    fontJhengHei115B.Dispose();
                    fontJhengHei105B.Dispose();
                    fontJhengHei105R.Dispose();
                    fontJhengHei10B.Dispose();
                    fontJhengHei9R.Dispose();
                    fontJhengHei10R.Dispose();
                    fontConsolas95.Dispose();
                    fontConsolas85.Dispose();
                } catch (Exception ex) {
                    Log("釋放資源失敗: " + ex.Message);
                }
            };
        }

        // 產生帶有圓角矩形的 GraphicsPath 物件，用於 UI 的圓角卡片與視窗繪製
        private GraphicsPath GetRoundPath(Rectangle r, int radius) {
            GraphicsPath path = new GraphicsPath();
            int d = radius * 2;
            path.AddArc(r.X, r.Y, d, d, 180, 90); // 左上角
            path.AddArc(r.Right - d, r.Y, d, d, 270, 90); // 右上角
            path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90); // 右下角
            path.AddArc(r.X, r.Bottom - d, d, d, 90, 90); // 左下角
            path.CloseFigure();
            return path;
        }

        // 卡片容器 Panel 的 Paint 事件處理程序，以 GDI+ 繪製深色圓角卡片背景與亮色邊框
        private void CardPanel_Paint(object? sender, PaintEventArgs e) {
            Panel pnl = (Panel)sender!;
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias; // 啟用抗鋸齒
            using (SolidBrush sb = new SolidBrush(Color.FromArgb(28, 28, 35))) {
                using (GraphicsPath path = GetRoundPath(new Rectangle(0, 0, pnl.Width, pnl.Height), 10)) {
                    e.Graphics.FillPath(sb, path); // 填滿背景色
                    using (Pen p = new Pen(Color.FromArgb(45, 45, 55), 1)) {
                        e.Graphics.DrawPath(p, path); // 繪製卡片邊框
                    }
                }
            }
        }

        // 輸入欄位外框 Panel 的 Paint 事件處理程序，繪製扁平風格的欄位外框
        private void InputPanel_Paint(object? sender, PaintEventArgs e) {
            Panel pnl = (Panel)sender!;
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using (Pen p = new Pen(Color.FromArgb(55, 55, 65), 1)) {
                e.Graphics.DrawRectangle(p, 0, 0, pnl.Width - 1, pnl.Height - 1);
            }
        }

        // 統一設定 Button 控制項的扁平化樣式、背景顏色、滑鼠懸停 (Hover) 微動畫與邊框發光效果
        private void StyleButton(Button btn, Color backColor, Color foreColor, Color hoverBorderColor) {
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 1;
            btn.FlatAppearance.BorderColor = Color.FromArgb(60, 60, 70);
            btn.BackColor = backColor;
            btn.ForeColor = foreColor;
            btn.Cursor = Cursors.Hand;
            btn.Font = fontJhengHei95B;
            // 註冊滑鼠移入事件，套用高亮懸停特效與霓虹邊框發光
            btn.MouseEnter += (s, e) => {
                btn.FlatAppearance.BorderColor = hoverBorderColor;
                if (backColor == ColorBtnDefault) {
                    btn.BackColor = Color.FromArgb(55, 55, 68);
                } else if (backColor == ColorBtnPrimary) {
                    btn.BackColor = Color.FromArgb(120, 40, 255);
                } else {
                    btn.BackColor = Color.FromArgb(
                        Math.Min(255, backColor.R + 20),
                        Math.Min(255, backColor.G + 20),
                        Math.Min(255, backColor.B + 20));
                }
            };
            // 註冊滑鼠移出事件，恢復原初按鈕狀態
            btn.MouseLeave += (s, e) => {
                btn.FlatAppearance.BorderColor = Color.FromArgb(60, 60, 70);
                btn.BackColor = backColor;
            };
        }

        // 統一設定 CheckBox 的扁平化與自訂色彩樣式，選取時文字顯示霓虹青色
        private void StyleCheckBox(CheckBox chk) {
            chk.FlatStyle = FlatStyle.Flat;
            chk.FlatAppearance.BorderColor = Color.FromArgb(60, 60, 70);
            chk.ForeColor = Color.FromArgb(200, 205, 210);
            chk.Cursor = Cursors.Hand;
            chk.CheckedChanged += (s, e) => {
                if (chk.Checked) {
                    chk.ForeColor = Color.FromArgb(0, 220, 255); // 已選取：青色
                } else {
                    chk.ForeColor = Color.FromArgb(200, 205, 210); // 未選取：灰色
                }
            };
        }

        // 切換主分頁 Page 的顯示，若分頁不存在則加入 TabPages 中並選取之
        private void ShowTabPage(TabPage page) {
            if (!mainTabControl.TabPages.Contains(page))
                mainTabControl.TabPages.Add(page);
            mainTabControl.SelectedTab = page;
        }

        // 自訂標題列滑鼠按下事件，啟用視窗拖曳狀態並記錄起點
        private void TitleBar_MouseDown(object? sender, MouseEventArgs e) {
            if (e.Button == MouseButtons.Left) {
                dragging = true;
                dragStart = new Point(e.X, e.Y);
            }
        }

        // 自訂標題列滑鼠移動事件，移動視窗位置至當前拖曳座標
        private void TitleBar_MouseMove(object? sender, MouseEventArgs e) {
            if (dragging) {
                Point p = PointToScreen(e.Location);
                Location = new Point(p.X - dragStart.X, p.Y - dragStart.Y);
            }
        }

        // 自訂標題列滑鼠放開事件，解除拖曳狀態
        private void TitleBar_MouseUp(object? sender, MouseEventArgs e) {
            dragging = false;
        }

        // 初始化表單的視覺元件佈局、大小、樣式、圓角區域與雙緩衝
        private void InitializeComponent() {
            this.Size = new Size(1840, 880);
            this.FormBorderStyle = FormBorderStyle.None; // 隱藏 Windows 預設視窗邊框
            this.StartPosition = FormStartPosition.CenterScreen; // 視窗預設居中
            this.BackColor = Color.FromArgb(16, 16, 20); // 深色科技感背景
            this.ForeColor = Color.FromArgb(230, 235, 240);
            this.Font = fontJhengHei95R;
            this.DoubleBuffered = true; // 啟用雙緩衝防止繪圖閃爍

            // 表單 Load 事件：使用 Win32 API 建立圓角裁剪區域
            this.Load += (s, e) => {
                this.Region = Region.FromHrgn(CreateRoundRectRgn(0, 0, Width, Height, 15, 15));
            };

            // 表單 Paint 事件：動態繪製霓虹青色外框線
            this.Paint += (s, e) => {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                using (Pen p = new Pen(Color.FromArgb(0, 220, 255), 2)) {
                    using (GraphicsPath path = GetRoundPath(new Rectangle(0, 0, this.Width, this.Height), 15)) {
                        e.Graphics.DrawPath(p, path);
                    }
                }
            };

            pnlTitleBar = new Panel {
                Location = new Point(0, 0),
                Size = new Size(1840, 50),
                BackColor = Color.FromArgb(24, 24, 30)
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblMainTitle = new Label {
                Text = "AGAINST ROME MODIFIER PRO",
                Location = new Point(20, 14),
                Size = new Size(300, 25),
                Font = fontJhengHei115B,
                ForeColor = Color.FromArgb(0, 220, 255)
            };
            lblMainTitle.MouseDown += TitleBar_MouseDown;
            lblMainTitle.MouseMove += TitleBar_MouseMove;
            lblMainTitle.MouseUp += TitleBar_MouseUp;

            Label lblVersion = new Label {
                Text = "v19.1",
                Location = new Point(310, 18),
                Size = new Size(50, 18),
                Font = fontConsolas85,
                ForeColor = Color.FromArgb(100, 110, 120)
            };
            lblVersion.MouseDown += TitleBar_MouseDown;
            lblVersion.MouseMove += TitleBar_MouseMove;
            lblVersion.MouseUp += TitleBar_MouseUp;

            btnClose = new Button {
                Text = "×",
                Location = new Point(1800, 10),
                Size = new Size(30, 30),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.Transparent,
                ForeColor = Color.White
            };
            btnClose.FlatAppearance.BorderSize = 0;
            btnClose.Cursor = Cursors.Hand;
            btnClose.Click += (s, e) => Application.Exit();
            btnClose.MouseEnter += (s, e) => {
                btnClose.BackColor = Color.FromArgb(232, 17, 35);
                btnClose.ForeColor = Color.White;
            };
            btnClose.MouseLeave += (s, e) => {
                btnClose.BackColor = Color.Transparent;
                btnClose.ForeColor = Color.White;
            };

            btnMinimize = new Button {
                Text = "—",
                Location = new Point(1760, 10),
                Size = new Size(30, 30),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.Transparent,
                ForeColor = Color.White
            };
            btnMinimize.FlatAppearance.BorderSize = 0;
            btnMinimize.Cursor = Cursors.Hand;
            btnMinimize.Click += (s, e) => this.WindowState = FormWindowState.Minimized;
            btnMinimize.MouseEnter += (s, e) => {
                btnMinimize.BackColor = Color.FromArgb(45, 45, 55);
            };
            btnMinimize.MouseLeave += (s, e) => {
                btnMinimize.BackColor = Color.Transparent;
            };

            pnlTitleBar.Controls.Add(lblMainTitle);
            pnlTitleBar.Controls.Add(lblVersion);
            pnlTitleBar.Controls.Add(btnClose);
            pnlTitleBar.Controls.Add(btnMinimize);

            pnlSidebar = new Panel {
                Location = new Point(0, 50),
                Size = new Size(220, 830),
                BackColor = Color.FromArgb(20, 20, 26)
            };

            // 改為按鈕自繪指示條，pnlActiveIndicator 不再需要
            btnNavSystem = new Button { Location = new Point(10, 30) };
            StyleNavButton(btnNavSystem, "⚙   主控制台", tabSystem);
            btnNavSystem.Click += (s, e) => {
                ShowTabPage(tabSystem);
                RefreshNavButtons();
            };

            btnNavDefaultStats = new Button { Location = new Point(10, 85) };
            StyleNavButton(btnNavDefaultStats, "📊   預設兵種屬性", tabDefaultStats);
            btnNavDefaultStats.Click += (s, e) => {
                ShowTabPage(tabDefaultStats);
                RefreshNavButtons();
            };

            btnNavCurrentStats = new Button { Location = new Point(10, 140) };
            StyleNavButton(btnNavCurrentStats, "📈   當前兵種數值", tabCurrentStats);
            btnNavCurrentStats.Click += (s, e) => {
                ShowTabPage(tabCurrentStats);
                RefreshNavButtons();
            };

            btnNavSaveManager = new Button { Location = new Point(10, 195) };
            StyleNavButton(btnNavSaveManager, "💾   遊戲存檔管理", tabSaveManager);
            btnNavSaveManager.Click += (s, e) => {
                ShowTabPage(tabSaveManager);
                RefreshNavButtons();
                RefreshSavesAndBackups();
            };

            btnNavDoc = new Button { Location = new Point(10, 250) };
            StyleNavButton(btnNavDoc, "📝   修改技術文件", tabDoc);
            btnNavDoc.Click += (s, e) => {
                ShowTabPage(tabDoc);
                RefreshNavButtons();
            };

            pnlSidebar.Controls.Add(btnNavSystem);
            pnlSidebar.Controls.Add(btnNavDefaultStats);
            pnlSidebar.Controls.Add(btnNavCurrentStats);
            pnlSidebar.Controls.Add(btnNavSaveManager);
            pnlSidebar.Controls.Add(btnNavDoc);

            mainTabControl = new TabControl {
                Location = new Point(230, 60),
                Size = new Size(1200, 810),
                SizeMode = TabSizeMode.Fixed,
                ItemSize = new Size(0, 1)
            };

            tabSystem = new TabPage {
                BackColor = Color.FromArgb(16, 16, 20),
                UseVisualStyleBackColor = false
            };

            tabDefaultStats = new TabPage {
                BackColor = Color.FromArgb(16, 16, 20),
                UseVisualStyleBackColor = false
            };

            tabCurrentStats = new TabPage {
                BackColor = Color.FromArgb(16, 16, 20),
                UseVisualStyleBackColor = false
            };

            mainTabControl.TabPages.Add(tabSystem);
            mainTabControl.TabPages.Add(tabDefaultStats);
            mainTabControl.TabPages.Add(tabCurrentStats);

            pnlNumericCard = new Panel {
                Location = new Point(0, 0),
                Size = new Size(585, 380)
            };
            pnlNumericCard.Paint += CardPanel_Paint;

            Label lblNumericTitle = new Label {
                Text = "數值偏好與系統設定",
                Location = new Point(30, 20),
                Size = new Size(250, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlNumericCard.Controls.Add(lblNumericTitle);

            lblPopLimit = new Label {
                Text = "最大人口上限:",
                Location = new Point(30, 60),
                Size = new Size(250, 25),
                ForeColor = Color.FromArgb(200, 205, 210),
                BackColor = Color.Transparent
            };
            Panel pnlPop = CreateInputWrapper(300, 58, 230, 28);
            numPopLimit = new NumericUpDown {
                Location = new Point(5, 5),
                Size = new Size(220, 20),
                Minimum = 1,
                Maximum = 10000,
                Value = 200,
                BorderStyle = BorderStyle.None,
                BackColor = Color.FromArgb(38, 38, 48),
                ForeColor = Color.White
            };
            pnlPop.Controls.Add(numPopLimit);
            pnlNumericCard.Controls.Add(lblPopLimit);
            pnlNumericCard.Controls.Add(pnlPop);

            lblCiviSpeed = new Label {
                Text = "村民生產速度倍率:",
                Location = new Point(30, 110),
                Size = new Size(250, 25),
                ForeColor = Color.FromArgb(200, 205, 210),
                BackColor = Color.Transparent
            };
            Panel pnlCivi = CreateInputWrapper(300, 108, 230, 28);
            numCiviSpeed = new NumericUpDown {
                Location = new Point(5, 5),
                Size = new Size(220, 20),
                Minimum = 1.0M,
                Maximum = 50.0M,
                DecimalPlaces = 1,
                Increment = 1.0M,
                Value = 1.0M,
                BorderStyle = BorderStyle.None,
                BackColor = Color.FromArgb(38, 38, 48),
                ForeColor = Color.White
            };
            pnlCivi.Controls.Add(numCiviSpeed);
            pnlNumericCard.Controls.Add(lblCiviSpeed);
            pnlNumericCard.Controls.Add(pnlCivi);

            chkFocusLoss = new ModernToggle {
                Text = "遊戲視窗失焦時不自動暫停 (背景執行)",
                Location = new Point(30, 170),
                Size = new Size(500, 25),
                Checked = false,
                BackColor = Color.Transparent
            };
            pnlNumericCard.Controls.Add(chkFocusLoss);

            chkToEng = new ModernToggle {
                Text = "強制英文語系 (介面圖示與核心文字)",
                Location = new Point(30, 210),
                Size = new Size(500, 25),
                Checked = true,
                BackColor = Color.Transparent
            };
            pnlNumericCard.Controls.Add(chkToEng);

            btnPresetSave = new Button {
                Text = "匯出設定",
                Location = new Point(30, 280),
                Size = new Size(130, 38)
            };
            StyleButton(btnPresetSave, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnPresetSave.Click += new EventHandler(BtnPresetSave_Click);

            btnPresetLoad = new Button {
                Text = "匯入設定",
                Location = new Point(180, 280),
                Size = new Size(130, 38)
            };
            StyleButton(btnPresetLoad, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(180, 100, 255));
            btnPresetLoad.Click += new EventHandler(BtnPresetLoad_Click);
            pnlNumericCard.Controls.Add(btnPresetSave);
            pnlNumericCard.Controls.Add(btnPresetLoad);

            pnlSwitchesCard = new Panel {
                Location = new Point(605, 0),
                Size = new Size(585, 380)
            };
            pnlSwitchesCard.Paint += CardPanel_Paint;

            Label lblSwitchesTitle = new Label {
                Text = "核心修改開關設定",
                Location = new Point(30, 20),
                Size = new Size(250, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlSwitchesCard.Controls.Add(lblSwitchesTitle);

            chkFreeProd = new ModernToggle {
                Text = "建造、修復與所有單位生產完全免費",
                Location = new Point(30, 65),
                Size = new Size(500, 25),
                Checked = false,
                BackColor = Color.Transparent
            };
            pnlSwitchesCard.Controls.Add(chkFreeProd);

            chkFreeUpgrade = new ModernToggle {
                Text = "陣型、研發、屬性解鎖升級免費",
                Location = new Point(30, 115),
                Size = new Size(500, 25),
                Checked = false,
                BackColor = Color.Transparent
            };
            pnlSwitchesCard.Controls.Add(chkFreeUpgrade);

            chkNoSpellCost = new ModernToggle {
                Text = "祭司與賢者法術無消耗 (MP 零消耗)",
                Location = new Point(30, 165),
                Size = new Size(500, 25),
                Checked = false,
                BackColor = Color.Transparent
            };
            pnlSwitchesCard.Controls.Add(chkNoSpellCost);

            chkInfiniteMorale = new ModernToggle {
                Text = "部隊無限士氣 (士氣不減且極速恢復)",
                Location = new Point(30, 215),
                Size = new Size(500, 25),
                Checked = false,
                BackColor = Color.Transparent
            };
            pnlSwitchesCard.Controls.Add(chkInfiniteMorale);

            // 新增下方使用指南卡片，使佈局平衡且資訊更完整
            Panel pnlTipsCard = new Panel {
                Location = new Point(0, 400),
                Size = new Size(1190, 380)
            };
            pnlTipsCard.Paint += CardPanel_Paint;

            Label lblTipsTitle = new Label {
                Text = "修改器使用指引與操作指南",
                Location = new Point(30, 20),
                Size = new Size(300, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlTipsCard.Controls.Add(lblTipsTitle);

            Label lblTipsContent = new Label {
                Location = new Point(30, 65),
                Size = new Size(1130, 280),
                Font = fontJhengHei105R,
                ForeColor = Color.FromArgb(180, 185, 195),
                BackColor = Color.Transparent,
                Text = "💡 快速操作指南：\n\n" +
                       "1. 設定遊戲路徑：請在右側「系統控制台」指定 Against Rome 安裝目錄（修改器會自動嘗試讀取註冊表以取得路徑）。\n\n" +
                       "2. 讀取現有設定：點擊右側「讀取現有設定」，修改器會自動從遊戲實體檔案（objdef.dau, ress.ini, cl_script.ini 等）解析目前套用的參數，並呈現於兵種列表對比中。\n\n" +
                       "3. 調整偏好與開關：在主控制台完成您喜好的修改配置（如人口上限、倍率開關等）。\n\n" +
                       "4. 執行修改與啟動：點擊右側「執行修改」按鈕將設定套入遊戲；完成後即可點擊「啟動遊戲」按鈕立刻開啟遊戲進入戰鬥！\n\n" +
                       "5. 兵種屬性觀察：可在左側導覽列切換至「預設兵種屬性」與「當前兵種數值」頁面，即時比對原版與修改後的細部屬性資料。"
            };
            pnlTipsCard.Controls.Add(lblTipsContent);

            tabSystem.Controls.Add(pnlNumericCard);
            tabSystem.Controls.Add(pnlSwitchesCard);
            tabSystem.Controls.Add(pnlTipsCard);

            pnlConsoleCard = new Panel {
                Location = new Point(1440, 60),
                Size = new Size(380, 800)
            };
            pnlConsoleCard.Paint += CardPanel_Paint;

            Label lblConsoleTitle = new Label {
                Text = "系統控制台與操作",
                Location = new Point(20, 20),
                Size = new Size(200, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlConsoleCard.Controls.Add(lblConsoleTitle);

            lblGamePath = new Label {
                Text = "遊戲路徑:",
                Location = new Point(20, 55),
                Size = new Size(80, 25),
                ForeColor = Color.FromArgb(200, 205, 210),
                BackColor = Color.Transparent
            };
            pnlConsoleCard.Controls.Add(lblGamePath);

            Panel pnlGamePath = CreateInputWrapper(20, 80, 260, 28);
            txtGamePath = new TextBox {
                Location = new Point(5, 5),
                Size = new Size(250, 20),
                BorderStyle = BorderStyle.None,
                BackColor = Color.FromArgb(38, 38, 48),
                ForeColor = Color.White
            };
            pnlGamePath.Controls.Add(txtGamePath);
            pnlConsoleCard.Controls.Add(pnlGamePath);

            btnBrowseGamePath = new Button {
                Text = "瀏覽...",
                Location = new Point(290, 80),
                Size = new Size(70, 28)
            };
            StyleButton(btnBrowseGamePath, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnBrowseGamePath.Click += new EventHandler(BtnBrowseGamePath_Click);
            pnlConsoleCard.Controls.Add(btnBrowseGamePath);

            string detectedPath = DetectGamePathFromRegistry();
            if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe"))) {
                txtGamePath.Text = AppContext.BaseDirectory;
            } else if (!string.IsNullOrEmpty(detectedPath)) {
                txtGamePath.Text = detectedPath;
            } else if (Directory.Exists(@"C:\Program Files (x86)\Against Rome")) {
                txtGamePath.Text = @"C:\Program Files (x86)\Against Rome";
            }

            txtLog = new TextBox {
                Location = new Point(20, 120),
                Size = new Size(340, 540),
                Multiline = true,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical,
                BackColor = Color.FromArgb(12, 12, 16),
                ForeColor = Color.FromArgb(0, 255, 128),
                Font = fontConsolas95,
                BorderStyle = BorderStyle.None,
                Text = "SYSTEM INITIALIZED\r\n"
            };
            pnlConsoleCard.Controls.Add(txtLog);

            menuRestore = new ContextMenuStrip { Renderer = new DarkContextMenuRenderer() };
            ToolStripMenuItem itemRestoreAll = new ToolStripMenuItem("全部還原");
            ToolStripMenuItem itemRestoreStats = new ToolStripMenuItem("僅還原兵種屬性");
            ToolStripMenuItem itemRestoreCompat = new ToolStripMenuItem("僅還原相容性修正");
            ToolStripMenuItem itemRestoreLang = new ToolStripMenuItem("僅還原語系設定");

            menuRestore.Items.Add(itemRestoreAll);
            menuRestore.Items.Add(itemRestoreStats);
            menuRestore.Items.Add(itemRestoreCompat);
            menuRestore.Items.Add(itemRestoreLang);

            itemRestoreAll.Click += (s, e) => RestoreAll();
            itemRestoreStats.Click += (s, e) => RestoreStatsOnly();
            itemRestoreCompat.Click += (s, e) => RestoreCompatOnly();
            itemRestoreLang.Click += (s, e) => RestoreLanguageOnly();

            btnLoadCurrent = new Button {
                Text = "讀取現有設定",
                Location = new Point(20, 675),
                Size = new Size(165, 45)
            };
            StyleButton(btnLoadCurrent, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnLoadCurrent.Click += new EventHandler(BtnLoadCurrent_Click);

            btnRestore = new Button {
                Text = "恢復原版",
                Location = new Point(195, 675),
                Size = new Size(165, 45)
            };
            StyleButton(btnRestore, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnRestore.Click += (s, e) => {
                menuRestore.Show(btnRestore, new Point(0, btnRestore.Height));
            };

            btnApply = new Button {
                Text = "執行修改",
                Location = new Point(20, 730),
                Size = new Size(165, 45)
            };
            StyleButton(btnApply, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnApply.Click += new EventHandler(BtnApply_Click);

            btnStartGame = new Button {
                Text = "啟動遊戲",
                Location = new Point(195, 730),
                Size = new Size(165, 45)
            };
            StyleButton(btnStartGame, Color.FromArgb(0, 180, 120), Color.White, Color.FromArgb(0, 220, 150));
            btnStartGame.Click += new EventHandler(BtnStartGame_Click);

            pnlConsoleCard.Controls.Add(btnLoadCurrent);
            pnlConsoleCard.Controls.Add(btnRestore);
            pnlConsoleCard.Controls.Add(btnApply);
            pnlConsoleCard.Controls.Add(btnStartGame);

            Panel pnlDefaultStatsTitle = new Panel {
                Location = new Point(0, 0),
                Size = new Size(1190, 65)
            };
            pnlDefaultStatsTitle.Paint += CardPanel_Paint;

            Label lblDefaultStatsTitle = new Label {
                Text = "預設兵種屬性對比 (無自訂加成)",
                Location = new Point(20, 20),
                Size = new Size(280, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };

            chkBalance = new ModernToggle {
                Text = "啟用預設兵種屬性平衡與陣營特色",
                Location = new Point(320, 18),
                Size = new Size(350, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            chkBalance.CheckedChanged += new EventHandler(ChkBalance_CheckedChanged);

            pnlDefaultStatsTitle.Controls.Add(lblDefaultStatsTitle);
            pnlDefaultStatsTitle.Controls.Add(chkBalance);

            defaultStatsTabControl = new TabControl {
                Location = new Point(0, 80),
                Size = new Size(1190, 715)
            };

            TabPage tabDefaultRoman = new TabPage { Text = " 羅馬 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };
            TabPage tabDefaultTeuton = new TabPage { Text = " 條頓 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };
            TabPage tabDefaultCelt = new TabPage { Text = " 塞爾特 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };
            TabPage tabDefaultHun = new TabPage { Text = " 匈奴 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };

            defaultStatsGrids["Roman"] = CreateDefaultStatsGrid();
            defaultStatsGrids["Teuton"] = CreateDefaultStatsGrid();
            defaultStatsGrids["Celt"] = CreateDefaultStatsGrid();
            defaultStatsGrids["Hun"] = CreateDefaultStatsGrid();

            tabDefaultRoman.Controls.Add(defaultStatsGrids["Roman"]);
            tabDefaultTeuton.Controls.Add(defaultStatsGrids["Teuton"]);
            tabDefaultCelt.Controls.Add(defaultStatsGrids["Celt"]);
            tabDefaultHun.Controls.Add(defaultStatsGrids["Hun"]);

            defaultStatsTabControl.TabPages.Add(tabDefaultRoman);
            defaultStatsTabControl.TabPages.Add(tabDefaultTeuton);
            defaultStatsTabControl.TabPages.Add(tabDefaultCelt);
            defaultStatsTabControl.TabPages.Add(tabDefaultHun);

            tabDefaultStats.Controls.Add(pnlDefaultStatsTitle);
            tabDefaultStats.Controls.Add(defaultStatsTabControl);

            Panel pnlCurrentStatsTitle = new Panel {
                Location = new Point(0, 0),
                Size = new Size(1190, 65)
            };
            pnlCurrentStatsTitle.Paint += CardPanel_Paint;

            Label lblCurrentStatsTitle = new Label {
                Text = "當前兵種數值 (原版與當前對比)",
                Location = new Point(20, 20),
                Size = new Size(350, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlCurrentStatsTitle.Controls.Add(lblCurrentStatsTitle);

            currentStatsTabControl = new TabControl {
                Location = new Point(0, 80),
                Size = new Size(1190, 715)
            };

            TabPage tabCurrentRoman = new TabPage { Text = " 羅馬 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };
            TabPage tabCurrentTeuton = new TabPage { Text = " 條頓 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };
            TabPage tabCurrentCelt = new TabPage { Text = " 塞爾特 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };
            TabPage tabCurrentHun = new TabPage { Text = " 匈奴 ", BackColor = Color.FromArgb(16, 16, 20), UseVisualStyleBackColor = false };

            currentStatsGrids["Roman"] = CreateCurrentStatsGrid();
            currentStatsGrids["Teuton"] = CreateCurrentStatsGrid();
            currentStatsGrids["Celt"] = CreateCurrentStatsGrid();
            currentStatsGrids["Hun"] = CreateCurrentStatsGrid();

            tabCurrentRoman.Controls.Add(currentStatsGrids["Roman"]);
            tabCurrentTeuton.Controls.Add(currentStatsGrids["Teuton"]);
            tabCurrentCelt.Controls.Add(currentStatsGrids["Celt"]);
            tabCurrentHun.Controls.Add(currentStatsGrids["Hun"]);

            currentStatsTabControl.TabPages.Add(tabCurrentRoman);
            currentStatsTabControl.TabPages.Add(tabCurrentTeuton);
            currentStatsTabControl.TabPages.Add(tabCurrentCelt);
            currentStatsTabControl.TabPages.Add(tabCurrentHun);

            tabCurrentStats.Controls.Add(pnlCurrentStatsTitle);
            tabCurrentStats.Controls.Add(currentStatsTabControl);

            tabDoc = new TabPage {
                BackColor = Color.FromArgb(16, 16, 20),
                UseVisualStyleBackColor = false
            };
            mainTabControl.TabPages.Add(tabDoc);

            string docText = "";
            try {
                using (Stream? stream = typeof(Program).Assembly.GetManifestResourceStream("TechDoc.md")) {
                    if (stream != null) {
                        using (StreamReader reader = new StreamReader(stream, Encoding.UTF8)) {
                            docText = reader.ReadToEnd();
                        }
                    } else {
                        foreach (string name in typeof(Program).Assembly.GetManifestResourceNames()) {
                            if (name.EndsWith("TechDoc.md")) {
                                using (Stream? s = typeof(Program).Assembly.GetManifestResourceStream(name)) {
                                    if (s != null) {
                                        using (StreamReader r = new StreamReader(s, Encoding.UTF8)) {
                                            docText = r.ReadToEnd();
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            } catch (Exception ex) {
                Log("載入技術文件資源失敗: " + ex.Message);
            }

            docText = docText.Replace("\r\n", "\n").Replace("\n", "\r\n");

            txtDoc = new TextBox {
                Dock = DockStyle.Fill,
                Multiline = true,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical,
                BackColor = Color.FromArgb(20, 20, 25),
                ForeColor = Color.FromArgb(230, 235, 240),
                Font = fontJhengHei105R,
                BorderStyle = BorderStyle.None,
                Text = docText
            };
            tabDoc.Controls.Add(txtDoc);

            tabSaveManager = new TabPage {
                BackColor = Color.FromArgb(16, 16, 20),
                UseVisualStyleBackColor = false
            };
            mainTabControl.TabPages.Add(tabSaveManager);

            Panel pnlLeftSave = new Panel {
                Location = new Point(0, 0),
                Size = new Size(800, 790),
                BackColor = Color.Transparent
            };

            Panel pnlRightSave = new Panel {
                Location = new Point(810, 0),
                Size = new Size(380, 790),
                BackColor = Color.Transparent
            };

            tabSaveManager.Controls.Add(pnlLeftSave);
            tabSaveManager.Controls.Add(pnlRightSave);

            Panel pnlGameSavesCard = new Panel {
                Location = new Point(0, 0),
                Size = new Size(800, 380)
            };
            pnlGameSavesCard.Paint += CardPanel_Paint;

            Panel pnlBackupsCard = new Panel {
                Location = new Point(0, 395),
                Size = new Size(800, 395)
            };
            pnlBackupsCard.Paint += CardPanel_Paint;

            pnlLeftSave.Controls.Add(pnlGameSavesCard);
            pnlLeftSave.Controls.Add(pnlBackupsCard);

            Panel pnlDetailCard = new Panel {
                Location = new Point(0, 0),
                Size = new Size(380, 790)
            };
            pnlDetailCard.Paint += CardPanel_Paint;
            pnlRightSave.Controls.Add(pnlDetailCard);

            Label lblGameSavesTitle = new Label {
                Text = "遊戲中存檔列表",
                Location = new Point(20, 15),
                Size = new Size(200, 20),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlGameSavesCard.Controls.Add(lblGameSavesTitle);

            dgvGameSaves = CreateSaveGrid(false);
            dgvGameSaves.Location = new Point(15, 45);
            dgvGameSaves.Size = new Size(770, 275);
            dgvGameSaves.SelectionChanged += DgvGameSaves_SelectionChanged;
            pnlGameSavesCard.Controls.Add(dgvGameSaves);

            Button btnBackupSave = new Button {
                Text = "備份此存檔",
                Location = new Point(15, 330),
                Size = new Size(140, 35)
            };
            StyleButton(btnBackupSave, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnBackupSave.Click += BtnBackupSave_Click;
            pnlGameSavesCard.Controls.Add(btnBackupSave);

            Button btnDeleteSave = new Button {
                Text = "刪除此存檔",
                Location = new Point(165, 330),
                Size = new Size(140, 35)
            };
            StyleButton(btnDeleteSave, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnDeleteSave.Click += BtnDeleteSave_Click;
            pnlGameSavesCard.Controls.Add(btnDeleteSave);

            Button btnRefreshSaves = new Button {
                Text = "重新整理",
                Location = new Point(315, 330),
                Size = new Size(140, 35)
            };
            StyleButton(btnRefreshSaves, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnRefreshSaves.Click += (s, e) => RefreshSavesAndBackups();
            pnlGameSavesCard.Controls.Add(btnRefreshSaves);

            Label lblBackupsTitle = new Label {
                Text = "備份歷史列表",
                Location = new Point(20, 15),
                Size = new Size(200, 20),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlBackupsCard.Controls.Add(lblBackupsTitle);

            dgvBackups = CreateSaveGrid(true);
            dgvBackups.Location = new Point(15, 45);
            dgvBackups.Size = new Size(770, 290);
            dgvBackups.SelectionChanged += DgvBackups_SelectionChanged;
            pnlBackupsCard.Controls.Add(dgvBackups);

            Button btnRestoreBackup = new Button {
                Text = "還原此備份",
                Location = new Point(15, 345),
                Size = new Size(140, 35)
            };
            StyleButton(btnRestoreBackup, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnRestoreBackup.Click += BtnRestoreBackup_Click;
            pnlBackupsCard.Controls.Add(btnRestoreBackup);

            Button btnDeleteBackup = new Button {
                Text = "刪除此備份",
                Location = new Point(165, 345),
                Size = new Size(140, 35)
            };
            StyleButton(btnDeleteBackup, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnDeleteBackup.Click += BtnDeleteBackup_Click;
            pnlBackupsCard.Controls.Add(btnDeleteBackup);

            Label lblDetailTitle = new Label {
                Text = "存檔詳細與預覽",
                Location = new Point(20, 20),
                Size = new Size(200, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlDetailCard.Controls.Add(lblDetailTitle);

            picSavePreview = new PictureBox {
                Location = new Point(20, 55),
                Size = new Size(340, 255),
                SizeMode = PictureBoxSizeMode.Zoom,
                BackColor = Color.FromArgb(12, 12, 16)
            };
            pnlDetailCard.Controls.Add(picSavePreview);

            lblSaveDetail = new Label {
                Location = new Point(20, 325),
                Size = new Size(340, 440),
                ForeColor = Color.FromArgb(200, 205, 210),
                BackColor = Color.Transparent,
                Font = fontJhengHei10R
            };
            pnlDetailCard.Controls.Add(lblSaveDetail);

            this.Controls.Add(pnlTitleBar);
            this.Controls.Add(pnlSidebar);
            this.Controls.Add(mainTabControl);
            this.Controls.Add(pnlConsoleCard);
            ShowTabPage(tabSystem);
        }

        private Panel CreateInputWrapper(int x, int y, int w, int h) {
            Panel p = new Panel {
                Location = new Point(x, y),
                Size = new Size(w, h),
                BackColor = Color.FromArgb(38, 38, 48)
            };
            p.Paint += InputPanel_Paint;
            return p;
        }

        /// <summary>
        /// 確保備份的 objdef.dau 檔案已被解析並快取至記憶體中。
        /// </summary>
        private void EnsureBackupUnitRowsParsed() {
            if (_backupUnitRowsParsed) return;
            try {
                byte[]? origBytes;
                if (backupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out origBytes)) {
                    byte[] decompBytes = GameLZSS.DecompressPfil(origBytes!);
                    string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
                    string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
                    string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);
                    for (int idx = 2; idx < lines.Length; idx++) {
                        string line = lines[idx];
                        if (line.Length < 100) continue;
                        string[] cols = ParseCsvLine(line);
                        if (cols.Length < 192) continue;
                        string name = cols[52].Trim();
                        if (TroopConfig.UnitMeta.ContainsKey(name) || name == "FigZivMan00_Zivilist") {
                            _backupUnitRows[name] = cols;
                        }
                    }
                }
                _backupUnitRowsParsed = true;
            } catch (Exception ex) {
                Log("解析備份 objdef.dau 失敗: " + ex.Message);
            }
        }

        /// <summary>
        /// 自訂導覽列按鈕繪製樣式，包含 Hover 漸層與選取指示條
        /// </summary>
        private void StyleNavButton(Button btn, string text, TabPage associatedPage) {
            btn.Text = ""; // 採用 Paint 自繪，清除原生文字
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 0;
            btn.BackColor = Color.Transparent;
            btn.Cursor = Cursors.Hand;
            btn.Size = new Size(200, 45);

            btn.Paint += (s, e) => {
                Graphics g = e.Graphics;
                g.SmoothingMode = SmoothingMode.AntiAlias;
                bool isSelected = (mainTabControl.SelectedTab == associatedPage);
                
                Point clientPos = btn.PointToClient(Cursor.Position);
                bool isHovered = btn.ClientRectangle.Contains(clientPos);

                // 1. 繪製背景 (選取時有漂亮的漸層藍色，Hover 時有微亮背景)
                if (isSelected) {
                    using (var brush = new LinearGradientBrush(btn.ClientRectangle, Color.FromArgb(30, 30, 42), Color.FromArgb(24, 40, 50), 0f)) {
                        g.FillRectangle(brush, btn.ClientRectangle);
                    }
                } else if (isHovered) {
                    using (var brush = new SolidBrush(Color.FromArgb(28, 28, 35))) {
                        g.FillRectangle(brush, btn.ClientRectangle);
                    }
                }

                // 2. 繪製文字與圖示 (Unicode 圖示)
                Color foreColor = isSelected ? Color.FromArgb(0, 220, 255) : (isHovered ? Color.White : Color.FromArgb(150, 160, 175));
                using (var brush = new SolidBrush(foreColor)) {
                    SizeF sz = g.MeasureString(text, fontJhengHei10B);
                    g.DrawString(text, fontJhengHei10B, brush, 15, (btn.Height - sz.Height) / 2);
                }

                // 3. 繪製左側發光指示條
                if (isSelected) {
                    using (var brush = new SolidBrush(Color.FromArgb(0, 220, 255))) {
                        g.FillRectangle(brush, 0, 8, 4, btn.Height - 16);
                    }
                }
            };

            // 註冊滑鼠事件以即時重繪
            btn.MouseEnter += (s, e) => btn.Invalidate();
            btn.MouseLeave += (s, e) => btn.Invalidate();
            btn.MouseMove += (s, e) => btn.Invalidate();
        }

        /// <summary>
        /// 強制重繪所有導覽按鈕，用以即時更新選取狀態
        /// </summary>
        private void RefreshNavButtons() {
            btnNavSystem.Invalidate();
            btnNavDefaultStats.Invalidate();
            btnNavCurrentStats.Invalidate();
            btnNavSaveManager.Invalidate();
            btnNavDoc.Invalidate();
        }
    }
}
