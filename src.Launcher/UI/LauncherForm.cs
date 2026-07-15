using System;
using System.Diagnostics;
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
        
        private Font fontJhengHei115B = new Font("Microsoft JhengHei", 11.5F, FontStyle.Bold);
        private Font fontJhengHei10B = new Font("Microsoft JhengHei", 10F, FontStyle.Bold);

        private bool dragging = false;
        private Point dragStart = new Point(0, 0);

        public LauncherForm() {
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

            pnlTitleBar.Controls.Add(lblMainTitle);
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
        }

        private void BtnModifier_Click(object? sender, EventArgs e) {
            LaunchApp("AgainstRomeModifier.exe", "修改器");
        }

        private void BtnSaveManager_Click(object? sender, EventArgs e) {
            LaunchApp("AgainstRomeSaveManager.exe", "存檔管理器");
        }

        private void BtnMapEditor_Click(object? sender, EventArgs e) {
            string? editorPath = ResolveExecutablePath("AgainstRomeMapEditor.exe", true);
            if (editorPath == null) {
                MessageBox.Show("找不到 AgainstRomeMapEditor.exe。請重新建置。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            
            // Note: Map Editor now has its own Game Path text box so it can discover or ask for the path on its own.
            // But we can try to pass the detected game path if we find it in registry.
            string args = "";
            string gamePath = DetectGamePathFromRegistry();
            if (!string.IsNullOrWhiteSpace(gamePath) && Directory.Exists(gamePath)) {
                args = "--game \"" + gamePath + "\"";
            }
            
            try {
                this.Hide();
                var process = Process.Start(new ProcessStartInfo(editorPath, args) { 
                    WorkingDirectory = string.IsNullOrWhiteSpace(gamePath) ? AppContext.BaseDirectory : gamePath, 
                    UseShellExecute = true 
                });
                
                if (process != null) {
                    process.EnableRaisingEvents = true;
                    process.Exited += (s, args) => {
                        this.Invoke(new Action(() => this.Show()));
                    };
                } else {
                    this.Show(); // Fallback if process failed to hook
                }
            }
            catch (Exception ex) {
                this.Show();
                MessageBox.Show("無法啟動地圖編輯器: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void LaunchApp(string exeName, string appName) {
            string? appPath = ResolveExecutablePath(exeName, false);
            if (appPath == null) {
                MessageBox.Show($"找不到 {exeName}。請重新建置{appName}。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }

            string args = "";
            string gamePath = DetectGamePathFromRegistry();
            if (!string.IsNullOrWhiteSpace(gamePath) && Directory.Exists(gamePath)) {
                args = "--game \"" + gamePath + "\"";
            }

            try {
                this.Hide();
                var process = Process.Start(new ProcessStartInfo(appPath, args) { 
                    WorkingDirectory = string.IsNullOrWhiteSpace(gamePath) ? Path.GetDirectoryName(appPath) : gamePath, 
                    UseShellExecute = true 
                });
                
                if (process != null) {
                    process.EnableRaisingEvents = true;
                    process.Exited += (s, args) => {
                        this.Invoke(new Action(() => this.Show()));
                    };
                } else {
                    this.Show();
                }
            }
            catch (Exception ex) {
                this.Show();
                MessageBox.Show($"無法啟動{appName}: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
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

        private static string? ResolveExecutablePath(string exeName, bool isMapEditor) {
            var candidates = new List<string> {
                Path.Combine(AppContext.BaseDirectory, exeName)
            };

            DirectoryInfo? directory = new DirectoryInfo(AppContext.BaseDirectory);
            while (directory != null && !File.Exists(Path.Combine(directory.FullName, "AgainstRomeModifier.slnx"))) {
                directory = directory.Parent;
            }
            if (directory != null) {
                if (isMapEditor) {
                    candidates.Add(Path.Combine(directory.FullName, "src.MapEditor", "bin", "Debug", "net8.0-windows", exeName));
                    candidates.Add(Path.Combine(directory.FullName, "src.MapEditor", "bin", "Release", "net8.0-windows", exeName));
                } else {
                    string projectName = exeName.Replace(".exe", "");
                    candidates.Add(Path.Combine(directory.FullName, $"src.{projectName.Replace("AgainstRome", "")}", "bin", "Debug", "net8.0-windows", exeName));
                    candidates.Add(Path.Combine(directory.FullName, $"src.{projectName.Replace("AgainstRome", "")}", "bin", "Release", "net8.0-windows", exeName));
                }
            }

            return candidates
                .Where(File.Exists)
                .OrderByDescending(path => {
                    string dll = Path.ChangeExtension(path, ".dll");
                    return File.Exists(dll) ? File.GetLastWriteTimeUtc(dll) : File.GetLastWriteTimeUtc(path);
                })
                .FirstOrDefault();
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
