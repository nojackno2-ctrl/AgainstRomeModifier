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
            chkFastBuildUpgradeRepair.Checked = true;
            chkFoodHealing10x.Checked = true;
            chkAiM1.Checked = true;
            chkAiM2.Checked = true;
            chkAiM3.Checked = true;
            chkAiM4.Checked = true;
            chkAiM5.Checked = true;
            chkAiM6.Checked = true;
            chkBalance.Checked = true;
            chkVillageBuildRange.Checked = true;
            chkDgVoodoo.Checked = true;

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
            chkFastBuildUpgradeRepair.Checked = false;
            chkFoodHealing10x.Checked = false;
            chkAiM1.Checked = false;
            chkAiM2.Checked = false;
            chkAiM3.Checked = false;
            chkAiM4.Checked = false;
            chkAiM5.Checked = false;
            chkAiM6.Checked = false;
            chkBalance.Checked = false;
            chkVillageBuildRange.Checked = false;
            chkDgVoodoo.Checked = false; // 相容性層關閉，以達到最乾淨的還原

            Log(Loc.CurrentLanguage == Language.English ? "All features disabled." : "已關閉所有功能。");
        }
    }
}
