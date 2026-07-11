using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier {
    // 修改器的主表單類別，繼承自 Windows Form
    public partial class ModifierForm : Form {
        // 導入 Gdi32.dll 的 Win32 API，用於在 Windows 10/11 下為視窗建立圓角區域
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        [DllImport("gdi32.dll", EntryPoint = "DeleteObject")]
        private static extern bool DeleteObject(IntPtr hObject);

        // 標題列與控制按鈕
        private Panel pnlTitleBar = null!;
        private Label lblMainTitle = null!;
        private Button btnClose = null!;
        private Button btnMinimize = null!;

        // 左側導覽列按鈕
        private Panel pnlSidebar = null!;
        private Panel pnlRightSidebar = null!;
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
        private Panel pnlBuildCard = null!;
        private Panel pnlAiCard = null!;

        // 數值控制項 (NumericUpDown) 的宣告
        private ModernToggle chkMaxPopulation = null!;
        private ModernToggle chkFastCiviProduction = null!;

        // 功能開關的核取方塊 (自訂 ModernToggle 開關)
        private ModernToggle chkFreeProd = null!;
        private ModernToggle chkFreeUpgrade = null!;
        private ModernToggle chkNoSpellCost = null!;
        private ModernToggle chkNoSpellAltar = null!;
        private ModernToggle chkFocusLoss = null!;
        private ModernToggle chkBalance = null!;
        private ModernToggle chkHousingCapacity20x = null!;
        private ModernToggle chkStorageCapacity10x = null!;
        private ModernToggle chkHqHp10x = null!;
        private ModernToggle chkFastBuildUpgradeRepair = null!;
        private ModernToggle chkFoodHealing10x = null!;
        private ModernToggle chkAiM1 = null!;
        private ModernToggle chkAiM2 = null!;
        private ModernToggle chkAiM3 = null!;
        private ModernToggle chkAiM4 = null!;
        private ModernToggle chkAiM5 = null!;
        private ModernToggle chkAiM6 = null!;
        private ModernToggle chkDgVoodoo = null!;
        private ModernToggle chkVillageBuildRange = null!;
        private Label lblGameSpeed = null!;
        private ComboBox cmbGameSpeed = null!;
        private Button btnTroopPreset = null!;
        private Label lblTroopTemplate = null!;
        private ComboBox cbTroopTemplate = null!;
        private Label lblTroopPresetFile = null!;
        private Dictionary<string, double[]>? customUnitStats = null;
        private string presetFileSourceType = "default";
        private string presetFileName = "";
        private ModernToggle chkToEng = null!;
        private ModernToggle chkInfiniteMorale = null!;

        // 所有功能開啟/關閉按鈕
        private Button btnEnableAll = null!;
        private Button btnDisableAll = null!;

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

        // 核心解耦服務實例
        private AgainstRomeModifier.Core.Services.BackupManager backupManager = null!;
        private AgainstRomeModifier.Core.Services.PatchEngine patchEngine = null!;

        private class FormLogger : AgainstRomeModifier.Core.Services.ILogger
        {
            private readonly ModifierForm _form;
            public FormLogger(ModifierForm form) => _form = form;
            public void Log(string message) => _form.Log(message);
        }

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
        
        // 語系切換 UI 欄位
        private Label lblSidebarLang = null!;
        private Button btnLangZH = null!;
        private Button btnLangEN = null!;
        private Label lblNumericTitle = null!;
        private Label lblSwitchesTitle = null!;
        private Label lblSystemHeading = null!;
        private Label lblSystemSubtitle = null!;
        private ToolTip myToolTip = null!;
        private Label lblBuildTitle = null!;
        private Label lblAiTitle = null!;
        private Label lblGameSavesTitle = null!;
        private Label lblBackupsTitle = null!;
        private Label lblDetailTitle = null!;
        private Button btnBackupSave = null!;
        private Button btnDeleteSave = null!;
        private Button btnRefreshSaves = null!;
        private Button btnRestoreBackup = null!;
        private Button btnDeleteBackup = null!;
        private Label lblDefaultStatsTitle = null!;
        private Label lblCurrentStatsTitle = null!;

        // 恢復原版功能選單
        private ToolStripMenuItem itemRestoreAll = null!;
        private ToolStripMenuItem itemRestoreStats = null!;
        private ToolStripMenuItem itemRestoreCompat = null!;
        private ToolStripMenuItem itemRestoreLang = null!;

        // 自訂兵種屬性分頁
        private TabPage tabDefaultRoman = null!;
        private TabPage tabDefaultTeuton = null!;
        private TabPage tabDefaultCelt = null!;
        private TabPage tabDefaultHun = null!;

        // 當前兵種數值分頁
        private TabPage tabCurrentRoman = null!;
        private TabPage tabCurrentTeuton = null!;
        private TabPage tabCurrentCelt = null!;
        private TabPage tabCurrentHun = null!;

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
        private Font fontConsolas85 = new Font("Consolas", 8.5F, FontStyle.Regular);
        
        // 統一風格的按鈕基礎顏色
        private static readonly Color ColorBtnPrimary = Color.FromArgb(38, 132, 255);

        // 建構函式：初始化 UI 元件，載入備份檔並初始化現有設定
        public ModifierForm() {
            // 開啟時讀取系統語言
            string sysLang = System.Globalization.CultureInfo.CurrentUICulture.Name;
            if (sysLang.StartsWith("en", StringComparison.OrdinalIgnoreCase)) {
                Loc.CurrentLanguage = Language.English;
            } else {
                Loc.CurrentLanguage = Language.TraditionalChinese;
            }

            InitializeComponent();

            // 更新語系按鈕視覺狀態與套用語系
            UpdateLanguageButtonStyles();
            ApplyLanguageToUI();

            var logger = new FormLogger(this);
            backupManager = new AgainstRomeModifier.Core.Services.BackupManager(logger);
            patchEngine = new AgainstRomeModifier.Core.Services.PatchEngine(logger);

            Log(Loc.Get("LogConstructCompleted"));
            // 將內嵌的 Backup.zip 載入記憶體
            backupManager.LoadBackupZipToMemory(GetGamePath());
            
            try {
                string gamePath = GetGamePath();
                if (!string.IsNullOrWhiteSpace(gamePath) && Directory.Exists(gamePath)) {
                    // 啟動只做針對性的安全遷移（R0 常駐修復 + 舊版首領閃退腳本重建），
                    // 不執行完整 ApplyPatches——完整套用會依「可偵測的選項」還原後重寫檔案，
                    // 而自訂兵種屬性偵測不回來，會在啟動瞬間被無聲覆蓋成平衡值。
                    using (var rollback = new FileRollbackScope()) {
                        patchEngine.RunStartupSafeMigrations(gamePath, backupManager, rollback);
                        rollback.Commit();
                    }
                }
            } catch (Exception ex) {
                Log("舊版腳本安全遷移失敗: " + ex.Message);
            }

            // 初始化資料與讀取自訂兵種資訊
            InitializeData();
        }

        /// <summary>
        /// 依 WinForms 慣例在 Dispose 釋放自建的字型與圖示資源
        /// （FormClosing 在部分關閉路徑不保證觸發，不適合當釋放點）。
        /// </summary>
        protected override void Dispose(bool disposing) {
            if (disposing) {
                try {
                    foreach (var img in unitIcons.Values) {
                        img?.Dispose();
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
                    fontConsolas85.Dispose();
                    myToolTip?.Dispose();
                } catch (Exception ex) {
                    System.Diagnostics.Debug.WriteLine("釋放資源失敗: " + ex.Message);
                }
            }
            base.Dispose(disposing);
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
            using (SolidBrush sb = new SolidBrush(Color.FromArgb(18, 22, 31))) {
                using (GraphicsPath path = GetRoundPath(new Rectangle(0, 0, pnl.Width, pnl.Height), 10)) {
                    e.Graphics.FillPath(sb, path); // 填滿背景色
                    using (Pen p = new Pen(Color.FromArgb(45, 53, 69), 1)) {
                        e.Graphics.DrawPath(p, path); // 繪製卡片細緻邊框
                    }
                }
            }
        }

        // 輸入欄位外框 Panel 的 Paint 事件處理程序，繪製扁平風格的欄位外框
        private void InputPanel_Paint(object? sender, PaintEventArgs e) {
            Panel pnl = (Panel)sender!;
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using (Pen p = new Pen(Color.FromArgb(50, 52, 70), 1)) {
                e.Graphics.DrawRectangle(p, 0, 0, pnl.Width - 1, pnl.Height - 1);
            }
        }

        // 統一設定 Button 控制項的扁平化樣式、背景顏色、滑鼠懸停 (Hover) 微動畫與邊框發光效果 (圓角漸層自繪樣式)
        private void StyleButton(Button btn, Color backColor, Color foreColor, Color hoverBorderColor) {
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 0; // 關閉預設邊框以利自繪
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
                    // 根據 backColor 判斷按鈕角色並套用漸層
                    Color startColor, endColor;
                    if (backColor == Color.FromArgb(98, 0, 238) || backColor == ColorBtnPrimary) {
                        startColor = isHovered ? Color.FromArgb(52, 151, 255) : Color.FromArgb(34, 124, 246);
                        endColor = isHovered ? Color.FromArgb(66, 199, 255) : Color.FromArgb(43, 160, 255);
                    } else if (backColor == Color.FromArgb(0, 180, 120)) {
                        startColor = isHovered ? Color.FromArgb(0, 200, 140) : Color.FromArgb(0, 160, 100);
                        endColor = isHovered ? Color.FromArgb(0, 240, 170) : Color.FromArgb(0, 190, 130);
                    } else { // 預設按鈕 (如灰色 btnRestore、btnBrowse 等)
                        startColor = isHovered ? Color.FromArgb(47, 55, 70) : Color.FromArgb(31, 37, 49);
                        endColor = isHovered ? Color.FromArgb(56, 66, 84) : Color.FromArgb(38, 45, 59);
                    }

                    using (LinearGradientBrush brush = new LinearGradientBrush(rect, startColor, endColor, 45F)) {
                        g.FillPath(brush, path);
                    }

                    // 繪製細緻邊框
                    Color borderColor = isHovered ? hoverBorderColor : Color.FromArgb(50, 52, 70);
                    using (Pen p = new Pen(borderColor, 1.2F)) {
                        g.DrawPath(p, path);
                    }

                    // 繪製按鈕文字
                    if (!string.IsNullOrEmpty(btn.Text)) {
                        TextRenderer.DrawText(
                            g,
                            btn.Text,
                            btn.Font,
                            rect,
                            foreColor,
                            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.WordBreak
                        );
                    }
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
            this.Text = "Against Rome Modifier Pro"; // OS 視窗標題（工作列／Alt-Tab 顯示；邊框仍隱藏）
            this.Size = new Size(1450, 880);
            this.MinimumSize = new Size(1280, 720);
            this.AutoScroll = true;
            this.AutoScrollMinSize = new Size(1450, 880);
            this.FormBorderStyle = FormBorderStyle.None; // 隱藏 Windows 預設視窗邊框
            this.StartPosition = FormStartPosition.CenterScreen; // 視窗預設居中
            this.BackColor = Color.FromArgb(10, 11, 16); // 深色科技感背景
            this.ForeColor = Color.FromArgb(230, 235, 240);
            this.Font = fontJhengHei95R;
            this.DoubleBuffered = true; // 啟用雙緩衝防止繪圖閃爍

            myToolTip = new ToolTip {
                InitialDelay = 150,
                ReshowDelay = 50,
                AutoPopDelay = 10000,
                ShowAlways = true,
                OwnerDraw = true
            };
            myToolTip.Popup += (s, e) => {
                string text = myToolTip.GetToolTip(e.AssociatedControl) ?? "";
                Size size = TextRenderer.MeasureText(text, fontJhengHei95R);
                e.ToolTipSize = new Size(size.Width + 18, size.Height + 12);
            };
            myToolTip.Draw += (s, e) => {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                using (SolidBrush bgBrush = new SolidBrush(Color.FromArgb(18, 19, 29))) {
                    e.Graphics.FillRectangle(bgBrush, e.Bounds);
                }
                using (Pen borderPen = new Pen(Color.FromArgb(0, 230, 255), 1)) {
                    e.Graphics.DrawRectangle(borderPen, 0, 0, e.Bounds.Width - 1, e.Bounds.Height - 1);
                }
                TextRenderer.DrawText(
                    e.Graphics,
                    e.ToolTipText,
                    fontJhengHei95R,
                    new Rectangle(9, 6, e.Bounds.Width - 18, e.Bounds.Height - 12),
                    Color.FromArgb(220, 225, 235),
                    TextFormatFlags.WordBreak | TextFormatFlags.VerticalCenter | TextFormatFlags.HorizontalCenter
                );
            };

            // 表單 Load 事件：使用 Win32 API 建立圓角裁剪區域
            this.Load += (s, e) => {
                Rectangle workingArea = Screen.FromControl(this).WorkingArea;
                if (Width > workingArea.Width || Height > workingArea.Height) {
                    Size = new Size(Math.Min(Width, workingArea.Width), Math.Min(Height, workingArea.Height));
                    Location = new Point(
                        workingArea.Left + Math.Max(0, (workingArea.Width - Width) / 2),
                        workingArea.Top + Math.Max(0, (workingArea.Height - Height) / 2)
                    );
                }
                IntPtr ptr = CreateRoundRectRgn(0, 0, Width, Height, 15, 15);
                this.Region = Region.FromHrgn(ptr);
                DeleteObject(ptr);
            };

            // 表單 Paint 事件：動態繪製霓虹青色外框線
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
                Size = new Size(1450, 50),
                BackColor = Color.FromArgb(18, 19, 29)
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblMainTitle = new Label {
                Text = "AGAINST ROME MODIFIER PRO",
                Location = new Point(20, 14),
                Size = new Size(300, 25),
                Font = fontJhengHei115B,
                ForeColor = Color.FromArgb(0, 230, 255)
            };
            lblMainTitle.MouseDown += TitleBar_MouseDown;
            lblMainTitle.MouseMove += TitleBar_MouseMove;
            lblMainTitle.MouseUp += TitleBar_MouseUp;

            btnClose = new Button {
                Text = "×",
                Location = new Point(1410, 10),
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
                Location = new Point(1370, 10),
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
            pnlTitleBar.Controls.Add(btnClose);
            pnlTitleBar.Controls.Add(btnMinimize);

            pnlSidebar = new Panel {
                Location = new Point(0, 50),
                Size = new Size(220, 830),
                BackColor = Color.FromArgb(15, 16, 24)
            };

            pnlRightSidebar = new Panel {
                Location = new Point(1200, 50),
                Size = new Size(250, 830),
                BackColor = Color.FromArgb(15, 16, 24)
            };

            // 改為按鈕自繪指示條，pnlActiveIndicator 不再需要
            btnNavSystem = new Button { Location = new Point(10, 30) };
            StyleNavButton(btnNavSystem, "NavSystem", tabSystem);
            btnNavSystem.Click += (s, e) => {
                ShowTabPage(tabSystem);
                RefreshNavButtons();
            };

            btnNavDefaultStats = new Button { Location = new Point(10, 85) };
            StyleNavButton(btnNavDefaultStats, "NavDefaultStats", tabDefaultStats);
            btnNavDefaultStats.Click += (s, e) => {
                ShowTabPage(tabDefaultStats);
                RefreshNavButtons();
            };

            btnNavCurrentStats = new Button { Location = new Point(10, 140) };
            StyleNavButton(btnNavCurrentStats, "NavCurrentStats", tabCurrentStats);
            btnNavCurrentStats.Click += (s, e) => {
                ShowTabPage(tabCurrentStats);
                RefreshNavButtons();
            };

            btnNavSaveManager = new Button { Location = new Point(10, 0) };
            StyleNavButton(btnNavSaveManager, "NavSaveManager", tabSaveManager);
            btnNavSaveManager.Click += (s, e) => {
                ShowTabPage(tabSaveManager);
                RefreshNavButtons();
                RefreshSavesAndBackups();
            };

            btnNavDoc = new Button { Location = new Point(10, 0) };
            StyleNavButton(btnNavDoc, "NavDoc", tabDoc);
            btnNavDoc.Click += (s, e) => {
                ShowTabPage(tabDoc);
                RefreshNavButtons();
            };

            // 語系切換元件初始化與事件綁定
            lblSidebarLang = new Label {
                Text = Loc.Get("LanguageLabel"),
                Location = new Point(15, 740),
                Size = new Size(190, 20),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(150, 160, 175),
                BackColor = Color.Transparent
            };

            btnLangZH = new Button {
                Text = Loc.Get("LangZhButton"),
                Location = new Point(15, 765),
                Size = new Size(90, 30),
                FlatStyle = FlatStyle.Flat,
                Font = fontJhengHei9R,
                Cursor = Cursors.Hand
            };
            btnLangZH.FlatAppearance.BorderSize = 1;
            btnLangZH.Click += (s, e) => {
                if (Loc.CurrentLanguage != Language.TraditionalChinese) {
                    Loc.CurrentLanguage = Language.TraditionalChinese;
                    UpdateLanguageButtonStyles();
                    ApplyLanguageToUI();
                }
            };

            btnLangEN = new Button {
                Text = Loc.Get("LangEnButton"),
                Location = new Point(115, 765),
                Size = new Size(90, 30),
                FlatStyle = FlatStyle.Flat,
                Font = fontJhengHei9R,
                Cursor = Cursors.Hand
            };
            btnLangEN.FlatAppearance.BorderSize = 1;
            btnLangEN.Click += (s, e) => {
                if (Loc.CurrentLanguage != Language.English) {
                    Loc.CurrentLanguage = Language.English;
                    UpdateLanguageButtonStyles();
                    ApplyLanguageToUI();
                }
            };

            pnlSidebar.Controls.Add(btnNavSystem);
            pnlSidebar.Controls.Add(btnNavDefaultStats);
            pnlSidebar.Controls.Add(btnNavCurrentStats);
            pnlSidebar.Controls.Add(btnNavSaveManager);
            pnlSidebar.Controls.Add(btnNavDoc);
            pnlSidebar.Controls.Add(lblSidebarLang);
            pnlSidebar.Controls.Add(btnLangZH);
            pnlSidebar.Controls.Add(btnLangEN);

            mainTabControl = new ModernTabControl {
                Location = new Point(230, 60),
                Size = new Size(1200, 810),
                SizeMode = TabSizeMode.Fixed,
                ItemSize = new Size(0, 1),
                HideTabs = true
            };

            tabSystem = new TabPage {
                BackColor = Color.FromArgb(10, 11, 16),
                UseVisualStyleBackColor = false
            };

            tabDefaultStats = new TabPage {
                BackColor = Color.FromArgb(10, 11, 16),
                UseVisualStyleBackColor = false
            };

            tabCurrentStats = new TabPage {
                BackColor = Color.FromArgb(10, 11, 16),
                UseVisualStyleBackColor = false
            };

            mainTabControl.TabPages.Add(tabSystem);
            mainTabControl.TabPages.Add(tabDefaultStats);
            mainTabControl.TabPages.Add(tabCurrentStats);

            pnlNumericCard = new Panel {
                Location = new Point(0, 0),
                Size = new Size(385, 790)
            };
            // pnlNumericCard.Paint += CardPanel_Paint;

            lblNumericTitle = new Label {
                Text = "系統與相容性設定",
                Location = new Point(25, 20),
                Size = new Size(250, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlNumericCard.Controls.Add(lblNumericTitle);

            chkFocusLoss = new ModernToggle {
                Text = "遊戲視窗失焦時不自動暫停 (背景執行)",
                Location = new Point(25, 80),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlNumericCard.Controls.Add(chkFocusLoss);

            chkToEng = new ModernToggle {
                Text = "強制英文語系 (介面圖示與核心文字)",
                Location = new Point(25, 160),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlNumericCard.Controls.Add(chkToEng);

            chkDgVoodoo = new ModernToggle {
                Text = Loc.Get("DgVoodoo"),
                Location = new Point(25, 240),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlNumericCard.Controls.Add(chkDgVoodoo);

            // 實際位置與寬度由 ConfigureGameSpeedRow 依卡片列版面統一計算，此處僅提供初始佔位值。
            lblGameSpeed = new Label {
                Text = Loc.Get("GameSpeedLabel"),
                Location = new Point(20, 207),
                Size = new Size(140, 25),
                Font = fontJhengHei95R,
                ForeColor = Color.White,
                BackColor = Color.Transparent,
                AutoSize = true
            };
            cmbGameSpeed = new ComboBox {
                Location = new Point(170, 204),
                Size = new Size(150, 25),
                DropDownStyle = ComboBoxStyle.DropDownList,
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.FromArgb(45, 45, 55),
                ForeColor = Color.White,
                Font = fontJhengHei10B
            };
            PopulateGameSpeedItems();
            cmbGameSpeed.SelectedIndex = 0;
            pnlNumericCard.Controls.Add(lblGameSpeed);
            pnlNumericCard.Controls.Add(cmbGameSpeed);

            btnEnableAll = new Button {
                Text = "所有功能開啟",
                Location = new Point(25, 710),
                Size = new Size(155, 42)
            };
            StyleButton(btnEnableAll, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnEnableAll.Click += new EventHandler(BtnEnableAll_Click);

            btnDisableAll = new Button {
                Text = "所有功能關閉",
                Location = new Point(200, 710),
                Size = new Size(155, 42)
            };
            StyleButton(btnDisableAll, Color.FromArgb(45, 45, 55), Color.FromArgb(255, 75, 75), Color.FromArgb(255, 75, 75));
            btnDisableAll.Click += new EventHandler(BtnDisableAll_Click);
            pnlNumericCard.Controls.Add(btnEnableAll);
            pnlNumericCard.Controls.Add(btnDisableAll);

            pnlSwitchesCard = new Panel {
                Location = new Point(402, 0),
                Size = new Size(386, 790)
            };
            // pnlSwitchesCard.Paint += CardPanel_Paint;

            lblSwitchesTitle = new Label {
                Text = "資源與戰鬥修改",
                Location = new Point(25, 20),
                Size = new Size(250, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlSwitchesCard.Controls.Add(lblSwitchesTitle);

            chkFreeProd = new ModernToggle {
                Text = "建造、修復與所有單位生產完全免費",
                Location = new Point(25, 80),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlSwitchesCard.Controls.Add(chkFreeProd);

            chkFreeUpgrade = new ModernToggle {
                Text = "陣型、研發、屬性解鎖升級免費",
                Location = new Point(25, 160),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlSwitchesCard.Controls.Add(chkFreeUpgrade);

            chkNoSpellCost = new ModernToggle {
                Text = "祭司與賢者法術無消耗 (MP 零消耗)",
                Location = new Point(25, 240),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlSwitchesCard.Controls.Add(chkNoSpellCost);

            chkInfiniteMorale = new ModernToggle {
                Text = "部隊無限士氣 (士氣不減且極速恢復)",
                Location = new Point(25, 320),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlSwitchesCard.Controls.Add(chkInfiniteMorale);

            chkBalance = new ModernToggle {
                Text = Loc.Get("EnableBalance"),
                Location = new Point(25, 400),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            chkBalance.CheckedChanged += new EventHandler(ChkBalance_CheckedChanged);
            pnlSwitchesCard.Controls.Add(chkBalance);

            chkNoSpellAltar = new ModernToggle {
                Text = "法術免除祭壇數量需求",
                Location = new Point(25, 480),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlSwitchesCard.Controls.Add(chkNoSpellAltar);

            // 新增：建設與人口修改卡片
            pnlBuildCard = new Panel {
                Location = new Point(805, 0),
                Size = new Size(385, 790)
            };
            // pnlBuildCard.Paint += CardPanel_Paint;

            lblBuildTitle = new Label {
                Text = Loc.Get("BuildTitle"),
                Location = new Point(25, 20),
                Size = new Size(250, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlBuildCard.Controls.Add(lblBuildTitle);

            chkMaxPopulation = new ModernToggle {
                Text = Loc.Get("MaxPopulation"),
                Location = new Point(25, 80),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkMaxPopulation);

            chkHousingCapacity20x = new ModernToggle {
                Text = Loc.Get("HousingCapacity20x"),
                Location = new Point(25, 150),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkHousingCapacity20x);

            chkStorageCapacity10x = new ModernToggle {
                Text = Loc.Get("StorageCapacity10x"),
                Location = new Point(25, 220),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkStorageCapacity10x);

            chkHqHp10x = new ModernToggle {
                Text = Loc.Get("HqHp10x"),
                Location = new Point(25, 256),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkHqHp10x);

            chkFastCiviProduction = new ModernToggle {
                Text = Loc.Get("FastCiviProduction"),
                Location = new Point(25, 290),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkFastCiviProduction);

            chkFastBuildUpgradeRepair = new ModernToggle {
                Text = Loc.Get("FastBuildUpgradeRepair"),
                Location = new Point(25, 360),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkFastBuildUpgradeRepair);

            chkFoodHealing10x = new ModernToggle {
                Text = Loc.Get("FoodHealing10x"),
                Location = new Point(25, 430),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkFoodHealing10x);

            chkVillageBuildRange = new ModernToggle {
                Text = Loc.Get("VillageBuildRange"),
                Location = new Point(25, 500),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            pnlBuildCard.Controls.Add(chkVillageBuildRange);

            // AI 終極模式已拆成 5 個可獨立勾選的模組（對應 EndlessAiOrchestrator M1..M5），
            // 集中放在專屬的整列卡片（設定頁第 2 列，橫跨三欄），由 ConfigureAiCardHorizontal
            // 以響應式網格橫向排列，說明文字改用滑鼠停留提示（tooltip）掛在各開關上。
            pnlAiCard = new Panel {
                Location = new Point(0, 0),
                Size = new Size(1180, 150)
            };
            // pnlAiCard.Paint += CardPanel_Paint;

            lblAiTitle = new Label {
                Text = Loc.Get("AiCardTitle"),
                Location = new Point(20, 14),
                Size = new Size(400, 24),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlAiCard.Controls.Add(lblAiTitle);

            chkAiM1 = new ModernToggle { Text = Loc.Get("AiM1"), Size = new Size(260, 26), Checked = false, BackColor = Color.Transparent, Font = fontJhengHei10B };
            chkAiM2 = new ModernToggle { Text = Loc.Get("AiM2"), Size = new Size(260, 26), Checked = false, BackColor = Color.Transparent, Font = fontJhengHei10B };
            chkAiM3 = new ModernToggle { Text = Loc.Get("AiM3"), Size = new Size(260, 26), Checked = false, BackColor = Color.Transparent, Font = fontJhengHei10B };
            chkAiM4 = new ModernToggle { Text = Loc.Get("AiM4"), Size = new Size(260, 26), Checked = false, BackColor = Color.Transparent, Font = fontJhengHei10B };
            chkAiM5 = new ModernToggle { Text = Loc.Get("AiM5"), Size = new Size(260, 26), Checked = false, BackColor = Color.Transparent, Font = fontJhengHei10B };
            chkAiM6 = new ModernToggle { Text = Loc.Get("AiM6"), Size = new Size(260, 26), Checked = false, BackColor = Color.Transparent, Font = fontJhengHei10B };
            pnlAiCard.Controls.Add(chkAiM1);
            pnlAiCard.Controls.Add(chkAiM2);
            pnlAiCard.Controls.Add(chkAiM3);
            pnlAiCard.Controls.Add(chkAiM4);
            pnlAiCard.Controls.Add(chkAiM5);
            pnlAiCard.Controls.Add(chkAiM6);

            tabSystem.Controls.Add(pnlNumericCard);
            tabSystem.Controls.Add(pnlSwitchesCard);
            tabSystem.Controls.Add(pnlBuildCard);

            lblGamePath = new Label {
                Text = "遊戲路徑:",
                Location = new Point(15, 330),
                Size = new Size(190, 20),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(150, 160, 175),
                BackColor = Color.Transparent
            };
            pnlRightSidebar.Controls.Add(lblGamePath);

            Panel pnlGamePath = CreateInputWrapper(10, 355, 200, 28);
            txtGamePath = new TextBox {
                Location = new Point(5, 5),
                Size = new Size(190, 20),
                BorderStyle = BorderStyle.None,
                BackColor = Color.FromArgb(38, 38, 48),
                ForeColor = Color.White
            };
            pnlGamePath.Controls.Add(txtGamePath);
            pnlRightSidebar.Controls.Add(pnlGamePath);

            btnBrowseGamePath = new Button {
                Text = "瀏覽...",
                Location = new Point(10, 390),
                Size = new Size(200, 30)
            };
            StyleButton(btnBrowseGamePath, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnBrowseGamePath.Click += new EventHandler(BtnBrowseGamePath_Click);
            pnlRightSidebar.Controls.Add(btnBrowseGamePath);

            string detectedPath = DetectGamePathFromRegistry();
            if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe"))) {
                txtGamePath.Text = AppContext.BaseDirectory;
            } else if (!string.IsNullOrEmpty(detectedPath)) {
                txtGamePath.Text = detectedPath;
            } else if (Directory.Exists(@"C:\Program Files (x86)\Against Rome")) {
                txtGamePath.Text = @"C:\Program Files (x86)\Against Rome";
            }

            menuRestore = new ContextMenuStrip { Renderer = new DarkContextMenuRenderer() };
            itemRestoreAll = new ToolStripMenuItem("全部還原");
            itemRestoreStats = new ToolStripMenuItem("僅還原兵種屬性");
            itemRestoreCompat = new ToolStripMenuItem("僅還原相容性修正");
            itemRestoreLang = new ToolStripMenuItem("僅還原語系設定");

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
                Location = new Point(10, 440),
                Size = new Size(200, 40)
            };
            StyleButton(btnLoadCurrent, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnLoadCurrent.Click += new EventHandler(BtnLoadCurrent_Click);

            btnRestore = new Button {
                Text = "恢復原版",
                Location = new Point(10, 490),
                Size = new Size(200, 40)
            };
            StyleButton(btnRestore, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnRestore.Click += (s, e) => {
                menuRestore.Show(btnRestore, new Point(0, btnRestore.Height));
            };

            btnApply = new Button {
                Text = "執行修改",
                Location = new Point(10, 550),
                Size = new Size(200, 40)
            };
            StyleButton(btnApply, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnApply.Click += new EventHandler(BtnApply_Click);

            btnStartGame = new Button {
                Text = "啟動遊戲",
                Location = new Point(10, 600),
                Size = new Size(200, 40)
            };
            StyleButton(btnStartGame, Color.FromArgb(0, 180, 120), Color.White, Color.FromArgb(0, 220, 150));
            btnStartGame.Click += new EventHandler(BtnStartGame_Click);

            pnlRightSidebar.Controls.Add(btnLoadCurrent);
            pnlRightSidebar.Controls.Add(btnRestore);
            pnlRightSidebar.Controls.Add(btnApply);
            pnlRightSidebar.Controls.Add(btnStartGame);

            Panel pnlDefaultStatsTitle = new Panel {
                Location = new Point(0, 0),
                Size = new Size(1190, 65)
            };
            pnlDefaultStatsTitle.Paint += CardPanel_Paint;

            lblDefaultStatsTitle = new Label {
                Text = Loc.Get("DefaultStatsTitle"),
                Location = new Point(20, 20),
                Size = new Size(280, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };

            lblTroopTemplate = new Label {
                Text = Loc.Get("TroopTemplateLabel"),
                Location = new Point(315, 20),
                Size = new Size(90, 25),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(200, 205, 210),
                BackColor = Color.Transparent
            };

            cbTroopTemplate = new ComboBox {
                Location = new Point(410, 16),
                Size = new Size(190, 28),
                DropDownStyle = ComboBoxStyle.DropDownList,
                BackColor = Color.FromArgb(32, 32, 40),
                ForeColor = Color.White,
                Font = fontJhengHei95B,
                FlatStyle = FlatStyle.Flat
            };
            RefreshTroopTemplateItems();
            cbTroopTemplate.SelectedIndexChanged += CbTroopTemplate_SelectedIndexChanged;

            btnTroopPreset = new Button {
                Text = Loc.Get("BtnTroopPreset"),
                Location = new Point(615, 14),
                Size = new Size(130, 30),
                Font = fontJhengHei95B
            };
            StyleButton(btnTroopPreset, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnTroopPreset.Click += new EventHandler(BtnTroopPreset_Click);

            lblTroopPresetFile = new Label {
                Text = Loc.Get("TroopPresetDefault"),
                Location = new Point(760, 18),
                Size = new Size(410, 25),
                Font = fontJhengHei9R,
                ForeColor = Color.FromArgb(160, 165, 170),
                BackColor = Color.Transparent
            };

            pnlDefaultStatsTitle.Controls.Add(lblDefaultStatsTitle);
            pnlDefaultStatsTitle.Controls.Add(lblTroopTemplate);
            pnlDefaultStatsTitle.Controls.Add(cbTroopTemplate);
            pnlDefaultStatsTitle.Controls.Add(btnTroopPreset);
            pnlDefaultStatsTitle.Controls.Add(lblTroopPresetFile);

            defaultStatsTabControl = new ModernTabControl {
                Location = new Point(0, 80),
                Size = new Size(1190, 715)
            };

            tabDefaultRoman = new TabPage { Text = " 羅馬 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };
            tabDefaultTeuton = new TabPage { Text = " 條頓 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };
            tabDefaultCelt = new TabPage { Text = " 塞爾特 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };
            tabDefaultHun = new TabPage { Text = " 匈奴 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };

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

            lblCurrentStatsTitle = new Label {
                Text = "當前兵種數值 (原版與當前對比)",
                Location = new Point(20, 20),
                Size = new Size(350, 25),
                Font = fontJhengHei105B,
                ForeColor = Color.FromArgb(0, 220, 255),
                BackColor = Color.Transparent
            };
            pnlCurrentStatsTitle.Controls.Add(lblCurrentStatsTitle);

            currentStatsTabControl = new ModernTabControl {
                Location = new Point(0, 80),
                Size = new Size(1190, 715)
            };

            tabCurrentRoman = new TabPage { Text = " 羅馬 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };
            tabCurrentTeuton = new TabPage { Text = " 條頓 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };
            tabCurrentCelt = new TabPage { Text = " 塞爾特 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };
            tabCurrentHun = new TabPage { Text = " 匈奴 ", BackColor = Color.FromArgb(10, 11, 16), UseVisualStyleBackColor = false };

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
                BackColor = Color.FromArgb(10, 11, 16),
                UseVisualStyleBackColor = false
            };
            mainTabControl.TabPages.Add(tabDoc);

            txtDoc = new TextBox {
                Dock = DockStyle.Fill,
                Multiline = true,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical,
                BackColor = Color.FromArgb(15, 16, 24),
                ForeColor = Color.FromArgb(230, 235, 240),
                Font = fontJhengHei105R,
                BorderStyle = BorderStyle.None
            };
            ReloadTechnicalDocument();
            tabDoc.Controls.Add(txtDoc);

            tabSaveManager = new TabPage {
                BackColor = Color.FromArgb(10, 11, 16),
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

            lblGameSavesTitle = new Label {
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

            btnBackupSave = new Button {
                Text = "備份此存檔",
                Location = new Point(15, 330),
                Size = new Size(140, 35)
            };
            StyleButton(btnBackupSave, Color.FromArgb(45, 45, 55), Color.FromArgb(0, 220, 255), Color.FromArgb(0, 220, 255));
            btnBackupSave.Click += BtnBackupSave_Click;
            pnlGameSavesCard.Controls.Add(btnBackupSave);

            btnDeleteSave = new Button {
                Text = "刪除此存檔",
                Location = new Point(165, 330),
                Size = new Size(140, 35)
            };
            StyleButton(btnDeleteSave, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnDeleteSave.Click += BtnDeleteSave_Click;
            pnlGameSavesCard.Controls.Add(btnDeleteSave);

            btnRefreshSaves = new Button {
                Text = "重新整理",
                Location = new Point(315, 330),
                Size = new Size(140, 35)
            };
            StyleButton(btnRefreshSaves, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnRefreshSaves.Click += (s, e) => RefreshSavesAndBackups();
            pnlGameSavesCard.Controls.Add(btnRefreshSaves);

            lblBackupsTitle = new Label {
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

            btnRestoreBackup = new Button {
                Text = "還原此備份",
                Location = new Point(15, 345),
                Size = new Size(140, 35)
            };
            StyleButton(btnRestoreBackup, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnRestoreBackup.Click += BtnRestoreBackup_Click;
            pnlBackupsCard.Controls.Add(btnRestoreBackup);

            btnDeleteBackup = new Button {
                Text = "刪除此備份",
                Location = new Point(165, 345),
                Size = new Size(140, 35)
            };
            StyleButton(btnDeleteBackup, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(255, 75, 75));
            btnDeleteBackup.Click += BtnDeleteBackup_Click;
            pnlBackupsCard.Controls.Add(btnDeleteBackup);

            lblDetailTitle = new Label {
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
            this.Controls.Add(pnlRightSidebar);
            this.Controls.Add(mainTabControl);
            ApplyModernLayout();
            ShowTabPage(tabSystem);
        }


        /// <summary>
        /// 自訂導覽列按鈕繪製樣式，包含 Hover 漸層與選取指示條
        /// </summary>
        private void StyleNavButton(Button btn, string key, TabPage associatedPage) {
            btn.Text = ""; // 採用 Paint 自繪，清除原生文字
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 0;
            btn.BackColor = Color.Transparent;
            btn.Cursor = Cursors.Hand;
            btn.Size = new Size(200, 45);

            btn.Paint += (s, e) => {
                Graphics g = e.Graphics;
                g.SmoothingMode = SmoothingMode.AntiAlias;
                bool isSelected = key switch {
                    "NavSystem" => mainTabControl.SelectedTab == tabSystem,
                    "NavDefaultStats" => mainTabControl.SelectedTab == tabDefaultStats,
                    "NavCurrentStats" => mainTabControl.SelectedTab == tabCurrentStats,
                    "NavSaveManager" => mainTabControl.SelectedTab == tabSaveManager,
                    "NavDoc" => mainTabControl.SelectedTab == tabDoc,
                    _ => mainTabControl.SelectedTab == associatedPage
                };
                
                Point clientPos = btn.PointToClient(Cursor.Position);
                bool isHovered = btn.ClientRectangle.Contains(clientPos);

                // 1. 繪製背景 (選取時有漂亮的漸層藍色，Hover 時有微亮背景，均帶有精緻圓角)
                int radius = 4;
                Rectangle contentRect = new Rectangle(5, 2, btn.Width - 10, btn.Height - 4);
                using (GraphicsPath path = GetRoundPath(contentRect, radius)) {
                    if (isSelected) {
                        using (var brush = new LinearGradientBrush(contentRect, Color.FromArgb(24, 25, 38), Color.FromArgb(16, 28, 40), 45F)) {
                            g.FillPath(brush, path);
                        }
                    } else if (isHovered) {
                        using (var brush = new SolidBrush(Color.FromArgb(20, 21, 31))) {
                            g.FillPath(brush, path);
                        }
                    }
                }

                // 2. 繪製文字與圖示 (Unicode 圖示)
                string text = Loc.Get(key);
                Color foreColor = isSelected ? Color.FromArgb(0, 230, 255) : (isHovered ? Color.White : Color.FromArgb(120, 125, 140));
                using (var brush = new SolidBrush(foreColor)) {
                    SizeF sz = g.MeasureString(text, fontJhengHei10B);
                    g.DrawString(text, fontJhengHei10B, brush, 22, (btn.Height - sz.Height) / 2);
                }

                // 3. 繪製左側發光指示條 (融入圓角邊緣)
                if (isSelected) {
                    using (var brush = new SolidBrush(Color.FromArgb(0, 230, 255))) {
                        g.FillRectangle(brush, 8, 10, 3, btn.Height - 20);
                    }
                }
            };

            // 註冊滑鼠進出事件以即時重繪 Hover 狀態
            //（Paint 內以游標位置判斷 hover，不需要在 MouseMove 每次移動都整鈕重繪）
            btn.MouseEnter += (s, e) => btn.Invalidate();
            btn.MouseLeave += (s, e) => btn.Invalidate();
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
