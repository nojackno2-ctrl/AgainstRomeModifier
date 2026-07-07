using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Windows.Forms;
using System.Threading.Tasks;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private static readonly object LogLock = new object();

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

                // 收集 UI 設定至 PatchOptions 模型中
                var options = new PatchOptions {
                    FocusLoss = chkFocusLoss.Checked,
                    FastCiviProduction = chkFastCiviProduction.Checked,
                    InfiniteMorale = chkInfiniteMorale.Checked,
                    FreeProduction = chkFreeProd.Checked,
                    FreeUpgrade = chkFreeUpgrade.Checked,
                    NoSpellCost = chkNoSpellCost.Checked,
                    MaxPopulation = chkMaxPopulation.Checked,
                    Balance = chkBalance.Checked,
                    HousingCapacity20x = chkHousingCapacity20x.Checked,
                    StorageCapacity10x = chkStorageCapacity10x.Checked,
                    FastBuildUpgradeRepair = chkFastBuildUpgradeRepair.Checked,
                    FoodHealing10x = chkFoodHealing10x.Checked,
                    ToEnglish = chkToEng.Checked,
                    DgVoodoo = chkDgVoodoo.Checked,
                    VillageBuildRange = chkVillageBuildRange.Checked,
                    NoSpellAltar = chkNoSpellAltar.Checked,
                    GameSpeed = GetSelectedGameSpeedMultiplier(),
                    CustomUnitStats = this.customUnitStats,
                    PresetFileSourceType = this.presetFileSourceType,
                    PresetFileName = this.presetFileName,
                    EndlessAiModules = new[] {
                        chkAiM1.Checked, chkAiM2.Checked, chkAiM3.Checked, chkAiM4.Checked, chkAiM5.Checked, chkAiM6.Checked
                    }
                };

                await Task.Run(() => {
                    patchEngine.ApplyPatches(gamePath, options, backupManager, rollback);
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
                
                // 還原後重設 UI
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
                chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(gamePath);
                cmbGameSpeed.SelectedIndex = 0;

                Log(Loc.Get("LogRestoreAllSuccess"));
                MessageBox.Show(Loc.Get("MsgRestoreSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
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

                // UI reset
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

                // UI reset
                chkFocusLoss.Checked = false;
                chkNoSpellAltar.Checked = false;
                chkAiM1.Checked = false; chkAiM2.Checked = false; chkAiM3.Checked = false; chkAiM4.Checked = false; chkAiM5.Checked = false; chkAiM6.Checked = false;
                chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(gamePath);
                chkVillageBuildRange.Checked = false;
                cmbGameSpeed.SelectedIndex = 0;

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

                // UI reset
                chkToEng.Checked = false;

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
