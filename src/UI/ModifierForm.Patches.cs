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
using System.Security.Cryptography;
using System.Threading.Tasks;
using AgainstRomeModifier.Core.Patches;

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
        private const int HousingCapacityMultiplier = 20;
        private const int StorageCapacityMultiplier = 10;
        private const int FoodHealAmountOriginal = 1;
        private const int FoodHealAmountUltimate = 10;
        // SHA-256 of the decompressed, withdrawn leader-glory-retention script
        // after its independent food-healing literal has been normalized to 1.
        private const string RetiredLeaderGloryScriptSha256 = "42B07605D20C81A93BE983252DF1CB34104EB7AE9F8456AB31A842D4CD9232DD";
        private const string VanillaLeaderScriptSha256 = "778A6E01B99664136AC0420B9F48212C41A5D6297A9952EA9D7D3A5D4851272C";

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
                    chkAiM1.Checked, chkAiM2.Checked, chkAiM3.Checked, chkAiM4.Checked, chkAiM5.Checked, chkAiM6.Checked
                };
                bool dgVoodoo = chkDgVoodoo.Checked;
                bool villageBuildRange = chkVillageBuildRange.Checked;
                bool noSpellAltar = chkNoSpellAltar.Checked;
                int gameSpeed = GetSelectedGameSpeedMultiplier();

                await Task.Run(() => {
                    // Always restore every modifier-managed surface before
                    // generating a new patch set. This prevents files written
                    // by older builds from surviving after their UI option has
                    // been removed.
                    Log(Loc.Get("LogPreApplyRestore"));
                    RestoreOriginalFilesInternal(gamePath, rollback);

                    // 1. Dry Run 階段：在記憶體中生成所有補丁 byte[] 並驗證
                    var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

                    // A. Against_Rome.exe
                    string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    bool exeModified = false;
                    ApplyExePatch(exeBytes, focusLoss, villageBuildRange, noSpellAltar, gameSpeed, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // B. cl_script.ini
                    byte[] clBytes = GetPatchedClScriptBytes(gamePath, fastCiviProduction, infMorale, balance);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_script.ini")] = clBytes;

                    // H. cl_epara.ini — 已移除技能因子功能，一律還原為原版備份
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_epara.ini")] = GetBackupBytes("SYSTEM/cl_epara.ini");

                    // G. cl_scint.ini — 已移除法術強化功能，一律還原為原版備份
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\CLAK\cl_scint.ini")] = GetBackupBytes("SYSTEM/CLAK/cl_scint.ini");

                    // C. ress.ini
                    byte[] ressBytes = GetPatchedRessBytes(freeProd, freeUp, noSpell);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\ress.ini")] = ressBytes;

                    // D. objdef.dau
                    byte[] objdefBytes = GetPatchedObjdefBytes(balance, housingCapacity20x, storageCapacity10x, fastBuildUpgradeRepair);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau")] = objdefBytes;

                    // E. team.dat
                    var teamDatPatches = GetPatchedTeamDatBytes(maxPopulation);
                    foreach (var kvp in teamDatPatches) {
                        patchedFiles[Path.Combine(gamePath, kvp.Key.Replace('/', '\\'))] = kvp.Value;
                    }

                    // F. endless AI scripts, templates, and economy modules
                    // 每個模組(M1..M6)由各自的獨立勾選框驅動；R0 常駐修復無論如何都執行。
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
                    RestoreOriginalFilesInternal(gamePath, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkFocusLoss.Checked = false;
                chkNoSpellAltar.Checked = false;
                chkToEng.Checked = false;
                chkAiM1.Checked = false; chkAiM2.Checked = false; chkAiM3.Checked = false; chkAiM4.Checked = false; chkAiM5.Checked = false; chkAiM6.Checked = false;
                chkHousingCapacity20x.Checked = false; chkStorageCapacity10x.Checked = false;
                chkFastBuildUpgradeRepair.Checked = false;
                chkFoodHealing10x.Checked = false;
                chkMaxPopulation.Checked = false;
                chkFastCiviProduction.Checked = false;
                chkFreeProd.Checked = false;
                chkFreeUpgrade.Checked = false;
                chkNoSpellCost.Checked = false;
                chkInfiniteMorale.Checked = false;
                chkBalance.Checked = false;
                chkDgVoodoo.Checked = IsDgVoodooInstalled(gamePath);
                chkVillageBuildRange.Checked = false;
                SetGameSpeedSelection(1);
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
                    ApplyFoodHealingAmountPatch(gamePath, false, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkHousingCapacity20x.Checked = false; chkStorageCapacity10x.Checked = false;
                chkFastBuildUpgradeRepair.Checked = false;
                chkFoodHealing10x.Checked = false;
                chkMaxPopulation.Checked = false;
                chkFastCiviProduction.Checked = false;
                chkFreeProd.Checked = false;
                chkFreeUpgrade.Checked = false;
                chkNoSpellCost.Checked = false;
                chkInfiniteMorale.Checked = false;
                chkBalance.Checked = false;
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
                    ApplyExePatch(exeBytes, false, false, false, 1, ref exeModified);
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
                chkAiM1.Checked = false; chkAiM2.Checked = false; chkAiM3.Checked = false; chkAiM4.Checked = false; chkAiM5.Checked = false; chkAiM6.Checked = false;
                chkDgVoodoo.Checked = IsDgVoodooInstalled(gamePath);
                chkVillageBuildRange.Checked = false;
                SetGameSpeedSelection(1);
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
            foreach (var kvp in backupFiles) {
                if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)) {
                    string destPath = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                    RestoreMemoryFile(kvp.Key, destPath, rollback);
                }
            }
        }

        /// <summary>
        /// Restores every file surface managed by the modifier without changing
        /// UI selections. Used by Restore All and by the mandatory clean-base
        /// phase at the beginning of Apply.
        /// </summary>
        private void RestoreOriginalFilesInternal(string gamePath, FileRollbackScope? rollback) {
            var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

            string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
            byte[] exeBytes = File.ReadAllBytes(exePath);
            bool exeModified = false;
            ApplyExePatch(exeBytes, false, false, false, 1, ref exeModified);
            if (exeModified) {
                patchedFiles[exePath] = exeBytes;
            }

            var orchestrator = new EndlessAiOrchestrator();
            foreach (var module in orchestrator.UserModules) {
                orchestrator.ApplyModule(gamePath, module, false);
            }
            orchestrator.ApplyMandatoryRepair(gamePath);

            RestoreStatsOnlyInternal(gamePath, rollback);

            foreach (var kvp in patchedFiles) {
                SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
            }
            orchestrator.SaveAll(gamePath, rollback);
            ApplyFoodHealingAmountPatch(gamePath, false, rollback);
            ApplyLanguagePatch(gamePath, false, rollback);
            ApplyDgVoodooPatch(gamePath, false, rollback);
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

        // Against_Rome.exe 的固定偏移補丁：狀態偵測與「state → 預期/取代位元組」的
        // 選擇邏輯集中在 ExePatchModel（純類別、可單獨測試）。以下方法只負責在地化
        // 日誌與 exeModified 旗標，實際寫入委派給 ExePatchModel。
        private void ApplySpellAltarPatch(byte[] exeBytes, bool noAltarChecked, ref bool exeModified) {
            ExeSpellAltarPatchState state = ExePatchModel.GetSpellAltarPatchState(exeBytes);
            if (state == ExeSpellAltarPatchState.Unknown) {
                throw new Exception("Against_Rome.exe 版本或法術祭壇特徵碼不符合預期，已停止套用法術祭壇補丁。");
            }

            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanSpellAltar(noAltarChecked, state);
            if (ops.Count > 0) {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
                Log(noAltarChecked ? "已套用法術免祭壇需求補丁。" : "已還原法術祭壇需求設定。");
            }
        }

        private void RestoreLegacyVillageBuildRangePatch(byte[] exeBytes, ref bool exeModified) {
            ExeVillageRangePatchState state = ExePatchModel.GetVillageBuildRangePatchState(exeBytes);
            if (state == ExeVillageRangePatchState.Unknown) {
                Log(Loc.Get("LogVillageBuildRangeWarning"));
                return;
            }

            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanVillageRangeRestore(state);
            if (ops.Count > 0) {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
                Log(Loc.Get("LogVillageBuildRangeRestored"));
            }
        }

        private void ApplyVillageSetterRangePatch(byte[] exeBytes, bool enabled, ref bool exeModified) {
            ExeVillageSetterPatchState state = ExePatchModel.GetVillageSetterPatchState(exeBytes);
            if (state == ExeVillageSetterPatchState.Unknown) {
                if (enabled) {
                    throw new InvalidOperationException(Loc.Get("LogVillageBuildRangeWarning"));
                }
                Log(Loc.Get("LogVillageBuildRangeWarning"));
                return;
            }

            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanVillageSetter(enabled, state);
            if (ops.Count > 0) {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
            }

            if (enabled) {
                Log(Loc.Get("LogVillageBuildRangeApplied"));
            } else if (ops.Count > 0) {
                Log(Loc.Get("LogVillageBuildRangeSetterRestored"));
            }
        }

        private void ApplyExePatch(byte[] exeBytes, bool focusLossChecked, bool villageBuildRangeChecked, bool noSpellAltarChecked, int gameSpeedMultiplier, ref bool exeModified) {
            ExePatchState state = ExePatchModel.GetExePatchState(exeBytes);
            if (state == ExePatchState.Unknown) {
                throw new Exception("Against_Rome.exe 版本或位元組特徵不符合預期，已停止相容性補丁以避免覆蓋未知版本。");
            }

            IReadOnlyList<ExeWriteOp> focusOps = ExePatchModel.PlanFocus(focusLossChecked, state);
            if (focusOps.Count > 0) {
                ExePatchModel.Apply(exeBytes, focusOps);
                exeModified = true;
            }
            Log(Loc.Get(focusLossChecked ? "LogExePatchFocus" : "LogExePatchOrig"));

            RestoreLegacyVillageBuildRangePatch(exeBytes, ref exeModified);
            if (villageBuildRangeChecked &&
                ExePatchModel.GetVillageBuildRangePatchState(exeBytes) != ExeVillageRangePatchState.Original) {
                throw new InvalidOperationException(Loc.Get("LogVillageBuildRangeWarning"));
            }
            ApplyVillageSetterRangePatch(exeBytes, villageBuildRangeChecked, ref exeModified);

            ApplySpellAltarPatch(exeBytes, noSpellAltarChecked, ref exeModified);

            ApplyGameSpeedPatch(exeBytes, gameSpeedMultiplier, ref exeModified);
        }

        /// <summary>
        /// 遊戲整體時脈加速：把主時脈函式的兩個 rodata 常數（QPC 與 timeGetTime 路徑）同步
        /// 乘上倍率，使移動／生產／戰鬥／AI 一起以該倍率前進。<paramref name="multiplier"/> = 1
        /// 代表關閉（還原原版）。選擇邏輯集中於 <see cref="ExePatchModel"/>，此處只負責日誌與旗標。
        /// </summary>
        /// <summary>以目前語系重建加速下拉選單的項目（index 0 = 原版，其後為各支援倍率），並保留目前選項。</summary>
        private void PopulateGameSpeedItems() {
            int prev = cmbGameSpeed.SelectedIndex;
            cmbGameSpeed.BeginUpdate();
            cmbGameSpeed.Items.Clear();
            cmbGameSpeed.Items.Add(Loc.Get("GameSpeedOff"));
            foreach (int m in ExePatchModel.GameSpeedSupportedMultipliers) {
                if (m <= 1) continue;
                cmbGameSpeed.Items.Add(string.Format(Loc.Get("GameSpeedItem"), m));
            }
            cmbGameSpeed.EndUpdate();
            cmbGameSpeed.SelectedIndex = prev >= 0 && prev < cmbGameSpeed.Items.Count ? prev : 0;
        }

        /// <summary>由下拉選單目前選項換算加速倍率（index 0 = 原版 → 1；其餘 → index+1）。</summary>
        private int GetSelectedGameSpeedMultiplier() {
            int idx = cmbGameSpeed.SelectedIndex;
            return idx <= 0 ? 1 : idx + 1;
        }

        /// <summary>依偵測到的倍率設定下拉選單目前選項；未知或超出範圍時回到原版。</summary>
        private void SetGameSpeedSelection(int multiplier) {
            int idx = multiplier >= 2 && multiplier <= cmbGameSpeed.Items.Count ? multiplier - 1 : 0;
            cmbGameSpeed.SelectedIndex = idx;
        }

        private void ApplyGameSpeedPatch(byte[] exeBytes, int multiplier, ref bool exeModified) {
            int current = ExePatchModel.GetGameSpeedMultiplier(exeBytes);
            if (current == 0) {
                Log("偵測到未知的遊戲時脈常數，已略過遊戲加速補丁以免覆蓋未知版本。");
                return;
            }
            IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanGameSpeed(multiplier, current);
            if (ops.Count > 0) {
                ExePatchModel.Apply(exeBytes, ops);
                exeModified = true;
            }
            Log(multiplier > 1 ? $"遊戲整體運行速度：{multiplier}× 加速。" : "遊戲整體運行速度：原版（未加速）。");
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

        private byte[] GetPatchedClScriptBytes(string gamePath, bool fastCiviProduction, bool infiniteMoraleChecked, bool balanceChecked) {
            byte[] original = GetBackupBytes("SYSTEM/cl_script.ini");
            byte[] patchBase = SelectValidatedPatchBase(gamePath, @"SYSTEM\cl_script.ini", original, bytes => {
                string current = Encoding.GetEncoding(1251).GetString(GameLZSS.DecompressPfil(bytes));
                return GetClScriptManagedKeys(Encoding.GetEncoding(1251).GetString(GameLZSS.DecompressPfil(original))).IsSubsetOf(GetClScriptManagedKeys(current));
            }, "cl_script.ini");
            double celtMultiplier = balanceChecked ? 2.5 : 1.0;
            double hunMultiplier = balanceChecked ? 2.5 : 1.0;
            if (customUnitStats != null && customUnitStats.TryGetValue("FigKelPri00_Priester", out double[]? celt) && celt.Length > 8) celtMultiplier = celt[8] / 500.0;
            if (customUnitStats != null && customUnitStats.TryGetValue("FigHunPri00_Priester", out double[]? hun) && hun.Length > 8) hunMultiplier = hun[8] / 500.0;
            // 法師強化與自訂技能已移除：spellEnhancement 固定 false、技能字典留空（一律寫入原版值）。
            return ClScriptPatcher.GetPatchedBytes(patchBase, new ClScriptOptions(fastCiviProduction, infiniteMoraleChecked, false, new Dictionary<string, double>(), 1.0, celtMultiplier, hunMultiplier, original));
        }

        private byte[] GetBackupBytes(string key) {
            if (!backupFiles.TryGetValue(key, out byte[]? bytes)) throw new InvalidOperationException("記憶體備份中找不到 " + key + "。");
            return bytes;
        }

        private byte[] SelectValidatedPatchBase(string gamePath, string relativePath, byte[] fallback, Func<byte[], bool> validate, string displayName) {
            string path = Path.Combine(gamePath, relativePath);
            if (!File.Exists(path)) return fallback;
            try {
                byte[] current = File.ReadAllBytes(path);
                if (validate(current)) return current;
                Log("現有 " + displayName + " 結構不完整，已改用安全備份作為修改基底。");
            } catch (Exception ex) {
                Log("現有 " + displayName + " 無法驗證，已改用安全備份作為修改基底: " + ex.Message);
            }
            return fallback;
        }

        private byte[] GetPatchedRessBytes(bool freeProdChecked, bool freeUpgradeChecked, bool noSpellCostChecked) {
            return RessPatcher.GetPatchedBytes(GetBackupBytes("SYSTEM/ress.ini"), new RessOptions(freeProdChecked, freeUpgradeChecked, noSpellCostChecked));
        }

        private byte[] GetPatchedObjdefBytes(bool balanceChecked, bool housingCapacity20xChecked, bool storageCapacity10xChecked, bool fastBuildUpgradeChecked) {
            byte[] original = GetBackupBytes("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
            var unitStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
            foreach (string key in TroopConfig.UnitMeta.Keys) unitStats[key] = GetBaseStatsForUnit(key, 0, 0, 0, 0, balanceChecked);
            // 首領榮耀成長功能已移除：leaderGlory 留空（objdef 榮耀欄位一律保持原版）。
            return ObjdefPatcher.GetPatchedBytes(original, new ObjdefOptions(balanceChecked, housingCapacity20xChecked, storageCapacity10xChecked, fastBuildUpgradeChecked, new Dictionary<string, double[]>(), unitStats));
        }

        private static void WriteBciInt32(byte[] buffer, int offset, int expectedValue, int value, string patchName) {
            BciPattern.WriteBciInt32(buffer, offset, expectedValue, value, patchName);
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
                bool retiredLeaderScriptMigrated = false;

                // Older builds could install an experimental ak_anfuehrer.bci
                // that crashes when combat invokes the leader script. Removing
                // the feature from the UI did not repair files already written
                // to the game directory. Detect that exact retired payload
                // (allowing either food-healing literal) and rebuild from the
                // clean embedded backup before applying the supported patch.
                if (file.Equals("ak_anfuehrer", StringComparison.OrdinalIgnoreCase)) {
                    byte[] normalized = (byte[])decomp.Clone();
                    var legacyOriginalSites = FindAllBciWordPatternSites(normalized,
                        BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountOriginal));
                    var legacyUltimateSites = FindAllBciWordPatternSites(normalized,
                        BuildFoodHealAmountSignature(symbolIndex, FoodHealAmountUltimate));
                    if (legacyOriginalSites.Count + legacyUltimateSites.Count == 1) {
                        if (legacyUltimateSites.Count == 1) {
                            int valueOffset = legacyUltimateSites[0] + 4;
                            WriteBciInt32(normalized, valueOffset,
                                FoodHealAmountUltimate, FoodHealAmountOriginal, "retired leader glory probe");
                        }

                        string normalizedHash = Convert.ToHexString(SHA256.HashData(normalized));
                        if (normalizedHash.Equals(RetiredLeaderGloryScriptSha256, StringComparison.Ordinal)) {
                            raw = GetBackupBytes("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci");
                            decomp = GameLZSS.DecompressPfil(raw);
                            string cleanHash = Convert.ToHexString(SHA256.HashData(decomp));
                            if (!cleanHash.Equals(VanillaLeaderScriptSha256, StringComparison.Ordinal)) {
                                throw new InvalidDataException("乾淨的原版 ak_anfuehrer.bci 備份不存在或版本不符，已取消安全遷移。");
                            }
                            retiredLeaderScriptMigrated = true;
                            Log("已移除會造成戰鬥閃退的舊版首領榮耀腳本，並以原版 ak_anfuehrer.bci 重建。");
                        }
                    }
                }

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
                if (currentValue == targetValue) {
                    if (retiredLeaderScriptMigrated) {
                        SafeWriteAllBytes(scriptPath, raw, rollback);
                    }
                    continue;
                }

                WriteBciInt32(decomp, patchOffset, currentValue, targetValue, "待機回血量");
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

        /// <summary>
        /// Performs a narrowly fingerprinted one-time safety migration when an
        /// older build left the withdrawn leader-glory script installed. This
        /// runs at startup because removing the old UI did not repair scripts
        /// already written to the game directory. The current supported
        /// food-healing state is preserved.
        /// </summary>
        private void RepairRetiredLeaderGloryScriptOnStartup() {
            try {
                string gamePath = GetGamePath();
                if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) return;
                if (!TryReadFoodHealingAmountState(gamePath, out bool foodHealingEnabled)) return;
                ApplyFoodHealingAmountPatch(gamePath, foodHealingEnabled, null);
            } catch (Exception ex) {
                Log("舊版首領腳本安全遷移失敗: " + ex.Message);
            }
        }





        private Dictionary<string, byte[]> GetPatchedTeamDatBytes(bool maxPopulation) {
            var results = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            foreach (var item in backupFiles.Where(item => item.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && item.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase))) {
                results[item.Key] = TeamDatPatcher.GetPatchedBytes(item.Value, new TeamDatOptions(maxPopulation));
            }
            Log(maxPopulation ? string.Format("已修改所有地圖的 team.dat 人口上限為 {0} (共處理 {1} 個檔案)。", 1600, results.Count) : string.Format(Loc.Get("LogRestored"), "team.dat"));
            return results;
        }

    }
}
