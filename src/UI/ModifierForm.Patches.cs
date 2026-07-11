using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Windows.Forms;
using System.Threading.Tasks;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private static readonly object LogLock = new object();
        private static bool _logRotationChecked;
        private const long LogRotationThresholdBytes = 5 * 1024 * 1024;

        private void BuildFeatureToggleMap() {
            featureToggles = new Dictionary<string, ModernToggle>(StringComparer.OrdinalIgnoreCase) {
                ["FocusLoss"] = chkFocusLoss, ["FastCiviProduction"] = chkFastCiviProduction,
                ["InfiniteMorale"] = chkInfiniteMorale, ["FreeProduction"] = chkFreeProd,
                ["FreeUpgrade"] = chkFreeUpgrade, ["NoSpellCost"] = chkNoSpellCost,
                ["MaxPopulation"] = chkMaxPopulation, ["Balance"] = chkBalance,
                ["RangedRange3x"] = chkRangedRange3x, ["UnitMovementSpeed2x"] = chkUnitMovementSpeed2x,
                ["SpellEntireMap"] = chkSpellEntireMap, ["SpellRange3x"] = chkSpellRange3x,
                ["ProjectileArcHeight"] = chkProjectileArcHeight, ["RangedAccuracy"] = chkRangedAccuracy,
                ["HousingCapacity20x"] = chkHousingCapacity20x, ["StorageCapacity10x"] = chkStorageCapacity10x,
                ["HqHp10x"] = chkHqHp10x, ["FastBuildUpgradeRepair"] = chkFastBuildUpgradeRepair,
                ["FoodHealing10x"] = chkFoodHealing10x, ["VillageBuildRange"] = chkVillageBuildRange,
                ["NoSpellAltar"] = chkNoSpellAltar, ["DgVoodoo"] = chkDgVoodoo,
                ["ToEnglish"] = chkToEng,
                ["SpellDamage5x"] = chkSpellDamage5x,
                ["SpellHealing10x"] = chkSpellHealing10x,
                ["SpellResurrection"] = chkSpellResurrection,
                ["GeneralSkills"] = chkGeneralSkills,
                ["LeaderGlory"] = chkLeaderGlory,
                ["EndlessAi.M1"] = chkAiM1, ["EndlessAi.M2"] = chkAiM2, ["EndlessAi.M3"] = chkAiM3,
                ["EndlessAi.M4"] = chkAiM4, ["EndlessAi.M5"] = chkAiM5, ["EndlessAi.M6"] = chkAiM6,
            };
        }

        private void ResetTogglesForCategory(FeatureCategory category, string gamePath) {
            foreach (FeatureDefinition feature in FeatureRegistry.ByCategory(category)) {
                if (featureToggles.TryGetValue(feature.Id, out ModernToggle? toggle)) toggle.Checked = false;
            }
            if (category == FeatureCategory.Compat) {
                chkGameSpeed.Checked = false;
                chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(gamePath);
            }
        }

        /// <summary>
        /// 記錄日誌訊息並寫入至本地 modifier_log.txt 檔案。
        /// 每次程式執行的第一筆日誌前檢查檔案大小，超過 5MB 即輪替為 modifier_log.old.txt，
        /// 避免日誌無上限成長。
        /// </summary>
        private void Log(string message) {
            string text = string.Format("[{0}] {1}\r\n", DateTime.Now.ToString("HH:mm:ss"), message);
            try {
                lock (LogLock) {
                    string logPath = Path.Combine(AppContext.BaseDirectory, "modifier_log.txt");
                    if (!_logRotationChecked) {
                        _logRotationChecked = true;
                        try {
                            var logInfo = new FileInfo(logPath);
                            if (logInfo.Exists && logInfo.Length > LogRotationThresholdBytes) {
                                string oldPath = Path.Combine(AppContext.BaseDirectory, "modifier_log.old.txt");
                                if (File.Exists(oldPath)) {
                                    File.Delete(oldPath);
                                }
                                File.Move(logPath, oldPath);
                            }
                        } catch (Exception rotateEx) {
                            System.Diagnostics.Debug.WriteLine("日誌輪替失敗: " + rotateEx.Message);
                        }
                    }
                    File.AppendAllText(logPath, text, Encoding.UTF8);
                }
            } catch (Exception ex) {
                System.Diagnostics.Debug.WriteLine("日誌檔案寫入失敗: " + ex.Message);
            }
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
                    backupManager.EnsureBackupLoadedForGamePath(fbd.SelectedPath);
                    chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(fbd.SelectedPath);
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
            if (!backupManager.EnsureBackupLoadedForGamePath(gamePath)) {
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

                var profile = new PatchProfile();
                foreach (var (id, toggle) in featureToggles) profile.Set(id, FeatureValue.Of(toggle.Checked));
                profile.Set("GameSpeed", FeatureValue.Of(chkGameSpeed.Checked ? 10 : 1));
                profile.Set("CustomUnitStats", FeatureValue.Of(this.customUnitStats));

                await Task.Run(() => {
                    patchEngine.ApplyPatches(gamePath, profile, backupManager, rollback);
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

        /// <summary>
        /// 將所有遊戲設定（屬性、相容性、語言包）恢復為官方原版初始設定。
        /// </summary>
        private async void RestoreAll() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath) || !File.Exists(Path.Combine(gamePath, "Against_Rome.exe"))) {
                MessageBox.Show(Loc.Get("MsgWrongGameDir"), Loc.Get("TitlePathError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            if (!backupManager.EnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreAll"));
                rollback = new FileRollbackScope();
                Log("已建立還原前檔案回復點。");
                await Task.Run(() => {
                    patchEngine.RestoreOriginalFiles(gamePath, backupManager, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;
                
                ResetTogglesForCategory(FeatureCategory.Stats, gamePath);
                ResetTogglesForCategory(FeatureCategory.Compat, gamePath);
                ResetTogglesForCategory(FeatureCategory.Language, gamePath);

                Log(Loc.Get("LogRestoreAllDone"));
                MessageBox.Show(Loc.Get("MsgRestoreAllSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復變更。");
                    rollback.RestoreAll(Log);
                    Log("還原失敗後的回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
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

        /// <summary>設定遊戲 10 倍加速開關的狀態。</summary>
        private void SetGameSpeedSelection(int multiplier) {
            chkGameSpeed.Checked = multiplier > 1;
        }

        private async void RestoreStatsOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            if (!backupManager.EnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }
            FileRollbackScope? rollback = null;
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreStats"));
                rollback = new FileRollbackScope();
                Log("已建立屬性檔案回復點。");
                await Task.Run(() => {
                    patchEngine.RestoreStatsOnly(gamePath, backupManager, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;

                ResetTogglesForCategory(FeatureCategory.Stats, gamePath);

                Log(Loc.Get("LogRestoreStatsDone"));
                MessageBox.Show(Loc.Get("MsgRestoreStatsSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful stats restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復變更。");
                    rollback.RestoreAll(Log);
                    Log("還原失敗後的回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

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
                Log("已建立相容性檔案回復點。");
                await Task.Run(() => {
                    patchEngine.RestoreCompatOnly(gamePath, backupManager, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;

                ResetTogglesForCategory(FeatureCategory.Compat, gamePath);

                Log(Loc.Get("LogRestoreCompatDone"));
                MessageBox.Show(Loc.Get("MsgRestoreCompatSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful compat restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復變更。");
                    rollback.RestoreAll(Log);
                    Log("還原失敗後的回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }

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
                Log("已建立語言檔案回復點。");
                await Task.Run(() => {
                    patchEngine.RestoreLanguageOnly(gamePath, rollback);
                });
                rollback.Commit();
                rollback.Dispose();
                rollback = null;

                ResetTogglesForCategory(FeatureCategory.Language, gamePath);

                Log(Loc.Get("LogRestoreLangDone"));
                MessageBox.Show(Loc.Get("MsgRestoreLangSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful language restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                if (rollback != null && !rollback.IsCommitted) {
                    Log("還原失敗，開始回復變更。");
                    rollback.RestoreAll(Log);
                    Log("還原失敗後的回復流程已完成。");
                }
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                rollback?.Dispose();
                SetActionButtonsEnabled(true);
            }
        }
    }
}
