//! Remaining TroopConfig.cs data contract: RessIndex/VolkresIndex, case-sensitive
//! table lookups and the static-constructor invariants. Units, names, metadata and
//! balanced stats live in `troops.rs`.
use crate::troops::{TroopConfig, UnitMetadata};

/// C# `RessIndex`: ress.ini CSV column indices. The C# enum has two aliased pairs
/// (`FigSiegeBuildCost*` == `BauBuildCost*`), exposed here as associated consts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RessIndex {
    BauBuildCostStart = 1,
    BauBuildCostEnd = 6,
    BauUpgradeCostStart = 7,
    BauUpgradeCostEnd = 12,
    FigProdCostStart = 13,
    FigProdCostEnd = 18,
    FigPriestSpellCostStart = 25,
    FigPriestSpellCostEnd = 28,
}

impl RessIndex {
    pub const FIG_SIEGE_BUILD_COST_START: RessIndex = RessIndex::BauBuildCostStart;
    pub const FIG_SIEGE_BUILD_COST_END: RessIndex = RessIndex::BauBuildCostEnd;
}

/// C# `VolkresIndex`: ress.ini `[volkres]` CSV column indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolkresIndex {
    ResearchUpgradeWood1 = 8,
    ResearchUpgradeGold1 = 10,
    ResearchUpgradeWood2 = 12,
    ResearchUpgradeGold2 = 14,
    TechCostStart = 24,
    TechCostEnd = 263,
    UnitUpgradeStart = 264,
    UnitUpgradeEnd = 295,
}

/// Case-sensitive lookup: C# `UnitNames` uses the default (ordinal) comparer.
pub fn unit_name_exact(key: &str) -> Option<&'static str> {
    TroopConfig::UNIT_NAME_ENTRIES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, n)| *n)
}

/// Case-sensitive lookup: C# `UnitMeta` uses the default (ordinal) comparer.
pub fn unit_meta_exact(key: &str) -> Option<&'static UnitMetadata> {
    TroopConfig::UNIT_METADATA_ENTRIES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, m)| m)
}

/// C# `BalancedUnitStats` uses `OrdinalIgnoreCase`.
pub fn balanced_stats_ignore_case(key: &str) -> Option<&'static [f64; 9]> {
    TroopConfig::get_balanced_stats(key)
}

/// Mirrors the C# static constructor's invariant checks; the error text is the
/// C# `InvalidOperationException` message.
pub fn validate_tables() -> Result<(), String> {
    if TroopConfig::BALANCED_STATS_ENTRIES.len() != TroopConfig::UNIT_METADATA_ENTRIES.len() {
        return Err("內建兵種平衡表與兵種 metadata 數量不一致。".to_string());
    }
    for key in TroopConfig::sorted_units() {
        if TroopConfig::get_balanced_stats(key).is_none() {
            return Err(format!("兵種缺少完整九項最終平衡值: {key}"));
        }
    }
    Ok(())
}
