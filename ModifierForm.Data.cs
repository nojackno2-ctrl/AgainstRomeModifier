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
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private static readonly Regex RegexSpellLoad = new Regex(@"Radius\s*=\s*(?:HUN|KEL|GER)\s*,\s*Spell\d+\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexCiviLoad = new Regex(@"CiviDelay\s*=\s*([A-Z]{3})\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleLoad = new Regex(@"MoralsDecLostMem\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);

        /// <summary>
        /// 從應用程式資源載入內嵌的 Backup.zip 至記憶體中。
        /// </summary>
        private void LoadBackupZipToMemory() {
            string? resourceName = typeof(Program).Assembly
                .GetManifestResourceNames()
                .FirstOrDefault(n => n.EndsWith("Backup.zip"));

            if (resourceName == null) {
                Log("找不到內嵌的 Backup.zip 資源。");
                return;
            }

            using Stream stream = typeof(Program).Assembly.GetManifestResourceStream(resourceName)!;
            LoadZipToDictionary(stream);
        }

        /// <summary>
        /// 獲取 UI 文字框中設定的遊戲路徑。
        /// </summary>
        private string GetGamePath() {
            return txtGamePath != null ? txtGamePath.Text.Trim() : "";
        }

        /// <summary>
        /// 解析單行 CSV 資料（逗號分隔）。
        /// </summary>
        private static string[] ParseCsvLine(string line) {
            if (line == null) return Array.Empty<string>();
            return line.Split(',');
        }

        /// <summary>
        /// 將字串陣列重新組合成 CSV 逗號分隔的字串。
        /// </summary>
        private static string ToCsvString(string[] cols) {
            if (cols == null) return "";
            return string.Join(",", cols);
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
            } catch { }
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
        /// 將備份 Zip 壓縮檔的串流解壓縮，並以相對路徑作為 Key，其 byte 陣列作為 Value 載入記憶體字典中。
        /// </summary>
        private void LoadZipToDictionary(Stream stream) {
            using (ZipArchive archive = new ZipArchive(stream)) {
                foreach (ZipArchiveEntry entry in archive.Entries) {
                    if (entry.Name == "") continue;
                    using (Stream entryStream = entry.Open()) {
                        using (MemoryStream ms = new MemoryStream()) {
                            entryStream.CopyTo(ms);
                            string key = entry.FullName.Replace('\\', '/');
                            backupFiles[key] = ms.ToArray();
                        }
                    }
                }
            }
        }

        /// <summary>
        /// 從遊戲目錄下的 gui.dat 以及內嵌的 icon.ini 中載入所有兵種對應的 TGA 圖示，
        /// 並轉換成 Bitmap 快取至記憶體中。
        /// </summary>
        private void LoadIcons() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                Log("遊戲路徑未設定或不存在，無法載入兵種圖示。");
                return;
            }
            string guiDatPath = Path.Combine(gamePath, "gui.dat");
            if (!File.Exists(guiDatPath)) {
                Log("找不到 gui.dat，無法載入兵種圖示。");
                return;
            }
            byte[]? iniData;
            if (!backupFiles.TryGetValue("SYSTEM/CLMK/icon.ini", out iniData)) {
                Log("記憶體備份中找不到 icon.ini，無法載入兵種圖示。");
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
                    foreach (var kvp in unitToTga) {
                        string entryName = "SYSTEM/CLMK/DLG/IGM0806/US/" + kvp.Value;
                        var entry = archive.GetEntry(entryName);
                        if (entry == null) {
                            entry = archive.GetEntry(entryName.Replace('/', '\\'));
                        }
                        if (entry != null) {
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
                Log("載入圖示失敗: " + ex.Message + "\r\n" + ex.StackTrace);
            }
        }

        /// <summary>
        /// 獲取各兵種的平衡基礎屬性，若未啟用平衡模式，則直接返回原版屬性。
        /// </summary>
        private double[] GetBaseStatsForUnit(string key, double origHp, double origDmg, double origVw, double origAw, bool forceBalance = false) {
            if (customUnitStats != null && customUnitStats.ContainsKey(key)) {
                double[] custom = customUnitStats[key];
                if (custom.Length >= 9) {
                    return custom;
                }
                
                double[] fullStats = new double[9];
                for (int i = 0; i < Math.Min(custom.Length, 4); i++) {
                    fullStats[i] = custom[i];
                }
                
                double[] fallback = (forceBalance || chkBalance.Checked) ? GetDefaultBalancedStats(key) : GetOriginalStats(key);
                for (int i = 4; i < 9; i++) {
                    fullStats[i] = fallback[i];
                }
                return fullStats;
            }
            if (!forceBalance && !chkBalance.Checked) {
                return GetOriginalStats(key);
            }
            return GetDefaultBalancedStats(key);
        }

        /// <summary>
        /// 將裝備分類代碼轉換為易懂的中文文字說明。
        /// </summary>
        private string GetStyleText(string style) {
            if (style == "shield") return "持盾";
            if (style == "two_handed") return "雙手武器";
            if (style == "dual_wield") return "雙持武器";
            if (style == "ranged") return "遠程";
            return "無";
        }

        /// <summary>
        /// 建立並設定用於顯示當前屬性（原版對比修改後）的 DataGridView 表格。
        /// </summary>
        private DataGridView CreateCurrentStatsGrid() {
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
                RowTemplate = { Height = 46 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 220, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 40;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(24, 24, 30);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(45, 45, 60);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(28, 28, 35);
            dgv.Columns.Add("Name", "兵種名稱");
            dgv.Columns["Name"].Width = 130;

            var imgColC = new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 55
            };
            dgv.Columns.Add(imgColC);

            dgv.Columns.Add("Type", "部隊類型");
            dgv.Columns["Type"].Width = 90;

            dgv.Columns.Add("Style", "裝備分類");
            dgv.Columns["Style"].Width = 90;

            dgv.Columns.Add("Hp", "生命值對比");
            dgv.Columns["Hp"].Width = 105;

            dgv.Columns.Add("MeleeDmg", "近戰傷害對比");
            dgv.Columns["MeleeDmg"].Width = 105;

            dgv.Columns.Add("RangedDmg", "遠程傷害對比");
            dgv.Columns["RangedDmg"].Width = 105;

            dgv.Columns.Add("MeleeRelt", "近戰冷卻對比");
            dgv.Columns["MeleeRelt"].Width = 105;

            dgv.Columns.Add("RangedRelt", "遠程冷卻對比");
            dgv.Columns["RangedRelt"].Width = 105;

            dgv.Columns.Add("Vw", "防禦對比");
            dgv.Columns["Vw"].Width = 105;

            dgv.Columns.Add("Aw", "戰鬥對比");
            dgv.Columns["Aw"].Width = 105;

            dgv.Columns.Add("Speed", "移動速度對比");
            dgv.Columns["Speed"].Width = 105;

            dgv.Columns.Add("Sight", "視野對比");
            dgv.Columns["Sight"].Width = 105;

            dgv.Columns.Add("Range", "射程對比");
            dgv.Columns["Range"].Width = 105;

            dgv.Columns.Add("SpellRadius", "法術半徑對比");
            dgv.Columns["SpellRadius"].Width = 105;

            dgv.Columns.Add("Tier", "階級");
            dgv.Columns["Tier"].Width = 75;
            dgv.Columns["Tier"].DisplayIndex = 4;

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
                BackgroundColor = Color.FromArgb(20, 20, 25),
                ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(45, 45, 55),
                BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false,
                RowTemplate = { Height = 46 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 220, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 40;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(24, 24, 30);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(45, 45, 60);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(28, 28, 35);

            dgv.Columns.Add("Name", "兵種名稱");
            dgv.Columns["Name"].Width = 135;

            var imgColD = new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 55
            };
            dgv.Columns.Add(imgColD);

            dgv.Columns.Add("Type", "部隊類型");
            dgv.Columns["Type"].Width = 95;

            dgv.Columns.Add("Style", "裝備分類");
            dgv.Columns["Style"].Width = 95;

            dgv.Columns.Add("Hp", "生命值");
            dgv.Columns["Hp"].Width = 80;

            dgv.Columns.Add("MeleeDmg", "近戰傷害");
            dgv.Columns["MeleeDmg"].Width = 90;

            dgv.Columns.Add("RangedDmg", "遠程傷害");
            dgv.Columns["RangedDmg"].Width = 90;

            dgv.Columns.Add("MeleeRelt", "近戰冷卻");
            dgv.Columns["MeleeRelt"].Width = 90;

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

            return dgv;
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
            EnsureBackupUnitRowsParsed();
            if (!_backupUnitRowsParsed || _backupUnitRows.Count == 0) {
                Log("記憶體備份中找不到 objdef.dau，無法載入預設兵種屬性。");
                return;
            }
            try {
                foreach (var dgv in defaultStatsGrids.Values) {
                     dgv.Rows.Clear();
                }

                var unitRows = _backupUnitRows;

                foreach (string key in TroopConfig.UnitOrder) {
                    if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                    var meta = TroopConfig.UnitMeta[key];
                    string faction = meta.Item1;
                    string tier = meta.Item2;
                    string utype = meta.Item3;
                    string style = meta.Item4;

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
                    double[] bases = GetBaseStatsForUnit(key, hp, origPrimaryDam, vw, aw, true);

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

                    double displayMeleeRelt = meleeRelt;
                    if (style == "dual_wield" && displayMeleeRelt > 0) {
                        displayMeleeRelt = Math.Round(displayMeleeRelt / 1.5);
                    }
                    double displayRangedRelt = rangedRelt;
                    if (utype == "ranged_inf" || utype == "ranged_cav" || utype == "hybrid_inf") {
                        if (displayRangedRelt > 0) {
                            displayRangedRelt = Math.Round(displayRangedRelt / 1.5);
                        }
                    }
                    if (style == "dual_wield" && displayRangedRelt > 0) {
                        displayRangedRelt = Math.Round(displayRangedRelt / 1.5);
                    }

                    if (style == "two_handed") {
                        displayMeleeDam = Math.Round(displayMeleeDam * 1.3, 1);
                        displayRangedDam = Math.Round(displayRangedDam * 1.3, 1);
                    }

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
                    } catch { }
                }
                string src = Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau");
                byte[]? dauBytes;
                if (File.Exists(src)) {
                    dauBytes = File.ReadAllBytes(src);
                } else {
                    if (!backupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out dauBytes)) {
                        Log(Loc.Get("LogNoObjdefForRead"));
                        return;
                    }
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

                EnsureBackupUnitRowsParsed();
                Dictionary<string, string[]> origUnitRows = _backupUnitRows;

                // 自訂倍率控制項已移除，不進行 UI 賦值。

                // 偵測遊戲當前檔案是否已啟用平衡模式
                bool isFileBalanced = false;
                {
                    int matchCount = 0; // 記錄符合平衡數值的兵種數量
                    int checkedUnits = 0; // 記錄參與比對的兵種總數
                    foreach (string key in TroopConfig.UnitMeta.Keys) {
                        if (!unitRows.ContainsKey(key) || !origUnitRows.ContainsKey(key)) continue;
                        string utype = TroopConfig.UnitMeta[key].Item3;
                        if (utype == "priest" || utype == "siege") continue; // 祭司與攻城武器不參與此比對

                        string[] cols = unitRows[key];
                        string[] origCols = origUnitRows[key];
                        string style = TroopConfig.UnitMeta[key].Item4;
                        double curHp, curVw, curAw;
                        double origHp;
                        
                        // 解析當前與原始的 HP、防禦力(VW)、戰鬥力(AW)
                        if (double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curHp) &&
                            double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curVw) &&
                            double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curAw) &&
                            double.TryParse(origCols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origHp)) {
                            
                            double origVw;
                            double.TryParse(origCols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origVw);
                            double origAw;
                            double.TryParse(origCols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origAw);
                            double origMeleeDmg = 0;
                            double origRangedDmg = 0;
                            GetMeleeAndRangedDmg(origCols, utype, out origMeleeDmg, out origRangedDmg);

                            double origPrimaryDam = (utype == "ranged_inf" || utype == "ranged_cav") ? origRangedDmg : origMeleeDmg;

                            // 獲取平衡後的預期基底數值
                            double[] basesTemp = GetBaseStatsForUnit(key, origHp, origPrimaryDam, origVw, origAw, true);
                            double expectedHp = basesTemp[0];
                            double expectedVw = basesTemp[2];
                            // 若是持盾單位，預期防禦力應加上 1.3 倍的盾牌加成
                            if (style == "shield") {
                                expectedVw = Math.Round(expectedVw * 1.3);
                            }
                            double expectedAw = basesTemp[3];
                            // 若是持盾單位，預期戰鬥力應加上 1.15 倍的盾牌加成
                            if (style == "shield") {
                                expectedAw = Math.Round(expectedAw * 1.15);
                            }

                            // 比對 HP、防禦力與戰鬥力是否均與預期平衡值相符
                            bool hpMatch = Math.Abs(curHp - expectedHp) < 0.1;
                            bool vwMatch = Math.Abs(curVw - expectedVw) < 0.1;
                            bool awMatch = Math.Abs(curAw - expectedAw) < 0.1;

                            if (hpMatch && vwMatch && awMatch) {
                                matchCount++;
                            }
                            checkedUnits++;
                        }
                    }
                    // 當絕大多數兵種 (允許最多3個誤差) 均符合預期平衡值時，才判定為已啟用平衡模式，防止原版狀態誤判
                    if (checkedUnits > 0 && matchCount >= checkedUnits - 3) {
                        isFileBalanced = true;
                    }
                }

                if (syncUIWithFile && chkBalance.Checked != isFileBalanced) {
                    chkBalance.CheckedChanged -= ChkBalance_CheckedChanged;
                    chkBalance.Checked = isFileBalanced;
                    chkBalance.CheckedChanged += ChkBalance_CheckedChanged;
                    LoadDefaultStatsData();
                }

                foreach (var dgv in currentStatsGrids.Values) {
                    dgv.Rows.Clear();
                }

                foreach (string key in TroopConfig.UnitOrder) {
                    if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                    if (!unitRows.ContainsKey(key) || !origUnitRows.ContainsKey(key)) continue;

                    string[] cols = unitRows[key];
                    string[] origCols = origUnitRows[key];
                    string faction = TroopConfig.UnitMeta[key].Item1;
                    string utype = TroopConfig.UnitMeta[key].Item3;
                    string style = TroopConfig.UnitMeta[key].Item4;

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
                    if (utype == "priest") {
                        origSpellRadius = 500;
                        curSpellRadius = 500 * spellRadMultVal;
                    }

                    string displayName = Loc.GetUnitName(key);
                    string typeText = Loc.GetUnitType(utype);
                    string styleText = Loc.GetStyleText(style);

                    string tier = TroopConfig.UnitMeta[key].Item2;
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

                string clPath = Path.Combine(gamePath, @"SYSTEM\cl_script.ini");
                if (File.Exists(clPath)) {
                    byte[] clBytes = File.ReadAllBytes(clPath);
                    byte[] decompCl = GameLZSS.DecompressPfil(clBytes);
                    string clText = Encoding.GetEncoding(1251).GetString(decompCl);
                    
                    var matchCivi = RegexCiviLoad.Match(clText);
                    if (matchCivi.Success) {
                        double delay;
                        if (double.TryParse(matchCivi.Groups[2].Value, NumberStyles.Any, CultureInfo.InvariantCulture, out delay) && delay > 0) {
                            double speedVal = 5000.0 / delay;
                            if (speedVal < 1.0) speedVal = 1.0;
                            if (speedVal > 50.0) speedVal = 50.0;
                            numCiviSpeed.Value = (decimal)speedVal;
                        }
                    }

                    // spellRadMult 已在方法開始時預先載入處理。

                    bool infiniteMorale = false;

                    var matchMorale = RegexMoraleLoad.Match(clText);
                    if (matchMorale.Success) {
                        int val;
                        if (int.TryParse(matchMorale.Groups[1].Value, out val) && val == 0) {
                            infiniteMorale = true;
                        }
                    }
                    chkInfiniteMorale.Checked = infiniteMorale;
                }

                string ressPath = Path.Combine(gamePath, @"SYSTEM\ress.ini");
                if (File.Exists(ressPath)) {
                    byte[] ressBytes = File.ReadAllBytes(ressPath);
                    byte[] decompRess = GameLZSS.DecompressPfil(ressBytes);
                    string ressText = Encoding.GetEncoding(1251).GetString(decompRess);
                    
                    var linesVolk = ressText.Split(new string[] { "\r\n", "\n" }, StringSplitOptions.None);
                    bool inVolkresSection = false;
                    foreach (var line in linesVolk) {
                        string stripped = line.Trim();
                        if (stripped == "[volkres]") {
                            inVolkresSection = true;
                            continue;
                        } else if (stripped.StartsWith("[")) {
                            inVolkresSection = false;
                            continue;
                        }
                        if (inVolkresSection && line.Contains(",")) {
                            string[] cols = ParseCsvLine(line);
                            if (cols.Length >= 10 && cols[9].Trim() == "Rom_Kampf") {
                                int pop;
                                if (int.TryParse(cols[(int)RessIndex.PopLimit].Trim(), out pop)) {
                                    if (pop >= 1 && pop <= 10000) {
                                        numPopLimit.Value = pop;
                                    }
                                }
                                break;
                            }
                        }
                    }

                    bool freeProd = false;
                    var mProdUnit = Regex.Match(ressText, @"^FigRomInf00_Lanze_Schild\s*,\s*(.*)$", RegexOptions.Multiline);
                    if (mProdUnit.Success) {
                        string[] colsU = ParseCsvLine(mProdUnit.Groups[1].Value);
                        if (colsU.Length > (int)RessIndex.FigProdCostStart + 4) {
                            if (colsU[(int)RessIndex.FigProdCostStart + 4 - 1].Trim() == "0") {
                                freeProd = true;
                            }
                        }
                    }
                    chkFreeProd.Checked = freeProd;

                    bool freeUp = false;
                    var mUp = Regex.Match(ressText, @"^.*Ger_Kampf.*$", RegexOptions.Multiline);
                    if (mUp.Success) {
                        string[] cols = ParseCsvLine(mUp.Value);
                        if (cols.Length > (int)RessIndex.VolkresUpgradeStart) {
                            if (cols[(int)RessIndex.VolkresUpgradeStart].Trim() == "0") {
                                freeUp = true;
                            }
                        }
                    }
                    chkFreeUpgrade.Checked = freeUp;

                    bool noSpell = true;
                    var mPri = Regex.Match(ressText, @"^FigGerPri00_Priester\s*,.*", RegexOptions.Multiline);
                    if (mPri.Success) {
                        string[] cols = ParseCsvLine(mPri.Value);
                        if (cols.Length > (int)RessIndex.FigPriestSpellCostEnd) {
                            if (cols[(int)RessIndex.FigPriestSpellCostStart - 1].Trim() != "0" || cols[(int)RessIndex.FigPriestSpellCostStart].Trim() != "0") noSpell = false;
                        }
                    }
                    chkNoSpellCost.Checked = noSpell;
                }

                string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                if (File.Exists(exePath)) {
                    using (var fs = new FileStream(exePath, FileMode.Open, FileAccess.Read)) {
                        if (fs.Length > 0x161a8e) {
                            fs.Seek(0x161a88, SeekOrigin.Begin);
                            byte[] oldBytes = new byte[6];
                            fs.Read(oldBytes, 0, 6);
                            if (oldBytes[0] == 0x90 && oldBytes[1] == 0x90 && oldBytes[2] == 0x90 && oldBytes[3] == 0x90 && oldBytes[4] == 0x90 && oldBytes[5] == 0x90) {
                                chkFocusLoss.Checked = true;
                            } else if (oldBytes[0] == 0x89 && oldBytes[1] == 0x15 && oldBytes[2] == 0xC4 && oldBytes[3] == 0x7D && oldBytes[4] == 0x9E && oldBytes[5] == 0x02) {
                                chkFocusLoss.Checked = false;
                            } else {
                                chkFocusLoss.Checked = false;
                                Log(Loc.Get("LogExePatchWarning"));
                            }
                        } else {
                            Log(Loc.Get("LogExePatchWarning"));
                        }
                    }
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
            if (cols[(int)ObjdefIndex.Weapon1Akti].Trim() == "1") {
                double.TryParse(cols[(int)ObjdefIndex.Weapon1Relt].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out meleeRelt);
            }
            for (int w = 2; w <= 8; w++) {
                int wAktiIdx = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int wReltIdx = wAktiIdx + 6;
                int wDtypIdx = (int)ObjdefIndex.Weapon1Dtyp + (w - 1);
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
        /// 從兵種 CSV 行中解析出最大射程（遠程）或施法距離（祭司）。
        /// </summary>
        private static double GetUnitMaxRange(string[] cols, string utype) {
            if (utype == "priest") {
                double maxR = 0;
                int[] priestFields = { (int)ObjdefIndex.PriestSpell1, (int)ObjdefIndex.PriestSpell2, (int)ObjdefIndex.PriestSpell3 };
                foreach (int f in priestFields) {
                    if (f < cols.Length) {
                        double val;
                        if (double.TryParse(cols[f].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out val)) {
                            if (val > maxR) maxR = val;
                        }
                    }
                }
                return maxR;
            } else {
                double maxR = 0;
                int[] idxFields = { (int)ObjdefIndex.Weapon2RangeMin, (int)ObjdefIndex.Weapon2RangeMax, (int)ObjdefIndex.Weapon3RangeMin, (int)ObjdefIndex.Weapon3RangeMax };
                foreach (int f in idxFields) {
                    if (f < cols.Length) {
                        double val;
                        if (double.TryParse(cols[f].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out val)) {
                            if (val > maxR) maxR = val;
                        }
                    }
                }
                return maxR;
            }
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
        /// 檢查數值轉換成字串後是否超出遊戲引擎欄位長度限制，若超出則嘗試降低精度（F1、F0）以適應長度。
        /// </summary>
        private bool CheckLen(string val, int targetLen, out string finalVal) {
            val = val.Trim();
            finalVal = val;
            if (val.Length <= targetLen) return true;
            double dVal;
            if (double.TryParse(val, NumberStyles.Any, CultureInfo.InvariantCulture, out dVal)) {
                string f1 = dVal.ToString("F1", CultureInfo.InvariantCulture);
                if (f1.Length <= targetLen) {
                    finalVal = f1;
                    return true;
                }
                string f0 = Math.Round(dVal).ToString("F0", CultureInfo.InvariantCulture);
                if (f0.Length <= targetLen) {
                    finalVal = f0;
                    return true;
                }
            }
            return false;
        }

        /// <summary>
        /// 當「平衡模式」勾選狀態改變時，更新預設與現有的數據表格顯示。
        /// </summary>
        private void ChkBalance_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            LoadCurrentData(false);
            Log(string.Format("兵種屬性平衡與陣營特色已{0}", chkBalance.Checked ? "啟用" : "停用"));
        }
    }
}
