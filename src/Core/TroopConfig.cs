using System;
using System.Collections.Generic;

namespace AgainstRomeModifier {
    /// <summary>
    /// 兵種元數據：具名欄位取代先前的 Tuple&lt;string,string,string,string&gt;。
    /// Faction 陣營 (Roman/Teuton/Celt/Hun)、Tier 階級 (low/mid/high/ace/leader/siege)、
    /// UnitType 類型分類 (melee_inf/ranged_inf/ranged_cav/hybrid_inf/cav/leader_melee/
    /// leader_cav/priest/siege)、Style 裝備特性 (shield/two_handed/dual_wield/ranged/none)。
    /// </summary>
    public sealed record UnitMetadata(string Faction, string Tier, string UnitType, string Style);

    /// <summary>
    /// 定義 objdef.dau CSV 欄位的常用索引
    /// </summary>
    public enum ObjdefIndex {
        Moves = 4,
        Hp = 19,
        Movsf = 23,
        Sirad = 24,
        Name = 52,
        Weapon1Akti = 78,
        Weapon1Dam = 79,
        Weapon1RangeMin = 80,
        Weapon1RangeMax = 81,
        Weapon1Relt = 84,
        Weapon1Emit = 85,
        Weapon1Drad = 164,
        Aw = 142,
        Vw = 146,
        HousingCapacity = 156,
        Bmovs = 191,
        Weapon1Dtyp = 199,
        StorageCapacity = 42
    }

    /// <summary>
    /// 定義 ress.ini CSV 欄位的常用索引
    /// </summary>
    public enum RessIndex {
        // objres 建築建造/修復費
        BauBuildCostStart = 1,
        BauBuildCostEnd = 6,
        BauUpgradeCostStart = 7,
        BauUpgradeCostEnd = 12,
        // objres 單位生產與解除返還
        FigProdCostStart = 13,
        FigProdCostEnd = 18,
        FigPriestSpellCostStart = 25,
        FigPriestSpellCostEnd = 28,
        // objres 攻城武器建造費
        FigSiegeBuildCostStart = 1,
        FigSiegeBuildCostEnd = 6
        // volkres 單位升級費範圍
    }

    /// <summary>
    /// 定義 ress.ini 中 [volkres] 區段 CSV 欄位的常用索引
    /// </summary>
    public enum VolkresIndex {
        ResearchUpgradeWood1 = 8,
        ResearchUpgradeGold1 = 10,
        ResearchUpgradeWood2 = 12,
        ResearchUpgradeGold2 = 14,
        TechCostStart = 24,
        TechCostEnd = 263,
        UnitUpgradeStart = 264,
        UnitUpgradeEnd = 295
    }

    // 儲存與處理遊戲兵種相關設定的靜態配置類別
    public static class TroopConfig {
        // 對應遊戲兵種 ID 與其繁體中文名稱的字典
        public static readonly Dictionary<string, string> UnitNames = new Dictionary<string, string> {
            {"FigRomAnf00_Anfuehrer", "羅馬領袖"},
            {"FigRomInf00_Lanze_Schild", "羅馬輕裝步兵"},
            {"FigRomInf01_Schwert_Schild", "羅馬劍盾兵"},
            {"FigRomKav00_Schwert_Schild", "羅馬突擊騎兵"},
            {"FigRomSch00_Speer_Schild", "羅馬重裝步兵"},
            {"FigRomSch01_Bogen", "羅馬弓箭手"},
            {"FigGerAnf00_Anfuehrer", "條頓領袖"},
            {"FigGerInf00_Hammer_Schild", "條頓錘盾兵"},
            {"FigGerInf01_Schwert", "條頓劍士"},
            {"FigGerInf02_Zweihandaxt", "條頓雙手斧兵"},
            {"FigGerInf03_Doppelhammer", "條頓雙錘兵"},
            {"FigGerKav00_Schwert_Schild", "條頓騎兵"},
            {"FigGerPri00_Priester", "條頓祭司"},
            {"FigGerSch00_Speer", "條頓矛兵"},
            {"FigGerSch01_Axt_Schild", "條頓斧盾兵"},
            {"FigKelAnf00_Anfuehrer", "塞爾特領袖"},
            {"FigKelInf00_Schwert", "塞爾特劍士"},
            {"FigKelInf01_Lanze", "塞爾特槍兵"},
            {"FigKelInf02_Doppelschwert", "塞爾特重裝步兵"},
            {"FigKelKav00_Lanze_Schild", "塞爾特槍騎兵"},
            {"FigKelPri00_Priester", "塞爾特祭司"},
            {"FigKelSch00_Bogen", "塞爾特弓箭手"},
            {"FigKelSch01_Schleuder", "塞爾特投石兵"},
            {"FigKelSch02_Schwere_Schleuder", "塞爾特重裝投石兵"},
            {"FigHunAnf00_Anfuehrer", "匈奴領袖"},
            {"FigHunInf00_Keule", "匈奴棍棒兵"},
            {"FigHunInf01_Schwert_Schild", "匈奴劍盾兵"},
            {"FigHunKav00_Schwert_Schild", "匈奴輕裝騎兵"},
            {"FigHunKav01_Bogen", "匈奴弓騎兵"},
            {"FigHunKav02_Lanze_Schild", "匈奴重裝騎兵"},
            {"FigHunKav03_Geisterreiter", "匈奴幽靈武士"},
            {"FigHunPri00_Priester", "匈奴祭司"},
            {"FigHunSch00_Bogen", "匈奴弓箭手"},
            {"FigGerArt00_Katapult", "條頓投石機"},
            {"FigGerArt00_Katapult_Aufbau", "條頓投石機(架設)"},
            {"FigRomArt00_Speerschleuder", "羅馬弩車"},
            {"FigRomArt00_Speerschleuder_Auf", "羅馬弩車(架設)"},
            {"FigRomArt01_Katapult", "羅馬投石機"},
            {"FigRomArt01_Katapult_Aufbau", "羅馬投石機(架設)"},
            {"FigKelArt00_Speerschleuder", "塞爾特弩車"},
            {"FigKelArt00_Speerschleuder_A", "塞爾特弩車(架設)"},
            {"FigKelArt01_Katapult", "塞爾特投石機"},
            {"FigKelArt01_Katapult_Aufbau", "塞爾特投石機(架設)"}
        };

        // 規定介面上兵種排列順序的清單
        public static readonly List<string> UnitOrder = new List<string> {
            "FigRomInf00_Lanze_Schild", "FigRomSch00_Speer_Schild", "FigRomInf01_Schwert_Schild",
            "FigRomSch01_Bogen", "FigRomKav00_Schwert_Schild", "FigRomAnf00_Anfuehrer",
            "FigGerInf01_Schwert", "FigGerSch00_Speer", "FigGerInf00_Hammer_Schild",
            "FigGerSch01_Axt_Schild", "FigGerInf02_Zweihandaxt", "FigGerKav00_Schwert_Schild",
            "FigGerInf03_Doppelhammer", "FigGerAnf00_Anfuehrer", "FigGerPri00_Priester",
            "FigKelInf00_Schwert", "FigKelSch00_Bogen", "FigKelInf01_Lanze",
            "FigKelSch01_Schleuder", "FigKelInf02_Doppelschwert", "FigKelSch02_Schwere_Schleuder",
            "FigKelKav00_Lanze_Schild", "FigKelAnf00_Anfuehrer", "FigKelPri00_Priester",
            "FigHunInf00_Keule", "FigHunSch00_Bogen", "FigHunInf01_Schwert_Schild",
            "FigHunKav00_Schwert_Schild", "FigHunKav01_Bogen", "FigHunKav02_Lanze_Schild",
            "FigHunKav03_Geisterreiter", "FigHunAnf00_Anfuehrer", "FigHunPri00_Priester",
            "FigGerArt00_Katapult", "FigGerArt00_Katapult_Aufbau",
            "FigRomArt00_Speerschleuder", "FigRomArt00_Speerschleuder_Auf",
            "FigRomArt01_Katapult", "FigRomArt01_Katapult_Aufbau",
            "FigKelArt00_Speerschleuder", "FigKelArt00_Speerschleuder_A",
            "FigKelArt01_Katapult", "FigKelArt01_Katapult_Aufbau"
        };

        // 兵種元數據字典：具名欄位 (Faction 陣營, Tier 階級, UnitType 類型分類, Style 裝備特性分類)
        public static readonly Dictionary<string, UnitMetadata> UnitMeta = new Dictionary<string, UnitMetadata> {
            {"FigRomInf00_Lanze_Schild", new UnitMetadata("Roman", "mid", "melee_inf", "shield")},
            {"FigRomSch00_Speer_Schild", new UnitMetadata("Roman", "high", "hybrid_inf", "shield")},
            {"FigRomInf01_Schwert_Schild", new UnitMetadata("Roman", "high", "melee_inf", "shield")},
            {"FigRomSch01_Bogen", new UnitMetadata("Roman", "mid", "ranged_inf", "ranged")},
            {"FigRomKav00_Schwert_Schild", new UnitMetadata("Roman", "ace", "cav", "shield")},
            {"FigRomAnf00_Anfuehrer", new UnitMetadata("Roman", "leader", "leader_melee", "none")},
            {"FigGerInf01_Schwert", new UnitMetadata("Teuton", "low", "melee_inf", "none")},
            {"FigGerSch00_Speer", new UnitMetadata("Teuton", "low", "ranged_inf", "two_handed")},
            {"FigGerInf00_Hammer_Schild", new UnitMetadata("Teuton", "mid", "melee_inf", "shield")},
            {"FigGerSch01_Axt_Schild", new UnitMetadata("Teuton", "mid", "hybrid_inf", "shield")},
            {"FigGerInf02_Zweihandaxt", new UnitMetadata("Teuton", "high", "melee_inf", "two_handed")},
            {"FigGerKav00_Schwert_Schild", new UnitMetadata("Teuton", "high", "cav", "shield")},
            {"FigGerInf03_Doppelhammer", new UnitMetadata("Teuton", "ace", "melee_inf", "dual_wield")},
            {"FigGerAnf00_Anfuehrer", new UnitMetadata("Teuton", "leader", "leader_melee", "two_handed")},
            {"FigGerPri00_Priester", new UnitMetadata("Teuton", "mid", "priest", "none")},
            {"FigKelInf00_Schwert", new UnitMetadata("Celt", "low", "melee_inf", "none")},
            {"FigKelSch00_Bogen", new UnitMetadata("Celt", "low", "ranged_inf", "ranged")},
            {"FigKelInf01_Lanze", new UnitMetadata("Celt", "mid", "melee_inf", "shield")},
            {"FigKelSch01_Schleuder", new UnitMetadata("Celt", "mid", "ranged_inf", "ranged")},
            {"FigKelInf02_Doppelschwert", new UnitMetadata("Celt", "high", "melee_inf", "dual_wield")},
            {"FigKelSch02_Schwere_Schleuder", new UnitMetadata("Celt", "ace", "ranged_inf", "ranged")},
            {"FigKelKav00_Lanze_Schild", new UnitMetadata("Celt", "high", "cav", "shield")},
            {"FigKelAnf00_Anfuehrer", new UnitMetadata("Celt", "leader", "leader_melee", "shield")},
            {"FigKelPri00_Priester", new UnitMetadata("Celt", "mid", "priest", "none")},
            {"FigHunInf00_Keule", new UnitMetadata("Hun", "low", "melee_inf", "none")},
            {"FigHunSch00_Bogen", new UnitMetadata("Hun", "low", "ranged_inf", "ranged")},
            {"FigHunInf01_Schwert_Schild", new UnitMetadata("Hun", "mid", "melee_inf", "shield")},
            {"FigHunKav00_Schwert_Schild", new UnitMetadata("Hun", "mid", "cav", "shield")},
            {"FigHunKav01_Bogen", new UnitMetadata("Hun", "high", "ranged_cav", "ranged")},
            {"FigHunKav02_Lanze_Schild", new UnitMetadata("Hun", "ace", "cav", "shield")},
            {"FigHunKav03_Geisterreiter", new UnitMetadata("Hun", "high", "cav", "none")},
            {"FigHunAnf00_Anfuehrer", new UnitMetadata("Hun", "leader", "leader_cav", "none")},
            {"FigHunPri00_Priester", new UnitMetadata("Hun", "mid", "priest", "none")},
            {"FigGerArt00_Katapult", new UnitMetadata("Teuton", "siege", "siege", "none")},
            {"FigGerArt00_Katapult_Aufbau", new UnitMetadata("Teuton", "siege", "siege", "none")},
            {"FigRomArt00_Speerschleuder", new UnitMetadata("Roman", "siege", "siege", "none")},
            {"FigRomArt00_Speerschleuder_Auf", new UnitMetadata("Roman", "siege", "siege", "none")},
            {"FigRomArt01_Katapult", new UnitMetadata("Roman", "siege", "siege", "none")},
            {"FigRomArt01_Katapult_Aufbau", new UnitMetadata("Roman", "siege", "siege", "none")},
            {"FigKelArt00_Speerschleuder", new UnitMetadata("Celt", "siege", "siege", "none")},
            {"FigKelArt00_Speerschleuder_A", new UnitMetadata("Celt", "siege", "siege", "none")},
            {"FigKelArt01_Katapult", new UnitMetadata("Celt", "siege", "siege", "none")},
            {"FigKelArt01_Katapult_Aufbau", new UnitMetadata("Celt", "siege", "siege", "none")}
        };

        // 內建平衡的最終九項屬性：HP, Damage, VW, AW, Speed, Sight,
        // Cooldown, Range, SpellRadius。這些值不再套用通用階級、盾牌、雙手
        // 武器或兵種類型倍率，避免預覽值與實際寫入值不一致。
        public static readonly Dictionary<string, double[]> BalancedUnitStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase) {
            // Roman：整體素質最強。
            {"FigRomInf00_Lanze_Schild", new double[] {140, 26, 28, 22, 3.2, 1500, 500, 400, 0}},
            {"FigRomSch00_Speer_Schild", new double[] {180, 34, 34, 30, 3.2, 4500, 400, 2700, 0}},
            {"FigRomInf01_Schwert_Schild", new double[] {210, 38, 36, 32, 3.2, 1500, 500, 400, 0}},
            {"FigRomSch01_Bogen", new double[] {140, 24, 14, 24, 3.2, 4500, 700, 3600, 0}},
            {"FigRomKav00_Schwert_Schild", new double[] {190, 54, 32, 32, 3.2, 1500, 500, 400, 0}},
            {"FigRomAnf00_Anfuehrer", new double[] {500, 85, 32, 40, 3.2, 1500, 500, 128, 0}},
            {"FigRomArt00_Speerschleuder", new double[] {1100, 180, 42, 62, 0, 4500, 3000, 3900, 0}},
            {"FigRomArt00_Speerschleuder_Auf", new double[] {1100, 180, 42, 62, 0, 4500, 3000, 3900, 0}},
            {"FigRomArt01_Katapult", new double[] {1600, 260, 42, 62, 0, 4500, 6000, 3600, 0}},
            {"FigRomArt01_Katapult_Aufbau", new double[] {1600, 260, 42, 62, 0, 4500, 6000, 3600, 0}},

            // Teuton：近戰傷害與爆發最高。
            {"FigGerInf01_Schwert", new double[] {110, 28, 10, 14, 3.2, 1500, 500, 400, 0}},
            {"FigGerSch00_Speer", new double[] {110, 28, 8, 14, 3.2, 4500, 900, 2700, 0}},
            {"FigGerInf00_Hammer_Schild", new double[] {140, 36, 22, 26, 3.2, 1500, 500, 400, 0}},
            {"FigGerSch01_Axt_Schild", new double[] {140, 34, 20, 24, 3.2, 4500, 400, 2100, 0}},
            {"FigGerInf02_Zweihandaxt", new double[] {165, 54, 14, 30, 3.2, 1500, 500, 400, 0}},
            {"FigGerKav00_Schwert_Schild", new double[] {165, 46, 24, 28, 3.2, 1500, 500, 400, 0}},
            {"FigGerInf03_Doppelhammer", new double[] {180, 68, 16, 38, 3.2, 1500, 333, 400, 0}},
            {"FigGerAnf00_Anfuehrer", new double[] {480, 96, 28, 42, 3.2, 1500, 500, 128, 0}},
            {"FigGerPri00_Priester", new double[] {110, 10, 70, 30, 2.6, 45000, 500, 3840, 0}},
            {"FigGerArt00_Katapult", new double[] {1500, 170, 50, 50, 0, 4500, 5000, 3000, 0}},
            {"FigGerArt00_Katapult_Aufbau", new double[] {1500, 170, 50, 50, 0, 4500, 5000, 3000, 0}},

            // Celt：步兵防禦與步行遠程最強，投石兵以高單發傷害輸出。
            {"FigKelInf00_Schwert", new double[] {120, 24, 12, 14, 3.2, 1500, 500, 400, 0}},
            {"FigKelSch00_Bogen", new double[] {110, 18, 8, 14, 3.2, 4500, 550, 3300, 0}},
            {"FigKelInf01_Lanze", new double[] {190, 26, 42, 24, 3.2, 1500, 500, 400, 0}},
            {"FigKelSch01_Schleuder", new double[] {140, 40, 12, 20, 3.2, 4500, 900, 3000, 0}},
            {"FigKelInf02_Doppelschwert", new double[] {160, 46, 16, 32, 3.2, 1500, 333, 400, 0}},
            {"FigKelSch02_Schwere_Schleuder", new double[] {180, 75, 20, 28, 3.2, 4500, 1000, 2700, 0}},
            {"FigKelKav00_Lanze_Schild", new double[] {170, 44, 30, 28, 3.2, 1500, 500, 400, 0}},
            {"FigKelAnf00_Anfuehrer", new double[] {480, 68, 42, 36, 3.2, 1500, 500, 128, 0}},
            {"FigKelPri00_Priester", new double[] {110, 10, 80, 60, 2.6, 45000, 500, 3840, 1250}},
            {"FigKelArt00_Speerschleuder", new double[] {1000, 110, 42, 60, 0, 4500, 3000, 3300, 0}},
            {"FigKelArt00_Speerschleuder_A", new double[] {1000, 110, 42, 60, 0, 4500, 3000, 3300, 0}},
            {"FigKelArt01_Katapult", new double[] {1500, 190, 52, 52, 0, 4500, 5000, 3000, 0}},
            {"FigKelArt01_Katapult_Aufbau", new double[] {1500, 190, 52, 52, 0, 4500, 5000, 3000, 0}},

            // Hun：騎兵整體最強，弓騎兵以低單發、高射速輸出。
            {"FigHunInf00_Keule", new double[] {100, 26, 8, 12, 3.2, 1500, 500, 400, 0}},
            {"FigHunSch00_Bogen", new double[] {100, 18, 8, 12, 3.2, 4500, 450, 3000, 0}},
            {"FigHunInf01_Schwert_Schild", new double[] {130, 26, 28, 22, 3.2, 1500, 500, 400, 0}},
            {"FigHunKav00_Schwert_Schild", new double[] {150, 40, 22, 26, 3.2, 1500, 500, 400, 0}},
            {"FigHunKav01_Bogen", new double[] {160, 24, 18, 28, 3.2, 4500, 350, 3000, 0}},
            {"FigHunKav02_Lanze_Schild", new double[] {210, 60, 34, 34, 3.2, 1500, 500, 400, 0}},
            {"FigHunKav03_Geisterreiter", new double[] {180, 52, 20, 32, 3.2, 1500, 500, 400, 0}},
            {"FigHunAnf00_Anfuehrer", new double[] {500, 90, 30, 42, 3.2, 1500, 500, 128, 0}},
            {"FigHunPri00_Priester", new double[] {110, 12, 30, 50, 2.6, 45000, 500, 3840, 1250}}
        };

        /// <summary>
        /// 靜態建構函式：在載入配置時，自動將 UnitOrder 依照兵種階級 (Tier) 進行穩定排序 (Stable Sort)，
        /// 優先級為：低階 -> 中階 -> 高階 -> 王牌 -> 領袖 -> 攻城武器。若階級相同則維持原本的陣營順序。
        /// </summary>
        static TroopConfig() {
            var tierPriority = new Dictionary<string, int> {
                { "low", 1 },
                { "mid", 2 },
                { "high", 3 },
                { "ace", 4 },
                { "leader", 5 },
                { "siege", 6 }
            };

            // 先建 index 對照表，避免比較器內 IndexOf 造成 O(n²) 查找
            var originalIndex = new Dictionary<string, int>(UnitOrder.Count);
            for (int i = 0; i < UnitOrder.Count; i++) {
                originalIndex[UnitOrder[i]] = i;
            }

            var sorted = new List<string>(UnitOrder);
            sorted.Sort((a, b) => {
                string tierA = UnitMeta.ContainsKey(a) ? UnitMeta[a].Tier : "low";
                string tierB = UnitMeta.ContainsKey(b) ? UnitMeta[b].Tier : "low";
                int pA = tierPriority.ContainsKey(tierA) ? tierPriority[tierA] : 99;
                int pB = tierPriority.ContainsKey(tierB) ? tierPriority[tierB] : 99;
                if (pA != pB) return pA.CompareTo(pB);
                return originalIndex[a].CompareTo(originalIndex[b]);
            });

            UnitOrder.Clear();
            UnitOrder.AddRange(sorted);

            if (BalancedUnitStats.Count != UnitMeta.Count) {
                throw new InvalidOperationException("內建兵種平衡表與兵種 metadata 數量不一致。");
            }
            foreach (string key in UnitOrder) {
                if (!BalancedUnitStats.TryGetValue(key, out double[]? stats) || stats.Length != 9) {
                    throw new InvalidOperationException("兵種缺少完整九項最終平衡值: " + key);
                }
            }
        }
    }
}
