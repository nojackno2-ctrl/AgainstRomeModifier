//! Tribal building presets and resource palettes.

use super::models::{BuildingBlueprint, SettlementTribe};

pub struct SettlementTribalPresets;

impl SettlementTribalPresets {
    pub fn get_blueprints(tribe: SettlementTribe) -> (BuildingBlueprint, Vec<BuildingBlueprint>) {
        match tribe {
            SettlementTribe::Roman => (
                BuildingBlueprint::new("ROM_HAU00", "MainHouse", 4.5, 4.5, 0.0, 0.0),
                vec![
                    BuildingBlueprint::new("ROM_LAG00", "Warehouse", 3.2, 3.2, 5.0, 45.0),
                    BuildingBlueprint::new("ROM_WOH00", "House", 2.6, 2.6, 5.5, 105.0),
                    BuildingBlueprint::new("ROM_BAU00", "Farm", 3.8, 3.8, 7.0, 165.0),
                    BuildingBlueprint::new("ROM_WAF00", "Blacksmith", 3.0, 3.0, 6.0, 225.0),
                    BuildingBlueprint::new("ROM_STA00", "Stable", 2.2, 2.2, 8.5, 285.0),
                    BuildingBlueprint::new("ROM_SCHRE00", "Workshop", 3.0, 3.0, 8.5, 345.0),
                ],
            ),
            SettlementTribe::Celtic => (
                BuildingBlueprint::new("KEL_HAU00", "MainHouse", 4.2, 4.2, 0.0, 0.0),
                vec![
                    BuildingBlueprint::new("KEL_LAG00", "Warehouse", 3.0, 3.0, 5.0, 40.0),
                    BuildingBlueprint::new("KEL_WOH00", "House", 2.5, 2.5, 5.2, 105.0),
                    BuildingBlueprint::new("KEL_BAU00", "Farm", 3.5, 3.5, 6.8, 165.0),
                    BuildingBlueprint::new("KEL_WAF00", "Blacksmith", 2.8, 2.8, 6.0, 225.0),
                    BuildingBlueprint::new("KEL_STA00", "Stable", 2.0, 2.0, 8.2, 285.0),
                    BuildingBlueprint::new("KEL_SCHRE00", "Workshop", 3.0, 3.0, 8.5, 345.0),
                ],
            ),
            SettlementTribe::Hun => (
                BuildingBlueprint::new("HUN_HAU00", "MainHouse", 4.2, 4.2, 0.0, 0.0),
                vec![
                    BuildingBlueprint::new("HUN_LAG00", "Warehouse", 3.0, 3.0, 5.0, 45.0),
                    BuildingBlueprint::new("HUN_WOH00", "House", 2.5, 2.5, 5.0, 105.0),
                    BuildingBlueprint::new("HUN_SCHLA00", "Butcher", 3.6, 3.6, 7.0, 165.0),
                    BuildingBlueprint::new("HUN_WAF00", "Blacksmith", 2.8, 2.8, 6.2, 225.0),
                    BuildingBlueprint::new("HUN_STA00", "Stable", 2.0, 2.0, 8.0, 285.0),
                    BuildingBlueprint::new("HUN_SCHRE00", "Workshop", 3.0, 3.0, 8.5, 345.0),
                ],
            ),
            SettlementTribe::Germanic => (
                BuildingBlueprint::new("GER_HAU00", "MainHouse", 4.2, 4.2, 0.0, 0.0),
                vec![
                    BuildingBlueprint::new("GER_LAG00", "Warehouse", 3.0, 3.0, 5.2, 45.0),
                    BuildingBlueprint::new("GER_WOH00", "House", 2.6, 2.6, 5.5, 105.0),
                    BuildingBlueprint::new("GER_BAU00", "Farm", 3.6, 3.6, 7.2, 165.0),
                    BuildingBlueprint::new("GER_WAF00", "Blacksmith", 3.0, 3.0, 6.0, 225.0),
                    BuildingBlueprint::new("GER_STA00", "Stable", 2.2, 2.2, 8.5, 285.0),
                    BuildingBlueprint::new("GER_SCHRE00", "Workshop", 3.0, 3.0, 8.5, 345.0),
                ],
            ),
        }
    }

    pub fn get_forest_tree_palette(tribe: SettlementTribe) -> &'static [&'static str] {
        match tribe {
            SettlementTribe::Roman => &[
                "LanItaPin00_Pinie",
                "LanItaPin01_Pinie",
                "LanItaZyp00_Zypresse",
                "LanItaZyp01_Zypresse",
                "LanItaBus08_Kleiner_Busch",
            ],
            _ => &[
                "LanGerNad00_Tanne_gross",
                "LanGerNad05_Tanne_gross",
                "LanGerNad18_Tanne_klein",
                "LanGerNad24_Tanne_mittel",
                "LanGerNabu00_Nadelbusch",
            ],
        }
    }

    pub fn get_stone_quarry_palette(tribe: SettlementTribe) -> &'static [&'static str] {
        match tribe {
            SettlementTribe::Roman => &[
                "LanItaSte00_1Stein",
                "LanItaSte01_1Stein",
                "LanItaSte02_1Stein",
            ],
            _ => &[
                "LanGerSte00_1Stein",
                "LanGerSte01_1Stein",
                "LanGerSte02_1Stein",
                "LanGerSte05_1Stein",
            ],
        }
    }

    pub fn get_wildlife_palette() -> &'static [&'static str] {
        &["ALL_EBE00"]
    }
}
