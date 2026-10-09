
namespace AgainstRomeMapEditor.Modules.Settlement;

/// <summary>部族風格代碼（對應原生命名習慣）。</summary>
public enum SettlementTribe
{
    Germanic,
    Roman,
    Celtic,
    Hun
}

/// <summary>多勢力分佈對稱模式。</summary>
public enum SymmetryMode
{
    /// <summary>中心/點對稱（適用 2 人對戰）。</summary>
    CentralSymmetry,
    /// <summary>環狀旋轉對稱（適用 3-8 人對戰）。</summary>
    RotationalSymmetry,
    /// <summary>拓撲等距/Voronoi 鬆弛（適用非規則/非對稱地圖）。</summary>
    TopologicalEquidistant
}

/// <summary>建築類型規格與佔地大小（單位：地圖格 tile）。</summary>
public sealed record BuildingBlueprint(
    string TypeName,
    string Category,
    float FootprintWidth,
    float FootprintHeight,
    float PreferredDistanceToMain,
    float PreferredAngleDeg = 0f);

/// <summary>單棟規劃完成之建築物擺放資訊。</summary>
public sealed record PlacedBuildingPlan(
    string TypeName,
    float WorldX,
    float WorldZ,
    float HeightOffset,
    float Angle,
    int Team,
    float FootprintRadius);

/// <summary>單個自然資源物件規劃（樹木、岩石露頭、灌木）。</summary>
public sealed record NatureClusterItem(
    string TypeName,
    float WorldX,
    float WorldZ,
    float HeightOffset,
    float AngleDeg);

/// <summary>單個放置型資源/單位規劃（野生動物、礦井等）。</summary>
public sealed record PlacedResourceItem(
    string TypeName,
    float WorldX,
    float WorldZ,
    float HeightOffset,
    float AngleDeg,
    int Team = -1,
    int Count = 0);

/// <summary>聚落腹地評估指標結果。</summary>
public sealed record SettlementSiteEvaluationResult(
    bool IsValid,
    float TotalScore,
    float FlatnessScore,
    float WaterSafetyScore,
    float SpaceClearanceScore,
    float ExpansionPotentialScore,
    float AnchorTileX,
    float AnchorTileZ,
    float WorldX,
    float WorldZ,
    IReadOnlyList<PlacedBuildingPlan> PlannedBuildings,
    string? RejectionReason = null);

/// <summary>單一勢力聚落與週邊資源規劃總集。</summary>
public sealed record PlayerSettlementLayout(
    int TeamIndex,
    SettlementTribe Tribe,
    SettlementSiteEvaluationResult SiteEvaluation,
    IReadOnlyList<NatureClusterItem> ForestTrees,
    IReadOnlyList<NatureClusterItem> StoneQuarries,
    IReadOnlyList<PlacedResourceItem> WildlifeAndFood,
    IReadOnlyList<PlacedBuildingPlan> Buildings);

/// <summary>多勢力生成全域方案與公平性度量。</summary>
public sealed record MultiplayerDistributionResult(
    SymmetryMode Mode,
    int PlayerCount,
    IReadOnlyList<PlayerSettlementLayout> Players,
    FairnessScoreReport FairnessReport);

/// <summary>公平性指標評估報告。</summary>
public sealed record FairnessScoreReport(
    float AverageInterPlayerDistance,
    float MinInterPlayerDistance,
    float MaxInterPlayerDistance,
    float WoodResourceVariance,
    float StoneResourceVariance,
    float FoodResourceVariance,
    bool IsBalanced);

/// <summary>部族預設建築與生態配置調色盤。</summary>
// 名稱以 TEMP 副本的 cl_scint.ini／objdef 核對：建築與動物用真實 alias，地景用完整 NameDef。
// Hun 無農場，改用屠宰場；足跡大小是規劃估值，尚非遊戲碰撞資料。
public static class SettlementTribalPresets
{
    public static (BuildingBlueprint MainHouse, IReadOnlyList<BuildingBlueprint> CoreBuildings) GetBlueprints(SettlementTribe tribe)
    {
        return tribe switch
        {
            SettlementTribe.Roman => (
                new BuildingBlueprint("ROM_HAU00", "MainHouse", 4.5f, 4.5f, 0f),
                new BuildingBlueprint[]
                {
                    new("ROM_LAG00", "Warehouse", 3.2f, 3.2f, 5.0f, 45f),
                    new("ROM_WOH00", "House", 2.6f, 2.6f, 5.5f, 105f),
                    new("ROM_BAU00", "Farm", 3.8f, 3.8f, 7.0f, 165f),
                    new("ROM_WAF00", "Blacksmith", 3.0f, 3.0f, 6.0f, 225f),
                    new("ROM_STA00", "Stable", 2.2f, 2.2f, 8.5f, 285f),
                    new("ROM_SCHRE00", "Workshop", 3.0f, 3.0f, 8.5f, 345f)
                }),
            SettlementTribe.Celtic => (
                new BuildingBlueprint("KEL_HAU00", "MainHouse", 4.2f, 4.2f, 0f),
                new BuildingBlueprint[]
                {
                    new("KEL_LAG00", "Warehouse", 3.0f, 3.0f, 5.0f, 40f),
                    new("KEL_WOH00", "House", 2.5f, 2.5f, 5.2f, 105f),
                    new("KEL_BAU00", "Farm", 3.5f, 3.5f, 6.8f, 165f),
                    new("KEL_WAF00", "Blacksmith", 2.8f, 2.8f, 6.0f, 225f),
                    new("KEL_STA00", "Stable", 2.0f, 2.0f, 8.2f, 285f),
                    new("KEL_SCHRE00", "Workshop", 3.0f, 3.0f, 8.5f, 345f)
                }),
            SettlementTribe.Hun => (
                new BuildingBlueprint("HUN_HAU00", "MainHouse", 4.2f, 4.2f, 0f),
                new BuildingBlueprint[]
                {
                    new("HUN_LAG00", "Warehouse", 3.0f, 3.0f, 5.0f, 45f),
                    new("HUN_WOH00", "House", 2.5f, 2.5f, 5.0f, 105f),
                    new("HUN_SCHLA00", "Butcher", 3.6f, 3.6f, 7.0f, 165f),
                    new("HUN_WAF00", "Blacksmith", 2.8f, 2.8f, 6.2f, 225f),
                    new("HUN_STA00", "Stable", 2.0f, 2.0f, 8.0f, 285f),
                    new("HUN_SCHRE00", "Workshop", 3.0f, 3.0f, 8.5f, 345f)
                }),
            _ => ( // Germanic (Default)
                new BuildingBlueprint("GER_HAU00", "MainHouse", 4.2f, 4.2f, 0f),
                new BuildingBlueprint[]
                {
                    new("GER_LAG00", "Warehouse", 3.0f, 3.0f, 5.2f, 45f),
                    new("GER_WOH00", "House", 2.6f, 2.6f, 5.5f, 105f),
                    new("GER_BAU00", "Farm", 3.6f, 3.6f, 7.2f, 165f),
                    new("GER_WAF00", "Blacksmith", 3.0f, 3.0f, 6.0f, 225f),
                    new("GER_STA00", "Stable", 2.2f, 2.2f, 8.5f, 285f),
                    new("GER_SCHRE00", "Workshop", 3.0f, 3.0f, 8.5f, 345f)
                })
        };
    }

    public static IReadOnlyList<string> GetForestTreePalette(SettlementTribe tribe)
    {
        return tribe switch
        {
            SettlementTribe.Roman => new[] { "LanItaPin00_Pinie", "LanItaPin01_Pinie", "LanItaZyp00_Zypresse", "LanItaZyp01_Zypresse", "LanItaBus08_Kleiner_Busch" },
            _ => new[] { "LanGerNad00_Tanne_gross", "LanGerNad05_Tanne_gross", "LanGerNad18_Tanne_klein", "LanGerNad24_Tanne_mittel", "LanGerNabu00_Nadelbusch" }
        };
    }

    public static IReadOnlyList<string> GetStoneQuarryPalette(SettlementTribe tribe)
    {
        return tribe switch
        {
            SettlementTribe.Roman => new[] { "LanItaSte00_1Stein", "LanItaSte01_1Stein", "LanItaSte02_1Stein" },
            _ => new[] { "LanGerSte00_1Stein", "LanGerSte01_1Stein", "LanGerSte02_1Stein", "LanGerSte05_1Stein" }
        };
    }

    public static IReadOnlyList<string> GetWildlifePalette()
    {
        return new[] { "ALL_EBE00" }; // FigTieEbe00_Wildschwein；不以掠食者充當食物群。
    }
}
