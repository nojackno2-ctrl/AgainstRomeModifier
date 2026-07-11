using System;
using System.Collections.Generic;

namespace AgainstRomeModifier.Core.Services
{
    public class PatchOptions
    {
        public bool FocusLoss { get; set; }
        public bool FastCiviProduction { get; set; }
        public bool InfiniteMorale { get; set; }
        public bool FreeProduction { get; set; }
        public bool FreeUpgrade { get; set; }
        public bool NoSpellCost { get; set; }
        public bool MaxPopulation { get; set; }
        public bool Balance { get; set; }
        public bool HousingCapacity20x { get; set; }
        public bool StorageCapacity10x { get; set; }
        public bool HqHp10x { get; set; }
        public bool FastBuildUpgradeRepair { get; set; }
        public bool FoodHealing10x { get; set; }
        public bool VillageBuildRange { get; set; }
        public bool DgVoodoo { get; set; }
        public bool ToEnglish { get; set; }
        public int GameSpeed { get; set; }
        public bool NoSpellAltar { get; set; }

        // 無盡模式 AI 模組終極狀態：以模組 Id ("M1".."M6") 為鍵。
        // 先前為 bool[6]，靠索引對位 UserModules[i]，一旦模組順序調整即錯位；
        // 改用字典後對應變為顯式、與 UserModules 的排列順序無關。
        public Dictionary<string, bool> EndlessAiModules { get; set; } = new Dictionary<string, bool>(StringComparer.OrdinalIgnoreCase);

        /// <summary>讀取指定模組 Id 的終極狀態；未設定時視為 false（未啟用）。</summary>
        public bool GetEndlessAiModule(string moduleId) =>
            EndlessAiModules.TryGetValue(moduleId, out bool enabled) && enabled;

        // Custom unit stats data override
        public Dictionary<string, double[]>? CustomUnitStats { get; set; }
        public string PresetFileSourceType { get; set; } = "default";
        public string PresetFileName { get; set; } = "";
    }
}
