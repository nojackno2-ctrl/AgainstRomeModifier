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
        private static readonly object LogLock = new object();
        private static readonly Regex RegexRadiusPatch = new Regex(@"^Radius\s*=\s*([A-Z]{3})\s*,\s*(Spell\d+)\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexCiviPatch = new Regex(@"^CiviDelay\s*=\s*([A-Z]{3})\s*,\s*([^;]+)(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleLostMemPatch = new Regex(@"^(MoralsDecLostMem\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleFleePatch = new Regex(@"^(MoralsDecFlee\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleOverPopPatch = new Regex(@"^(MoralsDecOverPop\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly Regex RegexMoraleIncIdlePatch = new Regex(@"^(MoralsIncIdle\s*=\s*[A-Z]{3}\s*,\s*)\d+(.*)$", RegexOptions.Compiled);
        private static readonly byte[] ExeFocusOriginalBytes = new byte[] { 0x89, 0x15, 0xC4, 0x7D, 0x9E, 0x02 };
        private static readonly byte[] ExeFocusPatchedBytes = new byte[] { 0x90, 0x90, 0x90, 0x90, 0x90, 0x90 };
        private const long ExeFocusPatchOffset = 0x161a88;
        private const long ExeFocusPatchRequiredLength = 0x161a8e;
        private static readonly string[] HorseCivilProductionIconIds = new string[] {
            "VerGerZivIco01_Zivil_Icon",
            "VerHunZivIco01_Zivil_Icon",
            "VerKelZivIco01_Zivil_Icon",
            "VerRomZivIco01_Zivil_Icon"
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

            public void RestoreAll(Action<string>? log) {
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
            }

            public void Dispose() {
                if (_committed || Directory.Exists(_rootPath)) {
                    try {
                        Directory.Delete(_rootPath, true);
                    } catch {
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

            FileRollbackScope? rollback = null;
            try {
                DialogResult confirm = MessageBox.Show(Loc.Get("MsgConfirmApply"), Loc.Get("TitleConfirm"), MessageBoxButtons.YesNo, MessageBoxIcon.Question);
                if (confirm != DialogResult.Yes) return;

                SetActionButtonsEnabled(false);
                Log(Loc.Get("LogStartApply"));
                rollback = new FileRollbackScope();
                Log("已建立修改前檔案回復點。");

                bool focusLoss = chkFocusLoss.Checked;
                decimal civiSpeed = numCiviSpeed.Value;
                bool infMorale = chkInfiniteMorale.Checked;
                bool freeProd = chkFreeProd.Checked;
                bool freeUp = chkFreeUpgrade.Checked;
                bool noSpell = chkNoSpellCost.Checked;
                decimal popLimit = numPopLimit.Value;
                bool balance = chkBalance.Checked;
                bool toEng = chkToEng.Checked;

                await Task.Run(() => {
                    ApplyExePatch(gamePath, focusLoss, rollback);
                    ApplyClScriptPatch(gamePath, civiSpeed, infMorale, balance, rollback);
                    ApplyRessPatch(gamePath, freeProd, freeUp, noSpell, popLimit, rollback);
                    ApplyObjdefPatch(gamePath, balance, rollback);
                    RestoreTeamFiles(gamePath, rollback);
                    ApplyTeamDatPatch(gamePath, popLimit, rollback);
                    ApplyLanguagePatch(gamePath, toEng, rollback);
                });

                rollback.Commit();
                Log(Loc.Get("LogApplyAllSuccess"));
                MessageBox.Show(Loc.Get("MsgApplySuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null) {
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

        private static string BuildHorseCivilProductionCostRow(string id) {
            string[] cols = new string[30];
            for (int i = 0; i < cols.Length - 1; i++) {
                cols[i] = "0";
            }
            cols[cols.Length - 1] = "";
            cols[0] = id;
            cols[(int)RessIndex.FigProdCostEnd] = "1";
            cols[(int)RessIndex.FigEquipmentRefundEnd] = "1";
            return ToCsvString(cols);
        }

        private static void EnsureHorseCivilProductionCostRows(List<string> lines) {
            int insertIndex = lines.FindIndex(line => line.Trim().Equals("[maxskills]", StringComparison.OrdinalIgnoreCase));
            if (insertIndex < 0) {
                insertIndex = lines.Count;
            }

            foreach (string id in HorseCivilProductionIconIds) {
                bool exists = lines.Any(line => line.StartsWith(id + ",", StringComparison.OrdinalIgnoreCase));
                if (!exists) {
                    lines.Insert(insertIndex, BuildHorseCivilProductionCostRow(id));
                    insertIndex++;
                }
            }
        }

        private static bool ShouldZeroFigFreeProductionField(int index) {
            return (index >= (int)RessIndex.FigProdCostStart && index <= (int)RessIndex.FigProdCostEnd) ||
                   (index >= (int)RessIndex.FigEquipmentRefundStart && index <= (int)RessIndex.FigEquipmentRefundEnd);
        }

        /// <summary>
        /// 將所有遊戲設定（屬性、相容性、語言包）恢復為官方原版初始設定。
        /// </summary>
        private async void RestoreAll() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreAll"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => {
                    RestoreStatsOnlyInternal(gamePath, rollback);
                    ApplyExePatch(gamePath, false, rollback);
                    ApplyLanguagePatch(gamePath, false, rollback);
                });
                rollback.Commit();
                chkFocusLoss.Checked = false;
                chkToEng.Checked = false;
                customUnitStats = null;
                presetFileSourceType = "default";
                presetFileName = "";
                UpdateTroopPresetLabel();
                LoadDefaultStatsData(); // 重新載入表格以呈現原版
                Log(Loc.Get("LogRestoreAllDone"));
                MessageBox.Show(Loc.Get("MsgRestoreAllSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null) {
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
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreStats"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => RestoreStatsOnlyInternal(gamePath, rollback));
                rollback.Commit();
                customUnitStats = null;
                presetFileSourceType = "default";
                presetFileName = "";
                UpdateTroopPresetLabel();
                LoadDefaultStatsData(); // 重新載入表格以呈現原版
                Log(Loc.Get("LogRestoreStatsDone"));
                MessageBox.Show(Loc.Get("MsgRestoreStatsSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null) {
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
                    ApplyExePatch(gamePath, false, rollback);
                });
                rollback.Commit();
                chkFocusLoss.Checked = false;
                Log(Loc.Get("LogRestoreCompatDone"));
                MessageBox.Show(Loc.Get("MsgRestoreCompatSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null) {
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
                chkToEng.Checked = false;
                Log(Loc.Get("LogRestoreLangDone"));
                MessageBox.Show(Loc.Get("MsgRestoreLangSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                if (rollback != null) {
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
        private void ApplyLanguagePatch(string gamePath, bool toEnglish, FileRollbackScope? rollback = null) {
            string localToEngDir = Path.Combine(gamePath, "ToEng");

            if (toEnglish) {
                if (!Directory.Exists(localToEngDir)) {
                    Log(Loc.Get("LogNoToEngDir"));
                    return;
                }

                string[] files = Directory.GetFiles(localToEngDir, "*", SearchOption.AllDirectories);
                foreach (string file in files) {
                    string relPath = file.Substring(localToEngDir.Length + 1);
                    string destPath = Path.Combine(gamePath, relPath);
                    SafeCopyFile(file, destPath, true, rollback);
                }
                Log(Loc.Get("LogLangToEng"));
                return;
            }

            var restoreKeys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            if (Directory.Exists(localToEngDir)) {
                string[] files = Directory.GetFiles(localToEngDir, "*", SearchOption.AllDirectories);
                foreach (string file in files) {
                    string relPath = file.Substring(localToEngDir.Length + 1).Replace('\\', '/');
                    if (backupFiles.ContainsKey(relPath)) {
                        restoreKeys.Add(relPath);
                    }
                }
            }

            if (restoreKeys.Count == 0 && backupFiles.ContainsKey("SYSTEM/CLMK/icon.ini")) {
                restoreKeys.Add("SYSTEM/CLMK/icon.ini");
            }

            foreach (string key in restoreKeys) {
                RestoreMemoryFile(key, Path.Combine(gamePath, key.Replace('/', '\\')), rollback);
            }

            if (restoreKeys.Count == 0) {
                Log("找不到可從內嵌備份還原的語系檔案。");
            } else {
                Log(Loc.Get("LogLangToOrig"));
            }
        }

        private enum ExePatchState {
            Unknown,
            Original,
            FocusPatched
        }

        private ExePatchState GetExePatchState(string exePath) {
            if (!File.Exists(exePath)) return ExePatchState.Unknown;
            using (var fs = new FileStream(exePath, FileMode.Open, FileAccess.Read, FileShare.Read)) {
                if (fs.Length < ExeFocusPatchRequiredLength) {
                    return ExePatchState.Unknown;
                }
                fs.Seek(ExeFocusPatchOffset, SeekOrigin.Begin);
                byte[] bytes = new byte[ExeFocusOriginalBytes.Length];
                int read = fs.Read(bytes, 0, bytes.Length);
                if (read != bytes.Length) return ExePatchState.Unknown;
                if (bytes.SequenceEqual(ExeFocusOriginalBytes)) return ExePatchState.Original;
                if (bytes.SequenceEqual(ExeFocusPatchedBytes)) return ExePatchState.FocusPatched;
            }
            return ExePatchState.Unknown;
        }

        private void WriteExePatchBytes(string exePath, byte[] patchBytes, FileRollbackScope? rollback) {
            rollback?.TrackFile(exePath);
            using (var fs = new FileStream(exePath, FileMode.Open, FileAccess.ReadWrite, FileShare.None)) {
                fs.Seek(ExeFocusPatchOffset, SeekOrigin.Begin);
                fs.Write(patchBytes, 0, patchBytes.Length);
            }
        }

        /// <summary>
        /// 修改 Against_Rome.exe 實體檔案，實現視窗失焦不自動暫停的功能。
        /// </summary>
        private void ApplyExePatch(string gamePath, bool focusLossChecked, FileRollbackScope? rollback = null) {
            string dest = Path.Combine(gamePath, @"Against_Rome.exe");
            ExePatchState state = GetExePatchState(dest);
            if (state == ExePatchState.Unknown) {
                throw new Exception("Against_Rome.exe 版本或位元組特徵不符合預期，已停止相容性補丁以避免覆蓋未知版本。");
            }

            if (focusLossChecked) {
                if (state == ExePatchState.Original) {
                    WriteExePatchBytes(dest, ExeFocusPatchedBytes, rollback);
                }
                Log(Loc.Get("LogExePatchFocus"));
            } else {
                if (state == ExePatchState.FocusPatched) {
                    WriteExePatchBytes(dest, ExeFocusOriginalBytes, rollback);
                }
                Log(Loc.Get("LogExePatchOrig"));
            }
        }

        /// <summary>
        /// 修改 cl_script.ini 檔案，自訂村民產生速度、法術影響半徑以及無限士氣等功能。
        /// </summary>
        private void ApplyClScriptPatch(string gamePath, decimal civiSpeedVal, bool infiniteMoraleChecked, bool balanceChecked, FileRollbackScope? rollback = null) {
            string dest = Path.Combine(gamePath, @"SYSTEM\cl_script.ini");
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/cl_script.ini", out origBytes)) return;

            bool hasCustomSpellRadius = customUnitStats != null && customUnitStats.Any(kvp =>
                (kvp.Key.Equals("FigGerPri00_Priester", StringComparison.OrdinalIgnoreCase) ||
                 kvp.Key.Equals("FigKelPri00_Priester", StringComparison.OrdinalIgnoreCase) ||
                 kvp.Key.Equals("FigHunPri00_Priester", StringComparison.OrdinalIgnoreCase)) &&
                kvp.Value.Length > 8);
            bool hasMod = (civiSpeedVal != 1.0M) || infiniteMoraleChecked || balanceChecked || hasCustomSpellRadius;

            if (!hasMod) {
                SafeWriteAllBytes(dest, origBytes!, rollback);
                Log("[已還原原版] cl_script.ini");
                return;
            }

            byte[] decompBytes = GameLZSS.DecompressPfil(origBytes!);
            string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
            string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);

            double gerMult = balanceChecked ? 2.5 : 1.0;
            double kelMult = balanceChecked ? 2.5 : 1.0;
            double hunMult = balanceChecked ? 2.5 : 1.0;

            if (customUnitStats != null) {
                if (customUnitStats.ContainsKey("FigGerPri00_Priester") && customUnitStats["FigGerPri00_Priester"].Length > 8) {
                    gerMult = customUnitStats["FigGerPri00_Priester"][8] / 500.0;
                }
                if (customUnitStats.ContainsKey("FigKelPri00_Priester") && customUnitStats["FigKelPri00_Priester"].Length > 8) {
                    kelMult = customUnitStats["FigKelPri00_Priester"][8] / 500.0;
                }
                if (customUnitStats.ContainsKey("FigHunPri00_Priester") && customUnitStats["FigHunPri00_Priester"].Length > 8) {
                    hunMult = customUnitStats["FigHunPri00_Priester"][8] / 500.0;
                }
            }
            double civiSpeed = (double)civiSpeedVal;

            var newLines = new List<string>();
            foreach (string line in lines) {
                string processedLine = line;
                var match = RegexRadiusPatch.Match(line);
                if (match.Success) {
                    string volk = match.Groups[1].Value;
                    string spell = match.Groups[2].Value;
                    string valStr = match.Groups[3].Value.Trim();
                    string comment = match.Groups[4].Value;
                    double val;
                    if (double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out val)) {
                        string volkClean = volk.Trim();
                        double mult = 1.0;
                        if (volkClean == "GER") mult = gerMult;
                        else if (volkClean == "KEL") mult = kelMult;
                        else if (volkClean == "HUN") mult = hunMult;
                        int newVal = (int)(val * mult);
                        processedLine = string.Format("Radius     ={0}, {1}, {2,-10}{3}", volk, spell, newVal, comment);
                    }
                }

                var matchCivi = RegexCiviPatch.Match(line);
                if (matchCivi.Success) {
                    string volk = matchCivi.Groups[1].Value;
                    string valStr = matchCivi.Groups[2].Value.Trim();
                    string comment = matchCivi.Groups[3].Value;
                    double val;
                    if (double.TryParse(valStr, NumberStyles.Any, CultureInfo.InvariantCulture, out val)) {
                        int newVal = (int)(5000 / civiSpeed);
                        if (newVal < 100) newVal = 100;
                        processedLine = string.Format("CiviDelay  ={0}, {1,-10}{2}", volk, newVal, comment);
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
                        if (m.Success) processedLine = m.Groups[1].Value + "1" + m.Groups[2].Value;
                    }
                }

                newLines.Add(processedLine);
            }

            string newContent = string.Join(lineEnding, newLines.ToArray());
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            byte[] compressed = GameLZSS.CompressPfil(newBytes, origBytes!);

            SafeWriteAllBytes(dest, compressed, rollback);
            Log("已成功修改與寫入 cl_script.ini (法術半徑與村民生產速度)。");
        }

        /// <summary>
        /// 修改 ress.ini 檔案，設定建築/部隊生產與升級的免費資源、移除祭司施法冷卻/消耗、以及全地圖人口上限。
        /// </summary>
        private void ApplyRessPatch(string gamePath, bool freeProdChecked, bool freeUpgradeChecked, bool noSpellCostChecked, decimal popLimitVal, FileRollbackScope? rollback = null) {
            string dest = Path.Combine(gamePath, @"SYSTEM\ress.ini");
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/ress.ini", out origBytes)) return;

            bool hasMod = freeProdChecked || freeUpgradeChecked || noSpellCostChecked || (popLimitVal != 100);

            if (!hasMod) {
                SafeWriteAllBytes(dest, origBytes!, rollback);
                Log("[已還原原版] ress.ini");
                return;
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
                                } else if (freeUpgradeChecked && (
                                    i == (int)RessIndex.BauUpgradeWood || 
                                    i == (int)RessIndex.BauUpgradeStone || 
                                    i == (int)RessIndex.BauUpgradeGold || 
                                    i == (int)RessIndex.BauUpgradeIron)) {
                                    newCols.Add("0");
                                } else if (freeProdChecked && (
                                    i == (int)RessIndex.BauBuildWood || 
                                    i == (int)RessIndex.BauBuildStone)) {
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
                        if (i == (int)RessIndex.PopLimit) {
                            newCols.Add(popLimitVal.ToString());
                        } else if (freeUpgradeChecked && (
                            i == 8 || i == 10 || i == 12 || i == 14 ||
                            (i >= 24 && i <= 263 && i % 2 == 0) ||
                            (i >= (int)RessIndex.VolkresUpgradeStart && i <= (int)RessIndex.VolkresUpgradeEnd)
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

            if (freeProdChecked) {
                EnsureHorseCivilProductionCostRows(newLines);
            }

            string newContent = string.Join(lineEnding, newLines.ToArray());
            if (decomp.EndsWith(lineEnding) && !newContent.EndsWith(lineEnding)) {
                newContent += lineEnding;
            }

            byte[] newBytes = Encoding.GetEncoding(1251).GetBytes(newContent);
            byte[] compressed = GameLZSS.CompressPfil(newBytes, origBytes);

            SafeWriteAllBytes(dest, compressed, rollback);
            Log("已成功修改與寫入 ress.ini (免費建造與人口)。");
        }

        /// <summary>
        /// 修改 objdef.dau 檔案，套用部隊屬性平衡模式、自訂部隊移動速度、射程、技能距離、近戰/遠程傷害與攻擊冷卻等倍率。
        /// </summary>
        private void ApplyObjdefPatch(string gamePath, bool balanceChecked, FileRollbackScope? rollback = null) {
            string dest = Path.Combine(gamePath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau");
            byte[]? origBytes;
            if (!backupFiles.TryGetValue("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", out origBytes)) return;

            bool hasMod = balanceChecked || (customUnitStats != null && customUnitStats.Count > 0);

            if (!hasMod) {
                SafeWriteAllBytes(dest, origBytes!, rollback);
                Log("[已還原原版] objdef.dau");
                return;
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

                    bool isPriest = utype == "priest";

                    double origRange = GetUnitMaxRange(origCols, utype);
                    double origMeleeRelt = 0, origRangedRelt = 0;
                    GetMeleeAndRangedRelt(origCols, utype, out origMeleeRelt, out origRangedRelt);
                    double origPrimaryRelt = origMeleeRelt;
                    if (utype == "ranged_inf" || utype == "ranged_cav") {
                        origPrimaryRelt = origRangedRelt;
                    } else if (utype == "siege") {
                        origPrimaryRelt = Math.Max(origMeleeRelt, origRangedRelt);
                    }

                    // 從 bal 陣列中讀取 9 大屬性的值 (bal 之前在 GetBaseStatsForUnit 已經載入)
                    // bal = [HP, Dmg, VW, AW, Speed, Sight, Relt, Range, SpellRadius]
                    double customSpeed = bal[4];
                    double customSight = bal[5];
                    double customRelt = bal[6];
                    double customRange = bal[7];

                    double speedMult = 1.0;
                    if (origMoves > 0 && customSpeed > 0) {
                        speedMult = customSpeed / (origMoves * 2.0);
                    }

                    int newSight = (int)customSight;

                    double rangeMult = 1.0;
                    if (origRange > 0 && customRange > 0) {
                        rangeMult = customRange / origRange;
                    }

                    double reltScale = 1.0;
                    if (origPrimaryRelt > 0 && customRelt > 0) {
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

                    if (isPriest) {
                        int[] priestFields = { (int)ObjdefIndex.PriestSpell1, (int)ObjdefIndex.PriestSpell2, (int)ObjdefIndex.PriestSpell3 };
                        foreach (int f in priestFields) {
                            double val;
                            if (double.TryParse(origCols[f], NumberStyles.Any, CultureInfo.InvariantCulture, out val) && val > 0) {
                                int newVal = (int)(val * rangeMult);
                                patchActions.Add(Tuple.Create(f, newVal.ToString(), "技能距離"));
                            }
                        }
                    } else {
                        int[] idxFields = { (int)ObjdefIndex.Weapon2RangeMin, (int)ObjdefIndex.Weapon2RangeMax, (int)ObjdefIndex.Weapon3RangeMin, (int)ObjdefIndex.Weapon3RangeMax };
                        foreach (int idxField in idxFields) {
                            double val;
                            if (double.TryParse(origCols[idxField], NumberStyles.Any, CultureInfo.InvariantCulture, out val) && val > 0) {
                                int newVal = (int)(val * rangeMult);
                                patchActions.Add(Tuple.Create(idxField, newVal.ToString(), "射程"));
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

                        if (cols[aktiIdx].Trim() == "1") {
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

                    bool hasError = false;
                    var updatedValues = new Dictionary<int, string>();
                    foreach (var action in patchActions) {
                        string finalVal;
                        int targetLen = cols[action.Item1].Length;
                        if (!CheckLen(action.Item2, targetLen, out finalVal)) {
                            Log(string.Format("[警告] 單位 {0} 的 {1} 數值 {2} 超出欄位長度限制 {3}，已略過修改。", name, action.Item3, action.Item2, targetLen));
                            hasError = true;
                            break;
                        }
                        updatedValues[action.Item1] = finalVal.PadLeft(targetLen);
                    }

                    if (hasError) continue;

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

                    bool hasError = false;
                    var updatedValues = new Dictionary<int, string>();
                    foreach (var action in patchActions) {
                        string finalVal;
                        int targetLen = cols[action.Item1].Length;
                        if (!CheckLen(action.Item2, targetLen, out finalVal)) {
                            Log(string.Format("[警告] 單位 {0} 的 {1} 數值 {2} 超出欄位長度限制 {3}，已略過修改。", name, action.Item3, action.Item2, targetLen));
                            hasError = true;
                            break;
                        }
                        updatedValues[action.Item1] = finalVal.PadLeft(targetLen);
                    }

                    if (hasError) continue;

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
            byte[] compressed = GameLZSS.CompressPfil(newBytes, origBytes!);

            SafeWriteAllBytes(dest, compressed, rollback);
            Log("已成功修改與寫入 objdef.dau (部隊個別屬性與倍率自訂)。");
        }

        /// <summary>
        /// 還原地圖目錄下所有的 team.dat 檔案。
        /// </summary>
        private void RestoreTeamFiles(string gamePath, FileRollbackScope? rollback = null) {
            foreach (var kvp in backupFiles) {
                if (kvp.Key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) && kvp.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase)) {
                    string destPath = Path.Combine(gamePath, kvp.Key.Replace('/', '\\'));
                    SafeWriteAllBytes(destPath, kvp.Value, rollback);
                }
            }
            Log("[已還原原版] 所有 team.dat 檔案。");
        }

        /// <summary>
        /// 修改各地圖目錄下的 team.dat，使其中定義的人口上限與主程式或 UI 界面上設定的人口數相符。
        /// </summary>
        private void ApplyTeamDatPatch(string gamePath, decimal popLimitVal, FileRollbackScope? rollback = null) {
            string[] teamFiles = Directory.GetFiles(Path.Combine(gamePath, "MAPS"), "team.dat", SearchOption.AllDirectories);
            int popLimit = (int)popLimitVal;
            Parallel.ForEach(teamFiles, file => {
                string mapKey = file.Substring(gamePath.Length + 1).Replace('\\', '/');
                byte[]? origBytes;
                if (!backupFiles.TryGetValue(mapKey, out origBytes) || origBytes == null) {
                    Log(string.Format("[警告] 記憶體備份中找不到 {0}，略過此 team.dat。", mapKey));
                    return;
                }
                byte[] decompBytes = GameLZSS.DecompressPfil(origBytes);
                string decomp = Encoding.GetEncoding(1251).GetString(decompBytes);
                string lineEnding = decomp.Contains("\r\n") ? "\r\n" : "\n";
                string[] lines = decomp.Split(new string[] { lineEnding }, StringSplitOptions.None);
                var newLines = new System.Collections.Generic.List<string>();
                bool inMaxTeamObj = false;
                bool inTeamData = false;
                foreach (string line in lines) {
                    string stripped = line.Trim();
                    if (stripped.StartsWith("[")) {
                        inMaxTeamObj = (stripped == "[maxteamobjgenerell]");
                        inTeamData = (stripped == "[teamdata]");
                        newLines.Add(line);
                        continue;
                    }
                    if (inMaxTeamObj && !string.IsNullOrEmpty(stripped)) {
                        newLines.Add(popLimit.ToString());
                        inMaxTeamObj = false;
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
                byte[] compressed = GameLZSS.CompressPfil(newBytes, origBytes);
                SafeWriteAllBytes(file, compressed, rollback);
            });
            Log(string.Format("已修改所有地圖的 team.dat 人口上限為 {0}。", popLimit));
        }
    }
}
