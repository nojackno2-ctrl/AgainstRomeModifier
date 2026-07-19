using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Windows.Forms;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private static readonly object LogLock = new object();
        private static bool _logRotationChecked;
        private const long LogRotationThresholdBytes = 5 * 1024 * 1024;

        private void BuildFeatureToggleMap() {
            featureToggles = new Dictionary<string, ModernToggle>(StringComparer.OrdinalIgnoreCase) {
                [FeatureKeys.FocusLoss.Id] = chkFocusLoss, [FeatureKeys.FastCiviProduction.Id] = chkFastCiviProduction,
                [FeatureKeys.InfiniteMorale.Id] = chkInfiniteMorale, [FeatureKeys.NoRunHpLoss.Id] = chkNoRunHpLoss, [FeatureKeys.FreeProduction.Id] = chkFreeProd,
                [FeatureKeys.FreeUpgrade.Id] = chkFreeUpgrade, [FeatureKeys.NoSpellCost.Id] = chkNoSpellCost,
                [FeatureKeys.MaxPopulation.Id] = chkMaxPopulation, [FeatureKeys.Balance.Id] = chkBalance,
                [FeatureKeys.AllUnitsEntireMapVision.Id] = chkAllUnitsEntireMapVision,
                [FeatureKeys.NativeWidescreen1920x1080.Id] = chkNativeWidescreen1920x1080,
                [FeatureKeys.CameraZoomOut1.Id] = chkCameraZoomOut1,
                [FeatureKeys.CorpseRetention.Id] = chkCorpseRetention,
                [FeatureKeys.RangedRange3x.Id] = chkRangedRange3x,
                [FeatureKeys.UnitMovementSpeed2x.Id] = chkUnitMovementSpeed2x,
                [FeatureKeys.VillagerMovementSpeed5x.Id] = chkVillagerMovementSpeed5x,
                [FeatureKeys.SpellEntireMap.Id] = chkSpellEntireMap, [FeatureKeys.SpellRange3x.Id] = chkSpellRange3x,
                [FeatureKeys.ProjectileArcHeight.Id] = chkProjectileArcHeight,
                [FeatureKeys.RomanEndless.Id] = chkRomanEndless,
                [FeatureKeys.HousingCapacity20x.Id] = chkHousingCapacity20x, [FeatureKeys.StorageCapacity10x.Id] = chkStorageCapacity10x,
                [FeatureKeys.HqHp10x.Id] = chkHqHp10x, [FeatureKeys.FastBuildUpgradeRepair.Id] = chkFastBuildUpgradeRepair,
                [FeatureKeys.FoodHealing10x.Id] = chkFoodHealing10x, [FeatureKeys.CiviProduce20.Id] = chkCiviProduce20,
                [FeatureKeys.UnitRecruit20.Id] = chkUnitRecruit20,
                [FeatureKeys.IdleSelect999.Id] = chkIdleSelect999,
                [FeatureKeys.VillageBuildRange.Id] = chkVillageBuildRange,
                [FeatureKeys.NoSpellAltar.Id] = chkNoSpellAltar, [FeatureKeys.DgVoodoo.Id] = chkDgVoodoo,
                [FeatureKeys.ArgmTrace.Id] = chkArgmTrace,
                [FeatureKeys.ToEnglish.Id] = chkToEng,
                [FeatureKeys.SpellDamage5x.Id] = chkSpellDamage5x,
                [FeatureKeys.SpellHealing10x.Id] = chkSpellHealing10x,
                [FeatureKeys.SpellResurrection.Id] = chkSpellResurrection,
                [FeatureKeys.GeneralSkills.Id] = chkGeneralSkills,
                [FeatureKeys.LeaderGlory.Id] = chkLeaderGlory,
                [FeatureKeys.EndlessAiM1.Id] = chkAiM1, [FeatureKeys.EndlessAiCore.Id] = chkAiCore,
                [FeatureKeys.EndlessAiM5.Id] = chkAiM5,
                [FeatureKeys.RomanReinforcementGarrison.Id] = chkRomanReinforcementGarrison,
            };
            FeatureRegistry.ValidateToggleIds(featureToggles.Keys);
        }

        private void ResetTogglesForCategory(FeatureCategory category, string gamePath) {
            foreach (FeatureDefinition feature in FeatureRegistry.ByCategory(category)) {
                if (featureToggles.TryGetValue(feature.Id, out ModernToggle? toggle)) toggle.Checked = false;
            }
            if (category == FeatureCategory.Compat) {
                SetGameSpeedSelection(1);
                chkVillageGarrisonQuota3x.Checked = false;
                chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(gamePath);
                chkArgmTrace.Checked = patchEngine.IsArgmTraceInstalled(gamePath);
            }
        }

        private PatchProfile BuildCurrentPatchProfile(bool forceBalance = false) {
            var profile = new PatchProfile();
            foreach (var (id, toggle) in featureToggles) profile.Set(id, FeatureValue.Of(toggle.Checked));
            if (forceBalance) profile.Balance = true;
            profile.Set(FeatureKeys.GameSpeed, chkGameSpeed.Checked ? SelectedGameSpeedMultiplier() : 1);
            profile.Set(
                FeatureKeys.VillageGarrisonQuotaMultiplier,
                chkVillageGarrisonQuota3x.Checked ? SelectedVillageGarrisonQuotaMultiplier() : 1);
            profile.Set(FeatureKeys.CustomUnitStats, customUnitStats);
            return profile;
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
            btnNavExperimental.Enabled = enabled;
            btnNavDefaultStats.Enabled = enabled;
            btnNavCurrentStats.Enabled = enabled;
        }
        /// <summary>
        /// 安全地嘗試載入或建立遊戲備份。如果發生例外 (例如遊戲檔案已被修改)，
        /// 會捕捉例外並顯示錯誤訊息，回傳 false。
        /// </summary>
        private bool TryEnsureBackupLoadedForGamePath(string gamePath) {
            try {
                return backupManager.EnsureBackupLoadedForGamePath(gamePath);
            } catch (Exception ex) {
                Log("EnsureBackupLoadedForGamePath 發生錯誤: " + ex.Message);
                MessageBox.Show(ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return false;
            }
        }

        /// <summary>
        /// 開啟資料夾瀏覽對話框，供使用者手動選擇遊戲的安裝路徑。
        /// </summary>
        private void BtnBrowseGamePath_Click(object? sender, EventArgs e) {
            using (var fbd = new FolderBrowserDialog()) {
                fbd.Description = Loc.Get("LogBrowseTitle");
                if (fbd.ShowDialog() == DialogResult.OK) {
                    txtGamePath.Text = fbd.SelectedPath;
                    TryEnsureBackupLoadedForGamePath(fbd.SelectedPath);
                    chkDgVoodoo.Checked = patchEngine.IsDgVoodooInstalled(fbd.SelectedPath);
                    chkArgmTrace.Checked = patchEngine.IsArgmTraceInstalled(fbd.SelectedPath);
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
            if (!TryEnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }

            try {
                DialogResult confirm = MessageBox.Show(Loc.Get("MsgConfirmApply"), Loc.Get("TitleConfirm"), MessageBoxButtons.YesNo, MessageBoxIcon.Question);
                if (confirm != DialogResult.Yes) return;

                SetActionButtonsEnabled(false);
                Log(Loc.Get("LogStartApply"));

                PatchProfile profile = BuildCurrentPatchProfile();

                await patchOperationRunner.ExecuteAsync(
                    rollback => patchEngine.ApplyPatches(gamePath, profile, backupManager, rollback),
                    "已建立修改前檔案回復點。",
                    "套用失敗，開始回復已修改的檔案。",
                    "檔案回復流程已完成。");
                Log(Loc.Get("LogApplyAllSuccess"));
                MessageBox.Show(Loc.Get("MsgApplySuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful apply: " + uiEx.Message);
                }
            } catch (Exception ex) {
                Log(Loc.Get("MsgApplyFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgApplyFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
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
            if (!TryEnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreAll"));
                await patchOperationRunner.ExecuteAsync(
                    rollback => patchEngine.RestoreOriginalFiles(gamePath, backupManager, rollback),
                    "已建立還原前檔案回復點。",
                    "還原失敗，開始回復變更。",
                    "還原失敗後的回復流程已完成。");
                
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
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
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

        /// <summary>取得下拉選單目前選擇的遊戲加速倍率。</summary>
        private int SelectedGameSpeedMultiplier() {
            int index = cboGameSpeedMultiplier.SelectedIndex;
            if (index < 0 || index >= GameSpeedMultiplierChoices.Length) return 10;
            return GameSpeedMultiplierChoices[index];
        }

        /// <summary>依偵測到的倍率同步遊戲加速開關與下拉選單（1 = 停用，僅取消勾選、保留選擇）。</summary>
        private void SetGameSpeedSelection(int multiplier) {
            chkGameSpeed.Checked = multiplier > 1;
            int index = Array.IndexOf(GameSpeedMultiplierChoices, multiplier);
            if (index >= 0) cboGameSpeedMultiplier.SelectedIndex = index;
        }

        /// <summary>取得下拉選單目前選擇的村莊駐軍配額倍率。</summary>
        private int SelectedVillageGarrisonQuotaMultiplier() {
            int index = cboVillageGarrisonQuotaMultiplier.SelectedIndex;
            if (index < 0 || index >= VillageGarrisonQuotaMultiplierChoices.Length) return 3;
            return VillageGarrisonQuotaMultiplierChoices[index];
        }

        /// <summary>依偵測到的倍率同步駐軍配額開關與下拉選單（1 = 停用，僅取消勾選、保留選擇）。</summary>
        private void SetVillageGarrisonQuotaSelection(int multiplier) {
            chkVillageGarrisonQuota3x.Checked = multiplier > 1;
            int index = Array.IndexOf(VillageGarrisonQuotaMultiplierChoices, multiplier);
            if (index >= 0) cboVillageGarrisonQuotaMultiplier.SelectedIndex = index;
        }

        private async void RestoreStatsOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            if (!TryEnsureBackupLoadedForGamePath(gamePath)) {
                return;
            }
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreStats"));
                await patchOperationRunner.ExecuteAsync(
                    rollback => patchEngine.RestoreStatsOnly(gamePath, backupManager, rollback),
                    "已建立屬性檔案回復點。",
                    "還原失敗，開始回復變更。",
                    "還原失敗後的回復流程已完成。");

                ResetTogglesForCategory(FeatureCategory.Stats, gamePath);

                Log(Loc.Get("LogRestoreStatsDone"));
                MessageBox.Show(Loc.Get("MsgRestoreStatsSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful stats restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                SetActionButtonsEnabled(true);
            }
        }

        private async void RestoreCompatOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreCompat"));
                await patchOperationRunner.ExecuteAsync(
                    rollback => patchEngine.RestoreCompatOnly(gamePath, backupManager, rollback),
                    "已建立相容性檔案回復點。",
                    "還原失敗，開始回復變更。",
                    "還原失敗後的回復流程已完成。");

                ResetTogglesForCategory(FeatureCategory.Compat, gamePath);

                Log(Loc.Get("LogRestoreCompatDone"));
                MessageBox.Show(Loc.Get("MsgRestoreCompatSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful compat restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                SetActionButtonsEnabled(true);
            }
        }

        private async void RestoreLanguageOnly() {
            string gamePath = GetGamePath();
            if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                MessageBox.Show(Loc.Get("MsgSelectGameDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                return;
            }
            SetActionButtonsEnabled(false);
            try {
                Log(Loc.Get("LogStartRestoreLang"));
                await patchOperationRunner.ExecuteAsync(
                    rollback => patchEngine.RestoreLanguageOnly(gamePath, rollback),
                    "已建立語言檔案回復點。",
                    "還原失敗，開始回復變更。",
                    "還原失敗後的回復流程已完成。");

                ResetTogglesForCategory(FeatureCategory.Language, gamePath);

                Log(Loc.Get("LogRestoreLangDone"));
                MessageBox.Show(Loc.Get("MsgRestoreLangSuccess"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                try {
                    LoadCurrentData();
                } catch (Exception uiEx) {
                    Log("Reload current data failed after successful language restore: " + uiEx.Message);
                }
            } catch (Exception ex) {
                Log(Loc.Get("MsgRestoreFailed") + ex.Message + "\r\n" + ex.StackTrace);
                MessageBox.Show(Loc.Get("MsgRestoreFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } finally {
                SetActionButtonsEnabled(true);
            }
        }
    }
}
