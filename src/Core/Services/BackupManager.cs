using System;
using System.IO;
using System.IO.Compression;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Globalization;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Core.Services
{
    public class BackupManager
    {
        private readonly Dictionary<string, byte[]> _backupFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        private Dictionary<string, string[]>? _backupUnitRows;
        private readonly object _unitRowsLock = new object();
        private readonly ILogger _logger;

        public BackupManager(ILogger logger)
        {
            _logger = logger;
        }

        public Dictionary<string, byte[]> BackupFiles => _backupFiles;

        public bool HasFile(string key) => _backupFiles.ContainsKey(key);

        public byte[] GetBackupBytes(string key)
        {
            if (_backupFiles.TryGetValue(key, out byte[]? bytes))
            {
                return bytes;
            }
            throw new InvalidOperationException("記憶體備份中找不到 " + key + "。");
        }

        public void Clear()
        {
            _backupFiles.Clear();
            lock (_unitRowsLock)
            {
                _backupUnitRows = null;
            }
        }

        public void LoadBackupZipToMemory(string gamePath)
        {
            string? resourceName = typeof(BackupManager).Assembly
                .GetManifestResourceNames()
                .FirstOrDefault(n => n.EndsWith("Backup.zip"));

            if (resourceName != null)
            {
                using Stream stream = typeof(BackupManager).Assembly.GetManifestResourceStream(resourceName)!;
                LoadZipToDictionary(stream);
                TryAutoHealBackupFiles(gamePath);
                ValidateBackupResources();
                _logger.Log(Loc.Get("SvcLogBackupLoadedEmbedded"));
                return;
            }

            string localZip = Path.Combine(AppContext.BaseDirectory, "Backup.zip");
            if (File.Exists(localZip))
            {
                using FileStream stream = File.OpenRead(localZip);
                LoadZipToDictionary(stream);
                TryAutoHealBackupFiles(gamePath);
                ValidateBackupResources();
                _logger.Log(Loc.Get("SvcLogBackupLoadedLocal"));
                return;
            }

            if (!TryLoadBackupFromGameDirectory(gamePath, false))
            {
                _logger.Log(Loc.Get("SvcLogBackupMissing"));
            }
        }

        public void TryAutoHealBackupFiles(string gamePath)
        {
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath))
            {
                return;
            }

            string[] requiredFiles = {
                "Against_Rome.exe",
                "SYSTEM/cl_script.ini",
                "SYSTEM/cl_epara.ini",
                "SYSTEM/ress.ini",
                "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
                "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau",
                "SYSTEM/CLMK/icon.ini",
                "SYSTEM/CLAK/cl_scint.ini",
                "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"
            };

            foreach (string relPath in requiredFiles)
            {
                if (!_backupFiles.ContainsKey(relPath))
                {
                    if (relPath == "SYSTEM/cl_epara.ini")
                    {
                        try
                        {
                            byte[] cleanEparaBytes = Encoding.GetEncoding(1251).GetBytes(GetCleanEparaText());
                            _backupFiles[relPath] = GameLZSS.CompressPfil(cleanEparaBytes, CreateEmptyPfilHeader());
                            _logger.Log(Loc.Get("SvcLogEparaHealed"));
                        }
                        catch (Exception ex)
                        {
                            _logger.Log(string.Format(Loc.Get("SvcLogEparaHealFailed"), ex.Message));
                        }
                    }
                    else
                    {
                        string fullPath = Path.Combine(gamePath, relPath.Replace('/', Path.DirectorySeparatorChar));
                        string bakPath = fullPath + ".bak";
                        string loadPath = File.Exists(bakPath) ? bakPath : fullPath;
                        if (File.Exists(loadPath))
                        {
                            try
                            {
                                byte[] healBytes = File.ReadAllBytes(loadPath);
                                if (relPath == "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau")
                                {
                                    // partgeo.dau 不在內嵌 Backup.zip 內，只能以現場檔案為基準：
                                    // 必須先驗證未被修改，並立即建立 .bak，之後每次啟動都優先讀 .bak，
                                    // 避免套用「拋射彈道增高」後的檔案被誤收為備份。
                                    if (!IsPartgeoOriginal(healBytes))
                                    {
                                        _logger.Log(string.Format(Loc.Get("SvcLogAutoHealFailed"), relPath, "partgeo.dau 已被修改，不能作為備份基準 / partgeo.dau is already modified"));
                                        continue;
                                    }
                                    if (!File.Exists(bakPath))
                                    {
                                        File.Copy(loadPath, bakPath, overwrite: false);
                                        _logger.Log(string.Format("已建立原版檔案實體備份: {0}", bakPath));
                                    }
                                }
                                _backupFiles[relPath] = healBytes;
                                _logger.Log(string.Format(Loc.Get("SvcLogAutoHealed"), relPath));
                            }
                            catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogAutoHealFailed"), relPath, ex.Message)); }
                        }
                    }
                }
            }

            bool hasTeamDat = _backupFiles.Keys.Any(k => k.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && k.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase));
            if (!hasTeamDat)
            {
                string mapsPath = Path.Combine(gamePath, "MAPS");
                if (Directory.Exists(mapsPath))
                {
                    try
                    {
                        string normalizedGamePath = Path.GetFullPath(gamePath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
                        foreach (string file in Directory.GetFiles(mapsPath, "team.dat", SearchOption.AllDirectories))
                        {
                            if (CustomMapManifest.IsCustomMapDirectory(Path.GetDirectoryName(file)!)) continue;
                            string relPath = Path.GetRelativePath(normalizedGamePath, file).Replace('\\', '/');
                            string bakPath = file + ".bak";
                            string loadPath = File.Exists(bakPath) ? bakPath : file;
                            _backupFiles[relPath] = File.ReadAllBytes(loadPath);
                        }
                        _logger.Log(Loc.Get("SvcLogTeamDatHealed"));
                    }
                    catch (Exception ex) { _logger.Log(string.Format(Loc.Get("SvcLogTeamDatHealFailed"), ex.Message)); }
                }
            }
        }

        public List<string> FindMissingBackupResources()
        {
            var missing = new List<string>();
            string[] requiredFiles = {
                "Against_Rome.exe",
                "SYSTEM/cl_script.ini",
                "SYSTEM/cl_epara.ini",
                "SYSTEM/ress.ini",
                "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
                "SYSTEM/CLMK/icon.ini",
                "SYSTEM/CLAK/cl_scint.ini",
                "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"
            };

            foreach (string key in requiredFiles)
            {
                if (!_backupFiles.ContainsKey(key))
                {
                    missing.Add(key);
                }
            }

            bool hasTeamDat = _backupFiles.Keys.Any(k => k.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && k.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase));
            if (!hasTeamDat)
            {
                missing.Add("MAPS/.../team.dat");
            }

            return missing;
        }

        public void ValidateBackupResources()
        {
            var missing = FindMissingBackupResources();
            if (missing.Count > 0)
            {
                string msg = Loc.Get("SvcLogBackupIncomplete") + "\r\n" + string.Join("\r\n", missing);
                _logger.Log(msg);
                throw new InvalidDataException(msg);
            }
        }

        public bool EnsureBackupLoadedForGamePath(string gamePath)
        {
            if (_backupFiles.Count > 0 && FindMissingBackupResources().Count == 0)
            {
                return true;
            }
            return TryLoadBackupFromGameDirectory(gamePath, true);
        }

        public bool TryLoadBackupFromGameDirectory(string gamePath, bool showError)
        {
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath))
            {
                if (showError)
                {
                    throw new InvalidDataException("找不到 Backup.zip，且遊戲路徑無效，無法建立本機備份。 / Backup.zip not found and game path is invalid.");
                }
                return false;
            }

            var loaded = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            string[] requiredFiles = {
                "Against_Rome.exe",
                "SYSTEM/cl_script.ini",
                "SYSTEM/cl_epara.ini",
                "SYSTEM/ress.ini",
                "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
                "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau",
                "SYSTEM/CLMK/icon.ini",
                "SYSTEM/CLAK/cl_scint.ini",
                "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"
            };

            try
            {
                foreach (string relPath in requiredFiles)
                {
                    if (relPath == "SYSTEM/cl_epara.ini")
                    {
                        try
                        {
                            byte[] cleanEparaBytes = Encoding.GetEncoding(1251).GetBytes(GetCleanEparaText());
                            loaded[relPath] = GameLZSS.CompressPfil(cleanEparaBytes, CreateEmptyPfilHeader());
                        }
                        catch (Exception ex)
                        {
                            _logger.Log(string.Format(Loc.Get("SvcLogEparaBuildFailed"), ex.Message));
                        }
                        continue;
                    }

                    string fullPath = Path.Combine(gamePath, relPath.Replace('/', Path.DirectorySeparatorChar));
                    string bakPath = fullPath + ".bak";

                    if (File.Exists(bakPath))
                    {
                        loaded[relPath] = File.ReadAllBytes(bakPath);
                    }
                    else
                    {
                        if (!File.Exists(fullPath))
                        {
                            continue;
                        }

                        byte[] fileBytes = File.ReadAllBytes(fullPath);

                        if (relPath == "Against_Rome.exe")
                        {
                            if (!IsExeOriginal(fileBytes))
                            {
                                throw new InvalidDataException("Against_Rome.exe 已經被修改過，或不是支援的原版檔案，無法建立備份。請先還原原版檔案。 / Against_Rome.exe is already modified or not supported. Please restore original file first.");
                            }
                        }
                        else if (relPath == "SYSTEM/DATA_MP/DEFAULTS/objdef.dau")
                        {
                            if (!IsObjdefOriginal(fileBytes))
                            {
                                throw new InvalidDataException("SYSTEM/DATA_MP/DEFAULTS/objdef.dau 已經被修改過，無法作為備份基準。請先驗證遊戲完整性。 / objdef.dau is already modified. Please verify game integrity first.");
                            }
                        }
                        else if (relPath == "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau")
                        {
                            if (!IsPartgeoOriginal(fileBytes))
                            {
                                throw new InvalidDataException("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau 已經被修改過，無法作為備份基準。請先驗證遊戲完整性。 / partgeo.dau is already modified. Please verify game integrity first.");
                            }
                        }

                        File.Copy(fullPath, bakPath, overwrite: false);
                        _logger.Log(string.Format("已建立原版檔案實體備份: {0}", bakPath));
                        loaded[relPath] = fileBytes;
                    }
                }

                string mapsPath = Path.Combine(gamePath, "MAPS");
                if (Directory.Exists(mapsPath))
                {
                    string normalizedGamePath = Path.GetFullPath(gamePath)
                        .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
                    foreach (string file in Directory.GetFiles(mapsPath, "team.dat", SearchOption.AllDirectories))
                    {
                        // 自製圖由 .arm_custom_map 明確標記；它們不是原廠備份基準，也不可建立 .bak。
                        if (CustomMapManifest.IsCustomMapDirectory(Path.GetDirectoryName(file)!)) continue;
                        string relPath = Path.GetRelativePath(normalizedGamePath, file).Replace('\\', '/');
                        string bakPath = file + ".bak";

                        if (File.Exists(bakPath))
                        {
                            loaded[relPath] = File.ReadAllBytes(bakPath);
                        }
                        else
                        {
                            byte[] fileBytes = File.ReadAllBytes(file);
                            File.Copy(file, bakPath, overwrite: false);
                            _logger.Log(string.Format("已建立原版地圖檔案實體備份: {0}", bakPath));
                            loaded[relPath] = fileBytes;
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                if (showError)
                {
                    throw;
                }
                _logger.Log("建立備份時發生錯誤: " + ex.Message);
                return false;
            }

            _backupFiles.Clear();
            foreach (var kvp in loaded)
            {
                _backupFiles[kvp.Key] = kvp.Value;
            }

            lock (_unitRowsLock)
            {
                _backupUnitRows = null;
            }

            TryAutoHealBackupFiles(gamePath);
            var missing = FindMissingBackupResources();
            if (missing.Count > 0)
            {
                if (showError)
                {
                    throw new InvalidDataException("遊戲目錄中缺少必要檔案，無法建立完整備份:\r\n" + string.Join("\r\n", missing));
                }
                return false;
            }

            _logger.Log(Loc.Get("SvcLogBackupFromGameDir"));
            return true;
        }

        private bool IsExeOriginal(byte[] exeBytes)
        {
            try
            {
                var focusState = ExePatchModel.GetExePatchState(exeBytes);
                var spellState = ExePatchModel.GetSpellAltarPatchState(exeBytes);
                var rangeState = ExePatchModel.GetVillageBuildRangePatchState(exeBytes);
                var setterState = ExePatchModel.GetVillageSetterPatchState(exeBytes);

                return focusState == ExePatchState.Original &&
                       spellState == ExeSpellAltarPatchState.Original &&
                       rangeState == ExeVillageRangePatchState.Original &&
                       setterState == ExeVillageSetterPatchState.Original;
            }
            catch
            {
                return false;
            }
        }

        private bool IsObjdefOriginal(byte[] dauBytes)
        {
            try
            {
                byte[] decomp = GameLZSS.DecompressPfil(dauBytes);
                string text = Encoding.GetEncoding(1251).GetString(decomp);
                string lineEnding = text.Contains("\r\n") ? "\r\n" : "\n";
                string[] lines = text.Split(new string[] { lineEnding }, StringSplitOptions.None);

                foreach (string line in lines)
                {
                    if (line.Length < 100) continue;
                    string[] cols = PatchText.ParseCsvLine(line);
                    if (cols.Length < 192) continue;

                    string name = cols[52].Trim();
                    if (name == "FigRomAnf00_Anfuehrer")
                    {
                        string hpStr = cols[19].Trim();
                        if (double.TryParse(hpStr, NumberStyles.Any, CultureInfo.InvariantCulture, out double hp))
                        {
                            if (Math.Abs(hp - 400) > 0.1)
                            {
                                return false;
                            }
                        }
                    }
                }
                return true;
            }
            catch
            {
                return false;
            }
        }

        /// <summary>以 Pfeil00（弓箭拋射物）的原版 ysub 值 5832704 (= 89.0 的 16.16 定點) 驗證 partgeo.dau 未被「拋射彈道增高」修改過。</summary>
        private static bool IsPartgeoOriginal(byte[] dauBytes)
        {
            try
            {
                byte[] decomp = GameLZSS.DecompressPfil(dauBytes);
                string text = Encoding.GetEncoding(1251).GetString(decomp);
                foreach (string line in text.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None))
                {
                    string[] cols = PatchText.ParseCsvLine(line);
                    if (cols.Length <= PartgeoPatcher.YsubColumn) continue;
                    if (cols[2].Trim() != "Pfeil00") continue;
                    return long.TryParse(cols[PartgeoPatcher.YsubColumn].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out long ysub) && ysub == 5832704;
                }
                return false;
            }
            catch
            {
                return false;
            }
        }

        private void LoadZipToDictionary(Stream stream)
        {
            _backupFiles.Clear();
            using (ZipArchive archive = new ZipArchive(stream))
            {
                foreach (ZipArchiveEntry entry in archive.Entries)
                {
                    if (entry.Name == "") continue;
                    string key = entry.FullName.Replace('\\', '/');
                    if (Path.IsPathRooted(key) ||
                        key.StartsWith("/", StringComparison.Ordinal) ||
                        key.Split('/').Any(part => part == ".."))
                    {
                        throw new InvalidDataException("Backup.zip contains an unsafe entry path: " + entry.FullName);
                    }
                    using (Stream entryStream = entry.Open())
                    {
                        using (MemoryStream ms = new MemoryStream())
                        {
                            entryStream.CopyTo(ms);
                            _backupFiles[key] = ms.ToArray();
                        }
                    }
                }
            }
            lock (_unitRowsLock)
            {
                _backupUnitRows = null;
            }
        }

        /// <summary>建立內容全零的 64-byte PFIL 標頭,供無原始檔可沿用標頭時壓縮使用。</summary>
        private static byte[] CreateEmptyPfilHeader()
        {
            byte[] header = new byte[64];
            header[0] = (byte)'P';
            header[1] = (byte)'F';
            header[2] = (byte)'I';
            header[3] = (byte)'L';
            return header;
        }

        public Dictionary<string, string[]> GetBackupUnitRows()
        {
            lock (_unitRowsLock)
            {
                if (_backupUnitRows != null) return _backupUnitRows;

                _backupUnitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
                if (_backupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out byte[]? dauBytes))
                {
                    byte[] decomp = GameLZSS.DecompressPfil(dauBytes);
                    string text = Encoding.GetEncoding(1251).GetString(decomp);
                    string lineEnding = text.Contains("\r\n") ? "\r\n" : "\n";
                    string[] lines = text.Split(new string[] { lineEnding }, StringSplitOptions.None);
                    for (int idx = 2; idx < lines.Length; idx++)
                    {
                        string line = lines[idx];
                        if (line.Length < 100) continue;
                        string[] cols = PatchText.ParseCsvLine(line);
                        if (cols.Length < 192) continue;
                        string name = cols[52].Trim(); // Name
                        if (TroopConfig.UnitMeta.ContainsKey(name) || name == "FigZivMan00_Zivilist")
                        {
                            _backupUnitRows[name] = cols;
                        }
                    }
                }
                return _backupUnitRows;
            }
        }

        // --- 屬性解析核心方法（原本寫在 UI 裡的） ---

        public static bool SupportsConfigurableSpellRadius(string key)
        {
            return key.Equals("FigKelPri00_Priester", StringComparison.OrdinalIgnoreCase) ||
                   key.Equals("FigHunPri00_Priester", StringComparison.OrdinalIgnoreCase);
        }

        public double[] GetOriginalStats(string key)
        {
            var backupUnitRows = GetBackupUnitRows();
            if (!backupUnitRows.ContainsKey(key))
            {
                return new double[] { 0, 0, 0, 0, 0, 0, 0, 0, 0 };
            }
            string[] cols = backupUnitRows[key];

            double hp = 0;
            double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out hp);

            double vw = 0;
            double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out vw);

            double aw = 0;
            double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out aw);

            string utype = "melee_inf";
            if (TroopConfig.UnitMeta.ContainsKey(key))
            {
                utype = TroopConfig.UnitMeta[key].UnitType;
            }

            double meleeDam = 0, rangedDam = 0;
            GetMeleeAndRangedDmg(cols, utype, out meleeDam, out rangedDam);

            double meleeRelt = 0, rangedRelt = 0;
            GetMeleeAndRangedRelt(cols, utype, out meleeRelt, out rangedRelt);

            double dmg = meleeDam;
            double relt = meleeRelt;
            if (utype == "ranged_inf" || utype == "ranged_cav")
            {
                dmg = rangedDam;
                relt = rangedRelt;
            }
            else if (utype == "siege")
            {
                dmg = Math.Max(meleeDam, rangedDam);
                relt = Math.Max(meleeRelt, rangedRelt);
            }
            else if (utype == "hybrid_inf")
            {
                dmg = meleeDam;
                relt = meleeRelt;
            }

            double origMoves = 0;
            double.TryParse(cols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);
            double speed = origMoves > 0 ? Math.Round(origMoves * 2.0, 1) : 0;

            double origSight = 0;
            double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origSight);
            double sight = origSight;

            double range = GetUnitMaxRange(cols, utype);

            bool supportsSpellRadius = SupportsConfigurableSpellRadius(key);
            double spellRadius = supportsSpellRadius ? 500 : 0;

            return new double[] { hp, dmg, vw, aw, speed, sight, relt, range, spellRadius };
        }

        public double[] GetDefaultBalancedStats(string key)
        {
            if (TroopConfig.BalancedUnitStats.TryGetValue(key, out double[]? stats))
            {
                return (double[])stats.Clone();
            }
            return GetOriginalStats(key);
        }

        public double[] GetBaseStatsForUnit(string key, PatchProfile options)
        {
            double[] original = GetOriginalStats(key);
            double[] balanced = options.Balance ? GetDefaultBalancedStats(key) : original;

            if (options.CustomUnitStats != null && options.CustomUnitStats.TryGetValue(key, out double[]? custom) && custom != null)
            {
                bool ignoreRange = TroopConfig.UnitMeta.TryGetValue(key, out var meta) &&
                    ((options.RangedRange3x && (meta.UnitType is "ranged_inf" or "ranged_cav" or "siege")) ||
                     (options.SpellEntireMap && meta.UnitType == "priest"));
                bool ignoreSpeed = options.UnitMovementSpeed2x;
                bool ignoreSpellRadius = options.SpellRange3x && SupportsConfigurableSpellRadius(key);
                return MergeUnitStatsLayers(balanced, custom, SupportsConfigurableSpellRadius(key), ignoreSpeed, ignoreRange, ignoreSpellRadius,
                    meta?.UnitType == "priest");
            }

            return balanced;
        }

        public static double[] MergeUnitStatsLayers(double[] fallback, double[] custom, bool supportsSpellRadius,
            bool ignoreMovementSpeed = false, bool ignoreRange = false, bool ignoreSpellRadius = false,
            bool removePriestSight = false)
        {
            ArgumentNullException.ThrowIfNull(fallback);
            ArgumentNullException.ThrowIfNull(custom);

            double[] layered = new double[9];
            for (int i = 0; i < layered.Length; i++)
            {
                if (i == 8 && !supportsSpellRadius)
                {
                    layered[i] = 0;
                    continue;
                }

                // 這三個欄位已從自訂兵種功能移除，永遠回到基準值；由四個獨立功能負責。
                if (i is 4 or 7 or 8 || (i == 5 && removePriestSight))
                {
                    layered[i] = fallback.Length > i ? fallback[i] : 0;
                    continue;
                }

                layered[i] = custom.Length > i
                    ? custom[i]
                    : (fallback.Length > i ? fallback[i] : 0);
            }
            return layered;
        }

        public static void GetMeleeAndRangedDmg(string[] cols, string utype, out double meleeDmg, out double rangedDmg)
        {
            meleeDmg = 0;
            rangedDmg = 0;
            for (int w = 1; w <= 8; w++)
            {
                int wAktiIdx = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int wDamIdx = wAktiIdx + 1;
                int wDtypIdx = (int)ObjdefIndex.Weapon1Dtyp + (w - 1);
                if (wAktiIdx >= cols.Length || wDamIdx >= cols.Length || wDtypIdx >= cols.Length)
                {
                    continue;
                }
                if (cols[wAktiIdx].Trim() == "1")
                {
                    double damVal;
                    double.TryParse(cols[wDamIdx].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out damVal);
                    string wDtyp = cols[wDtypIdx].Trim();
                    bool isRangedWeapon = (wDtyp == "1" || wDtyp == "2" || wDtyp == "3" || wDtyp == "4" || utype == "siege");
                    if (isRangedWeapon)
                    {
                        if (damVal > rangedDmg) rangedDmg = damVal;
                    }
                    else
                    {
                        if (damVal > meleeDmg) meleeDmg = damVal;
                    }
                }
            }
        }

        public static void GetMeleeAndRangedRelt(string[] cols, string utype, out double meleeRelt, out double rangedRelt)
        {
            meleeRelt = 0;
            rangedRelt = 0;
            for (int w = 1; w <= 8; w++)
            {
                int wAktiIdx = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int wReltIdx = wAktiIdx + 6;
                int wDtypIdx = (int)ObjdefIndex.Weapon1Dtyp + (w - 1);
                if (wAktiIdx >= cols.Length || wReltIdx >= cols.Length || wDtypIdx >= cols.Length)
                {
                    continue;
                }
                if (cols[wAktiIdx].Trim() == "1")
                {
                    double reltVal;
                    double.TryParse(cols[wReltIdx].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out reltVal);
                    string wDtyp = cols[wDtypIdx].Trim();
                    bool isRangedWeapon = (wDtyp == "1" || wDtyp == "2" || wDtyp == "3" || wDtyp == "4" || utype == "siege");
                    if (isRangedWeapon)
                    {
                        if (reltVal > 0 && (rangedRelt == 0 || reltVal < rangedRelt))
                        {
                            rangedRelt = reltVal;
                        }
                    }
                    else
                    {
                        if (reltVal > 0 && (meleeRelt == 0 || reltVal < meleeRelt))
                        {
                            meleeRelt = reltVal;
                        }
                    }
                }
            }
        }

        public static double GetUnitMaxRange(string[] cols, string utype)
        {
            double maxR = 0;
            if (utype == "priest")
                return double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double sight) ? sight : 0;
            for (int w = 1; w <= 8; w++)
            {
                int activeIndex = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                int rangeMinIndex = (int)ObjdefIndex.Weapon1RangeMin + (w - 1) * 8;
                int rangeMaxIndex = (int)ObjdefIndex.Weapon1RangeMax + (w - 1) * 8;
                if (rangeMaxIndex >= cols.Length || cols[activeIndex].Trim() != "1") continue;

                if (double.TryParse(cols[rangeMinIndex].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double minRange))
                {
                    maxR = Math.Max(maxR, minRange);
                }
                if (double.TryParse(cols[rangeMaxIndex].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double maxRange))
                {
                    maxR = Math.Max(maxR, maxRange);
                }
            }
            return maxR;
        }

        public static string GetCleanEparaText()
        {
            return @";Multiplikator fuer FormationsRotationTempo
;1.0 entspricht maximalem RotationsTempo wenn alle Figuren die FormationsPosition halten
;
;1.0 bis 500.0
[FormationRotationFaktor]
500.0

;Multiplikator fuer FormationsBewegungsTempo derjenigen Muckel 
;die die Formation gerade einhalten, so koennen nicht einhaltende Muckel wieder aufholen
;
;0.01 bis 1.00
[FormationSpeedFaktor]
0.7

;maximale Anzahl an Pfadfindungsversuchen pro Muckel wenn Ziel bei Stillstand der
;Formation durch Kollision belegt ist
;
;2..16
[FormationPathDepth]
2

;Zeit in ms die eine Figur in einer Formationsbewegung wartet, wenn sie auf eine Kollision trifft
;Defautl=1000
;0..X
[FormationCollisionWaitTime]
150

;Gibt in % an, wieviel eine Heohendifferenz von einem Pattern zum naechsten die
;Globale Hoehenrichtungsbeleuchtung beeinflusst
;
;0..100
[GlobalFloorLightIntensity]
10

;Gibt die Intensitдt an von 0 bis 100% an, mit welcher der 3-dimensionale Bewegungsvektor 
;genutzt wird, es ergibt sich fьr die Bewegungsgeschwindigkeit eine Konvexkombination (baryzentrisch)
;speed= 3Dspeed*Intensity + 2Dspeed*(100%-Intensity)  (default: intensity=100)
;
[MoveVector3DIntensity]
100

;Gibt die Geschwindigkeit der Bewegung der Wolkenspiegelungstextur an 
;0=keine 1=langsam 16=normal 256=schnell 4095=maximal (Default=16)
;
[CloudReflectMoveSpeed]
16

;Gibt den Angriffswertfaktor an, mit dem der normale Angriffswert im aktiven Zustand 'Berserker'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des AW, 0.5 bewirkt eine Halbierung
[BerserkerAWfaktor]
2.0

;Gibt den Damagewertfaktor an (Nahkampf), mit dem der normale Schaden im aktiven Zustand 'Berserker'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung
[BerserkerDAMfaktor]
2.0

;Gibt den Verteigungswertfaktor an (Nahkampf), mit dem der normale Verteigungswert im aktiven Zustand 'Berserker'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des VW, 0.5 bewirkt eine Halbierung, 0.0 bewirkt eine Setzung zu VW=0
[BerserkerVWfaktor]
0.0

;Gibt den Schussradiusfaktor an (Fernkampfwaffe 1+2), mit dem der normale Schussradius im aktiven Zustand 'Schuetzengeschick'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Radius, 0.5 bewirkt eine Halbierung
[SchuetzengeschickRADfaktor]
1.2

;Gibt den Schadensfaktor an (saemtlicher Schaeden), mit dem der normale Schaden im aktiven Zustand 'Schutzschild'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung
[SchutzschildDAMfaktor]
0.8

;Gibt den Schadensfaktor an (Waffe 0), mit dem der normale Schaden im aktiven Zustand 'Donnerschlag'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung
[DonnerschlagDAMfaktor]
1.5

;Gibt den Geschwindigkeitabschussfaktor fьr Geschosse an (Waffe 1-7) ausgehend vom ursprьnglich eingestellten Faktor 1.0
;annдhernde Korrektur der Flugbahnlдnge durch Multiplikation mit 1.52 des zugehцrigen Parameter Ysub in den ParticleDefaults
[ProjectileInitSpeedFactor]
1.5

;gibt die Unsicherheit der Vorhalte bei Projektilattacken an (nur fuer sich bewegende Ziele)
;0.0 bedeutet: keine Unsicherheit, das Projektil trifft mit Vorhalte absolut prдzise
;0.5 bedeutet: eine Abweichung von bis zu 0.5*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich
;1.0 bedeutet: eine Abweichung von bis zu 1.0*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich
;1.5 bedeutet: eine Abweichung von bis zu 1.5*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich
;Default =0.5
[ProjectileVarianceOnMove]
0.5

;gibt den Winkel zwischen Zielposition und prognostizierter Zielposition in Grad an, ab dem die Vorhalte abgeschaltet wird
;Vermeidung zu starker Abweichung zwischen Projektilflugrichtung und Blickrichtung des feuernden Objektes
;Default=45
[ProjectileVarianceMaximumAngle]
45

;Gibt den Bereich an, in dem die Distanz zwischen Zielposition und prognostizierter Zielposition variieren darf, bevor die
;Vorhalte abgeschaltet wird
;0.4 bedeutet: Distanz zur Vorhalteposition muss zwischen der (1-0.4)=0.6 und (1+0.4)=1.4'fachen Distanz zur Zielposition liegen
;Default=0.4
[ProjectileVarianceDistanceRange]
0.4

;Gibt die Zeit in ms, die als maximale Zeitdifferenz zwischen zwei logischen Frames an
;Default=3000
;(Wer hier rumfummelt und nicht genau weiss was er tut, bekommt die Figer abgehackt :-)
[MaxLogicFrameTime]
3000";
        }
    }
}
