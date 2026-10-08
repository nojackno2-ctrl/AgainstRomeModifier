using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Settlement;

/// <summary>
/// 聚落與資源智慧生成引擎（SettlementGeneratorEngine）：
/// 一鍵式串接腹地評估、資源聚落規劃與多勢力公平分佈，並輸出為與現有 PlacementEditSession 及 LayoutJson 完全整合的預設配置。
/// </summary>
public static class SettlementGeneratorEngine
{
    /// <summary>
    /// 將多勢力生成結果中的建築與放置單位轉為標準 Placement 類型的 MapLayoutPreset。
    /// </summary>
    public static MapLayoutPreset ToPlacementLayoutPreset(MultiplayerDistributionResult distribution)
    {
        ArgumentNullException.ThrowIfNull(distribution);

        var entries = new List<MapLayoutEntry>();

        foreach (var player in distribution.Players)
        {
            // 建築物
            foreach (var b in player.Buildings)
            {
                entries.Add(new MapLayoutEntry(
                    Type: b.TypeName,
                    X: b.WorldX,
                    Z: b.WorldZ,
                    HeightOffset: b.HeightOffset,
                    Angle: b.Angle,
                    Team: b.Team,
                    Count: 0));
            }

            // 野生動物或放置資源
            foreach (var w in player.WildlifeAndFood)
            {
                entries.Add(new MapLayoutEntry(
                    Type: w.TypeName,
                    X: w.WorldX,
                    Z: w.WorldZ,
                    HeightOffset: w.HeightOffset,
                    Angle: w.AngleDeg,
                    Team: w.Team,
                    Count: w.Count));
            }
        }

        return new MapLayoutPreset(
            Version: 1,
            Kind: MapLayoutKind.Placement,
            Entries: entries);
    }

    /// <summary>
    /// 將多勢力生成結果中的樹林與石礦自然地景物件轉為標準 Nature 類型的 MapLayoutPreset。
    /// </summary>
    public static MapLayoutPreset ToNatureLayoutPreset(MultiplayerDistributionResult distribution)
    {
        ArgumentNullException.ThrowIfNull(distribution);

        var entries = new List<MapLayoutEntry>();

        foreach (var player in distribution.Players)
        {
            // 森林樹木
            foreach (var tree in player.ForestTrees)
            {
                entries.Add(new MapLayoutEntry(
                    Type: tree.TypeName,
                    X: tree.WorldX,
                    Z: tree.WorldZ,
                    HeightOffset: tree.HeightOffset,
                    Angle: tree.AngleDeg,
                    Team: 0,
                    Count: 0));
            }

            // 採石場與石礦露頭
            foreach (var rock in player.StoneQuarries)
            {
                entries.Add(new MapLayoutEntry(
                    Type: rock.TypeName,
                    X: rock.WorldX,
                    Z: rock.WorldZ,
                    HeightOffset: rock.HeightOffset,
                    Angle: rock.AngleDeg,
                    Team: 0,
                    Count: 0));
            }
        }

        return new MapLayoutPreset(
            Version: 1,
            Kind: MapLayoutKind.Nature,
            Entries: entries);
    }

    /// <summary>
    /// 依據 Catalog 將生成的 Placement 佈局轉換為實體 SdlPlacedObject 清單，並直接注入 PlacementEditSession。
    /// </summary>
    public static IReadOnlyList<int> ApplyToPlacementSession(
        MultiplayerDistributionResult distribution,
        IReadOnlyList<SdlObjectType> catalog,
        PlacementEditSession session,
        Func<float, float, float> groundHeight)
    {
        ArgumentNullException.ThrowIfNull(distribution);
        ArgumentNullException.ThrowIfNull(catalog);
        ArgumentNullException.ThrowIfNull(session);
        ArgumentNullException.ThrowIfNull(groundHeight);

        var preset = ToPlacementLayoutPreset(distribution);
        // 規劃器用短型別名；解析成實際目錄別名，目錄沒有的型別略過而不是讓整批失敗。
        preset = preset with { Entries = ResolveEntries(preset.Entries, name => LayoutTypeResolver.ResolveAlias(catalog, name)) };

        // 使用 (0, 0, 0) 錨點，因為 entries 已具備世界絕對座標
        var plannedPlacements = MapLayoutPresets.PlanPlacements(
            preset,
            catalog,
            anchorX: 0,
            anchorZ: 0,
            rotation: 0,
            groundHeight: groundHeight);

        return session.AddMany(plannedPlacements);
    }

    /// <summary>
    /// 將生成的 Nature 佈局轉換為 NatureAddition 清單，並注入 NatureEditSession。
    /// </summary>
    public static bool ApplyToNatureSession(
        MultiplayerDistributionResult distribution,
        IReadOnlyDictionary<string, LevelObjectTemplate> templates,
        NatureEditSession session,
        Func<float, float, float> groundHeight)
    {
        ArgumentNullException.ThrowIfNull(distribution);
        ArgumentNullException.ThrowIfNull(templates);
        ArgumentNullException.ThrowIfNull(session);
        ArgumentNullException.ThrowIfNull(groundHeight);

        var preset = ToNatureLayoutPreset(distribution);
        preset = preset with { Entries = ResolveEntries(preset.Entries, name => LayoutTypeResolver.ResolveKey(templates, name)) };
        if (preset.Entries.Count == 0) return false;

        var additions = MapLayoutPresets.PlanNature(
            preset,
            templates,
            anchorX: 0,
            anchorZ: 0,
            rotation: 0,
            groundHeight: groundHeight);

        return session.PlantMany(additions);
    }

    private static IReadOnlyList<MapLayoutEntry> ResolveEntries(IReadOnlyList<MapLayoutEntry> entries, Func<string, string?> resolve)
    {
        var resolved = new List<MapLayoutEntry>(entries.Count);
        foreach (MapLayoutEntry entry in entries)
            if (resolve(entry.Type) is { } type) resolved.Add(entry with { Type = type });
        return resolved;
    }

    /// <summary>
    /// 匯出為可攜式 JSON 字串（與 .arm-layout.json 相容）。
    /// </summary>
    public static (string PlacementJson, string NatureJson) ExportLayoutJsons(MultiplayerDistributionResult distribution)
    {
        var placementPreset = ToPlacementLayoutPreset(distribution);
        var naturePreset = ToNatureLayoutPreset(distribution);

        return (
            MapLayoutPresets.Serialize(placementPreset),
            MapLayoutPresets.Serialize(naturePreset));
    }
}
