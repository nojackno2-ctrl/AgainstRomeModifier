using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    /// <summary>
    /// 現代科技感滑動開關 (Toggle Switch) 元件，繼承自 CheckBox 以維持相容性。
    /// </summary>
    public class ModernToggle : CheckBox {
        private int _toggleWidth = 40;
        private int _toggleHeight = 20;
        private Timer _animationTimer;
        private float _animPosition = 0f; // 0 = 關閉, 1 = 開啟
        private float _targetPosition = 0f;

        public ModernToggle() {
            this.SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
            this.Cursor = Cursors.Hand;
            this.Size = new Size(180, 25);
            
            _animationTimer = new Timer { Interval = 15 };
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
            g.Clear(this.Parent?.BackColor ?? Color.FromArgb(16, 16, 20));

            // 計算 Toggle 膠囊的繪製範圍
            int toggleY = (this.Height - _toggleHeight) / 2;
            Rectangle toggleRect = new Rectangle(0, toggleY, _toggleWidth, _toggleHeight);

            // 根據動畫位置插值顏色
            // 關閉時底色：深灰 (45, 45, 55)，開啟時底色：亮青 (0, 180, 255) 或 科技紫 (98, 0, 238)
            Color startColor = Color.FromArgb(45, 45, 55);
            Color endColor = Color.FromArgb(0, 180, 255); // 霓虹青

            int r = (int)(startColor.R + (endColor.R - startColor.R) * _animPosition);
            int gr = (int)(startColor.G + (endColor.G - startColor.G) * _animPosition);
            int b = (int)(startColor.B + (endColor.B - startColor.B) * _animPosition);
            Color trackColor = Color.FromArgb(r, gr, b);

            // 繪製外框膠囊
            using (GraphicsPath path = GetRoundRectPath(toggleRect, _toggleHeight / 2)) {
                using (SolidBrush brush = new SolidBrush(trackColor)) {
                    g.FillPath(brush, path);
                }
                // 繪製軌道框線
                using (Pen p = new Pen(Color.FromArgb(70, 70, 85), 1)) {
                    g.DrawPath(p, path);
                }
            }

            // 計算圓鈕 (Thumb) 的 X 座標
            // 關閉起點 X = 3，開啟終點 X = 23 (40 - 14 - 3)
            float thumbX = 3 + 20 * _animPosition;
            int thumbY = toggleY + 3;
            int thumbDiameter = 14;

            // 繪製圓鈕
            Color thumbColor = this.Checked ? Color.White : Color.FromArgb(150, 160, 175);
            using (SolidBrush brush = new SolidBrush(thumbColor)) {
                g.FillEllipse(brush, thumbX, thumbY, thumbDiameter, thumbDiameter);
            }

            // 繪製開關文字
            Color textColor = this.Checked ? Color.FromArgb(0, 220, 255) : Color.FromArgb(200, 205, 210);
            using (SolidBrush textBrush = new SolidBrush(textColor)) {
                string text = this.Text;
                Font f = this.Font;
                SizeF textSize = g.MeasureString(text, f);
                float textY = (this.Height - textSize.Height) / 2;
                g.DrawString(text, f, textBrush, _toggleWidth + 8, textY);
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
