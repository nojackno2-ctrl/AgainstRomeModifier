using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Diagnostics;

public enum MapIssueSeverity { Error, Warning }
public sealed record MapIssue(MapIssueSeverity Severity, string Code, string Chinese, string English,
    Guid ObjectId = default, int EventIndex = -1, float? WorldX = null, float? WorldZ = null);
public sealed record MapCheckObject(ScenarioSpawn Spawn, bool IsBuilding, bool HasCompletedTemplate);
public sealed record MapCheckSnapshot(IReadOnlyList<MapCheckObject> Objects, IReadOnlyList<ScenarioEvent> Events,
    IReadOnlyCollection<string> Aliases, int CollisionSize = 0, IReadOnlyList<byte>? Collision = null,
    int HeightSize = 0, IReadOnlyList<byte>? Heights = null, float HeightStep = 4, float WaterLevel = 0);

/// <summary>Read-only editor diagnostics. Reachability estimates collision/water connectivity, not game pathfinding.</summary>
public static class MapDiagnostics
{
    public static IReadOnlyList<MapIssue> Check(MapCheckSnapshot map)
    {
        ArgumentNullException.ThrowIfNull(map);
        var issues = new List<MapIssue>();
        void Add(string code, string zh, string en, MapCheckObject? obj = null, int eventIndex = -1,
            MapIssueSeverity severity = MapIssueSeverity.Warning, float? x = null, float? z = null) =>
            issues.Add(new(severity, code, zh, en, obj?.Spawn.Id ?? Guid.Empty, eventIndex, x ?? obj?.Spawn.X, z ?? obj?.Spawn.Z));
        var byId = new Dictionary<Guid, MapCheckObject>();
        foreach (MapCheckObject obj in map.Objects)
        {
            ScenarioSpawn spawn = obj.Spawn;
            if (spawn.Id == Guid.Empty || !byId.TryAdd(spawn.Id, obj))
                Add("identity", "放置物件的持久 ID 缺失或重複。", "A placed object has a missing or duplicate persistent ID.", obj, severity: MapIssueSeverity.Error);
            if (!ValidPoint(spawn.X, spawn.Z) || !float.IsFinite(spawn.Y))
                Add("coordinates", "物件座標超出地圖範圍或不是有限數值。", "Object coordinates are outside the map or non-finite.", obj, severity: MapIssueSeverity.Error);
            if (!spawn.Prebuilt && !map.Aliases.Contains(spawn.Alias, StringComparer.OrdinalIgnoreCase))
                Add("alias", $"未知物件別名：{spawn.Alias}", $"Unknown object alias: {spawn.Alias}", obj, severity: MapIssueSeverity.Error);
            if (spawn.Count is < 0 or > 20 || (spawn.Count > 0 ? spawn.Team is < 0 or > 7 : spawn.Team is < -1 or > 15))
                Add("team-count", "物件隊伍或人數無效。", "Object team or unit count is invalid.", obj, severity: MapIssueSeverity.Error);
            if (obj.IsBuilding && !obj.HasCompletedTemplate)
                Add("construction-site", $"{spawn.Alias} 缺完工範本，將儲存為腳本工地。", $"{spawn.Alias} has no completed template and will save as a construction site.", obj);
        }
        // Check the whole event list for aggregate limits/terminal-action conflicts, then locate individual problems.
        try { ScenarioEventValidator.Validate(map.Events, map.Aliases); }
        catch (InvalidDataException ex) { Add("event-validation", ex.Message, "Event configuration is invalid; review the event list.", severity: MapIssueSeverity.Error); }
        for (int index = 0; index < map.Events.Count; index++)
        {
            ScenarioEvent item = map.Events[index];
            if (!item.Enabled) continue;
            foreach (ScenarioCondition condition in item.Conditions)
                if (condition.TargetId != Guid.Empty && !byId.ContainsKey(condition.TargetId))
                    Add("event-target", $"事件「{item.Name}」的目標已刪除。", $"Event '{item.Name}' references a deleted object.", eventIndex: index, severity: MapIssueSeverity.Error);
        }
        MapCheckObject[] valid = map.Objects.Where(obj => ValidPoint(obj.Spawn.X, obj.Spawn.Z)).ToArray();
        foreach (var group in valid.GroupBy(obj => ((int)(obj.Spawn.X / 64), (int)(obj.Spawn.Z / 64))))
            if (group.Count() > 1)
                foreach (MapCheckObject obj in group)
                    Add("overlap", "多個放置物件錨點位於同一個 64 單位區域，請檢查重疊。", "Multiple placement anchors share a 64-unit area; inspect for overlap.", obj);
        MapCheckObject[] starts = valid.Where(obj => obj.Spawn.Team == 0 && obj.Spawn.Count > 0).ToArray();
        if (starts.Length == 0) Add("no-start", "未找到編輯器放置的玩家起始部隊；範本腳本可能另有生成。", "No editor-placed player starting troop was found; template scripts may spawn one.");
        if (map.Collision is null || map.CollisionSize <= 0 || map.Collision.Count != (long)map.CollisionSize * map.CollisionSize)
        {
            Add("no-collision", "缺少有效通行層，無法檢查阻擋與連通性。", "No valid collision layer is available for blockage/connectivity checks.");
            return issues;
        }
        int size = map.CollisionSize;
        // Imported editor maps use a 256x256 collision layer; keep hostile inputs bounded.
        if (size > 1024) throw new ArgumentOutOfRangeException(nameof(map), "Collision grid exceeds diagnostic limits.");
        bool hasHeight = map.Heights is not null && map.HeightSize > 1 && map.Heights.Count == (long)map.HeightSize * map.HeightSize
            && float.IsFinite(map.HeightStep) && map.HeightStep > 0 && float.IsFinite(map.WaterLevel);
        if (!hasHeight) Add("no-height", "缺少有效高度資料，連通估計只考慮通行層。", "No valid height data is available; connectivity considers collision only.");
        var components = new int[size * size];
        Array.Fill(components, -1);
        var queue = new int[components.Length];
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            bool submerged = false;
            if (hasHeight)
            {
                int hx = Math.Clamp((int)((x + .5f) * (map.HeightSize - 1) / size), 0, map.HeightSize - 1);
                int hy = Math.Clamp((int)((y + .5f) * (map.HeightSize - 1) / size), 0, map.HeightSize - 1);
                submerged = map.Heights![hy * map.HeightSize + hx] * map.HeightStep < map.WaterLevel;
            }
            if (map.Collision[y * size + x] != 0 || submerged) components[y * size + x] = -2;
        }
        int component = 0;
        for (int origin = 0; origin < components.Length; origin++)
        {
            if (components[origin] != -1) continue;
            int head = 0, tail = 0; queue[tail++] = origin; components[origin] = component;
            while (head < tail)
            {
                int cell = queue[head++], x = cell % size, y = cell / size;
                void Visit(int next)
                {
                    if (components[next] != -1) return;
                    components[next] = component; queue[tail++] = next;
                }
                if (x > 0) Visit(cell - 1); if (x + 1 < size) Visit(cell + 1);
                if (y > 0) Visit(cell - size); if (y + 1 < size) Visit(cell + size);
            }
            component++;
        }
        int Cell(float x, float z) => Math.Clamp((int)(z / 16384 * size), 0, size - 1) * size + Math.Clamp((int)(x / 16384 * size), 0, size - 1);
        var reachable = new HashSet<int>();
        foreach (MapCheckObject start in starts)
        {
            int region = components[Cell(start.Spawn.X, start.Spawn.Z)];
            if (region >= 0) reachable.Add(region);
            else Add("blocked-start", "玩家起始部隊錨點位於阻擋區或水面下。", "A player starting troop anchor lies in blocked or submerged ground.", start);
        }
        foreach (MapCheckObject obj in valid.Where(obj => obj.IsBuilding))
        {
            int region = components[Cell(obj.Spawn.X, obj.Spawn.Z)];
            if (region < 0) Add("blocked-building", "建築錨點位於阻擋區或水面下，請檢查放置位置。", "A building anchor lies in blocked or submerged ground; inspect placement.", obj);
            else if (reachable.Count > 0 && !reachable.Contains(region))
                Add("isolated-building", "建築所在通行區與玩家起始部隊不連通（估計）。", "A building's passable region is disconnected from player starting troops (estimate).", obj);
        }
        for (int index = 0; index < map.Events.Count; index++)
        {
            ScenarioEvent item = map.Events[index];
            if (!item.Enabled) continue;
            foreach (ScenarioCondition area in item.Conditions.Where(condition => condition.Kind == ScenarioConditionKind.ObjectInArea))
            {
                if (!byId.TryGetValue(area.TargetId, out MapCheckObject? target) || !ValidPoint(target.Spawn.X, target.Spawn.Z)
                    || area.MinX < 0 || area.MinZ < 0 || area.MaxX > 16383 || area.MaxZ > 16383 || area.MinX > area.MaxX || area.MinZ > area.MaxZ) continue;
                int targetRegion = components[Cell(target.Spawn.X, target.Spawn.Z)];
                if (targetRegion < 0) continue;
                int min = Cell(area.MinX, area.MinZ), max = Cell(area.MaxX, area.MaxZ);
                bool accessible = false;
                for (int y = min / size; y <= max / size && !accessible; y++)
                for (int x = min % size; x <= max % size; x++)
                    if (components[y * size + x] == targetRegion) { accessible = true; break; }
                if (!accessible) Add("isolated-area", $"事件「{item.Name}」的區域與目標物件不連通（估計）。", $"Event '{item.Name}' has an area disconnected from its target (estimate).", target, index,
                    x: (area.MinX + area.MaxX) / 2f, z: (area.MinZ + area.MaxZ) / 2f);
            }
        }
        return issues;
    }

    private static bool ValidPoint(float x, float z) => float.IsFinite(x) && float.IsFinite(z) && x is >= 0 and <= 16383 && z is >= 0 and <= 16383;

    /// <summary>
    /// 執行進階 NavMesh 連通性、道路間隙與部隊方陣通道評估，產生結構化分析報告與修復處方。
    /// </summary>
    public static AgainstRomeMapEditor.Modules.Pathfinding.NavMeshAnalysisResult AnalyzeNavMesh(
        MapCheckSnapshot map,
        IReadOnlyList<string>? textures = null,
        int minIsolatedSize = 1)
    {
        ArgumentNullException.ThrowIfNull(map);
        if (map.Collision is null || map.CollisionSize <= 0 || map.Collision.Count != (long)map.CollisionSize * map.CollisionSize)
            return AgainstRomeMapEditor.Modules.Pathfinding.NavMeshAnalysisResult.Empty;

        var grid = new AgainstRomeMapEditor.Modules.Pathfinding.NavMeshPassabilityGrid(
            map.CollisionSize,
            map.Collision,
            map.HeightSize,
            map.Heights,
            map.HeightStep,
            map.WaterLevel);

        var primarySeeds = map.Objects
            .Where(o => o.Spawn.Team == 0 && (o.Spawn.Count > 0 || o.IsBuilding))
            .Where(o => ValidPoint(o.Spawn.X, o.Spawn.Z))
            .Select(o => grid.ToCoordinate(grid.WorldToTile(o.Spawn.X, o.Spawn.Z).X, grid.WorldToTile(o.Spawn.X, o.Spawn.Z).Z))
            .ToList();

        var interestPoints = map.Objects
            .Where(o => ValidPoint(o.Spawn.X, o.Spawn.Z))
            .Select(o => grid.ToCoordinate(grid.WorldToTile(o.Spawn.X, o.Spawn.Z).X, grid.WorldToTile(o.Spawn.X, o.Spawn.Z).Z))
            .ToList();

        var isolated = AgainstRomeMapEditor.Modules.Pathfinding.NavMeshConnectivityAnalyzer.Analyze(
            grid, primarySeeds, interestPoints, minIsolatedSize);

        int texDimension = textures is not null && textures.Count > 0
            ? (int)MathF.Round(MathF.Sqrt(textures.Count))
            : 0;

        var gaps = texDimension > 0 && textures!.Count == texDimension * texDimension
            ? AgainstRomeMapEditor.Modules.Pathfinding.RoadGapDetector.DetectGaps(texDimension, textures, grid)
            : [];

        var repairs = new List<AgainstRomeMapEditor.Modules.Pathfinding.NavMeshRepairAction>();
        repairs.AddRange(AgainstRomeMapEditor.Modules.Pathfinding.RoadPathHealer.CreateRepairActions(gaps));

        foreach (var region in isolated.Where(r => r.RecommendedBridgePoint is not null))
        {
            if (AgainstRomeMapEditor.Modules.Pathfinding.RoadPathHealer.CreateBridgeAction(region, tileDimension: texDimension > 0 ? texDimension : 64) is { } bridge)
                repairs.Add(bridge);
        }

        var trips = new List<(AgainstRomeMapEditor.Modules.Pathfinding.NavMeshCoordinate, AgainstRomeMapEditor.Modules.Pathfinding.NavMeshCoordinate, int)>();
        var squads = map.Objects.Where(o => o.Spawn.Count > 1 && ValidPoint(o.Spawn.X, o.Spawn.Z)).ToArray();
        var enemyOrTargets = map.Objects.Where(o => o.Spawn.Team != 0 && ValidPoint(o.Spawn.X, o.Spawn.Z)).ToArray();

        foreach (var squad in squads)
        {
            var start = grid.ToCoordinate(grid.WorldToTile(squad.Spawn.X, squad.Spawn.Z).X, grid.WorldToTile(squad.Spawn.X, squad.Spawn.Z).Z);
            foreach (var target in enemyOrTargets.Take(3))
            {
                var end = grid.ToCoordinate(grid.WorldToTile(target.Spawn.X, target.Spawn.Z).X, grid.WorldToTile(target.Spawn.X, target.Spawn.Z).Z);
                trips.Add((start, end, squad.Spawn.Count));
            }
        }

        var chokes = AgainstRomeMapEditor.Modules.Pathfinding.FormationWidthValidator.EvaluateCorridors(grid, trips);
        if (chokes.Count == 0 && squads.Length > 0)
        {
            chokes = AgainstRomeMapEditor.Modules.Pathfinding.FormationWidthValidator.DetectTopologicalChokePoints(grid, squads.Max(s => s.Spawn.Count));
        }

        return new AgainstRomeMapEditor.Modules.Pathfinding.NavMeshAnalysisResult(isolated, gaps, chokes, repairs);
    }

    /// <summary>
    /// 將 NavMesh 分析報告轉為標準 MapIssue 項目，供地圖檢查面板顯示並支援座標定位。
    /// </summary>
    public static IReadOnlyList<MapIssue> ConvertToIssues(AgainstRomeMapEditor.Modules.Pathfinding.NavMeshAnalysisResult navResult)
    {
        ArgumentNullException.ThrowIfNull(navResult);
        var issues = new List<MapIssue>();

        foreach (var region in navResult.IsolatedRegions)
        {
            issues.Add(new MapIssue(
                MapIssueSeverity.Warning,
                "isolated-land",
                $"發現孤立無法通達的陸地區域（共 {region.TileCount} 格），部隊無法前往。" +
                (region.ContainsTroopOrBuilding ? " 區域內含有物件或建築！" : ""),
                $"Isolated land region detected ({region.TileCount} tiles); troops cannot reach this area." +
                (region.ContainsTroopOrBuilding ? " Area contains placed objects or buildings!" : ""),
                WorldX: region.Centroid.WorldX,
                WorldZ: region.Centroid.WorldZ));
        }

        foreach (var gap in navResult.RoadGaps)
        {
            var mid = gap.GapTiles.Count > 0 ? gap.GapTiles[0] : gap.Start;
            issues.Add(new MapIssue(
                MapIssueSeverity.Warning,
                "road-gap",
                $"偵測到道路中斷（間距 {gap.GapDistance} 格，{gap.Style} 風格），建議補齊圖塊「{gap.SuggestedTexture}」。",
                $"Road gap detected ({gap.GapDistance} tiles, {gap.Style} style); recommend placing '{gap.SuggestedTexture}'.",
                WorldX: mid.WorldX,
                WorldZ: mid.WorldZ));
        }

        foreach (var choke in navResult.FormationChokePoints)
        {
            issues.Add(new MapIssue(
                MapIssueSeverity.Warning,
                "formation-bottleneck",
                $"關鍵通道淨寬僅 {choke.MeasuredClearanceTiles} 格（約 {choke.MeasuredWidthUnits:F0} 單位），無法容納 {choke.BlockedTroopCount} 人方陣通行（需 {choke.RequiredClearanceTiles} 格，安全上限 {choke.MaxSafeUnitCount} 人）。",
                $"Choke point clearance is only {choke.MeasuredClearanceTiles} tiles (~{choke.MeasuredWidthUnits:F0} units); cannot fit {choke.BlockedTroopCount}-man formation (requires {choke.RequiredClearanceTiles} tiles, safe max {choke.MaxSafeUnitCount}).",
                WorldX: choke.Location.WorldX,
                WorldZ: choke.Location.WorldZ));
        }

        return issues;
    }
}
