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
        private static readonly Regex RegexMoraleLostMemLoad = new Regex(@"MoralsDecLostMem\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleFleeLoad = new Regex(@"MoralsDecFlee\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleOverPopLoad = new Regex(@"MoralsDecOverPop\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleIdleLoad = new Regex(@"MoralsIncIdle\s*=\s*GER\s*,\s*(\d+)", RegexOptions.Compiled);

        /// <summary>
        /// Loads the clean restore source. Public builds do not include original game assets,
        /// so the fallback source is the user's own installed game directory.
        /// </summary>
        private void LoadBackupZipToMemory() {
            string? resourceName = typeof(Program).Assembly
                .GetManifestResourceNames()
                .FirstOrDefault(n => n.EndsWith("Backup.zip"));

            if (resourceName != null) {
                using Stream stream = typeof(Program).Assembly.GetManifestResourceStream(resourceName)!;
                LoadZipToDictionary(stream);
                ValidateBackupResources();
                Log("已載入內嵌 Backup.zip 備份資料。");
                return;
            }

            string localZip = Path.Combine(AppContext.BaseDirectory, "Backup.zip");
            if (File.Exists(localZip)) {
                using FileStream stream = File.OpenRead(localZip);
                LoadZipToDictionary(stream);
                ValidateBackupResources();
                Log("已載入程式目錄中的 Backup.zip 備份資料。");
                return;
            }

            string gamePath = GetGamePath();
            if (!TryLoadBackupFromGameDirectory(gamePath, false)) {
                Log("找不到內嵌或本機 Backup.zip；請選擇合法的遊戲安裝目錄，程式會從該目錄建立本機記憶體備份。");
            }
        }

        private List<string> FindMissingBackupResources() {
            var missing = new List<string>();
            string[] requiredFiles = {
                "Against_Rome.exe",
                "SYSTEM/cl_script.ini",
                "SYSTEM/ress.ini",
                "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
                "SYSTEM/CLMK/icon.ini"
            };

            foreach (string key in requiredFiles) {
                if (!backupFiles.ContainsKey(key)) {
                    missing.Add(key);
                }
            }

            bool hasTeamDat = backupFiles.Keys.Any(k => k.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && k.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase));
            if (!hasTeamDat) {
                missing.Add("MAPS/.../team.dat");
            }

            return missing;
        }

        private void ValidateBackupResources() {
            var missing = FindMissingBackupResources();
            if (missing.Count > 0) {
                string msg = "備份來源缺少必要檔案，修改與還原功能可能無法安全執行:\r\n" + string.Join("\r\n", missing);
                Log(msg);
                MessageBox.Show(msg, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private bool EnsureBackupLoadedForGamePath(string gamePath) {
            if (backupFiles.Count > 0 && FindMissingBackupResources().Count == 0) {
                return true;
            }
            return TryLoadBackupFromGameDirectory(gamePath, true);
        }

        private bool TryLoadBackupFromGameDirectory(string gamePath, bool showError) {
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) {
                if (showError) {
                    MessageBox.Show("找不到 Backup.zip，且遊戲路徑無效，無法建立本機備份。", Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
                return false;
            }

            var loaded = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            string[] requiredFiles = {
                "Against_Rome.exe",
                "SYSTEM/cl_script.ini",
                "SYSTEM/ress.ini",
                "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
                "SYSTEM/CLMK/icon.ini"
            };

            foreach (string relPath in requiredFiles) {
                string fullPath = Path.Combine(gamePath, relPath.Replace('/', Path.DirectorySeparatorChar));
                if (File.Exists(fullPath)) {
                    loaded[relPath] = File.ReadAllBytes(fullPath);
                }
            }

            string mapsPath = Path.Combine(gamePath, "MAPS");
            if (Directory.Exists(mapsPath)) {
                string normalizedGamePath = Path.GetFullPath(gamePath)
                    .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
                foreach (string file in Directory.GetFiles(mapsPath, "team.dat", SearchOption.AllDirectories)) {
                    string relPath = Path.GetRelativePath(normalizedGamePath, file).Replace('\\', '/');
                    loaded[relPath] = File.ReadAllBytes(file);
                }
            }

            backupFiles.Clear();
            foreach (var kvp in loaded) {
                backupFiles[kvp.Key] = kvp.Value;
            }

            var missing = FindMissingBackupResources();
            if (missing.Count > 0) {
                string msg = "無法從遊戲目錄建立完整備份，缺少:\r\n" + string.Join("\r\n", missing);
                Log(msg);
                if (showError) {
                    MessageBox.Show(msg, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
                return false;
            }

            Log("已從使用者遊戲安裝目錄建立本機記憶體備份。");
            return true;
        }

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
        /// 將字串陣列重新組合成遊戲相容的逗號分隔字串。
        /// </summary>
        private static string ToCsvString(string[] cols) {
            if (cols == null) return "";
            return string.Join(",", cols);
        }

        private static bool HasHousingCapacityMultiplier(string currentContent, string originalContent, int multiplier) {
            var currentValues = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            string[] currentLines = currentContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in currentLines) {
                if (line.Length < 100) continue;
                string[] cols = ParseCsvLine(line);
                if (cols.Length <= (int)ObjdefIndex.HousingCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (int.TryParse(cols[(int)ObjdefIndex.HousingCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value)) {
                    currentValues[cols[(int)ObjdefIndex.Name].Trim()] = value;
                }
            }

            bool foundHousing = false;
            string[] originalLines = originalContent.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in originalLines) {
                if (line.Length < 100) continue;
                string[] cols = ParseCsvLine(line);
                if (cols.Length <= (int)ObjdefIndex.HousingCapacity || cols.Length <= (int)ObjdefIndex.Name) continue;
                if (!int.TryParse(cols[(int)ObjdefIndex.HousingCapacity].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int originalValue) || originalValue <= 0) continue;

                foundHousing = true;
                string name = cols[(int)ObjdefIndex.Name].Trim();
                if (!currentValues.TryGetValue(name, out int currentValue) || currentValue != checked(originalValue * multiplier)) {
                    return false;
                }
            }
            return foundHousing;
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
        /// 將備份 Zip 壓縮檔的串流解壓縮，並以相對路徑作為 Key，其 byte 陣列作為 Value 載入記憶體字典中。
        /// </summary>
        private void LoadZipToDictionary(Stream stream) {
            backupFiles.Clear();
            using (ZipArchive archive = new ZipArchive(stream)) {
                foreach (ZipArchiveEntry entry in archive.Entries) {
                    if (entry.Name == "") continue;
                    string key = entry.FullName.Replace('\\', '/');
                    if (Path.IsPathRooted(key) ||
                        key.StartsWith("/", StringComparison.Ordinal) ||
                        key.Split('/').Any(part => part == "..")) {
                        throw new InvalidDataException("Backup.zip contains an unsafe entry path: " + entry.FullName);
                    }
                    using (Stream entryStream = entry.Open()) {
                        using (MemoryStream ms = new MemoryStream()) {
                            entryStream.CopyTo(ms);
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
                Log(Loc.Get("LogGamePathNotSetIcon"));
                return;
            }
            string guiDatPath = Path.Combine(gamePath, "gui.dat");
            if (!File.Exists(guiDatPath)) {
                Log(Loc.Get("LogGuiDatNotFound"));
                return;
            }
            byte[]? iniData;
            if (!backupFiles.TryGetValue("SYSTEM/CLMK/icon.ini", out iniData)) {
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
                Log(Loc.Get("LogLoadIconFailed") + ex.Message + "\r\n" + ex.StackTrace);
            }
        }

        /// <summary>
        /// 獲取各兵種的平衡基礎屬性，若未啟用平衡模式，則直接返回原版屬性。
        /// </summary>
        private static double[] MergeUnitStatsLayers(double[] fallback, double[] custom, bool supportsSpellRadius) {
            ArgumentNullException.ThrowIfNull(fallback);
            ArgumentNullException.ThrowIfNull(custom);

            double[] layered = new double[9];
            for (int i = 0; i < layered.Length; i++) {
                if (i == 8 && !supportsSpellRadius) {
                    layered[i] = 0;
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
            double[] balanced = (forceBalance || chkBalance.Checked) ? GetDefaultBalancedStats(key) : original;

            if (customUnitStats != null && customUnitStats.TryGetValue(key, out double[]? custom) && custom != null) {
                return MergeUnitStatsLayers(balanced, custom, SupportsConfigurableSpellRadius(key));
            }

            return balanced;
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
                BackgroundColor = Color.FromArgb(20, 20, 25),
                ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(45, 45, 55),
                BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false,
                RowTemplate = { Height = 46 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical
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
                ReadOnly = true,
                ScrollBars = ScrollBars.Vertical
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
                Log(Loc.Get("LogObjdefNotFound"));
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

        private bool IsMaximumPopulationApplied(string gamePath) {
            string mapsPath = Path.Combine(gamePath, "MAPS");
            if (!Directory.Exists(mapsPath)) {
                return false;
            }

            bool foundActiveTeam = false;
            foreach (string teamFile in Directory.GetFiles(mapsPath, "team.dat", SearchOption.AllDirectories)) {
                try {
                    byte[] bytes = File.ReadAllBytes(teamFile);
                    byte[] decompBytes = GameLZSS.DecompressPfil(bytes);
                    string text = Encoding.GetEncoding(1251).GetString(decompBytes);
                    string[] lines = text.Split(new string[] { "\r\n", "\n" }, StringSplitOptions.None);
                    bool inTeamData = false;

                    foreach (string line in lines) {
                        string stripped = line.Trim();
                        if (stripped.StartsWith("[")) {
                            inTeamData = stripped.Equals("[teamdata]", StringComparison.OrdinalIgnoreCase);
                            continue;
                        }

                        if (inTeamData && stripped.Contains(",")) {
                            string[] cols = ParseCsvLine(line);
                            if (cols.Length >= 5 && int.TryParse(cols[4].Trim(), out int val) && val > 0) {
                                foundActiveTeam = true;
                                if (val != 1600) {
                                    return false;
                                }
                            }
                        }
                    }
                } catch {
                    return false;
                }
            }

            return foundActiveTeam;
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

                if (backupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out byte[]? originalObjdefBytes)) {
                    string originalObjdef = Encoding.GetEncoding(1251).GetString(GameLZSS.DecompressPfil(originalObjdefBytes));
                    chkHousingCapacity20x.Checked = HasHousingCapacityMultiplier(decomp, originalObjdef, HousingCapacityMultiplier);
                } else {
                    chkHousingCapacity20x.Checked = false;
                }

                // 自訂倍率控制項已移除，不進行 UI 賦值。

                // 偵測遊戲當前檔案是否已修改自訂兵種屬性（只要任何兵種有屬性與原版備份不同即視為已修改）
                bool isFileBalanced = false;
                foreach (string key in TroopConfig.UnitMeta.Keys) {
                    if (!unitRows.ContainsKey(key) || !origUnitRows.ContainsKey(key)) continue;
                    string utype = TroopConfig.UnitMeta[key].Item3;

                    string[] cols = unitRows[key];
                    string[] origCols = origUnitRows[key];

                    double curHp = 0, origHp = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curHp);
                    double.TryParse(origCols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origHp);

                    double curVw = 0, origVw = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curVw);
                    double.TryParse(origCols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origVw);

                    double curAw = 0, origAw = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curAw);
                    double.TryParse(origCols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origAw);

                    double curMoves = 0, origMoves = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curMoves);
                    double.TryParse(origCols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);

                    double curSight = 0, origSight = 0;
                    double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out curSight);
                    double.TryParse(origCols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origSight);

                    double curMeleeDmg = 0, curRangedDmg = 0;
                    GetMeleeAndRangedDmg(cols, utype, out curMeleeDmg, out curRangedDmg);
                    double origMeleeDmg = 0, origRangedDmg = 0;
                    GetMeleeAndRangedDmg(origCols, utype, out origMeleeDmg, out origRangedDmg);

                    double curMeleeRelt = 0, curRangedRelt = 0;
                    GetMeleeAndRangedRelt(cols, utype, out curMeleeRelt, out curRangedRelt);
                    double origMeleeRelt = 0, origRangedRelt = 0;
                    GetMeleeAndRangedRelt(origCols, utype, out origMeleeRelt, out origRangedRelt);

                    double curRange = GetUnitMaxRange(cols, utype);
                    double origRange = GetUnitMaxRange(origCols, utype);

                    bool hasDiff = Math.Abs(curHp - origHp) > 0.01 ||
                                   Math.Abs(curVw - origVw) > 0.01 ||
                                   Math.Abs(curAw - origAw) > 0.01 ||
                                   Math.Abs(curMoves - origMoves) > 0.01 ||
                                   Math.Abs(curSight - origSight) > 0.01 ||
                                   Math.Abs(curMeleeDmg - origMeleeDmg) > 0.01 ||
                                   Math.Abs(curRangedDmg - origRangedDmg) > 0.01 ||
                                   Math.Abs(curMeleeRelt - origMeleeRelt) > 0.01 ||
                                   Math.Abs(curRangedRelt - origRangedRelt) > 0.01 ||
                                   Math.Abs(curRange - origRange) > 0.01;

                    if (hasDiff) {
                        isFileBalanced = true;
                        break;
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
                    
                    MatchCollection civiMatches = RegexCiviLoad.Matches(clText);
                    if (syncUIWithFile) {
                        chkFastCiviProduction.Checked = civiMatches.Count > 0 && civiMatches.Cast<Match>().All(match =>
                            double.TryParse(match.Groups[2].Value, NumberStyles.Any, CultureInfo.InvariantCulture, out double delay) &&
                            delay > 0 && delay <= 500.0);
                    }

                    // spellRadMult 已在方法開始時預先載入處理。

                    bool infiniteMorale = false;
                    var matchLost = RegexMoraleLostMemLoad.Match(clText);
                    var matchFlee = RegexMoraleFleeLoad.Match(clText);
                    var matchOverPop = RegexMoraleOverPopLoad.Match(clText);
                    var matchIdle = RegexMoraleIdleLoad.Match(clText);
                    if (matchLost.Success && matchFlee.Success && matchOverPop.Success && matchIdle.Success &&
                        int.TryParse(matchLost.Groups[1].Value, out int lost) && lost == 0 &&
                        int.TryParse(matchFlee.Groups[1].Value, out int flee) && flee == 0 &&
                        int.TryParse(matchOverPop.Groups[1].Value, out int overPop) && overPop >= 99999999 &&
                        int.TryParse(matchIdle.Groups[1].Value, out int idle) && idle == 500) {
                        infiniteMorale = true;
                    }
                    chkInfiniteMorale.Checked = infiniteMorale;
                }

                string ressPath = Path.Combine(gamePath, @"SYSTEM\ress.ini");
                if (File.Exists(ressPath)) {
                    byte[] ressBytes = File.ReadAllBytes(ressPath);
                    byte[] decompRess = GameLZSS.DecompressPfil(ressBytes);
                    string ressText = Encoding.GetEncoding(1251).GetString(decompRess);
                    
                    bool freeProd = false;
                    var mProdUnit = Regex.Match(ressText, @"^FigRomInf00_Lanze_Schild\s*,.*$", RegexOptions.Multiline);
                    if (mProdUnit.Success) {
                        string[] colsU = ParseCsvLine(mProdUnit.Value);
                        freeProd = Enumerable.Range((int)RessIndex.FigProdCostStart,
                                (int)RessIndex.FigProdCostEnd - (int)RessIndex.FigProdCostStart + 1)
                            .All(index => index < colsU.Length && colsU[index].Trim() == "0");
                    }
                    chkFreeProd.Checked = freeProd;

                    bool freeUp = false;
                    var mUp = Regex.Match(ressText, @"^.*Ger_Kampf.*$", RegexOptions.Multiline);
                    if (mUp.Success) {
                        string[] cols = ParseCsvLine(mUp.Value);
                        freeUp = Enumerable.Range((int)VolkresIndex.UnitUpgradeStart,
                                (int)VolkresIndex.UnitUpgradeEnd - (int)VolkresIndex.UnitUpgradeStart + 1)
                            .All(index => index < cols.Length && cols[index].Trim() == "0");
                    }
                    chkFreeUpgrade.Checked = freeUp;

                    bool noSpell = false;
                    var mPri = Regex.Match(ressText, @"^FigGerPri00_Priester\s*,.*", RegexOptions.Multiline);
                    if (mPri.Success) {
                        string[] cols = ParseCsvLine(mPri.Value);
                        noSpell = Enumerable.Range((int)RessIndex.FigPriestSpellCostStart,
                                (int)RessIndex.FigPriestSpellCostEnd - (int)RessIndex.FigPriestSpellCostStart + 1)
                            .All(index => index < cols.Length && cols[index].Trim() == "0");
                    }
                    chkNoSpellCost.Checked = noSpell;
                }

                if (syncUIWithFile) {
                    chkMaxPopulation.Checked = IsMaximumPopulationApplied(gamePath);
                }

                if (TryReadEndlessAiModeState(gamePath, out bool endlessUltimateEnabled)) {
                    chkAiUltimateMode.Checked = endlessUltimateEnabled;
                } else {
                    chkAiUltimateMode.Checked = false;
                    Log("無盡模式 AI 腳本不是完整的原版或終極模式狀態；已取消勾選，重新套用可修復一致性。");
                }

                string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                chkDgVoodoo.Checked = IsDgVoodooInstalled(gamePath);
                if (File.Exists(exePath)) {
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    ExePatchState exePatchState = GetExePatchState(exeBytes);
                    chkFocusLoss.Checked = exePatchState == ExePatchState.FocusPatched;
                    if (exePatchState == ExePatchState.Unknown) {
                        Log(Loc.Get("LogExePatchWarning"));
                    }

                    ExeVillageRangePatchState villageRangeState = GetVillageBuildRangePatchState(exeBytes);
                    if (villageRangeState == ExeVillageRangePatchState.Expanded ||
                        villageRangeState == ExeVillageRangePatchState.LegacyLogicOnly) {
                        Log("偵測到已停用的村莊範圍候選補丁；下一次套用或相容性還原時會恢復四處原版 bytes。");
                    } else if (villageRangeState == ExeVillageRangePatchState.Unknown) {
                        Log(Loc.Get("LogVillageBuildRangeWarning"));
                    }

                    ExeVillageSetterPatchState villageSetterState = GetVillageSetterPatchState(exeBytes);
                    chkVillageBuildRange.Checked = villageSetterState == ExeVillageSetterPatchState.Legacy2x ||
                        villageSetterState == ExeVillageSetterPatchState.Expanded2Point5x;
                    if (villageSetterState == ExeVillageSetterPatchState.Unknown) {
                        Log(Loc.Get("LogVillageBuildRangeWarning"));
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
            string status = chkBalance.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogBalanceToggled"), status));
        }
    }
}
