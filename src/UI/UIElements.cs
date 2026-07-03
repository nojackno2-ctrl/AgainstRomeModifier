using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    /// <summary>
    /// 自訂現代科技感 TabControl，徹底移除預設白邊/灰色框線，並支援 OwnerDraw 繪製頁籤
    /// </summary>
    public class ModernTabControl : TabControl {
        public ModernTabControl() {
            this.DrawMode = TabDrawMode.OwnerDrawFixed;
            this.SizeMode = TabSizeMode.Fixed;
            this.Padding = new Point(18, 6);
        }

        protected override void WndProc(ref Message m) {
            // 0x1328 是 TCM_ADJUSTRECT。當不顯示頁籤 (ItemSize 高度設為 1) 時，
            // 阻斷此訊息可隱藏 WinForms 預設的內縮與灰色邊框。
            if (m.Msg == 0x1328 && !DesignMode) {
                m.Result = (IntPtr)1;
                return;
            }
            base.WndProc(ref m);
        }

        protected override void OnDrawItem(DrawItemEventArgs e) {
            if (this.ItemSize.Height <= 5 || e.Index < 0 || e.Index >= this.TabCount) {
                return;
            }

            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;

            Rectangle rect = this.GetTabRect(e.Index);
            bool isSelected = this.SelectedIndex == e.Index;
            Color tabBg = isSelected ? Color.FromArgb(26, 31, 43) : Color.FromArgb(14, 17, 24);
            using (SolidBrush tabBrush = new SolidBrush(tabBg)) {
                g.FillRectangle(tabBrush, rect);
            }

            if (isSelected) {
                using (SolidBrush indicatorBrush = new SolidBrush(Color.FromArgb(62, 203, 255))) {
                    g.FillRectangle(indicatorBrush, rect.X + 10, rect.Bottom - 3, rect.Width - 20, 3);
                }
            }

            if (isSelected) {
                using (Font selectedFont = new Font(this.Font, FontStyle.Bold)) {
                    TextRenderer.DrawText(g, this.TabPages[e.Index].Text, selectedFont, rect,
                        Color.FromArgb(235, 248, 255),
                        TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
                }
            } else {
                TextRenderer.DrawText(g, this.TabPages[e.Index].Text, this.Font, rect,
                    Color.FromArgb(145, 155, 172),
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
            }
        }
    }

    /// <summary>
    /// 現代科技感滑動開關 (Toggle Switch) 元件，繼承自 CheckBox 以維持相容性。
    /// </summary>
    public class ModernToggle : CheckBox {
        private int _toggleWidth = 40;
        private int _toggleHeight = 20;
        private System.Windows.Forms.Timer _animationTimer;
        private float _animPosition = 0f; // 0 = 關閉, 1 = 開啟
        private float _targetPosition = 0f;

        public ModernToggle() {
            this.SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
            this.Cursor = Cursors.Hand;
            this.Size = new Size(180, 25);
            
            _animationTimer = new System.Windows.Forms.Timer { Interval = 15 };
            _animationTimer.Tick += (s, e) => {
                float step = 0.15f;
                if (Math.Abs(_animPosition - _targetPosition) < step) {
                    _animPosition = _targetPosition;
                    _animationTimer.Stop();
                } else {
                    _animPosition += (_targetPosition > _animPosition) ? step : -step;
                }
                this.Invalidate();
            };
            
            this.CheckedChanged += (s, e) => {
                _targetPosition = this.Checked ? 1.0f : 0.0f;
                _animationTimer.Start();
            };
        }

        protected override void OnPaint(PaintEventArgs pevent) {
            Graphics g = pevent.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Color clearColor = this.Parent?.BackColor ?? Color.FromArgb(11, 14, 20);
            g.Clear(clearColor);

            // 計算 Toggle 膠囊的繪製範圍
            int toggleY = (this.Height - _toggleHeight) / 2;
            Rectangle toggleRect = new Rectangle(0, toggleY, _toggleWidth, _toggleHeight);

            // 根據動畫位置插值顏色
            Color startColor = Color.FromArgb(42, 48, 62);
            Color endColor = Color.FromArgb(38, 166, 255);

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
                using (Pen p = new Pen(this.Checked ? Color.FromArgb(82, 193, 255) : Color.FromArgb(66, 74, 92), 1)) {
                    g.DrawPath(p, path);
                }
            }

            // 計算圓鈕 (Thumb) 的 X 座標
            float thumbX = 3 + 20 * _animPosition;
            int thumbY = toggleY + 3;
            int thumbDiameter = 14;

            // 繪製圓鈕 (白色，加入微微的立體感)
            Color thumbColor = this.Checked ? Color.White : Color.FromArgb(160, 170, 185);
            using (SolidBrush brush = new SolidBrush(thumbColor)) {
                g.FillEllipse(brush, thumbX, thumbY, thumbDiameter, thumbDiameter);
            }

            // 繪製開關文字
            Color textColor = !this.Enabled
                ? Color.FromArgb(100, 108, 122)
                : (this.Checked ? Color.FromArgb(234, 247, 255) : Color.FromArgb(194, 201, 214));
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

        protected override void Dispose(bool disposing) {
            if (disposing) {
                _animationTimer.Dispose();
            }
            base.Dispose(disposing);
        }
    }

    /// <summary>
    /// 自訂深色選單顏色對照表，用於 ContextMenuStrip 美化。
    /// </summary>
    public class DarkColorTable : ProfessionalColorTable {
        public override Color ToolStripDropDownBackground => Color.FromArgb(24, 24, 30);
        public override Color ImageMarginGradientBegin => Color.FromArgb(24, 24, 30);
        public override Color ImageMarginGradientMiddle => Color.FromArgb(24, 24, 30);
        public override Color ImageMarginGradientEnd => Color.FromArgb(24, 24, 30);
        public override Color MenuBorder => Color.FromArgb(0, 220, 255);
        public override Color MenuItemSelected => Color.FromArgb(45, 45, 60);
        public override Color MenuItemSelectedGradientBegin => Color.FromArgb(45, 45, 60);
        public override Color MenuItemSelectedGradientEnd => Color.FromArgb(45, 45, 60);
        public override Color MenuItemBorder => Color.FromArgb(0, 220, 255);
        public override Color CheckBackground => Color.FromArgb(0, 220, 255);
        public override Color CheckSelectedBackground => Color.FromArgb(0, 220, 255);
        public override Color CheckPressedBackground => Color.FromArgb(0, 180, 220);
    }

    /// <summary>
    /// 自訂深色選單渲染器，以維持與整個修改器一致的視覺風格。
    /// </summary>
    public class DarkContextMenuRenderer : ToolStripProfessionalRenderer {
        public DarkContextMenuRenderer() : base(new DarkColorTable()) { }

        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e) {
            e.TextColor = Color.FromArgb(230, 235, 240); // 項目文字使用淡灰色
            base.OnRenderItemText(e);
        }
    }
}
