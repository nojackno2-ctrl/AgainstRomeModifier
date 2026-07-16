using System;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        /// <summary>
        /// 當使用者點擊「所有功能開啟」時觸發，一次性勾選所有修改功能開關。
        /// </summary>
        private void BtnEnableAll_Click(object? sender, EventArgs e) {
            chkFocusLoss.Checked = true;
            chkToEng.Checked = true;
            chkMaxPopulation.Checked = true;
            chkFastCiviProduction.Checked = true;
            chkFreeProd.Checked = true;
            chkFreeUpgrade.Checked = true;
            chkNoSpellCost.Checked = true;
            chkNoSpellAltar.Checked = true;
            chkInfiniteMorale.Checked = true;
            chkHousingCapacity20x.Checked = true;
            chkStorageCapacity10x.Checked = true;
            chkHqHp10x.Checked = true;
            chkFastBuildUpgradeRepair.Checked = true;
            chkFoodHealing10x.Checked = true;
            chkCiviProduce20.Checked = true;
            chkUnitRecruit20.Checked = true;
            chkIdleSelect999.Checked = true;
            chkAiM1.Checked = true;
            chkAiCore.Checked = true;
            chkAiM5.Checked = true;
            chkAiM6.Checked = true;
            chkVillageBuildRange.Checked = true;
            chkDgVoodoo.Checked = true;
            chkSpellDamage5x.Checked = true;
            chkRangedRange3x.Checked = true;
            chkUnitMovementSpeed2x.Checked = true;
            // 兩者各自獨立：全地圖 = 施法距離 (objdef Sirad)、3 倍 = 法術效果半徑 (cl_script Radius)。
            chkSpellEntireMap.Checked = true;
            chkSpellRange3x.Checked = true;
            chkProjectileArcHeight.Checked = true;
            chkRomanEndless.Checked = true;
            // 實驗性功能 (含 10 倍遊戲加速等) 排除在一鍵全開之外，保持原樣不變。

            Log(Loc.CurrentLanguage == Language.English ? "All features enabled." : "已開啟所有功能。");
        }

        /// <summary>
        /// 當使用者點擊「所有功能關閉」時觸發，一次性取消勾選所有修改功能與相容性開關。
        /// </summary>
        private void BtnDisableAll_Click(object? sender, EventArgs e) {
            chkFocusLoss.Checked = false;
            chkToEng.Checked = false;
            chkMaxPopulation.Checked = false;
            chkFastCiviProduction.Checked = false;
            chkFreeProd.Checked = false;
            chkFreeUpgrade.Checked = false;
            chkNoSpellCost.Checked = false;
            chkNoSpellAltar.Checked = false;
            chkInfiniteMorale.Checked = false;
            chkHousingCapacity20x.Checked = false;
            chkStorageCapacity10x.Checked = false;
            chkHqHp10x.Checked = false;
            chkFastBuildUpgradeRepair.Checked = false;
            chkFoodHealing10x.Checked = false;
            chkCiviProduce20.Checked = false;
            chkUnitRecruit20.Checked = false;
            chkIdleSelect999.Checked = false;
            chkAiM1.Checked = false;
            chkAiCore.Checked = false;
            chkAiM5.Checked = false;
            chkAiM6.Checked = false;
            chkVillageBuildRange.Checked = false;
            chkDgVoodoo.Checked = false; // 相容性層關閉，以達到最乾淨的還原

            // 關閉所有實驗性功能開關
            chkSpellDamage5x.Checked = false;
            chkSpellHealing10x.Checked = false;
            chkSpellResurrection.Checked = false;
            chkGeneralSkills.Checked = false;
            chkLeaderGlory.Checked = false;
            chkBalance.Checked = false;
            chkAllUnitsEntireMapVision.Checked = false;
            chkNativeWidescreen1920x1080.Checked = false;
            chkCameraZoomOut1.Checked = false;
            chkRangedRange3x.Checked = false;
            chkUnitMovementSpeed2x.Checked = false;
            chkSpellEntireMap.Checked = false;
            chkSpellRange3x.Checked = false;
            chkProjectileArcHeight.Checked = false;
            chkRomanEndless.Checked = false;
            SetGameSpeedSelection(1); // 還原為原版速度

            Log(Loc.CurrentLanguage == Language.English ? "All features disabled." : "已關閉所有功能。");
        }
    }
}
