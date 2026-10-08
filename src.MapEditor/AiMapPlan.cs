using System.Text.Json;
using System.Text.Json.Serialization;

namespace AgainstRomeMapEditor;

/// <summary>
/// AI 產生的「地圖計畫」：只描述地形特徵，不含任何檔案內容。
/// 座標一律為 0–63 的 tile 座標；高度單位為 boden.bmp 綠通道（0–255）。
/// 實際寫入由 <see cref="AiMapPlanApplier"/> 以既有的高度／材質／通行編輯完成，因此可預覽、復原並走同一儲存交易。
/// </summary>
internal sealed class AiMapPlan
{
    [JsonIgnore] internal AiMapEditScope? EditScope { get; set; }
    [JsonPropertyName("summary")] public string? Summary { get; set; }
    /// <summary>若有值，先把整張地圖的地面整平到此高度，再疊加各特徵。</summary>
    [JsonPropertyName("baseHeight")] public int? BaseHeight { get; set; }
    [JsonPropertyName("baseMaterial")] public string? BaseMaterial { get; set; }
    [JsonPropertyName("features")] public List<AiMapFeature> Features { get; set; } = new();

    public const int MaxFeatures = 24;

    public static readonly string[] FeatureTypes = ["hill", "mountain", "valley", "plateau", "lake", "ridge", "river", "material", "blocked", "passable"];

    /// <summary>
    /// 方位名稱 → tile 座標。小型本機模型對數字座標的方位常判斷錯誤，因此計畫以方位名稱為主，
    /// 由程式確定性換算；x/y 只在沒有方位時使用。
    /// </summary>
    public static readonly IReadOnlyDictionary<string, (int X, int Y)> Locations = new Dictionary<string, (int X, int Y)>(StringComparer.OrdinalIgnoreCase)
    {
        ["center"] = (32, 32), ["north"] = (32, 9), ["north-east"] = (52, 12), ["east"] = (55, 32), ["south-east"] = (52, 52),
        ["south"] = (32, 55), ["south-west"] = (12, 52), ["west"] = (9, 32), ["north-west"] = (12, 12),
        ["north-center"] = (32, 20), ["east-center"] = (44, 32), ["south-center"] = (32, 44), ["west-center"] = (20, 32),
    };

    private static readonly JsonSerializerOptions Options = new() { PropertyNameCaseInsensitive = true, ReadCommentHandling = JsonCommentHandling.Skip, AllowTrailingCommas = true };

    /// <summary>解析 AI 回應（容忍前後雜訊與 ```json 區塊），並把所有數值夾到合法範圍；無法解析時丟出 <see cref="InvalidDataException"/>。</summary>
    public static AiMapPlan Parse(string response, IReadOnlyCollection<string> materialIds)
    {
        if (string.IsNullOrWhiteSpace(response)) throw new InvalidDataException("AI 沒有回傳內容。");
        int start = response.IndexOf('{'), end = response.LastIndexOf('}');
        if (start < 0 || end <= start) throw new InvalidDataException("AI 回應不是 JSON 地圖計畫。");
        AiMapPlan? plan;
        try { plan = JsonSerializer.Deserialize<AiMapPlan>(response[start..(end + 1)], Options); }
        catch (JsonException ex) { throw new InvalidDataException("AI 回應的 JSON 無法解析：" + ex.Message, ex); }
        if (plan is null) throw new InvalidDataException("AI 回應的地圖計畫是空的。");
        plan.Normalize(materialIds);
        if (plan.Features.Count == 0 && plan.BaseHeight is null && plan.BaseMaterial is null) throw new InvalidDataException("AI 計畫沒有任何可套用的地形特徵。");
        return plan;
    }

    private void Normalize(IReadOnlyCollection<string> materialIds)
    {
        var known = new HashSet<string>(materialIds, StringComparer.OrdinalIgnoreCase);
        string? Material(string? id) => id is not null && known.Contains(id.Trim()) ? materialIds.First(item => item.Equals(id.Trim(), StringComparison.OrdinalIgnoreCase)) : null;
        BaseHeight = BaseHeight is int baseHeight ? Math.Clamp(baseHeight, 0, 255) : null;
        BaseMaterial = Material(BaseMaterial);
        Summary = Summary?.Trim();
        Features = Features
            .Where(feature => feature.Type is not null && FeatureTypes.Contains(feature.Type.Trim().ToLowerInvariant()))
            .Take(MaxFeatures)
            .Select(feature =>
            {
                feature.Type = feature.Type!.Trim().ToLowerInvariant();
                if (feature.Location is not null && Locations.TryGetValue(feature.Location.Trim(), out (int X, int Y) from)) (feature.X, feature.Y) = from;
                if (feature.ToLocation is not null && Locations.TryGetValue(feature.ToLocation.Trim(), out (int X, int Y) to)) (feature.X2, feature.Y2) = to;
                feature.X = Math.Clamp(feature.X, 0, 63); feature.Y = Math.Clamp(feature.Y, 0, 63);
                feature.X2 = feature.X2 is int x2 ? Math.Clamp(x2, 0, 63) : null;
                feature.Y2 = feature.Y2 is int y2 ? Math.Clamp(y2, 0, 63) : null;
                feature.Radius = Math.Clamp(feature.Radius <= 0 ? 4 : feature.Radius, 1, 32);
                feature.Amount = feature.Amount is int amount ? Math.Clamp(amount, 0, 255) : null;
                feature.Material = Material(feature.Material);
                return feature;
            })
            .Where(feature => feature.Type != "material" || feature.Material is not null)
            // 河流與山脊必須有不同的終點；小型模型偶爾只給起點，畫出來會變成坑洞或孤峰。
            .Where(feature => feature.Type is not ("river" or "ridge") || (feature.X2 is int x2 && feature.Y2 is int y2 && (x2 != feature.X || y2 != feature.Y)))
            // 模型偶爾陷入重複輸出同一特徵；完全相同的項目只保留一次。
            .DistinctBy(feature => (feature.Type, feature.X, feature.Y, feature.X2, feature.Y2, feature.Radius, feature.Amount, feature.Material))
            .ToList();
    }

    /// <summary>給 Ollama `format` 使用的 JSON Schema，讓模型只能輸出可解析的計畫。</summary>
    public static object JsonSchema(IReadOnlyCollection<string> materialIds) => new Dictionary<string, object>
    {
        ["type"] = "object",
        ["properties"] = new Dictionary<string, object>
        {
            ["summary"] = new Dictionary<string, object> { ["type"] = "string" },
            ["baseHeight"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 0, ["maximum"] = 255 },
            ["baseMaterial"] = new Dictionary<string, object> { ["type"] = "string", ["enum"] = materialIds.ToArray() },
            ["features"] = new Dictionary<string, object>
            {
                ["type"] = "array",
                ["maxItems"] = MaxFeatures,
                ["items"] = new Dictionary<string, object>
                {
                    ["type"] = "object",
                    ["properties"] = new Dictionary<string, object>
                    {
                        ["type"] = new Dictionary<string, object> { ["type"] = "string", ["enum"] = FeatureTypes },
                        ["location"] = new Dictionary<string, object> { ["type"] = "string", ["enum"] = Locations.Keys.ToArray() },
                        ["toLocation"] = new Dictionary<string, object> { ["type"] = "string", ["enum"] = Locations.Keys.ToArray() },
                        ["x"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 0, ["maximum"] = 63 },
                        ["y"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 0, ["maximum"] = 63 },
                        ["x2"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 0, ["maximum"] = 63 },
                        ["y2"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 0, ["maximum"] = 63 },
                        ["radius"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 1, ["maximum"] = 32 },
                        ["amount"] = new Dictionary<string, object> { ["type"] = "integer", ["minimum"] = 0, ["maximum"] = 255 },
                        ["material"] = new Dictionary<string, object> { ["type"] = "string", ["enum"] = materialIds.ToArray() },
                    },
                    ["required"] = new[] { "type", "location", "radius" },
                },
            },
        },
        ["required"] = new[] { "summary", "features" },
    };
}

internal sealed class AiMapFeature
{
    [JsonPropertyName("type")] public string? Type { get; set; }
    /// <summary>方位名稱（見 <see cref="AiMapPlan.Locations"/>）；有值時覆寫 x/y。</summary>
    [JsonPropertyName("location")] public string? Location { get; set; }
    /// <summary>線狀特徵（河流、山脊、阻擋線）的終點方位；有值時覆寫 x2/y2。</summary>
    [JsonPropertyName("toLocation")] public string? ToLocation { get; set; }
    [JsonPropertyName("x")] public int X { get; set; }
    [JsonPropertyName("y")] public int Y { get; set; }
    [JsonPropertyName("x2")] public int? X2 { get; set; }
    [JsonPropertyName("y2")] public int? Y2 { get; set; }
    [JsonPropertyName("radius")] public int Radius { get; set; }
    /// <summary>山丘／山脈／河谷的高度變化量，或高台的目標高度（0–255）。</summary>
    [JsonPropertyName("amount")] public int? Amount { get; set; }
    [JsonPropertyName("material")] public string? Material { get; set; }
}

internal sealed record AiMapApplyResult(int HeightSamplesChanged, int MaterialStrokes, int RejectedMaterialStrokes, int CollisionPixelsChanged);

/// <summary>把 <see cref="AiMapPlan"/> 確定性地轉成高度、材質與通行編輯（同一計畫永遠得到同一結果）。</summary>
internal static class AiMapPlanApplier
{
    /// <param name="plan">已正規化的 AI 地圖計畫。</param>
    /// <param name="heights">要修改的高度／通行狀態（變更留在待提交筆畫，由呼叫端 CommitStroke）。</param>
    /// <param name="dimension">地圖 tile 邊長（64）。</param>
    /// <param name="waterLevelSample">水位換算成 boden.bmp 綠通道單位（Waterlevel ÷ Heightmapstep）。</param>
    /// <param name="paintMaterial">以 tile 座標圓心與半徑塗材質；回傳是否成功（原版 transition 無法表達時會被拒絕並自動回滾）。</param>
    public static AiMapApplyResult Apply(AiMapPlan plan, TerrainHeightEditSession heights, int dimension, float waterLevelSample,
        Func<string, float, float, float, bool> paintMaterial)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(heights);
        float step = (heights.VertexSize - 1) / (float)dimension;
        var field = new float[heights.VertexSize * heights.VertexSize];
        for (int index = 0; index < field.Length; index++) field[index] = heights.Heights[index];
        if (plan.BaseHeight is int baseHeight) Array.Fill(field, baseHeight);

        foreach (AiMapFeature feature in plan.Features)
            ApplyHeightFeature(feature, field, heights.VertexSize, step, waterLevelSample);

        int heightChanges = heights.TransformHeights((x, y, before) => plan.EditScope is null || plan.EditScope.AllowsVertex(x / step, y / step, dimension)
            ? field[y * heights.VertexSize + x] : before).Count;

        int strokes = 0, rejected = 0;
        if (plan.BaseMaterial is not null)
        {
            strokes++;
            if (!paintMaterial(plan.BaseMaterial, dimension / 2f, dimension / 2f, dimension)) rejected++;
        }
        foreach (AiMapFeature feature in plan.Features.Where(item => item.Material is not null))
        {
            // 「material」以圓形塗色；其他特徵附帶的材質沿特徵塗上（河岸沙地、山上岩地），線狀特徵逐點塗並略寬於地形變化。
            bool linear = feature.Type is "river" or "ridge" or "blocked" or "passable";
            float radius = linear ? feature.Radius + 1 : feature.Type == "material" ? feature.Radius : Math.Max(1, feature.Radius * .6f);
            IEnumerable<(float X, float Y)> points = linear ? SegmentPoints(feature).Where((_, index) => index % 2 == 0) : [(feature.X, feature.Y)];
            foreach ((float x, float y) in points)
            {
                strokes++;
                if (!paintMaterial(feature.Material!, x + .5f, y + .5f, radius)) rejected++;
            }
        }

        int collision = 0;
        if (heights.HasCollision && (plan.EditScope?.EditPassability ?? true))
        {
            float collisionStep = heights.CollisionSize / (float)dimension;
            foreach (AiMapFeature feature in plan.Features.Where(item => item.Type is "blocked" or "passable"))
            {
                var operation = feature.Type == "blocked" ? TerrainCollisionOperation.Block : TerrainCollisionOperation.Clear;
                foreach ((float x, float y) in SegmentPoints(feature))
                    collision += heights.PaintCollision((x + .5f) * collisionStep, (y + .5f) * collisionStep, feature.Radius * collisionStep, operation,
                        (px, py) => plan.EditScope is null || plan.EditScope.AllowsTile((int)((px + .5f) / collisionStep), (int)((py + .5f) / collisionStep))).Count;
            }
        }
        return new AiMapApplyResult(heightChanges, strokes, rejected, collision);
    }

    private static void ApplyHeightFeature(AiMapFeature feature, float[] field, int size, float step, float water)
    {
        float cx = (feature.X + .5f) * step, cy = (feature.Y + .5f) * step;
        float radius = Math.Max(1, feature.Radius) * step;
        switch (feature.Type)
        {
            case "hill":
            case "mountain":
            case "valley":
            {
                float amount = feature.Amount ?? (feature.Type == "mountain" ? 110 : feature.Type == "hill" ? 45 : 40);
                float sign = feature.Type == "valley" ? -1 : 1;
                ForEach(field, size, (x, y, value) =>
                {
                    float d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                    return value + sign * amount * MathF.Exp(-d2 / (2 * (radius / 2) * (radius / 2)));
                });
                break;
            }
            case "plateau":
            {
                float target = feature.Amount ?? 150;
                ForEach(field, size, (x, y, value) =>
                {
                    float t = SmoothFalloff(MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy)), radius);
                    return value + (target - value) * t;
                });
                break;
            }
            case "lake":
            {
                // 湖底低於水位 amount（預設 12），邊緣以平滑曲線接回原地形，避免出現懸崖。
                float bottom = water - (feature.Amount is int depth ? Math.Max(2, depth) : 12);
                ForEach(field, size, (x, y, value) =>
                {
                    float t = SmoothFalloff(MathF.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy)), radius);
                    return t <= 0 ? value : Math.Min(value, value + (bottom - value) * t);
                });
                break;
            }
            case "ridge":
            case "river":
            {
                float ex = ((feature.X2 ?? feature.X) + .5f) * step, ey = ((feature.Y2 ?? feature.Y) + .5f) * step;
                float halfWidth = radius;
                bool river = feature.Type == "river";
                float amount = feature.Amount ?? (river ? 10 : 70);
                ForEach(field, size, (x, y, value) =>
                {
                    float t = SmoothFalloff(DistanceToSegment(x, y, cx, cy, ex, ey), halfWidth);
                    if (t <= 0) return value;
                    return river ? Math.Min(value, value + (water - amount - value) * t) : value + amount * t;
                });
                break;
            }
        }
    }

    private static IEnumerable<(float X, float Y)> SegmentPoints(AiMapFeature feature)
    {
        if (feature.X2 is not int x2 || feature.Y2 is not int y2) { yield return (feature.X, feature.Y); yield break; }
        yield return (feature.X, feature.Y);
        foreach ((int x, int y) in TerrainStrokePath.Between(feature.X, feature.Y, x2, y2)) yield return (x, y);
    }

    private static void ForEach(float[] field, int size, Func<int, int, float, float> transform)
    {
        for (int y = 0; y < size; y++) for (int x = 0; x < size; x++) field[y * size + x] = transform(x, y, field[y * size + x]);
    }

    /// <summary>距離 0 為 1、達到半徑為 0 的平滑曲線（smoothstep）。</summary>
    internal static float SmoothFalloff(float distance, float radius)
    {
        if (distance >= radius) return 0;
        float t = 1 - distance / radius;
        return t * t * (3 - 2 * t);
    }

    internal static float DistanceToSegment(float px, float py, float ax, float ay, float bx, float by)
    {
        float dx = bx - ax, dy = by - ay, lengthSquared = dx * dx + dy * dy;
        float t = lengthSquared <= 0 ? 0 : Math.Clamp(((px - ax) * dx + (py - ay) * dy) / lengthSquared, 0, 1);
        float nx = ax + t * dx - px, ny = ay + t * dy - py;
        return MathF.Sqrt(nx * nx + ny * ny);
    }
}
