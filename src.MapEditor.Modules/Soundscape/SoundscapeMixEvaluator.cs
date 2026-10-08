namespace AgainstRomeMapEditor.Modules.Soundscape;

/// <summary>
/// 空間環境音混音評估器與 Audio Gizmo 圖元產生器。
/// 支援虛擬聆聽者視角的實時 3D 空間立體聲（Stereo Panning）計算、
/// 通道預算診斷、重疊混音衰減試算與聲學熱度圖採樣。
/// </summary>
public sealed class SoundscapeMixEvaluator
{
    private readonly SoundscapeZoneCatalog _catalog;

    /// <summary>關聯的音效目錄。</summary>
    public SoundscapeZoneCatalog Catalog => _catalog;

    public SoundscapeMixEvaluator(SoundscapeZoneCatalog? catalog = null)
    {
        _catalog = catalog ?? SoundscapeZoneCatalog.Shared;
    }

    /// <summary>
    /// 在虛擬聆聽者（Listener）坐標處，評估所有有效環境音源的重疊混音、立體聲方位角與聲道負載。
    /// </summary>
    public SoundscapeMixResult EvaluateMix(
        float listenerX,
        float listenerY,
        float listenerZ,
        float listenerYawDeg,
        IReadOnlyList<SoundscapeZone> zones,
        int maxHardwareChannels = 24)
    {
        ArgumentNullException.ThrowIfNull(zones);

        var audibleList = new List<AudibleChannelEvaluation>();
        float sumLeftSq = 0f;
        float sumRightSq = 0f;
        float sumTotalSq = 0f;

        foreach (var zone in zones)
        {
            if (!zone.Flags.HasFlag(SoundscapeZoneFlags.IsActive))
                continue;

            float dist = SoundscapeZoneCatalog.CalculateDistanceToZone(listenerX, listenerZ, zone);
            float falloff = SoundscapeZoneCatalog.CalculateAttenuation(
                dist, zone.InnerRadius, zone.OuterRadius, zone.AttenuationCurve);

            float effectiveVol = zone.BaseVolume * falloff;
            if (effectiveVol < 0.001f)
                continue;

            // 計算相對聆聽者朝向之方位角（Azimuth）
            float dx = zone.CenterWorldX - listenerX;
            float dz = zone.CenterWorldZ - listenerZ;
            float worldAngleDeg = MathF.Atan2(dx, dz) * (180.0f / MathF.PI);
            float relAngleDeg = NormalizeAngle(worldAngleDeg - listenerYawDeg);

            // 立體聲 Panning 權重計算（微量交叉反饋 0.05 避免耳機單耳完全靜默）
            float rad = relAngleDeg * (MathF.PI / 180.0f);
            float lateral = MathF.Sin(rad); // -1 (完全在左) ~ +1 (完全在右)
            float panR = Math.Clamp(0.5f + 0.45f * lateral, 0.05f, 0.95f);
            float panL = 1.0f - panR;

            float volL = effectiveVol * panL;
            float volR = effectiveVol * panR;

            sumLeftSq += volL * volL;
            sumRightSq += volR * volR;
            sumTotalSq += effectiveVol * effectiveVol;

            audibleList.Add(new AudibleChannelEvaluation(
                zone.Id,
                zone.SoundDefId,
                zone.Category,
                dist,
                falloff,
                effectiveVol,
                panL,
                panR,
                relAngleDeg,
                zone.Priority));
        }

        bool budgetExceeded = audibleList.Count > maxHardwareChannels;

        // 若超出聲道限制，按優先級與有效音量排序並保留前 maxHardwareChannels 個
        if (budgetExceeded)
        {
            audibleList = audibleList
                .OrderByDescending(c => c.Priority)
                .ThenByDescending(c => c.EffectiveVolume)
                .Take(maxHardwareChannels)
                .ToList();
        }

        float perceivedTotal = Math.Min(1.0f, MathF.Sqrt(sumTotalSq));
        float masterL = Math.Min(1.0f, MathF.Sqrt(sumLeftSq));
        float masterR = Math.Min(1.0f, MathF.Sqrt(sumRightSq));

        return new SoundscapeMixResult(
            listenerX, listenerY, listenerZ,
            listenerYawDeg,
            perceivedTotal,
            masterL,
            masterR,
            audibleList.Count,
            budgetExceeded,
            audibleList);
    }

    /// <summary>
    /// 為 2D 畫布與 3D 視圖建構視覺化 Audio Gizmo 圖元清單。
    /// </summary>
    public IReadOnlyList<AudioGizmoRepresentation> BuildGizmos(
        IReadOnlyList<SoundscapeZone> zones,
        Guid? selectedZoneId = null)
    {
        ArgumentNullException.ThrowIfNull(zones);

        var gizmos = new List<AudioGizmoRepresentation>(zones.Count);
        foreach (var z in zones)
        {
            bool isSelected = selectedZoneId.HasValue && z.Id == selectedZoneId.Value;
            uint colorRgba = GetCategoryColor(z.Category, isSelected);

            var core = BuildCoreBoundary(z);
            var falloff = BuildFalloffBoundary(z);

            gizmos.Add(new AudioGizmoRepresentation(
                z.Id,
                z.Name,
                z.SoundDefId,
                z.Category,
                z.ShapeType,
                z.CenterWorldX,
                z.CenterWorldY,
                z.CenterWorldZ,
                z.InnerRadius,
                z.OuterRadius,
                core,
                falloff,
                colorRgba,
                isSelected));
        }

        return gizmos;
    }

    /// <summary>
    /// 採樣聲學空間音量強度熱度圖（供編輯器全域分析、盲區/嘈雜區診斷與視覺覆蓋層）。
    /// </summary>
    public float[,] GenerateAcousticHeatmap(
        IReadOnlyList<SoundscapeZone> zones,
        float worldMinX, float worldMinZ,
        float worldMaxX, float worldMaxZ,
        int sampleGridResolution = 64)
    {
        ArgumentNullException.ThrowIfNull(zones);

        sampleGridResolution = Math.Clamp(sampleGridResolution, 8, 256);
        float[,] heatmap = new float[sampleGridResolution, sampleGridResolution];

        float stepX = (worldMaxX - worldMinX) / sampleGridResolution;
        float stepZ = (worldMaxZ - worldMinZ) / sampleGridResolution;

        for (int gy = 0; gy < sampleGridResolution; gy++)
        {
            float pz = worldMinZ + (gy + 0.5f) * stepZ;
            for (int gx = 0; gx < sampleGridResolution; gx++)
            {
                float px = worldMinX + (gx + 0.5f) * stepX;
                float sumSq = 0f;

                foreach (var z in zones)
                {
                    if (!z.Flags.HasFlag(SoundscapeZoneFlags.IsActive))
                        continue;

                    float dist = SoundscapeZoneCatalog.CalculateDistanceToZone(px, pz, z);
                    float att = SoundscapeZoneCatalog.CalculateAttenuation(dist, z.InnerRadius, z.OuterRadius, z.AttenuationCurve);
                    float vol = z.BaseVolume * att;
                    sumSq += vol * vol;
                }

                heatmap[gy, gx] = Math.Min(1.0f, MathF.Sqrt(sumSq));
            }
        }

        return heatmap;
    }

    #region Gizmo 幾何頂點輔助

    private static IReadOnlyList<SoundscapeVertex> BuildCoreBoundary(SoundscapeZone zone)
    {
        return zone.ShapeType switch
        {
            SoundscapeShapeType.Point => [new SoundscapeVertex(zone.CenterWorldX, zone.CenterWorldZ)],
            SoundscapeShapeType.Circle => GenerateCircleVertices(zone.CenterWorldX, zone.CenterWorldZ, Math.Max(10f, zone.ParamA), 24),
            SoundscapeShapeType.OrientedRectangle => GenerateOrientedBoxVertices(zone.CenterWorldX, zone.CenterWorldZ, zone.ParamA, zone.ParamB, zone.ParamAngleDeg),
            SoundscapeShapeType.ConvexPolygon => zone.PolygonVertices.ToList(),
            _ => [new SoundscapeVertex(zone.CenterWorldX, zone.CenterWorldZ)]
        };
    }

    private static IReadOnlyList<SoundscapeVertex> BuildFalloffBoundary(SoundscapeZone zone)
    {
        float totalR = zone.OuterRadius;
        return zone.ShapeType switch
        {
            SoundscapeShapeType.Point => GenerateCircleVertices(zone.CenterWorldX, zone.CenterWorldZ, totalR, 24),
            SoundscapeShapeType.Circle => GenerateCircleVertices(zone.CenterWorldX, zone.CenterWorldZ, Math.Max(10f, zone.ParamA) + totalR, 24),
            SoundscapeShapeType.OrientedRectangle => GenerateOrientedBoxVertices(zone.CenterWorldX, zone.CenterWorldZ, zone.ParamA + totalR, zone.ParamB + totalR, zone.ParamAngleDeg),
            SoundscapeShapeType.ConvexPolygon => ExpandPolygon(zone.PolygonVertices, totalR),
            _ => GenerateCircleVertices(zone.CenterWorldX, zone.CenterWorldZ, totalR, 24)
        };
    }

    private static List<SoundscapeVertex> GenerateCircleVertices(float cx, float cz, float radius, int segments)
    {
        var list = new List<SoundscapeVertex>(segments);
        for (int i = 0; i < segments; i++)
        {
            float theta = i * (2.0f * MathF.PI / segments);
            list.Add(new SoundscapeVertex(cx + MathF.Cos(theta) * radius, cz + MathF.Sin(theta) * radius));
        }
        return list;
    }

    private static List<SoundscapeVertex> GenerateOrientedBoxVertices(float cx, float cz, float halfW, float halfH, float angleDeg)
    {
        float rad = angleDeg * (MathF.PI / 180.0f);
        float cos = MathF.Cos(rad);
        float sin = MathF.Sin(rad);

        ReadOnlySpan<(float X, float Z)> localCorners = [
            (-halfW, -halfH),
            (halfW, -halfH),
            (halfW, halfH),
            (-halfW, halfH)
        ];

        var worldCorners = new List<SoundscapeVertex>(4);
        foreach (var lc in localCorners)
        {
            float wx = cx + (lc.X * cos - lc.Z * sin);
            float wz = cz + (lc.X * sin + lc.Z * cos);
            worldCorners.Add(new SoundscapeVertex(wx, wz));
        }

        return worldCorners;
    }

    private static List<SoundscapeVertex> ExpandPolygon(IReadOnlyList<SoundscapeVertex> verts, float expansion)
    {
        if (verts == null || verts.Count < 3)
            return [];

        // 簡化向外擴展：以頂點質心為基準等比例徑向外推
        float sumX = verts.Average(v => v.X);
        float sumZ = verts.Average(v => v.Z);

        var expanded = new List<SoundscapeVertex>(verts.Count);
        foreach (var v in verts)
        {
            float dx = v.X - sumX;
            float dz = v.Z - sumZ;
            float len = MathF.Sqrt(dx * dx + dz * dz);
            if (len > 1e-4f)
            {
                float factor = (len + expansion) / len;
                expanded.Add(new SoundscapeVertex(sumX + dx * factor, sumZ + dz * factor));
            }
            else
            {
                expanded.Add(v);
            }
        }

        return expanded;
    }

    private static uint GetCategoryColor(AudioCategory category, bool isSelected)
    {
        if (isSelected)
            return 0xFFFFE000; // 選中高亮金黃色

        return category switch
        {
            AudioCategory.Hydrology => 0xFF00BFFF,   // 水文天藍色
            AudioCategory.Vegetation => 0xFF32CD32,  // 森林翠綠色
            AudioCategory.PointEmitter => 0xFFFF4500,// 定點發聲橙紅色
            AudioCategory.Settlement => 0xFFFF8C00,  // 聚落暖金橙
            AudioCategory.WeatherFx => 0xFF4682B4,   // 天氣鐵青藍
            AudioCategory.AmbientGlobal => 0xFFBA55D3,// 全域淡紫色
            _ => 0xFF87CEEB                          // 預設淡天藍
        };
    }

    private static float NormalizeAngle(float deg)
    {
        while (deg > 180.0f) deg -= 360.0f;
        while (deg <= -180.0f) deg += 360.0f;
        return deg;
    }

    #endregion
}
