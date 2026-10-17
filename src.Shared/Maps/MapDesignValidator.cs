using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Maps;

/// <summary>設計檢查問題的嚴重程度。</summary>
public enum DesignIssueSeverity
{
    Warning,
    Error
}

/// <summary>
/// 地圖設計檢查問題項目（包含物件別名、隊伍與座標資訊）。
/// </summary>
public sealed record MapDesignIssue(
    DesignIssueSeverity Severity,
    string Code,
    string Chinese,
    string English,
    string ObjectAlias = "",
    int Team = -1,
    float? WorldX = null,
    float? WorldZ = null,
    Guid ObjectId = default);

/// <summary>
/// 設計檢查請求模型。
/// </summary>
public sealed record MapDesignValidationRequest(
    IReadOnlyList<ScenarioSpawn> Objects,
    IReadOnlyList<ScenarioEvent> Events,
    IReadOnlyList<byte>? Collision = null,
    int CollisionSize = 0,
    IReadOnlyList<byte>? Heights = null,
    int HeightSize = 0,
    float HeightStep = 4.0f,
    float WaterLevel = 0.0f,
    float MapDimension = 16384f);

/// <summary>
/// 地圖設計層級的可玩性驗證邏輯（純邏輯，無 UI 相依）。
/// 檢查項目包含：
/// 1. 物件是否超出地圖邊界 (0..16383)。
/// 2. 放置物件是否位於水域或通行阻擋格。
/// 3. 各參戰勢力/玩家隊伍是否缺少主堡（Haupthaus）或缺少部隊/單位。
/// 4. 劇本是否缺少勝利或失敗事件。
/// </summary>
public static class MapDesignValidator
{
    public const float DefaultMapDimension = 16384f;

    /// <summary>
    /// 依雙線性內插計算地形原始高度（0..255），與 TerrainHeightEditSession.SampleHeight / TerrainHeightField.SampleRaw 邏輯一致。
    /// </summary>
    public static float SampleBilinearRawHeight(IReadOnlyList<byte> heights, int vertexSize, float normX, float normZ)
    {
        if (heights is null || vertexSize < 2 || heights.Count != vertexSize * vertexSize) return 0f;
        float gx = Math.Clamp(normX * (vertexSize - 1), 0f, vertexSize - 1);
        float gz = Math.Clamp(normZ * (vertexSize - 1), 0f, vertexSize - 1);
        int left = Math.Min((int)gx, vertexSize - 2);
        int top = Math.Min((int)gz, vertexSize - 2);
        float tx = gx - left, tz = gz - top;
        float a = heights[top * vertexSize + left];
        float b = heights[top * vertexSize + left + 1];
        float c = heights[(top + 1) * vertexSize + left];
        float d = heights[(top + 1) * vertexSize + left + 1];
        return (a + (b - a) * tx) * (1f - tz) + (c + (d - c) * tx) * tz;
    }

    /// <summary>判斷別名或名稱是否為部族主堡（Haupthaus / Hauptzelt）。</summary>
    public static bool IsMainHouse(string? aliasOrName)
    {
        if (string.IsNullOrWhiteSpace(aliasOrName)) return false;
        string s = aliasOrName.Trim();

        // 1. 標準別名：GER_HAU00, ROM_HAU00, KEL_HAU00, HUN_HAU00
        if (s.EndsWith("_HAU00", StringComparison.OrdinalIgnoreCase)
            || s.EndsWith("_HAU", StringComparison.OrdinalIgnoreCase)
            || s.Contains("HAU00", StringComparison.OrdinalIgnoreCase))
            return true;

        // 2. 原版德文 namedef：BauGerHau00_Haupthaus, BauRomHau00_Hauptzelt, BauKelHau00, BauHunHau00
        if (s.Contains("Haupthaus", StringComparison.OrdinalIgnoreCase)
            || s.Contains("Hauptzelt", StringComparison.OrdinalIgnoreCase))
            return true;

        if (s.StartsWith("Bau", StringComparison.OrdinalIgnoreCase) && s.Contains("Hau", StringComparison.OrdinalIgnoreCase))
            return true;

        return false;
    }

    /// <summary>判斷別名或名稱是否為戰鬥部隊或人物單位。</summary>
    public static bool IsUnitAlias(string? aliasOrName)
    {
        if (string.IsNullOrWhiteSpace(aliasOrName)) return false;
        string s = aliasOrName.Trim();

        // 標準兵種別名：步兵 INF、弓兵 ARC、騎兵 CAV/KAV/REI、英雄 HER、矛兵 SPI 等
        if (s.Contains("_INF", StringComparison.OrdinalIgnoreCase)
            || s.Contains("_ARC", StringComparison.OrdinalIgnoreCase)
            || s.Contains("_CAV", StringComparison.OrdinalIgnoreCase)
            || s.Contains("_KAV", StringComparison.OrdinalIgnoreCase)
            || s.Contains("_REI", StringComparison.OrdinalIgnoreCase)
            || s.Contains("_HER", StringComparison.OrdinalIgnoreCase)
            || s.Contains("_SPI", StringComparison.OrdinalIgnoreCase))
            return true;

        // Namedef 前綴判定：Ver...Ico 為部隊圖示，Fig 為人物（排除 FigTie 動物）
        if (s.StartsWith("Ver", StringComparison.OrdinalIgnoreCase) && s.Contains("Ico", StringComparison.OrdinalIgnoreCase))
            return true;

        if (s.StartsWith("Fig", StringComparison.OrdinalIgnoreCase) && !s.StartsWith("FigTie", StringComparison.OrdinalIgnoreCase))
            return true;

        return false;
    }

    /// <summary>
    /// 執行地圖設計檢查並回傳所有發現的警告與問題。
    /// </summary>
    public static IReadOnlyList<MapDesignIssue> Validate(MapDesignValidationRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        var issues = new List<MapDesignIssue>();
        float dim = request.MapDimension > 0 ? request.MapDimension : DefaultMapDimension;

        // --- 1. 物件座標與邊界檢查 ---
        foreach (ScenarioSpawn spawn in request.Objects)
        {
            if (spawn is null) continue;
            bool outOfBounds = !float.IsFinite(spawn.X) || !float.IsFinite(spawn.Z)
                || spawn.X < 0f || spawn.X > dim || spawn.Z < 0f || spawn.Z > dim;

            if (outOfBounds)
            {
                issues.Add(new MapDesignIssue(
                    DesignIssueSeverity.Warning,
                    "bounds",
                    $"物件「{spawn.Alias}」（隊伍 {spawn.Team}，座標 X={spawn.X:F0}, Z={spawn.Z:F0}）超出地圖邊界 (0..{dim:F0})。",
                    $"Object '{spawn.Alias}' (Team {spawn.Team}, Position X={spawn.X:F0}, Z={spawn.Z:F0}) is outside map bounds (0..{dim:F0}).",
                    ObjectAlias: spawn.Alias,
                    Team: spawn.Team,
                    WorldX: float.IsFinite(spawn.X) ? spawn.X : null,
                    WorldZ: float.IsFinite(spawn.Z) ? spawn.Z : null,
                    ObjectId: spawn.Id));
            }
        }

        // --- 2. 地形層檢查：水下與阻擋格判定 ---
        int colSize = request.CollisionSize;
        if (colSize <= 0 && request.Collision is not null && request.Collision.Count > 0)
        {
            int sqrt = (int)MathF.Round(MathF.Sqrt(request.Collision.Count));
            if (sqrt * sqrt == request.Collision.Count) colSize = sqrt;
        }

        int heightSize = request.HeightSize;
        if (heightSize <= 0 && request.Heights is not null && request.Heights.Count > 0)
        {
            int sqrt = (int)MathF.Round(MathF.Sqrt(request.Heights.Count));
            if (sqrt * sqrt == request.Heights.Count) heightSize = sqrt;
        }

        bool hasCollision = request.Collision is not null && colSize > 0
            && request.Collision.Count == colSize * colSize;
        bool hasHeights = request.Heights is not null && heightSize > 1
            && request.Heights.Count == heightSize * heightSize
            && float.IsFinite(request.HeightStep) && request.HeightStep > 0
            && float.IsFinite(request.WaterLevel);

        // 若高度圖與碰撞圖同時存在，兩者尺寸必須相容 (collision = boden - 1 = dimension * 4 對應 dimension * 4 + 1)
        // 若尺寸不一致，優雅略過兩者檢查，避免座標錯位或誤報
        if (hasCollision && hasHeights && colSize != heightSize - 1)
        {
            hasCollision = false;
            hasHeights = false;
        }

        foreach (ScenarioSpawn spawn in request.Objects)
        {
            if (spawn is null) continue;
            if (!float.IsFinite(spawn.X) || !float.IsFinite(spawn.Z) || spawn.X < 0f || spawn.X > dim || spawn.Z < 0f || spawn.Z > dim)
                continue; // 越界物件由第一條規則回報

            float normX = Math.Clamp(spawn.X / dim, 0f, 1f);
            float normZ = Math.Clamp(spawn.Z / dim, 0f, 1f);

            // 水下判定（採用與 TerrainHeightEditSession.SampleHeight / PaintWater 一致的雙線性內插）
            if (hasHeights)
            {
                float rawHeight = SampleBilinearRawHeight(request.Heights!, heightSize, normX, normZ);
                float groundHeight = rawHeight * request.HeightStep;
                if (groundHeight < request.WaterLevel)
                {
                    issues.Add(new MapDesignIssue(
                        DesignIssueSeverity.Warning,
                        "water",
                        $"物件「{spawn.Alias}」（隊伍 {spawn.Team}，座標 X={spawn.X:F0}, Z={spawn.Z:F0}）位於水下（地面高度 {groundHeight:F1} 低於水面高度 {request.WaterLevel:F1}）。",
                        $"Object '{spawn.Alias}' (Team {spawn.Team}, Position X={spawn.X:F0}, Z={spawn.Z:F0}) is submerged under water (ground height {groundHeight:F1} < water level {request.WaterLevel:F1}).",
                        ObjectAlias: spawn.Alias,
                        Team: spawn.Team,
                        WorldX: spawn.X,
                        WorldZ: spawn.Z,
                        ObjectId: spawn.Id));
                }
            }

            // 通行阻擋判定（對應至實際碰撞網格尺寸）
            if (hasCollision)
            {
                int cx = Math.Clamp((int)(normX * colSize), 0, colSize - 1);
                int cz = Math.Clamp((int)(normZ * colSize), 0, colSize - 1);
                byte colVal = request.Collision![cz * colSize + cx];
                if (colVal != 0)
                {
                    issues.Add(new MapDesignIssue(
                        DesignIssueSeverity.Warning,
                        "impassable",
                        $"物件「{spawn.Alias}」（隊伍 {spawn.Team}，座標 X={spawn.X:F0}, Z={spawn.Z:F0}）位於不可通行的阻擋格上（碰撞值 {colVal}）。",
                        $"Object '{spawn.Alias}' (Team {spawn.Team}, Position X={spawn.X:F0}, Z={spawn.Z:F0}) is placed on an impassable cell (collision value {colVal}).",
                        ObjectAlias: spawn.Alias,
                        Team: spawn.Team,
                        WorldX: spawn.X,
                        WorldZ: spawn.Z,
                        ObjectId: spawn.Id));
                }
            }
        }

        // --- 3. 參戰勢力/玩家隊伍完整性檢查（主堡與部隊） ---
        // 收集所有參戰勢力：隊伍 0 必定檢查；隊伍 1..7 若有放置物件或事件生成部隊，視為有效參戰隊伍
        var candidateTeams = new HashSet<int> { 0 };
        foreach (ScenarioSpawn s in request.Objects)
        {
            if (s.Team is >= 0 and <= 7) candidateTeams.Add(s.Team);
        }
        foreach (ScenarioEvent e in request.Events.Where(ev => ev.Enabled))
        {
            foreach (ScenarioAction a in e.Actions.Where(act => act.Kind == ScenarioActionKind.SpawnUnit))
            {
                if (a.Team is >= 0 and <= 7) candidateTeams.Add(a.Team);
            }
        }

        foreach (int team in candidateTeams.OrderBy(t => t))
        {
            var teamObjects = request.Objects.Where(s => s.Team == team).ToList();
            float? locX = teamObjects.FirstOrDefault()?.X;
            float? locZ = teamObjects.FirstOrDefault()?.Z;
            string locZh = locX.HasValue && locZ.HasValue ? $"（位於 X={locX.Value:F0}, Z={locZ.Value:F0} 附近）" : "";
            string locEn = locX.HasValue && locZ.HasValue ? $" (near X={locX.Value:F0}, Z={locZ.Value:F0})" : "";

            // 檢查主堡
            bool hasMainHouse = teamObjects.Any(s => IsMainHouse(s.Alias));
            if (!hasMainHouse)
            {
                issues.Add(new MapDesignIssue(
                    DesignIssueSeverity.Warning,
                    "team-no-main-house",
                    $"隊伍 {team}{locZh} 缺少主堡（Haupthaus），無法作為完整勢力基地運作。",
                    $"Team {team}{locEn} has no main house (Haupthaus) and cannot function as a complete base.",
                    Team: team,
                    WorldX: locX,
                    WorldZ: locZ));
            }

            // 檢查起始部隊/單位
            bool hasPlacedUnits = teamObjects.Any(s => s.Count > 0 || IsUnitAlias(s.Alias));
            bool hasEventUnits = request.Events.Where(e => e.Enabled)
                .Any(e => e.Actions.Any(a => a.Kind == ScenarioActionKind.SpawnUnit && a.Team == team && a.Count > 0));

            if (!hasPlacedUnits && !hasEventUnits)
            {
                issues.Add(new MapDesignIssue(
                    DesignIssueSeverity.Warning,
                    "team-no-units",
                    $"隊伍 {team}{locZh} 沒有任何起始部隊或生成單位。",
                    $"Team {team}{locEn} has no starting troops or spawned units.",
                    Team: team,
                    WorldX: locX,
                    WorldZ: locZ));
            }
        }

        // --- 4. 劇本勝負判定事件檢查 ---
        var enabledEvents = request.Events.Where(e => e.Enabled).ToList();
        bool hasVictory = enabledEvents.Any(e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Victory));
        bool hasDefeat = enabledEvents.Any(e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Defeat));

        if (!hasVictory && !hasDefeat)
        {
            issues.Add(new MapDesignIssue(
                DesignIssueSeverity.Warning,
                "scenario-no-win-loss",
                "劇本未設定任何勝利或失敗事件（Scenario 缺少 Victory 與 Defeat 動作），地圖在遊戲中將無法判定勝負或結束任務。",
                "Scenario has neither victory nor defeat events (missing Victory and Defeat actions); map cannot evaluate win/loss or complete the mission."));
        }
        else if (!hasVictory)
        {
            issues.Add(new MapDesignIssue(
                DesignIssueSeverity.Warning,
                "scenario-no-victory",
                "劇本未設定任何勝利事件（Scenario 缺少 Victory 動作），玩家將無法達成任務目標獲得勝利。",
                "Scenario has no victory event (missing Victory action); player cannot achieve mission victory."));
        }
        else if (!hasDefeat)
        {
            issues.Add(new MapDesignIssue(
                DesignIssueSeverity.Warning,
                "scenario-no-defeat",
                "劇本未設定任何失敗事件（Scenario 缺少 Defeat 動作），玩家全滅時可能無法正常判定失敗。",
                "Scenario has no defeat event (missing Defeat action); mission loss cannot be evaluated when player is defeated."));
        }

        return issues;
    }
}
