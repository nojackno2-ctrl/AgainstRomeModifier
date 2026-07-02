using System;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Text.Json;
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
        private static readonly object LogLock = new object();
        private const string LanguageBackupDirectoryName = ".against-rome-modifier-language-backup";
        private const string LanguageBackupManifestName = "manifest.json";
        private static readonly Regex RegexRadiusPatch = new Regex(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexCiviPatch = new Regex(@"^CiviDelay\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleLostMemPatch = new Regex(@"^(MoralsDecLostMem\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleFleePatch = new Regex(@"^(MoralsDecFlee\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleOverPopPatch = new Regex(@"^(MoralsDecOverPop\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleIncIdlePatch = new Regex(@"^(MoralsIncIdle\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleKey = new Regex(@"^(MoralsDecLostMem|MoralsDecFlee|MoralsDecOverPop|MoralsIncIdle)\s*=\s*([A-Z]{3})\s*,", RegexOptions.Compiled);
        private static readonly byte[] ExeFocusOriginalBytes = new byte[] { 0x89, 0x15, 0xC4, 0x7D, 0x9E, 0x02 };
        private static readonly byte[] ExeFocusPatchedBytes = new byte[] { 0x90, 0x90, 0x90, 0x90, 0x90, 0x90 };
        private const long ExeFocusPatchOffset = 0x161a88;
        private const long ExeFocusPatchRequiredLength = 0x161a8e;
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
            0x8D, 0x34, 0xB6, 0xD1, 0xEE,
            0x8D, 0x3C, 0xBF, 0xD1, 0xEF,
            0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
            0xE9, 0x1B, 0x3F, 0xFD, 0xFF
        };
        private const long ExeVillageSetterHookOffset = 0x1364c1;
        private const long ExeVillageSetterCaveOffset = 0x16258f;
        private const long ExeVillageSetterPatchRequiredLength = 0x1625b6;
        private const int EndlessAiOriginalMilitaryCount = 4;
        private const int EndlessAiUltimateMilitaryCount = 20;
        private const int HousingCapacityMultiplier = 20;
        private const int EndlessAiOriginalRespawnDelayMs = 180000;
        private const int EndlessAiUltimateRespawnDelayMs = 5000;
        private const int EndlessAiUltimateLoopDelayLowerMs = 5000;
        private const int EndlessAiUltimateLoopDelayUpperMs = 10000;
        private const int EndlessAiOriginalActivePartyLimit = 4;
        private const int EndlessAiUltimateActivePartyLimit = 8;
        private const int EndlessAiOriginalAutoRecycleCompletedJob = 0;
        private const int EndlessAiUltimateAutoRecycleCompletedJob = 1;
        private const int EndlessAiOriginalFreeCivilianReserve = 0;
        private const int EndlessAiUltimateFreeCivilianReserve = 20;
        private const int EndlessAiProductionGateOriginalOpcode = 117;
        private const int EndlessAiProductionGateBypassOpcode = 112;
        private const int EndlessAiProductionGateJump = 56;
        private const int EndlessAiFormationLimitOriginalOpcode = 81;
        private const int EndlessAiFormationLimitOriginalValue = 59;
        private const int EndlessAiFormationLimitPatchedOpcode = 66;
        private const int EndlessAiFormationLimitPatchedValue = 20;
        private const int EndlessAiActiveLimitOriginalOpcode = 66;
        private const int EndlessAiActiveLimitOriginalValue = 0;
        // Written by older builds. It bypassed the active-party gate and could
        // exhaust the 20 NPC-job slots available to each team.
        private const int EndlessAiActiveLimitPatchedOpcode = 112;
        private const int EndlessAiActiveLimitPatchedRelativeJump = 272;
        private const int EndlessAiReinforcementLoopCount = 3;
        private static readonly (int OriginalUpperMs, int OriginalLowerMs)[] EndlessAiLoopDelayRanges = new (int, int)[] {
            (960000, 480000),
            (960000, 480000),
            (360000, 240000),
            (120000, 60000),
            (120000, 60000),
            (240000, 120000)
        };

        private sealed class FileRollbackScope : IDisposable {
            private sealed class RollbackEntry {
                public string Path { get; set; } = "";
                public string BackupPath { get; set; } = "";
                public bool Existed { get; set; }
            }

            private readonly object _syncRoot = new object();
            private readonly Dictionary<string, RollbackEntry> _entries = new Dictionary<string, RollbackEntry>(StringComparer.OrdinalIgnoreCase);
            private readonly string _rootPath;
            private bool _committed;
            private bool _restored;

            public FileRollbackScope() {
                _rootPath = Path.Combine(Path.GetTempPath(), "AgainstRomeModifierRollback_" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(_rootPath);
            }

            public void TrackFile(string path) {
                string fullPath = Path.GetFullPath(path);
                lock (_syncRoot) {
                    if (_entries.ContainsKey(fullPath)) return;

                    var entry = new RollbackEntry {
                        Path = fullPath,
                        Existed = File.Exists(fullPath)
                    };

                    if (entry.Existed) {
                        string backupPath = Path.Combine(_rootPath, Guid.NewGuid().ToString("N") + ".bak");
                        File.Copy(fullPath, backupPath, true);
                        entry.BackupPath = backupPath;
                    }

                    _entries[fullPath] = entry;
                }
            }

            public void Commit() {
                _committed = true;
            }

            public bool IsCommitted {
                get { return _committed; }
            }

            public void RestoreAll(Action<string>? log) {
                if (_restored) return;
                foreach (var entry in _entries.Values.Reverse()) {
                    try {
                        string? dir = Path.GetDirectoryName(entry.Path);
                        if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) {
                            Directory.CreateDirectory(dir);
                        }

                        if (entry.Existed) {
                            if (File.Exists(entry.Path)) {
                                File.SetAttributes(entry.Path, FileAttributes.Normal);
                            }
                            File.Copy(entry.BackupPath, entry.Path, true);
                        } else if (File.Exists(entry.Path)) {
                            File.SetAttributes(entry.Path, FileAttributes.Normal);
                            File.Delete(entry.Path);
                        }
                    } catch (Exception ex) {
                        log?.Invoke(string.Format("[回復警告] 無法回復 {0}: {1}", entry.Path, ex.Message));
                    }
                }
                _restored = true;
            }

            public void Dispose() {
                if (!_committed && !_restored && _entries.Count > 0) {
                    RestoreAll(null);
                }
                if (Directory.Exists(_rootPath)) {
                    try {
                        Directory.Delete(_rootPath, true);
                    } catch (Exception ex) {
                        System.Diagnostics.Debug.WriteLine("[Rollback] Failed to delete temp directory " + _rootPath + ": " + ex.Message);
                    }
                }
            }
        }

        /// <summary>
        /// 記錄日誌訊息，輸出至 UI 的文字框，並非同步寫入至本地 modifier_log.txt 檔案。
        /// </summary>
        private void Log(string message) {
            string text = string.Format("[{0}] {1}\r\n", DateTime.Now.ToString("HH:mm:ss"), message);
            if (txtLog != null) {
                if (txtLog.InvokeRequired) {
                    txtLog.BeginInvoke(new Action(() => txtLog.AppendText(text)));
                } else {
                    txtLog.AppendText(text);
                }
            }
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
                bool housingCapacity20x = chkHousingCapacity20x.Checked;
                bool toEng = chkToEng.Checked;
                bool aiUltimateMode = chkAiUltimateMode.Checked;
                bool dgVoodoo = chkDgVoodoo.Checked;
                bool villageBuildRange = chkVillageBuildRange.Checked;

                await Task.Run(() => {
                    // 1. Dry Run 階段：在記憶體中生成所有補丁 byte[] 並驗證
                    var patchedFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

                    // A. Against_Rome.exe
                    string exePath = Path.Combine(gamePath, @"Against_Rome.exe");
                    byte[] exeBytes = File.ReadAllBytes(exePath);
                    bool exeModified = false;
                    ApplyExePatch(exeBytes, focusLoss, villageBuildRange, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // B. cl_script.ini
                    byte[] clBytes = GetPatchedClScriptBytes(gamePath, fastCiviProduction, infMorale, balance);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\cl_script.ini")] = clBytes;

                    // C. ress.ini
                    byte[] ressBytes = GetPatchedRessBytes(freeProd, freeUp, noSpell);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\ress.ini")] = ressBytes;

                    // D. objdef.dau
                    byte[] objdefBytes = GetPatchedObjdefBytes(balance, housingCapacity20x);
                    patchedFiles[Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau")] = objdefBytes;

                    // E. team.dat
                    var teamDatPatches = GetPatchedTeamDatBytes(maxPopulation);
                    foreach (var kvp in teamDatPatches) {
                        patchedFiles[Path.Combine(gamePath, kvp.Key.Replace('/', '\\'))] = kvp.Value;
                    }

                    // F. endless AI scripts
                    var endlessPatches = GetPatchedEndlessScripts(gamePath, aiUltimateMode);
                    foreach (var kvp in endlessPatches) {
                        patchedFiles[kvp.Key] = kvp.Value;
                    }

                    // Dry Run 順利結束，未拋出任何異常。進入實體檔案寫入與交易範圍
                    foreach (var kvp in patchedFiles) {
                        SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
                    }

                    // 其它不涉及複雜解壓修改且安全的補丁
                    ApplyLanguagePatch(gamePath, toEng, rollback);
                    ApplyDgVoodooPatch(gamePath, dgVoodoo, rollback);

                    // 呼叫無盡經濟模式還原
                    ApplyEndlessAiVillageEconomyPatch(gamePath, false, rollback);
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
                    ApplyExePatch(exeBytes, false, false, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // 無盡 AI 還原
                    var endlessPatches = GetPatchedEndlessScripts(gamePath, false);
                    foreach (var kvp in endlessPatches) {
                        patchedFiles[kvp.Key] = kvp.Value;
                    }

                    // 還原其它屬性 INI 與 team.dat 到備份原版
                    RestoreStatsOnlyInternal(gamePath, rollback);

                    // 統一寫入記憶體修改之檔案
                    foreach (var kvp in patchedFiles) {
                        SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
                    }

                    ApplyLanguagePatch(gamePath, false, rollback);
                    ApplyDgVoodooPatch(gamePath, false, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkFocusLoss.Checked = false;
                chkToEng.Checked = false;
                chkAiUltimateMode.Checked = false;
                chkHousingCapacity20x.Checked = false;
                chkMaxPopulation.Checked = false;
                chkFastCiviProduction.Checked = false;
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
                await Task.Run(() => RestoreStatsOnlyInternal(gamePath, rollback));
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkHousingCapacity20x.Checked = false;
                chkMaxPopulation.Checked = false;
                chkFastCiviProduction.Checked = false;
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
                    ApplyExePatch(exeBytes, false, false, ref exeModified);
                    if (exeModified) {
                        patchedFiles[exePath] = exeBytes;
                    }

                    // 無盡 AI 還原
                    var endlessPatches = GetPatchedEndlessScripts(gamePath, false);
                    foreach (var kvp in endlessPatches) {
                        patchedFiles[kvp.Key] = kvp.Value;
                    }

                    // 統一寫入記憶體修改之檔案
                    foreach (var kvp in patchedFiles) {
                        SafeWriteAllBytes(kvp.Key, kvp.Value, rollback);
                    }

                    ApplyDgVoodooPatch(gamePath, false, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                chkFocusLoss.Checked = false;
                chkAiUltimateMode.Checked = false;
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
            Expanded2Point5x
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
            bool expanded2Point5x = hookBytes.SequenceEqual(ExeVillageSetterHookPatchedBytes) &&
                caveBytes.SequenceEqual(ExeVillageSetterCavePatchedBytes);
            if (original) return ExeVillageSetterPatchState.Original;
            if (legacy2x) return ExeVillageSetterPatchState.Legacy2x;
            if (expanded2Point5x) return ExeVillageSetterPatchState.Expanded2Point5x;
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
                    state == ExeVillageSetterPatchState.Legacy2x) {
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterCaveOffset, ExeVillageSetterCavePatchedBytes);
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterHookOffset, ExeVillageSetterHookPatchedBytes);
                    exeModified = true;
                }
                Log(Loc.Get("LogVillageBuildRangeApplied"));
            } else {
                if (state == ExeVillageSetterPatchState.Legacy2x ||
                    state == ExeVillageSetterPatchState.Expanded2Point5x) {
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterHookOffset, ExeVillageSetterHookOriginalBytes);
                    WriteExeBytesInMemory(exeBytes, ExeVillageSetterCaveOffset, ExeVillageSetterCaveOriginalBytes);
                    exeModified = true;
                    Log(Loc.Get("LogVillageBuildRangeSetterRestored"));
                }
            }
        }

        private void ApplyExePatch(byte[] exeBytes, bool focusLossChecked, bool villageBuildRangeChecked, ref bool exeModified) {
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
            }
            return keys;
        }

        private byte[] GetPatchedClScriptBytes(string gamePath, bool fastCiviProduction, bool infiniteMoraleChecked, bool balanceChecked) {
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/cl_script.ini", out origBytes)) {
                throw new InvalidOperationException("記憶體備份中找不到 SYSTEM/cl_script.ini。");
            }

            // 1. 建立原版備份中的 Radius 對照字典，以防多次套用導致數值累乘
            var originalRadiuses = new Dictionary<string, double>(StringComparer.OrdinalIgnoreCase);
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
        private byte[] GetPatchedObjdefBytes(bool balanceChecked, bool housingCapacity20xChecked) {
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

        /// <summary>
        /// 還原地圖目錄下所有的 team.dat 檔案。
        /// </summary>
        /* private void RestoreTeamFiles(string gamePath, FileRollbackScope? rollback = null) {
            foreach (var kvp in backupFiles) {
                if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)) {
                    string destPath = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                    SafeWriteAllBytes(destPath, kvp.Value, rollback);
                }
            }
            Log(string.Format(Loc.Get("LogRestored"), "team.dat"));
        } */

        /// <summary>
        /// 修改各地圖目錄下的 team.dat，使其中定義的人口上限與主程式或 UI 界面上設定的人口數相符。
        /// </summary>
        private static int FindEndlessMilitaryCreateUnitCall(byte[] decompressedBci) {
            int?[] pattern = new int?[] {
                0x42, null,
                0x42, 1,
                0x42, null,
                0x42, null,
                0x42, 0,
                0x42, 0,
                0x42, 8,
                0x42, 3,
                0x5A, 7,
                0x80, 0xD4,
                0x49, unchecked((int)0xFFFFFFF7),
                0x56
            };

            int patternBytes = pattern.Length * 4;
            for (int offset = 0; offset <= decompressedBci.Length - patternBytes; offset += 4) {
                bool match = true;
                for (int i = 0; i < pattern.Length; i++) {
                    int? expected = pattern[i];
                    if (expected.HasValue && BitConverter.ToInt32(decompressedBci, offset + (i * 4)) != expected.Value) {
                        match = false;
                        break;
                    }
                }
                if (match) {
                    return offset;
                }
            }
            return -1;
        }

        private static int FindEndlessRespawnDelayLiteral(byte[] decompressedBci) {
            int?[] pattern = new int?[] {
                0x80, 83,
                0x56, 66,
                null, 32,
                44, 164,
                0x42, 34,
                0x5B, 5
            };

            int patternBytes = pattern.Length * 4;
            for (int offset = 0; offset <= decompressedBci.Length - patternBytes; offset += 4) {
                bool match = true;
                for (int i = 0; i < pattern.Length; i++) {
                    int? expected = pattern[i];
                    if (expected.HasValue && BitConverter.ToInt32(decompressedBci, offset + (i * 4)) != expected.Value) {
                        match = false;
                        break;
                    }
                }
                if (match) {
                    return offset + 16;
                }
            }
            return -1;
        }

        private static void WriteBciInt32(byte[] buffer, int offset, int value) {
            byte[] bytes = BitConverter.GetBytes(value);
            Buffer.BlockCopy(bytes, 0, buffer, offset, 4);
        }

        private static int FindBciWordPattern(byte[] decompressedBci, int?[] pattern) {
            int patternBytes = pattern.Length * 4;
            for (int offset = 0; offset <= decompressedBci.Length - patternBytes; offset += 4) {
                bool match = true;
                for (int i = 0; i < pattern.Length; i++) {
                    int? expected = pattern[i];
                    if (expected.HasValue && BitConverter.ToInt32(decompressedBci, offset + (i * 4)) != expected.Value) {
                        match = false;
                        break;
                    }
                }
                if (match) return offset;
            }
            return -1;
        }

        private void PatchEndlessAiEconomyScript(
            string scriptPath,
            int?[] signature,
            int patchWordIndex,
            int originalValue,
            int enabledValue,
            bool enabled,
            FileRollbackScope? rollback) {
            if (!File.Exists(scriptPath)) {
                throw new FileNotFoundException("找不到 AI 經濟腳本。", scriptPath);
            }

            byte[] raw = File.ReadAllBytes(scriptPath);
            byte[] decomp = GameLZSS.DecompressPfil(raw);
            int signatureOffset = FindBciWordPattern(decomp, signature);
            if (signatureOffset < 0) {
                throw new InvalidDataException("AI 經濟腳本特徵不符合預期: " + scriptPath);
            }

            int patchOffset = signatureOffset + (patchWordIndex * 4);
            int currentValue = BitConverter.ToInt32(decomp, patchOffset);
            if (currentValue != originalValue && currentValue != enabledValue) {
                throw new InvalidDataException("AI 經濟腳本目標值不是已知原版或修改值: " + scriptPath);
            }

            int targetValue = enabled ? enabledValue : originalValue;
            if (currentValue == targetValue) return;

            WriteBciInt32(decomp, patchOffset, targetValue);
            byte[] compressed = GameLZSS.CompressPfil(decomp, raw);
            SafeWriteAllBytes(scriptPath, compressed, rollback);
        }

        private void ApplyEndlessAiVillageEconomyPatch(string gamePath, bool enabled, FileRollbackScope? rollback) {
            string scriptRoot = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT");

            // Keep up to 20 free civilians available. As production buildings
            // consume workers, ak_npc replenishes the reserve with newborns.
            PatchEndlessAiEconomyScript(
                Path.Combine(scriptRoot, "ak_npc.bci"),
                new int?[] { 128, 43, 73, -2, 86, 66, null, 96, 99, 117, 476 },
                6,
                EndlessAiOriginalFreeCivilianReserve,
                EndlessAiUltimateFreeCivilianReserve,
                enabled,
                rollback);

            // This branch belongs to the NPC automatic-production path. The
            // patched unconditional jump lets staffed AI buildings continue
            // production even after their normal resource eligibility fails.
            PatchEndlessAiEconomyScript(
                Path.Combine(scriptRoot, "ak_produktion.bci"),
                new int?[] { 128, 69, 73, -2, 86, null, EndlessAiProductionGateJump, 66, 1, 82, 46 },
                5,
                EndlessAiProductionGateOriginalOpcode,
                EndlessAiProductionGateBypassOpcode,
                enabled,
                rollback);

            // Allow each main-house conversion pass to form up to 20 battle
            // units instead of using the original dynamic formation limit.
            PatchEndlessAiEconomyScript(
                Path.Combine(scriptRoot, "ak_haupthaus.bci"),
                new int?[] { null, null, 81, 11, 81, 10, 81, 98, 128, 81, 73, -4, 86 },
                0,
                EndlessAiFormationLimitOriginalOpcode,
                EndlessAiFormationLimitPatchedOpcode,
                enabled,
                rollback);
            PatchEndlessAiEconomyScript(
                Path.Combine(scriptRoot, "ak_haupthaus.bci"),
                new int?[] { null, null, 81, 11, 81, 10, 81, 98, 128, 81, 73, -4, 86 },
                1,
                EndlessAiFormationLimitOriginalValue,
                EndlessAiFormationLimitPatchedValue,
                enabled,
                rollback);
        }

        private static bool PatchEndlessLoopDelayLiterals(byte[] decompressedBci, bool enabled) {
            bool changed = false;
            int delaySiteIndex = 0;

            for (int offset = 0; offset <= decompressedBci.Length - 24; offset += 4) {
                if (BitConverter.ToInt32(decompressedBci, offset) != 0x42 ||
                    BitConverter.ToInt32(decompressedBci, offset + 8) != 0x42 ||
                    BitConverter.ToInt32(decompressedBci, offset + 16) != 0x80 ||
                    BitConverter.ToInt32(decompressedBci, offset + 20) != 16) {
                    continue;
                }

                int currentUpperMs = BitConverter.ToInt32(decompressedBci, offset + 4);
                int currentLowerMs = BitConverter.ToInt32(decompressedBci, offset + 12);
                if (delaySiteIndex >= EndlessAiLoopDelayRanges.Length) {
                    continue;
                }

                (int originalUpperMs, int originalLowerMs) = EndlessAiLoopDelayRanges[delaySiteIndex];
                bool matchesOriginal = currentUpperMs == originalUpperMs && currentLowerMs == originalLowerMs;
                bool matchesUltimate = currentUpperMs == EndlessAiUltimateLoopDelayUpperMs &&
                    currentLowerMs == EndlessAiUltimateLoopDelayLowerMs;
                if (!matchesOriginal && !matchesUltimate) {
                    continue;
                }

                bool accelerateReinforcementLoop = enabled && delaySiteIndex < EndlessAiReinforcementLoopCount;
                int targetUpperMs = accelerateReinforcementLoop ? EndlessAiUltimateLoopDelayUpperMs : originalUpperMs;
                int targetLowerMs = accelerateReinforcementLoop ? EndlessAiUltimateLoopDelayLowerMs : originalLowerMs;
                if (currentUpperMs != targetUpperMs || currentLowerMs != targetLowerMs) {
                    WriteBciInt32(decompressedBci, offset + 4, targetUpperMs);
                    WriteBciInt32(decompressedBci, offset + 12, targetLowerMs);
                    changed = true;
                }
                delaySiteIndex++;
            }

            return changed;
        }

        private static int FindEndlessActiveLimitSequenceOffset(byte[] decompressedBci) {
            int?[] pattern = new int?[] {
                0x5A, 0,
                0x42, null,
                96, 98,
                0x5B, 11,
                null, null,
                0x42, 0,
                0x42, 0,
                0x42, 0,
                0x42, 0,
                0x5A, 6,
                102, 117,
                32
            };

            int patternBytes = pattern.Length * 4;
            for (int offset = 0; offset <= decompressedBci.Length - patternBytes; offset += 4) {
                bool match = true;
                for (int i = 0; i < pattern.Length; i++) {
                    int? expected = pattern[i];
                    if (expected.HasValue && BitConverter.ToInt32(decompressedBci, offset + (i * 4)) != expected.Value) {
                        match = false;
                        break;
                    }
                }
                if (match) {
                    return offset;
                }
            }
            return -1;
        }

        private static bool PatchEndlessActiveAiLimit(byte[] decompressedBci, bool enabled) {
            int sequenceOffset = FindEndlessActiveLimitSequenceOffset(decompressedBci);
            if (sequenceOffset < 0) {
                return false;
            }

            int limitOffset = sequenceOffset + 12;
            int gateOffset = sequenceOffset + 32;
            int currentLimit = BitConverter.ToInt32(decompressedBci, limitOffset);
            int currentOpcode = BitConverter.ToInt32(decompressedBci, gateOffset);
            int currentValue = BitConverter.ToInt32(decompressedBci, gateOffset + 4);
            bool currentLimitKnown = currentLimit == EndlessAiOriginalActivePartyLimit ||
                currentLimit == EndlessAiUltimateActivePartyLimit;
            bool currentIsOriginal = currentOpcode == EndlessAiActiveLimitOriginalOpcode &&
                currentValue == EndlessAiActiveLimitOriginalValue;
            bool currentIsPatched = currentOpcode == EndlessAiActiveLimitPatchedOpcode &&
                currentValue == EndlessAiActiveLimitPatchedRelativeJump;
            if (!currentLimitKnown || (!currentIsOriginal && !currentIsPatched)) {
                return false;
            }

            bool changed = false;
            int targetLimit = enabled ? EndlessAiUltimateActivePartyLimit : EndlessAiOriginalActivePartyLimit;
            if (currentLimit != targetLimit) {
                WriteBciInt32(decompressedBci, limitOffset, targetLimit);
                changed = true;
            }

            // Always retain the original gate. This also migrates scripts patched
            // by older builds away from the unbounded jump.
            if (!currentIsOriginal) {
                WriteBciInt32(decompressedBci, gateOffset, EndlessAiActiveLimitOriginalOpcode);
                WriteBciInt32(decompressedBci, gateOffset + 4, EndlessAiActiveLimitOriginalValue);
                changed = true;
            }
            return changed;
        }

        private static bool HasExpectedEndlessLoopDelays(byte[] decompressedBci, bool enabled) {
            int expectedIndex = 0;
            for (int offset = 0; offset <= decompressedBci.Length - 24 && expectedIndex < EndlessAiLoopDelayRanges.Length; offset += 4) {
                if (BitConverter.ToInt32(decompressedBci, offset) != 0x42 ||
                    BitConverter.ToInt32(decompressedBci, offset + 8) != 0x42 ||
                    BitConverter.ToInt32(decompressedBci, offset + 16) != 0x80 ||
                    BitConverter.ToInt32(decompressedBci, offset + 20) != 16) {
                    continue;
                }

                (int originalUpperMs, int originalLowerMs) = EndlessAiLoopDelayRanges[expectedIndex];
                bool accelerated = enabled && expectedIndex < EndlessAiReinforcementLoopCount;
                int expectedUpperMs = accelerated ? EndlessAiUltimateLoopDelayUpperMs : originalUpperMs;
                int expectedLowerMs = accelerated ? EndlessAiUltimateLoopDelayLowerMs : originalLowerMs;
                if (BitConverter.ToInt32(decompressedBci, offset + 4) == expectedUpperMs &&
                    BitConverter.ToInt32(decompressedBci, offset + 12) == expectedLowerMs) {
                    expectedIndex++;
                }
            }
            return expectedIndex == EndlessAiLoopDelayRanges.Length;
        }

        private static bool HasPatchableEndlessLoopDelays(byte[] decompressedBci) {
            int expectedIndex = 0;
            for (int offset = 0; offset <= decompressedBci.Length - 24 && expectedIndex < EndlessAiLoopDelayRanges.Length; offset += 4) {
                if (BitConverter.ToInt32(decompressedBci, offset) != 0x42 ||
                    BitConverter.ToInt32(decompressedBci, offset + 8) != 0x42 ||
                    BitConverter.ToInt32(decompressedBci, offset + 16) != 0x80 ||
                    BitConverter.ToInt32(decompressedBci, offset + 20) != 16) {
                    continue;
                }

                (int originalUpperMs, int originalLowerMs) = EndlessAiLoopDelayRanges[expectedIndex];
                int currentUpperMs = BitConverter.ToInt32(decompressedBci, offset + 4);
                int currentLowerMs = BitConverter.ToInt32(decompressedBci, offset + 12);
                bool matchesOriginal = currentUpperMs == originalUpperMs && currentLowerMs == originalLowerMs;
                bool matchesUltimate = currentUpperMs == EndlessAiUltimateLoopDelayUpperMs &&
                    currentLowerMs == EndlessAiUltimateLoopDelayLowerMs;
                if (matchesOriginal || matchesUltimate) {
                    expectedIndex++;
                }
            }
            return expectedIndex == EndlessAiLoopDelayRanges.Length;
        }

        private bool TryReadEndlessAiModeState(string gamePath, out bool enabled) {
            enabled = false;
            string mapsPath = Path.Combine(gamePath, "MAPS");
            if (!Directory.Exists(mapsPath)) return false;

            string[] scripts = Directory.GetFiles(mapsPath, "ak_level.bci", SearchOption.AllDirectories)
                .Where(p => p.IndexOf(Path.DirectorySeparatorChar + "ENDL_", StringComparison.OrdinalIgnoreCase) >= 0)
                .ToArray();
            if (scripts.Length == 0) return false;

            bool? detectedState = null;
            foreach (string scriptPath in scripts) {
                byte[] decomp = GameLZSS.DecompressPfil(File.ReadAllBytes(scriptPath));
                int createOffset = FindEndlessMilitaryCreateUnitCall(decomp);
                int respawnOffset = FindEndlessRespawnDelayLiteral(decomp);
                int activeSequenceOffset = FindEndlessActiveLimitSequenceOffset(decomp);
                if (createOffset < 0 || respawnOffset < 0 || activeSequenceOffset < 0) {
                    return false;
                }

                int countMin = BitConverter.ToInt32(decomp, createOffset + 20);
                int countMax = BitConverter.ToInt32(decomp, createOffset + 28);
                int autoRecycleCompletedJob = BitConverter.ToInt32(decomp, createOffset + 4);
                int respawnDelay = BitConverter.ToInt32(decomp, respawnOffset);
                int activeLimit = BitConverter.ToInt32(decomp, activeSequenceOffset + 12);
                int gateOpcode = BitConverter.ToInt32(decomp, activeSequenceOffset + 32);
                int gateValue = BitConverter.ToInt32(decomp, activeSequenceOffset + 36);
                if (gateOpcode != EndlessAiActiveLimitOriginalOpcode || gateValue != EndlessAiActiveLimitOriginalValue) {
                    return false;
                }

                bool hasUltimateLoopDelays = HasExpectedEndlessLoopDelays(decomp, true);
                bool hasOriginalLoopDelays = HasExpectedEndlessLoopDelays(decomp, false);
                bool isUltimate = countMin == EndlessAiUltimateMilitaryCount &&
                    countMax == EndlessAiUltimateMilitaryCount &&
                    autoRecycleCompletedJob == EndlessAiUltimateAutoRecycleCompletedJob &&
                    respawnDelay == EndlessAiUltimateRespawnDelayMs &&
                    activeLimit == EndlessAiUltimateActivePartyLimit &&
                    (hasUltimateLoopDelays || hasOriginalLoopDelays);
                bool isLegacyUltimate = countMin == EndlessAiUltimateMilitaryCount &&
                    countMax == EndlessAiUltimateMilitaryCount &&
                    autoRecycleCompletedJob == EndlessAiOriginalAutoRecycleCompletedJob &&
                    respawnDelay == EndlessAiUltimateRespawnDelayMs &&
                    activeLimit == EndlessAiUltimateActivePartyLimit &&
                    hasOriginalLoopDelays;
                bool isOriginal = countMin == EndlessAiOriginalMilitaryCount &&
                    countMax == EndlessAiOriginalMilitaryCount &&
                    autoRecycleCompletedJob == EndlessAiOriginalAutoRecycleCompletedJob &&
                    respawnDelay == EndlessAiOriginalRespawnDelayMs &&
                    activeLimit == EndlessAiOriginalActivePartyLimit &&
                    hasOriginalLoopDelays;
                bool isEnabled = isUltimate || isLegacyUltimate;
                if (!isEnabled && !isOriginal) return false;
                if (detectedState.HasValue && detectedState.Value != isEnabled) return false;
                detectedState = isEnabled;
            }

            enabled = detectedState == true;
            return detectedState.HasValue;
        }

        private Dictionary<string, byte[]> GetPatchedEndlessScripts(string gamePath, bool enabled) {
            var results = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            string mapsPath = Path.Combine(gamePath, "MAPS");
            if (!Directory.Exists(mapsPath)) {
                string message = Loc.Get("LogEndlessAiNoMaps");
                if (enabled) throw new InvalidOperationException(message);
                Log(message);
                return results;
            }

            string[] scripts = Directory.GetFiles(mapsPath, "ak_level.bci", SearchOption.AllDirectories)
                .Where(p => p.IndexOf(Path.DirectorySeparatorChar + "ENDL_", StringComparison.OrdinalIgnoreCase) >= 0)
                .ToArray();
            if (scripts.Length == 0) {
                string message = Loc.Get("LogEndlessAiNoScripts");
                if (enabled) throw new InvalidOperationException(message);
                Log(message);
                return results;
            }

            int targetCount = enabled ? EndlessAiUltimateMilitaryCount : EndlessAiOriginalMilitaryCount;
            int targetAutoRecycleCompletedJob = enabled
                ? EndlessAiUltimateAutoRecycleCompletedJob
                : EndlessAiOriginalAutoRecycleCompletedJob;
            int targetRespawnDelayMs = enabled ? EndlessAiUltimateRespawnDelayMs : EndlessAiOriginalRespawnDelayMs;

            int patched = 0;
            var incompatibleScripts = new List<string>();
            foreach (string scriptPath in scripts) {
                byte[] raw = File.ReadAllBytes(scriptPath);
                byte[] decomp = GameLZSS.DecompressPfil(raw);
                int baseOffset = FindEndlessMilitaryCreateUnitCall(decomp);
                int respawnDelayOffset = FindEndlessRespawnDelayLiteral(decomp);
                int activeSequenceOffset = FindEndlessActiveLimitSequenceOffset(decomp);
                if (baseOffset < 0 || respawnDelayOffset < 0 || activeSequenceOffset < 0 ||
                    !HasPatchableEndlessLoopDelays(decomp)) {
                    incompatibleScripts.Add(scriptPath);
                    Log(string.Format(Loc.Get("LogEndlessAiPatternMissing"), scriptPath));
                    continue;
                }

                int currentMin = BitConverter.ToInt32(decomp, baseOffset + 20);
                int currentMax = BitConverter.ToInt32(decomp, baseOffset + 28);
                bool changed = false;
                int currentAutoRecycleCompletedJob = BitConverter.ToInt32(decomp, baseOffset + 4);
                int currentRespawnDelayMs = BitConverter.ToInt32(decomp, respawnDelayOffset);
                int currentActiveLimit = BitConverter.ToInt32(decomp, activeSequenceOffset + 12);
                int currentGateOpcode = BitConverter.ToInt32(decomp, activeSequenceOffset + 32);
                int currentGateValue = BitConverter.ToInt32(decomp, activeSequenceOffset + 36);
                bool countKnown = currentMin == currentMax &&
                    (currentMin == EndlessAiOriginalMilitaryCount || currentMin == EndlessAiUltimateMilitaryCount);
                bool autoRecycleKnown = currentAutoRecycleCompletedJob == EndlessAiOriginalAutoRecycleCompletedJob ||
                    currentAutoRecycleCompletedJob == EndlessAiUltimateAutoRecycleCompletedJob;
                bool respawnKnown = currentRespawnDelayMs == EndlessAiOriginalRespawnDelayMs ||
                    currentRespawnDelayMs == EndlessAiUltimateRespawnDelayMs;
                bool activeLimitKnown = currentActiveLimit == EndlessAiOriginalActivePartyLimit ||
                    currentActiveLimit == EndlessAiUltimateActivePartyLimit;
                bool gateKnown = (currentGateOpcode == EndlessAiActiveLimitOriginalOpcode &&
                    currentGateValue == EndlessAiActiveLimitOriginalValue) ||
                    (currentGateOpcode == EndlessAiActiveLimitPatchedOpcode &&
                    currentGateValue == EndlessAiActiveLimitPatchedRelativeJump);
                if (!countKnown || !autoRecycleKnown || !respawnKnown || !activeLimitKnown || !gateKnown) {
                    incompatibleScripts.Add(scriptPath);
                    Log(string.Format(Loc.Get("LogEndlessAiPatternMissing"), scriptPath));
                    continue;
                }
                if (currentAutoRecycleCompletedJob != targetAutoRecycleCompletedJob) {
                    WriteBciInt32(decomp, baseOffset + 4, targetAutoRecycleCompletedJob);
                    changed = true;
                }
                if (currentMin != targetCount || currentMax != targetCount) {
                    WriteBciInt32(decomp, baseOffset + 20, targetCount);
                    WriteBciInt32(decomp, baseOffset + 28, targetCount);
                    changed = true;
                }

                if (currentRespawnDelayMs != targetRespawnDelayMs) {
                    WriteBciInt32(decomp, respawnDelayOffset, targetRespawnDelayMs);
                    changed = true;
                }

                if (PatchEndlessLoopDelayLiterals(decomp, enabled)) {
                    changed = true;
                }

                if (PatchEndlessActiveAiLimit(decomp, enabled)) {
                    changed = true;
                }

                if (changed) {
                    byte[] compressed = GameLZSS.CompressPfil(decomp, raw);
                    results[scriptPath] = compressed;
                    patched++;
                }
            }

            if (enabled && incompatibleScripts.Count > 0) {
                throw new InvalidOperationException(string.Format(
                    "AI 終極模式偵測到 {0} 個不相容的無盡模式腳本，已取消整批套用。",
                    incompatibleScripts.Count));
            }

            if (enabled) {
                Log(string.Format(Loc.Get("LogEndlessAiUltimateApplied"), patched, targetCount));
            } else {
                Log(string.Format(Loc.Get("LogEndlessAiUltimateRestored"), patched, targetCount));
            }

            return results;
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
    }
}
