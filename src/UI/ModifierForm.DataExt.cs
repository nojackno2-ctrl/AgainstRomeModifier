using System;
using System.Globalization;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        public static bool SupportsConfigurableSpellRadius(string key) {
            return key.Equals("FigKelPri00_Priester", StringComparison.OrdinalIgnoreCase) ||
                key.Equals("FigHunPri00_Priester", StringComparison.OrdinalIgnoreCase);
        }

        /// <summary>
        /// 獲取某個兵種的原始 9 大屬性值 (HP, 傷害, 防禦 VW, 戰鬥 AW, 移動速度, 視野, 攻擊冷卻, 最大射程, 法術半徑)
        /// </summary>
        public double[] GetOriginalStats(string key) {
            EnsureBackupUnitRowsParsed();
            if (!_backupUnitRows.ContainsKey(key)) {
                return new double[] { 0, 0, 0, 0, 0, 0, 0, 0, 0 };
            }
            string[] cols = _backupUnitRows[key];
            
            double hp = 0;
            double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out hp);
            
            double vw = 0;
            double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out vw);
            
            double aw = 0;
            double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out aw);

            string utype = "melee_inf";
            if (TroopConfig.UnitMeta.ContainsKey(key)) {
                utype = TroopConfig.UnitMeta[key].Item3;
            }
            
            // 傷害與冷卻
            double meleeDam = 0, rangedDam = 0;
            GetMeleeAndRangedDmg(cols, utype, out meleeDam, out rangedDam);
            
            double meleeRelt = 0, rangedRelt = 0;
            GetMeleeAndRangedRelt(cols, utype, out meleeRelt, out rangedRelt);
            
            double dmg = meleeDam;
            double relt = meleeRelt;
            if (utype == "ranged_inf" || utype == "ranged_cav") {
                dmg = rangedDam;
                relt = rangedRelt;
            } else if (utype == "siege") {
                dmg = Math.Max(meleeDam, rangedDam);
                relt = Math.Max(meleeRelt, rangedRelt);
            } else if (utype == "hybrid_inf") {
                // 混合步兵取主要武器（近戰傷害/冷卻）
                dmg = meleeDam;
                relt = meleeRelt;
            }

            // 移動速度
            double origMoves = 0;
            double.TryParse(cols[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origMoves);
            double speed = origMoves > 0 ? Math.Round(origMoves * 2.0, 1) : 0;

            // 視野
            double origSight = 0;
            double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out origSight);
            double sight = origSight;

            // 射程 / 技能距離
            double range = GetUnitMaxRange(cols, utype);

            // 法術半徑
            bool supportsSpellRadius = SupportsConfigurableSpellRadius(key);
            double spellRadius = supportsSpellRadius ? 500 : 0;

            return new double[] { hp, dmg, vw, aw, speed, sight, relt, range, spellRadius };
        }

        /// <summary>
        /// 獲取某個兵種的內建平衡 9 大屬性值 (HP, 傷害, 防禦 VW, 戰鬥 AW, 移動速度, 視野, 攻擊冷卻, 最大射程, 法術半徑)
        /// </summary>
        public double[] GetDefaultBalancedStats(string key) {
            if (TroopConfig.BalancedUnitStats.TryGetValue(key, out double[]? stats)) {
                return (double[])stats.Clone();
            }
            return GetOriginalStats(key);
        }
    }
}
