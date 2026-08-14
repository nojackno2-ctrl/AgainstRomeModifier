using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    /// <summary>
    /// 自訂現代科技感 TabControl，徹底移除預設白邊/灰色框線，並支援 OwnerDraw 繪製頁籤
    /// </summary>
    public class ModernTabControl : TabControl {
        public bool HideTabs { get; set; }

        // 選中頁籤的粗體字型。過去每次 OnDrawItem 都 new 一次，等於每張重繪都配置／
        // 釋放一個 GDI 字型控制代碼；改為隨 Font 變更才重建的快取。
        private Font? _selectedTabFont;

        public ModernTabControl() {
            this.DrawMode = TabDrawMode.OwnerDrawFixed;
            this.SizeMode = TabSizeMode.Fixed;
            this.Padding = new Point(18, 6);
        }

        protected override void OnFontChanged(EventArgs e) {
            _selectedTabFont?.Dispose();
            _selectedTabFont = null;
            base.OnFontChanged(e);
        }

        private Font SelectedTabFont => _selectedTabFont ??= new Font(this.Font, FontStyle.Bold);

        protected override void WndProc(ref Message m) {
            // 0x1328 是 TCM_ADJUSTRECT。當不顯示頁籤 (HideTabs = true) 時，
            // 阻斷此訊息可隱藏 WinForms 預設的內縮與灰色邊框。
            if (m.Msg == 0x1328 && !DesignMode && HideTabs) {
                m.Result = (IntPtr)1;
                return;
            }
            base.WndProc(ref m);
        }

        protected override void OnDrawItem(DrawItemEventArgs e) {
            if (HideTabs || e.Index < 0 || e.Index >= this.TabCount) {
                return;
            }

            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;

            Rectangle rect = this.GetTabRect(e.Index);
            bool isSelected = this.SelectedIndex == e.Index;
            Color tabBg = isSelected ? WinFormsTheme.SurfaceRaised : WinFormsTheme.Surface;
            using (SolidBrush tabBrush = new SolidBrush(tabBg)) {
                g.FillRectangle(tabBrush, rect);
            }

            if (isSelected) {
                using (SolidBrush indicatorBrush = new SolidBrush(WinFormsTheme.Accent)) {
                    g.FillRectangle(indicatorBrush, rect.X + 12, rect.Bottom - 3, rect.Width - 24, 3);
                }
            }

            if (isSelected) {
                TextRenderer.DrawText(g, this.TabPages[e.Index].Text, SelectedTabFont, rect,
                    WinFormsTheme.TextPrimary,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
            } else {
                TextRenderer.DrawText(g, this.TabPages[e.Index].Text, this.Font, rect,
                    WinFormsTheme.TextMuted,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
            }
        }

        protected override void Dispose(bool disposing) {
            if (disposing) {
                _selectedTabFont?.Dispose();
                _selectedTabFont = null;
            }
            base.Dispose(disposing);
        }
    }

    /// <summary>
    /// 現代科技感滑動開關 (Toggle Switch) 元件，繼承自 CheckBox 以維持相容性。
    /// </summary>
    public class ModernToggle : CheckBox {
        private int _toggleWidth = 40;
        private int _toggleHeight = 20;
        private float _animPosition; // 0 = 關閉, 1 = 開啟
        private float _targetPosition;

        public ModernToggle() {
            this.SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
            this.Cursor = Cursors.Hand;
            this.Size = new Size(180, 25);
            
            this.CheckedChanged += (s, e) => {
                _targetPosition = this.Checked ? 1.0f : 0.0f;
                _animPosition = _targetPosition;
                this.Invalidate();
            };
        }

        protected override void OnPaint(PaintEventArgs pevent) {
            Graphics g = pevent.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Color clearColor = this.Parent?.BackColor ?? WinFormsTheme.Window;
            g.Clear(clearColor);

            // 計算 Toggle 膠囊的繪製範圍
            int toggleY = (this.Height - _toggleHeight) / 2;
            Rectangle toggleRect = new Rectangle(0, toggleY, _toggleWidth, _toggleHeight);

            // 根據動畫位置插值顏色
            Color startColor = WinFormsTheme.BorderStrong;
            Color endColor = WinFormsTheme.Accent;

            int r = (int)(startColor.R + (endColor.R - startColor.R) * _animPosition);
            int gr = (int)(startColor.G + (endColor.G - startColor.G) * _animPosition);
            int b = (int)(startColor.B + (endColor.B - startColor.B) * _animPosition);
            Color trackColor = Color.FromArgb(r, gr, b);

            // 繪製外框膠囊
            using (GraphicsPath path = GetRoundRectPath(toggleRect, _toggleHeight / 2)) {
                using (SolidBrush brush = new SolidBrush(trackColor)) {
                    g.FillPath(brush, path);
                }
                // 繪製軌道細緻框線
                using (Pen p = new Pen(this.Checked ? WinFormsTheme.AccentHover : WinFormsTheme.Border, 1)) {
                    g.DrawPath(p, path);
                }
            }

            // 計算圓鈕 (Thumb) 的 X 座標
            float thumbX = 3 + 20 * _animPosition;
            int thumbY = toggleY + 3;
            int thumbDiameter = 14;

            // 繪製圓鈕 (白色，加入微微的立體感)
            Color thumbColor = this.Checked ? Color.FromArgb(31, 24, 19) : WinFormsTheme.TextSecondary;
            using (SolidBrush brush = new SolidBrush(thumbColor)) {
                g.FillEllipse(brush, thumbX, thumbY, thumbDiameter, thumbDiameter);
            }

            // 繪製開關文字
            Color textColor = !this.Enabled
                ? WinFormsTheme.TextMuted
                : (this.Checked ? WinFormsTheme.TextPrimary : WinFormsTheme.TextSecondary);
            Rectangle textRect = new Rectangle(_toggleWidth + 10, 0, Math.Max(0, this.Width - _toggleWidth - 10), this.Height);
            TextRenderer.DrawText(g, this.Text, this.Font, textRect, textColor,
                TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.EndEllipsis);

            if (this.Focused && this.ShowFocusCues) {
                ControlPaint.DrawFocusRectangle(g, new Rectangle(0, 0, this.Width - 1, this.Height - 1));
            }
        }

        private GraphicsPath GetRoundRectPath(Rectangle rect, int radius) {
            GraphicsPath path = new GraphicsPath();
            int diameter = radius * 2;
            Rectangle arcRect = new Rectangle(rect.X, rect.Y, diameter, diameter);
            
            path.AddArc(arcRect, 180, 90);
            arcRect.X = rect.Right - diameter;
            path.AddArc(arcRect, 270, 90);
            arcRect.Y = rect.Bottom - diameter;
            path.AddArc(arcRect, 0, 90);
            arcRect.X = rect.X;
            path.AddArc(arcRect, 90, 90);
            path.CloseFigure();
            return path;
        }

        protected override void Dispose(bool disposing) => base.Dispose(disposing);
    }

    /// <summary>
    /// 自訂深色選單顏色對照表，用於 ContextMenuStrip 美化。
    /// </summary>
    public class DarkColorTable : ProfessionalColorTable {
        public override Color ToolStripDropDownBackground => WinFormsTheme.SurfaceRaised;
        public override Color ImageMarginGradientBegin => WinFormsTheme.SurfaceRaised;
        public override Color ImageMarginGradientMiddle => WinFormsTheme.SurfaceRaised;
        public override Color ImageMarginGradientEnd => WinFormsTheme.SurfaceRaised;
        public override Color MenuBorder => WinFormsTheme.BorderStrong;
        public override Color MenuItemSelected => WinFormsTheme.AccentSoft;
        public override Color MenuItemSelectedGradientBegin => WinFormsTheme.AccentSoft;
        public override Color MenuItemSelectedGradientEnd => WinFormsTheme.AccentSoft;
        public override Color MenuItemBorder => WinFormsTheme.Accent;
        public override Color CheckBackground => WinFormsTheme.Accent;
        public override Color CheckSelectedBackground => WinFormsTheme.AccentHover;
        public override Color CheckPressedBackground => WinFormsTheme.AccentPressed;
    }

    /// <summary>
    /// 自訂深色選單渲染器，以維持與整個修改器一致的視覺風格。
    /// </summary>
    public class DarkContextMenuRenderer : ToolStripProfessionalRenderer {
        public DarkContextMenuRenderer() : base(new DarkColorTable()) { }

        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e) {
            e.TextColor = WinFormsTheme.TextPrimary;
            base.OnRenderItemText(e);
        }
    }
}
