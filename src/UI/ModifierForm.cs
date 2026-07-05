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

        // 記憶體原版檔案備份字典，用以在修改時直接讀取乾淨數據，避免疊加修改
        private Dictionary<string, byte[]> backupFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

        // 快取的備份單兵屬性欄位字典 (以兵種名稱為 Key)
        private Dictionary<string, string[]> _backupUnitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
        private bool _backupUnitRowsParsed = false;
        private readonly object _backupUnitRowsLock = new object();

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
        private Label lblHelpFocusLoss = null!;
        private Label lblHelpToEng = null!;
        private Label lblHelpDgVoodoo = null!;
        private Label lblHelpFreeProd = null!;
        private Label lblHelpFreeUpgrade = null!;
        private Label lblHelpNoSpellCost = null!;
        private Label lblHelpInfiniteMorale = null!;
        private Label lblHelpBalance = null!;
        private Label lblHelpNoSpellAltar = null!;
        private Label lblHelpMaxPopulation = null!;
        private Label lblHelpHousingCapacity20x = null!;
        private Label lblHelpStorageCapacity10x = null!;
        private Label lblHelpFastCiviProduction = null!;
        private Label lblHelpFastBuildUpgradeRepair = null!;
        private Label lblHelpFoodHealing10x = null!;
        private Label lblHelpVillageBuildRange = null!;
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
        private static readonly Color ColorBtnDefault = Color.FromArgb(37, 43, 55);
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

            Log(Loc.Get("LogConstructCompleted"));
            // 將內嵌的 Backup.zip 載入記憶體
            LoadBackupZipToMemory();
            // 初始化資料與讀取自訂兵種資訊
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
            pnlNumericCard.Paint += CardPanel_Paint;

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
            lblHelpFocusLoss = CreateHelpLabel("FocusLossTip");
            lblHelpFocusLoss.Location = new Point(340, 80);
            pnlNumericCard.Controls.Add(chkFocusLoss);
            pnlNumericCard.Controls.Add(lblHelpFocusLoss);

            chkToEng = new ModernToggle {
                Text = "強制英文語系 (介面圖示與核心文字)",
                Location = new Point(25, 160),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpToEng = CreateHelpLabel("ToEngTip");
            lblHelpToEng.Location = new Point(340, 160);
            pnlNumericCard.Controls.Add(chkToEng);
            pnlNumericCard.Controls.Add(lblHelpToEng);

            chkDgVoodoo = new ModernToggle {
                Text = Loc.Get("DgVoodoo"),
                Location = new Point(25, 240),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpDgVoodoo = CreateHelpLabel("DgVoodooTip");
            lblHelpDgVoodoo.Location = new Point(340, 240);
            pnlNumericCard.Controls.Add(chkDgVoodoo);
            pnlNumericCard.Controls.Add(lblHelpDgVoodoo);

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
            pnlSwitchesCard.Paint += CardPanel_Paint;

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
            lblHelpFreeProd = CreateHelpLabel("FreeProdTip");
            lblHelpFreeProd.Location = new Point(340, 80);
            pnlSwitchesCard.Controls.Add(chkFreeProd);
            pnlSwitchesCard.Controls.Add(lblHelpFreeProd);

            chkFreeUpgrade = new ModernToggle {
                Text = "陣型、研發、屬性解鎖升級免費",
                Location = new Point(25, 160),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpFreeUpgrade = CreateHelpLabel("FreeUpgradeTip");
            lblHelpFreeUpgrade.Location = new Point(340, 160);
            pnlSwitchesCard.Controls.Add(chkFreeUpgrade);
            pnlSwitchesCard.Controls.Add(lblHelpFreeUpgrade);

            chkNoSpellCost = new ModernToggle {
                Text = "祭司與賢者法術無消耗 (MP 零消耗)",
                Location = new Point(25, 240),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpNoSpellCost = CreateHelpLabel("NoSpellCostTip");
            lblHelpNoSpellCost.Location = new Point(340, 240);
            pnlSwitchesCard.Controls.Add(chkNoSpellCost);
            pnlSwitchesCard.Controls.Add(lblHelpNoSpellCost);

            chkInfiniteMorale = new ModernToggle {
                Text = "部隊無限士氣 (士氣不減且極速恢復)",
                Location = new Point(25, 320),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpInfiniteMorale = CreateHelpLabel("InfiniteMoraleTip");
            lblHelpInfiniteMorale.Location = new Point(340, 320);
            pnlSwitchesCard.Controls.Add(chkInfiniteMorale);
            pnlSwitchesCard.Controls.Add(lblHelpInfiniteMorale);

            chkBalance = new ModernToggle {
                Text = Loc.Get("EnableBalance"),
                Location = new Point(25, 400),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            chkBalance.CheckedChanged += new EventHandler(ChkBalance_CheckedChanged);
            lblHelpBalance = CreateHelpLabel("BalanceTip");
            lblHelpBalance.Location = new Point(340, 400);
            pnlSwitchesCard.Controls.Add(chkBalance);
            pnlSwitchesCard.Controls.Add(lblHelpBalance);

            chkNoSpellAltar = new ModernToggle {
                Text = "法術免除祭壇數量需求",
                Location = new Point(25, 480),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpNoSpellAltar = CreateHelpLabel("NoSpellAltarTip");
            lblHelpNoSpellAltar.Location = new Point(340, 480);
            pnlSwitchesCard.Controls.Add(chkNoSpellAltar);
            pnlSwitchesCard.Controls.Add(lblHelpNoSpellAltar);

            // 新增：建設與人口修改卡片
            pnlBuildCard = new Panel {
                Location = new Point(805, 0),
                Size = new Size(385, 790)
            };
            pnlBuildCard.Paint += CardPanel_Paint;

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
            lblHelpMaxPopulation = CreateHelpLabel("MaxPopulationTip");
            lblHelpMaxPopulation.Location = new Point(340, 80);
            pnlBuildCard.Controls.Add(chkMaxPopulation);
            pnlBuildCard.Controls.Add(lblHelpMaxPopulation);

            chkHousingCapacity20x = new ModernToggle {
                Text = Loc.Get("HousingCapacity20x"),
                Location = new Point(25, 150),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpHousingCapacity20x = CreateHelpLabel("HousingCapacity20xTip");
            lblHelpHousingCapacity20x.Location = new Point(340, 150);
            pnlBuildCard.Controls.Add(chkHousingCapacity20x);
            pnlBuildCard.Controls.Add(lblHelpHousingCapacity20x);

            chkStorageCapacity10x = new ModernToggle {
                Text = Loc.Get("StorageCapacity10x"),
                Location = new Point(25, 220),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpStorageCapacity10x = CreateHelpLabel("StorageCapacity10xTip");
            lblHelpStorageCapacity10x.Location = new Point(340, 220);
            pnlBuildCard.Controls.Add(chkStorageCapacity10x);
            pnlBuildCard.Controls.Add(lblHelpStorageCapacity10x);

            chkFastCiviProduction = new ModernToggle {
                Text = Loc.Get("FastCiviProduction"),
                Location = new Point(25, 290),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpFastCiviProduction = CreateHelpLabel("FastCiviProductionTip");
            lblHelpFastCiviProduction.Location = new Point(340, 290);
            pnlBuildCard.Controls.Add(chkFastCiviProduction);
            pnlBuildCard.Controls.Add(lblHelpFastCiviProduction);

            chkFastBuildUpgradeRepair = new ModernToggle {
                Text = Loc.Get("FastBuildUpgradeRepair"),
                Location = new Point(25, 360),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpFastBuildUpgradeRepair = CreateHelpLabel("FastBuildUpgradeRepairTip");
            lblHelpFastBuildUpgradeRepair.Location = new Point(340, 360);
            pnlBuildCard.Controls.Add(chkFastBuildUpgradeRepair);
            pnlBuildCard.Controls.Add(lblHelpFastBuildUpgradeRepair);

            chkFoodHealing10x = new ModernToggle {
                Text = Loc.Get("FoodHealing10x"),
                Location = new Point(25, 430),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpFoodHealing10x = CreateHelpLabel("FoodHealing10xTip");
            lblHelpFoodHealing10x.Location = new Point(340, 430);
            pnlBuildCard.Controls.Add(chkFoodHealing10x);
            pnlBuildCard.Controls.Add(lblHelpFoodHealing10x);

            chkVillageBuildRange = new ModernToggle {
                Text = Loc.Get("VillageBuildRange"),
                Location = new Point(25, 500),
                Size = new Size(310, 25),
                Checked = false,
                BackColor = Color.Transparent,
                Font = fontJhengHei10B
            };
            lblHelpVillageBuildRange = CreateHelpLabel("VillageBuildRangeTip");
            lblHelpVillageBuildRange.Location = new Point(340, 500);
            pnlBuildCard.Controls.Add(chkVillageBuildRange);
            pnlBuildCard.Controls.Add(lblHelpVillageBuildRange);

            // AI 終極模式已拆成 6 個可獨立勾選的模組（對應 EndlessAiOrchestrator M1..M6），
            // 集中放在專屬的整列卡片（設定頁第 2 列，橫跨三欄），由 ConfigureAiCardHorizontal
            // 以響應式網格橫向排列，說明文字改用滑鼠停留提示（tooltip）掛在各開關上。
            pnlAiCard = new Panel {
                Location = new Point(0, 0),
                Size = new Size(1180, 150)
            };
            pnlAiCard.Paint += CardPanel_Paint;

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

        private void ApplyModernLayout() {
            SuspendLayout();

            Size = new Size(1600, 860);
            MinimumSize = new Size(1400, 760);
            AutoScroll = false;
            BackColor = Color.FromArgb(9, 12, 18);

            pnlTitleBar.Height = 56;
            pnlTitleBar.BackColor = Color.FromArgb(13, 17, 25);
            lblMainTitle.Location = new Point(24, 16);
            lblMainTitle.Size = new Size(360, 26);
            lblMainTitle.ForeColor = Color.FromArgb(226, 241, 252);


            pnlSidebar.BackColor = Color.FromArgb(12, 16, 24);
            pnlSidebar.Width = 250;

            pnlRightSidebar.BackColor = Color.FromArgb(12, 16, 24);
            pnlRightSidebar.Width = 250;

            ConfigureSidebarLayout();
            ConfigureSystemDashboard();
            ConfigureStatsPages();
            ConfigureSaveManagerLayout();

            tabDoc.Padding = new Padding(14);
            txtDoc.BackColor = Color.FromArgb(16, 20, 29);
            txtDoc.ForeColor = Color.FromArgb(210, 218, 230);

            foreach (DataGridView grid in defaultStatsGrids.Values.Concat(currentStatsGrids.Values)) {
                grid.ScrollBars = ScrollBars.Both;
            }

            LayoutModernShell();
            Resize += (s, e) => LayoutModernShell();
            ResumeLayout(true);
        }

        private void LayoutModernShell() {
            pnlTitleBar.Location = Point.Empty;
            pnlTitleBar.Size = new Size(ClientSize.Width, 56);
            btnClose.Location = new Point(ClientSize.Width - 46, 12);
            btnMinimize.Location = new Point(ClientSize.Width - 86, 12);

            pnlSidebar.Location = new Point(0, 56);
            pnlSidebar.Size = new Size(250, Math.Max(0, ClientSize.Height - 56));

            pnlRightSidebar.Location = new Point(ClientSize.Width - 250, 56);
            pnlRightSidebar.Size = new Size(250, Math.Max(0, ClientSize.Height - 56));

            mainTabControl.Location = new Point(266, 70);
            mainTabControl.Size = new Size(
                Math.Max(0, ClientSize.Width - 532),
                Math.Max(0, ClientSize.Height - 84));

            lblSidebarLang.Location = new Point(16, Math.Max(610, pnlSidebar.Height - 72));
            btnLangZH.Location = new Point(16, Math.Max(634, pnlSidebar.Height - 46));
            btnLangEN.Location = new Point(126, Math.Max(634, pnlSidebar.Height - 46));
        }

        private void ConfigureSidebarLayout() {
            Button[] navButtons = {
                btnNavSystem,
                btnNavDefaultStats,
                btnNavCurrentStats,
                btnNavSaveManager,
                btnNavDoc
            };
            for (int i = 0; i < navButtons.Length; i++) {
                navButtons[i].Location = new Point(10, 22 + i * 52);
                navButtons[i].Size = new Size(230, 44);
            }

            lblGamePath.Location = new Point(16, 22);
            lblGamePath.Size = new Size(218, 20);
            lblGamePath.ForeColor = Color.FromArgb(128, 143, 163);

            Panel pathWrapper = txtGamePath.Parent as Panel
                ?? throw new InvalidOperationException("Game path input wrapper was not initialized.");
            pathWrapper.Location = new Point(16, 48);
            pathWrapper.Size = new Size(218, 32);
            pathWrapper.BackColor = Color.FromArgb(22, 28, 39);
            txtGamePath.Location = new Point(9, 7);
            txtGamePath.Size = new Size(200, 20);
            txtGamePath.BackColor = pathWrapper.BackColor;
            txtGamePath.ForeColor = Color.FromArgb(222, 230, 240);

            btnBrowseGamePath.Location = new Point(16, 88);
            btnBrowseGamePath.Size = new Size(218, 34);
            btnLoadCurrent.Location = new Point(16, 148);
            btnRestore.Location = new Point(16, 198);
            btnApply.Location = new Point(16, 258);
            btnStartGame.Location = new Point(16, 308);
            foreach (Button actionButton in new[] { btnLoadCurrent, btnRestore, btnApply, btnStartGame }) {
                actionButton.Size = new Size(218, 40);
            }

            lblSidebarLang.Size = new Size(218, 20);
            lblSidebarLang.ForeColor = Color.FromArgb(128, 143, 163);
            btnLangZH.Size = new Size(102, 30);
            btnLangEN.Size = new Size(108, 30);
        }

        private void ConfigureSystemDashboard() {
            tabSystem.BackColor = Color.FromArgb(9, 12, 18);

            Panel header = new Panel {
                Dock = DockStyle.Top,
                Height = 72,
                BackColor = Color.FromArgb(9, 12, 18)
            };
            lblSystemHeading = new Label {
                Text = Loc.Get("SystemHeading"),
                Location = new Point(4, 4),
                Size = new Size(430, 28),
                Font = fontJhengHei115B,
                ForeColor = Color.FromArgb(235, 242, 250),
                BackColor = Color.Transparent
            };
            lblSystemSubtitle = new Label {
                Text = Loc.Get("SystemSubtitle"),
                Location = new Point(4, 35),
                Size = new Size(620, 22),
                Font = fontJhengHei9R,
                ForeColor = Color.FromArgb(128, 143, 163),
                BackColor = Color.Transparent
            };
            header.Controls.Add(lblSystemHeading);
            header.Controls.Add(lblSystemSubtitle);
            header.Controls.Add(btnEnableAll);
            header.Controls.Add(btnDisableAll);
            btnEnableAll.Size = new Size(136, 36);
            btnDisableAll.Size = new Size(136, 36);
            header.Resize += (s, e) => {
                btnDisableAll.Location = new Point(Math.Max(0, header.Width - 140), 12);
                btnEnableAll.Location = new Point(Math.Max(0, header.Width - 284), 12);
            };

            TableLayoutPanel settingsLayout = new TableLayoutPanel {
                Dock = DockStyle.Fill,
                BackColor = Color.FromArgb(9, 12, 18),
                ColumnCount = 3,
                RowCount = 2,
                // The page header is intentionally layered above this fill panel.
                // Reserve its height so card titles and first rows are never obscured.
                Padding = new Padding(0, 84, 0, 0)
            };
            settingsLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 33.333F));
            settingsLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 33.334F));
            settingsLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 33.333F));
            // 第 1 列：三張既有卡片；第 2 列：整列 AI 終極模式卡片（橫跨三欄）。
            settingsLayout.RowStyles.Add(new RowStyle(SizeType.Absolute, 560F));
            settingsLayout.RowStyles.Add(new RowStyle(SizeType.Absolute, 128F));

            ConfigureSettingsCard(pnlNumericCard, lblNumericTitle, 230,
                (chkFocusLoss, lblHelpFocusLoss),
                (chkToEng, lblHelpToEng),
                (chkDgVoodoo, lblHelpDgVoodoo));
            ConfigureSettingsCard(pnlSwitchesCard, lblSwitchesTitle, 492,
                (chkFreeProd, lblHelpFreeProd),
                (chkFreeUpgrade, lblHelpFreeUpgrade),
                (chkNoSpellCost, lblHelpNoSpellCost),
                (chkInfiniteMorale, lblHelpInfiniteMorale),
                (chkBalance, lblHelpBalance),
                (chkNoSpellAltar, lblHelpNoSpellAltar));
            ConfigureSettingsCard(pnlBuildCard, lblBuildTitle, 470,
                (chkMaxPopulation, lblHelpMaxPopulation),
                (chkHousingCapacity20x, lblHelpHousingCapacity20x),
                (chkStorageCapacity10x, lblHelpStorageCapacity10x),
                (chkFastCiviProduction, lblHelpFastCiviProduction),
                (chkFastBuildUpgradeRepair, lblHelpFastBuildUpgradeRepair),
                (chkFoodHealing10x, lblHelpFoodHealing10x),
                (chkVillageBuildRange, lblHelpVillageBuildRange));
            ConfigureAiCardHorizontal(pnlAiCard, lblAiTitle,
                chkAiM1, chkAiM2, chkAiM3, chkAiM4, chkAiM5, chkAiM6);

            settingsLayout.Controls.Add(pnlNumericCard, 0, 0);
            settingsLayout.Controls.Add(pnlSwitchesCard, 1, 0);
            settingsLayout.Controls.Add(pnlBuildCard, 2, 0);
            settingsLayout.Controls.Add(pnlAiCard, 0, 1);
            settingsLayout.SetColumnSpan(pnlAiCard, 3);
            tabSystem.Controls.Add(settingsLayout);
            tabSystem.Controls.Add(header);
            header.BringToFront();
        }

        private void ConfigureSettingsCard(
            Panel card,
            Label title,
            int height,
            params (ModernToggle Toggle, Label Help)[] rows) {
            card.Height = height;
            card.MinimumSize = new Size(0, height);
            card.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            card.Margin = new Padding(6, 0, 6, 0);
            card.BackColor = Color.FromArgb(18, 22, 31);

            title.Location = new Point(20, 17);
            title.Size = new Size(280, 24);
            title.ForeColor = Color.FromArgb(105, 205, 255);

            void LayoutRows() {
                for (int i = 0; i < rows.Length; i++) {
                    ModernToggle toggle = rows[i].Toggle;
                    Label help = rows[i].Help;
                    int y = 60 + i * 48;
                    toggle.Location = new Point(20, y);
                    toggle.Size = new Size(Math.Max(120, card.Width - 66), 26);
                    toggle.Font = fontJhengHei95R;
                    toggle.BackColor = card.BackColor;
                    help.Location = new Point(Math.Max(20, card.Width - 40), y + 2);
                }
            }

            card.Resize += (s, e) => LayoutRows();
            LayoutRows();
        }

        // AI 終極模式整列卡片：把 N 個開關以響應式網格橫向排列（每格約 250px，寬度不足時自動換行）。
        // 說明文字改用掛在開關上的 tooltip（在語言套用流程統一設定），故此處不需要 help 圖示。
        private void ConfigureAiCardHorizontal(Panel card, Label title, params ModernToggle[] toggles) {
            card.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
            card.Margin = new Padding(6, 6, 6, 0);
            card.BackColor = Color.FromArgb(18, 22, 31);

            title.Location = new Point(20, 14);
            title.Size = new Size(400, 24);
            title.ForeColor = Color.FromArgb(105, 205, 255);

            void LayoutGrid() {
                const int padX = 20, gapX = 16, gapY = 12, top = 52, cellH = 26, cellTarget = 180;
                int avail = Math.Max(cellTarget, card.Width - padX * 2);
                int cols = Math.Max(1, Math.Min(toggles.Length, (avail + gapX) / (cellTarget + gapX)));
                int cellW = (avail - gapX * (cols - 1)) / cols;
                for (int i = 0; i < toggles.Length; i++) {
                    int r = i / cols, c = i % cols;
                    toggles[i].Location = new Point(padX + c * (cellW + gapX), top + r * (cellH + gapY));
                    toggles[i].Size = new Size(cellW, cellH);
                    toggles[i].Font = fontJhengHei95R;
                    toggles[i].BackColor = card.BackColor;
                }
            }

            card.Resize += (s, e) => LayoutGrid();
            LayoutGrid();
        }

        private void ConfigureStatsPages() {
            Panel defaultHeader = lblDefaultStatsTitle.Parent as Panel
                ?? throw new InvalidOperationException("Default stats header was not initialized.");
            ConfigureStatsPage(tabDefaultStats, defaultHeader, defaultStatsTabControl);

            defaultHeader.Resize += (s, e) => {
                lblTroopPresetFile.Width = Math.Max(120, defaultHeader.Width - lblTroopPresetFile.Left - 18);
            };

            Panel currentHeader = lblCurrentStatsTitle.Parent as Panel
                ?? throw new InvalidOperationException("Current stats header was not initialized.");
            ConfigureStatsPage(tabCurrentStats, currentHeader, currentStatsTabControl);

            void ConfigureStatsPage(TabPage page, Panel header, TabControl statsTabs) {
                // Keep the title card and the tab content in separate layout rows. A Fill-docked
                // TabControl placed behind a Top-docked header still starts at y=0, which causes
                // the header to cover the faction tabs and most of the grid column headings.
                page.Controls.Remove(header);
                page.Controls.Remove(statsTabs);

                var layout = new TableLayoutPanel {
                    Dock = DockStyle.Fill,
                    BackColor = page.BackColor,
                    ColumnCount = 1,
                    RowCount = 3,
                    Margin = new Padding(0),
                    Padding = new Padding(0)
                };
                layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100F));
                layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 68F));
                layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 40F));
                layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));

                header.Dock = DockStyle.Fill;
                header.Margin = new Padding(0);
                header.BackColor = Color.FromArgb(18, 22, 31);
                statsTabs.Dock = DockStyle.Fill;
                statsTabs.Margin = new Padding(0);
                statsTabs.ItemSize = new Size(0, 1);
                statsTabs.Font = fontJhengHei95R;
                if (statsTabs is ModernTabControl modernTabs) {
                    modernTabs.HideTabs = true;
                }

                var factionBar = new TableLayoutPanel {
                    Dock = DockStyle.Fill,
                    BackColor = Color.FromArgb(14, 17, 24),
                    ColumnCount = statsTabs.TabCount,
                    RowCount = 1,
                    Margin = new Padding(0),
                    Padding = new Padding(0)
                };
                factionBar.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));

                var factionButtons = new List<Button>();
                for (int i = 0; i < statsTabs.TabCount; i++) {
                    int tabIndex = i;
                    factionBar.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100F / statsTabs.TabCount));

                    var button = new Button {
                        Dock = DockStyle.Fill,
                        FlatStyle = FlatStyle.Flat,
                        BackColor = Color.FromArgb(14, 17, 24),
                        Cursor = Cursors.Hand,
                        Margin = new Padding(0),
                        TabStop = false,
                        UseVisualStyleBackColor = false
                    };
                    button.FlatAppearance.BorderSize = 0;
                    button.Paint += (s, e) => {
                        bool selected = statsTabs.SelectedIndex == tabIndex;
                        Color background = selected
                            ? Color.FromArgb(26, 31, 43)
                            : Color.FromArgb(14, 17, 24);
                        e.Graphics.Clear(background);

                        if (tabIndex > 0) {
                            using (var divider = new Pen(Color.FromArgb(38, 44, 58))) {
                                e.Graphics.DrawLine(divider, 0, 8, 0, button.Height - 8);
                            }
                        }
                        if (selected) {
                            using (var indicator = new SolidBrush(Color.FromArgb(62, 203, 255))) {
                                e.Graphics.FillRectangle(indicator, 12, button.Height - 3, button.Width - 24, 3);
                            }
                        }

                        TextRenderer.DrawText(
                            e.Graphics,
                            statsTabs.TabPages[tabIndex].Text.Trim(),
                            selected ? fontJhengHei95B : fontJhengHei95R,
                            button.ClientRectangle,
                            selected ? Color.FromArgb(235, 248, 255) : Color.FromArgb(145, 155, 172),
                            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
                    };
                    button.Click += (s, e) => statsTabs.SelectedIndex = tabIndex;
                    factionButtons.Add(button);
                    factionBar.Controls.Add(button, i, 0);
                }
                statsTabs.SelectedIndexChanged += (s, e) => {
                    foreach (Button button in factionButtons) {
                        button.Invalidate();
                    }
                };

                layout.Controls.Add(header, 0, 0);
                layout.Controls.Add(factionBar, 0, 1);
                layout.Controls.Add(statsTabs, 0, 2);
                page.Controls.Add(layout);
            }
        }

        private void ConfigureSaveManagerLayout() {
            Panel gameCard = dgvGameSaves.Parent as Panel
                ?? throw new InvalidOperationException("Game saves card was not initialized.");
            Panel backupsCard = dgvBackups.Parent as Panel
                ?? throw new InvalidOperationException("Backups card was not initialized.");
            Panel detailCard = picSavePreview.Parent as Panel
                ?? throw new InvalidOperationException("Save detail card was not initialized.");
            Panel leftColumn = gameCard.Parent as Panel
                ?? throw new InvalidOperationException("Save list column was not initialized.");
            Panel rightColumn = detailCard.Parent as Panel
                ?? throw new InvalidOperationException("Save detail column was not initialized.");

            TableLayoutPanel root = new TableLayoutPanel {
                Dock = DockStyle.Fill,
                BackColor = Color.FromArgb(9, 12, 18),
                ColumnCount = 2,
                RowCount = 1
            };
            root.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 66F));
            root.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 34F));
            root.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));

            TableLayoutPanel leftStack = new TableLayoutPanel {
                Dock = DockStyle.Fill,
                BackColor = Color.Transparent,
                RowCount = 2,
                ColumnCount = 1,
                Margin = new Padding(0, 0, 10, 0)
            };
            leftStack.RowStyles.Add(new RowStyle(SizeType.Percent, 49F));
            leftStack.RowStyles.Add(new RowStyle(SizeType.Percent, 51F));

            gameCard.Dock = DockStyle.Fill;
            gameCard.Margin = new Padding(0, 0, 0, 6);
            gameCard.BackColor = Color.FromArgb(18, 22, 31);
            backupsCard.Dock = DockStyle.Fill;
            backupsCard.Margin = new Padding(0, 6, 0, 0);
            backupsCard.BackColor = Color.FromArgb(18, 22, 31);
            detailCard.Dock = DockStyle.Fill;
            detailCard.Margin = new Padding(0);
            detailCard.BackColor = Color.FromArgb(18, 22, 31);

            leftStack.Controls.Add(gameCard, 0, 0);
            leftStack.Controls.Add(backupsCard, 0, 1);
            leftColumn.Controls.Add(leftStack);
            root.Controls.Add(leftColumn, 0, 0);
            root.Controls.Add(rightColumn, 1, 0);
            leftColumn.Dock = DockStyle.Fill;
            rightColumn.Dock = DockStyle.Fill;
            tabSaveManager.Controls.Add(root);

            void LayoutGameCard() {
                dgvGameSaves.Location = new Point(16, 48);
                dgvGameSaves.Size = new Size(Math.Max(0, gameCard.Width - 32), Math.Max(70, gameCard.Height - 106));
                int y = Math.Max(54, gameCard.Height - 48);
                btnBackupSave.Location = new Point(16, y);
                btnDeleteSave.Location = new Point(164, y);
                btnRefreshSaves.Location = new Point(312, y);
            }
            void LayoutBackupsCard() {
                dgvBackups.Location = new Point(16, 48);
                dgvBackups.Size = new Size(Math.Max(0, backupsCard.Width - 32), Math.Max(70, backupsCard.Height - 106));
                int y = Math.Max(54, backupsCard.Height - 48);
                btnRestoreBackup.Location = new Point(16, y);
                btnDeleteBackup.Location = new Point(164, y);
            }
            void LayoutDetailCard() {
                picSavePreview.Location = new Point(18, 54);
                picSavePreview.Size = new Size(Math.Max(80, detailCard.Width - 36), Math.Min(250, Math.Max(120, detailCard.Height / 3)));
                lblSaveDetail.Location = new Point(18, picSavePreview.Bottom + 16);
                lblSaveDetail.Size = new Size(Math.Max(80, detailCard.Width - 36), Math.Max(80, detailCard.Height - picSavePreview.Bottom - 34));
            }

            dgvGameSaves.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
            dgvBackups.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
            gameCard.Resize += (s, e) => LayoutGameCard();
            backupsCard.Resize += (s, e) => LayoutBackupsCard();
            detailCard.Resize += (s, e) => LayoutDetailCard();
            LayoutGameCard();
            LayoutBackupsCard();
            LayoutDetailCard();
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
            string? errorMsg = null;
            lock (_backupUnitRowsLock) {
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
                    errorMsg = "Failed to parse backup objdef.dau: " + ex.Message;
                    _backupUnitRowsParsed = true;
                }
            }
            if (errorMsg != null) {
                Log(errorMsg);
            }
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

        /// <summary>
        /// 更新語系按鈕視覺樣式
        /// </summary>
        private void UpdateLanguageButtonStyles() {
            bool isZh = Loc.CurrentLanguage == Language.TraditionalChinese;

            btnLangZH.BackColor = isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangZH.ForeColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangZH.FlatAppearance.BorderColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);

            btnLangEN.BackColor = !isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangEN.ForeColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangEN.FlatAppearance.BorderColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);
        }

        /// <summary>
        /// 套用目前的語系設定到所有的 UI 元件
        /// </summary>
        private void ApplyLanguageToUI() {
            if (lblMainTitle == null) return; // 防止在 InitializeComponent 完成前被調用

            lblMainTitle.Text = Loc.Get("MainTitle");
            lblSystemHeading.Text = Loc.Get("SystemHeading");
            lblSystemSubtitle.Text = Loc.Get("SystemSubtitle");
            lblNumericTitle.Text = Loc.Get("NumericTitle");
            chkFocusLoss.Text = Loc.Get("FocusLoss");
            chkToEng.Text = Loc.Get("ToEng");
            chkAiM1.Text = Loc.Get("AiM1");
            chkAiM2.Text = Loc.Get("AiM2");
            chkAiM3.Text = Loc.Get("AiM3");
            chkAiM4.Text = Loc.Get("AiM4");
            chkAiM5.Text = Loc.Get("AiM5");
            chkAiM6.Text = Loc.Get("AiM6");
            chkHousingCapacity20x.Text = Loc.Get("HousingCapacity20x");
            chkStorageCapacity10x.Text = Loc.Get("StorageCapacity10x");
            chkFastBuildUpgradeRepair.Text = Loc.Get("FastBuildUpgradeRepair");
            chkFoodHealing10x.Text = Loc.Get("FoodHealing10x");
            chkDgVoodoo.Text = Loc.Get("DgVoodoo");
            chkVillageBuildRange.Text = Loc.Get("VillageBuildRange");
            btnEnableAll.Text = Loc.Get("EnableAll");
            btnDisableAll.Text = Loc.Get("DisableAll");
            lblSwitchesTitle.Text = Loc.Get("SwitchesTitle");
            lblBuildTitle.Text = Loc.Get("BuildTitle");
            lblAiTitle.Text = Loc.Get("AiCardTitle");
            chkMaxPopulation.Text = Loc.Get("MaxPopulation");
            chkFastCiviProduction.Text = Loc.Get("FastCiviProduction");
            chkFreeProd.Text = Loc.Get("FreeProd");
            chkFreeUpgrade.Text = Loc.Get("FreeUpgrade");
            chkNoSpellCost.Text = Loc.Get("NoSpellCost");
            chkNoSpellAltar.Text = Loc.Get("NoSpellAltar");
            chkInfiniteMorale.Text = Loc.Get("InfiniteMorale");
            lblGamePath.Text = Loc.Get("GamePath");
            btnBrowseGamePath.Text = Loc.Get("Browse");
            btnLoadCurrent.Text = Loc.Get("LoadCurrent");
            btnRestore.Text = Loc.Get("Restore");
            btnApply.Text = Loc.Get("Apply");
            btnTroopPreset.Text = Loc.Get("BtnTroopPreset");
            btnStartGame.Text = Loc.Get("StartGame");

            itemRestoreAll.Text = Loc.Get("RestoreAll");
            itemRestoreStats.Text = Loc.Get("RestoreStats");
            itemRestoreCompat.Text = Loc.Get("RestoreCompat");
            itemRestoreLang.Text = Loc.Get("RestoreLang");

            lblGameSavesTitle.Text = Loc.Get("GameSavesTitle");
            lblBackupsTitle.Text = Loc.Get("BackupsTitle");
            lblDetailTitle.Text = Loc.Get("DetailTitle");
            btnBackupSave.Text = Loc.Get("BackupSave");
            btnDeleteSave.Text = Loc.Get("DeleteSave");
            btnRefreshSaves.Text = Loc.Get("Refresh");
            btnRestoreBackup.Text = Loc.Get("RestoreBackup");
            btnDeleteBackup.Text = Loc.Get("DeleteBackup");

            lblSidebarLang.Text = Loc.Get("LanguageLabel");

            tabDefaultRoman.Text = Loc.Get("TabRoman");
            tabDefaultTeuton.Text = Loc.Get("TabTeuton");
            tabDefaultCelt.Text = Loc.Get("TabCelt");
            tabDefaultHun.Text = Loc.Get("TabHun");

            tabCurrentRoman.Text = Loc.Get("TabRoman");
            tabCurrentTeuton.Text = Loc.Get("TabTeuton");
            tabCurrentCelt.Text = Loc.Get("TabCelt");
            tabCurrentHun.Text = Loc.Get("TabHun");

            lblDefaultStatsTitle.Text = Loc.Get("DefaultStatsTitle");
            chkBalance.Text = Loc.Get("EnableBalance");
            lblTroopTemplate.Text = Loc.Get("TroopTemplateLabel");
            RefreshTroopTemplateItems();
            lblCurrentStatsTitle.Text = Loc.Get("CurrentStatsTitle");

            // 更新表格標頭
            UpdateGridHeaders();

            // 重新整理側邊導覽列按鈕
            RefreshNavButtons();

            // 重新載入技術文件
            ReloadTechnicalDocument();

            // 重新載入表格與存檔數據
            if (backupFiles != null && backupFiles.Count > 0) {
                LoadDefaultStatsData();
                LoadCurrentData(false);
                RefreshSavesAndBackups();
            }
            UpdateTroopPresetLabel();

            if (myToolTip != null) {
                myToolTip.SetToolTip(lblHelpFocusLoss, Loc.Get("FocusLossTip"));
                myToolTip.SetToolTip(lblHelpToEng, Loc.Get("ToEngTip"));
                myToolTip.SetToolTip(lblHelpDgVoodoo, Loc.Get("DgVoodooTip"));
                myToolTip.SetToolTip(lblHelpFreeProd, Loc.Get("FreeProdTip"));
                myToolTip.SetToolTip(lblHelpFreeUpgrade, Loc.Get("FreeUpgradeTip"));
                myToolTip.SetToolTip(lblHelpNoSpellCost, Loc.Get("NoSpellCostTip"));
                myToolTip.SetToolTip(lblHelpInfiniteMorale, Loc.Get("InfiniteMoraleTip"));
                myToolTip.SetToolTip(lblHelpBalance, Loc.Get("BalanceTip"));
                myToolTip.SetToolTip(lblHelpNoSpellAltar, Loc.Get("NoSpellAltarTip"));
                myToolTip.SetToolTip(lblHelpMaxPopulation, Loc.Get("MaxPopulationTip"));
                myToolTip.SetToolTip(lblHelpHousingCapacity20x, Loc.Get("HousingCapacity20xTip"));
                myToolTip.SetToolTip(lblHelpStorageCapacity10x, Loc.Get("StorageCapacity10xTip"));
                myToolTip.SetToolTip(lblHelpFastCiviProduction, Loc.Get("FastCiviProductionTip"));
                myToolTip.SetToolTip(lblHelpFastBuildUpgradeRepair, Loc.Get("FastBuildUpgradeRepairTip"));
                myToolTip.SetToolTip(lblHelpFoodHealing10x, Loc.Get("FoodHealing10xTip"));
                myToolTip.SetToolTip(lblHelpVillageBuildRange, Loc.Get("VillageBuildRangeTip"));
                myToolTip.SetToolTip(chkAiM1, Loc.Get("AiM1Tip"));
                myToolTip.SetToolTip(chkAiM2, Loc.Get("AiM2Tip"));
                myToolTip.SetToolTip(chkAiM3, Loc.Get("AiM3Tip"));
                myToolTip.SetToolTip(chkAiM4, Loc.Get("AiM4Tip"));
                myToolTip.SetToolTip(chkAiM5, Loc.Get("AiM5Tip"));
                myToolTip.SetToolTip(chkAiM6, Loc.Get("AiM6Tip"));
            }
        }

        private Label CreateHelpLabel(string tipKey) {
            Label lbl = new Label {
                Text = "?",
                Size = new Size(22, 22),
                Cursor = Cursors.Hand,
                ForeColor = Color.FromArgb(120, 130, 145),
                BackColor = Color.Transparent,
                Font = fontConsolas85, // 採用 Consolas 讓 "?" 顯得更好看
                TextAlign = ContentAlignment.MiddleCenter
            };

            lbl.Paint += (s, e) => {
                Graphics g = e.Graphics;
                g.SmoothingMode = SmoothingMode.AntiAlias;
                bool isHovered = lbl.ForeColor == Color.FromArgb(0, 230, 255);
                Color circColor = isHovered ? Color.FromArgb(0, 230, 255) : Color.FromArgb(60, 65, 80);
                using (Pen p = new Pen(circColor, 1.2F)) {
                    g.DrawEllipse(p, 1, 1, lbl.Width - 3, lbl.Height - 3);
                }
            };

            lbl.MouseEnter += (s, e) => {
                lbl.ForeColor = Color.FromArgb(0, 230, 255);
                lbl.Invalidate();
            };
            lbl.MouseLeave += (s, e) => {
                lbl.ForeColor = Color.FromArgb(120, 130, 145);
                lbl.Invalidate();
            };
            myToolTip.SetToolTip(lbl, Loc.Get(tipKey));
            return lbl;
        }

        /// <summary>
        /// 動態更新 DataGridView 標頭文字
        /// </summary>
        private void UpdateGridHeaders() {
            foreach (var grid in defaultStatsGrids.Values) {
                if (grid.Columns.Contains("Name")) grid.Columns["Name"].HeaderText = Loc.Get("HeaderName");
                if (grid.Columns.Contains("Icon")) grid.Columns["Icon"].HeaderText = Loc.Get("HeaderIcon");
                if (grid.Columns.Contains("Type")) grid.Columns["Type"].HeaderText = Loc.Get("HeaderType");
                if (grid.Columns.Contains("Style")) grid.Columns["Style"].HeaderText = Loc.Get("HeaderStyle");
                if (grid.Columns.Contains("Hp")) grid.Columns["Hp"].HeaderText = Loc.Get("HeaderHp");
                if (grid.Columns.Contains("MeleeDmg")) grid.Columns["MeleeDmg"].HeaderText = Loc.Get("HeaderMeleeDmg");
                if (grid.Columns.Contains("RangedDmg")) grid.Columns["RangedDmg"].HeaderText = Loc.Get("HeaderRangedDmg");
                if (grid.Columns.Contains("MeleeRelt")) grid.Columns["MeleeRelt"].HeaderText = Loc.Get("HeaderMeleeRelt");
                if (grid.Columns.Contains("RangedRelt")) grid.Columns["RangedRelt"].HeaderText = Loc.Get("HeaderRangedRelt");
                if (grid.Columns.Contains("Vw")) grid.Columns["Vw"].HeaderText = Loc.Get("HeaderVw");
                if (grid.Columns.Contains("Aw")) grid.Columns["Aw"].HeaderText = Loc.Get("HeaderAw");
                if (grid.Columns.Contains("Speed")) grid.Columns["Speed"].HeaderText = Loc.Get("HeaderSpeed");
                if (grid.Columns.Contains("Sight")) grid.Columns["Sight"].HeaderText = Loc.Get("HeaderSight");
                if (grid.Columns.Contains("Range")) grid.Columns["Range"].HeaderText = Loc.Get("HeaderRange");
                if (grid.Columns.Contains("SpellRadius")) grid.Columns["SpellRadius"].HeaderText = Loc.Get("HeaderSpellRadius");
                if (grid.Columns.Contains("Tier")) grid.Columns["Tier"].HeaderText = Loc.Get("HeaderTier");
            }
            foreach (var grid in currentStatsGrids.Values) {
                if (grid.Columns.Contains("Name")) grid.Columns["Name"].HeaderText = Loc.Get("HeaderName");
                if (grid.Columns.Contains("Icon")) grid.Columns["Icon"].HeaderText = Loc.Get("HeaderIcon");
                if (grid.Columns.Contains("Type")) grid.Columns["Type"].HeaderText = Loc.Get("HeaderType");
                if (grid.Columns.Contains("Style")) grid.Columns["Style"].HeaderText = Loc.Get("HeaderStyle");
                if (grid.Columns.Contains("Hp")) grid.Columns["Hp"].HeaderText = Loc.Get("HeaderHpComp");
                if (grid.Columns.Contains("MeleeDmg")) grid.Columns["MeleeDmg"].HeaderText = Loc.Get("HeaderMeleeDmgComp");
                if (grid.Columns.Contains("RangedDmg")) grid.Columns["RangedDmg"].HeaderText = Loc.Get("HeaderRangedDmgComp");
                if (grid.Columns.Contains("MeleeRelt")) grid.Columns["MeleeRelt"].HeaderText = Loc.Get("HeaderMeleeReltComp");
                if (grid.Columns.Contains("RangedRelt")) grid.Columns["RangedRelt"].HeaderText = Loc.Get("HeaderRangedReltComp");
                if (grid.Columns.Contains("Vw")) grid.Columns["Vw"].HeaderText = Loc.Get("HeaderVwComp");
                if (grid.Columns.Contains("Aw")) grid.Columns["Aw"].HeaderText = Loc.Get("HeaderAwComp");
                if (grid.Columns.Contains("Speed")) grid.Columns["Speed"].HeaderText = Loc.Get("HeaderSpeedComp");
                if (grid.Columns.Contains("Sight")) grid.Columns["Sight"].HeaderText = Loc.Get("HeaderSightComp");
                if (grid.Columns.Contains("Range")) grid.Columns["Range"].HeaderText = Loc.Get("HeaderRangeComp");
                if (grid.Columns.Contains("SpellRadius")) grid.Columns["SpellRadius"].HeaderText = Loc.Get("HeaderSpellRadiusComp");
                if (grid.Columns.Contains("Tier")) grid.Columns["Tier"].HeaderText = Loc.Get("HeaderTier");
            }
            if (dgvGameSaves.Columns.Contains("Folder")) dgvGameSaves.Columns["Folder"].HeaderText = Loc.Get("HeaderFolder");
            if (dgvGameSaves.Columns.Contains("Title")) dgvGameSaves.Columns["Title"].HeaderText = Loc.Get("HeaderSaveTitle");
            if (dgvGameSaves.Columns.Contains("Level")) dgvGameSaves.Columns["Level"].HeaderText = Loc.Get("HeaderLevel");
            if (dgvGameSaves.Columns.Contains("Time")) dgvGameSaves.Columns["Time"].HeaderText = Loc.Get("HeaderTime");

            if (dgvBackups.Columns.Contains("File")) dgvBackups.Columns["File"].HeaderText = Loc.Get("HeaderBackupFile");
            if (dgvBackups.Columns.Contains("Title")) dgvBackups.Columns["Title"].HeaderText = Loc.Get("HeaderSaveTitle");
            if (dgvBackups.Columns.Contains("Level")) dgvBackups.Columns["Level"].HeaderText = Loc.Get("HeaderLevel");
            if (dgvBackups.Columns.Contains("Time")) dgvBackups.Columns["Time"].HeaderText = Loc.Get("HeaderBackupTime");
            if (dgvBackups.Columns.Contains("Folder")) dgvBackups.Columns["Folder"].HeaderText = Loc.Get("HeaderOrigFolder");
        }

        private void UpdateTroopPresetLabel() {
            if (lblTroopPresetFile == null) return;
            if (presetFileSourceType == "manual") {
                lblTroopPresetFile.Text = Loc.Get("TroopPresetManual");
                lblTroopPresetFile.ForeColor = Color.FromArgb(200, 100, 255);
            } else if (presetFileSourceType == "file") {
                lblTroopPresetFile.Text = string.Format(Loc.Get("TroopPresetFile"), presetFileName);
                lblTroopPresetFile.ForeColor = Color.FromArgb(0, 220, 255);
            } else if (presetFileSourceType == "preset") {
                lblTroopPresetFile.Text = string.Format(Loc.Get("TroopPresetLoaded"), presetFileName);
                lblTroopPresetFile.ForeColor = Color.FromArgb(0, 220, 255);
            } else {
                lblTroopPresetFile.Text = Loc.Get("TroopPresetDefault");
                lblTroopPresetFile.ForeColor = Color.FromArgb(160, 165, 170);
            }
        }

        private void RefreshTroopTemplateItems() {
            if (cbTroopTemplate == null) return;
            int selectedIndex = cbTroopTemplate.SelectedIndex;
            cbTroopTemplate.SelectedIndexChanged -= CbTroopTemplate_SelectedIndexChanged;
            cbTroopTemplate.Items.Clear();
            cbTroopTemplate.Items.Add(Loc.Get("TroopTemplateSelect"));
            cbTroopTemplate.Items.Add(Loc.Get("TroopTemplateBalanced"));
            cbTroopTemplate.SelectedIndex = selectedIndex > 0 && selectedIndex < cbTroopTemplate.Items.Count ? selectedIndex : 0;
            cbTroopTemplate.SelectedIndexChanged += CbTroopTemplate_SelectedIndexChanged;
        }

        private void CbTroopTemplate_SelectedIndexChanged(object? sender, EventArgs e) {
            if (cbTroopTemplate == null || cbTroopTemplate.SelectedIndex == 0) return;

            string text = Loc.CurrentLanguage == Language.English
                ? "Are you sure you want to apply the 'Balanced Base Stats' template? This will overwrite your current custom unit stats."
                : "確定要套用「修改器內建平衡」範本嗎？這將覆蓋目前的自訂兵種屬性。";
            string title = Loc.CurrentLanguage == Language.English ? "Confirm Template Overwrite" : "確認範本覆蓋";

            if (MessageBox.Show(text, title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) {
                cbTroopTemplate.SelectedIndex = 0;
                return;
            }

            customUnitStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
            foreach (string key in TroopConfig.UnitOrder) {
                if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                customUnitStats[key] = GetDefaultBalancedStats(key).ToArray();
            }

            presetFileSourceType = "preset";
            presetFileName = Loc.Get("TroopTemplateBalanced");
            LoadDefaultStatsData();
            UpdateTroopPresetLabel();
            Log(Loc.CurrentLanguage == Language.English
                ? "Applied built-in balanced troop stats template."
                : "已套用修改器內建平衡兵種屬性範本。");

            cbTroopTemplate.SelectedIndex = 0;
        }
 
        private void BtnTroopPreset_Click(object? sender, EventArgs e) {
            using (var form = new TroopPresetForm(this, customUnitStats, unitIcons)) {
                if (form.ShowDialog() == DialogResult.OK) {
                    customUnitStats = form.CustomStats;
                    LoadDefaultStatsData(); // 重新整理預設屬性表格
                    Log("已套用自訂兵種屬性配置。");
 
                    if (!string.IsNullOrEmpty(form.LoadedFileName)) {
                        presetFileSourceType = "file";
                        presetFileName = form.LoadedFileName;
                    } else if (customUnitStats != null && customUnitStats.Count > 0) {
                        presetFileSourceType = "manual";
                        presetFileName = "";
                    } else {
                        presetFileSourceType = "default";
                        presetFileName = "";
                    }
                    UpdateTroopPresetLabel();
                }
            }
        }

        /// <summary>
        /// 重新載入技術文件內容
        /// </summary>
        private void ReloadTechnicalDocument() {
            if (txtDoc == null) return;
            string docText = "";
            string resourceKey = Loc.CurrentLanguage == Language.English ? "TechDoc_EN.md" : "TechDoc.md";
            try {
                using (Stream? stream = typeof(Program).Assembly.GetManifestResourceStream(resourceKey)) {
                    if (stream != null) {
                        using (StreamReader reader = new StreamReader(stream, Encoding.UTF8)) {
                            docText = reader.ReadToEnd();
                        }
                    } else {
                        foreach (string name in typeof(Program).Assembly.GetManifestResourceNames()) {
                            if (name.EndsWith(resourceKey)) {
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
                Log(Loc.Get("LogLoadTechDocFailed") + ex.Message);
            }

            docText = docText.Replace("\r\n", "\n").Replace("\n", "\r\n");
            txtDoc.Text = docText;
        }

        private DataGridView CreateBaseGrid() {
            var dgv = new DataGridView {
                Dock = DockStyle.None,
                BackgroundColor = Color.FromArgb(20, 21, 31),
                BorderStyle = BorderStyle.None,
                AllowUserToAddRows = false,
                AllowUserToDeleteRows = false,
                AllowUserToOrderColumns = false,
                AllowUserToResizeRows = false,
                RowHeadersVisible = false,
                SelectionMode = DataGridViewSelectionMode.CellSelect,
                MultiSelect = false,
                EnableHeadersVisualStyles = false,
                GridColor = Color.FromArgb(40, 42, 58),
                ScrollBars = ScrollBars.Vertical
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 230, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 36;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(20, 21, 31);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(35, 37, 54);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(24, 25, 35);
            return dgv;
        }

    }
}
