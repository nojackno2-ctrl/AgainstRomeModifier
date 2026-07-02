using System;
using System.Globalization;
using System.Collections.Generic;

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
            double[] orig = GetOriginalStats(key);
            if (!TroopConfig.UnitMeta.ContainsKey(key)) {
                return orig;
            }
            var meta = TroopConfig.UnitMeta[key];
            string faction = meta.Item1;
            string tier = meta.Item2;
            string utype = meta.Item3;
            string style = meta.Item4;

            // 內建平衡模式下的 HP, 傷害, 防禦, 戰鬥力
            double hp = orig[0];
            double dmg = orig[1];
            double vw = orig[2];
            double aw = orig[3];

            if (utype != "priest" && utype != "siege") {
                double[] bal = TroopConfig.CalculateFactionBaseStats(key, faction, tier, utype);
                hp = bal[0];
                dmg = bal[1];
                vw = bal[2];
                aw = bal[3];

                // 套用裝備修飾
                if (style == "two_handed") {
                    dmg = Math.Round(dmg * 1.3, 1);
                }
                if (style == "shield") {
                    vw = Math.Round(vw * 1.3, 1);
                    aw = Math.Round(aw * 1.15, 1);
                }
            }

            // 內建平衡模式下的速度 (原版 2.0 倍)
            double speed = orig[4] > 0 ? Math.Round(orig[4] * 2.0, 1) : 0;

            // 內建平衡模式下的視野與射程 (祭司 30 倍，遠程/攻城 3 倍，其餘維持原版)
            double sight = orig[5];
            double range = orig[7];
            if (utype == "priest") {
                sight = Math.Round(orig[5] * 30.0);
                range = Math.Round(orig[7] * 30.0);
            } else if (utype == "ranged_inf" || utype == "ranged_cav" || utype == "hybrid_inf" || utype == "siege") {
                sight = Math.Round(orig[5] * 3.0);
                range = Math.Round(orig[7] * 3.0);
            }

            // 內建平衡模式下的冷卻 (雙持與遠程折算 1.5 倍射速，冷卻除以 1.5)
            double relt = orig[6];
            if (utype == "ranged_inf" || utype == "ranged_cav" || utype == "hybrid_inf" || style == "dual_wield") {
                if (relt > 0) {
                    relt = Math.Round(relt / 1.5);
                }
            }

            // 內建平衡模式下的法術半徑 (祭司 2.5 倍，即 500 * 2.5 = 1250)
            bool supportsSpellRadius = SupportsConfigurableSpellRadius(key);
            double spellRadius = supportsSpellRadius ? (500 * 2.5) : 0;

            return new double[] { hp, dmg, vw, aw, speed, sight, relt, range, spellRadius };
        }
    }
}
