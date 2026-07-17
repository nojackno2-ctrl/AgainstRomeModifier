using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using System.Collections.Generic;

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
        
        private Font fontJhengHei115B = new Font("Microsoft JhengHei", 11.5F, FontStyle.Bold);
        private Font fontJhengHei10B = new Font("Microsoft JhengHei", 10F, FontStyle.Bold);

        private bool dragging = false;
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
            this.Size = new Size(600, 480);
            this.FormBorderStyle = FormBorderStyle.None;
            this.StartPosition = FormStartPosition.CenterScreen;
            this.BackColor = Color.FromArgb(10, 11, 16);
            this.ForeColor = Color.FromArgb(230, 235, 240);
            this.DoubleBuffered = true;

            this.Load += (s, e) => {
                IntPtr ptr = CreateRoundRectRgn(0, 0, Width, Height, 15, 15);
                this.Region = Region.FromHrgn(ptr);
                DeleteObject(ptr);
            };

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
                Size = new Size(this.Width, 50),
                BackColor = Color.FromArgb(18, 19, 29)
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblMainTitle = new Label {
                Text = "AGAINST ROME PRO LAUNCHER",
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
                Location = new Point(this.Width - 40, 10),
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
            };
            btnClose.MouseLeave += (s, e) => {
                btnClose.BackColor = Color.Transparent;
            };

            btnMinimize = new Button {
                Text = "—",
                Location = new Point(this.Width - 80, 10),
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

            btnLangZH = new Button {
                Text = "繁體中文",
                Location = new Point(this.Width - 270, 10),
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
                Location = new Point(this.Width - 180, 10),
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
            btnModifier = new Button { Text = "啟動修改器", Location = new Point(100, 100), Size = new Size(400, 60) };
            StyleButton(btnModifier, Color.FromArgb(38, 132, 255), Color.White, Color.FromArgb(0, 230, 255));
            btnModifier.Click += BtnModifier_Click;

            btnMapEditor = new Button { Text = "地圖編輯器", Location = new Point(100, 190), Size = new Size(400, 60) };
            StyleButton(btnMapEditor, Color.FromArgb(0, 180, 120), Color.White, Color.FromArgb(0, 255, 170));
            btnMapEditor.Click += BtnMapEditor_Click;

            btnSaveManager = new Button { Text = "存檔管理器", Location = new Point(100, 280), Size = new Size(400, 60) };
            StyleButton(btnSaveManager, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnSaveManager.Click += BtnSaveManager_Click;

            btnTechDoc = new Button { Text = "技術文件", Location = new Point(100, 370), Size = new Size(400, 60) };
            StyleButton(btnTechDoc, Color.FromArgb(40, 45, 55), Color.White, Color.FromArgb(100, 110, 130));
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

            btnLangZH.BackColor = isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangZH.ForeColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangZH.FlatAppearance.BorderColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);

            btnLangEN.BackColor = !isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangEN.ForeColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangEN.FlatAppearance.BorderColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);
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

        private void BtnModifier_Click(object? sender, EventArgs e) {
            string gamePath = _initialGamePath ?? DetectGamePathFromRegistry();
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) {
                if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe"))) {
                    gamePath = AppContext.BaseDirectory;
                }
            }

            try {
                this.Hide();
                using (var modifier = new AgainstRomeModifier.ModifierForm(gamePath)) {
                    modifier.ShowDialog(this);
                }
            }
            catch (Exception ex) {
                MessageBox.Show("無法啟動修改器: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            finally {
                Loc.ReloadLanguage();
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
                this.Show();
            }
        }

        private void BtnSaveManager_Click(object? sender, EventArgs e) {
            string gamePath = _initialGamePath ?? DetectGamePathFromRegistry();
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) {
                if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe"))) {
                    gamePath = AppContext.BaseDirectory;
                }
            }

            try {
                this.Hide();
                using (var saveManager = new AgainstRomeModifier.SaveManagerForm(gamePath)) {
                    saveManager.ShowDialog(this);
                }
            }
            catch (Exception ex) {
                MessageBox.Show("無法啟動存檔管理器: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            finally {
                Loc.ReloadLanguage();
                UpdateLanguageButtonStyles();
                ApplyLanguageToUI();
                this.Show();
            }
        }

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

        private string DetectGamePathFromRegistry() {
            try {
                using (var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Against Rome")) {
                    if (key != null) {
                        var val = key.GetValue("Path");
                        if (val != null) return val.ToString() ?? "";
                    }
                }
                using (var key = Microsoft.Win32.Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Against Rome")) {
                    if (key != null) {
                        var val = key.GetValue("Path");
                        if (val != null) return val.ToString() ?? "";
                    }
                }
                using (var key = Microsoft.Win32.Registry.LocalMachine.OpenSubKey(@"SOFTWARE\WOW6432Node\Against Rome")) {
                    if (key != null) {
                        var val = key.GetValue("Path");
                        if (val != null) return val.ToString() ?? "";
                    }
                }
            } catch (Exception) { }
            return "";
        }

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
                    if (backColor == Color.FromArgb(98, 0, 238) || backColor == Color.FromArgb(38, 132, 255)) {
                        startColor = isHovered ? Color.FromArgb(52, 151, 255) : backColor;
                        endColor = isHovered ? Color.FromArgb(66, 199, 255) : Color.FromArgb(backColor.R, Math.Min(255, backColor.G + 30), Math.Min(255, backColor.B + 30));
                    } else if (backColor == Color.FromArgb(0, 180, 120)) {
                        startColor = isHovered ? Color.FromArgb(0, 200, 140) : backColor;
                        endColor = isHovered ? Color.FromArgb(0, 240, 170) : Color.FromArgb(0, 210, 150);
                    } else {
                        startColor = isHovered ? Color.FromArgb(47, 55, 70) : backColor;
                        endColor = isHovered ? Color.FromArgb(56, 66, 84) : Color.FromArgb(backColor.R + 20, backColor.G + 20, backColor.B + 20);
                    }

                    using (LinearGradientBrush brush = new LinearGradientBrush(rect, startColor, endColor, 45F)) {
                        g.FillPath(brush, path);
                    }

                    Color borderColor = isHovered ? hoverBorderColor : Color.FromArgb(50, 52, 70);
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
