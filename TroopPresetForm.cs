using System;
using System.IO;
using System.Text;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using System.Globalization;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Linq;

namespace AgainstRomeModifier {
    public class TroopPresetForm : Form {
        [DllImport("Gdi32.dll", EntryPoint = "CreateRoundRectRgn")]
        private static extern IntPtr CreateRoundRectRgn(int nLeftRect, int nTopRect, int nRightRect, int nBottomRect, int nWidthEllipse, int nHeightEllipse);

        private ModifierForm mainForm;
        private Dictionary<string, Bitmap> unitIcons;
        
        // 傳遞編輯後結果的 Dictionary (每個兵種有 9 個屬性值)
        public Dictionary<string, double[]> CustomStats { get; private set; }
        public string LoadedFileName { get; private set; } = "";

        // UI 元件
        private Panel pnlTitleBar = null!;
        private Label lblTitle = null!;
        private Button btnClose = null!;
        
        private Label lblTemplate = null!;
        private ComboBox cbTemplate = null!;
        
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

        public TroopPresetForm(ModifierForm mainForm, Dictionary<string, double[]>? currentCustomStats, Dictionary<string, Bitmap> unitIcons) {
            this.mainForm = mainForm;
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
            this.Size = new Size(1250, 790); // 擴大寬度與高度以顯示 9 個屬性欄位且不要出現水平拉桿
            this.FormBorderStyle = FormBorderStyle.None;
            this.StartPosition = FormStartPosition.CenterParent;
            this.BackColor = Color.FromArgb(16, 16, 20);
            this.ForeColor = Color.FromArgb(230, 235, 240);
            this.DoubleBuffered = true;

            // 設置圓角
            IntPtr ptr = CreateRoundRectRgn(0, 0, this.Width, this.Height, 15, 15);
            this.Region = Region.FromHrgn(ptr);

            // 1. 標題列
            pnlTitleBar = new Panel {
                Location = new Point(0, 0),
                Size = new Size(this.Width, 50),
                BackColor = Color.FromArgb(24, 24, 30)
            };
            pnlTitleBar.MouseDown += TitleBar_MouseDown;
            pnlTitleBar.MouseMove += TitleBar_MouseMove;
            pnlTitleBar.MouseUp += TitleBar_MouseUp;

            lblTitle = new Label {
                Text = "🛡️  Against Rome 兵種自訂屬性設定檔案 (9 大屬性全面開放)",
                Location = new Point(20, 13),
                Size = new Size(500, 25),
                Font = fontJhengHei10B,
                ForeColor = Color.FromArgb(0, 220, 255),
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

            // 2. 範本選擇
            lblTemplate = new Label {
                Text = "選擇預設屬性範本:",
                Location = new Point(20, 68),
                Size = new Size(130, 25),
                Font = fontJhengHei95B,
                ForeColor = Color.FromArgb(200, 205, 210),
                BackColor = Color.Transparent
            };
            this.Controls.Add(lblTemplate);

            cbTemplate = new ComboBox {
                Location = new Point(160, 65),
                Size = new Size(200, 28),
                DropDownStyle = ComboBoxStyle.DropDownList,
                BackColor = Color.FromArgb(32, 32, 40),
                ForeColor = Color.White,
                Font = fontJhengHei95B,
                FlatStyle = FlatStyle.Flat
            };
            cbTemplate.Items.AddRange(new string[] { "(請選擇範本)", "官方原版數值", "修改器內建平衡" });
            cbTemplate.SelectedIndex = 0;
            cbTemplate.SelectedIndexChanged += CbTemplate_SelectedIndexChanged;
            this.Controls.Add(cbTemplate);

            // 3. 陣營分類 TabControl
            tabFaction = new TabControl {
                Location = new Point(20, 110),
                Size = new Size(1210, 590),
                Font = fontJhengHei95B
            };

            string[] factions = { "Teuton", "Celt", "Hun", "Roman" };
            string[] factionTexts = { "條頓 Teutons", "塞爾特 Celts", "匈奴 Huns", "羅馬 Romans" };

            for (int i = 0; i < factions.Length; i++) {
                string facKey = factions[i];
                string facText = factionTexts[i];

                TabPage tp = new TabPage(facText) {
                    BackColor = Color.FromArgb(16, 16, 20),
                    Padding = new Padding(3)
                };

                DataGridView dgv = CreateGrid();
                factionGrids[facKey] = dgv;
                tp.Controls.Add(dgv);

                tabFaction.TabPages.Add(tp);
            }
            this.Controls.Add(tabFaction);

            // 4. 按鈕
            btnImport = new Button { Text = "載入屬性檔 (匯入)", Location = new Point(20, 730), Size = new Size(160, 36) };
            StyleButton(btnImport, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(0, 220, 255));
            btnImport.Click += BtnImport_Click;

            btnExport = new Button { Text = "儲存屬性檔 (匯出)", Location = new Point(195, 730), Size = new Size(160, 36) };
            StyleButton(btnExport, Color.FromArgb(45, 45, 55), Color.FromArgb(240, 240, 240), Color.FromArgb(180, 100, 255));
            btnExport.Click += BtnExport_Click;

            btnApply = new Button { Text = "確定套用", Location = new Point(950, 730), Size = new Size(130, 36) };
            StyleButton(btnApply, Color.FromArgb(98, 0, 238), Color.White, Color.FromArgb(180, 100, 255));
            btnApply.Click += BtnApply_Click;

            btnCancel = new Button { Text = "取消", Location = new Point(1100, 730), Size = new Size(130, 36) };
            StyleButton(btnCancel, Color.FromArgb(45, 45, 55), Color.FromArgb(200, 200, 200), Color.FromArgb(255, 75, 75));
            btnCancel.Click += (s, e) => this.Close();

            this.Controls.Add(btnImport);
            this.Controls.Add(btnExport);
            this.Controls.Add(btnApply);
            this.Controls.Add(btnCancel);
        }

        private DataGridView CreateGrid() {
            var dgv = new DataGridView {
                Dock = DockStyle.Fill,
                AllowUserToAddRows = false,
                AllowUserToDeleteRows = false,
                RowHeadersVisible = false,
                BackgroundColor = Color.FromArgb(20, 20, 25),
                ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(45, 45, 55),
                BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false,
                RowTemplate = { Height = 42 },
                SelectionMode = DataGridViewSelectionMode.CellSelect,
                MultiSelect = false,
                ScrollBars = ScrollBars.Vertical // 只允許垂直拉桿，且因為行數少，Windows 預設會自動隱藏！徹底避免水平拉桿。
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 220, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 36;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(24, 24, 30);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(45, 45, 60);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(28, 28, 35);

            var imgCol = new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 50,
                ReadOnly = true
            };
            dgv.Columns.Add(imgCol);

            dgv.Columns.Add("Key", "兵種代碼");
            dgv.Columns["Key"].Visible = false;

            dgv.Columns.Add("Name", "兵種名稱");
            dgv.Columns["Name"].Width = 140;
            dgv.Columns["Name"].ReadOnly = true;

            dgv.Columns.Add("Faction", "陣營");
            dgv.Columns["Faction"].Visible = false;
            dgv.Columns["Faction"].ReadOnly = true;

            dgv.Columns.Add("Tier", "階級");
            dgv.Columns["Tier"].Width = 80;
            dgv.Columns["Tier"].ReadOnly = true;

            dgv.Columns.Add("Hp", "生命值");
            dgv.Columns["Hp"].Width = 90;

            dgv.Columns.Add("Dmg", "傷害");
            dgv.Columns["Dmg"].Width = 90;

            dgv.Columns.Add("VW", "防禦力");
            dgv.Columns["VW"].Width = 90;

            dgv.Columns.Add("AW", "戰鬥力");
            dgv.Columns["AW"].Width = 90;

            dgv.Columns.Add("Speed", "移動速度");
            dgv.Columns["Speed"].Width = 95;

            dgv.Columns.Add("Sight", "視野");
            dgv.Columns["Sight"].Width = 90;

            dgv.Columns.Add("Relt", "攻擊冷卻");
            dgv.Columns["Relt"].Width = 95;

            dgv.Columns.Add("Range", "最大射程");
            dgv.Columns["Range"].Width = 100;

            dgv.Columns.Add("SpellRadius", "法術半徑");
            dgv.Columns["SpellRadius"].Width = 95;

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

        private void StyleButton(Button btn, Color backColor, Color foreColor, Color hoverBorderColor) {
            btn.FlatStyle = FlatStyle.Flat;
            btn.FlatAppearance.BorderSize = 1;
            btn.FlatAppearance.BorderColor = Color.FromArgb(60, 60, 70);
            btn.BackColor = backColor;
            btn.ForeColor = foreColor;
            btn.Cursor = Cursors.Hand;
            btn.Font = fontJhengHei95B;

            btn.MouseEnter += (s, e) => {
                btn.FlatAppearance.BorderColor = hoverBorderColor;
                if (backColor == Color.FromArgb(45, 45, 55)) {
                    btn.BackColor = Color.FromArgb(55, 55, 68);
                } else if (backColor == Color.FromArgb(98, 0, 238)) {
                    btn.BackColor = Color.FromArgb(120, 40, 255);
                }
            };
            btn.MouseLeave += (s, e) => {
                btn.FlatAppearance.BorderColor = Color.FromArgb(60, 60, 70);
                btn.BackColor = backColor;
            };
        }

        private void ApplyCustomStatsToGrid() {
            foreach (var dgv in factionGrids.Values) {
                dgv.Rows.Clear();
            }

            foreach (string key in TroopConfig.UnitOrder) {
                if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                var meta = TroopConfig.UnitMeta[key];
                string faction = meta.Item1;
                string tier = meta.Item2;
                string utype = meta.Item3;

                string displayName = Loc.GetUnitName(key);
                string factionText = Loc.GetFactionName(faction);
                string tierText = Loc.GetTierText(tier);

                var iconImage = unitIcons.ContainsKey(key) ? unitIcons[key] : null;

                // 獲取 9 大屬性
                double hp, dmg, vw, aw, speed, sight, relt, range, spellRadius;
                if (CustomStats.ContainsKey(key)) {
                    var stats = CustomStats[key];
                    hp = stats[0];
                    dmg = stats[1];
                    vw = stats[2];
                    aw = stats[3];
                    speed = stats.Length > 4 ? stats[4] : 0;
                    sight = stats.Length > 5 ? stats[5] : 0;
                    relt = stats.Length > 6 ? stats[6] : 0;
                    range = stats.Length > 7 ? stats[7] : 0;
                    spellRadius = stats.Length > 8 ? stats[8] : 0;
                } else {
                    var stats = mainForm.GetDefaultBalancedStats(key);
                    hp = stats[0];
                    dmg = stats[1];
                    vw = stats[2];
                    aw = stats[3];
                    speed = stats[4];
                    sight = stats[5];
                    relt = stats[6];
                    range = stats[7];
                    spellRadius = stats[8];
                }

                if (factionGrids.ContainsKey(faction)) {
                    factionGrids[faction].Rows.Add(
                        iconImage,
                        key,
                        displayName,
                        factionText,
                        tierText,
                        Math.Round(hp).ToString(),
                        dmg.ToString("F1", CultureInfo.InvariantCulture),
                        Math.Round(vw).ToString(),
                        Math.Round(aw).ToString(),
                        speed.ToString("F1", CultureInfo.InvariantCulture),
                        Math.Round(sight).ToString(),
                        Math.Round(relt).ToString(),
                        Math.Round(range).ToString(),
                        Math.Round(spellRadius).ToString()
                    );
                }
            }
        }

        private void CbTemplate_SelectedIndexChanged(object? sender, EventArgs e) {
            int index = cbTemplate.SelectedIndex;
            if (index == 0) return;

            string text = index == 1 ? "確定要套用「官方原版數值」範本嗎？這將覆蓋目前表格中的編輯！" : "確定要套用「修改器內建平衡」範本嗎？這將覆蓋目前表格中的編輯！";
            if (MessageBox.Show(text, "確認範本覆蓋", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) {
                cbTemplate.SelectedIndex = 0;
                return;
            }

            foreach (var dgv in factionGrids.Values) {
                for (int i = 0; i < dgv.Rows.Count; i++) {
                    string key = dgv.Rows[i].Cells["Key"].Value?.ToString() ?? "";
                    if (string.IsNullOrEmpty(key)) continue;

                    double[] stats = index == 1 ? mainForm.GetOriginalStats(key) : mainForm.GetDefaultBalancedStats(key);
                    dgv.Rows[i].Cells["Hp"].Value = Math.Round(stats[0]).ToString();
                    dgv.Rows[i].Cells["Dmg"].Value = stats[1].ToString("F1", CultureInfo.InvariantCulture);
                    dgv.Rows[i].Cells["VW"].Value = Math.Round(stats[2]).ToString();
                    dgv.Rows[i].Cells["AW"].Value = Math.Round(stats[3]).ToString();
                    dgv.Rows[i].Cells["Speed"].Value = stats[4].ToString("F1", CultureInfo.InvariantCulture);
                    dgv.Rows[i].Cells["Sight"].Value = Math.Round(stats[5]).ToString();
                    dgv.Rows[i].Cells["Relt"].Value = Math.Round(stats[6]).ToString();
                    dgv.Rows[i].Cells["Range"].Value = Math.Round(stats[7]).ToString();
                    dgv.Rows[i].Cells["SpellRadius"].Value = Math.Round(stats[8]).ToString();
                }
            }

            cbTemplate.SelectedIndex = 0;
        }

        // 載入自訂檔案 (匯入)
        private void BtnImport_Click(object? sender, EventArgs e) {
            OpenFileDialog ofd = new OpenFileDialog {
                Filter = "兵種自訂屬性檔 (*.artroop)|*.artroop",
                Title = "匯入自訂兵種屬性"
            };
            if (ofd.ShowDialog() != DialogResult.OK) return;
            this.LoadedFileName = Path.GetFileName(ofd.FileName);

            try {
                string[] lines = File.ReadAllLines(ofd.FileName, Encoding.UTF8);
                var loadedStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);

                foreach (string line in lines) {
                    string l = line.Trim();
                    if (string.IsNullOrEmpty(l) || l.StartsWith("#") || l.StartsWith(";")) continue;
                    if (!l.Contains("=")) continue;

                    string[] kv = l.Split(new char[] { '=' }, 2);
                    string key = kv[0].Trim();
                    string[] vals = kv[1].Split(',');

                    if (vals.Length >= 4) {
                        double hp = double.Parse(vals[0].Trim(), CultureInfo.InvariantCulture);
                        double dmg = double.Parse(vals[1].Trim(), CultureInfo.InvariantCulture);
                        double vw = double.Parse(vals[2].Trim(), CultureInfo.InvariantCulture);
                        double aw = double.Parse(vals[3].Trim(), CultureInfo.InvariantCulture);
                        
                        // 支援相容讀取舊版 4 欄位或新版 9 欄位
                        double speed = vals.Length > 4 ? double.Parse(vals[4].Trim(), CultureInfo.InvariantCulture) : 0;
                        double sight = vals.Length > 5 ? double.Parse(vals[5].Trim(), CultureInfo.InvariantCulture) : 0;
                        double relt = vals.Length > 6 ? double.Parse(vals[6].Trim(), CultureInfo.InvariantCulture) : 0;
                        double range = vals.Length > 7 ? double.Parse(vals[7].Trim(), CultureInfo.InvariantCulture) : 0;
                        double spellRadius = vals.Length > 8 ? double.Parse(vals[8].Trim(), CultureInfo.InvariantCulture) : 0;

                        // 若為舊版屬性沒有配置剩餘欄位，匯入時會自動取內建平衡值
                        if (vals.Length < 9) {
                            double[] defStats = mainForm.GetDefaultBalancedStats(key);
                            if (vals.Length <= 4) speed = defStats[4];
                            if (vals.Length <= 5) sight = defStats[5];
                            if (vals.Length <= 6) relt = defStats[6];
                            if (vals.Length <= 7) range = defStats[7];
                            if (vals.Length <= 8) spellRadius = defStats[8];
                        }

                        loadedStats[key] = new double[] { hp, dmg, vw, aw, speed, sight, relt, range, spellRadius };
                    }
                }

                // 填入 各個 DataGridView
                foreach (var dgv in factionGrids.Values) {
                    for (int i = 0; i < dgv.Rows.Count; i++) {
                        string key = dgv.Rows[i].Cells["Key"].Value?.ToString() ?? "";
                        if (loadedStats.ContainsKey(key)) {
                            var stats = loadedStats[key];
                            dgv.Rows[i].Cells["Hp"].Value = Math.Round(stats[0]).ToString();
                            dgv.Rows[i].Cells["Dmg"].Value = stats[1].ToString("F1", CultureInfo.InvariantCulture);
                            dgv.Rows[i].Cells["VW"].Value = Math.Round(stats[2]).ToString();
                            dgv.Rows[i].Cells["AW"].Value = Math.Round(stats[3]).ToString();
                            dgv.Rows[i].Cells["Speed"].Value = stats[4].ToString("F1", CultureInfo.InvariantCulture);
                            dgv.Rows[i].Cells["Sight"].Value = Math.Round(stats[5]).ToString();
                            dgv.Rows[i].Cells["Relt"].Value = Math.Round(stats[6]).ToString();
                            dgv.Rows[i].Cells["Range"].Value = Math.Round(stats[7]).ToString();
                            dgv.Rows[i].Cells["SpellRadius"].Value = Math.Round(stats[8]).ToString();
                        }
                    }
                }

                MessageBox.Show("自訂兵種屬性匯入成功！", "匯入完成", MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                MessageBox.Show("匯入屬性檔案失敗: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        // 儲存自訂檔案 (匯出)
        private void BtnExport_Click(object? sender, EventArgs e) {
            if (!ValidateGridInputs()) return;

            SaveFileDialog sfd = new SaveFileDialog {
                Filter = "兵種自訂屬性檔 (*.artroop)|*.artroop",
                DefaultExt = "artroop",
                FileName = "custom_troop_preset",
                Title = "匯出自訂兵種屬性"
            };
            if (sfd.ShowDialog() != DialogResult.OK) return;
            this.LoadedFileName = Path.GetFileName(sfd.FileName);

            try {
                StringBuilder sb = new StringBuilder();
                sb.AppendLine("# Against Rome Modifier - Custom Troop Preset File (9 Stats Mode)");
                sb.AppendLine(string.Format("# Generated on: {0}", DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss")));
                sb.AppendLine("# Format: UnitKey=HP,Dmg,VW,AW,Speed,Sight,Relt,Range,SpellRadius");
                sb.AppendLine();

                foreach (var dgv in factionGrids.Values) {
                    for (int i = 0; i < dgv.Rows.Count; i++) {
                        string key = dgv.Rows[i].Cells["Key"].Value?.ToString() ?? "FigUnknown";
                        string hp = dgv.Rows[i].Cells["Hp"].Value?.ToString() ?? "0";
                        string dmg = dgv.Rows[i].Cells["Dmg"].Value?.ToString() ?? "0";
                        string vw = dgv.Rows[i].Cells["VW"].Value?.ToString() ?? "0";
                        string aw = dgv.Rows[i].Cells["AW"].Value?.ToString() ?? "0";
                        string speed = dgv.Rows[i].Cells["Speed"].Value?.ToString() ?? "0";
                        string sight = dgv.Rows[i].Cells["Sight"].Value?.ToString() ?? "0";
                        string relt = dgv.Rows[i].Cells["Relt"].Value?.ToString() ?? "0";
                        string range = dgv.Rows[i].Cells["Range"].Value?.ToString() ?? "0";
                        string spellRadius = dgv.Rows[i].Cells["SpellRadius"].Value?.ToString() ?? "0";

                        sb.AppendLine(string.Format(CultureInfo.InvariantCulture, "{0}={1},{2},{3},{4},{5},{6},{7},{8},{9}", 
                            key, hp, dmg, vw, aw, speed, sight, relt, range, spellRadius));
                    }
                }

                File.WriteAllText(sfd.FileName, sb.ToString(), Encoding.UTF8);
                MessageBox.Show("自訂兵種屬性匯出成功！", "匯出完成", MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                MessageBox.Show("匯出屬性檔案失敗: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
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

                    double hp = double.Parse(dgv.Rows[i].Cells["Hp"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double dmg = double.Parse(dgv.Rows[i].Cells["Dmg"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double vw = double.Parse(dgv.Rows[i].Cells["VW"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double aw = double.Parse(dgv.Rows[i].Cells["AW"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double speed = double.Parse(dgv.Rows[i].Cells["Speed"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double sight = double.Parse(dgv.Rows[i].Cells["Sight"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double relt = double.Parse(dgv.Rows[i].Cells["Relt"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double range = double.Parse(dgv.Rows[i].Cells["Range"].Value!.ToString()!, CultureInfo.InvariantCulture);
                    double spellRadius = double.Parse(dgv.Rows[i].Cells["SpellRadius"].Value!.ToString()!, CultureInfo.InvariantCulture);

                    CustomStats[key] = new double[] { hp, dmg, vw, aw, speed, sight, relt, range, spellRadius };
                }
            }

            this.DialogResult = DialogResult.OK;
            this.Close();
        }

        // 完整的 9 欄位輸入驗證與防呆
        private bool ValidateGridInputs() {
            foreach (var dgv in factionGrids.Values) {
                dgv.EndEdit();

                for (int i = 0; i < dgv.Rows.Count; i++) {
                    string unitName = dgv.Rows[i].Cells["Name"].Value?.ToString() ?? "未知兵種";
                    
                    string hpVal = dgv.Rows[i].Cells["Hp"].Value?.ToString() ?? "";
                    string dmgVal = dgv.Rows[i].Cells["Dmg"].Value?.ToString() ?? "";
                    string vwVal = dgv.Rows[i].Cells["VW"].Value?.ToString() ?? "";
                    string awVal = dgv.Rows[i].Cells["AW"].Value?.ToString() ?? "";
                    string speedVal = dgv.Rows[i].Cells["Speed"].Value?.ToString() ?? "";
                    string sightVal = dgv.Rows[i].Cells["Sight"].Value?.ToString() ?? "";
                    string reltVal = dgv.Rows[i].Cells["Relt"].Value?.ToString() ?? "";
                    string rangeVal = dgv.Rows[i].Cells["Range"].Value?.ToString() ?? "";
                    string spellRadiusVal = dgv.Rows[i].Cells["SpellRadius"].Value?.ToString() ?? "";

                    double val;

                    if (!double.TryParse(hpVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val <= 0) {
                        MessageBox.Show(string.Format("【{0}】的生命值 (HP) 必須是有效且大於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(dmgVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的傷害 (Dmg) 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(vwVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的防禦力 (VW) 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(awVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的戰鬥力 (AW) 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(speedVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的移動速度 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(sightVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的視野 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(reltVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的攻擊冷卻 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(rangeVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的最大射程/施法距離 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }

                    if (!double.TryParse(spellRadiusVal, NumberStyles.Any, CultureInfo.InvariantCulture, out val) || val < 0) {
                        MessageBox.Show(string.Format("【{0}】的法術半徑 必須是有效且大於等於 0 的數值！", unitName), "輸入錯誤", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                        return false;
                    }
                }
            }

            return true;
        }

        protected override void OnFormClosing(FormClosingEventArgs e) {
            fontJhengHei10B.Dispose();
            fontJhengHei95B.Dispose();
            fontJhengHei9R.Dispose();
            base.OnFormClosing(e);
        }
    }
}
