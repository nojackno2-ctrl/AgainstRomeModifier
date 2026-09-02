using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    public class LauncherForm : Form {
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        [DllImport("gdi32.dll", EntryPoint = "DeleteObject")]
        private static extern bool DeleteObject(IntPtr hObject);

        private Panel pnlTitleBar = null!;
        private Label lblMainTitle = null!;
        private Button btnClose = null!;
        private Button btnMinimize = null!;
        
        private Button btnModifier = null!;
        private Button btnMapEditor = null!;
        private Button btnSaveManager = null!;
        private Button btnTechDoc = null!;
        private Button btnLangZH = null!;
        private Button btnLangEN = null!;
        
        private Font fontJhengHei115B = WinFormsTheme.CreateDisplayFont(11.5F);
        private Font fontJhengHei10B = WinFormsTheme.CreateFont(10F, FontStyle.Bold);

        private bool dragging;
        private Point dragStart = new Point(0, 0);

        private string? _initialGamePath;

        public LauncherForm() : this(null) {
        }

        public LauncherForm(string? initialGamePath) {
            _initialGamePath = initialGamePath;
            InitializeComponent();
        }

        private void InitializeComponent() {
            this.Text = "Against Rome Pro Launcher";
            this.ClientSize = new Size(720, 500);
            this.FormBorderStyle = FormBorderStyle.None;
            this.StartPosition = FormStartPosition.CenterScreen;
            this.BackColor = WinFormsTheme.Window;
            this.ForeColor = WinFormsTheme.TextPrimary;
            this.Font = WinFormsTheme.CreateFont(9F);
            this.DoubleBuffered = true;

            this.Load += (s, e) => {
                IntPtr ptr = CreateRoundRectRgn(0, 0, Width, Height, 15, 15);
                this.Region = Region.FromHrgn(ptr);
                DeleteObject(ptr);
            };

            this.Paint += (s, e) => {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                using (Pen p = new Pen(WinFormsTheme.BorderStrong, 1)) {
                    using (GraphicsPath path = GetRoundPath(new Rectangle(0, 0, this.Width - 1, this.Height - 1), WinFormsTheme.ContainerRadius)) {
                        e.Graphics.DrawPath(p, path);
                    }
                }
            };

            pnlTitleBar = new Panel {
                Location = new Point(0, 0),
                Size = new Size(this.Width, 64),
                BackColor = WinFormsTheme.Surface
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblMainTitle = new Label {
                Text = "AGAINST ROME PRO LAUNCHER",
                Location = new Point(24, 19),
                Size = new Size(330, 28),
                Font = fontJhengHei115B,
                ForeColor = WinFormsTheme.TextPrimary
            };
            lblMainTitle.MouseDown += TitleBar_MouseDown;
            lblMainTitle.MouseMove += TitleBar_MouseMove;
            lblMainTitle.MouseUp += TitleBar_MouseUp;

            btnClose = new Button {
                Text = "×",
                Location = new Point(this.Width - 44, 16),
                Size = new Size(28, 28),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.Transparent,
                ForeColor = WinFormsTheme.TextSecondary
            };
            btnClose.FlatAppearance.BorderSize = 0;
            btnClose.Cursor = Cursors.Hand;
            btnClose.Click += (s, e) => Application.Exit();
            btnClose.MouseEnter += (s, e) => {
                btnClose.BackColor = Color.FromArgb(76, 34, 37);
                btnClose.ForeColor = WinFormsTheme.Danger;
            };
            btnClose.MouseLeave += (s, e) => {
                btnClose.BackColor = Color.Transparent;
                btnClose.ForeColor = WinFormsTheme.TextSecondary;
            };

            btnMinimize = new Button {
                Text = "−",
                Location = new Point(this.Width - 80, 16),
                Size = new Size(28, 28),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.Transparent,
                ForeColor = WinFormsTheme.TextSecondary
            };
            btnMinimize.FlatAppearance.BorderSize = 0;
            btnMinimize.Cursor = Cursors.Hand;
            btnMinimize.Click += (s, e) => this.WindowState = FormWindowState.Minimized;
            btnMinimize.MouseEnter += (s, e) => {
                btnMinimize.BackColor = WinFormsTheme.SurfaceHover;
            };
            btnMinimize.MouseLeave += (s, e) => {
                btnMinimize.BackColor = Color.Transparent;
            };

            btnLangZH = new Button {
                Text = "繁體中文",
                Location = new Point(this.Width - 282, 16),
                Size = new Size(80, 30),
                FlatStyle = FlatStyle.Flat,
                Font = fontJhengHei10B,
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
                Text = "English",
                Location = new Point(this.Width - 188, 16),
                Size = new Size(80, 30),
                FlatStyle = FlatStyle.Flat,
                Font = fontJhengHei10B,
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

            pnlTitleBar.Controls.Add(lblMainTitle);
            pnlTitleBar.Controls.Add(btnLangZH);
            pnlTitleBar.Controls.Add(btnLangEN);
            pnlTitleBar.Controls.Add(btnClose);
            pnlTitleBar.Controls.Add(btnMinimize);
            this.Controls.Add(pnlTitleBar);

            // Launcher Buttons
            btnModifier = new Button { Text = "啟動修改器", Location = new Point(36, 100), Size = new Size(310, 340) };
            StyleButton(btnModifier, WinFormsTheme.Accent, Color.FromArgb(27, 20, 15), WinFormsTheme.AccentHover);
            btnModifier.Click += BtnModifier_Click;

            btnMapEditor = new Button { Text = "地圖編輯器", Location = new Point(374, 100), Size = new Size(310, 100) };
            StyleButton(btnMapEditor, WinFormsTheme.SurfaceRaised, WinFormsTheme.TextPrimary, WinFormsTheme.Accent);
            btnMapEditor.Click += BtnMapEditor_Click;

            btnSaveManager = new Button { Text = "存檔管理器", Location = new Point(374, 220), Size = new Size(310, 100) };
            StyleButton(btnSaveManager, WinFormsTheme.SurfaceRaised, WinFormsTheme.TextPrimary, WinFormsTheme.Accent);
            btnSaveManager.Click += BtnSaveManager_Click;

            btnTechDoc = new Button { Text = "技術文件", Location = new Point(374, 340), Size = new Size(310, 100) };
            StyleButton(btnTechDoc, WinFormsTheme.SurfaceRaised, WinFormsTheme.TextPrimary, WinFormsTheme.Accent);
            btnTechDoc.Click += (s, e) => {
                var docForm = new TechDocForm();
                docForm.ShowDialog(this);
            };

            this.Controls.Add(btnModifier);
            this.Controls.Add(btnMapEditor);
            this.Controls.Add(btnSaveManager);
            this.Controls.Add(btnTechDoc);

            UpdateLanguageButtonStyles();
            ApplyLanguageToUI();
        }

        private void UpdateLanguageButtonStyles() {
            bool isZh = Loc.CurrentLanguage == Language.TraditionalChinese;

            WinFormsTheme.StyleLanguageButton(btnLangZH, isZh);
            WinFormsTheme.StyleLanguageButton(btnLangEN, !isZh);
        }

        private void ApplyLanguageToUI() {
            bool isEn = Loc.CurrentLanguage == Language.English;
            this.Text = isEn ? "Against Rome Pro Launcher" : "Against Rome 啟動器";
            lblMainTitle.Text = isEn ? "AGAINST ROME PRO LAUNCHER" : "AGAINST ROME 啟動器";
            btnModifier.Text = isEn ? "Launch Modifier" : "啟動修改器";
            btnMapEditor.Text = isEn ? "Map Editor" : "地圖編輯器";
            btnSaveManager.Text = isEn ? "Save Manager" : "存檔管理器";
            btnTechDoc.Text = isEn ? "Technical Document" : "修改技術文件";
        }

        /// <summary>
        /// 啟動子表單（修改器 / 存檔管理器）的共用流程：解析遊戲路徑 → 隱藏啟動器 →
        /// ShowDialog → 無論成敗都重載語系並復原啟動器視窗。兩個入口僅差在建立的表單與錯誤前綴。
        /// </summary>
        private void LaunchChildForm(Func<string, Form> childFactory, string launchErrorPrefix) {
            string gamePath = _initialGamePath ?? DetectGamePathFromRegistry();
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) {
                if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe"))) {
                    gamePath = AppContext.BaseDirectory;
                }
            }

            try {
                this.Hide();
                using var child = childFactory(gamePath);
                child.ShowDialog(this);
            }
            catch (Exception ex) {
                MessageBox.Show(launchErrorPrefix + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            finally {
                Loc.ReloadLanguage();
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
                this.Show();
            }
        }

        private void BtnModifier_Click(object? sender, EventArgs e) =>
            LaunchChildForm(gamePath => new AgainstRomeModifier.ModifierForm(gamePath), "無法啟動修改器: ");

        private void BtnSaveManager_Click(object? sender, EventArgs e) =>
            LaunchChildForm(gamePath => new AgainstRomeModifier.SaveManagerForm(gamePath), "無法啟動存檔管理器: ");

        private void BtnMapEditor_Click(object? sender, EventArgs e) {
            string gamePath = _initialGamePath ?? DetectGamePathFromRegistry();
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) {
                if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe"))) {
                    gamePath = AppContext.BaseDirectory;
                } else {
                    gamePath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Against Rome");
                }
            }

            try {
                this.Hide();
                string? preferredMapId = null;
                while (true) {
                    using var selection = new AgainstRomeMapEditor.MapSelectionForm(gamePath, preferredMapId);
                    if (selection.ShowDialog() != DialogResult.OK || selection.SelectedMap is null) {
                        break;
                    }
                    gamePath = selection.GamePath;
                    preferredMapId = selection.SelectedMap.Id;
                    using var editor = new AgainstRomeMapEditor.MapEditorForm(gamePath, selection.SelectedMap);
                    editor.ShowDialog();
                    if (!editor.ReturnToMapMenu) {
                        break;
                    }
                }
            }
            catch (Exception ex) {
                MessageBox.Show("無法啟動地圖編輯器: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            finally {
                Loc.ReloadLanguage();
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
                this.Show();
            }
        }

        private string DetectGamePathFromRegistry() =>
            AgainstRomeModifier.Core.Services.GameDirectoryLocator.DetectFromRegistry();

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

        private void TitleBar_MouseDown(object? sender, MouseEventArgs e) {
            if (e.Button == MouseButtons.Left) {
                dragging = true;
                dragStart = new Point(e.X, e.Y);
            }
        }
        private void TitleBar_MouseMove(object? sender, MouseEventArgs e) {
            if (dragging) {
                Point p = PointToScreen(e.Location);
                Location = new Point(p.X - dragStart.X, p.Y - dragStart.Y);
            }
        }
        private void TitleBar_MouseUp(object? sender, MouseEventArgs e) {
            dragging = false;
        }

        private void StyleButton(Button btn, Color backColor, Color foreColor, Color hoverBorderColor) {
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 0;
            btn.BackColor = Color.Transparent;
            btn.ForeColor = foreColor;
            btn.Cursor = Cursors.Hand;
            btn.Font = fontJhengHei10B;

            bool isHovered = false;
            btn.MouseEnter += (s, e) => { isHovered = true; btn.Invalidate(); };
            btn.MouseLeave += (s, e) => { isHovered = false; btn.Invalidate(); };

            btn.Paint += (s, e) => {
                Graphics g = e.Graphics;
                g.SmoothingMode = SmoothingMode.AntiAlias;

                Rectangle rect = new Rectangle(0, 0, btn.Width, btn.Height);
                int radius = 8;

                using (GraphicsPath path = GetRoundPath(rect, radius)) {
                    Color startColor, endColor;
                    bool primary = backColor == WinFormsTheme.Accent;
                    startColor = isHovered
                        ? (primary ? WinFormsTheme.AccentHover : WinFormsTheme.SurfaceHover)
                        : backColor;
                    endColor = primary ? WinFormsTheme.AccentPressed : WinFormsTheme.Surface;

                    using (LinearGradientBrush brush = new LinearGradientBrush(rect, startColor, endColor, 45F)) {
                        g.FillPath(brush, path);
                    }

                    Color borderColor = isHovered ? hoverBorderColor : (primary ? WinFormsTheme.Accent : WinFormsTheme.Border);
                    using (Pen p = new Pen(borderColor, 1.2F)) {
                        g.DrawPath(p, path);
                    }

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
        
        protected override void Dispose(bool disposing) {
            if (disposing) {
                fontJhengHei115B?.Dispose();
                fontJhengHei10B?.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
