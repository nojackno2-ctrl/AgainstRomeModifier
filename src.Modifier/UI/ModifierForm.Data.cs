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
using AgainstRomeModifier.Core.Services;
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
        /// 解析 TGA 圖像位元組資料，並將其轉換成 GDI+ 的 Bitmap 物件。
        /// 支援 8 位元索引彩色（附 24 位元調色盤）以及 24/32 位元真彩色 TGA 圖檔。
        /// </summary>
        public static Bitmap? LoadTga(byte[] tgaBytes) {
            // 解析下沉至 Core.TgaDecoder（無 System.Drawing 依賴）；此處僅把緊密排列的
            // BGRA 緩衝包成 32bpp ARGB Bitmap。SaveManagerForm 亦有相同薄殼。
            TgaImage? image = TgaDecoder.Decode(tgaBytes);
            if (image == null) return null;
            var bmp = new Bitmap(image.Width, image.Height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            var bmpData = bmp.LockBits(new Rectangle(0, 0, image.Width, image.Height),
                System.Drawing.Imaging.ImageLockMode.WriteOnly, bmp.PixelFormat);
            try {
                int rowBytes = image.Width * 4;
                if (bmpData.Stride == rowBytes) {
                    System.Runtime.InteropServices.Marshal.Copy(image.Bgra, 0, bmpData.Scan0, image.Bgra.Length);
                } else {
                    for (int y = 0; y < image.Height; y++)
                        System.Runtime.InteropServices.Marshal.Copy(
                            image.Bgra, y * rowBytes, bmpData.Scan0 + y * bmpData.Stride, rowBytes);
                }
            } finally {
                bmp.UnlockBits(bmpData);
            }
            return bmp;
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
                        string[] parts = PatchText.ParseCsvLine(line);
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

        /// <summary>顯示「當前屬性（原版對比修改後）」的表格。</summary>
        private DataGridView CreateCurrentStatsGrid() => CreateStatsGrid(isComparison: true);

        /// <summary>顯示「預設屬性（平衡模式則為平衡後數值）」的表格。</summary>
        private DataGridView CreateDefaultStatsGrid() => CreateStatsGrid(isComparison: false);

        /// <summary>
        /// 建立兵種屬性 DataGridView。當前頁與預設頁共用同一結構，僅差在欄位標題（是否帶「對比」）、
        /// 少數欄寬、Tier 欄的可見性/位置，以及當前頁專有的「a -> b」增減上色（CellFormatting）。
        /// </summary>
        private DataGridView CreateStatsGrid(bool isComparison) {
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

            dgv.Columns.Add(new DataGridViewImageColumn {
                Name = "Icon",
                HeaderText = "圖示",
                ImageLayout = DataGridViewImageCellLayout.Zoom,
                Width = 40
            });

            dgv.Columns.Add("Type", "部隊類型");
            dgv.Columns["Type"].Visible = false;

            dgv.Columns.Add("Style", "裝備分類");
            dgv.Columns["Style"].Visible = false;

            // 每欄：欄位鍵、對比頁標題/欄寬、預設頁標題/欄寬。
            (string Name, string CompareHeader, int CompareWidth, string PlainHeader, int PlainWidth)[] dataColumns = {
                ("Hp", "生命值對比", 85, "生命值", 85),
                ("MeleeDmg", "近戰傷害對比", 85, "近戰傷害", 85),
                ("RangedDmg", "遠程傷害對比", 85, "遠程傷害", 85),
                ("MeleeRelt", "近戰冷卻對比", 85, "近戰冷卻", 85),
                ("RangedRelt", "遠程冷卻對比", 85, "遠程冷卻", 90),
                ("Vw", "防禦對比", 85, "防禦力", 80),
                ("Aw", "戰鬥對比", 85, "戰鬥力", 80),
                ("Speed", "移動速度對比", 85, "移動速度", 80),
                ("Sight", "視野對比", 85, "視野", 80),
                ("Range", "射程對比", 85, "射程/技能距離", 100),
                ("SpellRadius", "法術半徑對比", 85, "法術半徑", 80),
            };
            foreach (var column in dataColumns) {
                dgv.Columns.Add(column.Name, isComparison ? column.CompareHeader : column.PlainHeader);
                dgv.Columns[column.Name].Width = isComparison ? column.CompareWidth : column.PlainWidth;
            }

            dgv.Columns.Add("Tier", "階級");
            if (isComparison) {
                dgv.Columns["Tier"].Visible = false;
            } else {
                dgv.Columns["Tier"].Width = 75;
                dgv.Columns["Tier"].DisplayIndex = 4;
            }

            if (isComparison) {
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
            }

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

                PatchProfile displayProfile = BuildCurrentPatchProfile(chkBalance.Checked);

                foreach (string key in TroopConfig.UnitOrder) {
                    if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                    var meta = TroopConfig.UnitMeta[key];
                    string faction = meta.Faction;
                    string tier = meta.Tier;
                    string utype = meta.UnitType;
                    string style = meta.Style;

                    if (!unitRows.ContainsKey(key)) continue;
                    string[] cols = unitRows[key];

                    double hp = PatchText.ParseDouble(cols, (int)ObjdefIndex.Hp);
                    double vw = PatchText.ParseDouble(cols, (int)ObjdefIndex.Vw);
                    double aw = PatchText.ParseDouble(cols, (int)ObjdefIndex.Aw);

                    double meleeDam = 0;
                    double rangedDam = 0;
                    UnitStatParser.GetMeleeAndRangedDamage(cols, utype, out meleeDam, out rangedDam);

                    double meleeRelt = 0;
                    double rangedRelt = 0;
                    UnitStatParser.GetMeleeAndRangedReload(cols, utype, out meleeRelt, out rangedRelt);

                    // 遠程/近戰主武器傷害基準值，供下方 scale 比例計算使用。
                    // 速度/視野/射程/法術半徑的最終值一律取自 unitStatsProjection.Project()
                    // 的 bases[]（見下方），故此處不再重複計算。
                    double origPrimaryDam = 1.0;
                    if (utype == "ranged_inf" || utype == "ranged_cav") {
                        origPrimaryDam = rangedDam;
                    } else if (utype == "siege") {
                        origPrimaryDam = Math.Max(meleeDam, rangedDam);
                    } else {
                        origPrimaryDam = meleeDam;
                    }

                    string displayName = Loc.GetUnitName(key);
                    string typeText = Loc.GetUnitType(utype);
                    string styleText = Loc.GetStyleText(style);

                    var iconImage = unitIcons.ContainsKey(key) ? unitIcons[key] : null;
                    double[] bases = unitStatsProjection.Project(key, displayProfile);

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
                    double defaultSpeed = bases[4];
                    double defaultSight = bases[5];
                    double defaultRange = bases[7];
                    double defaultSpellRadius = bases[8];

                    string meleeReltText = FormatVal(displayMeleeRelt, "F0");
                    string rangedReltText = FormatVal(displayRangedRelt, "F0");
                    string meleeDmgText = FormatVal(displayMeleeDam, "F1");
                    string rangedDmgText = FormatVal(displayRangedDam, "F1");

                    string tierText = Loc.GetTierText(tier);

                    var dgvTarget = defaultStatsGrids[faction];
                    dgvTarget.Rows.Add(
                        displayName, iconImage, typeText, styleText,
                        Math.Round(bases[0], 1),
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
                PatchProfile profile = patchEngine.DetectCurrentPatchState(gamePath, backupManager);

                if (syncUIWithFile) {
                    chkBalance.CheckedChanged -= ChkBalance_CheckedChanged;
                    chkAllUnitsEntireMapVision.CheckedChanged -= ChkAllUnitsEntireMapVision_CheckedChanged;
                    chkRangedRange3x.CheckedChanged -= ChkRangedRange3x_CheckedChanged;
                    chkUnitMovementSpeed2x.CheckedChanged -= ChkUnitMovementSpeed2x_CheckedChanged;
                    chkVillagerMovementSpeed5x.CheckedChanged -= ChkVillagerMovementSpeed5x_CheckedChanged;
                    chkSpellEntireMap.CheckedChanged -= ChkSpellEntireMap_CheckedChanged;
                    chkSpellRange3x.CheckedChanged -= ChkSpellRange3x_CheckedChanged;

                    foreach (var (id, toggle) in featureToggles) toggle.Checked = profile.GetBool(id);

                    chkBalance.CheckedChanged += ChkBalance_CheckedChanged;
                    chkAllUnitsEntireMapVision.CheckedChanged += ChkAllUnitsEntireMapVision_CheckedChanged;
                    chkRangedRange3x.CheckedChanged += ChkRangedRange3x_CheckedChanged;
                    chkUnitMovementSpeed2x.CheckedChanged += ChkUnitMovementSpeed2x_CheckedChanged;
                    chkVillagerMovementSpeed5x.CheckedChanged += ChkVillagerMovementSpeed5x_CheckedChanged;
                    chkSpellEntireMap.CheckedChanged += ChkSpellEntireMap_CheckedChanged;
                    chkSpellRange3x.CheckedChanged += ChkSpellRange3x_CheckedChanged;
                    SetGameSpeedSelection(profile.GameSpeed);
                    SetVillageGarrisonQuotaSelection(profile.VillageGarrisonQuotaMultiplier);

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
                    string[] cols = PatchText.ParseCsvLine(line);
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

                    double curHp = PatchText.ParseDouble(cols, (int)ObjdefIndex.Hp);
                    double curVw = PatchText.ParseDouble(cols, (int)ObjdefIndex.Vw);
                    double curAw = PatchText.ParseDouble(cols, (int)ObjdefIndex.Aw);

                    double origHp = PatchText.ParseDouble(origCols, (int)ObjdefIndex.Hp);
                    double origVw = PatchText.ParseDouble(origCols, (int)ObjdefIndex.Vw);
                    double origAw = PatchText.ParseDouble(origCols, (int)ObjdefIndex.Aw);

                    double origMeleeDmg = 0;
                    double origRangedDmg = 0;
                    UnitStatParser.GetMeleeAndRangedDamage(origCols, utype, out origMeleeDmg, out origRangedDmg);

                    double curMeleeDmg = 0;
                    double curRangedDmg = 0;
                    UnitStatParser.GetMeleeAndRangedDamage(cols, utype, out curMeleeDmg, out curRangedDmg);

                    double tempOrigMeleeRelt = 0;
                    double tempOrigRangedRelt = 0;
                    UnitStatParser.GetMeleeAndRangedReload(origCols, utype, out tempOrigMeleeRelt, out tempOrigRangedRelt);

                    double tempCurMeleeRelt = 0;
                    double tempCurRangedRelt = 0;
                    UnitStatParser.GetMeleeAndRangedReload(cols, utype, out tempCurMeleeRelt, out tempCurRangedRelt);

                    double origMoves = PatchText.ParseDouble(origCols, (int)ObjdefIndex.Moves);
                    double curMoves = PatchText.ParseDouble(cols, (int)ObjdefIndex.Moves);

                    double origSight = PatchText.ParseDouble(origCols, (int)ObjdefIndex.Sirad);
                    double curSight = PatchText.ParseDouble(cols, (int)ObjdefIndex.Sirad);

                    double origRange = UnitStatParser.GetMaximumRange(origCols, utype);
                    double curRange = UnitStatParser.GetMaximumRange(cols, utype);

                    double origSpellRadius = 0;
                    double curSpellRadius = 0;
                    // 只有 KEL/HUN 祭司的法術半徑是可設定項；GER 祭司顯示變動值會造成假象。
                    if (utype == "priest" && UnitStatParser.SupportsConfigurableSpellRadius(key)) {
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

        private void ChkAllUnitsEntireMapVision_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkAllUnitsEntireMapVision.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogAllUnitsEntireMapVisionToggled"), status));
        }

        private void ChkUnitMovementSpeed2x_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkUnitMovementSpeed2x.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogUnitMovementSpeed2xToggled"), status));
        }

        private void ChkVillagerMovementSpeed5x_CheckedChanged(object? sender, EventArgs e) {
            LoadDefaultStatsData();
            string status = chkVillagerMovementSpeed5x.Checked ? (Loc.CurrentLanguage == Language.English ? "enabled" : "啟用") : (Loc.CurrentLanguage == Language.English ? "disabled" : "停用");
            Log(string.Format(Loc.Get("LogVillagerMovementSpeed5xToggled"), status));
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
