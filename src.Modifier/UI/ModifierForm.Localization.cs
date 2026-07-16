using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Drawing2D;

namespace AgainstRomeModifier {
    // 主表單的語系套用與文字更新：語系按鈕樣式、全 UI 文字在地化、表頭在地化、
    // 兵種範本標籤/下拉、技術文件重載。由 ModifierForm.cs 拆出（純程式碼搬移，行為不變）。
    public partial class ModifierForm {

        /// <summary>
        /// 更新語系按鈕視覺樣式
        /// </summary>
        private void UpdateLanguageButtonStyles() {
            bool isZh = Loc.CurrentLanguage == Language.TraditionalChinese;

            btnLangZH.BackColor = isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangZH.ForeColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangZH.FlatAppearance.BorderColor = isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);

            btnLangEN.BackColor = !isZh ? Color.FromArgb(30, 30, 42) : Color.Transparent;
            btnLangEN.ForeColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(120, 125, 135);
            btnLangEN.FlatAppearance.BorderColor = !isZh ? Color.FromArgb(0, 220, 255) : Color.FromArgb(50, 50, 60);
        }

        /// <summary>
        /// 套用目前的語系設定到所有的 UI 元件
        /// </summary>
        private void ApplyLanguageToUI() {
            if (lblMainTitle == null) return; // 防止在 InitializeComponent 完成前被調用

            lblMainTitle.Text = Loc.Get("MainTitle");
            lblSystemHeading.Text = Loc.Get("SystemHeading");
            lblSystemSubtitle.Text = Loc.Get("SystemSubtitle");
            lblNumericTitle.Text = Loc.Get("NumericTitle");
            chkFocusLoss.Text = Loc.Get("FocusLoss");
            chkToEng.Text = Loc.Get("ToEng");
            chkAiM1.Text = Loc.Get("AiM1");
            chkAiCore.Text = Loc.Get("AiCore");
            chkAiM5.Text = Loc.Get("AiM5");
            chkAiM6.Text = Loc.Get("AiM6");
            chkHousingCapacity20x.Text = Loc.Get("HousingCapacity20x");
            chkStorageCapacity10x.Text = Loc.Get("StorageCapacity10x");
            chkHqHp10x.Text = Loc.Get("HqHp10x");
            chkFastBuildUpgradeRepair.Text = Loc.Get("FastBuildUpgradeRepair");
            chkFoodHealing10x.Text = Loc.Get("FoodHealing10x");
            chkCiviProduce20.Text = Loc.Get("CiviProduce20");
            chkUnitRecruit20.Text = Loc.Get("UnitRecruit20");
            chkDgVoodoo.Text = Loc.Get("DgVoodoo");
            chkGameSpeed.Text = Loc.Get("GameSpeedLabel");
            chkVillageBuildRange.Text = Loc.Get("VillageBuildRange");
            lblExperimentalTitle.Text = Loc.Get("ExperimentalTitle");
            chkSpellDamage5x.Text = Loc.Get("SpellDamage5x");
            chkSpellHealing10x.Text = Loc.Get("SpellHealing10x");
            chkSpellResurrection.Text = Loc.Get("SpellResurrection");
            chkGeneralSkills.Text = Loc.Get("GeneralSkills");
            chkLeaderGlory.Text = Loc.Get("LeaderGlory");
            btnEnableAll.Text = Loc.Get("EnableAll");
            btnDisableAll.Text = Loc.Get("DisableAll");
            lblSwitchesTitle.Text = Loc.Get("SwitchesTitle");
            lblBuildTitle.Text = Loc.Get("BuildTitle");
            lblAiTitle.Text = Loc.Get("AiCardTitle");
            chkMaxPopulation.Text = Loc.Get("MaxPopulation");
            chkFastCiviProduction.Text = Loc.Get("FastCiviProduction");
            chkFreeProd.Text = Loc.Get("FreeProd");
            chkFreeUpgrade.Text = Loc.Get("FreeUpgrade");
            chkNoSpellCost.Text = Loc.Get("NoSpellCost");
            chkNoSpellAltar.Text = Loc.Get("NoSpellAltar");
            chkInfiniteMorale.Text = Loc.Get("InfiniteMorale");
            lblGamePath.Text = Loc.Get("GamePath");
            btnBrowseGamePath.Text = Loc.Get("Browse");
            btnLoadCurrent.Text = Loc.Get("LoadCurrent");
            btnRestore.Text = Loc.Get("Restore");
            btnApply.Text = Loc.Get("Apply");
            btnTroopPreset.Text = Loc.Get("BtnTroopPreset");
            btnStartGame.Text = Loc.Get("StartGame");

            itemRestoreAll.Text = Loc.Get("RestoreAll");
            itemRestoreStats.Text = Loc.Get("RestoreStats");
            itemRestoreCompat.Text = Loc.Get("RestoreCompat");
            itemRestoreLang.Text = Loc.Get("RestoreLang");



            lblSidebarLang.Text = Loc.Get("LanguageLabel");

            tabDefaultRoman.Text = Loc.Get("TabRoman");
            tabDefaultTeuton.Text = Loc.Get("TabTeuton");
            tabDefaultCelt.Text = Loc.Get("TabCelt");
            tabDefaultHun.Text = Loc.Get("TabHun");

            tabCurrentRoman.Text = Loc.Get("TabRoman");
            tabCurrentTeuton.Text = Loc.Get("TabTeuton");
            tabCurrentCelt.Text = Loc.Get("TabCelt");
            tabCurrentHun.Text = Loc.Get("TabHun");

            lblDefaultStatsTitle.Text = Loc.Get("DefaultStatsTitle");
            chkBalance.Text = Loc.Get("EnableBalance");
            chkAllUnitsEntireMapVision.Text = Loc.Get("AllUnitsEntireMapVision");
            chkRangedRange3x.Text = Loc.Get("RangedRange3x");
            chkUnitMovementSpeed2x.Text = Loc.Get("UnitMovementSpeed2x");
            chkSpellEntireMap.Text = Loc.Get("SpellEntireMap");
            chkSpellRange3x.Text = Loc.Get("SpellRange3x");
            chkProjectileArcHeight.Text = Loc.Get("ProjectileArcHeight");
            chkRomanEndless.Text = Loc.Get("RomanEndless");
            lblTroopTemplate.Text = Loc.Get("TroopTemplateLabel");
            RefreshTroopTemplateItems();
            lblCurrentStatsTitle.Text = Loc.Get("CurrentStatsTitle");

            // 更新表格標頭
            UpdateGridHeaders();

            // 重新整理側邊導覽列按鈕
            RefreshNavButtons();


            // 重新載入表格數據
            if (backupManager != null && backupManager.BackupFiles.Count > 0) {
                LoadDefaultStatsData();
                LoadCurrentData(false);
            }
            UpdateTroopPresetLabel();

            if (myToolTip != null) {
                myToolTip.SetToolTip(chkFocusLoss, Loc.Get("FocusLossTip"));
                myToolTip.SetToolTip(chkToEng, Loc.Get("ToEngTip"));
                myToolTip.SetToolTip(chkDgVoodoo, Loc.Get("DgVoodooTip"));
                myToolTip.SetToolTip(chkGameSpeed, Loc.Get("GameSpeedTip"));
                myToolTip.SetToolTip(chkSpellDamage5x, Loc.Get("SpellDamage5xTip"));
                myToolTip.SetToolTip(chkSpellHealing10x, Loc.Get("SpellHealing10xTip"));
                myToolTip.SetToolTip(chkSpellResurrection, Loc.Get("SpellResurrectionTip"));
                myToolTip.SetToolTip(chkGeneralSkills, Loc.Get("GeneralSkillsTip"));
                myToolTip.SetToolTip(chkLeaderGlory, Loc.Get("LeaderGloryTip"));
                myToolTip.SetToolTip(chkFreeProd, Loc.Get("FreeProdTip"));
                myToolTip.SetToolTip(chkFreeUpgrade, Loc.Get("FreeUpgradeTip"));
                myToolTip.SetToolTip(chkNoSpellCost, Loc.Get("NoSpellCostTip"));
                myToolTip.SetToolTip(chkInfiniteMorale, Loc.Get("InfiniteMoraleTip"));
                myToolTip.SetToolTip(chkBalance, Loc.Get("BalanceTip"));
                myToolTip.SetToolTip(chkAllUnitsEntireMapVision, Loc.Get("AllUnitsEntireMapVisionTip"));
                myToolTip.SetToolTip(chkRangedRange3x, Loc.Get("RangedRange3xTip"));
                myToolTip.SetToolTip(chkUnitMovementSpeed2x, Loc.Get("UnitMovementSpeed2xTip"));
                myToolTip.SetToolTip(chkSpellEntireMap, Loc.Get("SpellEntireMapTip"));
                myToolTip.SetToolTip(chkSpellRange3x, Loc.Get("SpellRange3xTip"));
                myToolTip.SetToolTip(chkProjectileArcHeight, Loc.Get("ProjectileArcHeightTip"));
                myToolTip.SetToolTip(chkRomanEndless, Loc.Get("RomanEndlessTip"));
                myToolTip.SetToolTip(chkNoSpellAltar, Loc.Get("NoSpellAltarTip"));
                myToolTip.SetToolTip(chkMaxPopulation, Loc.Get("MaxPopulationTip"));
                myToolTip.SetToolTip(chkHousingCapacity20x, Loc.Get("HousingCapacity20xTip"));
                myToolTip.SetToolTip(chkStorageCapacity10x, Loc.Get("StorageCapacity10xTip"));
                myToolTip.SetToolTip(chkHqHp10x, Loc.Get("HqHp10xTip"));
                myToolTip.SetToolTip(chkFastCiviProduction, Loc.Get("FastCiviProductionTip"));
                myToolTip.SetToolTip(chkFastBuildUpgradeRepair, Loc.Get("FastBuildUpgradeRepairTip"));
                myToolTip.SetToolTip(chkFoodHealing10x, Loc.Get("FoodHealing10xTip"));
                myToolTip.SetToolTip(chkCiviProduce20, Loc.Get("CiviProduce20Tip"));
                myToolTip.SetToolTip(chkUnitRecruit20, Loc.Get("UnitRecruit20Tip"));
                myToolTip.SetToolTip(chkVillageBuildRange, Loc.Get("VillageBuildRangeTip"));
                myToolTip.SetToolTip(chkAiM1, Loc.Get("AiM1Tip"));
                myToolTip.SetToolTip(chkAiCore, Loc.Get("AiCoreTip"));
                myToolTip.SetToolTip(chkAiM5, Loc.Get("AiM5Tip"));
                myToolTip.SetToolTip(chkAiM6, Loc.Get("AiM6Tip"));
            }
        }

        /// <summary>
        /// 動態更新 DataGridView 標頭文字
        /// </summary>
        private void UpdateGridHeaders() {
            foreach (var grid in defaultStatsGrids.Values) {
                if (grid.Columns.Contains("Name")) grid.Columns["Name"].HeaderText = Loc.Get("HeaderName");
                if (grid.Columns.Contains("Icon")) grid.Columns["Icon"].HeaderText = Loc.Get("HeaderIcon");
                if (grid.Columns.Contains("Type")) grid.Columns["Type"].HeaderText = Loc.Get("HeaderType");
                if (grid.Columns.Contains("Style")) grid.Columns["Style"].HeaderText = Loc.Get("HeaderStyle");
                if (grid.Columns.Contains("Hp")) grid.Columns["Hp"].HeaderText = Loc.Get("HeaderHp");
                if (grid.Columns.Contains("MeleeDmg")) grid.Columns["MeleeDmg"].HeaderText = Loc.Get("HeaderMeleeDmg");
                if (grid.Columns.Contains("RangedDmg")) grid.Columns["RangedDmg"].HeaderText = Loc.Get("HeaderRangedDmg");
                if (grid.Columns.Contains("MeleeRelt")) grid.Columns["MeleeRelt"].HeaderText = Loc.Get("HeaderMeleeRelt");
                if (grid.Columns.Contains("RangedRelt")) grid.Columns["RangedRelt"].HeaderText = Loc.Get("HeaderRangedRelt");
                if (grid.Columns.Contains("Vw")) grid.Columns["Vw"].HeaderText = Loc.Get("HeaderVw");
                if (grid.Columns.Contains("Aw")) grid.Columns["Aw"].HeaderText = Loc.Get("HeaderAw");
                if (grid.Columns.Contains("Speed")) grid.Columns["Speed"].HeaderText = Loc.Get("HeaderSpeed");
                if (grid.Columns.Contains("Sight")) grid.Columns["Sight"].HeaderText = Loc.Get("HeaderSight");
                if (grid.Columns.Contains("Range")) grid.Columns["Range"].HeaderText = Loc.Get("HeaderRange");
                if (grid.Columns.Contains("SpellRadius")) grid.Columns["SpellRadius"].HeaderText = Loc.Get("HeaderSpellRadius");
                if (grid.Columns.Contains("Tier")) grid.Columns["Tier"].HeaderText = Loc.Get("HeaderTier");
            }
            foreach (var grid in currentStatsGrids.Values) {
                if (grid.Columns.Contains("Name")) grid.Columns["Name"].HeaderText = Loc.Get("HeaderName");
                if (grid.Columns.Contains("Icon")) grid.Columns["Icon"].HeaderText = Loc.Get("HeaderIcon");
                if (grid.Columns.Contains("Type")) grid.Columns["Type"].HeaderText = Loc.Get("HeaderType");
                if (grid.Columns.Contains("Style")) grid.Columns["Style"].HeaderText = Loc.Get("HeaderStyle");
                if (grid.Columns.Contains("Hp")) grid.Columns["Hp"].HeaderText = Loc.Get("HeaderHpComp");
                if (grid.Columns.Contains("MeleeDmg")) grid.Columns["MeleeDmg"].HeaderText = Loc.Get("HeaderMeleeDmgComp");
                if (grid.Columns.Contains("RangedDmg")) grid.Columns["RangedDmg"].HeaderText = Loc.Get("HeaderRangedDmgComp");
                if (grid.Columns.Contains("MeleeRelt")) grid.Columns["MeleeRelt"].HeaderText = Loc.Get("HeaderMeleeReltComp");
                if (grid.Columns.Contains("RangedRelt")) grid.Columns["RangedRelt"].HeaderText = Loc.Get("HeaderRangedReltComp");
                if (grid.Columns.Contains("Vw")) grid.Columns["Vw"].HeaderText = Loc.Get("HeaderVwComp");
                if (grid.Columns.Contains("Aw")) grid.Columns["Aw"].HeaderText = Loc.Get("HeaderAwComp");
                if (grid.Columns.Contains("Speed")) grid.Columns["Speed"].HeaderText = Loc.Get("HeaderSpeedComp");
                if (grid.Columns.Contains("Sight")) grid.Columns["Sight"].HeaderText = Loc.Get("HeaderSightComp");
                if (grid.Columns.Contains("Range")) grid.Columns["Range"].HeaderText = Loc.Get("HeaderRangeComp");
                if (grid.Columns.Contains("SpellRadius")) grid.Columns["SpellRadius"].HeaderText = Loc.Get("HeaderSpellRadiusComp");
                if (grid.Columns.Contains("Tier")) grid.Columns["Tier"].HeaderText = Loc.Get("HeaderTier");
            }
        }

        private void UpdateTroopPresetLabel() {
            if (lblTroopPresetFile == null) return;
            if (presetFileSourceType == "manual") {
                lblTroopPresetFile.Text = Loc.Get("TroopPresetManual");
                lblTroopPresetFile.ForeColor = Color.FromArgb(200, 100, 255);
            } else if (presetFileSourceType == "file") {
                lblTroopPresetFile.Text = string.Format(Loc.Get("TroopPresetFile"), presetFileName);
                lblTroopPresetFile.ForeColor = Color.FromArgb(0, 220, 255);
            } else if (presetFileSourceType == "preset") {
                lblTroopPresetFile.Text = string.Format(Loc.Get("TroopPresetLoaded"), presetFileName);
                lblTroopPresetFile.ForeColor = Color.FromArgb(0, 220, 255);
            } else {
                lblTroopPresetFile.Text = Loc.Get("TroopPresetDefault");
                lblTroopPresetFile.ForeColor = Color.FromArgb(160, 165, 170);
            }
        }

        private void RefreshTroopTemplateItems() {
            if (cbTroopTemplate == null) return;
            int selectedIndex = cbTroopTemplate.SelectedIndex;
            cbTroopTemplate.SelectedIndexChanged -= CbTroopTemplate_SelectedIndexChanged;
            cbTroopTemplate.Items.Clear();
            cbTroopTemplate.Items.Add(Loc.Get("TroopTemplateSelect"));
            cbTroopTemplate.Items.Add(Loc.Get("TroopTemplateBalanced"));
            cbTroopTemplate.SelectedIndex = selectedIndex > 0 && selectedIndex < cbTroopTemplate.Items.Count ? selectedIndex : 0;
            cbTroopTemplate.SelectedIndexChanged += CbTroopTemplate_SelectedIndexChanged;
        }

        private void CbTroopTemplate_SelectedIndexChanged(object? sender, EventArgs e) {
            if (cbTroopTemplate == null || cbTroopTemplate.SelectedIndex == 0) return;

            string text = Loc.CurrentLanguage == Language.English
                ? "Are you sure you want to apply the 'Balanced Base Stats' template? This will overwrite your current custom unit stats."
                : "確定要套用「修改器內建平衡」範本嗎？這將覆蓋目前的自訂兵種屬性。";
            string title = Loc.CurrentLanguage == Language.English ? "Confirm Template Overwrite" : "確認範本覆蓋";

            if (MessageBox.Show(text, title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) {
                cbTroopTemplate.SelectedIndex = 0;
                return;
            }

            customUnitStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);
            foreach (string key in TroopConfig.UnitOrder) {
                if (!TroopConfig.UnitMeta.ContainsKey(key)) continue;
                customUnitStats[key] = unitStatsEditorService.NormalizeIndependentFields(
                    key, unitStatsEditorService.GetBalanced(key));
            }

            presetFileSourceType = "preset";
            presetFileName = Loc.Get("TroopTemplateBalanced");
            LoadDefaultStatsData();
            UpdateTroopPresetLabel();
            Log(Loc.CurrentLanguage == Language.English
                ? "Applied built-in balanced troop stats template."
                : "已套用修改器內建平衡兵種屬性範本。");

            cbTroopTemplate.SelectedIndex = 0;
        }
 
        private void BtnTroopPreset_Click(object? sender, EventArgs e) {
            using (var form = new TroopPresetForm(unitStatsEditorService, customUnitStats, unitIcons)) {
                if (form.ShowDialog() == DialogResult.OK) {
                    customUnitStats = form.CustomStats;
                    LoadDefaultStatsData(); // 重新整理預設屬性表格
                    Log("已套用自訂兵種屬性配置。");
 
                    if (!string.IsNullOrEmpty(form.LoadedFileName)) {
                        presetFileSourceType = "file";
                        presetFileName = form.LoadedFileName;
                    } else if (customUnitStats != null && customUnitStats.Count > 0) {
                        presetFileSourceType = "manual";
                        presetFileName = "";
                    } else {
                        presetFileSourceType = "default";
                        presetFileName = "";
                    }
                    UpdateTroopPresetLabel();
                }
            }
        }


    }
}
