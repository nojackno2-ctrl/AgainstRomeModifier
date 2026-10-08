
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
public static class SettlementTribalPresets
{
    public static (BuildingBlueprint MainHouse, IReadOnlyList<BuildingBlueprint> CoreBuildings) GetBlueprints(SettlementTribe tribe)
    {
        return tribe switch
        {
            SettlementTribe.Roman => (
                new BuildingBlueprint("BauRomHau00", "MainHouse", 4.5f, 4.5f, 0f),
                new BuildingBlueprint[]
                {
                    new("BauRomLag00", "Warehouse", 3.2f, 3.2f, 5.0f, 45f),
                    new("BauRomWoh00", "House", 2.6f, 2.6f, 5.5f, 135f),
                    new("BauRomKas00", "Barracks", 3.8f, 3.8f, 7.0f, 225f),
                    new("BauRomSch00", "Blacksmith", 3.0f, 3.0f, 6.0f, 315f),
                    new("BauRomTur00", "Tower", 2.2f, 2.2f, 8.5f, 90f)
                }),
            SettlementTribe.Celtic => (
                new BuildingBlueprint("BauKelHau00", "MainHouse", 4.2f, 4.2f, 0f),
                new BuildingBlueprint[]
                {
                    new("BauKelLag00", "Warehouse", 3.0f, 3.0f, 5.0f, 40f),
                    new("BauKelWoh00", "House", 2.5f, 2.5f, 5.2f, 130f),
                    new("BauKelKas00", "Barracks", 3.5f, 3.5f, 6.8f, 220f),
                    new("BauKelSch00", "Blacksmith", 2.8f, 2.8f, 6.0f, 310f),
                    new("BauKelTur00", "Tower", 2.0f, 2.0f, 8.2f, 85f)
                }),
            SettlementTribe.Hun => (
                new BuildingBlueprint("BauHunHau00", "MainHouse", 4.2f, 4.2f, 0f),
                new BuildingBlueprint[]
                {
                    new("BauHunLag00", "Warehouse", 3.0f, 3.0f, 5.0f, 45f),
                    new("BauHunWoh00", "House", 2.5f, 2.5f, 5.0f, 140f),
                    new("BauHunKas00", "Barracks", 3.6f, 3.6f, 7.0f, 230f),
                    new("BauHunSch00", "Blacksmith", 2.8f, 2.8f, 6.2f, 315f),
                    new("BauHunTur00", "Tower", 2.0f, 2.0f, 8.0f, 90f)
                }),
            _ => ( // Germanic (Default)
                new BuildingBlueprint("BauGerHau00", "MainHouse", 4.2f, 4.2f, 0f),
                new BuildingBlueprint[]
                {
                    new("BauGerLag00", "Warehouse", 3.0f, 3.0f, 5.2f, 45f),
                    new("BauGerWoh00", "House", 2.6f, 2.6f, 5.5f, 135f),
                    new("BauGerKas00", "Barracks", 3.6f, 3.6f, 7.2f, 225f),
                    new("BauGerSch00", "Blacksmith", 3.0f, 3.0f, 6.0f, 315f),
                    new("BauGerTur00", "Tower", 2.2f, 2.2f, 8.5f, 90f)
                })
        };
    }

    public static IReadOnlyList<string> GetForestTreePalette(SettlementTribe tribe)
    {
        return tribe switch
        {
            SettlementTribe.Roman => new[] { "LanRomNad00", "LanRomNad01", "LanRomLau00", "LanRomLau01", "LanRomNabu00" },
            _ => new[] { "LanGerNad00", "LanGerNad05", "LanGerNad18", "LanGerLau00", "LanGerNabu00" }
        };
    }

    public static IReadOnlyList<string> GetStoneQuarryPalette(SettlementTribe tribe)
    {
        return tribe switch
        {
            SettlementTribe.Roman => new[] { "LanRomSte00", "LanRomSte01", "LanRomSte02" },
            _ => new[] { "LanGerSte00", "LanGerSte01", "LanGerSte02", "LanGerSte05" }
        };
    }

    public static IReadOnlyList<string> GetWildlifePalette()
    {
        return new[] { "FigHir00", "FigSch00" }; // 鹿 (Hirsch), 野豬 (Schwein)
    }
}
