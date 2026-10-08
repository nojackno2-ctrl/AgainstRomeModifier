namespace AgainstRomeMapEditor.Modules.Soundscape;

/// <summary>
/// 原生音效定義目錄與聲學空間衰減計算核心。
/// 提供原生環境音預設登錄表、精確空間幾何最短距離計算與多種衰減曲線模型。
/// </summary>
public sealed class SoundscapeZoneCatalog
{
    private readonly Dictionary<string, SoundDef> _registry = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>全域預設單例目錄。</summary>
    public static SoundscapeZoneCatalog Shared { get; } = CreateDefaultCatalog();

    public SoundscapeZoneCatalog()
    {
    }

    /// <summary>註冊自訂或原生音效定義。</summary>
    public void Register(SoundDef def)
    {
        _registry[def.SoundId] = def;
    }

    /// <summary>查詢音效定義，若無則回傳備用預設項。</summary>
    public SoundDef Resolve(string soundId)
    {
        if (_registry.TryGetValue(soundId, out var def))
            return def;

        return new SoundDef(
            soundId,
            soundId,
            AudioCategory.AmbientZone,
            SoundTriggerMode.ContinuousLoop);
    }

    /// <summary>列舉所有已註冊的音效定義。</summary>
    public IReadOnlyCollection<SoundDef> GetAllDefinitions() => _registry.Values;

    /// <summary>
    /// 計算空間點 P(px, pz) 到特定音效區域幾何外邊界的最短歐幾里得距離。
    /// 若點位於形狀內部，最短距離為 0。
    /// </summary>
    public static float CalculateDistanceToZone(float px, float pz, SoundscapeZone zone)
    {
        ArgumentNullException.ThrowIfNull(zone);

        return zone.ShapeType switch
        {
            SoundscapeShapeType.Point => CalculateDistanceToPoint(px, pz, zone.CenterWorldX, zone.CenterWorldZ),
            SoundscapeShapeType.Circle => CalculateDistanceToCircle(px, pz, zone.CenterWorldX, zone.CenterWorldZ, Math.Max(0f, zone.ParamA)),
            SoundscapeShapeType.OrientedRectangle => CalculateDistanceToOrientedRectangle(
                px, pz, zone.CenterWorldX, zone.CenterWorldZ,
                Math.Max(0f, zone.ParamA), Math.Max(0f, zone.ParamB), zone.ParamAngleDeg),
            SoundscapeShapeType.ConvexPolygon => CalculateDistanceToPolygon(px, pz, zone.PolygonVertices),
            _ => CalculateDistanceToPoint(px, pz, zone.CenterWorldX, zone.CenterWorldZ)
        };
    }

    /// <summary>
    /// 依據距離與半徑門檻，透過特定衰減曲線計算音量衰減乘數（0.0 ~ 1.0）。
    /// </summary>
    public static float CalculateAttenuation(
        float distance,
        float innerRadius,
        float outerRadius,
        AttenuationCurveKind curve)
    {
        if (distance <= innerRadius)
            return 1.0f;

        if (distance >= outerRadius || outerRadius <= innerRadius)
            return 0.0f;

        float range = outerRadius - innerRadius;
        float t = Math.Clamp((distance - innerRadius) / range, 0.0f, 1.0f);

        return curve switch
        {
            AttenuationCurveKind.Linear => 1.0f - t,
            AttenuationCurveKind.SmoothStep => 1.0f - (3.0f * t * t - 2.0f * t * t * t),
            AttenuationCurveKind.Logarithmic => Math.Clamp(1.0f - (float)(Math.Log(1.0 + 9.0 * t) / Math.Log(10.0)), 0.0f, 1.0f),
            AttenuationCurveKind.Exponential => (1.0f - t) * (1.0f - t),
            _ => 1.0f - t
        };
    }

    #region 幾何最短距離計算

    private static float CalculateDistanceToPoint(float px, float pz, float cx, float cz)
    {
        float dx = px - cx;
        float dz = pz - cz;
        return MathF.Sqrt(dx * dx + dz * dz);
    }

    private static float CalculateDistanceToCircle(float px, float pz, float cx, float cz, float radius)
    {
        float distToCenter = CalculateDistanceToPoint(px, pz, cx, cz);
        return Math.Max(0f, distToCenter - radius);
    }

    private static float CalculateDistanceToOrientedRectangle(
        float px, float pz, float cx, float cz, float halfW, float halfH, float angleDeg)
    {
        float dx = px - cx;
        float dz = pz - cz;

        // 旋轉回局部軸對齊空間 (-angle)
        float rad = -angleDeg * (MathF.PI / 180.0f);
        float cos = MathF.Cos(rad);
        float sin = MathF.Sin(rad);

        float lx = dx * cos - dz * sin;
        float lz = dx * sin + dz * cos;

        // 計算點到 AABB 的最近距離
        float qx = Math.Max(0f, MathF.Abs(lx) - halfW);
        float qz = Math.Max(0f, MathF.Abs(lz) - halfH);

        return MathF.Sqrt(qx * qx + qz * qz);
    }

    private static float CalculateDistanceToPolygon(float px, float pz, IReadOnlyList<SoundscapeVertex> vertices)
    {
        if (vertices == null || vertices.Count < 3)
            return float.MaxValue;

        // 1. 若點在多邊形內部，距離為 0
        if (IsPointInsidePolygon(px, pz, vertices))
            return 0.0f;

        // 2. 外部點到各邊線段之最短距離的最小值
        float minDistance = float.MaxValue;
        int count = vertices.Count;

        for (int i = 0; i < count; i++)
        {
            var p1 = vertices[i];
            var p2 = vertices[(i + 1) % count];

            float dist = DistancePointToSegment(px, pz, p1.X, p1.Z, p2.X, p2.Z);
            if (dist < minDistance)
                minDistance = dist;
        }

        return minDistance;
    }

    /// <summary>
    /// 射線法（Ray-Casting Algorithm）判斷點是否位於多邊形內部。
    /// </summary>
    public static bool IsPointInsidePolygon(float px, float pz, IReadOnlyList<SoundscapeVertex> vertices)
    {
        if (vertices == null || vertices.Count < 3)
            return false;

        bool inside = false;
        int count = vertices.Count;

        for (int i = 0, j = count - 1; i < count; j = i++)
        {
            float xi = vertices[i].X, zi = vertices[i].Z;
            float xj = vertices[j].X, zj = vertices[j].Z;

            bool intersect = ((zi > pz) != (zj > pz)) &&
                (px < (xj - xi) * (pz - zi) / (zj - zi + 1e-7f) + xi);

            if (intersect)
                inside = !inside;
        }

        return inside;
    }

    /// <summary>
    /// 計算二維點到線段 (x1, z1)-(x2, z2) 的最短歐幾里得距離。
    /// </summary>
    public static float DistancePointToSegment(float px, float pz, float x1, float z1, float x2, float z2)
    {
        float l2 = (x2 - x1) * (x2 - x1) + (z2 - z1) * (z2 - z1);
        if (l2 <= 1e-6f)
            return MathF.Sqrt((px - x1) * (px - x1) + (pz - z1) * (pz - z1));

        // 投影比例 t
        float t = ((px - x1) * (x2 - x1) + (pz - z1) * (z2 - z1)) / l2;
        t = Math.Clamp(t, 0.0f, 1.0f);

        float projX = x1 + t * (x2 - x1);
        float projZ = z1 + t * (z2 - z1);

        float dx = px - projX;
        float dz = pz - projZ;
        return MathF.Sqrt(dx * dx + dz * dz);
    }

    #endregion

    /// <summary>
    /// 初始化《反抗羅馬》預設環境音項目庫。
    /// </summary>
    private static SoundscapeZoneCatalog CreateDefaultCatalog()
    {
        var catalog = new SoundscapeZoneCatalog();

        // 森林與生態系
        catalog.Register(new SoundDef("Amb_Forest_Dense_Day", "茂密森林（日間鳥鳴風動）", AudioCategory.Vegetation, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.85f, DefaultInnerRadius: 300f, DefaultOuterRadius: 1000f, DayOnly: true));
        catalog.Register(new SoundDef("Amb_Forest_Dense_Night", "茂密森林（夜間蟋蟀貓頭鷹）", AudioCategory.Vegetation, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.75f, DefaultInnerRadius: 300f, DefaultOuterRadius: 1000f, NightOnly: true));
        catalog.Register(new SoundDef("Amb_Forest_Light", "稀疏林地（微風枝葉作響）", AudioCategory.Vegetation, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.65f, DefaultInnerRadius: 200f, DefaultOuterRadius: 750f));
        catalog.Register(new SoundDef("Amb_Forest_Birds", "林冠飛鳥間歇鳴叫", AudioCategory.Vegetation, SoundTriggerMode.RandomStochasticInterval, DefaultVolume: 0.9f, DefaultInnerRadius: 150f, DefaultOuterRadius: 600f, MinIntervalSeconds: 6f, MaxIntervalSeconds: 22f));

        // 水系與水文
        catalog.Register(new SoundDef("Amb_River_Gentle", "平緩河流潺潺水聲", AudioCategory.Hydrology, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.70f, DefaultInnerRadius: 160f, DefaultOuterRadius: 550f));
        catalog.Register(new SoundDef("Amb_River_Rapid", "峽谷急流湍急咆哮", AudioCategory.Hydrology, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.88f, DefaultInnerRadius: 220f, DefaultOuterRadius: 750f));
        catalog.Register(new SoundDef("Amb_Waterfall_Roar", "斷崖瀑布萬鈞轟鳴", AudioCategory.Hydrology, SoundTriggerMode.ContinuousLoop, DefaultVolume: 1.00f, DefaultInnerRadius: 350f, DefaultOuterRadius: 1200f));
        catalog.Register(new SoundDef("Amb_Coast_Waves", "海岸邊際浪濤拍岸", AudioCategory.Hydrology, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.82f, DefaultInnerRadius: 250f, DefaultOuterRadius: 900f));
        catalog.Register(new SoundDef("Amb_Pond_Marsh", "靜水池塘水泡蛙鳴", AudioCategory.Hydrology, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.60f, DefaultInnerRadius: 120f, DefaultOuterRadius: 450f));

        // 地形與開闊氣候
        catalog.Register(new SoundDef("Amb_Plains_Wind", "開闊荒野平原長風", AudioCategory.AmbientGlobal, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.55f, DefaultInnerRadius: 500f, DefaultOuterRadius: 1800f));
        catalog.Register(new SoundDef("Amb_Tundra_Blizzard", "極北雪原酷寒暴風", AudioCategory.AmbientZone, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.90f, DefaultInnerRadius: 400f, DefaultOuterRadius: 1400f));
        catalog.Register(new SoundDef("Amb_Mountain_Wind", "山嶽之巔孤寂寒風", AudioCategory.AmbientZone, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.75f, DefaultInnerRadius: 300f, DefaultOuterRadius: 1100f));

        // 聚落與定點事件
        catalog.Register(new SoundDef("Snd_Campfire_Crackling", "營火柴薪劈啪燃燒", AudioCategory.PointEmitter, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.75f, DefaultInnerRadius: 80f, DefaultOuterRadius: 350f));
        catalog.Register(new SoundDef("Snd_Blacksmith_Anvil", "鐵匠鋪鍛鐵打砧迴響", AudioCategory.Settlement, SoundTriggerMode.RandomStochasticInterval, DefaultVolume: 0.85f, DefaultInnerRadius: 120f, DefaultOuterRadius: 500f, MinIntervalSeconds: 3f, MaxIntervalSeconds: 9f));
        catalog.Register(new SoundDef("Snd_Shrine_Chant", "祭壇遠古呢喃神諭", AudioCategory.Settlement, SoundTriggerMode.ContinuousLoop, DefaultVolume: 0.65f, DefaultInnerRadius: 100f, DefaultOuterRadius: 400f));

        return catalog;
    }
}
