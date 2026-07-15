using System;
using System.IO;
using System.Text;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using System.Globalization;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier {
    internal sealed class TroopPresetForm : Form {
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        [DllImport("gdi32.dll", EntryPoint = "DeleteObject")]
        private static extern bool DeleteObject(IntPtr hObject);

        private readonly UnitStatsEditorService statsService;
        private readonly Dictionary<string, Bitmap> unitIcons;
        
        // 傳遞編輯後結果的 Dictionary (每個兵種有 9 個屬性值)
        public Dictionary<string, double[]> CustomStats { get; private set; }
        public string LoadedFileName { get; private set; } = "";

        // UI 元件
        private Panel pnlTitleBar = null!;
        private Label lblTitle = null!;
        private Button btnClose = null!;
        
        private TabControl tabFaction = null!;
        private Dictionary<string, DataGridView> factionGrids = new Dictionary<string, DataGridView>(StringComparer.OrdinalIgnoreCase);
        
        private Button btnImport = null!;
        private Button btnExport = null!;
        private Button btnApply = null!;
        private Button btnCancel = null!;

        // 狀態與拖曳變數
        private bool dragging = false;
        private Point dragStart = new Point(0, 0);

        // 字型物件
        private Font fontJhengHei10B = new Font("Microsoft JhengHei", 10F, FontStyle.Bold);
        private Font fontJhengHei95B = new Font("Microsoft JhengHei", 9.5F, FontStyle.Bold);
        private Font fontJhengHei9R = new Font("Microsoft JhengHei", 9F, FontStyle.Regular);
        private bool fontsDisposed;

        internal TroopPresetForm(UnitStatsEditorService statsService, Dictionary<string, double[]>? currentCustomStats, Dictionary<string, Bitmap> unitIcons) {
            this.statsService = statsService;
            this.unitIcons = unitIcons;
            
            if (currentCustomStats != null) {
                this.CustomStats = new Dictionary<string, double[]>(currentCustomStats, StringComparer.OrdinalIgnoreCase);
            } else {
                this.CustomStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
            }

            InitializeComponent();
            ApplyCustomStatsToGrid();
        }

        private void InitializeComponent() {
            bool isEn = Loc.CurrentLanguage == Language.English;
            this.Size = new Size(1250, 790); // 擴大寬度與高度以顯示 9 個屬性欄位且不要出現水平拉桿
            this.FormBorderStyle = FormBorderStyle.None;
            this.StartPosition = FormStartPosition.CenterParent;
            this.BackColor = Color.FromArgb(10, 11, 16);
            this.ForeColor = Color.FromArgb(230, 235, 240);
            this.DoubleBuffered = true;

            // 設置圓角
            IntPtr ptr = CreateRoundRectRgn(0, 0, this.Width, this.Height, 15, 15);
            this.Region = Region.FromHrgn(ptr);
            DeleteObject(ptr);

            // 1. 標題列
            pnlTitleBar = new Panel {
                Location = new Point(0, 0),
                Size = new Size(this.Width, 50),
                BackColor = Color.FromArgb(18, 19, 29)
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblTitle = new Label {
                Text = isEn 
                    ? "🛡️  Against Rome Troop Custom Preset Profile (6 Editable Stats)"
                    : "🛡️  Against Rome 兵種自訂屬性設定檔案 (6 項可自訂屬性)",
                Location = new Point(20, 13),
                Size = new Size(600, 25),
                Font = fontJhengHei10B,
                ForeColor = Color.FromArgb(0, 230, 255),
                BackColor = Color.Transparent
            };
            pnlTitleBar.Controls.Add(lblTitle);

            btnClose = new Button {
                Text = "×",
                Location = new Point(this.Width - 45, 10),
                Size = new Size(30, 30),
                FlatStyle = FlatStyle.Flat,
                Font = new Font("Arial", 14F, FontStyle.Bold),
                ForeColor = Color.FromArgb(150, 150, 160),
                BackColor = Color.Transparent,
                Cursor = Cursors.Hand
            };
            btnClose.FlatAppearance.BorderSize = 0;
            btnClose.FlatAppearance.MouseDownBackColor = Color.FromArgb(80, 20, 20);
            btnClose.FlatAppearance.MouseOverBackColor = Color.FromArgb(200, 40, 40);
            btnClose.MouseEnter += (s, e) => btnClose.ForeColor = Color.White;
            btnClose.MouseLeave += (s, e) => btnClose.ForeColor = Color.FromArgb(150, 150, 160);
            btnClose.Click += (s, e) => this.Close();
            pnlTitleBar.Controls.Add(btnClose);

            this.Controls.Add(pnlTitleBar);

            // 2. 陣營分類 TabControl
            tabFaction = new ModernTabControl {
                Location = new Point(20, 65),
                Size = new Size(1210, 635),
                Font = fontJhengHei95B
            };

            string[] factions = { "Teuton", "Celt", "Hun", "Roman" };
            string[] factionTexts = isEn 
                ? new string[] { "Teutons", "Celts", "Huns", "Romans" }
                : new string[] { "條頓 Teutons", "塞爾特 Celts", "匈奴 Huns", "羅馬 Romans" };

            for (int i = 0; i < factions.Length; i++) {
                string facKey = factions[i];
                string facText = factionTexts[i];

                TabPage tp = new TabPage(facText) {
                    BackColor = Color.FromArgb(10, 11, 16),
                    Padding = new Padding(3)
                };

                DataGridView dgv = CreateGrid();
                factionGrids[facKey] = dgv;
                tp.Controls.Add(dgv);

                tabFaction.TabPages.Add(tp);
            }
            this.Controls.Add(tabFaction);

            // 4. 按鈕
            btnImport = new Button { Text = isEn ? "Load Stats File (Import)" : "載入屬性檔 (匯入)", Location = new Point(20, 730), Size = isEn ? new Size(180, 36) : new Size(160, 36) };
            StyleButton(btnImport, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnImport.Click += BtnImport_Click;

            btnExport = new Button { Text = isEn ? "Save Stats File (Export)" : "儲存屬性檔 (匯出)", Location = isEn ? new Point(215, 730) : new Point(195, 730), Size = isEn ? new Size(180, 36) : new Size(160, 36) };
            StyleButton(btnExport, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(180, 100, 255));
            btnExport.Click += BtnExport_Click;

            btnApply = new Button { Text = isEn ? "Apply" : "確定套用", Location = new Point(950, 730), Size = new Size(130, 36) };
            StyleButton(btnApply, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnApply.Click += BtnApply_Click;

            btnCancel = new Button { Text = isEn ? "Cancel" : "取消", Location = new Point(1100, 730), Size = new Size(130, 36) };
            StyleButton(btnCancel, Color.FromArgb(45, 45, 55), Color.FromArgb(200, 200, 200), Color.FromArgb(255, 75, 75));
            btnCancel.Click += (s, e) => this.Close();

            this.Controls.Add(btnImport);
            this.Controls.Add(btnExport);
            this.Controls.Add(btnApply);
            this.Controls.Add(btnCancel);
        }

        private DataGridView CreateGrid() {
            bool isEn = Loc.CurrentLanguage == Language.English;
            var dgv = new DataGridView {
                Dock = DockStyle.Fill,
                AllowUserToAddRows = false,
                AllowUserToDeleteRows = false,
                RowHeadersVisible = false,
                BackgroundColor = Color.FromArgb(10, 11, 16),
                ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(28, 30, 42),
                BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false,
                RowTemplate = { Height = 42 },
                SelectionMode = DataGridViewSelectionMode.CellSelect,
                MultiSelect = false,
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

            var imgCol = new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = isEn ? "Icon" : "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 50,
                ReadOnly = true
            };
            dgv.Columns.Add(imgCol);

            dgv.Columns.Add("Key", isEn ? "Unit Key" : "兵種代碼");
            dgv.Columns["Key"].Visible = false;

            dgv.Columns.Add("Name", Loc.Get("HeaderName"));
            dgv.Columns["Name"].Width = 140;
            dgv.Columns["Name"].ReadOnly = true;

            dgv.Columns.Add("Faction", isEn ? "Faction" : "陣營");
            dgv.Columns["Faction"].Visible = false;
            dgv.Columns["Faction"].ReadOnly = true;

            dgv.Columns.Add("Tier", Loc.Get("HeaderTier"));
            dgv.Columns["Tier"].Width = 80;
            dgv.Columns["Tier"].ReadOnly = true;

            dgv.Columns.Add("Hp", Loc.Get("HeaderHp"));
            dgv.Columns["Hp"].Width = 90;

            dgv.Columns.Add("Dmg", isEn ? "Damage" : "傷害");
            dgv.Columns["Dmg"].Width = 90;

            dgv.Columns.Add("VW", Loc.Get("HeaderVw"));
            dgv.Columns["VW"].Width = 90;

            dgv.Columns.Add("AW", Loc.Get("HeaderAw"));
            dgv.Columns["AW"].Width = 90;

            dgv.Columns.Add("Sight", Loc.Get("HeaderSight"));
            dgv.Columns["Sight"].Width = 90;

            dgv.Columns.Add("Relt", isEn ? "Cooldown" : "攻擊冷卻");
            dgv.Columns["Relt"].Width = 95;

            return dgv;
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
                this.Location = new Point(p.X - dragStart.X, p.Y - dragStart.Y);
            }
        }
        private void TitleBar_MouseUp(object? sender, MouseEventArgs e) {
            dragging = false;
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

        private void StyleButton(Button btn, Color backColor, Color foreColor, Color hoverBorderColor) {
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 0; // 關閉邊框以利自繪
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
                    // 根據顏色判定角色並套用漸層
                    Color startColor, endColor;
                    if (backColor == Color.FromArgb(98, 0, 238)) { // Apply 確定套用 (Primary 紫色)
                        startColor = isHovered ? Color.FromArgb(120, 30, 255) : Color.FromArgb(98, 0, 238);
                        endColor = isHovered ? Color.FromArgb(160, 60, 255) : Color.FromArgb(130, 40, 255);
                    } else { // 預設按鈕 (卡片暗灰漸層)
                        startColor = isHovered ? Color.FromArgb(40, 42, 54) : Color.FromArgb(28, 30, 40);
                        endColor = isHovered ? Color.FromArgb(50, 52, 68) : Color.FromArgb(35, 37, 48);
                    }

                    using (LinearGradientBrush brush = new LinearGradientBrush(rect, startColor, endColor, 45F)) {
                        g.FillPath(brush, path);
                    }

                    // 繪製細線邊框
                    Color borderColor = isHovered ? hoverBorderColor : Color.FromArgb(50, 52, 70);
                    using (Pen p = new Pen(borderColor, 1.2F)) {
                        g.DrawPath(p, path);
                    }

                    // 繪製文字
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

        private void ApplyCustomStatsToGrid() {
            foreach (var dgv in factionGrids.Values) {
                dgv.Rows.Clear();
            }

            foreach (string key in TroopConfig.UnitOrder) {
                if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                var meta = TroopConfig.UnitMeta[key];
                string faction = meta.Faction;
                string tier = meta.Tier;
                string utype = meta.UnitType;

                string displayName = Loc.GetUnitName(key);
                string factionText = Loc.GetFactionName(faction);
                string tierText = Loc.GetTierText(tier);

                var iconImage = unitIcons.ContainsKey(key) ? unitIcons[key] : null;

                // 自訂功能只保留 HP、傷害、防禦、戰鬥、視野、攻擊冷卻；速度/射程/法術欄位已移除。
                double hp, dmg, vw, aw, speed, sight, relt, range, spellRadius;
                var baselineStats = statsService.GetBalanced(key);
                var independentStats = statsService.GetOriginal(key);
                if (CustomStats.ContainsKey(key)) {
                    var stats = CustomStats[key];
                    hp = stats[0];
                    dmg = stats[1];
                    vw = stats[2];
                    aw = stats[3];
                    speed = independentStats.Length > 4 ? independentStats[4] : 0;
                    sight = utype == "priest" && baselineStats.Length > 5 ? baselineStats[5] : (stats.Length > 5 ? stats[5] : 0);
                    relt = stats.Length > 6 ? stats[6] : 0;
                    range = independentStats.Length > 7 ? independentStats[7] : 0;
                    spellRadius = independentStats.Length > 8 ? independentStats[8] : 0;
                } else {
                    var stats = baselineStats;
                    hp = stats.Length > 0 ? stats[0] : 0;
                    dmg = stats.Length > 1 ? stats[1] : 0;
                    vw = stats.Length > 2 ? stats[2] : 0;
                    aw = stats.Length > 3 ? stats[3] : 0;
                    speed = stats.Length > 4 ? stats[4] : 0;
                    sight = stats.Length > 5 ? stats[5] : 0;
                    relt = stats.Length > 6 ? stats[6] : 0;
                    range = stats.Length > 7 ? stats[7] : 0;
                    spellRadius = stats.Length > 8 ? stats[8] : 0;
                }

                if (factionGrids.ContainsKey(faction)) {
                    int rowIndex = factionGrids[faction].Rows.Add(
                        iconImage,
                        key,
                        displayName,
                        factionText,
                        tierText,
                        Math.Round(hp).ToString(),
                        dmg.ToString("F1", CultureInfo.InvariantCulture),
                        Math.Round(vw).ToString(),
                        Math.Round(aw).ToString(),
                        Math.Round(sight).ToString(),
                        Math.Round(relt).ToString()
                    );
                    if (utype == "priest")
                        factionGrids[faction].Rows[rowIndex].Cells["Sight"].ReadOnly = true;
                }
            }
        }

        // 載入自訂檔案 (匯入)
        private void BtnImport_Click(object? sender, EventArgs e) {
            bool isEn = Loc.CurrentLanguage == Language.English;
            using (OpenFileDialog ofd = new OpenFileDialog {
                Filter = isEn ? "Troop Preset Files (*.artroop)|*.artroop" : "兵種自訂屬性檔 (*.artroop)|*.artroop",
                Title = isEn ? "Import Custom Troop Stats" : "匯入自訂兵種屬性"
            }) {
                if (ofd.ShowDialog() != DialogResult.OK) return;

                try {
                    TroopPresetParseResult parsedPreset = TroopPresetCodec.Parse(
                        File.ReadLines(ofd.FileName, Encoding.UTF8), statsService.GetBalanced);

                    foreach (var dgv in factionGrids.Values) {
                        for (int i = 0; i < dgv.Rows.Count; i++) {
                            string key = dgv.Rows[i].Cells["Key"].Value?.ToString() ?? "";
                            if (parsedPreset.Stats.TryGetValue(key, out double[]? stats)) {
                                // 以 0.## 呈現而非取整：保留檔案中的小數精度，套用時直接由表格讀回。
                                dgv.Rows[i].Cells["Hp"].Value = stats[0].ToString("0.##", CultureInfo.InvariantCulture);
                                dgv.Rows[i].Cells["Dmg"].Value = stats[1].ToString("0.##", CultureInfo.InvariantCulture);
                                dgv.Rows[i].Cells["VW"].Value = stats[2].ToString("0.##", CultureInfo.InvariantCulture);
                                dgv.Rows[i].Cells["AW"].Value = stats[3].ToString("0.##", CultureInfo.InvariantCulture);
                                dgv.Rows[i].Cells["Sight"].Value = TroopConfig.UnitMeta.TryGetValue(key, out var importedMeta) && importedMeta.UnitType == "priest"
                                    ? statsService.GetOriginal(key)[5].ToString("0.##", CultureInfo.InvariantCulture)
                                    : stats[5].ToString("0.##", CultureInfo.InvariantCulture);
                                dgv.Rows[i].Cells["Relt"].Value = stats[6].ToString("0.##", CultureInfo.InvariantCulture);
                            }
                        }
                    }

                    // 匯入成功後才更新檔名狀態，避免失敗時殘留「已載入」的假狀態。
                    this.LoadedFileName = Path.GetFileName(ofd.FileName);

                    string doneMsg = isEn ? "Custom troop stats successfully imported!" : "自訂兵種屬性匯入成功！";
                    if (parsedPreset.SkippedLines > 0) {
                        doneMsg += isEn
                            ? string.Format("\n({0} malformed line(s) were skipped.)", parsedPreset.SkippedLines)
                            : string.Format("\n（有 {0} 行格式錯誤已略過。）", parsedPreset.SkippedLines);
                    }
                    MessageBox.Show(doneMsg, isEn ? "Import Completed" : "匯入完成", MessageBoxButtons.OK, MessageBoxIcon.Information);
                } catch (Exception ex) {
                    MessageBox.Show((isEn ? "Failed to import stats file: " : "匯入屬性檔案失敗: ") + ex.Message, isEn ? "Error" : "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
            }
        }

        // 儲存自訂檔案 (匯出)
        private void BtnExport_Click(object? sender, EventArgs e) {
            if (!ValidateGridInputs()) return;

            bool isEn = Loc.CurrentLanguage == Language.English;
            using (SaveFileDialog sfd = new SaveFileDialog {
                Filter = isEn ? "Troop Preset Files (*.artroop)|*.artroop" : "兵種自訂屬性檔 (*.artroop)|*.artroop",
                DefaultExt = "artroop",
                FileName = "custom_troop_preset",
                Title = isEn ? "Export Custom Troop Stats" : "匯出自訂兵種屬性"
            }) {
                if (sfd.ShowDialog() != DialogResult.OK) return;

                try {
                    var stats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
                    foreach (var dgv in factionGrids.Values) {
                        for (int i = 0; i < dgv.Rows.Count; i++) {
                            string key = dgv.Rows[i].Cells["Key"].Value?.ToString() ?? "FigUnknown";
                            TryReadValidatedCell(dgv.Rows[i], "Hp", out double hp);
                            TryReadValidatedCell(dgv.Rows[i], "Dmg", out double damage);
                            TryReadValidatedCell(dgv.Rows[i], "VW", out double vw);
                            TryReadValidatedCell(dgv.Rows[i], "AW", out double aw);
                            TryReadValidatedCell(dgv.Rows[i], "Sight", out double sight);
                            TryReadValidatedCell(dgv.Rows[i], "Relt", out double reload);
                            stats[key] = new[] { hp, damage, vw, aw, 0, sight, reload, 0, 0 };
                        }
                    }

                    File.WriteAllText(sfd.FileName, TroopPresetCodec.Write(stats, DateTime.Now), Encoding.UTF8);
                    // 寫檔成功後才更新檔名狀態，避免失敗時殘留假狀態。
                    this.LoadedFileName = Path.GetFileName(sfd.FileName);
                    MessageBox.Show(isEn ? "Custom troop stats successfully exported!" : "自訂兵種屬性匯出成功！", isEn ? "Export Completed" : "匯出完成", MessageBoxButtons.OK, MessageBoxIcon.Information);
                } catch (Exception ex) {
                    MessageBox.Show((isEn ? "Failed to export stats file: " : "匯出屬性檔案失敗: ") + ex.Message, isEn ? "Error" : "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
            }
        }

        // 確定套用
        private void BtnApply_Click(object? sender, EventArgs e) {
            if (!ValidateGridInputs()) return;

            CustomStats.Clear();
            foreach (var dgv in factionGrids.Values) {
                for (int i = 0; i < dgv.Rows.Count; i++) {
                    string key = dgv.Rows[i].Cells["Key"].Value?.ToString() ?? "";
                    if (string.IsNullOrEmpty(key)) continue;

                    if (!TryReadValidatedCell(dgv.Rows[i], "Hp", out double hp)) return;
                    if (!TryReadValidatedCell(dgv.Rows[i], "Dmg", out double dmg)) return;
                    if (!TryReadValidatedCell(dgv.Rows[i], "VW", out double vw)) return;
                    if (!TryReadValidatedCell(dgv.Rows[i], "AW", out double aw)) return;
                    if (!TryReadValidatedCell(dgv.Rows[i], "Sight", out double sight)) return;
                    if (!TryReadValidatedCell(dgv.Rows[i], "Relt", out double relt)) return;
                    double[] original = statsService.GetOriginal(key);
                    double speed = original.Length > 4 ? original[4] : 0;
                    double range = original.Length > 7 ? original[7] : 0;
                    double spellRadius = original.Length > 8 ? original[8] : 0;

                    CustomStats[key] = statsService.NormalizeIndependentFields(key,
                        new double[] { hp, dmg, vw, aw, speed, sight, relt, range, spellRadius });
                }
            }

            this.DialogResult = DialogResult.OK;
            this.Close();
        }

        private static bool TryReadValidatedCell(DataGridViewRow row, string columnName, out double value) {
            string text = row.Cells[columnName].Value?.ToString() ?? "";
            return double.TryParse(text, NumberStyles.Any, CultureInfo.InvariantCulture, out value);
        }

        // 完整的 9 欄位輸入驗證與防呆
        private bool ValidateGridInputs() {
            bool isEn = Loc.CurrentLanguage == Language.English;
            string errTitle = isEn ? "Input Error" : "輸入錯誤";
            string unknownUnit = isEn ? "Unknown Unit" : "未知兵種";

            foreach (var dgv in factionGrids.Values) {
                dgv.EndEdit();
 
                for (int i = 0; i < dgv.Rows.Count; i++) {
                    string unitName = dgv.Rows[i].Cells["Name"].Value?.ToString() ?? unknownUnit;
                    
                    string hpVal = dgv.Rows[i].Cells["Hp"].Value?.ToString() ?? "";
                    string dmgVal = dgv.Rows[i].Cells["Dmg"].Value?.ToString() ?? "";
                    string vwVal = dgv.Rows[i].Cells["VW"].Value?.ToString() ?? "";
                    string awVal = dgv.Rows[i].Cells["AW"].Value?.ToString() ?? "";
                    string sightVal = dgv.Rows[i].Cells["Sight"].Value?.ToString() ?? "";
                    string reltVal = dgv.Rows[i].Cells["Relt"].Value?.ToString() ?? "";
 
                    double val;
 
                    if (!double.TryParse(hpVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val <= 0) {
                        string msg = isEn 
                            ? string.Format("HP value of 【{0}】 must be a valid number greater than 0!", unitName)
                            : string.Format("【{0}】的生命值 (HP) 必須是有效且大於 0 的數值！", unitName);
                        MessageBox.Show(msg, errTitle, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }
 
                    if (!double.TryParse(dmgVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        string msg = isEn 
                            ? string.Format("Damage (Dmg) of 【{0}】 must be a valid number greater than or equal to 0!", unitName)
                            : string.Format("【{0}】的傷害 (Dmg) 必須是有效且大於等於 0 的數值！", unitName);
                        MessageBox.Show(msg, errTitle, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }
 
                    if (!double.TryParse(vwVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        string msg = isEn 
                            ? string.Format("Defense (VW) of 【{0}】 must be a valid number greater than or equal to 0!", unitName)
                            : string.Format("【{0}】的防禦力 (VW) 必須是有效且大於等於 0 的數值！", unitName);
                        MessageBox.Show(msg, errTitle, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }
 
                    if (!double.TryParse(awVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        string msg = isEn 
                            ? string.Format("Combat (AW) of 【{0}】 must be a valid number greater than or equal to 0!", unitName)
                            : string.Format("【{0}】的戰鬥力 (AW) 必須是有效且大於等於 0 的數值！", unitName);
                        MessageBox.Show(msg, errTitle, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }
 
                    if (!double.TryParse(sightVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        string msg = isEn 
                            ? string.Format("Sight of 【{0}】 must be a valid number greater than or equal to 0!", unitName)
                            : string.Format("【{0}】的視野 必須是有效且大於等於 0 的數值！", unitName);
                        MessageBox.Show(msg, errTitle, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }
 
                    if (!double.TryParse(reltVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        string msg = isEn 
                            ? string.Format("Cooldown of 【{0}】 must be a valid number greater than or equal to 0!", unitName)
                            : string.Format("【{0}】的攻擊冷卻 必須是有效且大於等於 0 的數值！", unitName);
                        MessageBox.Show(msg, errTitle, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                }
            }

            return true;
        }

        protected override void Dispose(bool disposing) {
            if (disposing && !fontsDisposed) {
                fontJhengHei10B.Dispose();
                fontJhengHei95B.Dispose();
                fontJhengHei9R.Dispose();
                fontsDisposed = true;
            }
            base.Dispose(disposing);
        }
    }
}
