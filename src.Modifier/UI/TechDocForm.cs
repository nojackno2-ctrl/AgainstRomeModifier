using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    public class TechDocForm : Form {
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        [DllImport("gdi32.dll", EntryPoint = "DeleteObject")]
        private static extern bool DeleteObject(IntPtr hObject);

        private Panel pnlTitleBar = null!;
        private Label lblMainTitle = null!;
        private Button btnClose = null!;
        private TextBox txtDoc = null!;

        private bool dragging = false;
        private Point dragStart = new Point(0, 0);
        private Font fontJhengHei115B = new Font("Microsoft JhengHei", 11.5F, FontStyle.Bold);
        private Font fontJhengHei105R = new Font("Microsoft JhengHei", 10.5F, FontStyle.Regular);

        public TechDocForm() {
            InitializeComponent();
            LoadTechnicalDocument();
        }

        private void InitializeComponent() {
            this.Text = Loc.CurrentLanguage == Language.English ? "Technical Document" : "技術文件 (Technical Document)";
            this.Size = new Size(800, 600);
            this.FormBorderStyle = FormBorderStyle.None;
            this.StartPosition = FormStartPosition.CenterScreen;
            this.BackColor = Color.FromArgb(10, 11, 16);
            this.ForeColor = Color.FromArgb(230, 235, 240);

            this.Load += (s, e) => {
                IntPtr ptr = CreateRoundRectRgn(0, 0, Width, Height, 15, 15);
                this.Region = Region.FromHrgn(ptr);
                DeleteObject(ptr);
            };

            this.Paint += (s, e) => {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                using (Pen p = new Pen(Color.FromArgb(100, 110, 130), 2)) {
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
                Text = "TECHNICAL DOCUMENT",
                Location = new Point(20, 14),
                Size = new Size(300, 25),
                Font = fontJhengHei115B,
                ForeColor = Color.FromArgb(210, 218, 230)
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
            btnClose.Click += (s, e) => this.Close();
            btnClose.MouseEnter += (s, e) => { btnClose.BackColor = Color.FromArgb(232, 17, 35); };
            btnClose.MouseLeave += (s, e) => { btnClose.BackColor = Color.Transparent; };

            pnlTitleBar.Controls.Add(lblMainTitle);
            pnlTitleBar.Controls.Add(btnClose);
            this.Controls.Add(pnlTitleBar);

            Panel pnlContent = new Panel {
                Location = new Point(20, 70),
                Size = new Size(this.Width - 40, this.Height - 90),
                BackColor = Color.FromArgb(16, 20, 29),
                Padding = new Padding(10)
            };

            txtDoc = new TextBox {
                Dock = DockStyle.Fill,
                Multiline = true,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical,
                BackColor = Color.FromArgb(16, 20, 29),
                ForeColor = Color.FromArgb(210, 218, 230),
                Font = fontJhengHei105R,
                BorderStyle = BorderStyle.None
            };

            pnlContent.Controls.Add(txtDoc);
            this.Controls.Add(pnlContent);
        }

        private void LoadTechnicalDocument() {
            string docText = "";
            string resourceKey = Loc.CurrentLanguage == Language.English ? "TechDoc_EN.md" : "TechDoc.md"; 
            try {
                using (Stream? stream = typeof(TechDocForm).Assembly.GetManifestResourceStream(resourceKey)) {
                    if (stream != null) {
                        using (StreamReader reader = new StreamReader(stream, Encoding.UTF8)) {
                            docText = reader.ReadToEnd();
                        }
                    } else {
                        foreach (string name in typeof(TechDocForm).Assembly.GetManifestResourceNames()) {
                            if (name.EndsWith(resourceKey)) {
                                using (Stream? s = typeof(TechDocForm).Assembly.GetManifestResourceStream(name)) {
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
                docText = (Loc.CurrentLanguage == Language.English ? "Failed to load technical document: " : "無法載入技術文件: ") + ex.Message;
            }

            docText = docText.Replace("\r\n", "\n").Replace("\n", "\r\n");
            txtDoc.Text = docText;
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

        protected override void Dispose(bool disposing) {
            if (disposing) {
                fontJhengHei115B?.Dispose();
                fontJhengHei105R?.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
