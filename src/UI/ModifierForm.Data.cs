using System;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Drawing;
using AgainstRomeModifier.Core.Patches;
using System.Drawing.Drawing2D;
using AgainstRomeModifier.Core.Features;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private static readonly Regex RegexSpellLoad = new Regex(@"Radius\s*=\s*(?:HUN|KEL|GER)\s*,\s*Spell\d+\s*,\s*(\d+)", RegexOptions.Compiled);

        /// <summary>
        /// 獲取 UI 文字框中設定的遊戲路徑。
        /// </summary>
        private string GetGamePath() {
            return txtGamePath != null ? txtGamePath.Text.Trim() : "";
        }

        /// <summary>
        /// 解析遊戲資料列。遊戲資料使用單純逗號分隔，不支援 RFC 4180 引號跳脫。
        /// </summary>
        private static string[] ParseCsvLine(string line) {
            if (line == null) return Array.Empty<string>();
            return line.Split(',');
        }

        /// <summary>
        /// 從系統登錄檔中自動偵測《Against Rome》的安裝路徑。
        /// </summary>
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
            } catch (Exception ex) { System.Diagnostics.Debug.WriteLine("讀取登錄檔遊戲路徑失敗: " + ex.Message); }
            return "";
        }

        /// <summary>
        /// 解析 TGA 圖像位元組資料，並將其轉換成 GDI+ 的 Bitmap 物件。
        /// 支援 8 位元索引彩色（附 24 位元調色盤）以及 24/32 位元真彩色 TGA 圖檔。
        /// </summary>
        public static Bitmap? LoadTga(byte[] tgaBytes) {
            if (tgaBytes.Length < 18) return null;
            int idLength = tgaBytes[0];
            int colorMapType = tgaBytes[1];
            int imageType = tgaBytes[2];
            int width = BitConverter.ToUInt16(tgaBytes, 12);
            int height = BitConverter.ToUInt16(tgaBytes, 14);
            int pixelDepth = tgaBytes[16];
            int descriptor = tgaBytes[17];

            if (width <= 0 || height <= 0) return null;

            if (imageType == 1) {
                if (colorMapType != 1 || pixelDepth != 8) return null;
                int colorMapLength = BitConverter.ToUInt16(tgaBytes, 5);
                int colorMapEntrySize = tgaBytes[7];
                if (colorMapEntrySize != 24) return null;
                int colorMapOffset = 18 + idLength;
                int pixelDataOffset = colorMapOffset + colorMapLength * 3;

                if (pixelDataOffset + width * height > tgaBytes.Length) return null;

                Color[] palette = new Color[colorMapLength];
                for (int i = 0; i < colorMapLength; i++) {
                    int entryOffset = colorMapOffset + i * 3;
                    if (entryOffset + 2 >= tgaBytes.Length) break;
                    byte b = tgaBytes[entryOffset];
                    byte g = tgaBytes[entryOffset + 1];
                    byte r = tgaBytes[entryOffset + 2];
                    // Against Rome indexed UI sprites use pure black palette entries as transparent.
                    if (r == 0 && g == 0 && b == 0) {
                        palette[i] = Color.FromArgb(0, 0, 0, 0);
                    } else {
                        palette[i] = Color.FromArgb(255, r, g, b);
                    }
                }

                Bitmap bmp = new Bitmap(width, height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
                var bmpData = bmp.LockBits(new Rectangle(0, 0, width, height), System.Drawing.Imaging.ImageLockMode.WriteOnly, bmp.PixelFormat);
                try {
                    bool topToBottom = (descriptor & 0x20) != 0;
                    int stride = bmpData.Stride;
                    byte[] argbBuffer = new byte[stride * height];
                    for (int y = 0; y < height; y++) {
                        int targetY = topToBottom ? y : (height - 1 - y);
                        int targetOffset = targetY * stride;
                        int rowDataOffset = pixelDataOffset + y * width;
                        for (int x = 0; x < width; x++) {
                            int pixelOffset = rowDataOffset + x;
                            if (pixelOffset >= tgaBytes.Length) break;
                            byte index = tgaBytes[pixelOffset];
                            Color c = (index < palette.Length) ? palette[index] : Color.Transparent;
                            int pixel = targetOffset + x * 4;
                            argbBuffer[pixel] = c.B;
                            argbBuffer[pixel + 1] = c.G;
                            argbBuffer[pixel + 2] = c.R;
                            argbBuffer[pixel + 3] = c.A;
                        }
                    }
                    System.Runtime.InteropServices.Marshal.Copy(argbBuffer, 0, bmpData.Scan0, argbBuffer.Length);
                } finally {
                    bmp.UnlockBits(bmpData);
                }
                return bmp;
            } else if (imageType == 2) {
                if (pixelDepth != 24 && pixelDepth != 32) return null;
                int pixelDataOffset = 18 + idLength;
                int bytesPerPixel = pixelDepth / 8;

                if (pixelDataOffset + width * height * bytesPerPixel > tgaBytes.Length) return null;

                Bitmap bmp = new Bitmap(width, height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
                var bmpData = bmp.LockBits(new Rectangle(0, 0, width, height), System.Drawing.Imaging.ImageLockMode.WriteOnly, bmp.PixelFormat);
                try {
                    bool topToBottom = (descriptor & 0x20) != 0;
                    int stride = bmpData.Stride;
                    byte[] argbBuffer = new byte[stride * height];
                    for (int y = 0; y < height; y++) {
                        int targetY = topToBottom ? y : (height - 1 - y);
                        int targetOffset = targetY * stride;
                        int rowDataOffset = pixelDataOffset + y * width * bytesPerPixel;
                        for (int x = 0; x < width; x++) {
                            int pixelOffset = rowDataOffset + x * bytesPerPixel;
                            if (pixelOffset + 2 >= tgaBytes.Length) break;
                            byte b = tgaBytes[pixelOffset];
                            byte g = tgaBytes[pixelOffset + 1];
                            byte r = tgaBytes[pixelOffset + 2];
                            byte a = 255;
                            if (bytesPerPixel == 4 && pixelOffset + 3 < tgaBytes.Length) {
                                a = tgaBytes[pixelOffset + 3];
                            }
                            int pixel = targetOffset + x * 4;
                            argbBuffer[pixel] = b;
                            argbBuffer[pixel + 1] = g;
                            argbBuffer[pixel + 2] = r;
                            argbBuffer[pixel + 3] = a;
                        }
                    }
                    System.Runtime.InteropServices.Marshal.Copy(argbBuffer, 0, bmpData.Scan0, argbBuffer.Length);
                } finally {
                    bmp.UnlockBits(bmpData);
                }
                return bmp;
            }
            return null;
        }



        /// <summary>
        /// 從遊戲目錄下的 gui.dat 以及內嵌的 icon.ini 中載入所有兵種對應的 TGA 圖示，
        /// 並轉換成 Bitmap 快取至記憶體中。
        /// </summary>
        private void LoadIcons() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                Log(Loc.Get("LogGamePathNotSetIcon"));
                return;
            }
            string guiDatPath = Path.Combine(gamePath, "gui.dat");
            if (!File.Exists(guiDatPath)) {
                Log(Loc.Get("LogGuiDatNotFound"));
                return;
            }
            byte[]? iniData;
            if (!backupManager.BackupFiles.TryGetValue("SYSTEM/CLMK/icon.ini", out iniData)) {
                Log(Loc.Get("LogIconIniNotFound"));
                return;
            }

            foreach (var img in unitIcons.Values) {
                if (img != null) {
                    img.Dispose();
                }
            }
            unitIcons.Clear();
            try {
                byte[] decompIni = GameLZSS.DecompressPfil(iniData!);
                // Against Rome data files are stored as Windows-1251, not UTF-8.
                string iniText = Encoding.GetEncoding(1251).GetString(decompIni);
                string[] lines = iniText.Split(new string[] { "\r\n", "\n" }, System.StringSplitOptions.RemoveEmptyEntries);
                Dictionary<string, string> unitToTga = new Dictionary<string, string>();
                foreach (string line in lines) {
                    if (line.StartsWith("Fig") && line.Contains(",")) {
                        string[] parts = ParseCsvLine(line);
                        if (parts.Length >= 2) {
                            string key = parts[0].Trim();
                            string tgaName = parts[1].Trim();
                            unitToTga[key] = tgaName;
                        }
                    }
                }
                using (var archive = ZipFile.OpenRead(guiDatPath)) {
                    // 非英文版遊戲的圖示可能放在 IGM0806/ 下其它語系資料夾（不一定是 US）。
                    // 先建「檔名 → 壓縮項」索引，US 優先，找不到 US 時退回任一語系的同名檔。
                    var iconEntries = new Dictionary<string, ZipArchiveEntry>(StringComparer.OrdinalIgnoreCase);
                    foreach (var archiveEntry in archive.Entries) {
                        if (string.IsNullOrEmpty(archiveEntry.Name)) continue;
                        string fullName = archiveEntry.FullName.Replace('\\', '/');
                        if (!fullName.StartsWith("SYSTEM/CLMK/DLG/IGM0806/", StringComparison.OrdinalIgnoreCase)) continue;
                        bool isUsFolder = fullName.StartsWith("SYSTEM/CLMK/DLG/IGM0806/US/", StringComparison.OrdinalIgnoreCase);
                        if (isUsFolder || !iconEntries.ContainsKey(archiveEntry.Name)) {
                            iconEntries[archiveEntry.Name] = archiveEntry;
                        }
                    }

                    foreach (var kvp in unitToTga) {
                        if (iconEntries.TryGetValue(kvp.Value, out ZipArchiveEntry? entry)) {
                            using (var stream = entry.Open()) {
                                using (var ms = new MemoryStream()) {
                                    stream.CopyTo(ms);
                                    byte[] buffer = ms.ToArray();
                                    Bitmap? bmp = LoadTga(buffer);
                                    if (bmp != null) {
                                        unitIcons[kvp.Key] = bmp;
                                    }
                                }
                            }
                        }
                    }
                }
                Log(string.Format("成功載入 {0} 個兵種圖示。", unitIcons.Count));
            } catch (Exception ex) {
                Log(Loc.Get("LogLoadIconFailed") + ex.Message + "\r\n" + ex.StackTrace);
            }
        }

        /// <summary>
        /// 獲取各兵種的平衡基礎屬性，若未啟用平衡模式，則直接返回原版屬性。
        /// </summary>
        private static double[] MergeUnitStatsLayers(double[] fallback, double[] custom, bool supportsSpellRadius,
            bool ignoreMovementSpeed = false, bool ignoreRange = false, bool ignoreSpellRadius = false,
            bool removePriestSight = false) {
            ArgumentNullException.ThrowIfNull(fallback);
            ArgumentNullException.ThrowIfNull(custom);

            double[] layered = new double[9];
            for (int i = 0; i < layered.Length; i++) {
                if (i == 8 && !supportsSpellRadius) {
                    layered[i] = 0;
                    continue;
                }

                // 速度、射程、法術範圍不再是自訂兵種欄位，由獨立功能負責。
                if (i is 4 or 7 or 8 || (i == 5 && removePriestSight)) {
                    layered[i] = fallback.Length > i ? fallback[i] : 0;
                    continue;
                }

                // Preset values are concrete overrides. Only fields omitted by an
                // older/short preset inherit the active balanced or original layer.
                layered[i] = custom.Length > i
                    ? custom[i]
                    : (fallback.Length > i ? fallback[i] : 0);
            }
            return layered;
        }

        private double[] GetBaseStatsForUnit(string key, double origHp, double origDmg, double origVw, double origAw, bool forceBalance = false) {
            double[] original = GetOriginalStats(key);
            bool balanceEnabled = forceBalance || chkBalance.Checked;
            double[] balanced = balanceEnabled ? GetDefaultBalancedStats(key) : original;

            double[] result;
            if (!TroopConfig.UnitMeta.TryGetValue(key, out var meta)) return balanced;
            // 與 BackupManager.GetBaseStatsForUnit 一致：自訂屬性同受平衡開關把關。
            if (balanceEnabled && customUnitStats != null && customUnitStats.TryGetValue(key, out double[]? custom) && custom != null) {
                bool ignoreRange = (chkRangedRange3x.Checked && (meta.UnitType is "ranged_inf" or "ranged_cav" or "siege")) ||
                    (chkSpellEntireMap.Checked && meta.UnitType == "priest");
                result = MergeUnitStatsLayers(balanced, custom, SupportsConfigurableSpellRadius(key),
                    chkUnitMovementSpeed2x.Checked, ignoreRange,
                    chkSpellRange3x.Checked && SupportsConfigurableSpellRadius(key),
                    meta.UnitType == "priest");
            } else {
                result = (double[])balanced.Clone();
            }

            if (meta != null) {
                bool isRanged = meta.UnitType is "ranged_inf" or "ranged_cav" or "siege";
                if (isRanged && chkRangedRange3x.Checked) {
                    result[7] *= 3.0;
                }
                if (chkUnitMovementSpeed2x.Checked) {
                    result[4] *= 2.0;
                }
                bool isPriest = meta.UnitType == "priest";
                if (isPriest) {
                    if (chkSpellEntireMap.Checked) {
                        result[7] = 30000.0;
                    }
                    if (chkSpellRange3x.Checked && SupportsConfigurableSpellRadius(key)) result[8] *= 3.0;
                }
            }

            return result;
        }

        /// <summary>
        /// 將裝備分類代碼轉換為易懂的中文文字說明。
        /// </summary>
        /// <summary>
        /// 建立並設定用於顯示當前屬性（原版對比修改後）的 DataGridView 表格。
        /// </summary>
        private DataGridView CreateCurrentStatsGrid() {
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
                RowTemplate = { Height = 46 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 230, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 40;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(20, 21, 31);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(35, 37, 54);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(24, 25, 35);
            dgv.Columns.Add("Name", "兵種名稱");
            dgv.Columns["Name"].Width = 110;

            var imgColC = new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 40
            };
            dgv.Columns.Add(imgColC);

            dgv.Columns.Add("Type", "部隊類型");
            dgv.Columns["Type"].Visible = false;

            dgv.Columns.Add("Style", "裝備分類");
            dgv.Columns["Style"].Visible = false;

            dgv.Columns.Add("Hp", "生命值對比");
            dgv.Columns["Hp"].Width = 85;

            dgv.Columns.Add("MeleeDmg", "近戰傷害對比");
            dgv.Columns["MeleeDmg"].Width = 85;

            dgv.Columns.Add("RangedDmg", "遠程傷害對比");
            dgv.Columns["RangedDmg"].Width = 85;

            dgv.Columns.Add("MeleeRelt", "近戰冷卻對比");
            dgv.Columns["MeleeRelt"].Width = 85;

            dgv.Columns.Add("RangedRelt", "遠程冷卻對比");
            dgv.Columns["RangedRelt"].Width = 85;

            dgv.Columns.Add("Vw", "防禦對比");
            dgv.Columns["Vw"].Width = 85;

            dgv.Columns.Add("Aw", "戰鬥對比");
            dgv.Columns["Aw"].Width = 85;

            dgv.Columns.Add("Speed", "移動速度對比");
            dgv.Columns["Speed"].Width = 85;

            dgv.Columns.Add("Sight", "視野對比");
            dgv.Columns["Sight"].Width = 85;

            dgv.Columns.Add("Range", "射程對比");
            dgv.Columns["Range"].Width = 85;

            dgv.Columns.Add("SpellRadius", "法術半徑對比");
            dgv.Columns["SpellRadius"].Width = 85;

            dgv.Columns.Add("Tier", "階級");
            dgv.Columns["Tier"].Visible = false;

            dgv.CellFormatting += (s, e) => {
                if (e.Value != null) {
                    string valStr = e.Value.ToString() ?? "";
                    if (valStr.Contains(" -> ")) {
                        string[] parts = valStr.Split(new string[] { " -> " }, StringSplitOptions.None);
                        if (parts.Length == 2) {
                            double origVal, curVal;
                            if (double.TryParse(parts[0], NumberStyles.Any, CultureInfo.InvariantCulture, out origVal) &&
                                double.TryParse(parts[1], NumberStyles.Any, CultureInfo.InvariantCulture, out curVal)) {
                                if (e.CellStyle != null) {
                                    if (curVal > origVal) {
                                        e.CellStyle.ForeColor = Color.FromArgb(0, 255, 128); // 增強：亮綠色
                                        e.CellStyle.SelectionForeColor = Color.FromArgb(0, 255, 128);
                                    } else if (curVal < origVal) {
                                        e.CellStyle.ForeColor = Color.FromArgb(255, 75, 75); // 減弱：亮紅色
                                        e.CellStyle.SelectionForeColor = Color.FromArgb(255, 75, 75);
                                    }
                                }
                            }
                        }
                    }
                }
            };

            ConfigureStatsGridColumnsToFit(dgv);

            return dgv;
        }

        /// <summary>
        /// 建立並設定用於顯示預設屬性（若是平衡模式則為平衡後數值）的 DataGridView 表格。
        /// </summary>
        private DataGridView CreateDefaultStatsGrid() {
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
                RowTemplate = { Height = 46 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 230, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 40;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(20, 21, 31);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(35, 37, 54);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(24, 25, 35);

            dgv.Columns.Add("Name", "兵種名稱");
            dgv.Columns["Name"].Width = 110;

            var imgColD = new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 40
            };
            dgv.Columns.Add(imgColD);

            dgv.Columns.Add("Type", "部隊類型");
            dgv.Columns["Type"].Visible = false;

            dgv.Columns.Add("Style", "裝備分類");
            dgv.Columns["Style"].Visible = false;

            dgv.Columns.Add("Hp", "生命值");
            dgv.Columns["Hp"].Width = 85;

            dgv.Columns.Add("MeleeDmg", "近戰傷害");
            dgv.Columns["MeleeDmg"].Width = 85;

            dgv.Columns.Add("RangedDmg", "遠程傷害");
            dgv.Columns["RangedDmg"].Width = 85;

            dgv.Columns.Add("MeleeRelt", "近戰冷卻");
            dgv.Columns["MeleeRelt"].Width = 85;

            dgv.Columns.Add("RangedRelt", "遠程冷卻");
            dgv.Columns["RangedRelt"].Width = 90;

            dgv.Columns.Add("Vw", "防禦力");
            dgv.Columns["Vw"].Width = 80;

            dgv.Columns.Add("Aw", "戰鬥力");
            dgv.Columns["Aw"].Width = 80;

            dgv.Columns.Add("Speed", "移動速度");
            dgv.Columns["Speed"].Width = 80;

            dgv.Columns.Add("Sight", "視野");
            dgv.Columns["Sight"].Width = 80;

            dgv.Columns.Add("Range", "射程/技能距離");
            dgv.Columns["Range"].Width = 100;

            dgv.Columns.Add("SpellRadius", "法術半徑");
            dgv.Columns["SpellRadius"].Width = 80;

            dgv.Columns.Add("Tier", "階級");
            dgv.Columns["Tier"].Width = 75;
            dgv.Columns["Tier"].DisplayIndex = 4;

            ConfigureStatsGridColumnsToFit(dgv);

            return dgv;
        }

        private static void ConfigureStatsGridColumnsToFit(DataGridView dgv) {
            foreach (DataGridViewColumn column in dgv.Columns) {
                if (!column.Visible || column.Name == "Name" || column.Name == "Icon" || column.Name == "Tier") {
                    continue;
                }

                column.FillWeight = Math.Max(60, column.Width);
                column.MinimumWidth = 60;
                column.AutoSizeMode = DataGridViewAutoSizeColumnMode.Fill;
            }
        }

        /// <summary>
        /// 初始化兵種數據，載入圖示並從備份載入預設的屬性數值。
        /// </summary>
        private void InitializeData() {
            LoadIcons();

            string gamePath = GetGamePath();
            if (!string.IsNullOrEmpty(gamePath) && Directory.Exists(gamePath)) {
                LoadDefaultStatsData();
            } else {
                Log("請先在右上角設定遊戲路徑。");
            }
        }

        /// <summary>
        /// 從內嵌的 objdef.dau 備份資源解壓並解析，取得並顯示原版或平衡後的兵種屬性設定到預設屬性表格中。
        /// </summary>
        private void LoadDefaultStatsData() {
            var unitRows = backupManager.GetBackupUnitRows();
            if (unitRows == null || unitRows.Count == 0) {
                Log(Loc.Get("LogObjdefNotFound"));
                return;
            }
            try {
                foreach (var dgv in defaultStatsGrids.Values) {
                     dgv.Rows.Clear();
                }



                foreach (string key in TroopConfig.UnitOrder) {
                    if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                    var meta = TroopConfig.UnitMeta[key];
                    string faction = meta.Faction;
                    string tier = meta.Tier;
                    string utype = meta.UnitType;
                    string style = meta.Style;

                    if (!unitRows.ContainsKey(key)) continue;
                    string[] cols = unitRows[key];

                    double hp = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out hp);

                    double vw = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out vw);

                    double aw = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out aw);

                    double meleeDam = 0;
                    double rangedDam = 0;
                    GetMeleeAndRangedDmg(cols, utype, out meleeDam, out rangedDam);

                    double meleeRelt = 0;
                    double rangedRelt = 0;
                    GetMeleeAndRangedRelt(cols, utype, out meleeRelt, out rangedRelt);

                    double origMoves = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);

                    double origSight = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origSight);

                    double origRange = GetUnitMaxRange(cols, utype);

                    double defaultSpeed = 0;
                    if (origMoves > 0) {
                        defaultSpeed = Math.Round(origMoves * 2.0, 1);
                    }

                    double defaultSight = 0;
                    if (origSight > 0) {
                        if (utype == "priest") {
                            defaultSight = Math.Round(origSight * 30.0);
                        } else if (utype == "ranged_inf" || utype == "ranged_cav" || utype == "hybrid_inf") {
                            defaultSight = Math.Round(origSight * 3.0);
                        } else if (utype == "siege") {
                            defaultSight = Math.Round(origSight * 3.0);
                        } else {
                            defaultSight = origSight;
                        }
                    }

                    double defaultRange = 0;
                    if (origRange > 0) {
                        if (utype == "priest") {
                            defaultRange = Math.Round(origRange * 30.0);
                        } else if (utype == "ranged_inf" || utype == "ranged_cav" || utype == "hybrid_inf") {
                            defaultRange = Math.Round(origRange * 3.0);
                        } else if (utype == "siege") {
                            defaultRange = Math.Round(origRange * 3.0);
                        } else {
                            defaultRange = origRange;
                        }
                    }

                    double defaultSpellRadius = 0;
                    if (utype == "priest") {
                        defaultSpellRadius = 500 * 2.5;
                    }

                    double origPrimaryDam = 1.0;
                    if (utype == "ranged_inf" || utype == "ranged_cav") {
                        origPrimaryDam = rangedDam;
                    } else if (utype == "siege") {
                        origPrimaryDam = Math.Max(meleeDam, rangedDam);
                    } else {
                        origPrimaryDam = meleeDam;
                    }

                    double defHpMult = 1.0;


                    string displayName = Loc.GetUnitName(key);
                    string typeText = Loc.GetUnitType(utype);
                    string styleText = Loc.GetStyleText(style);

                    var iconImage = unitIcons.ContainsKey(key) ? unitIcons[key] : null;
                    double[] bases = GetBaseStatsForUnit(key, hp, origPrimaryDam, vw, aw, chkBalance.Checked);

                    double displayMeleeDam = 0;
                    double displayRangedDam = 0;

                    double scale = 1.0;
                    if (origPrimaryDam > 0) {
                        scale = bases[1] / origPrimaryDam;
                    }

                    if (utype == "ranged_inf" || utype == "ranged_cav") {
                        displayMeleeDam = meleeDam;
                        displayRangedDam = bases[1];
                    } else if (utype == "hybrid_inf") {
                        displayMeleeDam = meleeDam * scale;
                        displayRangedDam = rangedDam * scale;
                    } else if (utype == "siege") {
                        displayMeleeDam = 0;
                        displayRangedDam = bases[1];
                    } else {
                        displayMeleeDam = bases[1];
                        displayRangedDam = 0;
                    }

                    double origPrimaryRelt = meleeRelt;
                    if (utype == "ranged_inf" || utype == "ranged_cav") {
                        origPrimaryRelt = rangedRelt;
                    } else if (utype == "siege") {
                        origPrimaryRelt = Math.Max(meleeRelt, rangedRelt);
                    }

                    double reltScale = 1.0;
                    if (origPrimaryRelt > 0) {
                        reltScale = bases[6] / origPrimaryRelt;
                    }
                    double displayMeleeRelt = meleeRelt > 0
                        ? Math.Round(meleeRelt * reltScale)
                        : 0;
                    double displayRangedRelt = rangedRelt > 0
                        ? Math.Round(rangedRelt * reltScale)
                        : 0;

                    double finalDefVw = bases[2];
                    double finalDefAw = bases[3];
                    defaultSpeed = bases[4];
                    defaultSight = bases[5];
                    defaultRange = bases[7];
                    defaultSpellRadius = bases[8];

                    string meleeReltText = FormatVal(displayMeleeRelt, "F0");
                    string rangedReltText = FormatVal(displayRangedRelt, "F0");
                    string meleeDmgText = FormatVal(displayMeleeDam, "F1");
                    string rangedDmgText = FormatVal(displayRangedDam, "F1");

                    string tierText = Loc.GetTierText(tier);

                    var dgvTarget = defaultStatsGrids[faction];
                    dgvTarget.Rows.Add(
                        displayName, iconImage, typeText, styleText,
                        Math.Round(bases[0] * defHpMult, 1),
                        meleeDmgText,
                        rangedDmgText,
                        meleeReltText,
                        rangedReltText,
                        Math.Round(finalDefVw, 1),
                        Math.Round(finalDefAw, 1),
                        FormatVal(defaultSpeed, "F1"),
                        FormatVal(defaultSight, "F0"),
                        FormatVal(defaultRange, "F0"),
                        FormatVal(defaultSpellRadius, "F0"),
                        tierText
                    );
                }
                int totalRows = 0;
                foreach (var dgv in defaultStatsGrids.Values) totalRows += dgv.Rows.Count;
                Log(string.Format(Loc.Get("LogDefaultStatsLoaded"), totalRows));
            } catch (Exception ex) {
                Log(Loc.Get("LogDefaultStatsLoadError") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("LogDefaultStatsLoadError") + ex.Message + "\n" + ex.StackTrace, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「讀取現有設定」按鈕時觸發，加載遊戲目錄下的當前屬性設定。
        /// </summary>
        private void BtnLoadCurrent_Click(object? sender, EventArgs e) {
            LoadCurrentData();
        }

        /// <summary>
        /// 從遊戲目錄下的實體檔案（objdef.dau, cl_script.ini, ress.ini, Against_Rome.exe）讀取目前的設定值並顯示在介面上。
        /// </summary>
        private void LoadCurrentData(bool syncUIWithFile = true) {
            try {
                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath) || !File.Exists(Path.Combine(gamePath, "Against_Rome.exe"))) {
                    Log(Loc.Get("MsgWrongGameDir"));
                    return;
                }

                // 呼叫解耦的 patchEngine 進行全方位修改狀態偵測
                PatchProfile profile = patchEngine.DetectCurrentPatchProfile(gamePath, backupManager);

                if (syncUIWithFile) {
                    chkBalance.CheckedChanged -= ChkBalance_CheckedChanged;
                    chkRangedRange3x.CheckedChanged -= ChkRangedRange3x_CheckedChanged;
                    chkUnitMovementSpeed2x.CheckedChanged -= ChkUnitMovementSpeed2x_CheckedChanged;
                    chkSpellEntireMap.CheckedChanged -= ChkSpellEntireMap_CheckedChanged;
                    chkSpellRange3x.CheckedChanged -= ChkSpellRange3x_CheckedChanged;

                    foreach (var (id, toggle) in featureToggles) toggle.Checked = profile.GetBool(id);

                    chkBalance.CheckedChanged += ChkBalance_CheckedChanged;
                    chkRangedRange3x.CheckedChanged += ChkRangedRange3x_CheckedChanged;
                    chkUnitMovementSpeed2x.CheckedChanged += ChkUnitMovementSpeed2x_CheckedChanged;
                    chkSpellEntireMap.CheckedChanged += ChkSpellEntireMap_CheckedChanged;
                    chkSpellRange3x.CheckedChanged += ChkSpellRange3x_CheckedChanged;
                    chkGameSpeed.Checked = profile.GameSpeed > 1;

                    LoadDefaultStatsData();
                }

                double spellRadMultVal = 1.0;
                string initClPath = Path.Combine(gamePath, @"SYSTEM\cl_script.ini");
                if (File.Exists(initClPath)) {
                    try {
                        byte[] clBytes = File.ReadAllBytes(initClPath);
                        byte[] decompCl = GameLZSS.DecompressPfil(clBytes);
                        string clText = Encoding.GetEncoding(1251).GetString(decompCl);
                        var matchSpell = RegexSpellLoad.Match(clText);
                        if (matchSpell.Success) {
                            double r;
                            if (double.TryParse(matchSpell.Groups[1].Value, NumberStyles.Any, CultureInfo.InvariantCulture, out r) && r > 0) {
                                spellRadMultVal = r / 500.0;
                            }
                        }
                    } catch (Exception ex) { Log("讀取 cl_script.ini 法術半徑失敗，將以原版半徑顯示: " + ex.Message); }
                }
                string src = Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau");
                byte[]? dauBytes;
                if (File.Exists(src)) {
                    dauBytes = File.ReadAllBytes(src);
                } else {
                    if (!backupManager.HasFile("SYSTEM/DATA_MP/DEFAULTS/objdef.dau")) {
                        Log(Loc.Get("LogNoObjdefForRead"));
                        return;
                    }
                    dauBytes = backupManager.GetBackupBytes("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
                }
                Log(Loc.Get("LogReadCurrent"));
                byte[] decompBytes = GameLZSS.DecompressPfil(dauBytes!);
                string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
                string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
                string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);

                Dictionary<string, string[]> unitRows = new Dictionary<string, string[]>();
                for (int idx = 2; idx < lines.Length; idx++) {
                    string line = lines[idx];
                    if (line.Length < 100) continue;
                    string[] cols = ParseCsvLine(line);
                    if (cols.Length < 192) continue;
                    string name = cols[(int)ObjdefIndex.Name].Trim();
                    if (TroopConfig.UnitMeta.ContainsKey(name) || name == "FigZivMan00_Zivilist") {
                        unitRows[name] = cols;
                    }
                }

                var origUnitRows = backupManager.GetBackupUnitRows();

                foreach (var dgv in currentStatsGrids.Values) {
                    dgv.Rows.Clear();
                }

                foreach (string key in TroopConfig.UnitOrder) {
                    if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                    if (!unitRows.ContainsKey(key) || !origUnitRows.ContainsKey(key)) continue;

                    string[] cols = unitRows[key];
                    string[] origCols = origUnitRows[key];
                    string faction = TroopConfig.UnitMeta[key].Faction;
                    string utype = TroopConfig.UnitMeta[key].UnitType;
                    string style = TroopConfig.UnitMeta[key].Style;

                    double curHp;
                    double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curHp);
                    double curVw;
                    double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curVw);
                    double curAw;
                    double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curAw);

                    double origHp;
                    double.TryParse(origCols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origHp);
                    double origVw;
                    double.TryParse(origCols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origVw);
                    double origAw;
                    double.TryParse(origCols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origAw);

                    double origMeleeDmg = 0;
                    double origRangedDmg = 0;
                    GetMeleeAndRangedDmg(origCols, utype, out origMeleeDmg, out origRangedDmg);

                    double curMeleeDmg = 0;
                    double curRangedDmg = 0;
                    GetMeleeAndRangedDmg(cols, utype, out curMeleeDmg, out curRangedDmg);

                    double tempOrigMeleeRelt = 0;
                    double tempOrigRangedRelt = 0;
                    GetMeleeAndRangedRelt(origCols, utype, out tempOrigMeleeRelt, out tempOrigRangedRelt);

                    double tempCurMeleeRelt = 0;
                    double tempCurRangedRelt = 0;
                    GetMeleeAndRangedRelt(cols, utype, out tempCurMeleeRelt, out tempCurRangedRelt);

                    double origMoves = 0;
                    double.TryParse(origCols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);
                    double curMoves = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curMoves);

                    double origSight = 0;
                    double.TryParse(origCols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origSight);
                    double curSight = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curSight);

                    double origRange = GetUnitMaxRange(origCols, utype);
                    double curRange = GetUnitMaxRange(cols, utype);

                    double origSpellRadius = 0;
                    double curSpellRadius = 0;
                    // 只有 KEL/HUN 祭司的法術半徑是可設定項；GER 祭司顯示變動值會造成假象。
                    if (utype == "priest" && SupportsConfigurableSpellRadius(key)) {
                        origSpellRadius = 500;
                        curSpellRadius = 500 * spellRadMultVal;
                    }

                    string displayName = Loc.GetUnitName(key);
                    string typeText = Loc.GetUnitType(utype);
                    string styleText = Loc.GetStyleText(style);

                    string tier = TroopConfig.UnitMeta[key].Tier;
                    string tierText = Loc.GetTierText(tier);

                    var iconImage = unitIcons.ContainsKey(key) ? unitIcons[key] : null;
                    var dgvCurrent = currentStatsGrids[faction];
                    dgvCurrent.Rows.Add(
                        displayName,
                        iconImage,
                        typeText,
                        styleText,
                        FormatValueCompare(origHp, curHp),
                        FormatValueCompare(origMeleeDmg, curMeleeDmg),
                        FormatValueCompare(origRangedDmg, curRangedDmg),
                        FormatValueCompare(tempOrigMeleeRelt, tempCurMeleeRelt),
                        FormatValueCompare(tempOrigRangedRelt, tempCurRangedRelt),
                        FormatValueCompare(origVw, curVw),
                        FormatValueCompare(origAw, curAw),
                        FormatValueCompare(origMoves, curMoves),
                        FormatValueCompare(origSight, curSight),
                        FormatValueCompare(origRange, curRange),
                        FormatValueCompare(origSpellRadius, curSpellRadius),
                        tierText
                    );
                }


                int totalCurrentRows = 0;
                foreach (var dgv in currentStatsGrids.Values) totalCurrentRows += dgv.Rows.Count;
                Log(string.Format(Loc.Get("LogReadCurrentDone"), totalCurrentRows));
            } catch (Exception ex) {
                Log(Loc.Get("LogPresetImportError") + ex.Message + "\r\n" + ex.StackTrace);
            }
        }

        /// <summary>
        /// 從兵種 CSV 行中解析出近戰傷害與遠程傷害。
        /// 遍歷 8 個武器槽位，若啟用且武器類型匹配則回傳最高傷害。
        /// </summary>
        private static void GetMeleeAndRangedDmg(string[] cols, string utype, out double meleeDmg, out double rangedDmg) {
            meleeDmg = 0;
            rangedDmg = 0;
            for (int w = 1; w <= 8; w++) {
                int wAktiIdx = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int wDamIdx = wAktiIdx + 1;
                int wDtypIdx = (int)ObjdefIndex.Weapon1Dtyp + (w - 1);
                if (wAktiIdx >= cols.Length || wDamIdx >= cols.Length || wDtypIdx >= cols.Length) {
                    continue;
                }
                if (cols[wAktiIdx].Trim() == "1") {
                    double damVal;
                    double.TryParse(cols[wDamIdx].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out damVal);
                    string wDtyp = cols[wDtypIdx].Trim();
                    bool isRangedWeapon = (wDtyp == "1" || wDtyp == "2" || wDtyp == "3" || wDtyp == "4" || utype == "siege");
                    if (isRangedWeapon) {
                        if (damVal > rangedDmg) rangedDmg = damVal;
                    } else {
                        if (damVal > meleeDmg) meleeDmg = damVal;
                    }
                }
            }
        }

        /// <summary>
        /// 從兵種 CSV 行中解析出近戰武器與遠程武器的最小冷卻時間（攻擊間隔時間）。
        /// </summary>
        private static void GetMeleeAndRangedRelt(string[] cols, string utype, out double meleeRelt, out double rangedRelt) {
            meleeRelt = 0;
            rangedRelt = 0;
            for (int w = 1; w <= 8; w++) {
                int wAktiIdx = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int wReltIdx = wAktiIdx + 6;
                int wDtypIdx = (int)ObjdefIndex.Weapon1Dtyp + (w - 1);
                if (wAktiIdx >= cols.Length || wReltIdx >= cols.Length || wDtypIdx >= cols.Length) {
                    continue;
                }
                if (cols[wAktiIdx].Trim() == "1") {
                    double reltVal;
                    double.TryParse(cols[wReltIdx].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out reltVal);
                    string wDtyp = cols[wDtypIdx].Trim();
                    bool isRangedWeapon = (wDtyp == "1" || wDtyp == "2" || wDtyp == "3" || wDtyp == "4" || utype == "siege");
                    if (isRangedWeapon) {
                        if (reltVal > 0 && (rangedRelt == 0 || reltVal < rangedRelt)) {
                            rangedRelt = reltVal;
                        }
                    } else {
                        if (reltVal > 0 && (meleeRelt == 0 || reltVal < meleeRelt)) {
                            meleeRelt = reltVal;
                        }
                    }
                }
            }
        }


        /// <summary>
        /// 從兵種 CSV 行中解析所有啟用武器槽的最大射程。
        /// </summary>
        private static double GetUnitMaxRange(string[] cols, string utype) {
            double maxR = 0;
            if (utype == "priest")
                return double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double sight) ? sight : 0;
            for (int w = 1; w <= 8; w++) {
                int activeIndex = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int rangeMinIndex = (int)ObjdefIndex.Weapon1RangeMin + (w - 1) * 8;
                int rangeMaxIndex = (int)ObjdefIndex.Weapon1RangeMax + (w - 1) * 8;
                if (rangeMaxIndex >= cols.Length || cols[activeIndex].Trim() != "1") continue;

                if (double.TryParse(cols[rangeMinIndex].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double minRange)) {
                    maxR = Math.Max(maxR, minRange);
                }
                if (double.TryParse(cols[rangeMaxIndex].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double maxRange)) {
                    maxR = Math.Max(maxR, maxRange);
                }
            }
            return maxR;
        }

        /// <summary>
        /// 格式化數值為字串，若數值小於等於 0 則顯示為 "-"。
        /// </summary>
        private static string FormatVal(double val, string format = "0.##") {
            if (val <= 0) return "-";
            return val.ToString(format, CultureInfo.InvariantCulture);
        }

        /// <summary>
        /// 格式化原版與修改後數值的對比字串（例如 "100 -> 150"），若兩者相同則僅顯示單一數值。
        /// </summary>
        private static string FormatValueCompare(double origVal, double curVal) {
            if (origVal <= 0 && curVal <= 0) return "-";
            string origStr = origVal.ToString("0.##", CultureInfo.InvariantCulture);
            string curStr = curVal.ToString("0.##", CultureInfo.InvariantCulture);
            if (Math.Abs(origVal - curVal) < 0.01) {
                return origStr;
            }
            return string.Format(CultureInfo.InvariantCulture, "{0} -> {1}", origStr, curStr);
        }


        /// <summary>
        /// 當「平衡模式」勾選狀態改變時，更新預設與現有的數據表格顯示。
        /// </summary>
        private void ChkBalance_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkBalance.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogBalanceToggled"), status));
        }

        private void ChkRangedRange3x_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkRangedRange3x.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogRangedRange3xToggled"), status));
        }
        private void ChkUnitMovementSpeed2x_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkUnitMovementSpeed2x.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogUnitMovementSpeed2xToggled"), status));
        }

        private void ChkSpellEntireMap_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkSpellEntireMap.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogSpellEntireMapToggled"), status));
        }

        private void ChkSpellRange3x_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkSpellRange3x.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogSpellRange3xToggled"), status));
        }
    }
}
