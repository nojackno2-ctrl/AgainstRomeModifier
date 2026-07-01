using System;
using System.Collections.Generic;

namespace AgainstRomeModifier {
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
        Aw = 142,
        Vw = 146,
        HousingCapacity = 156,
        Bmovs = 191,
        Weapon1Dtyp = 199
    }

    /// <summary>
    /// 定義 ress.ini CSV 欄位的常用索引
    /// </summary>
    public enum RessIndex {
        // objres 建築建造/修復費
        BauBuildWood = 2,
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

        // 兵種元數據字典：包含 [陣營名稱, 階級, 類型分類, 裝備特性分類] 的 Tuple 對應
        public static readonly Dictionary<string, Tuple<string, string, string, string>> UnitMeta = new Dictionary<string, Tuple<string, string, string, string>> {
            {"FigRomInf00_Lanze_Schild", Tuple.Create("Roman", "mid", "melee_inf", "shield")},
            {"FigRomSch00_Speer_Schild", Tuple.Create("Roman", "high", "hybrid_inf", "shield")},
            {"FigRomInf01_Schwert_Schild", Tuple.Create("Roman", "high", "melee_inf", "shield")},
            {"FigRomSch01_Bogen", Tuple.Create("Roman", "mid", "ranged_inf", "ranged")},
            {"FigRomKav00_Schwert_Schild", Tuple.Create("Roman", "ace", "cav", "shield")},
            {"FigRomAnf00_Anfuehrer", Tuple.Create("Roman", "leader", "leader_melee", "none")},
            {"FigGerInf01_Schwert", Tuple.Create("Teuton", "low", "melee_inf", "none")},
            {"FigGerSch00_Speer", Tuple.Create("Teuton", "low", "ranged_inf", "two_handed")},
            {"FigGerInf00_Hammer_Schild", Tuple.Create("Teuton", "mid", "melee_inf", "shield")},
            {"FigGerSch01_Axt_Schild", Tuple.Create("Teuton", "mid", "hybrid_inf", "shield")},
            {"FigGerInf02_Zweihandaxt", Tuple.Create("Teuton", "high", "melee_inf", "two_handed")},
            {"FigGerKav00_Schwert_Schild", Tuple.Create("Teuton", "high", "cav", "shield")},
            {"FigGerInf03_Doppelhammer", Tuple.Create("Teuton", "ace", "melee_inf", "dual_wield")},
            {"FigGerAnf00_Anfuehrer", Tuple.Create("Teuton", "leader", "leader_melee", "two_handed")},
            {"FigGerPri00_Priester", Tuple.Create("Teuton", "mid", "priest", "none")},
            {"FigKelInf00_Schwert", Tuple.Create("Celt", "low", "melee_inf", "none")},
            {"FigKelSch00_Bogen", Tuple.Create("Celt", "low", "ranged_inf", "ranged")},
            {"FigKelInf01_Lanze", Tuple.Create("Celt", "mid", "melee_inf", "shield")},
            {"FigKelSch01_Schleuder", Tuple.Create("Celt", "mid", "ranged_inf", "ranged")},
            {"FigKelInf02_Doppelschwert", Tuple.Create("Celt", "high", "melee_inf", "dual_wield")},
            {"FigKelSch02_Schwere_Schleuder", Tuple.Create("Celt", "ace", "ranged_inf", "ranged")},
            {"FigKelKav00_Lanze_Schild", Tuple.Create("Celt", "high", "cav", "shield")},
            {"FigKelAnf00_Anfuehrer", Tuple.Create("Celt", "leader", "leader_melee", "shield")},
            {"FigKelPri00_Priester", Tuple.Create("Celt", "mid", "priest", "none")},
            {"FigHunInf00_Keule", Tuple.Create("Hun", "low", "melee_inf", "none")},
            {"FigHunSch00_Bogen", Tuple.Create("Hun", "low", "ranged_inf", "ranged")},
            {"FigHunInf01_Schwert_Schild", Tuple.Create("Hun", "mid", "melee_inf", "shield")},
            {"FigHunKav00_Schwert_Schild", Tuple.Create("Hun", "mid", "cav", "shield")},
            {"FigHunKav01_Bogen", Tuple.Create("Hun", "high", "ranged_cav", "ranged")},
            {"FigHunKav02_Lanze_Schild", Tuple.Create("Hun", "ace", "cav", "shield")},
            {"FigHunKav03_Geisterreiter", Tuple.Create("Hun", "high", "cav", "none")},
            {"FigHunAnf00_Anfuehrer", Tuple.Create("Hun", "leader", "leader_cav", "none")},
            {"FigHunPri00_Priester", Tuple.Create("Hun", "mid", "priest", "none")},
            {"FigGerArt00_Katapult", Tuple.Create("Teuton", "siege", "siege", "none")},
            {"FigGerArt00_Katapult_Aufbau", Tuple.Create("Teuton", "siege", "siege", "none")},
            {"FigRomArt00_Speerschleuder", Tuple.Create("Roman", "siege", "siege", "none")},
            {"FigRomArt00_Speerschleuder_Auf", Tuple.Create("Roman", "siege", "siege", "none")},
            {"FigRomArt01_Katapult", Tuple.Create("Roman", "siege", "siege", "none")},
            {"FigRomArt01_Katapult_Aufbau", Tuple.Create("Roman", "siege", "siege", "none")},
            {"FigKelArt00_Speerschleuder", Tuple.Create("Celt", "siege", "siege", "none")},
            {"FigKelArt00_Speerschleuder_A", Tuple.Create("Celt", "siege", "siege", "none")},
            {"FigKelArt01_Katapult", Tuple.Create("Celt", "siege", "siege", "none")},
            {"FigKelArt01_Katapult_Aufbau", Tuple.Create("Celt", "siege", "siege", "none")}
        };

        // 計算並獲取各陣營指定兵種的平衡基礎數值 (回傳 [HP, 傷害, 防禦力 VW, 戰鬥力 AW] 陣列)
        public static double[] CalculateFactionBaseStats(string key, string faction, string tier, string utype) {
            // 祭司與攻城武器有獨立的平衡計算豁免，在此處不返回覆蓋資料
            if (utype == "priest" || utype == "siege") {
                return new double[] { 0, 0, 0, 0 };
            }

            // 1. 特定單位屬性特化 (防禦特化、王牌/精銳兵種特化)
            if (key == "FigKelInf01_Lanze") {
                // 塞爾特槍兵 (槍盾兵)：防禦特化 (VW 加成後達 42)，高生命，適度傷害
                return new double[] { 180, 22, 32, 18 };
            }
            if (key == "FigRomInf00_Lanze_Schild") {
                // 羅馬輕裝步兵：防禦特化，中階盾兵
                return new double[] { 130, 24, 22, 18 };
            }
            if (key == "FigRomSch00_Speer_Schild") {
                // 羅馬重裝步兵 (遠程)：高生命、高防禦
                return new double[] { 140, 25, 26, 24 };
            }
            if (key == "FigRomInf01_Schwert_Schild") {
                // 羅馬禁衛軍：最強步兵單位，高生命高攻防
                return new double[] { 200, 36, 28, 28 };
            }
            if (key == "FigHunInf01_Schwert_Schild") {
                // 匈奴劍盾兵：匈奴唯一防禦特化盾兵
                return new double[] { 140, 24, 24, 20 };
            }
            if (key == "FigKelInf02_Doppelschwert") {
                // 塞爾特雙劍兵：中高階雙持，高傷害高戰鬥力
                return new double[] { 130, 40, 15, 28 };
            }
            if (key == "FigGerInf03_Doppelhammer") {
                // 條頓雙錘兵：王牌雙持，極高傷害與戰鬥力
                return new double[] { 150, 60, 16, 34 };
            }

            // 2. 通用階級生命值 (階梯化 HP)
            double hp = 100;
            if (tier == "low") hp = 110;
            else if (tier == "mid") hp = 130;
            else if (tier == "high") hp = 150;
            else if (tier == "ace") hp = 160;
            else if (tier == "leader") hp = 450;

            double maxDam = 0;
            double vw = 0;
            double aw = 0;

            // 3. 通用屬性矩陣 (調降秒殺傷害，微調攻防比例)
            if (faction == "Roman") {
                if (tier == "low") {
                    if (utype == "melee_inf") { vw = 8; aw = 12; maxDam = 20; }
                } else if (tier == "mid") {
                    if (utype == "melee_inf") { vw = 14; aw = 20; maxDam = 28; } else if (utype == "ranged_inf") { vw = 12; aw = 24; maxDam = 22; }
                } else if (tier == "high") {
                    if (utype == "melee_inf") { vw = 20; aw = 22; maxDam = 42; } else if (utype == "ranged_inf") { vw = 16; aw = 26; maxDam = 30; } else if (utype == "hybrid_inf") { vw = 18; aw = 22; maxDam = 38; }
                } else if (tier == "ace") {
                    if (utype == "cav") { vw = 24; aw = 26; maxDam = 50; } // 羅馬突擊騎兵
                } else if (tier == "leader") {
                    if (utype == "leader_melee") { vw = 28; aw = 36; maxDam = 80; }
                }
            } else if (faction == "Teuton") {
                if (tier == "low") {
                    if (utype == "melee_inf") { vw = 10; aw = 12; maxDam = 25; } else if (utype == "ranged_inf") { vw = 6; aw = 12; maxDam = 20; }
                } else if (tier == "mid") {
                    if (utype == "melee_inf") { vw = 16; aw = 22; maxDam = 32; } else if (utype == "hybrid_inf") { vw = 14; aw = 20; maxDam = 28; }
                } else if (tier == "high") {
                    if (utype == "melee_inf") { vw = 14; aw = 26; maxDam = 38; } else if (utype == "cav") { vw = 20; aw = 24; maxDam = 42; }
                } else if (tier == "ace") {
                    if (utype == "melee_inf") { vw = 12; aw = 30; maxDam = 65; }
                } else if (tier == "leader") {
                    if (utype == "leader_melee") { vw = 26; aw = 38; maxDam = 70; }
                }
            } else if (faction == "Celt") {
                if (tier == "low") {
                    if (utype == "melee_inf") { vw = 10; aw = 12; maxDam = 24; } else if (utype == "ranged_inf") { vw = 8; aw = 12; maxDam = 20; }
                } else if (tier == "mid") {
                    if (utype == "melee_inf") { vw = 18; aw = 18; maxDam = 24; } else if (utype == "ranged_inf") { vw = 12; aw = 18; maxDam = 20; }
                } else if (tier == "high") {
                    if (utype == "melee_inf") { vw = 12; aw = 24; maxDam = 38; } else if (utype == "cav") { vw = 22; aw = 22; maxDam = 38; } // 塞爾特槍騎兵
                } else if (tier == "ace") {
                    if (utype == "ranged_inf") { vw = 18; aw = 25; maxDam = 65; }
                } else if (tier == "leader") {
                    if (utype == "leader_melee") { vw = 30; aw = 28; maxDam = 60; }
                }
            } else if (faction == "Hun") {
                if (tier == "low") {
                    if (utype == "melee_inf") { vw = 10; aw = 10; maxDam = 26; } else if (utype == "ranged_inf") { vw = 8; aw = 12; maxDam = 20; }
                } else if (tier == "mid") {
                    if (utype == "melee_inf") { vw = 12; aw = 18; maxDam = 24; } else if (utype == "cav") { vw = 16; aw = 20; maxDam = 32; } // 匈奴輕裝騎兵
                } else if (tier == "high") {
                    if (utype == "melee_inf") { vw = 8; aw = 22; maxDam = 36; }
                    else if (utype == "ranged_inf") { vw = 16; aw = 24; maxDam = 32; }
                    else if (utype == "cav") { vw = 18; aw = 26; maxDam = 45; } // 匈奴幽靈武士
                    else if (utype == "ranged_cav") { vw = 16; aw = 24; maxDam = 36; } // 匈奴弓騎兵
                } else if (tier == "ace") {
                    if (utype == "cav") { vw = 22; aw = 26; maxDam = 52; } // 匈奴重裝騎兵
                } else if (tier == "leader") {
                    if (utype == "leader_cav") { vw = 25; aw = 36; maxDam = 80; }
                }
            }

            return new double[] { hp, maxDam, vw, aw };
        }

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

            var sorted = new List<string>(UnitOrder);
            sorted.Sort((a, b) => {
                string tierA = UnitMeta.ContainsKey(a) ? UnitMeta[a].Item2 : "low";
                string tierB = UnitMeta.ContainsKey(b) ? UnitMeta[b].Item2 : "low";
                int pA = tierPriority.ContainsKey(tierA) ? tierPriority[tierA] : 99;
                int pB = tierPriority.ContainsKey(tierB) ? tierPriority[tierB] : 99;
                if (pA != pB) return pA.CompareTo(pB);
                return UnitOrder.IndexOf(a).CompareTo(UnitOrder.IndexOf(b));
            });

            UnitOrder.Clear();
            UnitOrder.AddRange(sorted);
        }
    }
}
