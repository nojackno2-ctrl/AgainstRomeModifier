using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private static readonly object LogLock = new object();
        private const string LanguageBackupDirectoryName = ".against-rome-modifier-language-backup";
        private const string LanguageBackupManifestName = "manifest.json";
        private static readonly Regex RegexRadiusPatch = new Regex(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexCiviPatch = new Regex(@"^CiviDelay\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        // LPIncIdle：閒置單位每 N 毫秒執行一次自動回血（原版四陣營皆 15000）。
        // 現行功能不修改此間隔；保留規則只用於把舊版安裝遷回原始值。
        private static readonly Regex RegexLpIncIdlePatch = new Regex(@"^LPIncIdle\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleLostMemPatch = new Regex(@"^(MoralsDecLostMem\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleFleePatch = new Regex(@"^(MoralsDecFlee\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleOverPopPatch = new Regex(@"^(MoralsDecOverPop\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleIncIdlePatch = new Regex(@"^(MoralsIncIdle\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleKey = new Regex(@"^(MoralsDecLostMem|MoralsDecFlee|MoralsDecOverPop|MoralsIncIdle)\s*=\s*([A-Z]{3})\s*,", RegexOptions.Compiled);
        private static readonly Regex RegexSpellValuePatch = new Regex(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexSpecialAbilityValuePatch = new Regex(@"^(Value\d*)\s*=\s*([A-Z]{3})\s*,\s*(SAbility\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexSpellODefPatch = new Regex(@"^(SpellODef\d*)\s*=\s*(KEL)\s*,\s*(Spell3)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly byte[] ExeFocusOriginalBytes = new byte[] { 0x89, 0x15, 0xC4, 0x7D, 0x9E, 0x02 };
        private static readonly byte[] ExeFocusPatchedBytes = new byte[] { 0x90, 0x90, 0x90, 0x90, 0x90, 0x90 };
        private const long ExeFocusPatchOffset = 0x161a88;
        private const long ExeFocusPatchRequiredLength = 0x161a8e;

        private static readonly (long Offset, byte[] Original, byte[] Patched)[] SpellAltarPatchSites = new[] {
            // Germans
            (0x4A112L, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF }),
            (0x4A136L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF }),
            (0x4A15AL, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF }),
            (0x4A0E3L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00 }),

            // Celts
            (0x4A1CCL, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }),
            (0x4A293L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }),
            (0x4A2B7L, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }),
            (0x4A249L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }),

            // Huns
            (0x4A329L, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }),
            (0x4A3F0L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }),
            (0x4A414L, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }),
            (0x4A3A6L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }),
        };
        // Rejected village-range candidates. Retained only to detect and restore old writes.
        private static readonly byte[] ExeVillageRangeXOriginalBytes = new byte[] { 0xC1, 0xE2, 0x06 };
        private static readonly byte[] ExeVillageRangeZOriginalBytes = new byte[] { 0xC1, 0xE1, 0x06 };
        private static readonly byte[] ExeVillageRangeXPatchedBytes = new byte[] { 0xC1, 0xE2, 0x07 };
        private static readonly byte[] ExeVillageRangeZPatchedBytes = new byte[] { 0xC1, 0xE1, 0x07 };
        private static readonly byte[] ExeVillageFrameXOriginalBytes = new byte[] { 0xC1, 0xE6, 0x06 };
        private static readonly byte[] ExeVillageFrameZOriginalBytes = new byte[] { 0xC1, 0xE7, 0x06 };
        private static readonly byte[] ExeVillageFrameXPatchedBytes = new byte[] { 0xC1, 0xE6, 0x07 };
        private static readonly byte[] ExeVillageFrameZPatchedBytes = new byte[] { 0xC1, 0xE7, 0x07 };
        private const long ExeVillageRangeXPatchOffset = 0x1366c4;
        private const long ExeVillageRangeZPatchOffset = 0x1366cd;
        private const long ExeVillageFrameXPatchOffset = 0x0d722c;
        private const long ExeVillageFrameZPatchOffset = 0x0d723b;
        private const long ExeVillageRangePatchRequiredLength = 0x1366d0;
        private static readonly byte[] ExeVillageSetterHookOriginalBytes = new byte[] {
            0x85, 0xF6, 0x7C, 0xA6, 0x85, 0xFF, 0x7C, 0xA2
        };
        private static readonly byte[] ExeVillageSetterHookPatchedBytes = new byte[] {
            0xE9, 0xC9, 0xC0, 0x02, 0x00, 0x90, 0x90, 0x90
        };
        private static readonly byte[] ExeVillageSetterCaveOriginalBytes = new byte[39];
        // Previous modifier builds installed a 33-byte 2x trampoline and left the
        // following six bytes as zero padding. Keep recognizing it so Apply can
        // migrate an already-patched executable to the current 2.5x version.
        private static readonly byte[] ExeVillageSetterCaveLegacy2xBytes = new byte[] {
            0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
            0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
            0xD1, 0xE6, 0xD1, 0xE7, 0x57, 0x56, 0x50, 0xE8,
            0x55, 0xE3, 0xF5, 0xFF, 0xE9, 0x21, 0x3F, 0xFD, 0xFF,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        };
        private static readonly byte[] ExeVillageSetterCavePatchedBytes = new byte[] {
            0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
            0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
            0x8D, 0x34, 0x76, 0x90, 0x90,
            0x8D, 0x3C, 0x7F, 0x90, 0x90,
            0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
            0xE9, 0x1B, 0x3F, 0xFD, 0xFF
        };
        private static readonly byte[] ExeVillageSetterCaveLegacy2Point5xBytes = new byte[] {
            0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
            0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
            0x8D, 0x34, 0xB6, 0xD1, 0xEE,
            0x8D, 0x3C, 0xBF, 0xD1, 0xEF,
            0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
            0xE9, 0x1B, 0x3F, 0xFD, 0xFF
        };
        private const long ExeVillageSetterHookOffset = 0x1364c1;
        private const long ExeVillageSetterCaveOffset = 0x16258f;
        private const long ExeVillageSetterPatchRequiredLength = 0x1625b6;
        private const int HousingCapacityMultiplier = 20;
        private const int StorageCapacityMultiplier = 10;
        private const int FoodHealAmountOriginal = 1;
        private const int FoodHealAmountUltimate = 10;

        private static readonly (string File, int AddLpSymbolIndex)[] FoodHealAmountSites = new[] {
            ("ak_anfuehrer", 87),
            ("ak_artillerie", 69),
            ("ak_geisterreiter", 68),
            ("ak_kampfverband", 89),
            ("ak_krieger", 73),
            ("ak_kundschafterwolf", 77),
            ("ak_landtier", 72),
            ("ak_packpferd", 57),
            ("ak_priester", 92),
            ("ak_verbandswolf", 68),
            ("ak_zivilist", 83),
            ("ak_zivilverband", 87),
        };



        /// <summary>
        /// 記錄日誌訊息並寫入至本地 modifier_log.txt 檔案。
        /// </summary>
        private void Log(string message) {
            string text = string.Format("[{0}] {1}\r\n", DateTime.Now.ToString("HH:mm:ss"), message);
            try {
                lock (LogLock) {
                    File.AppendAllText(Path.Combine(AppContext.BaseDirectory, "modifier_log.txt"), text, Encoding.UTF8);
                }
            } catch (Exception ex) {
                System.Diagnostics.Debug.WriteLine("日誌檔案寫入失敗: " + ex.Message);
            }
        }

        /// <summary>
        /// 安全寫入檔案至指定路徑（附帶 3 次重試機制以防止檔案暫時鎖定）。
        /// </summary>
        private void SafeWriteAllBytes(string dest, byte[] bytes, FileRollbackScope? rollback = null) {
            int maxRetries = 3;
            int delayMs = 500;
            string? dir = Path.GetDirectoryName(dest);
            if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) {
                Directory.CreateDirectory(dir);
            }

            for (int i = 0; i < maxRetries; i++) {
                string tempFile = Path.Combine(dir ?? AppContext.BaseDirectory, Path.GetFileName(dest) + "." + Guid.NewGuid().ToString("N") + ".tmp");
                try {
                    rollback?.TrackFile(dest);
                    File.WriteAllBytes(tempFile, bytes);

                    if (File.Exists(dest)) {
                        File.SetAttributes(dest, FileAttributes.Normal);
                        File.Replace(tempFile, dest, null, true);
                    } else {
                        File.Move(tempFile, dest);
                    }
                    return;
                } catch (IOException ioEx) {
                    try {
                        if (File.Exists(tempFile)) {
                            File.SetAttributes(tempFile, FileAttributes.Normal);
                            File.Delete(tempFile);
                        }
                    } catch { }

                    if (i == maxRetries - 1) {
                        throw new Exception(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, ioEx.Message), ioEx);
                    }
                    System.Threading.Thread.Sleep(delayMs);
                } catch (UnauthorizedAccessException accessEx) {
                    try {
                        if (File.Exists(tempFile)) {
                            File.SetAttributes(tempFile, FileAttributes.Normal);
                            File.Delete(tempFile);
                        }
                    } catch { }

                    if (i == maxRetries - 1) {
                        throw new Exception(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, accessEx.Message), accessEx);
                    }
                    System.Threading.Thread.Sleep(delayMs);
                }
            }
        }

        /// <summary>
        /// 安全複製檔案至指定路徑（附帶 3 次重試機制以防止檔案暫時鎖定）。
        /// </summary>
        private void SafeCopyFile(string src, string dest, bool overwrite, FileRollbackScope? rollback = null) {
            if (!overwrite && File.Exists(dest)) {
                throw new IOException("目標檔案已存在: " + dest);
            }
            SafeWriteAllBytes(dest, File.ReadAllBytes(src), rollback);
        }

        /// <summary>
        /// 設定所有操作按鈕的啟用/停用狀態，避免在套用或還原時發生重複操作。
        /// </summary>
        private void SetActionButtonsEnabled(bool enabled) {
            if (InvokeRequired) {
                Invoke(new Action(() => SetActionButtonsEnabled(enabled)));
                return;
            }
            btnLoadCurrent.Enabled = enabled;
            btnRestore.Enabled = enabled;
            btnApply.Enabled = enabled;
            btnStartGame.Enabled = enabled;
            btnNavSystem.Enabled = enabled;
            btnNavDefaultStats.Enabled = enabled;
            btnNavCurrentStats.Enabled = enabled;
            btnNavSkills.Enabled = enabled;
            btnNavDoc.Enabled = enabled;
            btnNavSaveManager.Enabled = enabled;
        }

        /// <summary>
        /// 開啟資料夾瀏覽對話框，供使用者手動選擇遊戲的安裝路徑。
        /// </summary>
        private void BtnBrowseGamePath_Click(object? sender, EventArgs e) {
            using (var fbd = new FolderBrowserDialog()) {
                fbd.Description = Loc.Get("LogBrowseTitle");
                if (fbd.ShowDialog() == DialogResult.OK) {
                    txtGamePath.Text = fbd.SelectedPath;
                    EnsureBackupLoadedForGamePath(fbd.SelectedPath);
                    chkDgVoodoo.Checked = IsDgVoodooInstalled(fbd.SelectedPath);
                    LoadIcons();
                    LoadDefaultStatsData();
                }
            }
        }

        /// <summary>
        /// 當使用者點擊「套用修改」按鈕時觸發，非同步套用畫面上設定的所有兵種屬性、遊戲規則、語言包等修改。
        /// </summary>
        private async void BtnApply_Click(object? sender, EventArgs e) {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath) || !File.Exists(Path.Combine(gamePath, "Against_Rome.exe"))) {
                MessageBox.Show(Loc.Get("MsgWrongGameDir"), Loc.Get("TitlePathError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            if (!EnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }

            FileRollbackScope? rollback = null;
            try {
                DialogResult confirm = MessageBox.Show(Loc.Get("MsgConfirmApply"), Loc.Get("TitleConfirm"), MessageBoxButtons.YesNo, MessageBoxIcon.Question);
                if (confirm != DialogResult.Yes) return;

                SetActionButtonsEnabled(false);
                Log(Loc.Get("LogStartApply"));
                rollback = new FileRollbackScope();
                Log("已建立修改前檔案回復點。");

                bool focusLoss = chkFocusLoss.Checked;
                bool fastCiviProduction = chkFastCiviProduction.Checked;
                bool infMorale = chkInfiniteMorale.Checked;
                bool freeProd = chkFreeProd.Checked;
                bool freeUp = chkFreeUpgrade.Checked;
                bool noSpell = chkNoSpellCost.Checked;
                bool maxPopulation = chkMaxPopulation.Checked;
                bool balance = chkBalance.Checked;
                bool housingCapacity20x = chkHousingCapacity20x.Checked; bool storageCapacity10x = chkStorageCapacity10x.Checked;
                bool fastBuildUpgradeRepair = chkFastBuildUpgradeRepair.Checked;
                bool foodHealing10x = chkFoodHealing10x.Checked;
                bool toEng = chkToEng.Checked;
                bool[] aiModuleEnabled = {
                    chkAiM1.Checked, chkAiM2.Checked, chkAiM3.Checked, chkAiM4.Checked, chkAiM5.Checked
                };
                bool dgVoodoo = chkDgVoodoo.Checked;
                bool villageBuildRange = chkVillageBuildRange.Checked;
                bool noSpellAltar = chkNoSpellAltar.Checked;
                bool spellEnhancement = chkSpellEnhancement.Checked;
                bool leaderGloryKeep = chkLeaderGloryKeep.Checked;

                // 收集技能與首領屬性字典 (cl_epara & cl_script SAbility & objdef.dau)
                bool modSkillsAndGlory = chkModSkillsAndGlory.Checked;
                var generalSkillsDict = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
                foreach (DataGridViewRow row in dgvGeneralSkills.Rows) {
                    string key = row.Cells["SkillKey"].Value?.ToString() ?? "";
                    string valStr = modSkillsAndGlory
                        ? (row.Cells["SkillValue"].Value?.ToString() ?? "0")
                        : (row.Cells["SkillDefault"].Value?.ToString() ?? "0");
                    if (!string.IsNullOrEmpty(key) && double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out double v)) {
                        generalSkillsDict[key] = v;
                    }
                }

                var leaderGloryDict = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
                if (modSkillsAndGlory) {
                    foreach (DataGridViewRow row in dgvLeaderGlory.Rows) {
                        string leaderKey = row.Cells["LeaderKey"].Value?.ToString() ?? "";
                        if (!string.IsNullOrEmpty(leaderKey)) {
                            double awStuf = double.Parse(row.Cells["AwStuf"].Value?.ToString() ?? "0", CultureInfo.InvariantCulture);
                            double vwStuf = double.Parse(row.Cells["VwStuf"].Value?.ToString() ?? "0", CultureInfo.InvariantCulture);
                            double damStuf = double.Parse(row.Cells["DamStuf"].Value?.ToString() ?? "0", CultureInfo.InvariantCulture);
                            double moraleBonus = double.Parse(row.Cells["MoraleBonus"].Value?.ToString() ?? "0", CultureInfo.InvariantCulture);
                            double moraleTime = double.Parse(row.Cells["MoraleTime"].Value?.ToString() ?? "0", CultureInfo.InvariantCulture);
                            double maxRuhm = double.Parse(row.Cells["MaxRuhm"].Value?.ToString() ?? "0", CultureInfo.InvariantCulture);
                            leaderGloryDict[leaderKey] = new double[] { awStuf, vwStuf, damStuf, moraleBonus, moraleTime, maxRuhm };
                        }
                    }
                }

                await Task.Run(() => {
                    // 1. Dry Run 階段：在記憶體中生成所有補丁 byte[] 並驗證
                    var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

                    // A. Against_Rome.exe
                    string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    bool exeModified = false;
                    ApplyExePatch(exeBytes, focusLoss, villageBuildRange, noSpellAltar, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // B. cl_script.ini
                    byte[] clBytes = GetPatchedClScriptBytes(gamePath, fastCiviProduction, infMorale, balance, spellEnhancement, generalSkillsDict);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_script.ini")] = clBytes;

                    // H. cl_epara.ini
                    byte[] eparaBytes = GetPatchedClEparaBytes(gamePath, generalSkillsDict);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_epara.ini")] = eparaBytes;

                    // G. cl_scint.ini
                    byte[] scintBytes = GetPatchedClScintBytes(gamePath, spellEnhancement);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini")] = scintBytes;

                    // C. ress.ini
                    byte[] ressBytes = GetPatchedRessBytes(freeProd, freeUp, noSpell);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\ress.ini")] = ressBytes;

                    // D. objdef.dau
                    byte[] objdefBytes = GetPatchedObjdefBytes(balance, housingCapacity20x, storageCapacity10x, fastBuildUpgradeRepair, leaderGloryDict);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau")] = objdefBytes;

                    // E. team.dat
                    var teamDatPatches = GetPatchedTeamDatBytes(maxPopulation);
                    foreach (var kvp in teamDatPatches) {
                        patchedFiles[Path.Combine(gamePath, kvp.Key.Replace('/', '\\'))] = kvp.Value;
                    }

                    // F. endless AI scripts, templates, and economy modules
                    // 每個模組(M1..M5)由各自的獨立勾選框驅動；R0 常駐修復無論如何都執行。
                    var orchestrator = new EndlessAiOrchestrator();
                    for (int i = 0; i < orchestrator.UserModules.Count && i < aiModuleEnabled.Length; i++) {
                        orchestrator.ApplyModule(gamePath, orchestrator.UserModules[i], aiModuleEnabled[i]);
                    }
                    orchestrator.ApplyMandatoryRepair(gamePath);

                    // Dry Run 順利結束，未拋出任何異常。進入實體檔案寫入與交易範圍
                    foreach (var kvp in patchedFiles) {
                        SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
                    }
                    orchestrator.SaveAll(gamePath, rollback);

                    // 其它不涉及複雜解壓修改且安全的補丁
                    ApplyLanguagePatch(gamePath, toEng, rollback);
                    ApplyDgVoodooPatch(gamePath, dgVoodoo, rollback);

                    // 首領死亡榮耀保留
                    ApplyLeaderGloryKeepPatch(gamePath, leaderGloryKeep, rollback);

                    // 待機回血：12 個 Fig* 單位 AI 腳本的 s_addLP 單次加血量 1 -> 10。
                    ApplyFoodHealingAmountPatch(gamePath, foodHealing10x, rollback);
                });

                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                Log(Loc.Get("LogApplyAllSuccess"));
                MessageBox.Show(Loc.Get("MsgApplySuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful apply: " + uiEx.Message);
                }
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("套用失敗，開始回復已修改的檔案。");
                    rollback.RestoreAll(Log);
                    Log("檔案回復流程已完成。");
                }
                Log(Loc.Get("MsgApplyFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgApplyFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

        private static bool ShouldZeroFigFreeProductionField(int index) {
            return index >= (int)RessIndex.FigProdCostStart && index <= (int)RessIndex.FigProdCostEnd;
        }

        private sealed class LanguageBackupManifest {
            public List<string> ExistingFiles { get; set; } = new List<string>();
            public List<string> MissingFiles { get; set; } = new List<string>();
        }

        /// <summary>
        /// 將所有遊戲設定（屬性、相容性、語言包）恢復為官方原版初始設定。
        /// </summary>
        private async void RestoreAll() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath) || !File.Exists(Path.Combine(gamePath, "Against_Rome.exe"))) {
                MessageBox.Show(Loc.Get("MsgWrongGameDir"), Loc.Get("TitlePathError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            if (!EnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreAll"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => {
                    var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

                    // EXE 還原
                    string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    bool exeModified = false;
                    ApplyExePatch(exeBytes, false, false, false, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // 無盡 AI 還原
                    var orchestrator = new EndlessAiOrchestrator();
                    foreach (var module in orchestrator.UserModules) {
                        orchestrator.ApplyModule(gamePath, module, false);
                    }
                    orchestrator.ApplyMandatoryRepair(gamePath);

                    // 還原其它屬性 INI 與 team.dat 到備份原版
                    RestoreStatsOnlyInternal(gamePath, rollback);

                    // 統一寫入記憶體修改之檔案
                    foreach (var kvp in patchedFiles) {
                        SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
                    }
                    orchestrator.SaveAll(gamePath, rollback);
                    ApplyLeaderGloryKeepPatch(gamePath, false, rollback);
                    ApplyFoodHealingAmountPatch(gamePath, false, rollback);
                    ApplyLanguagePatch(gamePath, false, rollback);
                    ApplyDgVoodooPatch(gamePath, false, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkFocusLoss.Checked = false;
                chkNoSpellAltar.Checked = false;
                chkToEng.Checked = false;
                chkAiM1.Checked = false; chkAiM2.Checked = false; chkAiM3.Checked = false; chkAiM4.Checked = false; chkAiM5.Checked = false;
                chkHousingCapacity20x.Checked = false; chkStorageCapacity10x.Checked = false;
                chkFastBuildUpgradeRepair.Checked = false;
                chkFoodHealing10x.Checked = false;
                chkLeaderGloryKeep.Checked = false;
                chkMaxPopulation.Checked = false;
                chkFastCiviProduction.Checked = false;
                chkFreeProd.Checked = false;
                chkFreeUpgrade.Checked = false;
                chkNoSpellCost.Checked = false;
                chkInfiniteMorale.Checked = false;
                chkBalance.Checked = false;
                chkModSkillsAndGlory.Checked = false;
                chkDgVoodoo.Checked = IsDgVoodooInstalled(gamePath);
                chkVillageBuildRange.Checked = false;
                customUnitStats = null;
                presetFileSourceType = "default";
                presetFileName = "";
                UpdateTroopPresetLabel();
                LoadDefaultStatsData(); // 重新載入表格以呈現原版
                Log(Loc.Get("LogRestoreAllDone"));
                MessageBox.Show(Loc.Get("MsgRestoreAllSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復還原前檔案。");
                    rollback.RestoreAll(Log);
                    Log("檔案回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

        /// <summary>
        /// 僅恢復兵種屬性設定（如生命、防禦、冷卻、地圖人口等）為原版，保留相容性與語言設定。
        /// </summary>
        private async void RestoreStatsOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            if (!EnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreStats"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => {
                    RestoreStatsOnlyInternal(gamePath, rollback);
                    ApplyLeaderGloryKeepPatch(gamePath, false, rollback);
                    ApplyFoodHealingAmountPatch(gamePath, false, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkHousingCapacity20x.Checked = false; chkStorageCapacity10x.Checked = false;
                chkFastBuildUpgradeRepair.Checked = false;
                chkFoodHealing10x.Checked = false;
                chkLeaderGloryKeep.Checked = false;
                chkMaxPopulation.Checked = false;
                chkFastCiviProduction.Checked = false;
                chkFreeProd.Checked = false;
                chkFreeUpgrade.Checked = false;
                chkNoSpellCost.Checked = false;
                chkInfiniteMorale.Checked = false;
                chkBalance.Checked = false;
                chkModSkillsAndGlory.Checked = false;
                customUnitStats = null;
                presetFileSourceType = "default";
                presetFileName = "";
                UpdateTroopPresetLabel();
                LoadDefaultStatsData(); // 重新載入表格以呈現原版
                Log(Loc.Get("LogRestoreStatsDone"));
                MessageBox.Show(Loc.Get("MsgRestoreStatsSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復還原前檔案。");
                    rollback.RestoreAll(Log);
                    Log("檔案回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

        /// <summary>
        /// 僅恢復 Against_Rome.exe 等主程式為官方原版（即恢復失焦暫停與移除其他相容修正）。
        /// </summary>
        private async void RestoreCompatOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreCompat"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => {
                    var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

                    // EXE 還原
                    string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    bool exeModified = false;
                    ApplyExePatch(exeBytes, false, false, false, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // 無盡 AI 還原
                    var orchestrator = new EndlessAiOrchestrator();
                    foreach (var module in orchestrator.UserModules) {
                        orchestrator.ApplyModule(gamePath, module, false);
                    }
                    orchestrator.ApplyMandatoryRepair(gamePath);

                    // 統一寫入記憶體修改之檔案
                    foreach (var kvp in patchedFiles) {
                        SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
                    }
                    orchestrator.SaveAll(gamePath, rollback);
                    ApplyDgVoodooPatch(gamePath, false, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkFocusLoss.Checked = false;
                chkNoSpellAltar.Checked = false;
                chkAiM1.Checked = false; chkAiM2.Checked = false; chkAiM3.Checked = false; chkAiM4.Checked = false; chkAiM5.Checked = false;
                chkDgVoodoo.Checked = IsDgVoodooInstalled(gamePath);
                chkVillageBuildRange.Checked = false;
                Log(Loc.Get("LogRestoreCompatDone"));
                MessageBox.Show(Loc.Get("MsgRestoreCompatSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復還原前檔案。");
                    rollback.RestoreAll(Log);
                    Log("檔案回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

        /// <summary>
        /// 僅還原語言套件，讓介面與地圖語言變回遊戲原本安裝時的語系。
        /// </summary>
        private async void RestoreLanguageOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreLang"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => ApplyLanguagePatch(gamePath, false, rollback));
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkToEng.Checked = false;
                Log(Loc.Get("LogRestoreLangDone"));
                MessageBox.Show(Loc.Get("MsgRestoreLangSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復還原前檔案。");
                    rollback.RestoreAll(Log);
                    Log("檔案回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

        /// <summary>
        /// 僅還原兵種屬性設定，將 cl_script.ini、ress.ini、objdef.dau 以及所有地圖的 team.dat 覆寫回備份資料。
        /// </summary>
        private void RestoreStatsOnlyInternal(string gamePath, FileRollbackScope? rollback = null) {
            RestoreMemoryFile("SYSTEM/cl_script.ini", Path.Combine(gamePath, @"SYSTEM\cl_script.ini"), rollback);
            RestoreMemoryFile("SYSTEM/cl_epara.ini", Path.Combine(gamePath, @"SYSTEM\cl_epara.ini"), rollback);
            RestoreMemoryFile("SYSTEM/CLAK/cl_scint.ini", Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini"), rollback);
            RestoreMemoryFile("SYSTEM/ress.ini", Path.Combine(gamePath, @"SYSTEM\ress.ini"), rollback);
            RestoreMemoryFile("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau"), rollback);
            RestoreMemoryFile("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci", Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT\ak_anfuehrer.bci"), rollback);

            foreach (var kvp in backupFiles) {
                if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)) {
                    string destPath = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                    RestoreMemoryFile(kvp.Key, destPath, rollback);
                }
            }
        }

        /// <summary>
        /// 從記憶體備份字典中取出對應的 byte 陣列，寫入至指定的實體路徑。
        /// </summary>
        private void RestoreMemoryFile(string key, string dest, FileRollbackScope? rollback = null) {
            byte[]? fileBytes;
            if (backupFiles.TryGetValue(key, out fileBytes)) {
                SafeWriteAllBytes(dest, fileBytes!, rollback);
                Log(string.Format("已還原: {0}", dest));
            }
        }

        /// <summary>
        /// 當使用者點擊「啟動遊戲」按鈕時觸發，於後台啟動遊戲主程式。
        /// </summary>
        private void BtnStartGame_Click(object? sender, EventArgs e) {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            string exePath = Path.Combine(gamePath, "Against_Rome.exe");
            if (!File.Exists(exePath)) {
                MessageBox.Show(Loc.Get("MsgExeNotFound"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            try {
                System.Diagnostics.ProcessStartInfo psi = new System.Diagnostics.ProcessStartInfo {
                    FileName = exePath,
                    WorkingDirectory = gamePath
                };
                System.Diagnostics.Process.Start(psi);
                Log(Loc.Get("LogGameStarted"));
            } catch (Exception ex) {
                Log(Loc.Get("MsgLaunchFailed") + ex.Message);
                MessageBox.Show(Loc.Get("MsgLaunchFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 套用或還原英文介面與地圖語言包。
        /// </summary>
        private static string GetLanguageBackupDirectory(string gamePath) {
            return Path.Combine(gamePath, LanguageBackupDirectoryName);
        }

        private static string GetSafeLanguagePath(string rootPath, string relativePath) {
            if (string.IsNullOrWhiteSpace(relativePath) || Path.IsPathRooted(relativePath)) {
                throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
            }

            string root = Path.GetFullPath(rootPath).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
            string fullPath = Path.GetFullPath(Path.Combine(root, relativePath.Replace('/', Path.DirectorySeparatorChar)));
            if (!fullPath.StartsWith(root, StringComparison.OrdinalIgnoreCase)) {
                throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
            }
            return fullPath;
        }

        private static bool FilesAreEqual(string firstPath, string secondPath) {
            var firstInfo = new FileInfo(firstPath);
            var secondInfo = new FileInfo(secondPath);
            if (!firstInfo.Exists || !secondInfo.Exists || firstInfo.Length != secondInfo.Length) return false;

            const int bufferSize = 81920;
            byte[] firstBuffer = new byte[bufferSize];
            byte[] secondBuffer = new byte[bufferSize];
            using var first = new FileStream(firstPath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, FileOptions.SequentialScan);
            using var second = new FileStream(secondPath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, FileOptions.SequentialScan);
            while (true) {
                int firstRead = first.Read(firstBuffer, 0, firstBuffer.Length);
                int secondRead = second.Read(secondBuffer, 0, secondBuffer.Length);
                if (firstRead != secondRead) return false;
                if (firstRead == 0) return true;
                if (!firstBuffer.AsSpan(0, firstRead).SequenceEqual(secondBuffer.AsSpan(0, secondRead))) return false;
            }
        }

        private static bool TryGetLanguageOverlayState(string gamePath, out bool enabled) {
            enabled = false;
            string sourceRoot = Path.Combine(gamePath, "ToEng");
            if (!Directory.Exists(sourceRoot)) return false;
            string[] sourceFiles = Directory.GetFiles(sourceRoot, "*", SearchOption.AllDirectories);
            if (sourceFiles.Length == 0) return false;

            foreach (string sourcePath in sourceFiles) {
                string relativePath = Path.GetRelativePath(sourceRoot, sourcePath);
                string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                if (!FilesAreEqual(sourcePath, destinationPath)) return true;
            }
            enabled = true;
            return true;
        }

        private static void EnsureLanguageBackup(string gamePath, string sourceRoot, string[] sourceFiles) {
            string backupRoot = GetLanguageBackupDirectory(gamePath);
            string manifestPath = Path.Combine(backupRoot, LanguageBackupManifestName);
            if (File.Exists(manifestPath)) {
                LanguageBackupManifest? existingManifest = JsonSerializer.Deserialize<LanguageBackupManifest>(File.ReadAllText(manifestPath, Encoding.UTF8));
                if (existingManifest == null || existingManifest.ExistingFiles == null || existingManifest.MissingFiles == null) {
                    throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
                }
                var coveredPaths = new HashSet<string>(existingManifest.ExistingFiles.Concat(existingManifest.MissingFiles), StringComparer.OrdinalIgnoreCase);
                foreach (string sourcePath in sourceFiles) {
                    string relativePath = Path.GetRelativePath(sourceRoot, sourcePath).Replace('\\', '/');
                    if (!coveredPaths.Contains(relativePath)) throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
                }
                foreach (string relativePath in existingManifest.ExistingFiles) {
                    if (!File.Exists(GetSafeLanguagePath(Path.Combine(backupRoot, "files"), relativePath))) {
                        throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
                    }
                }
                return;
            }
            if (Directory.Exists(backupRoot)) {
                throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
            }
            if (TryGetLanguageOverlayState(gamePath, out bool alreadyEnabled) && alreadyEnabled) {
                throw new InvalidOperationException(Loc.Get("LogLanguageBackupMissing"));
            }

            string tempRoot = backupRoot + ".tmp-" + Guid.NewGuid().ToString("N");
            var manifest = new LanguageBackupManifest();
            try {
                foreach (string sourcePath in sourceFiles) {
                    string relativePath = Path.GetRelativePath(sourceRoot, sourcePath).Replace('\\', '/');
                    string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                    if (File.Exists(destinationPath)) {
                        string backupPath = GetSafeLanguagePath(Path.Combine(tempRoot, "files"), relativePath);
                        Directory.CreateDirectory(Path.GetDirectoryName(backupPath)!);
                        File.Copy(destinationPath, backupPath, true);
                        manifest.ExistingFiles.Add(relativePath);
                    } else {
                        manifest.MissingFiles.Add(relativePath);
                    }
                }

                Directory.CreateDirectory(tempRoot);
                File.WriteAllText(
                    Path.Combine(tempRoot, LanguageBackupManifestName),
                    JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }),
                    Encoding.UTF8);
                Directory.Move(tempRoot, backupRoot);
            } finally {
                if (Directory.Exists(tempRoot)) Directory.Delete(tempRoot, true);
            }
        }

        private void RestoreLanguageBackup(string gamePath, FileRollbackScope? rollback) {
            string backupRoot = GetLanguageBackupDirectory(gamePath);
            string manifestPath = Path.Combine(backupRoot, LanguageBackupManifestName);
            if (!File.Exists(manifestPath)) {
                throw new InvalidOperationException(Loc.Get("LogLanguageBackupMissing"));
            }

            LanguageBackupManifest? manifest = JsonSerializer.Deserialize<LanguageBackupManifest>(File.ReadAllText(manifestPath, Encoding.UTF8));
            if (manifest == null || manifest.ExistingFiles == null || manifest.MissingFiles == null) {
                throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
            }

            foreach (string relativePath in manifest.ExistingFiles) {
                string backupPath = GetSafeLanguagePath(Path.Combine(backupRoot, "files"), relativePath);
                if (!File.Exists(backupPath)) throw new InvalidDataException(Loc.Get("LogLanguageBackupInvalid"));
                SafeCopyFile(backupPath, GetSafeLanguagePath(gamePath, relativePath), true, rollback);
            }
            foreach (string relativePath in manifest.MissingFiles) {
                string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                if (!File.Exists(destinationPath)) continue;
                rollback?.TrackFile(destinationPath);
                File.SetAttributes(destinationPath, FileAttributes.Normal);
                File.Delete(destinationPath);
            }
        }

        private void ApplyLanguagePatch(string gamePath, bool toEnglish, FileRollbackScope? rollback = null) {
            string localToEngDir = Path.Combine(gamePath, "ToEng");

            if (toEnglish) {
                if (!Directory.Exists(localToEngDir)) {
                    Log(Loc.Get("LogNoToEngDir"));
                    throw new DirectoryNotFoundException(Loc.Get("LogNoToEngDir"));
                }

                string[] files = Directory.GetFiles(localToEngDir, "*", SearchOption.AllDirectories);
                if (files.Length == 0) {
                    throw new InvalidDataException(Loc.Get("LogNoToEngDir"));
                }
                EnsureLanguageBackup(gamePath, localToEngDir, files);
                foreach (string file in files) {
                    string relPath = Path.GetRelativePath(localToEngDir, file);
                    string destPath = GetSafeLanguagePath(gamePath, relPath);
                    SafeCopyFile(file, destPath, true, rollback);
                }
                Log(Loc.Get("LogLangToEng"));
                return;
            }

            string languageManifestPath = Path.Combine(GetLanguageBackupDirectory(gamePath), LanguageBackupManifestName);
            if (!File.Exists(languageManifestPath)) {
                if (TryGetLanguageOverlayState(gamePath, out bool overlayEnabled) && overlayEnabled) {
                    throw new InvalidOperationException(Loc.Get("LogLanguageBackupMissing"));
                }
                return;
            }
            RestoreLanguageBackup(gamePath, rollback);
            Log(Loc.Get("LogLangToOrig"));
        }

        private enum ExePatchState {
            Unknown,
            Original,
            FocusPatched
        }

        private enum ExeSpellAltarPatchState {
            Unknown,
            Original,
            Patched
        }

        private enum ExeVillageRangePatchState {
            Unknown,
            Original,
            LegacyLogicOnly,
            Expanded
        }

        private enum ExeVillageSetterPatchState {
            Unknown,
            Original,
            Legacy2x,
            Legacy2Point5x,
            Expanded3x
        }

        private ExePatchState GetExePatchState(byte[] exeBytes) {
            if (exeBytes.Length < ExeFocusPatchRequiredLength) {
                return ExePatchState.Unknown;
            }
            byte[] bytes = new byte[ExeFocusOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeFocusPatchOffset, bytes, 0, bytes.Length);
            if (bytes.SequenceEqual(ExeFocusOriginalBytes)) return ExePatchState.Original;
            if (bytes.SequenceEqual(ExeFocusPatchedBytes)) return ExePatchState.FocusPatched;
            return ExePatchState.Unknown;
        }

        private ExeSpellAltarPatchState GetSpellAltarPatchState(byte[] exeBytes) {
            bool allOriginal = true;
            bool allPatched = true;

            foreach (var site in SpellAltarPatchSites) {
                if (exeBytes.Length < site.Offset + site.Original.Length) {
                    return ExeSpellAltarPatchState.Unknown;
                }
                byte[] current = new byte[site.Original.Length];
                Buffer.BlockCopy(exeBytes, (int)site.Offset, current, 0, current.Length);

                if (!current.SequenceEqual(site.Original)) {
                    allOriginal = false;
                }
                if (!current.SequenceEqual(site.Patched)) {
                    allPatched = false;
                }
            }

            if (allOriginal) return ExeSpellAltarPatchState.Original;
            if (allPatched) return ExeSpellAltarPatchState.Patched;
            return ExeSpellAltarPatchState.Unknown;
        }

        private void ApplySpellAltarPatch(byte[] exeBytes, bool noAltarChecked, ref bool exeModified) {
            ExeSpellAltarPatchState state = GetSpellAltarPatchState(exeBytes);
            if (state == ExeSpellAltarPatchState.Unknown) {
                throw new Exception("Against_Rome.exe 版本或法術祭壇特徵碼不符合預期，已停止套用法術祭壇補丁。");
            }

            if (noAltarChecked) {
                if (state == ExeSpellAltarPatchState.Original) {
                    foreach (var site in SpellAltarPatchSites) {
                        WriteExeBytesInMemory(exeBytes, site.Offset, site.Patched);
                    }
                    exeModified = true;
                    Log("已套用法術免祭壇需求補丁。");
                }
            } else {
                if (state == ExeSpellAltarPatchState.Patched) {
                    foreach (var site in SpellAltarPatchSites) {
                        WriteExeBytesInMemory(exeBytes, site.Offset, site.Original);
                    }
                    exeModified = true;
                    Log("已還原法術祭壇需求設定。");
                }
            }
        }

        private static void WriteExeBytesInMemory(byte[] exeBytes, long patchOffset, byte[] patchBytes) {
            Buffer.BlockCopy(patchBytes, 0, exeBytes, (int)patchOffset, patchBytes.Length);
        }

        private ExeVillageRangePatchState GetVillageBuildRangePatchState(byte[] exeBytes) {
            if (exeBytes.Length < ExeVillageRangePatchRequiredLength) {
                return ExeVillageRangePatchState.Unknown;
            }

            byte[] xBytes = new byte[ExeVillageRangeXOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeVillageRangeXPatchOffset, xBytes, 0, xBytes.Length);

            byte[] zBytes = new byte[ExeVillageRangeZOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeVillageRangeZPatchOffset, zBytes, 0, zBytes.Length);

            byte[] frameXBytes = new byte[ExeVillageFrameXOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeVillageFrameXPatchOffset, frameXBytes, 0, frameXBytes.Length);

            byte[] frameZBytes = new byte[ExeVillageFrameZOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeVillageFrameZPatchOffset, frameZBytes, 0, frameZBytes.Length);

            bool original = xBytes.SequenceEqual(ExeVillageRangeXOriginalBytes) &&
                zBytes.SequenceEqual(ExeVillageRangeZOriginalBytes) &&
                frameXBytes.SequenceEqual(ExeVillageFrameXOriginalBytes) &&
                frameZBytes.SequenceEqual(ExeVillageFrameZOriginalBytes);
            bool legacyLogicOnly = xBytes.SequenceEqual(ExeVillageRangeXPatchedBytes) &&
                zBytes.SequenceEqual(ExeVillageRangeZPatchedBytes) &&
                frameXBytes.SequenceEqual(ExeVillageFrameXOriginalBytes) &&
                frameZBytes.SequenceEqual(ExeVillageFrameZOriginalBytes);
            bool expanded = xBytes.SequenceEqual(ExeVillageRangeXPatchedBytes) &&
                zBytes.SequenceEqual(ExeVillageRangeZPatchedBytes) &&
                frameXBytes.SequenceEqual(ExeVillageFrameXPatchedBytes) &&
                frameZBytes.SequenceEqual(ExeVillageFrameZPatchedBytes);
            if (original) return ExeVillageRangePatchState.Original;
            if (legacyLogicOnly) return ExeVillageRangePatchState.LegacyLogicOnly;
            if (expanded) return ExeVillageRangePatchState.Expanded;
            return ExeVillageRangePatchState.Unknown;
        }

        private ExeVillageSetterPatchState GetVillageSetterPatchState(byte[] exeBytes) {
            if (exeBytes.Length < ExeVillageSetterPatchRequiredLength) {
                return ExeVillageSetterPatchState.Unknown;
            }

            byte[] hookBytes = new byte[ExeVillageSetterHookOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeVillageSetterHookOffset, hookBytes, 0, hookBytes.Length);

            byte[] caveBytes = new byte[ExeVillageSetterCaveOriginalBytes.Length];
            Buffer.BlockCopy(exeBytes, (int)ExeVillageSetterCaveOffset, caveBytes, 0, caveBytes.Length);

            bool original = hookBytes.SequenceEqual(ExeVillageSetterHookOriginalBytes) &&
                caveBytes.SequenceEqual(ExeVillageSetterCaveOriginalBytes);
            bool legacy2x = hookBytes.SequenceEqual(ExeVillageSetterHookPatchedBytes) &&
                caveBytes.SequenceEqual(ExeVillageSetterCaveLegacy2xBytes);
            bool legacy2Point5x = hookBytes.SequenceEqual(ExeVillageSetterHookPatchedBytes) &&
                caveBytes.SequenceEqual(ExeVillageSetterCaveLegacy2Point5xBytes);
            bool expanded3x = hookBytes.SequenceEqual(ExeVillageSetterHookPatchedBytes) &&
                caveBytes.SequenceEqual(ExeVillageSetterCavePatchedBytes);
            if (original) return ExeVillageSetterPatchState.Original;
            if (legacy2x) return ExeVillageSetterPatchState.Legacy2x;
            if (legacy2Point5x) return ExeVillageSetterPatchState.Legacy2Point5x;
            if (expanded3x) return ExeVillageSetterPatchState.Expanded3x;
            return ExeVillageSetterPatchState.Unknown;
        }

        private void RestoreLegacyVillageBuildRangePatch(byte[] exeBytes, ref bool exeModified) {
            ExeVillageRangePatchState state = GetVillageBuildRangePatchState(exeBytes);
            if (state == ExeVillageRangePatchState.Unknown) {
                Log(Loc.Get("LogVillageBuildRangeWarning"));
                return;
            }

            if (state == ExeVillageRangePatchState.Expanded ||
                state == ExeVillageRangePatchState.LegacyLogicOnly) {
                WriteExeBytesInMemory(exeBytes, ExeVillageRangeXPatchOffset, ExeVillageRangeXOriginalBytes);
                WriteExeBytesInMemory(exeBytes, ExeVillageRangeZPatchOffset, ExeVillageRangeZOriginalBytes);
                WriteExeBytesInMemory(exeBytes, ExeVillageFrameXPatchOffset, ExeVillageFrameXOriginalBytes);
                WriteExeBytesInMemory(exeBytes, ExeVillageFrameZPatchOffset, ExeVillageFrameZOriginalBytes);
                exeModified = true;
                Log(Loc.Get("LogVillageBuildRangeRestored"));
            }
        }

        private void ApplyVillageSetterRangePatch(byte[] exeBytes, bool enabled, ref bool exeModified) {
            ExeVillageSetterPatchState state = GetVillageSetterPatchState(exeBytes);
            if (state == ExeVillageSetterPatchState.Unknown) {
                if (enabled) {
                    throw new InvalidOperationException(Loc.Get("LogVillageBuildRangeWarning"));
                }
                Log(Loc.Get("LogVillageBuildRangeWarning"));
                return;
            }

            if (enabled) {
                if (state == ExeVillageSetterPatchState.Original ||
                    state == ExeVillageSetterPatchState.Legacy2x ||
                    state == ExeVillageSetterPatchState.Legacy2Point5x) {
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterCaveOffset, ExeVillageSetterCavePatchedBytes);
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterHookOffset, ExeVillageSetterHookPatchedBytes);
                    exeModified = true;
                }
                Log(Loc.Get("LogVillageBuildRangeApplied"));
            } else {
                if (state == ExeVillageSetterPatchState.Legacy2x ||
                    state == ExeVillageSetterPatchState.Legacy2Point5x ||
                    state == ExeVillageSetterPatchState.Expanded3x) {
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterHookOffset, ExeVillageSetterHookOriginalBytes);
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterCaveOffset, ExeVillageSetterCaveOriginalBytes);
                    exeModified = true;
                    Log(Loc.Get("LogVillageBuildRangeSetterRestored"));
                }
            }
        }

        private void ApplyExePatch(byte[] exeBytes, bool focusLossChecked, bool villageBuildRangeChecked, bool noSpellAltarChecked, ref bool exeModified) {
            ExePatchState state = GetExePatchState(exeBytes);
            if (state == ExePatchState.Unknown) {
                throw new Exception("Against_Rome.exe 版本或位元組特徵不符合預期，已停止相容性補丁以避免覆蓋未知版本。");
            }

            if (focusLossChecked) {
                if (state == ExePatchState.Original) {
                    WriteExeBytesInMemory(exeBytes, ExeFocusPatchOffset, ExeFocusPatchedBytes);
                    exeModified = true;
                }
                Log(Loc.Get("LogExePatchFocus"));
            } else {
                if (state == ExePatchState.FocusPatched) {
                    WriteExeBytesInMemory(exeBytes, ExeFocusPatchOffset, ExeFocusOriginalBytes);
                    exeModified = true;
                }
                Log(Loc.Get("LogExePatchOrig"));
            }

            RestoreLegacyVillageBuildRangePatch(exeBytes, ref exeModified);
            if (villageBuildRangeChecked &&
                GetVillageBuildRangePatchState(exeBytes) != ExeVillageRangePatchState.Original) {
                throw new InvalidOperationException(Loc.Get("LogVillageBuildRangeWarning"));
            }
            ApplyVillageSetterRangePatch(exeBytes, villageBuildRangeChecked, ref exeModified);

            ApplySpellAltarPatch(exeBytes, noSpellAltarChecked, ref exeModified);
        }

        /// <summary>
        /// 修改 cl_script.ini 檔案，自訂村民產生速度、法術影響半徑以及無限士氣等功能。
        /// </summary>
        private static HashSet<string> GetClScriptManagedKeys(string text) {
            var keys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            string[] lines = text.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            foreach (string line in lines) {
                Match radius = RegexRadiusPatch.Match(line);
                if (radius.Success) {
                    keys.Add($"Radius|{radius.Groups[1].Value.Trim()}|{radius.Groups[2].Value.Trim()}");
                    continue;
                }

                Match civi = RegexCiviPatch.Match(line);
                if (civi.Success) {
                    keys.Add($"CiviDelay|{civi.Groups[1].Value.Trim()}");
                    continue;
                }

                Match morale = RegexMoraleKey.Match(line);
                if (morale.Success) {
                    keys.Add($"{morale.Groups[1].Value.Trim()}|{morale.Groups[2].Value.Trim()}");
                }

                Match spellVal = RegexSpellValuePatch.Match(line);
                if (spellVal.Success) {
                    keys.Add($"SpellValue|{spellVal.Groups[1].Value.Trim()}|{spellVal.Groups[2].Value.Trim()}|{spellVal.Groups[3].Value.Trim()}");
                }

                Match specVal = RegexSpecialAbilityValuePatch.Match(line);
                if (specVal.Success) {
                    keys.Add($"SpecialAbilityValue|{specVal.Groups[1].Value.Trim()}|{specVal.Groups[2].Value.Trim()}|{specVal.Groups[3].Value.Trim()}");
                }
            }
            return keys;
        }

        private byte[] GetPatchedClScriptBytes(string gamePath, bool fastCiviProduction, bool infiniteMoraleChecked, bool balanceChecked, bool spellEnhancementChecked, Dictionary<string, double> generalSkills) {
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/cl_script.ini", out origBytes)) {
                throw new InvalidOperationException("記憶體備份中找不到 SYSTEM/cl_script.ini。");
            }

            // 1. 建立原版備份中的 Radius 對照字典，以防多次套用導致數值累乘
            var originalRadiuses = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
            var originalSpellValues = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
            var originalSpecialAbilityValues = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
            byte[] origDecomp = GameLZSS.DecompressPfil(origBytes!);
            string origText = Encoding.GetEncoding(1251).GetString(origDecomp);
            string[] origLines = origText.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            HashSet<string> originalManagedKeys = GetClScriptManagedKeys(origText);
            if (originalManagedKeys.Count == 0) {
                throw new InvalidDataException("備份中的 SYSTEM/cl_script.ini 缺少可辨識的受管理設定。");
            }
            foreach (string line in origLines) {
                var match = RegexRadiusPatch.Match(line);
                if (match.Success) {
                    string volk = match.Groups[1].Value.Trim();
                    string spell = match.Groups[2].Value.Trim();
                    string valStr = match.Groups[3].Value.Trim();
                    if (double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out double val)) {
                        originalRadiuses[$"{volk}_{spell}"] = val;
                    }
                }
                var mVal = RegexSpellValuePatch.Match(line);
                if (mVal.Success) {
                    string keyName = mVal.Groups[1].Value.Trim();
                    string volk = mVal.Groups[2].Value.Trim();
                    string spell = mVal.Groups[3].Value.Trim();
                    string valStr = mVal.Groups[4].Value.Trim();
                    if (double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out double val)) {
                        originalSpellValues[$"{keyName}_{volk}_{spell}"] = val;
                    }
                }
                var mSpec = RegexSpecialAbilityValuePatch.Match(line);
                if (mSpec.Success) {
                    string keyName = mSpec.Groups[1].Value.Trim();
                    string volk = mSpec.Groups[2].Value.Trim();
                    string ability = mSpec.Groups[3].Value.Trim();
                    string valStr = mSpec.Groups[4].Value.Trim();
                    if (double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out double val)) {
                        originalSpecialAbilityValues[$"{keyName}_{volk}_{ability}"] = val;
                    }
                }
            }

            // 2. 優先讀取遊戲目錄下現有的 cl_script.ini 作為修改基底（增量修改）
            byte[] baseBytes = origBytes;
            string destPath = Path.Combine(gamePath, @"SYSTEM\cl_script.ini");
            if (File.Exists(destPath)) {
                try {
                    byte[] currentBytes = File.ReadAllBytes(destPath);
                    byte[] currentDecomp = GameLZSS.DecompressPfil(currentBytes);
                    string currentText = Encoding.GetEncoding(1251).GetString(currentDecomp);
                    HashSet<string> currentManagedKeys = GetClScriptManagedKeys(currentText);
                    if (originalManagedKeys.IsSubsetOf(currentManagedKeys)) {
                        baseBytes = currentBytes;
                    } else {
                        Log("現有 cl_script.ini 結構不完整，已改用安全備份作為修改基底。");
                    }
                } catch (Exception ex) {
                    Log("現有 cl_script.ini 無法驗證，已改用安全備份作為修改基底: " + ex.Message);
                }
            }

            byte[] decompBytes = GameLZSS.DecompressPfil(baseBytes);
            string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
            string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);

            double gerMult = 1.0;
            double kelMult = balanceChecked ? 2.5 : 1.0;
            double hunMult = balanceChecked ? 2.5 : 1.0;

            if (customUnitStats != null) {
                if (customUnitStats.ContainsKey("FigKelPri00_Priester") && customUnitStats["FigKelPri00_Priester"].Length > 8) {
                    kelMult = customUnitStats["FigKelPri00_Priester"][8] / 500.0;
                }
                if (customUnitStats.ContainsKey("FigHunPri00_Priester") && customUnitStats["FigHunPri00_Priester"].Length > 8) {
                    hunMult = customUnitStats["FigHunPri00_Priester"][8] / 500.0;
                }
            }

            var newLines = new List<string>();
            foreach (string line in lines) {
                string processedLine = line;
                var match = RegexRadiusPatch.Match(line);
                if (match.Success) {
                    string volk = match.Groups[1].Value;
                    string spell = match.Groups[2].Value;
                    string valStr = match.Groups[3].Value.Trim();
                    string comment = match.Groups[4].Value;

                    string volkClean = volk.Trim();
                    string spellClean = spell.Trim();
                    string dictKey = $"{volkClean}_{spellClean}";

                    // 優先使用原版備份對照，避免多次修改累乘
                    double val = 0;
                    if (originalRadiuses.TryGetValue(dictKey, out double origVal)) {
                        val = origVal;
                    } else {
                        double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out val);
                    }

                    double mult = 1.0;
                    if (volkClean == "GER") mult = gerMult;
                    else if (volkClean == "KEL") mult = kelMult;
                    else if (volkClean == "HUN") mult = hunMult;

                    int newVal = (int)(val * mult);
                    processedLine = string.Format("Radius     ={0}, {1}, {2,-10}{3}", volk, spell, newVal, comment);
                }

                var matchSpellVal = RegexSpellValuePatch.Match(line);
                if (matchSpellVal.Success) {
                    string keyName = matchSpellVal.Groups[1].Value.Trim();
                    string volk = matchSpellVal.Groups[2].Value.Trim();
                    string spell = matchSpellVal.Groups[3].Value.Trim();
                    string valStr = matchSpellVal.Groups[4].Value.Trim();
                    string comment = matchSpellVal.Groups[5].Value;

                    string dictKey = $"{keyName}_{volk}_{spell}";
                    double origVal = 0;
                    if (!originalSpellValues.TryGetValue(dictKey, out origVal)) {
                        double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out origVal);
                    }

                    int newVal = (int)origVal;
                    if (spellEnhancementChecked) {
                        // Apply multiplier: 5x for damage, 50x for healing
                        if (volk == "GER" && spell == "Spell2" && keyName == "Value") newVal = (int)(origVal * 5);
                        else if (volk == "HUN" && spell == "Spell0" && keyName == "Value") newVal = (int)(origVal * 5);
                        else if (volk == "HUN" && spell == "Spell1" && keyName == "Value") newVal = (int)(origVal * 5);
                        else if (volk == "HUN" && spell == "Spell2" && keyName == "Value") newVal = (int)(origVal * 5);
                        else if (volk == "KEL" && spell == "Spell0" && keyName == "Value") newVal = (int)(origVal * 5);
                        else if (volk == "KEL" && spell == "Spell2" && keyName == "Value") newVal = (int)(origVal * 5);
                        else if (volk == "KEL" && spell == "Spell1" && keyName == "Value") newVal = (int)(origVal * 50);
                        else if (volk == "KEL" && spell == "Spell3" && (keyName == "Value" || keyName == "Value2")) newVal = 100;
                    }

                    processedLine = string.Format("{0,-10} ={1}, {2}, {3,-10}{4}", keyName, volk, spell, newVal, comment);
                }

                var matchSpecVal = RegexSpecialAbilityValuePatch.Match(line);
                if (matchSpecVal.Success) {
                    string keyName = matchSpecVal.Groups[1].Value.Trim();
                    string volk = matchSpecVal.Groups[2].Value.Trim();
                    string ability = matchSpecVal.Groups[3].Value.Trim();
                    string valStr = matchSpecVal.Groups[4].Value.Trim();
                    string comment = matchSpecVal.Groups[5].Value;

                    string dictKey = $"{volk}_{ability}_Value";
                    double targetVal = 0;
                    if (generalSkills != null && generalSkills.TryGetValue(dictKey, out double uiVal)) {
                        targetVal = uiVal;
                    } else {
                        string cacheKey = $"{keyName}_{volk}_{ability}";
                        if (!originalSpecialAbilityValues.TryGetValue(cacheKey, out targetVal)) {
                            double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out targetVal);
                        }
                    }
                    processedLine = string.Format("{0,-10} ={1}, {2}, {3,-10}{4}", keyName, volk, ability, (int)targetVal, comment);
                }

                var matchCivi = RegexCiviPatch.Match(line);
                if (matchCivi.Success) {
                    string volk = matchCivi.Groups[1].Value;
                    string comment = matchCivi.Groups[3].Value;
                    if (fastCiviProduction) {
                        processedLine = string.Format("CiviDelay  ={0}, {1,-10}{2}", volk, 500, comment);
                    } else {
                        // 若未開啟，則從原版備份中還原該陣營的原始延遲
                        string volkClean = volk.Trim();
                        double origDelay = 5000; // 安全 fallback
                        string? origLine = origLines.FirstOrDefault(l => RegexCiviPatch.Match(l).Success && RegexCiviPatch.Match(l).Groups[1].Value.Trim() == volkClean);
                        if (origLine != null) {
                            var m = RegexCiviPatch.Match(origLine);
                            if (double.TryParse(m.Groups[2].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double d)) {
                                origDelay = d;
                            }
                        }
                        processedLine = string.Format("CiviDelay  ={0}, {1,-10}{2}", volk, (int)origDelay, comment);
                    }
                }

                // LPIncIdle 的「頻率 x10」機制已由「單次加血量 x10」（Fig* AI 腳本
                // s_addLP 字面值補丁，見 ApplyFoodHealingAmountPatch）取代。
                // 這裡一律還原為原版間隔，讓舊版套用過的安裝自動遷移回原版。
                var matchLpIdle = RegexLpIncIdlePatch.Match(line);
                if (matchLpIdle.Success) {
                    string volk = matchLpIdle.Groups[1].Value;
                    string comment = matchLpIdle.Groups[3].Value;
                    string volkClean = volk.Trim();
                    double origInterval = 15000; // 安全 fallback（原版四陣營皆 15000）
                    string? origLine = origLines.FirstOrDefault(l => RegexLpIncIdlePatch.Match(l).Success && RegexLpIncIdlePatch.Match(l).Groups[1].Value.Trim() == volkClean);
                    if (origLine != null) {
                        var m = RegexLpIncIdlePatch.Match(origLine);
                        if (double.TryParse(m.Groups[2].Value.Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double d)) {
                            origInterval = d;
                        }
                    }
                    processedLine = string.Format("LPIncIdle       ={0}, {1,-10}{2}", volk, (int)origInterval, comment);
                }

                if (infiniteMoraleChecked) {
                    if (line.StartsWith("MoralsDecLostMem")) {
                        var m = RegexMoraleLostMemPatch.Match(line);
                        if (m.Success) processedLine = m.Groups[1].Value + "0" + m.Groups[2].Value;
                    } else if (line.StartsWith("MoralsDecFlee")) {
                        var m = RegexMoraleFleePatch.Match(line);
                        if (m.Success) processedLine = m.Groups[1].Value + "0" + m.Groups[2].Value;
                    } else if (line.StartsWith("MoralsDecOverPop")) {
                        var m = RegexMoraleOverPopPatch.Match(line);
                        if (m.Success) processedLine = m.Groups[1].Value + "99999999" + m.Groups[2].Value;
                    } else if (line.StartsWith("MoralsIncIdle")) {
                        var m = RegexMoraleIncIdlePatch.Match(line);
                        if (m.Success) processedLine = m.Groups[1].Value + "500" + m.Groups[2].Value;
                    }
                } else {
                    // 若未開啟，則從原版備份中還原士氣參數
                    Match moraleKey = RegexMoraleKey.Match(line);
                    if (moraleKey.Success) {
                        string setting = moraleKey.Groups[1].Value;
                        string faction = moraleKey.Groups[2].Value;
                        string? origLine = origLines.FirstOrDefault(originalLine => {
                            Match originalKey = RegexMoraleKey.Match(originalLine);
                            return originalKey.Success &&
                                originalKey.Groups[1].Value.Equals(setting, StringComparison.OrdinalIgnoreCase) &&
                                originalKey.Groups[2].Value.Equals(faction, StringComparison.OrdinalIgnoreCase);
                        });
                        if (origLine != null) {
                            processedLine = origLine;
                        }
                    }
                }

                newLines.Add(processedLine);
            }

            string newContent = string.Join(lineEnding, newLines.ToArray());
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            return GameLZSS.CompressPfil(newBytes, origBytes);
        }

        private byte[] GetPatchedClEparaBytes(string gamePath, Dictionary<string, double> generalSkills) {
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/cl_epara.ini", out origBytes)) {
                throw new InvalidOperationException("記憶體備份中找不到 SYSTEM/cl_epara.ini。");
            }

            byte[] baseBytes = origBytes;
            string destPath = Path.Combine(gamePath, @"SYSTEM\cl_epara.ini");
            if (File.Exists(destPath)) {
                try {
                    baseBytes = File.ReadAllBytes(destPath);
                } catch { }
            }

            byte[] decompBytes = GameLZSS.DecompressPfil(baseBytes);
            string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
            string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);

            for (int i = 0; i < lines.Length; i++) {
                string trimmedLine = lines[i].Trim();
                if (trimmedLine.StartsWith("[") && trimmedLine.EndsWith("]")) {
                    string key = trimmedLine.Substring(1, trimmedLine.Length - 2).Trim();
                    if (generalSkills.TryGetValue(key, out double val)) {
                        int valIdx = i + 1;
                        while (valIdx < lines.Length && (string.IsNullOrWhiteSpace(lines[valIdx]) || lines[valIdx].Trim().StartsWith(";"))) {
                            valIdx++;
                        }
                        if (valIdx < lines.Length) {
                            lines[valIdx] = val.ToString("0.##", CultureInfo.InvariantCulture);
                        }
                    }
                }
            }

            string newContent = string.Join(lineEnding, lines);
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            return GameLZSS.CompressPfil(newBytes, origBytes);
        }

        private byte[] GetPatchedClScintBytes(string gamePath, bool spellEnhancementChecked) {
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/CLAK/cl_scint.ini", out origBytes)) {
                throw new InvalidOperationException("記憶體備份中找不到 SYSTEM/CLAK/cl_scint.ini。");
            }

            var originalODefs = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            byte[] origDecomp = GameLZSS.DecompressPfil(origBytes!);
            string origText = Encoding.GetEncoding(1251).GetString(origDecomp);
            string[] origLines = origText.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            
            foreach (string line in origLines) {
                var match = RegexSpellODefPatch.Match(line);
                if (match.Success) {
                    string keyName = match.Groups[1].Value.Trim();
                    string valStr = match.Groups[4].Value.Trim();
                    originalODefs[keyName] = valStr;
                }
            }

            byte[] baseBytes = origBytes;
            string destPath = Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini");
            if (File.Exists(destPath)) {
                try {
                    byte[] currentBytes = File.ReadAllBytes(destPath);
                    byte[] currentDecomp = GameLZSS.DecompressPfil(currentBytes);
                    string currentText = Encoding.GetEncoding(1251).GetString(currentDecomp);
                    if (currentText.Contains("SpellODef") && currentText.Contains("KEL, Spell3")) {
                        baseBytes = currentBytes;
                    } else {
                        Log("現有 cl_scint.ini 結構不完整，已改用安全備份作為修改基底。");
                    }
                } catch (Exception ex) {
                    Log("現有 cl_scint.ini 無法驗證，已改用安全備份作為修改基底: " + ex.Message);
                }
            }

            byte[] decompBytes = GameLZSS.DecompressPfil(baseBytes);
            string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
            string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);

            var newLines = new List<string>();
            foreach (string line in lines) {
                string processedLine = line;
                var match = RegexSpellODefPatch.Match(line);
                if (match.Success) {
                    string keyName = match.Groups[1].Value.Trim();
                    string volk = match.Groups[2].Value.Trim();
                    string spell = match.Groups[3].Value.Trim();
                    string valStr = match.Groups[4].Value.Trim();
                    string comment = match.Groups[5].Value;

                    string? origVal = "";
                    if (!originalODefs.TryGetValue(keyName, out origVal)) {
                        origVal = valStr;
                    }

                    string newVal = origVal;
                    if (spellEnhancementChecked) {
                        if (keyName == "SpellODef") {
                            newVal = "KEL_INF01";
                        } else if (keyName == "SpellODef2") {
                            newVal = "KEL_INF02";
                        }
                    }

                    processedLine = string.Format("{0,-10}={1}, {2}, {3,-12}{4}", keyName, volk, spell, newVal, comment);
                }
                newLines.Add(processedLine);
            }

            string newContent = string.Join(lineEnding, newLines.ToArray());
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            return GameLZSS.CompressPfil(newBytes, origBytes);
        }

        /// <summary>
        /// 修改 ress.ini 檔案，設定建築/部隊生產與升級的免費資源，以及移除祭司施法冷卻/消耗。
        /// </summary>
        private byte[] GetPatchedRessBytes(bool freeProdChecked, bool freeUpgradeChecked, bool noSpellCostChecked) {
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/ress.ini", out origBytes)) {
                throw new InvalidOperationException("記憶體備份中找不到 SYSTEM/ress.ini。");
            }

            byte[] decompBytes = GameLZSS.DecompressPfil(origBytes!);
            string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
            string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);

            var newLines = new List<string>();
            bool inObjres = false;
            bool inVolkres = false;

            foreach (string line in lines) {
                string stripped = line.Trim();
                if (stripped.StartsWith("[")) {
                    if (stripped == "[objres]") {
                        inObjres = true;
                        inVolkres = false;
                    } else if (stripped == "[volkres]") {
                        inObjres = false;
                        inVolkres = true;
                    } else {
                        inObjres = false;
                        inVolkres = false;
                    }
                    newLines.Add(line);
                    continue;
                }
                if (inObjres && line.Contains(",")) {
                    string[] cols = ParseCsvLine(line);
                    if (cols.Length > 0) {
                        string name = cols[0].Trim();
                        if (name.StartsWith("Bau")) {
                            if (cols.Length < 10) {
                                newLines.Add(line);
                                continue;
                            }
                            var newCols = new List<string> { cols[0] };
                            for (int i = 1; i < cols.Length; i++) {
                                if (string.IsNullOrEmpty(cols[i].Trim())) {
                                    newCols.Add(cols[i]);
                                } else if (freeUpgradeChecked &&
                                    i >= (int)RessIndex.BauUpgradeCostStart &&
                                    i <= (int)RessIndex.BauUpgradeCostEnd) {
                                    newCols.Add("0");
                                } else if (freeProdChecked &&
                                    i >= (int)RessIndex.BauBuildCostStart &&
                                    i <= (int)RessIndex.BauBuildCostEnd) {
                                    newCols.Add("0");
                                } else {
                                    newCols.Add(cols[i]);
                                }
                            }
                            newLines.Add(ToCsvString(newCols.ToArray()));
                        } else if (name.StartsWith("Fig")) {
                            if (cols.Length < 29) {
                                newLines.Add(line);
                                continue;
                            }
                            if (name.Equals("FigTiePac00_Packpferd", StringComparison.OrdinalIgnoreCase)) {
                                newLines.Add(line);
                                continue;
                            }
                            bool isSiegeTrap = name.Contains("Art") || name.Contains("Bar") || name.Contains("Fal");
                            var newCols = new List<string> { cols[0] };
                            bool isPriest = name.Contains("Pri") || name.Contains("Dru");
                            for (int i = 1; i < cols.Length; i++) {
                                string val = cols[i].Trim();
                                if (string.IsNullOrEmpty(val)) {
                                    newCols.Add(cols[i]);
                                } else if (freeProdChecked && ShouldZeroFigFreeProductionField(i)) {
                                    newCols.Add("0");
                                } else if (isSiegeTrap && i >= (int)RessIndex.FigSiegeBuildCostStart && i <= (int)RessIndex.FigSiegeBuildCostEnd) {
                                    newCols.Add(freeProdChecked ? "0" : val);
                                } else if (isPriest && i >= (int)RessIndex.FigPriestSpellCostStart && i <= (int)RessIndex.FigPriestSpellCostEnd) {
                                    newCols.Add(noSpellCostChecked ? "0" : val);
                                } else {
                                    newCols.Add(cols[i]);
                                }
                            }
                            newLines.Add(ToCsvString(newCols.ToArray()));
                        } else {
                            newLines.Add(line);
                        }
                    } else {
                        newLines.Add(line);
                    }
                } else if (inVolkres && line.Contains(",")) {
                    string[] cols = ParseCsvLine(line);
                    if (cols.Length < 3) {
                        newLines.Add(line);
                        continue;
                    }
                    var newCols = new List<string>();
                    for (int i = 0; i < cols.Length; i++) {
                        if (freeUpgradeChecked && (
                            i == (int)VolkresIndex.ResearchUpgradeWood1 ||
                            i == (int)VolkresIndex.ResearchUpgradeGold1 ||
                            i == (int)VolkresIndex.ResearchUpgradeWood2 ||
                            i == (int)VolkresIndex.ResearchUpgradeGold2 ||
                            (i >= (int)VolkresIndex.TechCostStart && i <= (int)VolkresIndex.TechCostEnd && i % 2 == 0) ||
                            (i >= (int)VolkresIndex.UnitUpgradeStart && i <= (int)VolkresIndex.UnitUpgradeEnd)
                        )) {
                            newCols.Add("0");
                        } else {
                            newCols.Add(cols[i]);
                        }
                    }
                    newLines.Add(ToCsvString(newCols.ToArray()));
                } else {
                    newLines.Add(line);
                }
            }

            string newContent = string.Join(lineEnding, newLines.ToArray());
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            return GameLZSS.CompressPfil(newBytes, origBytes);
        }

        /// <summary>
        /// 修改 objdef.dau 檔案，套用部隊屬性平衡模式、自訂部隊移動速度、射程、技能距離、近戰/遠程傷害與攻擊冷卻等倍率。
        /// </summary>
        private byte[] GetPatchedObjdefBytes(bool balanceChecked, bool housingCapacity20xChecked, bool storageCapacity10xChecked, bool fastBuildUpgradeChecked, Dictionary<string, double[]> leaderGlory) {
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out origBytes)) {
                throw new InvalidOperationException("記憶體備份中找不到 SYSTEM/DATA_MP/DEFAULTS/objdef.dau。");
            }

            byte[] decompBytes = GameLZSS.DecompressPfil(origBytes!);
            string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
            string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);
            string[] originalLines = (string[])lines.Clone();
            int originalContentLen = decomp.Length;

            for (int idx = 2; idx < lines.Length; idx++) {
                string line = lines[idx];
                if (line.Length < 100) continue;
                string[] cols = ParseCsvLine(line);
                if (cols.Length < 192) continue;
                string name = cols[52].Trim();

                if (leaderGlory != null && leaderGlory.TryGetValue(name, out double[]? stats)) {
                    // stats = { awStuf, vwStuf, damStuf, moraleBonus, moraleTime, maxRuhm }
                    int[] indices = { 148, 149, 150, 161, 162, 153 };
                    for (int i = 0; i < indices.Length; i++) {
                        int colIdx = indices[i];
                        double val = stats[i];
                        string targetValue = val.ToString(CultureInfo.InvariantCulture);
                        int targetLen = cols[colIdx].Length;
                        if (CheckLen(targetValue, targetLen, out string finalValue)) {
                            cols[colIdx] = finalValue.PadLeft(targetLen);
                        }
                    }
                }

                if (housingCapacity20xChecked) {
                    string[] origColsForHousing = ParseCsvLine(originalLines[idx]);
                    int housingIndex = (int)ObjdefIndex.HousingCapacity;
                    if (housingIndex < cols.Length && housingIndex < origColsForHousing.Length &&
                        int.TryParse(origColsForHousing[housingIndex].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int originalHousingCapacity) &&
                        originalHousingCapacity > 0) {
                        int multipliedCapacity = checked(originalHousingCapacity * HousingCapacityMultiplier);
                        string targetValue = multipliedCapacity.ToString(CultureInfo.InvariantCulture);
                        int targetLen = cols[housingIndex].Length;
                        if (!CheckLen(targetValue, targetLen, out string finalValue)) {
                            throw new InvalidDataException(string.Format(
                                "Object {0} housing capacity {1} exceeds objdef.dau field length {2}; the entire apply operation was cancelled.",
                                name, targetValue, targetLen));
                        }
                        cols[housingIndex] = finalValue.PadLeft(targetLen);
                    }
                }

                if (storageCapacity10xChecked && name.StartsWith("Bau") && (name.Contains("Hau") || name.Contains("Lag"))) {
                    string[] origColsForStorage = ParseCsvLine(originalLines[idx]);
                    int storageIndex = (int)ObjdefIndex.StorageCapacity;
                    if (storageIndex < cols.Length && storageIndex < origColsForStorage.Length &&
                        int.TryParse(origColsForStorage[storageIndex].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int originalStorageCapacity) &&
                        originalStorageCapacity > 0) {
                        int multipliedCapacity = checked(originalStorageCapacity * StorageCapacityMultiplier);
                        string targetValue = multipliedCapacity.ToString(CultureInfo.InvariantCulture);
                        int targetLen = cols[storageIndex].Length;
                        if (!CheckLen(targetValue, targetLen, out string finalValue)) {
                            throw new InvalidDataException(string.Format(
                                "Object {0} storage capacity {1} exceeds objdef.dau field length {2}; the entire apply operation was cancelled.",
                                name, targetValue, targetLen));
                        }
                        cols[storageIndex] = finalValue.PadLeft(targetLen);
                    }
                }

                if (fastBuildUpgradeChecked && name.StartsWith("Bau")) {
                    string[] origCols = ParseCsvLine(originalLines[idx]);
                    int buildtIndex = 73;
                    if (buildtIndex < cols.Length && buildtIndex < origCols.Length) {
                        if (int.TryParse(origCols[buildtIndex].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int origBuildt) && origBuildt > 0) {
                            int newBuildt = Math.Max(1, origBuildt / 10);
                            string targetValue = newBuildt.ToString(CultureInfo.InvariantCulture);
                            int targetLen = cols[buildtIndex].Length;
                            if (CheckLen(targetValue, targetLen, out string finalValue)) {
                                cols[buildtIndex] = finalValue.PadLeft(targetLen);
                            }
                        }
                    }
                    int upgrdtIndex = 74;
                    if (upgrdtIndex < cols.Length && upgrdtIndex < origCols.Length) {
                        if (int.TryParse(origCols[upgrdtIndex].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int origUpgrdt) && origUpgrdt > 0) {
                            int newUpgrdt = Math.Max(1, origUpgrdt / 10);
                            string targetValue = newUpgrdt.ToString(CultureInfo.InvariantCulture);
                            int targetLen = cols[upgrdtIndex].Length;
                            if (CheckLen(targetValue, targetLen, out string finalValue)) {
                                cols[upgrdtIndex] = finalValue.PadLeft(targetLen);
                            }
                        }
                    }
                }

                if (TroopConfig.UnitMeta.ContainsKey(name)) {
                    var meta = TroopConfig.UnitMeta[name];
                    string faction = meta.Item1;
                    string tier = meta.Item2;
                    string utype = meta.Item3;
                    string style = meta.Item4;

                    string origLine = originalLines[idx];
                    string[] origCols = ParseCsvLine(origLine);
                    for (int c = 0; c < origCols.Length; c++) origCols[c] = origCols[c].Trim();

                    double origMoves;
                    double.TryParse(origCols[(int)ObjdefIndex.Moves], NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);

                    double origMovsf;
                    double.TryParse(origCols[(int)ObjdefIndex.Movsf], NumberStyles.Any, CultureInfo.InvariantCulture, out origMovsf);

                    double origBmovs;
                    double.TryParse(origCols[(int)ObjdefIndex.Bmovs], NumberStyles.Any, CultureInfo.InvariantCulture, out origBmovs);

                    double origHp;
                    double.TryParse(origCols[(int)ObjdefIndex.Hp], NumberStyles.Any, CultureInfo.InvariantCulture, out origHp);

                    double origVw;
                    double.TryParse(origCols[(int)ObjdefIndex.Vw], NumberStyles.Any, CultureInfo.InvariantCulture, out origVw);

                    double origAw;
                    double.TryParse(origCols[(int)ObjdefIndex.Aw], NumberStyles.Any, CultureInfo.InvariantCulture, out origAw);

                    double meleeDam = 0, rangedDam = 0;
                    GetMeleeAndRangedDmg(origCols, utype, out meleeDam, out rangedDam);
                    double origPrimaryDam = (utype == "ranged_inf" || utype == "ranged_cav") ? rangedDam : meleeDam;
                    if (utype == "siege") {
                        origPrimaryDam = Math.Max(meleeDam, rangedDam);
                    }

                    double[] bal = GetBaseStatsForUnit(name, origHp, origPrimaryDam, origVw, origAw, balanceChecked);

                    double baseHp = bal[0];
                    double baseDmg = bal[1];
                    double baseVw = bal[2];
                    double baseAw = bal[3];

                    double origRange = GetUnitMaxRange(origCols, utype);
                    double origMeleeRelt = 0, origRangedRelt = 0;
                    GetMeleeAndRangedRelt(origCols, utype, out origMeleeRelt, out origRangedRelt);
                    double origPrimaryRelt = origMeleeRelt;
                    if (utype == "ranged_inf" || utype == "ranged_cav") {
                        origPrimaryRelt = origRangedRelt;
                    } else if (utype == "siege") {
                        origPrimaryRelt = Math.Max(origMeleeRelt, origRangedRelt);
                    }

                    // 從 bal 陣列中讀取 9 大屬性的值
                    double customSpeed = bal[4];
                    double customSight = bal[5];
                    double customRelt = bal[6];
                    double customRange = bal[7];

                    double speedMult = 1.0;
                    if (origMoves > 0) {
                        speedMult = customSpeed / (origMoves * 2.0);
                    }

                    int newSight = (int)customSight;

                    double rangeMult = 1.0;
                    if (origRange > 0) {
                        rangeMult = customRange / origRange;
                    }

                    double reltScale = 1.0;
                    if (origPrimaryRelt > 0) {
                        reltScale = customRelt / origPrimaryRelt;
                    }

                    int finalHp = (int)baseHp;
                    int finalVw = (int)baseVw;
                    int finalAw = (int)baseAw;
                    double finalDmg = baseDmg;

                    var patchActions = new List<Tuple<int, string, string>>();

                    if (origMoves > 0) {
                        patchActions.Add(Tuple.Create((int)ObjdefIndex.Moves, (origMoves * speedMult).ToString("F2", CultureInfo.InvariantCulture), "移動速度"));
                    }
                    if (origMovsf > 0) {
                        patchActions.Add(Tuple.Create((int)ObjdefIndex.Movsf, (origMovsf * speedMult).ToString("F2", CultureInfo.InvariantCulture), "移動速度"));
                    }
                    if (origBmovs > 0) {
                        patchActions.Add(Tuple.Create((int)ObjdefIndex.Bmovs, (origBmovs * speedMult).ToString("F2", CultureInfo.InvariantCulture), "移動速度"));
                    }

                    patchActions.Add(Tuple.Create((int)ObjdefIndex.Sirad, newSight.ToString(), "視野"));

                    for (int w = 1; w <= 8; w++) {
                        int activeIndex = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                        int rangeMinIndex = (int)ObjdefIndex.Weapon1RangeMin + (w - 1) * 8;
                        int rangeMaxIndex = (int)ObjdefIndex.Weapon1RangeMax + (w - 1) * 8;
                        if (rangeMaxIndex >= origCols.Length || origCols[activeIndex].Trim() != "1") continue;

                        foreach (int rangeIndex in new int[] { rangeMinIndex, rangeMaxIndex }) {
                            if (double.TryParse(origCols[rangeIndex], NumberStyles.Any, CultureInfo.InvariantCulture, out double val) && val > 0) {
                                double newVal = val * rangeMult;
                                patchActions.Add(Tuple.Create(rangeIndex, newVal.ToString("F2", CultureInfo.InvariantCulture), "射程"));
                            }
                        }
                    }

                    patchActions.Add(Tuple.Create((int)ObjdefIndex.Hp, finalHp.ToString(), "生命值"));
                    patchActions.Add(Tuple.Create((int)ObjdefIndex.Aw, finalAw.ToString(), "戰鬥"));
                    patchActions.Add(Tuple.Create((int)ObjdefIndex.Vw, finalVw.ToString(), "防禦"));

                    double scaleFactor = origPrimaryDam > 0 ? (finalDmg / origPrimaryDam) : 1.0;

                    for (int w = 1; w <= 8; w++) {
                        int aktiIdx = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
                        int damIdx = (int)ObjdefIndex.Weapon1Dam + (w - 1) * 8;
                        int reltIdx = (int)ObjdefIndex.Weapon1Relt + (w - 1) * 8;

                        if (aktiIdx >= cols.Length || damIdx >= cols.Length || reltIdx >= cols.Length ||
                            aktiIdx >= origCols.Length || damIdx >= origCols.Length || reltIdx >= origCols.Length) {
                            continue;
                        }
                        if (origCols[aktiIdx].Trim() == "1") {
                            double wDam = double.Parse(origCols[damIdx], CultureInfo.InvariantCulture);
                            int wRelt = int.Parse(origCols[reltIdx]);

                            double newDam = wDam * scaleFactor;
                            if ((utype == "ranged_inf" || utype == "ranged_cav") && w == 1) {
                                newDam = wDam;
                            }
                            patchActions.Add(Tuple.Create(damIdx, newDam.ToString("F2", CultureInfo.InvariantCulture), "傷害"));

                            int newRelt = (int)Math.Round(wRelt * reltScale);
                            patchActions.Add(Tuple.Create(reltIdx, newRelt.ToString(), "攻擊冷卻"));
                        }
                    }

                    var updatedValues = new Dictionary<int, string>();
                    foreach (var action in patchActions) {
                        string finalVal;
                        int targetLen = cols[action.Item1].Length;
                        if (!CheckLen(action.Item2, targetLen, out finalVal)) {
                            throw new InvalidDataException(string.Format(
                                "單位 {0} 的 {1} 數值 {2} 超出 objdef.dau 欄位長度 {3}；已取消整次套用。",
                                name, action.Item3, action.Item2, targetLen));
                        }
                        updatedValues[action.Item1] = finalVal.PadLeft(targetLen);
                    }

                    foreach (var kvp in updatedValues) {
                        cols[kvp.Key] = kvp.Value;
                    }

                } else if (name == "FigZivMan00_Zivilist" || name == "FigZivWei00_Zivilistin" || name == "FigTiePac00_Packpferd") {
                    string origLine = originalLines[idx];
                    string[] origCols = ParseCsvLine(origLine);
                    for (int c = 0; c < origCols.Length; c++) origCols[c] = origCols[c].Trim();

                    double origMoves;
                    double.TryParse(origCols[(int)ObjdefIndex.Moves], NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);

                    double origMovsf;
                    double.TryParse(origCols[(int)ObjdefIndex.Movsf], NumberStyles.Any, CultureInfo.InvariantCulture, out origMovsf);

                    double origBmovs;
                    double.TryParse(origCols[(int)ObjdefIndex.Bmovs], NumberStyles.Any, CultureInfo.InvariantCulture, out origBmovs);

                    double speedMult = balanceChecked ? 2.0 : 1.0;

                    var patchActions = new List<Tuple<int, string, string>>();
                    if (origMoves > 0) {
                        patchActions.Add(Tuple.Create((int)ObjdefIndex.Moves, (origMoves * speedMult).ToString("F2", CultureInfo.InvariantCulture), "移動速度"));
                    }
                    if (origMovsf > 0) {
                        patchActions.Add(Tuple.Create((int)ObjdefIndex.Movsf, (origMovsf * speedMult).ToString("F2", CultureInfo.InvariantCulture), "移動速度"));
                    }
                    if (origBmovs > 0) {
                        patchActions.Add(Tuple.Create((int)ObjdefIndex.Bmovs, (origBmovs * speedMult).ToString("F2", CultureInfo.InvariantCulture), "移動速度"));
                    }

                    var updatedValues = new Dictionary<int, string>();
                    foreach (var action in patchActions) {
                        string finalVal;
                        int targetLen = cols[action.Item1].Length;
                        if (!CheckLen(action.Item2, targetLen, out finalVal)) {
                            throw new InvalidDataException(string.Format(
                                "單位 {0} 的 {1} 數值 {2} 超出 objdef.dau 欄位長度 {3}；已取消整次套用。",
                                name, action.Item3, action.Item2, targetLen));
                        }
                        updatedValues[action.Item1] = finalVal.PadLeft(targetLen);
                    }

                    foreach (var kvp in updatedValues) {
                        cols[kvp.Key] = kvp.Value;
                    }
                }

                lines[idx] = ToCsvString(cols);
            }

            string newContent = string.Join(lineEnding, lines);
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }
            if (newContent.Length != originalContentLen) {
                throw new Exception(string.Format("objdef.dau 長度不匹配！原始長度: {0}, 修改後長度: {1}", originalContentLen, newContent.Length));
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            return GameLZSS.CompressPfil(newBytes, origBytes!);
        }

        private static void WriteBciInt32(byte[] buffer, int offset, int value) {
            BciPattern.WriteBciInt32(buffer, offset, value);
        }


        /// <summary>
        /// 建立單一腳本的 s_addLP 加血量簽章（word 1 = 加血量字面值，寫死為
        /// amountLiteral，不使用萬用字元）：
        /// pushlit amountLiteral / pushvar 10 / pushvar 98 / pushsym addLpSymbolIndex /
        /// argc -3 / callext。
        /// 呼叫端必須同時嘗試 FoodHealAmountOriginal 與 FoodHealAmountUltimate
        /// 兩個字面值各自的簽章、合計命中次數，才能同時偵測「原版」與「已修改」
        /// 兩種狀態——若把這個字面值改成萬用字元，會連同同腳本內字面值為 -1 的
        /// 扣血（衰減）呼叫點一起命中，錯把兩種不同語意的呼叫點視為同一組。
        /// </summary>
        private static int?[] BuildFoodHealAmountSignature(int addLpSymbolIndex, int amountLiteral) {
            return new int?[] { 66, amountLiteral, 81, 10, 81, 98, 128, addLpSymbolIndex, 73, -3, 86 };
        }

        private static List<int> FindAllBciWordPatternSites(byte[] decompressedBci, int?[] signature) {
            return BciPattern.FindAllBciWordPatternSites(decompressedBci, signature);
        }

        /// <summary>
        /// 待機回血「單次加血量」補丁：12 個 Fig* 單位 AI 腳本各自唯一一處
        /// s_addLP(obj, N) 呼叫點的字面值在 1（原版）與 10（已修改）之間切換。
        /// 每個腳本要求兩種字面值合計剛好命中一次，否則視為未知狀態並中止整批套用。
        /// </summary>
        private void ApplyFoodHealingAmountPatch(string gamePath, bool enabled, FileRollbackScope? rollback) {
            string scriptRoot = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT");
            int targetValue = enabled ? FoodHealAmountUltimate : FoodHealAmountOriginal;

            foreach (var (file, symbolIndex) in FoodHealAmountSites) {
                string scriptPath = Path.Combine(scriptRoot, file + ".bci");
                if (!File.Exists(scriptPath)) {
                    throw new FileNotFoundException("找不到待機回血 AI 腳本。", scriptPath);
                }

                byte[] raw = File.ReadAllBytes(scriptPath);
                byte[] decomp = GameLZSS.DecompressPfil(raw);

                var originalSites = FindAllBciWordPatternSites(decomp,
                    BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountOriginal));
                var ultimateSites = FindAllBciWordPatternSites(decomp,
                    BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountUltimate));
                if (originalSites.Count + ultimateSites.Count != 1) {
                    throw new InvalidDataException(string.Format(
                        "待機回血 AI 腳本特徵數量不符（預期 1、實際 {0}）: {1}",
                        originalSites.Count + ultimateSites.Count, scriptPath));
                }

                int currentValue = originalSites.Count == 1 ? FoodHealAmountOriginal : FoodHealAmountUltimate;
                int patchOffset = (originalSites.Count == 1 ? originalSites[0] : ultimateSites[0]) + 4;
                if (currentValue == targetValue) continue;

                WriteBciInt32(decomp, patchOffset, targetValue);
                byte[] compressed = GameLZSS.CompressPfil(decomp, raw);
                SafeWriteAllBytes(scriptPath, compressed, rollback);
            }
        }

        /// <summary>
        /// 讀取目前遊戲目錄下待機回血補丁的啟用狀態；12 個腳本必須一致，
        /// 否則視為未知狀態（回傳 false，UI 取消勾選）。
        /// </summary>
        private bool TryReadFoodHealingAmountState(string gamePath, out bool enabled) {
            enabled = false;
            string scriptRoot = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT");
            if (!Directory.Exists(scriptRoot)) return false;

            bool? detectedState = null;
            foreach (var (file, symbolIndex) in FoodHealAmountSites) {
                string scriptPath = Path.Combine(scriptRoot, file + ".bci");
                if (!File.Exists(scriptPath)) return false;

                byte[] decomp = GameLZSS.DecompressPfil(File.ReadAllBytes(scriptPath));
                var originalSites = FindAllBciWordPatternSites(decomp,
                    BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountOriginal));
                var ultimateSites = FindAllBciWordPatternSites(decomp,
                    BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountUltimate));
                if (originalSites.Count + ultimateSites.Count != 1) {
                    return false; // 未命中或命中超過一次，視為未知狀態
                }

                bool isEnabled = ultimateSites.Count == 1;
                if (detectedState.HasValue && detectedState.Value != isEnabled) return false;
                detectedState = isEnabled;
            }

            enabled = detectedState == true;
            return detectedState.HasValue;
        }





        private Dictionary<string, byte[]> GetPatchedTeamDatBytes(bool maxPopulation) {
            const int popLimit = 1600;
            var results = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            int processedCount = 0;
            foreach (var kvp in backupFiles) {
                if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)) {
                    if (maxPopulation) {
                        byte[] decompBytes = GameLZSS.DecompressPfil(kvp.Value);
                        string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
                        string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
                        string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);
                        var newLines = new System.Collections.Generic.List<string>();
                        bool inTeamData = false;
                        foreach (string line in lines) {
                            string stripped = line.Trim();
                            if (stripped.StartsWith("[")) {
                                inTeamData = (stripped == "[teamdata]");
                                newLines.Add(line);
                                continue;
                            }
                            if (inTeamData && line.Contains(",")) {
                                string[] cols = ParseCsvLine(line);
                                if (cols.Length >= 5) {
                                    int val;
                                    if (int.TryParse(cols[4].Trim(), out val) && val > 0) {
                                        cols[4] = popLimit.ToString();
                                    }
                                }
                                newLines.Add(ToCsvString(cols));
                                continue;
                            }
                            newLines.Add(line);
                        }
                        string newContent = string.Join(lineEnding, newLines.ToArray());
                        if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                            newContent += lineEnding;
                        }
                        byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
                        byte[] compressed = GameLZSS.CompressPfil(newBytes, kvp.Value);
                        results[kvp.Key] = compressed;
                    } else {
                        results[kvp.Key] = kvp.Value;
                    }
                    processedCount++;
                }
            }
            if (maxPopulation) {
                Log(string.Format("已修改所有地圖的 team.dat 人口上限為 {0} (共處理 {1} 個檔案)。", popLimit, processedCount));
            } else {
                Log(string.Format(Loc.Get("LogRestored"), "team.dat"));
            }
            return results;
        }

        /// <summary>
        /// 套用首領死亡榮耀保留補丁。啟用時，直接覆寫遊戲的 ak_anfuehrer.bci 檔案為內嵌的 patched.bci；
        /// 停用時，將其還原為備份的原版。
        /// </summary>
        private void ApplyLeaderGloryKeepPatch(string gamePath, bool enabled, FileRollbackScope? rollback) {
            string scriptPath = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT\ak_anfuehrer.bci");
            string backupKey = "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci";

            if (enabled) {
                if (!File.Exists(scriptPath)) {
                    throw new FileNotFoundException("找不到首領 AI 腳本。", scriptPath);
                }

                // 備份原版檔案
                if (!backupFiles.ContainsKey(backupKey)) {
                    byte[] originalBytes = File.ReadAllBytes(scriptPath);
                    bool isAlreadyPatched = false;
                    try {
                        byte[] decomp = GameLZSS.DecompressPfil(originalBytes);
                        string text = Encoding.ASCII.GetString(decomp);
                        isAlreadyPatched = text.Contains("s_getObjGlory");
                    } catch {}

                    if (!isAlreadyPatched) {
                        backupFiles[backupKey] = originalBytes;
                    }
                }

                // 載入內嵌資源並寫入
                var assembly = typeof(Program).Assembly;
                using Stream? resourceStream = assembly.GetManifestResourceStream("ak_anfuehrer.patched.bci");
                if (resourceStream == null) {
                    throw new InvalidDataException("找不到內嵌的首領榮耀保留補丁資源 (ak_anfuehrer.patched.bci)。");
                }

                using MemoryStream ms = new MemoryStream();
                resourceStream.CopyTo(ms);
                byte[] patchedBytes = ms.ToArray();

                SafeWriteAllBytes(scriptPath, patchedBytes, rollback);
                Log("已套用首領死亡榮耀保留補丁。");
            } else {
                // 停用：還原為原版
                if (backupFiles.TryGetValue(backupKey, out byte[]? origBytes)) {
                    SafeWriteAllBytes(scriptPath, origBytes, rollback);
                    Log("已將首領 AI 腳本還原為備份的原版。");
                }
            }
        }
    }
}
