using System;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        public static bool SupportsConfigurableSpellRadius(string key) {
            return AgainstRomeModifier.Core.Services.BackupManager.SupportsConfigurableSpellRadius(key);
        }

        /// <summary>
        /// 獲取某個兵種的原始 9 大屬性值 (HP, 傷害, 防禦 VW, 戰鬥 AW, 移動速度, 視野, 攻擊冷卻, 最大射程, 法術半徑)
        /// </summary>
        public double[] GetOriginalStats(string key) {
            return backupManager.GetOriginalStats(key);
        }

        /// <summary>
        /// 獲取某個兵種的內建平衡 9 大屬性值 (HP, 傷害, 防禦 VW, 戰鬥 AW, 移動速度, 視野, 攻擊冷卻, 最大射程, 法術半徑)
        /// </summary>
        public double[] GetDefaultBalancedStats(string key) {
            return backupManager.GetDefaultBalancedStats(key);
        }

        public double[] NormalizeIndependentCustomFields(string key, double[] stats) {
            double[] normalized = (double[])stats.Clone();
            double[] original = GetOriginalStats(key);
            foreach (int index in new[] { 4, 7, 8 })
                if (index < normalized.Length && index < original.Length) normalized[index] = original[index];
            if (TroopConfig.UnitMeta.TryGetValue(key, out var meta) && meta.UnitType == "priest" && normalized.Length > 5 && original.Length > 5)
                normalized[5] = original[5];
            return normalized;
        }
    }
}
