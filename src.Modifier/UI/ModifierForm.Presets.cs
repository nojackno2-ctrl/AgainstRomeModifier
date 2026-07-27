using System;
using System.Collections.Generic;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        /// <summary>
        /// 「所有功能開啟」刻意不涵蓋的開關。分兩類：
        /// 1. 已驗證但需手動開啟（攝影機拉遠、除錯記錄器）——預設不該被一鍵打開。
        /// 2. 會改寫兵種數值或仍待個別評估的實驗性項目（平衡、屍體保留、法術治療/復活、
        ///    將領技能、首領榮耀）——一鍵全開若順手改動它們會覆蓋使用者的自訂配置。
        /// 「所有功能關閉」則一律涵蓋全部開關，沒有例外。
        /// </summary>
        internal static readonly IReadOnlyList<string> EnableAllExcludedFeatureIds = new[] {
            FeatureKeys.Balance.Id,
            FeatureKeys.CameraZoomOut1.Id,
            FeatureKeys.CorpseRetention.Id,
            FeatureKeys.ArgmTrace.Id,
            FeatureKeys.SpellHealing10x.Id,
            FeatureKeys.SpellResurrection.Id,
            FeatureKeys.GeneralSkills.Id,
            FeatureKeys.LeaderGlory.Id,
        };

        private static readonly HashSet<string> EnableAllExcluded =
            new(EnableAllExcludedFeatureIds, StringComparer.OrdinalIgnoreCase);

        /// <summary>
        /// 當使用者點擊「所有功能開啟」時觸發，勾選 <see cref="EnableAllExcludedFeatureIds"/>
        /// 以外的所有修改功能開關。遊戲加速與駐軍配額倍率屬需手動選擇的倍率項，同樣不動。
        /// </summary>
        private void BtnEnableAll_Click(object? sender, EventArgs e) {
            foreach (var (id, toggle) in featureToggles) {
                if (EnableAllExcluded.Contains(id)) continue;
                toggle.Checked = true;
            }

            Log(Loc.CurrentLanguage == Language.English ? "All features enabled." : "已開啟所有功能。");
        }

        /// <summary>
        /// 當使用者點擊「所有功能關閉」時觸發，取消勾選全部修改與相容性開關，
        /// 並把兩個倍率下拉選單還原為原版（1 倍）。
        /// </summary>
        private void BtnDisableAll_Click(object? sender, EventArgs e) {
            foreach (var toggle in featureToggles.Values) {
                toggle.Checked = false;
            }
            SetGameSpeedSelection(1);            // 還原為原版速度
            SetVillageGarrisonQuotaSelection(1); // 還原為原版駐軍配額

            Log(Loc.CurrentLanguage == Language.English ? "All features disabled." : "已關閉所有功能。");
        }
    }
}
