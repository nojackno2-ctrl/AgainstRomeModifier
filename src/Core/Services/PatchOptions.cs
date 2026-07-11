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
        public bool FastBuildUpgradeRepair { get; set; }
        public bool FoodHealing10x { get; set; }
        public bool VillageBuildRange { get; set; }
        public bool DgVoodoo { get; set; }
        public bool ToEnglish { get; set; }
        public int GameSpeed { get; set; }
        public bool NoSpellAltar { get; set; }

        // AI Ultimate modules M1..M6
        public bool[] EndlessAiModules { get; set; } = new bool[6];

        // Custom unit stats data override
        public Dictionary<string, double[]>? CustomUnitStats { get; set; }
        public string PresetFileSourceType { get; set; } = "default";
        public string PresetFileName { get; set; } = "";
    }
}
