using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Placement;

/// <summary>原生 formdef 線段取樣；不包含碰撞、尋路、地表高度或動畫。詳見 spawn-formations.md。</summary>
public static class FormationLayout
{
    public const float InitialSpacing = 320;
    public const int MaximumScenarioMembers = 20;
    public static Vector2 BannerOffset => Vector2.Zero;

    /// <summary>
    /// 回傳依成員索引排列的世界 X/Z 偏移。definition 必須來自離線 formdef.dau；
    /// 不以平方根網格猜測 formDef=0。spacing 是整個定義的倍率，不是相鄰士兵距離。
    /// </summary>
    public static IReadOnlyList<Vector2> Create(int count, float angleDegrees,
        FormationLayoutDefinition definition, float spacing = InitialSpacing)
    {
        ArgumentNullException.ThrowIfNull(definition);
        if (count is < 0 or > MaximumScenarioMembers) throw new ArgumentOutOfRangeException(nameof(count));
        if (!float.IsFinite(angleDegrees)) throw new ArgumentOutOfRangeException(nameof(angleDegrees));
        if (!float.IsFinite(spacing) || spacing <= 0) throw new ArgumentOutOfRangeException(nameof(spacing));
        if (count == 0) return Array.Empty<Vector2>();
        // Native 0x4BB06A..0x4BB094: reflect local X, rotate by 90 - scenario angle.
        double radians = (90 - (double)(angleDegrees % 360)) * Math.PI / 180;
        double sine = Math.Sin(radians), cosine = Math.Cos(radians);
        var result = new Vector2[count];
        float ratio = (float)definition.Segments.Count / count;
        for (int member = 0; member < count; member++)
        {
            // Native 0x4BAEDA..0x4BAF53 samples the midpoint of each member's interval.
            float position = ratio * (member + 0.5f);
            int segmentIndex = (int)MathF.Floor(position);
            FormationLayoutSegment segment = definition.Segments[segmentIndex];
            float fraction = position - segmentIndex;
            Vector2 local = segment.Start * (1 - fraction) + segment.End * fraction;
            double x = -local.X * spacing, z = local.Y * spacing;
            result[member] = new((float)(x * cosine - z * sine), (float)(x * sine + z * cosine));
        }
        return Array.AsReadOnly(result);
    }
}
